use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};

const SECRET_KEYS: &[&str] = &["gdriveClientSecret"];

fn dump(conn: &Connection, sql: &str) -> Result<Vec<Value>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let mut object = Map::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index).map_err(|e| e.to_string())? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(number) => json!(number),
                rusqlite::types::ValueRef::Real(number) => json!(number),
                rusqlite::types::ValueRef::Text(text) => {
                    json!(String::from_utf8_lossy(text).to_string())
                }
                rusqlite::types::ValueRef::Blob(_) => Value::Null,
            };
            object.insert(name.clone(), value);
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}

pub fn export_json(conn: &Connection) -> Result<String, String> {
    let mut settings = dump(conn, "SELECT key, value FROM settings")?;
    settings.retain(|row| {
        row.get("key")
            .and_then(|key| key.as_str())
            .map(|key| !SECRET_KEYS.contains(&key))
            .unwrap_or(true)
    });

    let payload = json!({
        "app": "WorkTime",
        "schema": 1,
        "exportedAt": crate::db::now_str(),
        "data": {
            "profiles": dump(conn, "SELECT id, name, hourly_rate, currency, color, is_active, created_at FROM profiles")?,
            "clients": dump(conn, "SELECT id, profile_id, name, contact, email, phone, address, notes, hourly_rate, archived, created_at FROM clients")?,
            "projects": dump(conn, "SELECT id, profile_id, client_id, name, description, status, hourly_rate, budget, color, archived, created_at FROM projects")?,
            "entries": dump(conn, "SELECT id, profile_id, project_id, client_id, description, started_at, ended_at, duration_sec, rate, amount, billable, paid, created_at FROM entries")?,
            "payments": dump(conn, "SELECT id, profile_id, client_id, project_id, amount, currency, method, note, paid_at, created_at FROM payments")?,
            "notifications": dump(conn, "SELECT id, title, body, kind, created_at, read FROM notifications")?,
            "settings": settings,
        }
    });

    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn optional_text(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(|value| value.as_str()).map(|v| v.to_string())
}

fn number(row: &Value, key: &str) -> f64 {
    row.get(key).and_then(|value| value.as_f64()).unwrap_or(0.0)
}

fn optional_number(row: &Value, key: &str) -> Option<f64> {
    row.get(key).and_then(|value| value.as_f64())
}

fn integer(row: &Value, key: &str) -> i64 {
    row.get(key)
        .map(|value| {
            value
                .as_i64()
                .or_else(|| value.as_bool().map(|b| b as i64))
                .or_else(|| value.as_f64().map(|f| f as i64))
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

fn array<'a>(payload: &'a Value, key: &str) -> &'a [Value] {
    payload
        .get("data")
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_array())
        .map(|items| items.as_slice())
        .unwrap_or(&[])
}

pub fn import_json(conn: &mut Connection, raw: &str) -> Result<(), String> {
    let payload: Value = serde_json::from_str(raw).map_err(|e| format!("קובץ הגיבוי אינו תקין: {e}"))?;
    if payload.get("app").and_then(|v| v.as_str()) != Some("WorkTime") {
        return Err("הקובץ אינו גיבוי של WorkTime".into());
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch(
        "DELETE FROM entries; DELETE FROM payments; DELETE FROM projects; DELETE FROM clients; DELETE FROM notifications; DELETE FROM profiles;",
    )
    .map_err(|e| e.to_string())?;

    for row in array(&payload, "profiles") {
        tx.execute(
            "INSERT INTO profiles (id, name, hourly_rate, currency, color, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                text(row, "id"),
                text(row, "name"),
                number(row, "hourly_rate"),
                text(row, "currency"),
                text(row, "color"),
                integer(row, "is_active"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "clients") {
        tx.execute(
            "INSERT INTO clients (id, profile_id, name, contact, email, phone, address, notes, hourly_rate, archived, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                text(row, "name"),
                text(row, "contact"),
                text(row, "email"),
                text(row, "phone"),
                text(row, "address"),
                text(row, "notes"),
                optional_number(row, "hourly_rate"),
                integer(row, "archived"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "projects") {
        tx.execute(
            "INSERT INTO projects (id, profile_id, client_id, name, description, status, hourly_rate, budget, color, archived, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "client_id"),
                text(row, "name"),
                text(row, "description"),
                text(row, "status"),
                optional_number(row, "hourly_rate"),
                optional_number(row, "budget"),
                text(row, "color"),
                integer(row, "archived"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "entries") {
        tx.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at, ended_at, duration_sec, rate, amount, billable, paid, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "project_id"),
                optional_text(row, "client_id"),
                text(row, "description"),
                text(row, "started_at"),
                optional_text(row, "ended_at"),
                integer(row, "duration_sec"),
                number(row, "rate"),
                number(row, "amount"),
                integer(row, "billable"),
                integer(row, "paid"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "payments") {
        tx.execute(
            "INSERT INTO payments (id, profile_id, client_id, project_id, amount, currency, method, note, paid_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "client_id"),
                optional_text(row, "project_id"),
                number(row, "amount"),
                text(row, "currency"),
                text(row, "method"),
                text(row, "note"),
                text(row, "paid_at"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "notifications") {
        tx.execute(
            "INSERT INTO notifications (id, title, body, kind, created_at, read)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                text(row, "id"),
                text(row, "title"),
                text(row, "body"),
                text(row, "kind"),
                text(row, "created_at"),
                integer(row, "read")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "settings") {
        let key = text(row, "key");
        if key.is_empty() || SECRET_KEYS.contains(&key.as_str()) {
            continue;
        }
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, text(row, "value")],
        )
        .map_err(|e| e.to_string())?;
    }

    let profiles: i64 = tx
        .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
        .unwrap_or(0);
    if profiles == 0 {
        return Err("הגיבוי לא הכיל פרופילים".into());
    }
    tx.execute("UPDATE profiles SET is_active = 0", [])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE profiles SET is_active = 1 WHERE id = (SELECT id FROM profiles ORDER BY created_at LIMIT 1)",
        [],
    )
    .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
