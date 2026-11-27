use crate::db;
use crate::models::{DriveFile, DriveStatus};
use crate::AppState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const FOLDER_NAME: &str = "WorkTime Backups";
const KEEP_BACKUPS: usize = 30;

#[derive(Serialize, Deserialize, Default, Clone)]
struct StoredToken {
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_at: i64,
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("WorkTime/1.0")
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())
}

fn token_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("gdrive-token.json"))
}

fn read_token(app: &AppHandle) -> StoredToken {
    token_path(app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_token(app: &AppHandle, token: &StoredToken) -> Result<(), String> {
    let path = token_path(app)?;
    let raw = serde_json::to_string(token).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn credentials(app: &AppHandle) -> (String, String) {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    (
        db::setting_or(&conn, "gdriveClientId", ""),
        db::setting_or(&conn, "gdriveClientSecret", ""),
    )
}

fn random_token(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

async fn error_body(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(message) = value
            .get("error_description")
            .or_else(|| value.get("error").and_then(|e| e.get("message")))
            .and_then(|v| v.as_str())
        {
            return format!("שגיאת Google ({status}): {message}");
        }
    }
    format!("שגיאת Google ({status})")
}

async fn refresh_access_token(app: &AppHandle, client: &reqwest::Client) -> Result<String, String> {
    let stored = read_token(app);
    if stored.refresh_token.is_empty() {
        return Err("לא נמצא חיבור פעיל ל-Google Drive. יש להתחבר מחדש.".into());
    }
    let now = chrono::Local::now().timestamp();
    if !stored.access_token.is_empty() && stored.expires_at - 60 > now {
        return Ok(stored.access_token);
    }

    let (client_id, client_secret) = credentials(app);
    let response = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", stored.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;

    if !response.status().is_success() {
        return Err(error_body(response).await);
    }

    let payload: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let access = payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let expires_in = payload
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);

    let mut updated = stored;
    updated.access_token = access.clone();
    updated.expires_at = chrono::Local::now().timestamp() + expires_in;
    write_token(app, &updated)?;
    Ok(access)
}

async fn folder_id(app: &AppHandle, client: &reqwest::Client, token: &str) -> Result<String, String> {
    let existing = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        db::setting_or(&conn, "gdriveFolderId", "")
    };
    if !existing.is_empty() {
        return Ok(existing);
    }

    let query = format!(
        "name='{}' and mimeType='application/vnd.google-apps.folder' and trashed=false",
        FOLDER_NAME
    );
    let response = client
        .get(format!("{API}/files"))
        .bearer_auth(token)
        .query(&[("q", query.as_str()), ("fields", "files(id,name)"), ("pageSize", "10")])
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !response.status().is_success() {
        return Err(error_body(response).await);
    }
    let payload: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    if let Some(id) = payload
        .get("files")
        .and_then(|v| v.as_array())
        .and_then(|files| files.first())
        .and_then(|file| file.get("id"))
        .and_then(|id| id.as_str())
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::set_setting(&conn, "gdriveFolderId", id);
        return Ok(id.to_string());
    }

    let response = client
        .post(format!("{API}/files?fields=id"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": FOLDER_NAME,
            "mimeType": "application/vnd.google-apps.folder"
        }))
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !response.status().is_success() {
        return Err(error_body(response).await);
    }
    let payload: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("לא ניתן היה ליצור תיקיית גיבוי ב-Drive")?
        .to_string();
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let _ = db::set_setting(&conn, "gdriveFolderId", &id);
    Ok(id)
}

pub async fn list_files(app: &AppHandle) -> Result<Vec<DriveFile>, String> {
    let client = http()?;
    let token = refresh_access_token(app, &client).await?;
    let folder = folder_id(app, &client, &token).await?;
    let query = format!("'{}' in parents and trashed=false", folder);
    let response = client
        .get(format!("{API}/files"))
        .bearer_auth(&token)
        .query(&[
            ("q", query.as_str()),
            ("fields", "files(id,name,createdTime,size)"),
            ("orderBy", "createdTime desc"),
            ("pageSize", "50"),
        ])
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !response.status().is_success() {
        return Err(error_body(response).await);
    }
    let payload: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    if let Some(items) = payload.get("files").and_then(|v| v.as_array()) {
        for item in items {
            files.push(DriveFile {
                id: item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: item
                    .get("createdTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                size: item
                    .get("size")
                    .and_then(|v| v.as_str())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0),
            });
        }
    }
    Ok(files)
}

pub async fn status(app: &AppHandle) -> DriveStatus {
    let (configured, account, last_backup) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        (
            !db::setting_or(&conn, "gdriveClientId", "").is_empty(),
            db::setting_or(&conn, "gdriveAccount", ""),
            db::setting_or(&conn, "lastBackupAt", ""),
        )
    };
    let connected = configured && !read_token(app).refresh_token.is_empty();
    let mut files = Vec::new();
    if connected {
        if let Ok(list) = list_files(app).await {
            files = list;
        }
    }
    DriveStatus {
        configured,
        connected,
        account,
        folder_id: "WorkTime Backups".to_string(),
        last_backup_at: last_backup,
        files,
    }
}

pub async fn connect(app: &AppHandle, client_id: String, client_secret: String) -> Result<DriveStatus, String> {
    let client_id = client_id.trim().to_string();
    let client_secret = client_secret.trim().to_string();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err("יש להזין Client ID ו-Client Secret של אפליקציית Google (סוג Desktop).".into());
    }
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        db::set_setting(&conn, "gdriveClientId", &client_id).map_err(|e| e.to_string())?;
        db::set_setting(&conn, "gdriveClientSecret", &client_secret).map_err(|e| e.to_string())?;
    }

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| format!("לא ניתן לפתוח חיבור מקומי: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();
    let redirect = format!("http://127.0.0.1:{port}");

    let verifier = random_token(64);
    let challenge = pkce_challenge(&verifier);
    let state_token = random_token(24);

    let auth_url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode(SCOPE),
        challenge,
        state_token
    );

    app.opener()
        .open_url(auth_url, None::<&str>)
        .map_err(|e| format!("לא ניתן לפתוח את הדפדפן: {e}"))?;

    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| "פג הזמן להשלמת ההתחברות (5 דקות). נסו שוב.".to_string())?
        .map_err(|e| e.to_string())?;

    let mut buffer = vec![0u8; 16 * 1024];
    let read = tokio::time::timeout(Duration::from_secs(20), socket.read(&mut buffer))
        .await
        .map_err(|_| "לא התקבלה תשובה מהדפדפן".to_string())?
        .map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buffer[..read]).to_string();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

    let mut code: Option<String> = None;
    let mut returned_state: Option<String> = None;
    let mut error: Option<String> = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = urlencoding::decode(value)
            .map(|v| v.to_string())
            .unwrap_or_default();
        match key {
            "code" => code = Some(value),
            "state" => returned_state = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }

    let page = |title: &str, message: &str| {
        format!(
            "<!doctype html><html lang=\"he\" dir=\"rtl\"><head><meta charset=\"utf-8\"><title>{title}</title></head>\
             <body style=\"font-family:Segoe UI,Arial;background:#0b0d12;color:#e7ecf5;display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
             <div style=\"text-align:center;padding:32px 40px;border-radius:16px;background:#121620;border:1px solid #253048\">\
             <h1 style=\"font-size:20px;margin:0 0 8px\">{title}</h1><p style=\"margin:0;color:#9aa8c2\">{message}</p>\
             <p style=\"margin-top:18px;color:#5f6f8c;font-size:13px\">אפשר לסגור את החלון ולחזור ל-WorkTime</p></div></body></html>"
        )
    };

    let response_html = if let Some(message) = error.as_deref() {
        page("ההתחברות בוטלה", message)
    } else if code.is_none() {
        page("ההתחברות נכשלה", "לא התקבל קוד הרשאה מגוגל.")
    } else if returned_state.as_deref() != Some(state_token.as_str()) {
        page("ההתחברות נכשלה", "אימות ה-state נכשל.")
    } else {
        page("ההתחברות הושלמה", "WorkTime מחובר כעת ל-Google Drive.")
    };
    let status_line = if error.is_none() && code.is_some() {
        "200 OK"
    } else {
        "400 Bad Request"
    };
    let response = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        response_html.as_bytes().len(),
        response_html
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.flush().await;

    let code = code.ok_or("לא התקבל קוד הרשאה מגוגל")?;
    let client = http()?;
    let exchange = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !exchange.status().is_success() {
        return Err(error_body(exchange).await);
    }
    let payload: serde_json::Value = exchange.json().await.map_err(|e| e.to_string())?;
    let refresh = payload
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if refresh.is_empty() {
        return Err("גוגל לא החזיר Refresh Token. הסירו את ההרשאה בחשבון Google ונסו שוב.".into());
    }
    let access = payload
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let expires_in = payload
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);

    write_token(
        app,
        &StoredToken {
            refresh_token: refresh,
            access_token: access,
            expires_at: chrono::Local::now().timestamp() + expires_in,
        },
    )?;

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::set_setting(&conn, "gdriveAccount", "חשבון Google מחובר");
        let _ = db::set_setting(&conn, "gdriveFolderId", "");
        let _ = db::push_notification(
            &conn,
            &db::active_profile_id(&conn),
            "Google Drive מחובר",
            "מעכשיו אפשר לגבות את הנתונים ישירות לדרייב, גם אוטומטית.",
            "backup",
        );
    }

    let token = refresh_access_token(app, &client).await?;
    let _ = folder_id(app, &client, &token).await;

    Ok(status(app).await)
}

pub fn disconnect(app: &AppHandle) -> Result<(), String> {
    if let Ok(path) = token_path(app) {
        let _ = std::fs::remove_file(path);
    }
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let _ = db::set_setting(&conn, "gdriveAccount", "");
    let _ = db::set_setting(&conn, "gdriveFolderId", "");
    let _ = db::set_setting(&conn, "lastBackupAt", "");
    Ok(())
}

pub async fn backup(app: &AppHandle) -> Result<DriveStatus, String> {
    let json = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        crate::backup::export_json(&conn)?
    };

    let client = http()?;
    let token = refresh_access_token(app, &client).await?;
    let folder = folder_id(app, &client, &token).await?;
    let name = format!(
        "worktime-{}.json",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let metadata = serde_json::json!({ "name": name, "parents": [folder] }).to_string();

    let form = reqwest::multipart::Form::new()
        .part(
            "metadata",
            reqwest::multipart::Part::text(metadata)
                .mime_str("application/json; charset=UTF-8")
                .map_err(|e| e.to_string())?,
        )
        .part(
            "file",
            reqwest::multipart::Part::text(json)
                .mime_str("application/json; charset=UTF-8")
                .map_err(|e| e.to_string())?,
        );

    let response = client
        .post(format!("{UPLOAD_URL}?uploadType=multipart&fields=id"))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !response.status().is_success() {
        return Err(error_body(response).await);
    }

    let files = list_files(app).await.unwrap_or_default();
    for old in files.iter().skip(KEEP_BACKUPS) {
        let _ = client
            .delete(format!("{API}/files/{}", old.id))
            .bearer_auth(&token)
            .send()
            .await;
    }

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::set_setting(&conn, "lastBackupAt", &db::now_str());
    }

    Ok(status(app).await)
}

pub async fn restore(app: &AppHandle, file_id: &str) -> Result<(), String> {
    let client = http()?;
    let token = refresh_access_token(app, &client).await?;
    let response = client
        .get(format!("{API}/files/{file_id}?alt=media"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("שגיאת רשת: {e}"))?;
    if !response.status().is_success() {
        return Err(error_body(response).await);
    }
    let raw = response.text().await.map_err(|e| e.to_string())?;

    {
        let state = app.state::<AppState>();
        let mut conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        crate::backup::import_json(&mut conn, &raw)?;
    }
    crate::tray::refresh(app);
    let _ = app.emit("state-changed", ());
    Ok(())
}

pub fn auto_backup_if_due(app: &AppHandle) {
    let due = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        if !db::setting_bool(&conn, "backupEnabled", false) {
            false
        } else if read_token(app).refresh_token.is_empty() {
            false
        } else {
            let hours = db::setting_i64(&conn, "backupIntervalHours", 24).max(1);
            let last = db::setting_or(&conn, "lastBackupAt", "");
            match db::parse_local(&last) {
                Some(time) => {
                    (chrono::Local::now().naive_local() - time).num_hours() >= hours
                }
                None => true,
            }
        }
    };
    if !due {
        return;
    }

    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::set_setting(&conn, "lastBackupAt", &crate::db::now_str());
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = backup(&handle).await;
    });
}
