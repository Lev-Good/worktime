use crate::db;
use crate::AppState;
use chrono::Local;
use rusqlite::params;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub fn check(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut pending: Option<(String, String, &'static str)> = None;

    {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        if !db::setting_bool(&conn, "notificationsEnabled", true) {
            return;
        }

        let profile = db::active_profile_id(&conn);
        let now = Local::now();
        let clock = now.format("%H:%M").to_string();
        let work_start = db::setting_or(&conn, "workStart", "09:00");
        let work_end = db::setting_or(&conn, "workEnd", "18:00");
        let within_work_hours = clock >= work_start && clock <= work_end;

        let running: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM entries WHERE ended_at IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0;

        let today_seconds: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(duration_sec),0) FROM entries
                 WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3",
                params![profile, db::today_start(), db::today_end()],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let idle_minutes = db::setting_i64(&conn, "idleReminderMinutes", 45);
        let goal_hours = db::setting_f64(&conn, "dailyGoalHours", 8.0);

        let mut notices = state.notices.lock().unwrap_or_else(|e| e.into_inner());
        let now_ts = now.timestamp();

        if within_work_hours && !running && idle_minutes > 0 {
            let last = *notices.get("idle").unwrap_or(&0);
            let last_activity: Option<String> = conn
                .query_row(
                    "SELECT MAX(COALESCE(ended_at, started_at)) FROM entries WHERE profile_id = ?1 AND started_at >= ?2",
                    params![profile, db::today_start()],
                    |row| row.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten();
            let minutes_since_activity = last_activity
                .and_then(|value| db::parse_local(&value))
                .map(|time| (now.naive_local() - time).num_minutes())
                .unwrap_or(999);

            if minutes_since_activity >= idle_minutes && now_ts - last >= idle_minutes * 60 {
                notices.insert("idle".into(), now_ts);
                pending = Some((
                    "הטיימר כבוי".into(),
                    format!(
                        "עברו {} דקות מאז הרשומה האחרונה. אפשר להתחיל עבודה جديدة מהתפריט בשורת המשימות.",
                        minutes_since_activity.min(999)
                    ),
                    "reminder",
                ));
            }
        }

        if goal_hours > 0.0 && today_seconds as f64 >= goal_hours * 3600.0 {
            let last = *notices.get("goal").unwrap_or(&0);
            if now_ts - last > 12 * 3600 {
                notices.insert("goal".into(), now_ts);
                pending = Some((
                    "הגעתם ליעד היומי 🎯".into(),
                    format!(
                        "צברתם {} שעות עבודה היום (יעד: {} שעות).",
                        (today_seconds as f64 / 3600.0 * 10.0).round() / 10.0,
                        goal_hours
                    ),
                    "goal",
                ));
            }
        }
    }

    if let Some((title, body, kind)) = pending {
        {
            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
            let profile = db::active_profile_id(&conn);
            let _ = db::push_notification(&conn, &profile, &title, &body, kind);
        }
        let _ = app.notification().builder().title(&title).body(&body).show();
        let _ = app.emit("state-changed", ());
    }
}
