use crate::db::Database;
use crate::ignore::{read_config, write_config, IgnoreConfig};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_ignore_config(db: State<'_, Arc<Database>>) -> Result<IgnoreConfig, String> {
    let conn = db.conn();
    Ok(read_config(&conn))
}

#[tauri::command]
pub fn set_ignore_config(
    db: State<'_, Arc<Database>>,
    config: IgnoreConfig,
) -> Result<(), String> {
    let conn = db.conn();
    write_config(&conn, &config)
}
