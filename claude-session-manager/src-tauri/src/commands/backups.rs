use crate::backup;
use crate::db::Database;
use crate::db::models::{Backup, BackupConfig};
use crate::parser;
use crate::parser::messages::ParsedMessage;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn backup_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<Backup, String> {
    let config = backup::get_backup_config(&db);
    backup::backup_session_file(&db, session_id, &config)
}

#[tauri::command]
pub fn backup_all_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Backup>, String> {
    let config = backup::get_backup_config(&db);
    let conn = db.conn();

    let session_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM sessions")
            .map_err(|e| format!("DB error: {}", e))?;
        let ids = stmt.query_map([], |row| row.get(0))
            .map_err(|e| format!("DB error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        ids
    };
    drop(conn);

    let mut results = Vec::new();
    for sid in session_ids {
        match backup::backup_session_file(&db, sid, &config) {
            Ok(b) => results.push(b),
            Err(e) => eprintln!("Backup failed for session {}: {}", sid, e),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn restore_session_backup(
    db: State<'_, Arc<Database>>,
    backup_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    let (backup_path, compressed): (String, bool) = conn.query_row(
        "SELECT backup_path, compressed FROM backups WHERE id = ?1",
        params![backup_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| format!("Backup not found: {}", e))?;

    backup::restore_backup(&backup_path, compressed)
}

#[tauri::command]
pub fn list_backups(
    db: State<'_, Arc<Database>>,
    session_id: Option<i64>,
) -> Result<Vec<Backup>, String> {
    let conn = db.conn();
    let query = if session_id.is_some() {
        "SELECT id, session_id, backup_path, backup_type, original_size, compressed, created_at
         FROM backups WHERE session_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, session_id, backup_path, backup_type, original_size, compressed, created_at
         FROM backups ORDER BY created_at DESC"
    };

    let mut stmt = conn.prepare(query).map_err(|e| format!("DB error: {}", e))?;

    let backups = if let Some(sid) = session_id {
        stmt.query_map(params![sid], |row| {
            Ok(Backup {
                id: row.get(0)?,
                session_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_type: row.get(3)?,
                original_size: row.get(4)?,
                compressed: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect()
    } else {
        stmt.query_map([], |row| {
            Ok(Backup {
                id: row.get(0)?,
                session_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_type: row.get(3)?,
                original_size: row.get(4)?,
                compressed: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(backups)
}

#[tauri::command]
pub fn delete_backup(
    db: State<'_, Arc<Database>>,
    backup_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    let backup_path: String = conn.query_row(
        "SELECT backup_path FROM backups WHERE id = ?1",
        params![backup_id],
        |row| row.get(0),
    ).map_err(|e| format!("Backup not found: {}", e))?;

    std::fs::remove_file(&backup_path).ok();
    conn.execute("DELETE FROM backups WHERE id = ?1", params![backup_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_backup_messages(
    backup_path: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ParsedMessage>, String> {
    let path = Path::new(&backup_path);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }
    parser::load_messages(path, offset.unwrap_or(0), limit.unwrap_or(200))
}

#[tauri::command]
pub fn migrate_backups_cmd(
    db: State<'_, Arc<Database>>,
    old_dir: String,
    new_dir: String,
) -> Result<u32, String> {
    backup::migrate_backups(&db, &old_dir, &new_dir)
}

#[tauri::command]
pub fn get_backup_config_cmd(db: State<'_, Arc<Database>>) -> Result<BackupConfig, String> {
    Ok(backup::get_backup_config(&db))
}

#[tauri::command]
pub fn set_backup_config_cmd(
    db: State<'_, Arc<Database>>,
    config: BackupConfig,
) -> Result<(), String> {
    backup::save_backup_config(&db, &config)
}
