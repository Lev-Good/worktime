use crate::db;
use crate::models::*;
use crate::AppState;
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension, ToSql};
use std::sync::MutexGuard;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

type CmdResult<T> = Result<T, String>;

const ENTRY_SELECT: &str = "SELECT e.id, e.profile_id, e.project_id, p.name, e.client_id, c.name,
        e.description, e.started_at, e.ended_at, e.duration_sec, e.rate_agorot, e.amount_agorot,
        e.billable, e.paid
     FROM entries e
     LEFT JOIN projects p ON p.id = e.project_id
     LEFT JOIN clients c ON c.id = e.client_id";

fn map_entry(row: &rusqlite::Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        project_id: row.get(2)?,
        project_name: row.get(3)?,
        client_id: row.get(4)?,
        client_name: row.get(5)?,
        description: row.get(6)?,
        started_at: row.get(7)?,
        ended_at: row.get(8)?,
        duration_sec: row.get(9)?,
        rate: db::to_shekels(row.get(10)?),
        amount: db::to_shekels(row.get(11)?),
        billable: row.get::<_, i64>(12)? == 1,
        paid: row.get::<_, i64>(13)? == 1,
    })
}

fn lock<'a>(state: &'a State<'_, AppState>) -> MutexGuard<'a, Connection> {
    state.db.lock().unwrap_or_else(|e| e.into_inner())
}

fn active_entry(conn: &Connection) -> Option<Entry> {
    conn.query_row(
        &format!("{ENTRY_SELECT} WHERE e.ended_at IS NULL ORDER BY e.started_at DESC LIMIT 1"),
        [],
        map_entry,
    )
    .optional()
    .ok()
    .flatten()
}

pub fn snapshot_of(conn: &Connection) -> TimerSnapshot {
    db::sync_active_entry(conn);
    let active = active_entry(conn);
    TimerSnapshot {
        running: active.is_some(),
        elapsed_sec: active.as_ref().map(|e| e.duration_sec).unwrap_or(0),
        project_name: active.as_ref().and_then(|e| e.project_name.clone()),
        client_name: active.as_ref().and_then(|e| e.client_name.clone()),
        description: active
            .as_ref()
            .map(|e| e.description.clone())
            .unwrap_or_default(),
        rate: active.as_ref().map(|e| e.rate).unwrap_or(0.0),
        active,
    }
}

pub fn timer_snapshot(app: &AppHandle) -> TimerSnapshot {
    let state = app.state::<AppState>();
    let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
    snapshot_of(&conn)
}

fn broadcast(app: &AppHandle) {
    let snapshot = timer_snapshot(app);
    let _ = app.emit("tick", &snapshot);
    let _ = app.emit("state-changed", ());
    crate::tray::refresh(app);
}

/// Worked seconds in the range, and the money that range is actually worth in agorot.
///
/// Hours count every entry (the time was spent), money counts only entries marked
/// billable, so the dashboard agrees with the report and with "pending payment".
pub(crate) fn section_total(conn: &Connection, profile: &str, from: &str, to: &str) -> (i64, i64) {
    conn.query_row(
        "SELECT COALESCE(SUM(duration_sec),0),
                COALESCE(SUM(CASE WHEN billable = 1 THEN amount_agorot ELSE 0 END),0)
         FROM entries
         WHERE profile_id = ?1 AND started_at >= ?2 AND started_at <= ?3",
        params![profile, from, to],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap_or((0, 0))
}

/// Money already collected, in agorot: paid and billable entries only.
fn paid_total(conn: &Connection, profile: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(amount_agorot),0) FROM entries
         WHERE profile_id = ?1 AND paid = 1 AND billable = 1",
        params![profile],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

#[tauri::command]
pub fn bootstrap(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Bootstrap> {
    let conn = lock(&state);
    db::sync_active_entry(&conn);
    db::auto_close_stale(&conn);

    let mut profiles = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.name, p.hourly_rate_agorot, p.currency, p.color, p.is_active,
                    p.created_at,
                    COALESCE((SELECT SUM(duration_sec) FROM entries WHERE profile_id = p.id),0)
                 FROM profiles p ORDER BY p.is_active DESC, p.created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Profile {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    hourly_rate: db::to_shekels(row.get(2)?),
                    currency: row.get(3)?,
                    color: row.get(4)?,
                    is_active: row.get::<_, i64>(5)? == 1,
                    created_at: row.get(6)?,
                    total_seconds: row.get(7)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            profiles.push(row);
        }
    }

    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);

    Ok(Bootstrap {
        version: app.package_info().version.to_string(),
        settings: db::all_settings(&conn),
        active_profile_id: db::active_profile_id(&conn),
        profiles,
        timer: snapshot_of(&conn),
        floating_visible: app
            .get_webview_window("floating")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false),
        autostart_enabled,
    })
}

#[tauri::command]
pub fn dashboard(state: State<'_, AppState>) -> CmdResult<Dashboard> {
    let conn = lock(&state);
    db::sync_active_entry(&conn);
    let profile = db::active_profile_id(&conn);

    let (today_seconds, today_earnings) =
        section_total(&conn, &profile, &db::today_start(), &db::today_end());
    let (week_seconds, week_earnings) =
        section_total(&conn, &profile, &db::week_start(), &db::today_end());
    let (month_seconds, _) = section_total(&conn, &profile, &db::month_start(), &db::today_end());

    let unpaid_total: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount_agorot),0) FROM entries
             WHERE profile_id = ?1 AND billable = 1 AND paid = 0",
            params![profile],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let paid_total = paid_total(&conn, &profile);
    let entries_today: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entries WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3",
            params![profile, db::today_start(), db::today_end()],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let active_projects: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE profile_id = ?1 AND archived = 0",
            params![profile],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let active_clients: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clients WHERE profile_id = ?1 AND archived = 0",
            params![profile],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut per_day: Vec<(String, i64)> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT substr(started_at,1,10) AS day, SUM(duration_sec) FROM entries
         WHERE profile_id = ?1 AND started_at >= ?2 GROUP BY day",
    ) {
        if let Ok(rows) = stmt.query_map(params![profile, db::week_start()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }) {
            per_day = rows.flatten().collect();
        }
    }

    let week_start_date = db::week_start()[..10].to_string();
    let names = ["א׳", "ב׳", "ג׳", "ד׳", "ה׳", "ו׳", "ש׳"];
    let mut week_days = Vec::new();
    for index in 0..7 {
        let date = db::add_days(&week_start_date, index);
        let seconds = per_day
            .iter()
            .find(|(d, _)| d == &date)
            .map(|(_, s)| *s)
            .unwrap_or(0);
        week_days.push(DayStat {
            date: date.clone(),
            label: names[index as usize].to_string(),
            seconds,
        });
    }

    Ok(Dashboard {
        today_seconds,
        week_seconds,
        month_seconds,
        today_earnings: db::to_shekels(today_earnings),
        week_earnings: db::to_shekels(week_earnings),
        unpaid_total: db::to_shekels(unpaid_total),
        paid_total: db::to_shekels(paid_total),
        entries_today,
        active_projects,
        active_clients,
        week_days,
    })
}

#[tauri::command]
pub fn list_clients(state: State<'_, AppState>) -> CmdResult<Vec<Client>> {
    let conn = lock(&state);
    let profile = db::active_profile_id(&conn);
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.profile_id, c.name, c.contact, c.email, c.phone, c.address, c.notes,
                    c.hourly_rate_agorot, c.archived, c.created_at,
                    COALESCE((SELECT SUM(duration_sec) FROM entries WHERE client_id = c.id),0),
                    COALESCE((SELECT SUM(amount_agorot) FROM entries WHERE client_id = c.id AND paid = 0 AND billable = 1),0)
             FROM clients c WHERE c.profile_id = ?1
             ORDER BY c.archived ASC, c.name COLLATE NOCASE ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![profile], |row| {
            Ok(Client {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                name: row.get(2)?,
                contact: row.get(3)?,
                email: row.get(4)?,
                phone: row.get(5)?,
                address: row.get(6)?,
                notes: row.get(7)?,
                hourly_rate: row.get::<_, Option<i64>>(8)?.map(db::to_shekels),
                archived: row.get::<_, i64>(9)? == 1,
                created_at: row.get(10)?,
                total_seconds: row.get(11)?,
                unpaid_amount: db::to_shekels(row.get::<_, i64>(12)?),
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn save_client(app: AppHandle, state: State<'_, AppState>, client: ClientInput) -> CmdResult<Client> {
    let name = client.name.trim().to_string();
    if name.is_empty() {
        return Err("יש להזין שם לקוח".into());
    }
    let id = {
        let conn = lock(&state);
        let profile = db::active_profile_id(&conn);
        let id = client.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM clients WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            conn.execute(
                "UPDATE clients SET name = ?1, contact = ?2, email = ?3, phone = ?4, address = ?5,
                    notes = ?6, hourly_rate_agorot = ?7, archived = ?8 WHERE id = ?9",
                params![
                    name,
                    client.contact,
                    client.email,
                    client.phone,
                    client.address,
                    client.notes,
                    client.hourly_rate.map(db::to_agorot),
                    client.archived as i64,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO clients (id, profile_id, name, contact, email, phone, address, notes,
                    hourly_rate_agorot, archived, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    profile,
                    name,
                    client.contact,
                    client.email,
                    client.phone,
                    client.address,
                    client.notes,
                    client.hourly_rate.map(db::to_agorot),
                    client.archived as i64,
                    db::now_str()
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        id
    };
    crate::tray::refresh(&app);
    let conn = lock(&state);
    let profile = db::active_profile_id(&conn);
    conn.query_row(
        "SELECT c.id, c.profile_id, c.name, c.contact, c.email, c.phone, c.address, c.notes,
                c.hourly_rate_agorot, c.archived, c.created_at, 0, 0
         FROM clients c WHERE c.id = ?1 AND c.profile_id = ?2",
        params![id, profile],
        |row| {
            Ok(Client {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                name: row.get(2)?,
                contact: row.get(3)?,
                email: row.get(4)?,
                phone: row.get(5)?,
                address: row.get(6)?,
                notes: row.get(7)?,
                hourly_rate: row.get::<_, Option<i64>>(8)?.map(db::to_shekels),
                archived: row.get::<_, i64>(9)? == 1,
                created_at: row.get(10)?,
                total_seconds: 0,
                unpaid_amount: 0.0,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_client(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute("UPDATE projects SET client_id = NULL WHERE client_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE entries SET client_id = NULL WHERE client_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE payments SET client_id = NULL WHERE client_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM clients WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> CmdResult<Vec<Project>> {
    let conn = lock(&state);
    let profile = db::active_profile_id(&conn);
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.profile_id, p.client_id, c.name, p.name, p.description, p.status,
                    p.hourly_rate_agorot, p.budget_agorot, p.color, p.archived, p.created_at,
                    COALESCE((SELECT SUM(duration_sec) FROM entries WHERE project_id = p.id),0),
                    COALESCE((SELECT SUM(amount_agorot) FROM entries WHERE project_id = p.id AND paid = 0 AND billable = 1),0),
                    (SELECT MAX(started_at) FROM entries WHERE project_id = p.id)
             FROM projects p LEFT JOIN clients c ON c.id = p.client_id
             WHERE p.profile_id = ?1
             ORDER BY p.archived ASC, COALESCE((SELECT MAX(started_at) FROM entries WHERE project_id = p.id), p.created_at) DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![profile], |row| {
            Ok(Project {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                client_id: row.get(2)?,
                client_name: row.get(3)?,
                name: row.get(4)?,
                description: row.get(5)?,
                status: row.get(6)?,
                hourly_rate: row.get::<_, Option<i64>>(7)?.map(db::to_shekels),
                budget: row.get::<_, Option<i64>>(8)?.map(db::to_shekels),
                color: row.get(9)?,
                archived: row.get::<_, i64>(10)? == 1,
                created_at: row.get(11)?,
                total_seconds: row.get(12)?,
                unpaid_amount: db::to_shekels(row.get::<_, i64>(13)?),
                last_used_at: row.get(14)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn save_project(app: AppHandle, state: State<'_, AppState>, project: ProjectInput) -> CmdResult<Project> {
    let name = project.name.trim().to_string();
    if name.is_empty() {
        return Err("יש להזין שם פרויקט".into());
    }
    let id = {
        let conn = lock(&state);
        let profile = db::active_profile_id(&conn);
        let id = project.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let status = project.status.clone().unwrap_or_else(|| "active".into());
        let color = project.color.clone().unwrap_or_else(|| "#7c9cff".into());
        if exists > 0 {
            conn.execute(
                "UPDATE projects SET name = ?1, client_id = ?2, description = ?3, status = ?4,
                    hourly_rate_agorot = ?5, budget_agorot = ?6, color = ?7, archived = ?8 WHERE id = ?9",
                params![
                    name,
                    project.client_id,
                    project.description,
                    status,
                    project.hourly_rate.map(db::to_agorot),
                    project.budget.map(db::to_agorot),
                    color,
                    project.archived as i64,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO projects (id, profile_id, client_id, name, description, status,
                    hourly_rate_agorot, budget_agorot, color, archived, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    profile,
                    project.client_id,
                    name,
                    project.description,
                    status,
                    project.hourly_rate.map(db::to_agorot),
                    project.budget.map(db::to_agorot),
                    color,
                    project.archived as i64,
                    db::now_str()
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        id
    };
    crate::tray::refresh(&app);
    let conn = lock(&state);
    let profile = db::active_profile_id(&conn);
    conn.query_row(
        "SELECT p.id, p.profile_id, p.client_id, c.name, p.name, p.description, p.status,
                p.hourly_rate_agorot, p.budget_agorot, p.color, p.archived, p.created_at, 0, 0, NULL
         FROM projects p LEFT JOIN clients c ON c.id = p.client_id
         WHERE p.id = ?1 AND p.profile_id = ?2",
        params![id, profile],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                client_id: row.get(2)?,
                client_name: row.get(3)?,
                name: row.get(4)?,
                description: row.get(5)?,
                status: row.get(6)?,
                hourly_rate: row.get::<_, Option<i64>>(7)?.map(db::to_shekels),
                budget: row.get::<_, Option<i64>>(8)?.map(db::to_shekels),
                color: row.get(9)?,
                archived: row.get::<_, i64>(10)? == 1,
                created_at: row.get(11)?,
                total_seconds: 0,
                unpaid_amount: 0.0,
                last_used_at: None,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute("UPDATE entries SET project_id = NULL WHERE project_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE payments SET project_id = NULL WHERE project_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    crate::tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn list_entries(
    state: State<'_, AppState>,
    from: Option<String>,
    to: Option<String>,
    client_id: Option<String>,
    project_id: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CmdResult<EntriesPage> {
    let conn = lock(&state);
    db::sync_active_entry(&conn);
    let profile = db::active_profile_id(&conn);
    let from = from.unwrap_or_else(|| db::add_days(&db::today_end()[..10], -30) + "T00:00:00");
    let to = to.unwrap_or_else(db::today_end);
    let query = EntryQuery {
        from: &from,
        to: &to,
        client_id: client_id.as_deref().filter(|value| !value.is_empty()),
        project_id: project_id.as_deref().filter(|value| !value.is_empty()),
        search: search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
        limit: limit.unwrap_or(ENTRIES_PAGE_MAX).clamp(1, ENTRIES_PAGE_MAX),
        offset: offset.unwrap_or(0).max(0),
    };
    Ok(entries_for(&conn, &profile, &query))
}

/// Largest page `list_entries` will return in one go.
const ENTRIES_PAGE_MAX: i64 = 500;

struct EntryQuery<'a> {
    from: &'a str,
    to: &'a str,
    client_id: Option<&'a str>,
    project_id: Option<&'a str>,
    search: Option<&'a str>,
    limit: i64,
    offset: i64,
}

/// The filters shared by the page query and the totals query, so both always agree.
/// It binds `?1`..`?6`; the page query continues with `?7` (limit) and `?8` (offset).
const ENTRIES_FILTER: &str = r#"
    e.profile_id = ?1 AND e.started_at >= ?2 AND e.started_at <= ?3
      AND (?4 IS NULL OR e.client_id = ?4)
      AND (?5 IS NULL OR e.project_id = ?5)
      AND (?6 IS NULL OR e.description LIKE ?6 ESCAPE '\')"#;

/// One page of the journal plus the totals of every matching entry, so the screen never
/// shows fewer rows than the numbers above it account for.
fn entries_for(conn: &Connection, profile: &str, query: &EntryQuery) -> EntriesPage {
    let search = query.search.map(like_pattern);
    // The bind values for `ENTRIES_FILTER`, in the order its placeholders expect. Both
    // statements bind the same list, so a filter cannot reach one query and miss the other.
    let mut binds: Vec<&dyn ToSql> = vec![
        &profile,
        &query.from,
        &query.to,
        &query.client_id,
        &query.project_id,
        &search,
    ];

    // Hours cover every matching entry; money only covers billable ones, exactly like the report.
    let (total, total_seconds, total_amount): (i64, i64, i64) = conn
        .query_row(
            &format!(
                "SELECT COUNT(*), COALESCE(SUM(e.duration_sec),0),
                        COALESCE(SUM(CASE WHEN e.billable = 1 THEN e.amount_agorot ELSE 0 END),0)
                 FROM entries e WHERE {ENTRIES_FILTER}"
            ),
            &binds[..],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    binds.push(&query.limit);
    binds.push(&query.offset);

    let mut entries = Vec::new();
    if let Ok(mut stmt) = conn.prepare(&format!(
        "{ENTRY_SELECT} WHERE {ENTRIES_FILTER}
         ORDER BY e.started_at DESC, e.id DESC LIMIT ?7 OFFSET ?8"
    )) {
        if let Ok(rows) = stmt.query_map(&binds[..], map_entry) {
            entries = rows.flatten().collect();
        }
    }

    EntriesPage {
        has_more: (query.offset + entries.len() as i64) < total,
        entries,
        total,
        total_seconds,
        total_amount: db::to_shekels(total_amount),
    }
}

/// Turns a search term into a LIKE pattern, escaping `%`, `_` and the escape character
/// itself so a term typed by the user only ever matches those characters literally.
fn like_pattern(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for ch in term.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod entries_tests {
    use super::*;

    fn insert_entry(
        conn: &Connection,
        profile: &str,
        id: &str,
        description: &str,
        started_at: &str,
        billable: bool,
    ) {
        conn.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description,
                started_at, ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid,
                created_at)
             VALUES (?1, ?2, NULL, NULL, ?3, ?4, ?4, 3600, 10000, 10000, ?5, 0, ?4)",
            params![id, profile, description, started_at, billable as i64],
        )
        .expect("entry");
    }

    /// A seeded in-memory database with the production schema, shared by the unit tests.
    fn fixture() -> Connection {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        // e2 is not billable: its hour counts as time but must never count as money.
        insert_entry(&conn, &profile, "e0", "עבודה על אתר תדמית", "2026-11-02T09:00:00", true);
        insert_entry(&conn, &profile, "e1", "100% מהמפרט הושלם", "2026-11-02T12:00:00", true);
        insert_entry(&conn, &profile, "e2", "פגישת צוות", "2026-11-03T09:00:00", false);
        conn
    }

    fn query<'a>(from: &'a str, to: &'a str, search: Option<&'a str>) -> EntryQuery<'a> {
        EntryQuery {
            from,
            to,
            client_id: None,
            project_id: None,
            search,
            limit: 50,
            offset: 0,
        }
    }

    #[test]
    fn search_matches_the_description_and_escapes_wildcards() {
        let conn = fixture();
        let profile = db::active_profile_id(&conn);

        let page = entries_for(&conn, &profile, &query("2026-11-01", "2026-11-30", Some("אתר")));
        assert_eq!(page.total, 1);
        assert_eq!(page.entries[0].id, "e0");

        // A "%" typed by the user must match a literal percent sign, not act as a wildcard.
        let percent = entries_for(&conn, &profile, &query("2026-11-01", "2026-11-30", Some("%")));
        assert_eq!(percent.total, 1, "a percent sign is a character, not a wildcard");
        assert_eq!(percent.entries[0].id, "e1");

        let underscore = entries_for(&conn, &profile, &query("2026-11-01", "2026-11-30", Some("_")));
        assert_eq!(underscore.total, 0, "no description contains an underscore");

        // The escape character itself is escaped too, so a path stays a literal search.
        insert_entry(&conn, &profile, "e3", "גיבוי ל-C:\\work", "2026-11-06T09:00:00", true);
        let backslash = entries_for(&conn, &profile, &query("2026-11-01", "2026-11-30", Some("\\")));
        assert_eq!(backslash.total, 1, "a backslash is a character, not an escape");
        assert_eq!(backslash.entries[0].id, "e3");
    }

    #[test]
    fn a_page_reports_the_full_totals_and_whether_more_rows_follow() {
        let conn = fixture();
        let profile = db::active_profile_id(&conn);

        let mut first = query("2026-11-01", "2026-11-30", None);
        first.limit = 2;
        let page = entries_for(&conn, &profile, &first);
        assert_eq!(page.entries.len(), 2, "the page is limited");
        assert_eq!(page.total, 3, "the total is not");
        assert_eq!(page.total_seconds, 10_800, "hours cover every entry");
        assert_eq!(
            page.total_amount, 200.0,
            "money covers only the billable entries"
        );
        assert!(page.has_more);
        assert_eq!(page.entries[0].id, "e2", "newest first");
        assert_eq!(page.entries[1].id, "e1");

        let mut second = query("2026-11-01", "2026-11-30", None);
        second.limit = 2;
        second.offset = 2;
        let page = entries_for(&conn, &profile, &second);
        assert_eq!(page.entries.len(), 1, "the offset skips the rows already shown");
        assert_eq!(page.entries[0].id, "e0");
        assert!(!page.has_more);

        let mut beyond = query("2026-11-01", "2026-11-30", None);
        beyond.offset = 99;
        let empty = entries_for(&conn, &profile, &beyond);
        assert!(empty.entries.is_empty(), "an offset past the end returns no rows");
        assert!(!empty.has_more, "and nothing left to load");
        assert_eq!(empty.total, 3, "the totals still describe the whole range");
    }
}

#[tauri::command]
pub fn start_timer(app: AppHandle, state: State<'_, AppState>, project_id: Option<String>, description: Option<String>) -> CmdResult<TimerSnapshot> {
    {
        let conn = lock(&state);
        stop_active(&conn)?;
        let profile = db::active_profile_id(&conn);
        let client_id: Option<String> = match &project_id {
            Some(pid) => conn
                .query_row(
                    "SELECT client_id FROM projects WHERE id = ?1",
                    params![pid],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .ok()
                .flatten()
                .flatten(),
            None => None,
        };
        let rate = db::resolve_rate_agorot(&conn, project_id.as_deref(), client_id.as_deref(), &profile);
        let description = description.unwrap_or_default().trim().to_string();
        conn.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
                ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 0, ?7, 0, ?8, 0, ?6)",
            params![
                uuid::Uuid::new_v4().to_string(),
                profile,
                project_id,
                client_id,
                description,
                db::now_str(),
                rate,
                db::setting_bool(&conn, "billableDefault", true) as i64
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    broadcast(&app);
    Ok(timer_snapshot(&app))
}

fn stop_active(conn: &Connection) -> Result<(), String> {
    db::sync_active_entry(conn);
    // The rate is read in agorot straight from the row, so the price is computed once,
    // in integers, without a shekel round trip in between.
    let active: Option<(String, i64, i64)> = conn
        .query_row(
            "SELECT id, duration_sec, rate_agorot FROM entries WHERE ended_at IS NULL
             ORDER BY started_at DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();

    if let Some((id, worked, rate_agorot)) = active {
        let rounding = db::setting_i64(conn, "roundingMinutes", 0);
        let seconds = db::round_seconds(worked.max(0), rounding);
        let amount = db::amount_agorot(seconds, rate_agorot);
        conn.execute(
            "UPDATE entries SET ended_at = ?1, duration_sec = ?2, amount_agorot = ?3 WHERE id = ?4",
            params![db::now_str(), seconds, amount, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn stop_timer(app: AppHandle, state: State<'_, AppState>) -> CmdResult<TimerSnapshot> {
    {
        let conn = lock(&state);
        stop_active(&conn)?;
    }
    broadcast(&app);
    Ok(timer_snapshot(&app))
}

#[tauri::command]
pub fn save_entry(app: AppHandle, state: State<'_, AppState>, entry: EntryInput) -> CmdResult<()> {
    let start = entry
        .started_at
        .trim()
        .replace(' ', "T");
    let end = entry.ended_at.trim().replace(' ', "T");
    let started = db::parse_local(&start).ok_or("שעת התחלה לא תקינה")?;
    let ended = db::parse_local(&end).ok_or("שעת סיום לא תקינה")?;
    if ended < started {
        return Err("שעת הסיום חייבת להיות אחרי שעת ההתחלה".into());
    }
    let seconds = (ended - started).num_seconds();
    {
        let conn = lock(&state);
        let profile = db::active_profile_id(&conn);
        let client_id: Option<String> = match &entry.project_id {
            Some(pid) => conn
                .query_row(
                    "SELECT client_id FROM projects WHERE id = ?1",
                    params![pid],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()
                .ok()
                .flatten()
                .flatten(),
            None => None,
        };
        let rate = db::resolve_rate_agorot(&conn, entry.project_id.as_deref(), client_id.as_deref(), &profile);
        let amount = db::amount_agorot(seconds, rate);

        if let Some(id) = &entry.id {
            conn.execute(
                "UPDATE entries SET project_id = ?1, client_id = ?2, description = ?3, started_at = ?4,
                    ended_at = ?5, duration_sec = ?6, rate_agorot = ?7, amount_agorot = ?8,
                    billable = ?9, paid = ?10
                 WHERE id = ?11",
                params![
                    entry.project_id,
                    client_id,
                    entry.description,
                    start,
                    end,
                    seconds,
                    rate,
                    amount,
                    entry.billable as i64,
                    entry.paid as i64,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
                    ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?6)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    profile,
                    entry.project_id,
                    client_id,
                    entry.description,
                    start,
                    end,
                    seconds,
                    rate,
                    amount,
                    entry.billable as i64,
                    entry.paid as i64
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute("DELETE FROM entries WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn set_entry_paid(app: AppHandle, state: State<'_, AppState>, id: String, paid: bool) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute(
            "UPDATE entries SET paid = ?1 WHERE id = ?2",
            params![paid as i64, id],
        )
        .map_err(|e| e.to_string())?;
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn report(state: State<'_, AppState>, from: String, to: String) -> CmdResult<Report> {
    let conn = lock(&state);
    Ok(report_for(&conn, &from, &to))
}

/// Builds the report for a date range. Free of Tauri so it can be unit tested.
fn report_for(conn: &Connection, from: &str, to: &str) -> Report {
    db::sync_active_entry(conn);
    let profile = db::active_profile_id(conn);
    let from_ts = format!("{from}T00:00:00");
    let to_ts = format!("{to}T23:59:59");

    // Hours cover every entry in the range; money only covers billable entries.
    let (total_seconds, total_earnings, entry_count): (i64, i64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(duration_sec),0),
                    COALESCE(SUM(CASE WHEN billable = 1 THEN amount_agorot ELSE 0 END),0), COUNT(*)
             FROM entries WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3",
            params![profile, from_ts, to_ts],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let billable_seconds: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(duration_sec),0) FROM entries
             WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3 AND billable = 1",
            params![profile, from_ts, to_ts],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let paid_earnings: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount_agorot),0) FROM entries
             WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3 AND billable = 1 AND paid = 1",
            params![profile, from_ts, to_ts],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut by_day = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT substr(started_at,1,10) AS day, COALESCE(SUM(duration_sec),0),
                COALESCE(SUM(CASE WHEN billable = 1 THEN amount_agorot ELSE 0 END),0)
         FROM entries WHERE profile_id = ?1 AND started_at BETWEEN ?2 AND ?3
         GROUP BY day ORDER BY day ASC",
    ) {
        if let Ok(rows) = stmt.query_map(params![profile, from_ts, to_ts], |row| {
            let day: String = row.get(0)?;
            let label = db::parse_local(&format!("{day}T00:00:00"))
                .map(|d| d.format("%d/%m").to_string())
                .unwrap_or_else(|| day.clone());
            Ok(SummaryRow {
                key: day,
                label,
                seconds: row.get(1)?,
                earnings: db::to_shekels(row.get::<_, i64>(2)?),
                color: None,
            })
        }) {
            by_day = rows.flatten().collect();
        }
    }

    let mut by_client = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT COALESCE(c.id,''), COALESCE(c.name,'ללא לקוח'), COALESCE(SUM(e.duration_sec),0),
                COALESCE(SUM(CASE WHEN e.billable = 1 THEN e.amount_agorot ELSE 0 END),0)
         FROM entries e LEFT JOIN clients c ON c.id = e.client_id
         WHERE e.profile_id = ?1 AND e.started_at BETWEEN ?2 AND ?3
         GROUP BY COALESCE(c.id,'') ORDER BY 3 DESC",
    ) {
        if let Ok(rows) = stmt.query_map(params![profile, from_ts, to_ts], |row| {
            Ok(SummaryRow {
                key: row.get(0)?,
                label: row.get(1)?,
                seconds: row.get(2)?,
                earnings: db::to_shekels(row.get::<_, i64>(3)?),
                color: None,
            })
        }) {
            by_client = rows.flatten().collect();
        }
    }

    let mut by_project = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT COALESCE(p.id,''), COALESCE(p.name,'ללא פרויקט'), COALESCE(SUM(e.duration_sec),0),
                COALESCE(SUM(CASE WHEN e.billable = 1 THEN e.amount_agorot ELSE 0 END),0), p.color
         FROM entries e LEFT JOIN projects p ON p.id = e.project_id
         WHERE e.profile_id = ?1 AND e.started_at BETWEEN ?2 AND ?3
         GROUP BY COALESCE(p.id,'') ORDER BY 3 DESC",
    ) {
        if let Ok(rows) = stmt.query_map(params![profile, from_ts, to_ts], |row| {
            Ok(SummaryRow {
                key: row.get(0)?,
                label: row.get(1)?,
                seconds: row.get(2)?,
                earnings: db::to_shekels(row.get::<_, i64>(3)?),
                color: row.get(4)?,
            })
        }) {
            by_project = rows.flatten().collect();
        }
    }

    let mut entries = Vec::new();
    if let Ok(mut stmt) = conn.prepare(&format!(
        "{ENTRY_SELECT} WHERE e.profile_id = ?1 AND e.started_at BETWEEN ?2 AND ?3
         ORDER BY e.started_at ASC"
    )) {
        if let Ok(rows) = stmt.query_map(params![profile, from_ts, to_ts], map_entry) {
            entries = rows.flatten().collect();
        }
    }

    Report {
        from: from.to_string(),
        to: to.to_string(),
        total_seconds,
        billable_seconds,
        total_earnings: db::to_shekels(total_earnings),
        paid_earnings: db::to_shekels(paid_earnings),
        unpaid_earnings: db::to_shekels(total_earnings - paid_earnings),
        entry_count,
        by_day,
        by_client,
        by_project,
        entries,
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;

    /// A seeded in-memory database with the production schema, shared by the unit tests.
    fn fixture() -> Connection {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        conn.execute(
            "INSERT INTO clients (id, profile_id, name, created_at)
             VALUES ('c1', ?1, 'client', '2026-01-01T00:00:00')",
            params![profile],
        )
        .expect("client");
        conn.execute(
            "INSERT INTO projects (id, profile_id, client_id, name, created_at)
             VALUES ('pr1', ?1, 'c1', 'project', '2026-01-01T00:00:00')",
            params![profile],
        )
        .expect("project");

        // Billable and still unpaid.
        insert_entry(&conn, &profile, "b1", "2026-11-02T09:00:00", 3600, 10_000, true, false);
        // Not billable but priced: this is the money that must not leak into the totals.
        insert_entry(&conn, &profile, "nb1", "2026-11-02T13:00:00", 7200, 20_000, false, false);
        // Billable and paid.
        insert_entry(&conn, &profile, "b2", "2026-11-03T10:00:00", 1800, 5_000, true, true);
        // A whole day of non-billable work.
        insert_entry(&conn, &profile, "nb2", "2026-11-04T10:00:00", 3600, 9_000, false, false);
        // Non-billable and flagged as paid.
        insert_entry(&conn, &profile, "nb3", "2026-11-05T10:00:00", 3600, 7_000, false, true);

        conn
    }

    fn insert_entry(
        conn: &Connection,
        profile_id: &str,
        id: &str,
        started_at: &str,
        duration_sec: i64,
        amount_agorot: i64,
        billable: bool,
        paid: bool,
    ) {
        conn.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
                ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
             VALUES (?1, ?2, 'pr1', 'c1', '', ?3, ?3, ?4, 10000, ?5, ?6, ?7, ?3)",
            params![
                id,
                profile_id,
                started_at,
                duration_sec,
                amount_agorot,
                billable as i64,
                paid as i64
            ],
        )
        .expect("entry");
    }

    fn row<'a>(rows: &'a [SummaryRow], key: &str) -> &'a SummaryRow {
        rows.iter().find(|row| row.key == key).expect("row")
    }

    #[test]
    fn report_counts_hours_from_every_entry_but_money_only_from_billable_ones() {
        let conn = fixture();
        let report = report_for(&conn, "2026-11-01", "2026-11-30");

        assert_eq!((report.from.as_str(), report.to.as_str()), ("2026-11-01", "2026-11-30"));
        assert_eq!(report.entry_count, 5);
        assert_eq!(report.total_seconds, 19_800, "every entry counts towards the hours");
        assert_eq!(report.billable_seconds, 5_400, "3600 + 1800");
        assert_eq!(report.total_earnings, 150.0, "100 + 50, not the 460 stored on all five");
        assert_eq!(report.paid_earnings, 50.0, "the paid non-billable 70 is not earnings");
        assert_eq!(report.unpaid_earnings, 100.0);

        let day = row(&report.by_day, "2026-11-02");
        assert_eq!(day.seconds, 10_800, "the non-billable 7200s still shows up as hours");
        assert_eq!(day.earnings, 100.0, "but only the billable 100 is money");
        assert_eq!(row(&report.by_day, "2026-11-04").seconds, 3_600);
        assert_eq!(row(&report.by_day, "2026-11-04").earnings, 0.0);

        let client = row(&report.by_client, "c1");
        assert_eq!(client.seconds, 19_800);
        assert_eq!(client.earnings, 150.0);

        let project = row(&report.by_project, "pr1");
        assert_eq!(project.seconds, 19_800);
        assert_eq!(project.earnings, 150.0);
    }

    #[test]
    fn report_shows_hours_but_no_money_for_a_non_billable_day() {
        let conn = fixture();
        let report = report_for(&conn, "2026-11-04", "2026-11-04");

        assert_eq!(report.entry_count, 1);
        assert_eq!(report.total_seconds, 3_600, "the hours are still reported");
        assert_eq!(report.billable_seconds, 0);
        assert_eq!(report.total_earnings, 0.0);
        assert_eq!(report.paid_earnings, 0.0);
        assert_eq!(report.unpaid_earnings, 0.0, "nothing is outstanding either");
        assert_eq!(row(&report.by_day, "2026-11-04").earnings, 0.0);
        assert_eq!(row(&report.by_client, "c1").earnings, 0.0);
        assert_eq!(row(&report.by_project, "pr1").earnings, 0.0);
    }
}

#[tauri::command]
pub fn export_report_csv(app: AppHandle, state: State<'_, AppState>, from: String, to: String) -> CmdResult<String> {
    let data = report(state, from, to)?;
    let mut csv = String::from("\u{feff}תאריך,התחלה,סיום,לקוח,פרויקט,תיאור,שעות,תעריף,סכום,לחיוב,שולם\n");
    for entry in &data.entries {
        let hours = (entry.duration_sec as f64 / 3600.0 * 100.0).round() / 100.0;
        let row = [
            entry.started_at[..10].to_string(),
            entry.started_at[11..16].to_string(),
            entry
                .ended_at
                .as_ref()
                .map(|e| e.get(11..16).unwrap_or("").to_string())
                .unwrap_or_default(),
            entry.client_name.clone().unwrap_or_default(),
            entry.project_name.clone().unwrap_or_default(),
            entry.description.clone(),
            format!("{hours:.2}"),
            format!("{:.2}", entry.rate),
            format!("{:.2}", entry.amount),
            if entry.billable { "כן" } else { "לא" }.to_string(),
            if entry.paid { "כן" } else { "לא" }.to_string(),
        ];
        csv.push_str(
            &row.iter()
                .map(|cell| format!("\"{}\"", cell.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }

    let dir = dirs_backup_dir(&app)?;
    let path = dir.join(format!("worktime-report-{}-{}.csv", data.from, data.to));
    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn dirs_backup_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let base = app
        .path()
        .document_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|e| e.to_string())?;
    let dir = base.join("WorkTime");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[tauri::command]
pub fn list_payments(state: State<'_, AppState>) -> CmdResult<Vec<Payment>> {
    let conn = lock(&state);
    let profile = db::active_profile_id(&conn);
    let mut stmt = conn
        .prepare(
            "SELECT pm.id, pm.profile_id, pm.client_id, c.name, pm.project_id, p.name,
                    pm.amount_agorot, pm.currency, pm.method, pm.note, pm.paid_at, pm.created_at
             FROM payments pm
             LEFT JOIN clients c ON c.id = pm.client_id
             LEFT JOIN projects p ON p.id = pm.project_id
             WHERE pm.profile_id = ?1 ORDER BY pm.paid_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![profile], |row| {
            Ok(Payment {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                client_id: row.get(2)?,
                client_name: row.get(3)?,
                project_id: row.get(4)?,
                project_name: row.get(5)?,
                amount: db::to_shekels(row.get(6)?),
                currency: row.get(7)?,
                method: row.get(8)?,
                note: row.get(9)?,
                paid_at: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn save_payment(app: AppHandle, state: State<'_, AppState>, payment: PaymentInput) -> CmdResult<()> {
    let amount_agorot = db::to_agorot(payment.amount);
    if amount_agorot <= 0 {
        return Err("יש להזין סכום חיובי".into());
    }
    {
        let conn = lock(&state);
        let profile = db::active_profile_id(&conn);
        let currency = db::setting_or(&conn, "currency", "ILS");
        if let Some(id) = &payment.id {
            conn.execute(
                "UPDATE payments SET client_id = ?1, project_id = ?2, amount_agorot = ?3,
                    method = ?4, note = ?5, paid_at = ?6 WHERE id = ?7",
                params![
                    payment.client_id,
                    payment.project_id,
                    amount_agorot,
                    payment.method,
                    payment.note,
                    payment.paid_at,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO payments (id, profile_id, client_id, project_id, amount_agorot,
                    currency, method, note, paid_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    profile,
                    payment.client_id,
                    payment.project_id,
                    amount_agorot,
                    currency,
                    payment.method,
                    payment.note,
                    payment.paid_at,
                    db::now_str()
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        if payment.mark_entries_paid {
            match (&payment.client_id, &payment.project_id) {
                (Some(cid), Some(pid)) => {
                    conn.execute(
                        "UPDATE entries SET paid = 1 WHERE profile_id = ?1 AND client_id = ?2 AND project_id = ?3 AND billable = 1",
                        params![profile, cid, pid],
                    )
                    .map_err(|e| e.to_string())?;
                }
                (Some(cid), None) => {
                    conn.execute(
                        "UPDATE entries SET paid = 1 WHERE profile_id = ?1 AND client_id = ?2 AND billable = 1",
                        params![profile, cid],
                    )
                    .map_err(|e| e.to_string())?;
                }
                (None, Some(pid)) => {
                    conn.execute(
                        "UPDATE entries SET paid = 1 WHERE profile_id = ?1 AND project_id = ?2 AND billable = 1",
                        params![profile, pid],
                    )
                    .map_err(|e| e.to_string())?;
                }
                (None, None) => {
                    conn.execute(
                        "UPDATE entries SET paid = 1 WHERE profile_id = ?1 AND billable = 1",
                        params![profile],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_payment(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute("DELETE FROM payments WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn list_notifications(state: State<'_, AppState>) -> CmdResult<Vec<NotificationItem>> {
    let conn = lock(&state);
    let mut stmt = conn
        .prepare(
            "SELECT id, title, body, kind, created_at, read FROM notifications
             ORDER BY created_at DESC LIMIT 120",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(NotificationItem {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                kind: row.get(3)?,
                created_at: row.get(4)?,
                read: row.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

#[tauri::command]
pub fn mark_notification(state: State<'_, AppState>, id: String, read: bool) -> CmdResult<()> {
    let conn = lock(&state);
    conn.execute(
        "UPDATE notifications SET read = ?1 WHERE id = ?2",
        params![read as i64, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_notifications(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = lock(&state);
        conn.execute("DELETE FROM notifications", []).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("state-changed", ());
    Ok(())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: std::collections::HashMap<String, String>) -> CmdResult<std::collections::HashMap<String, String>> {
    {
        let conn = lock(&state);
        for (key, value) in &settings {
            db::set_setting(&conn, key, value).map_err(|e| e.to_string())?;
        }

        if let Some(value) = settings.get("autostart") {
            let enabled = value == "true";
            let manager = app.autolaunch();
            let result = if enabled { manager.enable() } else { manager.disable() };
            let _ = result;
        }
    }

    if let Some(value) = settings.get("floatingEnabled") {
        let _ = crate::set_floating_visible(&app, value == "true");
    }
    apply_floating_options(&app);
    let _ = app.emit("settings-changed", ());
    let _ = app.emit("state-changed", ());
    crate::tray::refresh(&app);
    let conn = lock(&state);
    Ok(db::all_settings(&conn))
}

fn apply_floating_options(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("floating") {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let on_top = db::setting_bool(&conn, "floatingAlwaysOnTop", true);
        let _ = window.set_always_on_top(on_top);
    }
    let _ = app.emit("floating-options", ());
}

#[tauri::command]
pub fn save_profile(app: AppHandle, state: State<'_, AppState>, profile: ProfileInput) -> CmdResult<Profile> {
    let name = profile.name.trim().to_string();
    if name.is_empty() {
        return Err("יש להזין שם פרופיל".into());
    }
    let id = {
        let conn = lock(&state);
        let id = profile.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles WHERE id = ?1", params![id], |row| row.get(0))
            .unwrap_or(0);
        let currency = profile
            .currency
            .clone()
            .unwrap_or_else(|| db::setting_or(&conn, "currency", "ILS"));
        let color = profile.color.clone().unwrap_or_else(|| "#7c9cff".into());
        if exists > 0 {
            conn.execute(
                "UPDATE profiles SET name = ?1, hourly_rate_agorot = ?2, currency = ?3, color = ?4
                 WHERE id = ?5",
                params![
                    name,
                    db::to_agorot(profile.hourly_rate),
                    currency,
                    color,
                    id
                ],
            )
            .map_err(|e| e.to_string())?;
        } else {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
                .unwrap_or(0);
            conn.execute(
                "INSERT INTO profiles (id, name, hourly_rate_agorot, currency, color, is_active,
                    created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    name,
                    db::to_agorot(profile.hourly_rate),
                    currency,
                    color,
                    (count == 0) as i64,
                    db::now_str()
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        id
    };
    crate::tray::refresh(&app);
    let conn = lock(&state);
    conn.query_row(
        "SELECT id, name, hourly_rate_agorot, currency, color, is_active, created_at, 0
         FROM profiles WHERE id = ?1",
        params![id],
        |row| {
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                hourly_rate: db::to_shekels(row.get(2)?),
                currency: row.get(3)?,
                color: row.get(4)?,
                is_active: row.get::<_, i64>(5)? == 1,
                created_at: row.get(6)?,
                total_seconds: 0,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))
            .unwrap_or(0);
        if count <= 1 {
            return Err("לא ניתן למחוק את הפרופיל היחיד במערכת".into());
        }
        let was_active: i64 = conn
            .query_row(
                "SELECT is_active FROM profiles WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute("DELETE FROM entries WHERE profile_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM payments WHERE profile_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM projects WHERE profile_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM clients WHERE profile_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM profiles WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        if was_active == 1 {
            if let Ok(next) = conn.query_row("SELECT id FROM profiles LIMIT 1", [], |row| {
                row.get::<_, String>(0)
            }) {
                conn.execute("UPDATE profiles SET is_active = 1 WHERE id = ?1", params![next])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn switch_profile(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        stop_active(&conn)?;
        conn.execute("UPDATE profiles SET is_active = 0", [])
            .map_err(|e| e.to_string())?;
        conn.execute("UPDATE profiles SET is_active = 1 WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
    }
    broadcast(&app);
    Ok(())
}

#[tauri::command]
pub fn set_floating(app: AppHandle, state: State<'_, AppState>, visible: bool) -> CmdResult<bool> {
    {
        let conn = lock(&state);
        db::set_setting(&conn, "floatingEnabled", if visible { "true" } else { "false" })
            .map_err(|e| e.to_string())?;
    }
    let result = crate::set_floating_visible(&app, visible)?;
    crate::tray::refresh(&app);
    let _ = app.emit("state-changed", ());
    Ok(result)
}

#[tauri::command]
pub fn timer(state: State<'_, AppState>) -> CmdResult<TimerSnapshot> {
    let conn = lock(&state);
    Ok(snapshot_of(&conn))
}

#[tauri::command]
pub async fn check_updates(app: AppHandle, force: Option<bool>) -> CmdResult<UpdateInfo> {
    let (repo, current) = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        (
            db::setting_or(&conn, "updateRepo", "Lev-Good/worktime"),
            app.package_info().version.to_string(),
        )
    };
    let info = crate::updates::check(&repo, &current).await.map_err(|e| e)?;
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::set_setting(&conn, "lastUpdateCheck", &db::now_str());
        if info.has_update && force.unwrap_or(false) {
            let _ = db::push_notification(
                &conn,
                &db::active_profile_id(&conn),
                &format!("גרסה חדשה זמינה: {}", info.latest_version),
                "עדכון התוכנה זמין להורדה מדף המהדורה בגיטהאב.",
                "update",
            );
        }
    }
    Ok(info)
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("כתובת לא חוקית".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::flush_floating_position(&app);
    app.exit(0);
}

#[tauri::command]
pub fn backup_local(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let json = {
        let conn = lock(&state);
        crate::backup::export_json(&conn)?
    };
    let dir = dirs_backup_dir(&app)?.join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "worktime-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn notify(app: AppHandle, state: State<'_, AppState>, title: String, body: String) -> CmdResult<()> {
    {
        let conn = lock(&state);
        if !db::setting_bool(&conn, "notificationsEnabled", true) {
            return Ok(());
        }
        let profile = db::active_profile_id(&conn);
        let _ = db::push_notification(&conn, &profile, &title, &body, "info");
    }
    let _ = app
        .notification()
        .builder()
        .title(&title)
        .body(&body)
        .show();
    let _ = app.emit("state-changed", ());
    Ok(())
}

pub fn spawn_ticker(app: AppHandle) {
    std::thread::spawn(move || {
        let mut tick: u64 = 0;
        let mut last_beat = Local::now().naive_local();
        let mut idle_notified = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            tick += 1;
            // The wall clock, not a monotonic timer: it is what jumps forward when the
            // machine suspends, which is exactly the gap that must not be billed.
            let now = Local::now().naive_local();
            let gap = (now - last_beat).num_seconds();
            last_beat = now;
            pause_unavailable(&app, gap, &mut idle_notified);
            crate::flush_floating_position(&app);
            let snapshot = timer_snapshot(&app);
            let _ = app.emit("tick", &snapshot);
            crate::tray::sync_status(&app, &snapshot);
            if tick % 30 == 0 {
                crate::reminders::check(&app);
                crate::gdrive::auto_backup_if_due(&app);
            }
        }
    });
}

/// The seconds of a tick that cannot have been worked: a suspended machine (the ticker
/// missed its beat by far more than a second) or a user who has been away past the limit.
pub fn unavailable_seconds(gap: i64, idle_millis: Option<u64>, idle_limit_minutes: i64) -> i64 {
    if gap <= 0 {
        return 0;
    }
    if gap >= crate::idle::SLEEP_GAP_SECONDS {
        return gap;
    }
    match idle_millis {
        Some(millis) if idle_limit_minutes > 0 && millis >= idle_limit_minutes as u64 * 60_000 => gap,
        _ => 0,
    }
}

/// Keeps suspended and idle time out of the running entry, and says so once per episode so
/// the user knows why the number stopped moving.
fn pause_unavailable(app: &AppHandle, gap: i64, idle_notified: &mut bool) {
    let seconds = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let limit = db::setting_i64(&conn, "idlePauseMinutes", db::DEFAULT_IDLE_PAUSE_MINUTES);
        unavailable_seconds(gap, crate::idle::idle_millis(), limit)
    };
    if seconds <= 0 {
        *idle_notified = false;
        return;
    }

    let paused = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        db::pause_active_entry(&conn, seconds)
    };
    if paused.is_none() {
        return;
    }

    if gap >= crate::idle::SLEEP_GAP_SECONDS {
        *idle_notified = false;
        notice(
            app,
            "זמן שינה לא נספר",
            &format!(
                "המחשב לא הגיב במשך {} דקות, ולכן הזמן הזה לא נכלל ברשומה.",
                (gap / 60).max(1)
            ),
            "warning",
        );
    } else if !*idle_notified {
        *idle_notified = true;
        notice(
            app,
            "הטיימר הושהה — אין פעילות",
            "לא זוהתה פעילות במקלדת או בעכבר, ולכן הזמן לא נכלל ברשומה. הוא ימשיך להיספר ברגע שתחזרו.",
            "warning",
        );
    }
}

/// Records a notification in the app and shows it on the desktop, unless the user muted them.
fn notice(app: &AppHandle, title: &str, body: &str, kind: &str) {
    {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap_or_else(|e| e.into_inner());
        if !db::setting_bool(&conn, "notificationsEnabled", true) {
            return;
        }
        let profile = db::active_profile_id(&conn);
        let _ = db::push_notification(&conn, &profile, title, body, kind);
    }
    let _ = app.notification().builder().title(title).body(body).show();
    let _ = app.emit("state-changed", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_active_rounds_up_and_prices_the_entry() {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        db::set_setting(&conn, "roundingMinutes", "15").expect("setting");
        db::insert_test_running_entry(&conn, "running", &profile, 301, 12_000);

        stop_active(&conn).expect("stop");

        let (ended_at, duration_sec, amount): (Option<String>, i64, i64) = conn
            .query_row(
                "SELECT ended_at, duration_sec, amount_agorot FROM entries WHERE id = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("entry");
        assert!(ended_at.is_some(), "the timer should be closed");
        assert_eq!(duration_sec, 900, "301s rounds up to a 15 minute step");
        assert_eq!(amount, 3_000, "900s at 120/hour is 3000 agorot");
    }

    #[test]
    fn stop_active_is_a_no_op_when_no_timer_is_running() {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        db::insert_test_running_entry(&conn, "running", &profile, 600, 10_000);

        stop_active(&conn).expect("first stop");
        let closed: (i64, i64) = conn
            .query_row(
                "SELECT duration_sec, amount_agorot FROM entries WHERE id = 'running'",
                [],
                |row| {
                    Ok((row.get(0)?, row.get(1)?))
                },
            )
            .expect("entry");

        stop_active(&conn).expect("second stop is a no-op");

        let after: (i64, i64, i64) = conn
            .query_row(
                "SELECT duration_sec, amount_agorot, (SELECT COUNT(*) FROM entries) FROM entries WHERE id = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("entry");
        assert_eq!(
            after,
            (closed.0, closed.1, 1),
            "a second stop must not touch the closed entry or open a new one"
        );
    }

    fn insert_finished_entry(
        conn: &Connection,
        id: &str,
        profile: &str,
        hour: u32,
        duration_sec: i64,
        amount_agorot: i64,
        billable: bool,
        paid: bool,
    ) {
        let started_at = Local::now()
            .date_naive()
            .and_hms_opt(hour, 0, 0)
            .expect("clock time")
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        conn.execute(
            "INSERT INTO entries (id, profile_id, project_id, client_id, description, started_at,
                ended_at, duration_sec, rate_agorot, amount_agorot, billable, paid, created_at)
             VALUES (?1, ?2, NULL, NULL, '', ?3, ?3, ?4, 10000, ?5, ?6, ?7, ?3)",
            params![
                id,
                profile,
                started_at,
                duration_sec,
                amount_agorot,
                billable as i64,
                paid as i64
            ],
        )
        .expect("insert entry");
    }

    #[test]
    fn section_total_counts_every_hour_but_only_billable_money() {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        insert_finished_entry(&conn, "billed", &profile, 9, 3600, 10_000, true, false);
        insert_finished_entry(&conn, "internal", &profile, 10, 1800, 5_000, false, false);
        insert_finished_entry(&conn, "short", &profile, 11, 600, 2_000, true, false);

        let (seconds, amount) =
            section_total(&conn, &profile, &db::today_start(), &db::today_end());

        assert_eq!(seconds, 6000, "hours still include the non-billable entry");
        assert_eq!(amount, 12_000, "money counts the billable entries only");
    }

    #[test]
    fn paid_total_ignores_entries_paid_but_marked_not_billable() {
        let conn = db::test_connection();
        let profile = db::active_profile_id(&conn);
        insert_finished_entry(&conn, "paid-billable", &profile, 9, 3600, 10_000, true, true);
        insert_finished_entry(&conn, "paid-extra", &profile, 10, 3600, 5_000, false, true);
        insert_finished_entry(&conn, "open", &profile, 11, 3600, 3_000, true, false);

        assert_eq!(paid_total(&conn, &profile), 10_000);
    }

    #[test]
    fn unavailable_seconds_skips_sleep_and_respects_the_idle_limit() {
        // A tick that arrives on time is work, whatever the keyboard is doing.
        assert_eq!(unavailable_seconds(1, None, 15), 0);
        assert_eq!(unavailable_seconds(1, Some(60_000), 15), 0, "a quiet minute is not idle");

        // No input past the limit: the tick stops being work.
        assert_eq!(unavailable_seconds(1, Some(15 * 60_000), 15), 1);
        assert_eq!(unavailable_seconds(1, Some(10 * 60_000), 15), 0);
        assert_eq!(unavailable_seconds(1, Some(10 * 60_000), 0), 0, "0 turns the idle pause off");

        // A missed beat of a minute and a half or more means the machine was suspended.
        assert_eq!(
            unavailable_seconds(crate::idle::SLEEP_GAP_SECONDS, Some(0), 15),
            crate::idle::SLEEP_GAP_SECONDS
        );
        assert_eq!(unavailable_seconds(3 * 3600, None, 15), 3 * 3600, "a whole sleep is skipped");
        assert_eq!(unavailable_seconds(0, Some(60 * 60_000), 15), 0, "a zero gap adds nothing");
    }
}
