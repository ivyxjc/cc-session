use crate::db::Database;
use crate::db::models::Project;

use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_projects(
    db: State<'_, Arc<Database>>,
    sort_by: Option<String>,
) -> Result<Vec<Project>, String> {
    let conn = db.conn();
    let order = match sort_by.as_deref() {
        Some("name") => "display_name ASC",
        Some("sessions") => "session_count DESC",
        _ => "last_active DESC NULLS LAST",
    };

    let query = format!(
        "SELECT id, encoded_path, original_path, display_name, session_count, last_active
         FROM projects ORDER BY {}",
        order
    );

    let mut stmt = conn.prepare(&query).map_err(|e| format!("DB error: {}", e))?;
    let projects = stmt.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            encoded_path: row.get(1)?,
            original_path: row.get(2)?,
            display_name: row.get(3)?,
            session_count: row.get(4)?,
            last_active: row.get(5)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(projects)
}
