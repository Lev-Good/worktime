use chrono::{Datelike, Duration, Local, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

/// The schema version this build writes. Every database — brand new or years old — reaches
/// it by running the same steps, so a fresh install and an upgrade end up identical and the
/// upgrade path is exercised by every test run.
pub const SCHEMA_VERSION: i64 = 3;

/// Minutes without keyboard or mouse input after which a running timer stops charging,
/// until the user is back. `0` turns the idle pause off.
pub const DEFAULT_IDLE_PAUSE_MINUTES: i64 = 15;

pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("theme", "dark"),
    ("currency", "ILS"),
    ("workdayHours", "8.5"),
    ("dailyGoalHours", "8"),
    ("workStart", "09:00"),
    ("workEnd", "18:00"),
    ("idleReminderMinutes", "45"),
    ("idlePauseMinutes", "15"),
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

/// Money is stored as whole agorot (1/100 of a shekel) in `INTEGER` columns, so adding
/// amounts is exact integer arithmetic instead of binary fractions that drift.
///
/// Shekels to agorot, rounding halves away from zero: 12.345 becomes 1235.
pub fn to_agorot(shekels: f64) -> i64 {
    if !shekels.is_finite() {
        return 0;
    }
    (shekels * 100.0).round() as i64
}

/// Agorot back to shekels, for the JSON boundary and display only.
pub fn to_shekels(agorot: i64) -> f64 {
    agorot as f64 / 100.0
}

/// Billable amount in agorot for `seconds` at `rate_agorot` per hour, rounded to the agora.
///
/// The multiplication is done in `i128`, so neither a long entry nor a high rate can
/// overflow the intermediate value, and the rounding is the same half-up rule as `to_agorot`.
pub fn amount_agorot(seconds: i64, rate_agorot: i64) -> i64 {
    if seconds <= 0 || rate_agorot <= 0 {
        return 0;
    }
    let numerator = seconds as i128 * rate_agorot as i128;
    ((numerator + 1800) / 3600) as i64
}

/// Seconds actually worked: the clock time minus the parts the machine was asleep or the
/// user was away. Never negative.
pub fn worked_seconds(elapsed_sec: i64, paused_sec: i64) -> i64 {
    (elapsed_sec.max(0) - paused_sec.max(0)).max(0)
}

/// Rounds a duration up to the nearest whole `rounding_minutes` step.
/// `0` (or any non-positive value) leaves the duration unchanged.
pub fn round_seconds(seconds: i64, rounding_minutes: i64) -> i64 {
    let step = rounding_minutes * 60;
    if step <= 0 {
        return seconds;
    }
    ((seconds + step - 1) / step) * step
}

fn schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn set_schema_version(conn: &Connection, version: i64) -> rusqlite::Result<()> {
    conn.execute_batch(&format!("PRAGMA user_version = {version}"))
}

/// Brings any database up to `SCHEMA_VERSION`, one numbered step at a time.
///
/// A version 0 database may be empty (a new install) or full of old rows (an install from
/// before versions were recorded); step 1 creates the original tables with
/// `CREATE TABLE IF NOT EXISTS`, so it is a no-op for an existing database, and steps 2 and
/// 3 then convert and extend whatever was already there.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let mut version = schema_version(conn)?;
    while version < SCHEMA_VERSION {
        match version {
            0 => create_initial_schema(conn)?,
            1 => upgrade_money_to_agorot(conn)?,
            2 => add_paused_seconds(conn)?,
            // A version this build does not know cannot be migrated forward safely.
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
        version += 1;
        set_schema_version(conn, version)?;
    }
    Ok(())
}

/// Version 1: the original schema, with money as `REAL` shekels.
fn create_initial_schema(conn: &Connection) -> rusqlite::Result<()> {
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

/// Version 2: money moves from `REAL` shekels to `INTEGER` agorot.
///
/// Each table is rebuilt rather than patched, because `ALTER TABLE` cannot change a column's
/// type or affinity: a rebuilt table declares the money columns `INTEGER`, so SQLite stores
/// and returns whole numbers. Existing values are multiplied by 100 and rounded, which is
/// exact for every amount the app could have written (it always rounded to cents itself).
fn upgrade_money_to_agorot(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        r#"
        CREATE TABLE profiles_v2 (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            hourly_rate_agorot INTEGER NOT NULL DEFAULT 0,
            currency TEXT NOT NULL DEFAULT 'ILS',
            color TEXT NOT NULL DEFAULT '#7c9cff',
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        INSERT INTO profiles_v2 (id, name, hourly_rate_agorot, currency, color, is_active, created_at)
            SELECT id, name, CAST(ROUND(hourly_rate * 100) AS INTEGER), currency, color,
                   is_active, created_at
            FROM profiles;
        DROP TABLE profiles;
        ALTER TABLE profiles_v2 RENAME TO profiles;

        CREATE TABLE clients_v2 (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            name TEXT NOT NULL,
            contact TEXT NOT NULL DEFAULT '',
            email TEXT NOT NULL DEFAULT '',
            phone TEXT NOT NULL DEFAULT '',
            address TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            hourly_rate_agorot INTEGER,
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        INSERT INTO clients_v2 (id, profile_id, name, contact, email, phone, address, notes,
                hourly_rate_agorot, archived, created_at)
            SELECT id, profile_id, name, contact, email, phone, address, notes,
                   CASE WHEN hourly_rate IS NULL THEN NULL
                        ELSE CAST(ROUND(hourly_rate * 100) AS INTEGER) END,
                   archived, created_at
            FROM clients;
        DROP TABLE clients;
        ALTER TABLE clients_v2 RENAME TO clients;

        CREATE TABLE projects_v2 (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            client_id TEXT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            hourly_rate_agorot INTEGER,
            budget_agorot INTEGER,
            color TEXT NOT NULL DEFAULT '#7c9cff',
            archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        INSERT INTO projects_v2 (id, profile_id, client_id, name, description, status,
                hourly_rate_agorot, budget_agorot, color, archived, created_at)
            SELECT id, profile_id, client_id, name, description, status,
                   CASE WHEN hourly_rate IS NULL THEN NULL
                        ELSE CAST(ROUND(hourly_rate * 100) AS INTEGER) END,
                   CASE WHEN budget IS NULL THEN NULL
                        ELSE CAST(ROUND(budget * 100) AS INTEGER) END,
                   color, archived, created_at
            FROM projects;
        DROP TABLE projects;
        ALTER TABLE projects_v2 RENAME TO projects;

        CREATE TABLE entries_v2 (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            project_id TEXT,
            client_id TEXT,
            description TEXT NOT NULL DEFAULT '',
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_sec INTEGER NOT NULL DEFAULT 0,
            rate_agorot INTEGER NOT NULL DEFAULT 0,
            amount_agorot INTEGER NOT NULL DEFAULT 0,
            billable INTEGER NOT NULL DEFAULT 1,
            paid INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        INSERT INTO entries_v2 (id, profile_id, project_id, client_id, description, started_at,
                ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
            SELECT id, profile_id, project_id, client_id, description, started_at, ended_at,
                   duration_sec,
                   CAST(ROUND(rate * 100) AS INTEGER),
                   CAST(ROUND(amount * 100) AS INTEGER),
                   billable, paid, created_at
            FROM entries;
        DROP TABLE entries;
        ALTER TABLE entries_v2 RENAME TO entries;

        CREATE TABLE payments_v2 (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL DEFAULT '',
            client_id TEXT,
            project_id TEXT,
            amount_agorot INTEGER NOT NULL DEFAULT 0,
            currency TEXT NOT NULL DEFAULT 'ILS',
            method TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            paid_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        INSERT INTO payments_v2 (id, profile_id, client_id, project_id, amount_agorot, currency,
                method, note, paid_at, created_at)
            SELECT id, profile_id, client_id, project_id,
                   CAST(ROUND(amount * 100) AS INTEGER),
                   currency, method, note, paid_at, created_at
            FROM payments;
        DROP TABLE payments;
        ALTER TABLE payments_v2 RENAME TO payments;

        CREATE INDEX IF NOT EXISTS idx_entries_started ON entries(started_at);
        CREATE INDEX IF NOT EXISTS idx_entries_profile ON entries(profile_id, started_at);
        CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project_id);
        CREATE INDEX IF NOT EXISTS idx_payments_paid_at ON payments(paid_at);
        "#,
    )?;
    tx.commit()
}

/// Version 3: how much of an entry the machine was asleep or the user was away.
fn add_paused_seconds(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "ALTER TABLE entries ADD COLUMN paused_sec INTEGER NOT NULL DEFAULT 0",
        [],
    )?;
    Ok(())
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
            "INSERT INTO profiles (id, name, hourly_rate_agorot, currency, color, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![id, "עבודה", 0_i64, "ILS", "#7c9cff", now_str()],
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
    let active: Option<(String, i64, String, i64)> = conn
        .query_row(
            "SELECT id, rate_agorot, started_at, paused_sec FROM entries
             WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten();

    if let Some((id, rate_agorot, started_at, paused)) = active {
        let elapsed = parse_local(&started_at)
            .map(|start| (Local::now().naive_local() - start).num_seconds().max(0))
            .unwrap_or(0);
        // Suspended or idle time is kept out of the clock, not just out of the price: the
        // duration the user sees is the time actually worked.
        let worked = worked_seconds(elapsed, paused);
        // The amount is priced the way stop_active will price it, rounding step included,
        // so the number the user watches does not jump when the timer stops.
        let billed = round_seconds(worked, setting_i64(conn, "roundingMinutes", 0));
        let amount = amount_agorot(billed, rate_agorot);
        let _ = conn.execute(
            "UPDATE entries SET duration_sec = ?1, amount_agorot = ?2 WHERE id = ?3",
            params![worked, amount, id],
        );
    }
}

/// Keeps `seconds` of the running entry out of the bill: the machine was suspended, or the
/// user was away longer than allowed. Returns the new total of paused seconds, or `None`
/// when no timer is running.
pub fn pause_active_entry(conn: &Connection, seconds: i64) -> Option<i64> {
    if seconds <= 0 {
        return None;
    }
    let active: Option<(String, i64)> = conn
        .query_row(
            "SELECT id, paused_sec FROM entries WHERE ended_at IS NULL
             ORDER BY started_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .ok()
        .flatten();

    let (id, paused) = active?;
    let paused = paused + seconds;
    conn.execute(
        "UPDATE entries SET paused_sec = ?1 WHERE id = ?2",
        params![paused, id],
    )
    .ok()?;
    Some(paused)
}

/// Closes an entry that has been running longer than the configured limit.
pub fn auto_close_stale(conn: &Connection) {
    let limit = setting_i64(conn, "autoStopHours", 12);
    if limit <= 0 {
        return;
    }
    let stale: Option<(String, String, i64, String, i64)> = conn
        .query_row(
            "SELECT id, started_at, rate_agorot, profile_id, paused_sec FROM entries
             WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten();

    if let Some((id, started_at, rate_agorot, profile_id, paused)) = stale {
        if let Some(start) = parse_local(&started_at) {
            let elapsed = (Local::now().naive_local() - start).num_seconds();
            if elapsed > limit * 3600 {
                let end = start + Duration::seconds(limit * 3600);
                // The limit is wall clock, so suspended time inside it is not charged.
                let seconds = worked_seconds(limit * 3600, paused);
                let amount = amount_agorot(seconds, rate_agorot);
                let _ = conn.execute(
                    "UPDATE entries SET ended_at = ?1, duration_sec = ?2, amount_agorot = ?3 WHERE id = ?4",
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

/// The hourly rate that applies to a new entry, in agorot: the project's own rate first,
/// then the client's, then the profile's.
pub fn resolve_rate_agorot(
    conn: &Connection,
    project_id: Option<&str>,
    client_id: Option<&str>,
    profile_id: &str,
) -> i64 {
    if let Some(pid) = project_id {
        if let Ok(Some(rate)) = conn
            .query_row(
                "SELECT hourly_rate_agorot FROM projects WHERE id = ?1",
                params![pid],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
        {
            if let Some(rate) = rate {
                if rate > 0 {
                    return rate;
                }
            }
        }
    }
    if let Some(cid) = client_id {
        if let Ok(Some(rate)) = conn
            .query_row(
                "SELECT hourly_rate_agorot FROM clients WHERE id = ?1",
                params![cid],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
        {
            if let Some(rate) = rate {
                if rate > 0 {
                    return rate;
                }
            }
        }
    }
    conn.query_row(
        "SELECT hourly_rate_agorot FROM profiles WHERE id = ?1",
        params![profile_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
}

/// In-memory database with the real schema and defaults, for unit tests only.
#[cfg(test)]
pub fn test_connection() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory database");
    migrate(&conn).expect("schema");
    seed(&conn).expect("defaults");
    conn
}

/// A still-running entry that started `seconds_ago`, for unit tests only.
#[cfg(test)]
pub fn insert_test_running_entry(
    conn: &Connection,
    id: &str,
    profile_id: &str,
    seconds_ago: i64,
    rate_agorot: i64,
) {
    let started_at = (Local::now().naive_local() - Duration::seconds(seconds_ago))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    conn.execute(
        "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
            ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
         VALUES (?1, ?2, NULL, NULL, '', ?3, NULL, 0, ?4, 0, 1, 0, ?3)",
        params![id, profile_id, started_at, rate_agorot],
    )
    .expect("insert entry");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert_profile(conn: &Connection, id: &str, rate_agorot: i64) {
        conn.execute(
            "INSERT INTO profiles (id, name, hourly_rate_agorot, currency, color, is_active, created_at)
             VALUES (?1, 'test', ?2, 'ILS', '#7c9cff', 0, '2026-01-01T00:00:00')",
            params![id, rate_agorot],
        )
        .expect("insert profile");
    }

    fn insert_client(conn: &Connection, id: &str, profile_id: &str, rate_agorot: Option<i64>) {
        conn.execute(
            "INSERT INTO clients (id, profile_id, name, hourly_rate_agorot, created_at)
             VALUES (?1, ?2, 'client', ?3, '2026-01-01T00:00:00')",
            params![id, profile_id, rate_agorot],
        )
        .expect("insert client");
    }

    fn insert_project(conn: &Connection, id: &str, client_id: &str, rate_agorot: Option<i64>) {
        conn.execute(
            "INSERT INTO projects (id, profile_id, client_id, name, hourly_rate_agorot, created_at)
             VALUES (?1, '', ?2, 'project', ?3, '2026-01-01T00:00:00')",
            params![id, client_id, rate_agorot],
        )
        .expect("insert project");
    }

    fn set_rate(conn: &Connection, table: &str, id: &str, rate_agorot: Option<i64>) {
        conn.execute(
            &format!("UPDATE {table} SET hourly_rate_agorot = ?1 WHERE id = ?2"),
            params![rate_agorot, id],
        )
        .expect("update rate");
    }

    #[test]
    fn shekels_become_whole_agorot_and_back() {
        assert_eq!(to_agorot(0.0), 0);
        assert_eq!(to_agorot(250.0), 25_000);
        assert_eq!(to_agorot(12.344), 1_234);
        assert_eq!(to_agorot(12.346), 1_235);
        assert_eq!(to_agorot(0.005), 1);
        assert_eq!(to_agorot(1234.5678), 123_457);
        assert_eq!(to_agorot(f64::NAN), 0);
        assert_eq!(to_shekels(123_457), 1234.57);
    }

    #[test]
    fn round_seconds_rounds_up_to_the_step() {
        assert_eq!(round_seconds(61, 5), 300);
        assert_eq!(round_seconds(300, 5), 300);
        assert_eq!(round_seconds(301, 5), 600);
        assert_eq!(round_seconds(3600, 15), 3600);
        assert_eq!(round_seconds(3601, 15), 4500);
        assert_eq!(round_seconds(90, 1), 120);
        assert_eq!(round_seconds(0, 30), 0);
    }

    #[test]
    fn round_seconds_is_a_no_op_when_rounding_is_off() {
        assert_eq!(round_seconds(301, 0), 301);
        assert_eq!(round_seconds(301, -5), 301);
    }

    #[test]
    fn amount_is_the_hourly_rate_in_agorot() {
        assert_eq!(amount_agorot(3600, 10_000), 10_000);
        assert_eq!(amount_agorot(5400, 12_000), 18_000);
        assert_eq!(amount_agorot(900, 25_000), 6_250);
        assert_eq!(amount_agorot(1, 8_500), 2);
        assert_eq!(amount_agorot(3600, 0), 0);
    }

    #[test]
    fn amount_sums_exactly_where_floats_would_drift() {
        // Ten one-ore entries at 0.10 must add up to exactly one shekel, not 0.9999999.
        let one = amount_agorot(36, 1_000);
        assert_eq!(one, 10);
        let total: i64 = (0..10).map(|_| one).sum();
        assert_eq!(total, 100, "the sum is whole agorot, so it cannot drift");
        assert_eq!(to_shekels(total), 1.0);
    }

    #[test]
    fn resolve_rate_prefers_the_project_then_client_then_profile() {
        let conn = test_connection();
        insert_profile(&conn, "p1", 9_000);
        insert_client(&conn, "c1", "p1", Some(18_000));
        insert_project(&conn, "pr1", "c1", Some(25_000));

        assert_eq!(resolve_rate_agorot(&conn, Some("pr1"), Some("c1"), "p1"), 25_000);

        set_rate(&conn, "projects", "pr1", None);
        assert_eq!(resolve_rate_agorot(&conn, Some("pr1"), Some("c1"), "p1"), 18_000);

        set_rate(&conn, "projects", "pr1", Some(0));
        assert_eq!(resolve_rate_agorot(&conn, Some("pr1"), Some("c1"), "p1"), 18_000);

        set_rate(&conn, "clients", "c1", None);
        assert_eq!(resolve_rate_agorot(&conn, Some("pr1"), Some("c1"), "p1"), 9_000);

        set_rate(&conn, "clients", "c1", Some(0));
        assert_eq!(resolve_rate_agorot(&conn, Some("pr1"), Some("c1"), "p1"), 9_000);
    }

    #[test]
    fn resolve_rate_falls_back_when_ids_are_unknown_or_missing() {
        let conn = test_connection();
        insert_profile(&conn, "p1", 9_000);
        insert_client(&conn, "c1", "p1", Some(18_000));

        assert_eq!(resolve_rate_agorot(&conn, Some("missing"), Some("c1"), "p1"), 18_000);
        assert_eq!(resolve_rate_agorot(&conn, None, None, "p1"), 9_000);
        assert_eq!(resolve_rate_agorot(&conn, None, None, "missing"), 0);
    }

    #[test]
    fn auto_close_stale_closes_a_timer_at_the_configured_limit() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        insert_test_running_entry(&conn, "stale", &profile, 20 * 3600, 10_000);

        auto_close_stale(&conn);

        let (ended_at, duration_sec, amount): (Option<String>, i64, i64) = conn
            .query_row(
                "SELECT ended_at, duration_sec, amount_agorot FROM entries WHERE id = 'stale'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("entry");
        assert!(ended_at.is_some(), "stale timer should be closed");
        assert_eq!(duration_sec, 12 * 3600);
        assert_eq!(amount, 120_000);
    }

    #[test]
    fn auto_close_stale_leaves_a_fresh_timer_running() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        insert_test_running_entry(&conn, "fresh", &profile, 3600, 10_000);

        auto_close_stale(&conn);

        let ended_at: Option<String> = conn
            .query_row("SELECT ended_at FROM entries WHERE id = 'fresh'", [], |row| row.get(0))
            .expect("entry");
        assert!(ended_at.is_none(), "fresh timer must keep running");
    }

    #[test]
    fn auto_close_stale_is_disabled_when_the_limit_is_zero() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        set_setting(&conn, "autoStopHours", "0").expect("setting");
        insert_test_running_entry(&conn, "stale", &profile, 20 * 3600, 10_000);

        auto_close_stale(&conn);

        let ended_at: Option<String> = conn
            .query_row("SELECT ended_at FROM entries WHERE id = 'stale'", [], |row| row.get(0))
            .expect("entry");
        assert!(ended_at.is_none(), "a zero limit disables automatic closing");
    }

    #[test]
    fn sync_active_entry_prices_the_running_entry_the_way_stopping_will() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        set_setting(&conn, "roundingMinutes", "15").expect("setting");
        insert_test_running_entry(&conn, "running", &profile, 301, 12_000);

        sync_active_entry(&conn);

        let (duration, amount): (i64, i64) = conn
            .query_row(
                "SELECT duration_sec, amount_agorot FROM entries WHERE id = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("entry");
        assert!(
            (301..900).contains(&duration),
            "the clock keeps counting the real seconds, got {duration}"
        );
        assert_ne!(duration % 900, 0, "the stored duration is not rounded, only the price is");
        assert_eq!(amount, 3_000, "the live amount is the 900s step that stopping charges");
    }

    #[test]
    fn sync_active_entry_charges_the_exact_time_when_rounding_is_off() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        insert_test_running_entry(&conn, "running", &profile, 600, 36_000);

        sync_active_entry(&conn);

        let (duration, amount): (i64, i64) = conn
            .query_row(
                "SELECT duration_sec, amount_agorot FROM entries WHERE id = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("entry");
        assert_eq!(amount, amount_agorot(duration, 36_000));
        assert!((5_900..6_100).contains(&amount), "600s at 360/h is about 60.00, got {amount}");
    }

    #[test]
    fn paused_time_is_kept_out_of_the_running_clock() {
        let conn = test_connection();
        let profile = active_profile_id(&conn);
        insert_test_running_entry(&conn, "running", &profile, 3600, 36_000);

        assert_eq!(
            pause_active_entry(&conn, 1800),
            Some(1800),
            "the sleep is recorded on the running entry"
        );
        sync_active_entry(&conn);

        let (duration, amount): (i64, i64) = conn
            .query_row(
                "SELECT duration_sec, amount_agorot FROM entries WHERE id = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("entry");
        assert!(
            (1795..=1805).contains(&duration),
            "one hour minus the half hour asleep is about 1800s, got {duration}"
        );
        assert!(
            (17_995..=18_050).contains(&amount),
            "half an hour at 360/h is about 180.00, got {amount}"
        );
    }

    #[test]
    fn pausing_without_a_running_timer_does_nothing() {
        let conn = test_connection();
        assert_eq!(pause_active_entry(&conn, 60), None);
        assert_eq!(pause_active_entry(&conn, 0), None);
    }

    #[test]
    fn worked_seconds_never_goes_negative() {
        assert_eq!(worked_seconds(600, 0), 600);
        assert_eq!(worked_seconds(600, 600), 0);
        assert_eq!(worked_seconds(600, 900), 0, "a pause cannot invent negative work");
        assert_eq!(worked_seconds(-5, 0), 0);
    }

    #[test]
    fn a_fresh_database_is_created_at_the_current_schema_version() {
        let conn = test_connection();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        let kind: String = conn
            .query_row("SELECT typeof(hourly_rate_agorot) FROM profiles LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("profile");
        assert_eq!(kind, "integer", "money columns must hold whole agorot");
    }

    /// A database exactly as version 1 left it: real money, no paused column, no version.
    fn legacy_database() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory database");
        create_initial_schema(&conn).expect("v1 schema");
        conn.execute_batch(
            r#"
            INSERT INTO profiles (id, name, hourly_rate, currency, color, is_active, created_at)
                VALUES ('p1', 'עבודה', 250.4, 'ILS', '#7c9cff', 1, '2025-01-01T00:00:00');
            INSERT INTO clients (id, profile_id, name, hourly_rate, created_at)
                VALUES ('c1', 'p1', 'לקוח', 180.01, '2025-01-02T00:00:00');
            INSERT INTO projects (id, profile_id, client_id, name, hourly_rate, budget, created_at)
                VALUES ('pr1', 'p1', 'c1', 'פרויקט', NULL, 5000.55, '2025-01-03T00:00:00');
            INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
                    ended_at, duration_sec, rate, amount, billable, paid, created_at)
                VALUES ('e1', 'p1', 'pr1', 'c1', '', '2025-02-01T09:00:00', '2025-02-01T10:30:00',
                        5400, 83.33, 123.456, 1, 1, '2025-02-01T09:00:00');
            INSERT INTO payments (id, profile_id, client_id, project_id, amount, currency, paid_at, created_at)
                VALUES ('m1', 'p1', 'c1', 'pr1', 19.99, 'ILS', '2025-02-02T10:00:00', '2025-02-02T10:00:00');
            INSERT INTO settings (key, value) VALUES ('currency', 'ILS');
            "#,
        )
        .expect("legacy rows");
        assert_eq!(schema_version(&conn).unwrap(), 0, "an old install has no recorded version");
        conn
    }

    #[test]
    fn upgrading_converts_stored_shekels_to_whole_agorot() {
        let conn = legacy_database();

        migrate(&conn).expect("upgrade");

        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        let profile: i64 = conn
            .query_row("SELECT hourly_rate_agorot FROM profiles WHERE id = 'p1'", [], |row| row.get(0))
            .expect("profile");
        assert_eq!(profile, 25_040, "250.40 shekels an hour the same as before, exactly");

        let client: i64 = conn
            .query_row("SELECT hourly_rate_agorot FROM clients WHERE id = 'c1'", [], |row| row.get(0))
            .expect("client");
        assert_eq!(client, 18_001);

        let (budget, rate): (i64, Option<i64>) = conn
            .query_row(
                "SELECT budget_agorot, hourly_rate_agorot FROM projects WHERE id = 'pr1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("project");
        assert_eq!(budget, 500_055);
        assert_eq!(rate, None, "a project without its own rate stays without one");

        let (rate, amount, kind): (i64, i64, String) = conn
            .query_row(
                "SELECT rate_agorot, amount_agorot, typeof(amount_agorot) FROM entries WHERE id = 'e1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("entry");
        assert_eq!(rate, 8_333);
        assert_eq!(amount, 12_346, "123.456 shekels becomes 12346 agorot");
        assert_eq!(kind, "integer", "the rebuilt column stores whole numbers");

        let paid: i64 = conn
            .query_row("SELECT amount_agorot FROM payments WHERE id = 'm1'", [], |row| row.get(0))
            .expect("payment");
        assert_eq!(paid, 1_999);
    }

    #[test]
    fn upgrading_keeps_every_row_and_adds_the_paused_column() {
        let conn = legacy_database();

        migrate(&conn).expect("upgrade");

        for table in ["profiles", "clients", "projects", "entries", "payments"] {
            let rows: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                .expect("count");
            assert_eq!(rows, 1, "{table} kept its single row");
        }
        let paused: i64 = conn
            .query_row("SELECT paused_sec FROM entries WHERE id = 'e1'", [], |row| row.get(0))
            .expect("paused column");
        assert_eq!(paused, 0, "a finished entry from before the upgrade has nothing paused");
        let currency: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'currency'", [], |row| row.get(0))
            .expect("settings survive the rebuild");
        assert_eq!(currency, "ILS");
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let conn = legacy_database();
        migrate(&conn).expect("first upgrade");
        let before: (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT hourly_rate_agorot FROM profiles WHERE id = 'p1'),
                        (SELECT amount_agorot FROM entries WHERE id = 'e1'),
                        (SELECT amount_agorot FROM payments WHERE id = 'm1')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("values");

        migrate(&conn).expect("second upgrade is a no-op");

        let after: (i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT hourly_rate_agorot FROM profiles WHERE id = 'p1'),
                        (SELECT amount_agorot FROM entries WHERE id = 'e1'),
                        (SELECT amount_agorot FROM payments WHERE id = 'm1')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("values");
        assert_eq!(before, after, "amounts are never multiplied by 100 twice");
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }
}
