use crate::db::Database;

use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn toggle_favorite(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    note: Option<String>,
) -> Result<bool, String> {
    let conn = db.conn();

    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM favorites WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, i64>(0),
    ).map_err(|e| format!("DB error: {}", e))? > 0;

    if exists {
        conn.execute("DELETE FROM favorites WHERE session_id = ?1", params![session_id])
            .map_err(|e| format!("DB error: {}", e))?;
        Ok(false)
    } else {
        conn.execute(
            "INSERT INTO favorites (session_id, note, created_at) VALUES (?1, ?2, ?3)",
            params![session_id, note, chrono::Utc::now().timestamp_millis()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(true)
    }
}
