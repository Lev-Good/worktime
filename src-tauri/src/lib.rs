mod backup;
mod commands;
mod db;
mod gdrive;
mod models;
mod reminders;
mod tray;
mod updates;

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub notices: Mutex<HashMap<String, i64>>,
    pub tray: Mutex<Option<tray::TrayItems>>,
}

pub fn set_floating_visible(app: &AppHandle, visible: bool) -> Result<bool, String> {
    if !visible {
        if let Some(window) = app.get_webview_window("floating") {
            let _ = window.hide();
        }
        return Ok(false);
    }

    let mut created = false;
    let window = match app.get_webview_window("floating") {
        Some(window) => window,
        None => {
            created = true;
            WebviewWindowBuilder::new(app, "floating", WebviewUrl::App("floating.html".into()))
                .title("WorkTime — שעון צף")
                .inner_size(252.0, 92.0)
                .min_inner_size(180.0, 70.0)
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?
        }
    };

    if created {
        if let Ok(Some(monitor)) = app.primary_monitor() {
            let scale = monitor.scale_factor();
            let width = monitor.size().width as f64 / scale;
            let height = monitor.size().height as f64 / scale;
            let _ = window.set_position(tauri::LogicalPosition::new(28.0, height - 92.0 - 96.0));
            let _ = window.set_size(tauri::LogicalSize::new(
                (width - 56.0).clamp(180.0, 252.0),
                92.0,
            ));
        }
    }

    let on_top = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        db::setting_bool(&conn, "floatingAlwaysOnTop", true)
    };
    let _ = window.set_always_on_top(on_top);
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(true)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::open_main(app, None);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)?;
            let connection = db::open(&data_dir.join("worktime.db"))
                .map_err(|e| format!("database: {e}"))?;

            let floating_enabled = db::setting_bool(&connection, "floatingEnabled", false);
            app.manage(AppState {
                db: Mutex::new(connection),
                notices: Mutex::new(HashMap::new()),
                tray: Mutex::new(None),
            });

            tray::build(app.handle())?;

            if floating_enabled {
                let _ = set_floating_visible(app.handle(), true);
            }

            let hidden = std::env::args().any(|arg| arg == "--hidden");
            if hidden {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            commands::spawn_ticker(app.handle().clone());

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let auto = {
                    let state = handle.state::<AppState>();
                    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
                    db::setting_bool(&conn, "autoCheckUpdates", true)
                        && match db::setting(&conn, "lastUpdateCheck") {
                            Some(value) => db::parse_local(&value)
                                .map(|time| {
                                    (chrono::Local::now().naive_local() - time).num_hours() >= 12
                                })
                                .unwrap_or(true),
                            None => true,
                        }
                };
                if auto {
                    let _ = commands::check_updates(handle, Some(true)).await;
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::dashboard,
            commands::list_clients,
            commands::save_client,
            commands::delete_client,
            commands::list_projects,
            commands::save_project,
            commands::delete_project,
            commands::list_entries,
            commands::start_timer,
            commands::stop_timer,
            commands::save_entry,
            commands::delete_entry,
            commands::set_entry_paid,
            commands::report,
            commands::export_report_csv,
            commands::list_payments,
            commands::save_payment,
            commands::delete_payment,
            commands::list_notifications,
            commands::mark_notification,
            commands::clear_notifications,
            commands::save_settings,
            commands::save_profile,
            commands::delete_profile,
            commands::switch_profile,
            commands::set_floating,
            commands::timer,
            commands::check_updates,
            commands::open_url,
            commands::backup_local,
            commands::notify,
            commands::quit_app,
            show_main,
            gdrive_status,
            gdrive_connect,
            gdrive_disconnect,
            gdrive_backup,
            gdrive_backups,
            gdrive_restore,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn show_main(app: AppHandle, view: Option<String>) {
    tray::open_main(&app, view.as_deref());
}

#[tauri::command]
async fn gdrive_status(app: AppHandle) -> Result<models::DriveStatus, String> {
    Ok(gdrive::status(&app).await)
}

#[tauri::command]
async fn gdrive_connect(
    app: AppHandle,
    client_id: String,
    client_secret: String,
) -> Result<models::DriveStatus, String> {
    gdrive::connect(&app, client_id, client_secret).await
}

#[tauri::command]
async fn gdrive_disconnect(app: AppHandle) -> Result<(), String> {
    gdrive::disconnect(&app)
}

#[tauri::command]
async fn gdrive_backup(app: AppHandle) -> Result<models::DriveStatus, String> {
    gdrive::backup(&app).await
}

#[tauri::command]
async fn gdrive_backups(app: AppHandle) -> Result<Vec<models::DriveFile>, String> {
    gdrive::list_files(&app).await
}

#[tauri::command]
async fn gdrive_restore(app: AppHandle, file_id: String) -> Result<(), String> {
    gdrive::restore(&app, &file_id).await
}
