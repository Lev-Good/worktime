use crate::commands;
use crate::db;
use crate::models::TimerSnapshot;
use crate::AppState;
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

pub struct TrayItems {
    pub status: MenuItem<Wry>,
    pub toggle: MenuItem<Wry>,
}

pub fn currency_symbol(code: &str) -> &'static str {
    match code {
        "ILS" => "₪",
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        _ => "₪",
    }
}

fn hhmmss(seconds: i64) -> String {
    let s = seconds.max(0);
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    format!("{h:02}:{m:02}:{sec:02}")
}

fn hours_label(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    format!("{}:{:02}", minutes / 60, minutes % 60)
}

pub fn status_text(snapshot: &TimerSnapshot, show_timer: bool) -> String {
    if !show_timer {
        return "WorkTime".to_string();
    }
    if snapshot.running {
        let project = snapshot
            .project_name
            .clone()
            .unwrap_or_else(|| "ללא פרויקט".to_string());
        format!("⏱ {} · {}", hhmmss(snapshot.elapsed_sec), project)
    } else {
        format!("⏱ {} · לא פעיל", hhmmss(0))
    }
}

fn toggle_label(running: bool) -> &'static str {
    if running {
        "⏹ עצור וסכם"
    } else {
        "▶ התחל עבודה"
    }
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let _ = app.remove_tray_by_id("main");

    let state = app.state::<AppState>();
    let (
        snapshot,
        floating_enabled,
        notifications_enabled,
        profiles,
        active_profile,
        projects,
        today_seconds,
        today_earnings,
        unpaid,
        currency,
        show_timer,
    ) = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let profile = db::active_profile_id(&conn);
        let mut profiles: Vec<(String, String)> = Vec::new();
        if let Ok(mut stmt) = conn.prepare("SELECT id, name FROM profiles ORDER BY created_at ASC") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                profiles = rows.flatten().collect();
            }
        }
        let mut projects: Vec<(String, String)> = Vec::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, name FROM projects WHERE profile_id = ?1 AND archived = 0
             ORDER BY COALESCE((SELECT MAX(started_at) FROM entries WHERE project_id = projects.id), created_at) DESC
             LIMIT 8",
        ) {
            if let Ok(rows) = stmt.query_map(rusqlite::params![profile], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                projects = rows.flatten().collect();
            }
        }
        let (today_seconds, today_earnings) =
            commands::section_total(&conn, &profile, &db::today_start(), &db::today_end());
        let unpaid: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount),0) FROM entries
                 WHERE profile_id = ?1 AND billable = 1 AND paid = 0",
                rusqlite::params![profile],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        (
            commands::snapshot_of(&conn),
            db::setting_bool(&conn, "floatingEnabled", false),
            db::setting_bool(&conn, "notificationsEnabled", true),
            profiles,
            profile,
            projects,
            today_seconds,
            today_earnings,
            unpaid,
            db::setting_or(&conn, "currency", "ILS"),
            db::setting_bool(&conn, "trayShowTimer", true),
        )
    };

    let symbol = currency_symbol(&currency);
    let status = MenuItem::with_id(
        app,
        "status",
        status_text(&snapshot, show_timer),
        false,
        None::<&str>,
    )?;
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        toggle_label(snapshot.running),
        true,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "open", "פתח את WorkTime", true, None::<&str>)?;
    let nav_dashboard = MenuItem::with_id(app, "nav:dashboard", "לוח בקרה", true, None::<&str>)?;
    let nav_entries = MenuItem::with_id(app, "nav:entries", "יומן עבודה", true, None::<&str>)?;
    let nav_clients = MenuItem::with_id(app, "nav:clients", "לקוחות", true, None::<&str>)?;
    let nav_projects = MenuItem::with_id(app, "nav:projects", "פרויקטים", true, None::<&str>)?;
    let nav_reports = MenuItem::with_id(app, "nav:reports", "דוחות וסיכומים", true, None::<&str>)?;
    let nav_payments = MenuItem::with_id(app, "nav:payments", "תשלומים", true, None::<&str>)?;
    let nav_notifications = MenuItem::with_id(
        app,
        "nav:notifications",
        "התראות והודעות",
        true,
        None::<&str>,
    )?;
    let nav_settings = MenuItem::with_id(app, "nav:settings", "הגדרות", true, None::<&str>)?;
    let floating_item = CheckMenuItem::with_id(
        app,
        "floating",
        "שעון צף על המסך",
        true,
        floating_enabled,
        None::<&str>,
    )?;
    let mute_item = CheckMenuItem::with_id(
        app,
        "mute",
        "התראות",
        true,
        notifications_enabled,
        None::<&str>,
    )?;

    let today_item = MenuItem::with_id(
        app,
        "sum_today",
        format!(
            "היום: {} שעות · {}{}",
            hours_label(today_seconds),
            symbol,
            format!("{:.0}", today_earnings)
        ),
        false,
        None::<&str>,
    )?;
    let unpaid_item = MenuItem::with_id(
        app,
        "sum_unpaid",
        format!("ממתין לתשלום: {}{}", symbol, format!("{:.0}", unpaid)),
        false,
        None::<&str>,
    )?;

    let backup_item = MenuItem::with_id(app, "gdrive_backup", "גיבוי ל-Google Drive", true, None::<&str>)?;
    let update_item = MenuItem::with_id(app, "check_updates", "בדוק עדכוני גרסה", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "יציאה", true, None::<&str>)?;

    let mut project_items: Vec<MenuItem<Wry>> = Vec::new();
    for (id, name) in &projects {
        project_items.push(MenuItem::with_id(
            app,
            format!("proj:{id}"),
            name,
            true,
            None::<&str>,
        )?);
    }
    project_items.push(MenuItem::with_id(
        app,
        "nav:projects",
        "פרויקט חדש…",
        true,
        None::<&str>,
    )?);
    let project_refs: Vec<&dyn IsMenuItem<Wry>> = project_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();
    let projects_menu = Submenu::with_items(app, "התחל עבודה בפרויקט", true, &project_refs)?;

    let mut profile_items: Vec<CheckMenuItem<Wry>> = Vec::new();
    for (id, name) in &profiles {
        profile_items.push(CheckMenuItem::with_id(
            app,
            format!("profile:{id}"),
            name,
            true,
            *id == active_profile,
            None::<&str>,
        )?);
    }
    let profile_refs: Vec<&dyn IsMenuItem<Wry>> = profile_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<Wry>)
        .collect();
    let profiles_menu = Submenu::with_items(app, "פרופיל פעיל", true, &profile_refs)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    let items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &status,
        &toggle,
        &open,
        &sep1,
        &projects_menu,
        &floating_item,
        &mute_item,
        &profiles_menu,
        &sep2,
        &today_item,
        &unpaid_item,
        &nav_dashboard,
        &nav_entries,
        &nav_clients,
        &nav_projects,
        &nav_reports,
        &nav_payments,
        &nav_notifications,
        &sep3,
        &backup_item,
        &update_item,
        &nav_settings,
        &quit_item,
    ];
    let menu = Menu::with_items(app, &items)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip(status_text(&snapshot, true))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .build(app)?;

    {
        let mut handle = state.tray.lock().unwrap_or_else(|e| e.into_inner());
        *handle = Some(TrayItems {
            status: status.clone(),
            toggle: toggle.clone(),
        });
    }

    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = build(&handle);
    });
}

pub fn sync_status(app: &AppHandle, snapshot: &TimerSnapshot) {
    let (show_timer, text, wanted) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let show_timer = db::setting_bool(&conn, "trayShowTimer", true);
        (
            show_timer,
            status_text(snapshot, show_timer),
            toggle_label(snapshot.running),
        )
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(text.clone()));
    }

    let state = app.state::<AppState>();
    let guard = state.tray.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(items) = guard.as_ref() {
        let _ = items.status.set_text(&text);
        if items.toggle.text().map(|current| current != wanted).unwrap_or(false) {
            let _ = items.toggle.set_text(wanted);
        }
    }
    let _ = show_timer;
}

pub fn open_main(app: &AppHandle, view: Option<&str>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        if let Some(view) = view {
            let _ = app.emit("navigate", view.to_string());
        }
    }
}

fn toggle_timer(app: &AppHandle) {
    let state = app.state::<AppState>();
    let running = {
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row("SELECT COUNT(*) FROM entries WHERE ended_at IS NULL", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
            > 0
    };
    if running {
        let _ = commands::stop_timer(app.clone(), app.state::<AppState>());
    } else {
        let project_id = {
            let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
            let profile = db::active_profile_id(&conn);
            conn.query_row(
                "SELECT id FROM projects WHERE profile_id = ?1 AND archived = 0
                 ORDER BY COALESCE((SELECT MAX(started_at) FROM entries WHERE project_id = projects.id), created_at) DESC
                 LIMIT 1",
                rusqlite::params![profile],
                |row| row.get::<_, String>(0),
            )
            .ok()
        };
        let _ = commands::start_timer(app.clone(), app.state::<AppState>(), project_id, None);
    }
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().0.clone();
    if let Some(view) = id.strip_prefix("nav:") {
        open_main(app, Some(view));
        return;
    }
    if let Some(project_id) = id.strip_prefix("proj:") {
        let _ = commands::start_timer(
            app.clone(),
            app.state::<AppState>(),
            Some(project_id.to_string()),
            None,
        );
        return;
    }
    if let Some(profile_id) = id.strip_prefix("profile:") {
        let _ = commands::switch_profile(
            app.clone(),
            app.state::<AppState>(),
            profile_id.to_string(),
        );
        return;
    }

    match id.as_str() {
        "toggle" => toggle_timer(app),
        "open" => open_main(app, None),
        "floating" => {
            let enabled = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                db::setting_bool(&conn, "floatingEnabled", false)
            };
            let _ = commands::set_floating(app.clone(), app.state::<AppState>(), !enabled);
        }
        "mute" => {
            let enabled = {
                let state = app.state::<AppState>();
                let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                db::setting_bool(&conn, "notificationsEnabled", true)
            };
            let mut map = std::collections::HashMap::new();
            map.insert(
                "notificationsEnabled".to_string(),
                if enabled { "false" } else { "true" }.to_string(),
            );
            let _ = commands::save_settings(app.clone(), app.state::<AppState>(), map);
        }
        "gdrive_backup" => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                match crate::gdrive::backup(&handle).await {
                    Ok(_) => {
                        let _ = commands::notify(
                            handle.clone(),
                            handle.state::<AppState>(),
                            "הגיבוי הושלם".into(),
                            "הנתונים הועלו בהצלחה לחשבון Google Drive.".into(),
                        );
                    }
                    Err(err) => {
                        let _ = commands::notify(
                            handle.clone(),
                            handle.state::<AppState>(),
                            "הגיבוי נכשל".into(),
                            err,
                        );
                    }
                }
            });
        }
        "check_updates" => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                match commands::check_updates(handle.clone(), Some(true)).await {
                    Ok(info) => {
                        if !info.has_update {
                            let _ = commands::notify(
                                handle.clone(),
                                handle.state::<AppState>(),
                                "אין עדכונים".into(),
                                format!("אתם מעודכנים — גרסה {}.", info.current_version),
                            );
                        }
                    }
                    Err(err) => {
                        let _ = commands::notify(
                            handle.clone(),
                            handle.state::<AppState>(),
                            "בדיקת העדכונים נכשלה".into(),
                            err,
                        );
                    }
                }
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
