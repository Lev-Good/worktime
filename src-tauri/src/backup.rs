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
        "schema": 2,
        "exportedAt": crate::db::now_str(),
        "data": {
            // Money leaves as whole agorot, under the same key names the old backups used.
            "profiles": dump(conn, "SELECT id, name, hourly_rate_agorot AS hourly_rate, currency, color, is_active, created_at FROM profiles")?,
            "clients": dump(conn, "SELECT id, profile_id, name, contact, email, phone, address, notes, hourly_rate_agorot AS hourly_rate, archived, created_at FROM clients")?,
            "projects": dump(conn, "SELECT id, profile_id, client_id, name, description, status, hourly_rate_agorot AS hourly_rate, budget_agorot AS budget, color, archived, created_at FROM projects")?,
            "entries": dump(conn, "SELECT id, profile_id, project_id, client_id, description, started_at, ended_at, duration_sec, rate_agorot AS rate, amount_agorot AS amount, billable, paid, paused_sec, created_at FROM entries")?,
            "payments": dump(conn, "SELECT id, profile_id, client_id, project_id, amount_agorot AS amount, currency, method, note, paid_at, created_at FROM payments")?,
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

/// Money from a backup, in whole agorot. Backups written before schema 2 stored shekels,
/// so those are multiplied by 100 exactly as the database upgrade does.
fn agorot(row: &Value, key: &str, schema: i64) -> i64 {
    let value = match row.get(key) {
        Some(value) => value,
        None => return 0,
    };
    let raw = value
        .as_i64()
        .map(|whole| whole as f64)
        .or_else(|| value.as_f64())
        .unwrap_or(0.0);
    if schema >= 2 {
        raw as i64
    } else {
        crate::db::to_agorot(raw)
    }
}

fn optional_agorot(row: &Value, key: &str, schema: i64) -> Option<i64> {
    match row.get(key) {
        None | Some(Value::Null) => None,
        Some(_) => Some(agorot(row, key, schema)),
    }
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

    // 1 (and anything unlabelled) is the old shekel format; 2 and up are already agorot.
    let schema = payload
        .get("schema")
        .and_then(|value| value.as_i64())
        .unwrap_or(1);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch(
        "DELETE FROM entries; DELETE FROM payments; DELETE FROM projects; DELETE FROM clients; DELETE FROM notifications; DELETE FROM profiles;",
    )
    .map_err(|e| e.to_string())?;

    for row in array(&payload, "profiles") {
        tx.execute(
            "INSERT INTO profiles (id, name, hourly_rate_agorot, currency, color, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                text(row, "id"),
                text(row, "name"),
                agorot(row, "hourly_rate", schema),
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
            "INSERT INTO clients (id, profile_id, name, contact, email, phone, address, notes, hourly_rate_agorot, archived, created_at)
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
                optional_agorot(row, "hourly_rate", schema),
                integer(row, "archived"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "projects") {
        tx.execute(
            "INSERT INTO projects (id, profile_id, client_id, name, description, status, hourly_rate_agorot, budget_agorot, color, archived, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "client_id"),
                text(row, "name"),
                text(row, "description"),
                text(row, "status"),
                optional_agorot(row, "hourly_rate", schema),
                optional_agorot(row, "budget", schema),
                text(row, "color"),
                integer(row, "archived"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "entries") {
        tx.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at, ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, paused_sec, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "project_id"),
                optional_text(row, "client_id"),
                text(row, "description"),
                text(row, "started_at"),
                optional_text(row, "ended_at"),
                integer(row, "duration_sec"),
                agorot(row, "rate", schema),
                agorot(row, "amount", schema),
                integer(row, "billable"),
                integer(row, "paid"),
                integer(row, "paused_sec"),
                text(row, "created_at")
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    for row in array(&payload, "payments") {
        tx.execute(
            "INSERT INTO payments (id, profile_id, client_id, project_id, amount_agorot, currency, method, note, paid_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                text(row, "id"),
                text(row, "profile_id"),
                optional_text(row, "client_id"),
                optional_text(row, "project_id"),
                agorot(row, "amount", schema),
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

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_BACKUP: &str = r##"{
        "app": "WorkTime",
        "schema": 1,
        "exportedAt": "2025-06-01T10:00:00",
        "data": {
            "profiles": [{"id": "p9", "name": "old", "hourly_rate": 250.4,
                "currency": "ILS", "color": "#7c9cff", "is_active": 1,
                "created_at": "2025-01-01T00:00:00"}],
            "clients": [],
            "projects": [],
            "entries": [{"id": "e9", "profile_id": "p9", "project_id": null,
                "client_id": null, "description": "", "started_at": "2025-02-01T09:00:00",
                "ended_at": "2025-02-01T10:30:00", "duration_sec": 5400, "rate": 83.33,
                "amount": 123.456, "billable": 1, "paid": 1,
                "created_at": "2025-02-01T09:00:00"}],
            "payments": [{"id": "m9", "profile_id": "p9", "client_id": null,
                "project_id": null, "amount": 19.99, "currency": "ILS", "method": "",
                "note": "", "paid_at": "2025-02-02T10:00:00",
                "created_at": "2025-02-02T10:00:00"}],
            "notifications": [],
            "settings": []
        }
    }"##;

    #[test]
    fn an_old_shekel_backup_lands_as_whole_agorot() {
        let mut conn = crate::db::test_connection();

        import_json(&mut conn, OLD_BACKUP).expect("import");

        let rate: i64 = conn
            .query_row("SELECT hourly_rate_agorot FROM profiles WHERE id = 'p9'", [], |row| {
                row.get(0)
            })
            .expect("profile");
        let (entry_rate, entry_amount): (i64, i64) = conn
            .query_row(
                "SELECT rate_agorot, amount_agorot FROM entries WHERE id = 'e9'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("entry");
        let paid: i64 = conn
            .query_row("SELECT amount_agorot FROM payments WHERE id = 'm9'", [], |row| row.get(0))
            .expect("payment");

        assert_eq!(rate, 25_040, "a shekel backup is converted on the way in, once");
        assert_eq!((entry_rate, entry_amount), (8_333, 12_346));
        assert_eq!(paid, 1_999);
    }

    #[test]
    fn a_current_backup_round_trips_the_amounts_untouched() {
        let source = crate::db::test_connection();
        let profile = crate::db::active_profile_id(&source);
        source
            .execute(
                "INSERT INTO entries (id, profile_id, project_id, client_id, description,
                    started_at, ended_at, duration_sec, rate_agorot, amount_agorot, billable,
                    paid, paused_sec, created_at)
                 VALUES ('e1', ?1, NULL, NULL, '', '2026-11-02T09:00:00',
                    '2026-11-02T10:00:00', 3600, 8_333, 12_346, 1, 0, 600,
                    '2026-11-02T09:00:00')",
                params![profile],
            )
            .expect("entry");
        let json = export_json(&source).expect("export");
        let payload: Value = serde_json::from_str(&json).expect("json");
        assert_eq!(payload.get("schema").and_then(|v| v.as_i64()), Some(2));

        let mut restored = crate::db::test_connection();
        import_json(&mut restored, &json).expect("import");

        let (amount, paused): (i64, i64) = restored
            .query_row(
                "SELECT amount_agorot, paused_sec FROM entries WHERE id = 'e1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("entry");
        assert_eq!(amount, 12_346, "a current backup keeps its agorot as they are");
        assert_eq!(paused, 600, "paused time survives a backup and restore");
    }
}
