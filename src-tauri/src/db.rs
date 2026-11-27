use chrono::{Datelike, Duration, Local, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("theme", "dark"),
    ("currency", "ILS"),
    ("workdayHours", "8.5"),
    ("dailyGoalHours", "8"),
    ("workStart", "09:00"),
    ("workEnd", "18:00"),
    ("idleReminderMinutes", "45"),
    ("notificationsEnabled", "true"),
    ("roundingMinutes", "0"),
    ("autoStopHours", "12"),
    ("floatingEnabled", "false"),
    ("floatingSeconds", "true"),
    ("floatingOpacity", "0.94"),
    ("floatingAlwaysOnTop", "true"),
    ("floatingCompact", "false"),
    ("trayShowTimer", "true"),
    ("autostart", "false"),
    ("autoCheckUpdates", "true"),
    ("updateRepo", "Lev-Good/worktime"),
    ("backupEnabled", "false"),
    ("backupIntervalHours", "24"),
    ("gdriveClientId", ""),
    ("gdriveClientSecret", ""),
    ("gdriveFolderId", ""),
    ("gdriveAccount", ""),
    ("lastBackupAt", ""),
    ("lastUpdateCheck", ""),
    ("billableDefault", "true"),
    ("firstRun", "true"),
];

pub fn now_str() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn today_start() -> String {
    format!("{}T00:00:00", Local::now().format("%Y-%m-%d"))
}

pub fn today_end() -> String {
    format!("{}T23:59:59", Local::now().format("%Y-%m-%d"))
}

pub fn week_start() -> String {
    let now = Local::now();
    let dow = now.weekday().num_days_from_sunday() as i64;
    let date = now.date_naive() - Duration::days(dow);
    format!("{}T00:00:00", date.format("%Y-%m-%d"))
}

pub fn month_start() -> String {
    let now = Local::now();
    format!("{}-{:02}-01T00:00:00", now.year(), now.month())
}

pub fn add_days(date: &str, days: i64) -> String {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap_or_else(|_| Local::now().date_naive());
    (d + Duration::days(days)).format("%Y-%m-%d").to_string()
}

pub fn parse_local(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").ok()
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA foreign_keys=ON;
         PRAGMA busy_timeout=4000;",
    )?;
    migrate(&conn)?;
    seed(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            hourly_rate REAL NOT NULL DEFAULT 0,
            currency TEXT NOT NULL DEFAULT 'ILS',
            color TEXT NOT NULL DEFAULT '#7c9cff',
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS clients (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL,
            contact TEXT NOT NULL DEFAULT '',
            email TEXT NOT NULL DEFAULT '',
            phone TEXT NOT NULL DEFAULT '',
            address TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            hourly_rate REAL,
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            client_id TEXT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            hourly_rate REAL,
            budget REAL,
            color TEXT NOT NULL DEFAULT '#7c9cff',
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS entries (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            project_id TEXT,
            client_id TEXT,
            description TEXT NOT NULL DEFAULT '',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_sec INTEGER NOT NULL DEFAULT 0,
            rate REAL NOT NULL DEFAULT 0,
            amount REAL NOT NULL DEFAULT 0,
            billable INTEGER NOT NULL DEFAULT 1,
            paid INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS payments (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            client_id TEXT,
            project_id TEXT,
            amount REAL NOT NULL DEFAULT 0,
            currency TEXT NOT NULL DEFAULT 'ILS',
            method TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            paid_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS notifications (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            body TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'info',
            created_at TEXT NOT NULL,
            read INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_entries_started ON entries(started_at);
        CREATE INDEX IF NOT EXISTS idx_entries_profile ON entries(profile_id, started_at);
        CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project_id);
        CREATE INDEX IF NOT EXISTS idx_payments_paid_at ON payments(paid_at);
        "#,
    )
}

fn seed(conn: &Connection) -> rusqlite::Result<()> {
    for (key, value) in DEFAULT_SETTINGS {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO NOTHING",
            params![key, value],
        )?;
    }

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))?;
    if count == 0 {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO profiles (id, name, hourly_rate, currency, color, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![id, "עבודה", 0.0_f64, "ILS", "#7c9cff", now_str()],
        )?;
    }
    Ok(())
}

pub fn setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn setting_or(conn: &Connection, key: &str, fallback: &str) -> String {
    setting(conn, key).unwrap_or_else(|| fallback.to_string())
}

pub fn setting_f64(conn: &Connection, key: &str, fallback: f64) -> f64 {
    setting(conn, key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(fallback)
}

pub fn setting_i64(conn: &Connection, key: &str, fallback: i64) -> i64 {
    setting(conn, key)
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(fallback)
}

pub fn setting_bool(conn: &Connection, key: &str, fallback: bool) -> bool {
    setting(conn, key)
        .map(|v| v == "true" || v == "1")
        .unwrap_or(fallback)
}

pub fn all_settings(conn: &Connection) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = DEFAULT_SETTINGS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM settings") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                map.insert(row.0, row.1);
            }
        }
    }
    map
}

pub fn active_profile_id(conn: &Connection) -> String {
    if let Ok(id) = conn.query_row(
        "SELECT id FROM profiles WHERE is_active = 1 LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    ) {
        return id;
    }
    if let Ok(id) = conn.query_row("SELECT id FROM profiles LIMIT 1", [], |row| {
        row.get::<_, String>(0)
    }) {
        let _ = conn.execute("UPDATE profiles SET is_active = 1 WHERE id = ?1", params![id]);
        return id;
    }
    String::new()
}

/// Recomputes the duration of the running entry so every aggregate query stays
/// a plain SUM over stored columns.
pub fn sync_active_entry(conn: &Connection) {
    let active: Option<(String, f64, String)> = conn
        .query_row(
            "SELECT id, rate, started_at FROM entries WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?, row.get::<_, String>(2)?)),
        )
        .optional()
        .ok()
        .flatten();

    if let Some((id, rate, started_at)) = active {
        let elapsed = parse_local(&started_at)
            .map(|start| (Local::now().naive_local() - start).num_seconds().max(0))
            .unwrap_or(0);
        let amount = (elapsed as f64 / 3600.0 * rate * 100.0).round() / 100.0;
        let _ = conn.execute(
            "UPDATE entries SET duration_sec = ?1, amount = ?2 WHERE id = ?3",
            params![elapsed, amount, id],
        );
    }
}

/// Closes an entry that has been running longer than the configured limit.
pub fn auto_close_stale(conn: &Connection) {
    let limit = setting_i64(conn, "autoStopHours", 12);
    if limit <= 0 {
        return;
    }
    let stale: Option<(String, String, f64, String)> = conn
        .query_row(
            "SELECT id, started_at, rate, profile_id FROM entries WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten();

    if let Some((id, started_at, rate, profile_id)) = stale {
        if let Some(start) = parse_local(&started_at) {
            let elapsed = (Local::now().naive_local() - start).num_seconds();
            if elapsed > limit * 3600 {
                let end = start + Duration::seconds(limit * 3600);
                let seconds = limit * 3600;
                let amount = (seconds as f64 / 3600.0 * rate * 100.0).round() / 100.0;
                let _ = conn.execute(
                    "UPDATE entries SET ended_at = ?1, duration_sec = ?2, amount = ?3 WHERE id = ?4",
                    params![end.format("%Y-%m-%dT%H:%M:%S").to_string(), seconds, amount, id],
                );
                let _ = push_notification(
                    conn,
                    &profile_id,
                    "טיימר נסגר אוטומטית",
                    &format!("העבודה נמשכה מעל {} שעות ולכן נסגרה אוטומטית.", limit),
                    "warning",
                );
            }
        }
    }
}

pub fn push_notification(
    conn: &Connection,
    profile_id: &str,
    title: &str,
    body: &str,
    kind: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO notifications (id, title, body, kind, created_at, read) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![
            uuid::Uuid::new_v4().to_string(),
            title,
            body,
            kind,
            now_str()
        ],
    )?;
    let _ = profile_id;
    Ok(())
}

pub fn resolve_rate(
    conn: &Connection,
    project_id: Option<&str>,
    client_id: Option<&str>,
    profile_id: &str,
) -> f64 {
    if let Some(pid) = project_id {
        if let Ok(Some(rate)) = conn
            .query_row(
                "SELECT hourly_rate FROM projects WHERE id = ?1",
                params![pid],
                |row| row.get::<_, Option<f64>>(0),
            )
            .optional()
        {
            if let Some(rate) = rate {
                if rate > 0.0 {
                    return rate;
                }
            }
        }
    }
    if let Some(cid) = client_id {
        if let Ok(Some(rate)) = conn
            .query_row(
                "SELECT hourly_rate FROM clients WHERE id = ?1",
                params![cid],
                |row| row.get::<_, Option<f64>>(0),
            )
            .optional()
        {
            if let Some(rate) = rate {
                if rate > 0.0 {
                    return rate;
                }
            }
        }
    }
    conn.query_row(
        "SELECT hourly_rate FROM profiles WHERE id = ?1",
        params![profile_id],
        |row| row.get::<_, f64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0.0)
}

pub fn round_amount(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
