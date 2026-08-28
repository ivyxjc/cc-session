use crate::db::Database;
use crate::db::models::Project;
use crate::sources::Provider;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_projects(
    db: State<'_, Arc<Database>>,
    provider: Option<Provider>,
    sort_by: Option<String>,
) -> Result<Vec<Project>, String> {
    let conn = db.conn();
    let order = match sort_by.as_deref() {
        Some("name") => "is_starred DESC, display_name ASC",
        Some("sessions") => "is_starred DESC, session_count DESC",
        _ => "is_starred DESC, last_active DESC NULLS LAST",
    };

    let where_clause = match provider {
        Some(_) => "WHERE provider = ?1",
        None => "",
    };
    let query = format!(
        "SELECT id, provider, encoded_path, original_path, display_name, session_count, last_active, is_starred
         FROM projects {} ORDER BY {}",
        where_clause, order
    );

    let mut stmt = conn.prepare(&query).map_err(|e| format!("DB error: {}", e))?;
    let map_row = |row: &rusqlite::Row<'_>| {
        Ok(Project {
            id: row.get(0)?,
            provider: row.get(1)?,
            encoded_path: row.get(2)?,
            original_path: row.get(3)?,
            display_name: row.get(4)?,
            session_count: row.get(5)?,
            last_active: row.get(6)?,
            is_starred: row.get(7)?,
        })
    };
    let rows = match provider {
        Some(p) => stmt.query_map(params![p], map_row),
        None => stmt.query_map([], map_row),
    };
    let projects = rows
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(projects)
}

#[tauri::command]
pub fn toggle_star_project(
    db: State<'_, Arc<Database>>,
    project_id: i64,
) -> Result<bool, String> {
    let conn = db.conn();
    let current: bool = conn.query_row(
        "SELECT is_starred FROM projects WHERE id = ?1",
        params![project_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    let new_val = !current;
    conn.execute(
        "UPDATE projects SET is_starred = ?1 WHERE id = ?2",
        params![new_val, project_id],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(new_val)
}
