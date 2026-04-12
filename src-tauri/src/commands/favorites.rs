use crate::db::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoHideConfig {
    pub enabled: bool,
    pub min_message_count: i64,
}

impl Default for AutoHideConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_message_count: 3,
        }
    }
}

#[tauri::command]
pub fn get_auto_hide_config(db: State<'_, Arc<Database>>) -> Result<AutoHideConfig, String> {
    let conn = db.conn();
    let json: Option<String> = conn
        .query_row("SELECT value FROM app_config WHERE key = 'auto_hide_config'", [], |row| row.get(0))
        .ok();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
}

#[tauri::command]
pub fn set_auto_hide_config(db: State<'_, Arc<Database>>, config: AutoHideConfig) -> Result<(), String> {
    let conn = db.conn();
    let json = serde_json::to_string(&config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('auto_hide_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    _note: Option<String>,
) -> Result<bool, String> {
    let conn = db.conn();
    let current: bool = conn.query_row(
        "SELECT is_favorited FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    let new_val = !current;
    conn.execute(
        "UPDATE sessions SET is_favorited = ?1 WHERE id = ?2",
        params![new_val, session_id],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(new_val)
}

#[tauri::command]
pub fn toggle_hide_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<bool, String> {
    let conn = db.conn();
    let current: bool = conn.query_row(
        "SELECT is_hidden FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    let new_val = !current;
    conn.execute(
        "UPDATE sessions SET is_hidden = ?1 WHERE id = ?2",
        params![new_val, session_id],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(new_val)
}
