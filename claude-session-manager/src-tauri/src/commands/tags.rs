use crate::db::Database;
use crate::db::models::Tag;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn create_tag(
    db: State<'_, Arc<Database>>,
    name: String,
    color: String,
) -> Result<Tag, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO tags (name, color) VALUES (?1, ?2)",
        params![name, color],
    ).map_err(|e| format!("DB error: {}", e))?;

    let id = conn.last_insert_rowid();
    Ok(Tag { id, name, color })
}

#[tauri::command]
pub fn delete_tag(
    db: State<'_, Arc<Database>>,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM session_tags WHERE tag_id = ?1", params![tag_id])
        .map_err(|e| format!("DB error: {}", e))?;
    conn.execute("DELETE FROM tags WHERE id = ?1", params![tag_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn list_tags(db: State<'_, Arc<Database>>) -> Result<Vec<Tag>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(|e| format!("DB error: {}", e))?;

    let tags = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(tags)
}

#[tauri::command]
pub fn tag_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR IGNORE INTO session_tags (session_id, tag_id) VALUES (?1, ?2)",
        params![session_id, tag_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn untag_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM session_tags WHERE session_id = ?1 AND tag_id = ?2",
        params![session_id, tag_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}
