use crate::db::Database;
use crate::db::models::BackupConfig;
use crate::sources::Provider;
use rusqlite::params;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn get_backup_config(db: &Arc<Database>) -> BackupConfig {
    let conn = db.conn();
    let json: Option<String> = conn.query_row(
        "SELECT value FROM app_config WHERE key = 'backup_config'",
        [],
        |row| row.get(0),
    ).ok();

    json.and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

pub fn save_backup_config(db: &Arc<Database>, config: &BackupConfig) -> Result<(), String> {
    let conn = db.conn();
    let json = serde_json::to_string(config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('backup_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

pub fn backup_session_file(
    db: &Arc<Database>,
    session_db_id: i64,
    config: &BackupConfig,
) -> Result<crate::db::models::Backup, String> {
    let conn = db.conn();

    let (provider, jsonl_path, session_id, project_encoded): (Provider, String, String, String) = conn.query_row(
        "SELECT s.provider, s.jsonl_path, s.session_id, p.encoded_path
         FROM sessions s JOIN projects p ON s.project_id = p.id
         WHERE s.id = ?1",
        params![session_db_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let source = Path::new(&jsonl_path);
    if !source.exists() {
        return Err(format!("Source file not found: {}", jsonl_path));
    }

    let original_size = source.metadata()
        .map_err(|e| format!("Metadata error: {}", e))?.len() as i64;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let backup_dir = PathBuf::from(&config.backup_dir)
        .join(project_dir_key(&project_encoded))
        .join(&session_id);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup dir: {}", e))?;

    let backup_filename = if config.compress {
        format!("{}.jsonl.zst", timestamp)
    } else {
        format!("{}.jsonl", timestamp)
    };
    let backup_path = backup_dir.join(&backup_filename);

    // Read source
    let mut source_data = Vec::new();
    fs::File::open(source)
        .map_err(|e| format!("Read error: {}", e))?
        .read_to_end(&mut source_data)
        .map_err(|e| format!("Read error: {}", e))?;

    // Write backup (optionally compressed)
    if config.compress {
        let compressed = zstd::encode_all(source_data.as_slice(), 3)
            .map_err(|e| format!("Compression error: {}", e))?;
        fs::write(&backup_path, compressed)
            .map_err(|e| format!("Write error: {}", e))?;
    } else {
        fs::write(&backup_path, &source_data)
            .map_err(|e| format!("Write error: {}", e))?;
    }

    // Also backup subagent files
    let subagent_dir = Path::new(&jsonl_path)
        .parent().unwrap_or(Path::new(""))
        .join(&session_id).join("subagents");
    if subagent_dir.exists() {
        let backup_subagent_dir = backup_dir.join("subagents");
        fs::create_dir_all(&backup_subagent_dir).ok();
        for entry in fs::read_dir(&subagent_dir).map_err(|e| format!("Read error: {}", e))? {
            let entry = entry.map_err(|e| format!("Read error: {}", e))?;
            let src_path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.ends_with(".meta.json") {
                fs::copy(&src_path, backup_subagent_dir.join(&filename)).ok();
            } else if filename.ends_with(".jsonl") {
                let mut data = Vec::new();
                fs::File::open(&src_path).ok()
                    .map(|mut f| f.read_to_end(&mut data));
                if config.compress {
                    let compressed = zstd::encode_all(data.as_slice(), 3).ok();
                    if let Some(c) = compressed {
                        fs::write(backup_subagent_dir.join(format!("{}.zst", filename)), c).ok();
                    }
                } else {
                    fs::write(backup_subagent_dir.join(&filename), &data).ok();
                }
            }
        }
    }

    let backup_path_str = backup_path.to_string_lossy().to_string();

    // Record in DB
    conn.execute(
        "INSERT INTO backups (provider, session_id, backup_path, backup_type, original_size, compressed, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![provider, session_db_id, backup_path_str, "manual", original_size, config.compress, timestamp],
    ).map_err(|e| format!("DB error: {}", e))?;

    conn.execute(
        "UPDATE sessions SET is_backed_up = 1 WHERE id = ?1",
        params![session_db_id],
    ).ok();

    // Enforce max_backup_copies
    let backups: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, backup_path FROM backups WHERE session_id = ?1 ORDER BY created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        let result = stmt.query_map(params![session_db_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("DB error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    if backups.len() > config.max_backup_copies as usize {
        for (id, path) in backups.iter().skip(config.max_backup_copies as usize) {
            fs::remove_file(path).ok();
            conn.execute("DELETE FROM backups WHERE id = ?1", params![id]).ok();
        }
    }

    let backup_id = conn.last_insert_rowid();
    Ok(crate::db::models::Backup {
        id: backup_id,
        session_id: session_db_id,
        backup_path: backup_path_str,
        backup_type: "manual".to_string(),
        original_size,
        compressed: config.compress,
        created_at: timestamp,
    })
}

pub fn migrate_backups(db: &Arc<Database>, old_dir: &str, new_dir: &str) -> Result<u32, String> {
    let old_path = Path::new(old_dir);
    let new_path = Path::new(new_dir);

    if !old_path.exists() {
        return Ok(0);
    }
    if old_dir == new_dir {
        return Ok(0);
    }

    fs::create_dir_all(new_path)
        .map_err(|e| format!("Failed to create new backup dir: {}", e))?;

    // Recursively copy all files
    let mut count: u32 = 0;
    copy_dir_recursive(old_path, new_path)?;

    // Update all backup_path records in DB
    let conn = db.conn();
    let old_prefix = old_dir.trim_end_matches('/');
    let new_prefix = new_dir.trim_end_matches('/');

    let mut stmt = conn.prepare("SELECT id, backup_path FROM backups")
        .map_err(|e| format!("DB error: {}", e))?;
    let rows: Vec<(i64, String)> = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    for (id, path) in &rows {
        if let Some(suffix) = path.strip_prefix(old_prefix) {
            let new_bp = format!("{}{}", new_prefix, suffix);
            conn.execute("UPDATE backups SET backup_path = ?1 WHERE id = ?2", params![new_bp, id])
                .map_err(|e| format!("DB error: {}", e))?;
            count += 1;
        }
    }

    // Remove old directory
    fs::remove_dir_all(old_path).ok();

    Ok(count)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("mkdir error: {}", e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("read_dir error: {}", e))? {
        let entry = entry.map_err(|e| format!("entry error: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("copy error {}: {}", src_path.display(), e))?;
        }
    }
    Ok(())
}

/// Directory component for a project inside the backup tree.
///
/// Claude's `encoded_path` is already a flat `-Users-…` token. Codex stores the
/// raw cwd, and `PathBuf::join` with an absolute path *replaces* the base — so
/// joining it unencoded would drop the backup into the user's own project
/// directory instead of the backup directory. Encode anything absolute.
fn project_dir_key(encoded_path: &str) -> String {
    if encoded_path.starts_with('/') {
        crate::scanner::encode_path(encoded_path)
    } else {
        encoded_path.to_string()
    }
}

/// Read a backup file, decompressing when it was stored compressed.
fn read_backup_bytes(path: &Path, compressed: bool) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("Read error: {}", e))?
        .read_to_end(&mut data)
        .map_err(|e| format!("Read error: {}", e))?;
    if compressed {
        zstd::decode_all(data.as_slice()).map_err(|e| format!("Decompression error: {}", e))
    } else {
        Ok(data)
    }
}

/// Restore a backup over an explicit destination file.
///
/// Used for providers whose on-disk layout can't be reconstructed from the
/// backup directory structure (Codex rollouts live under date-based paths).
pub fn restore_backup_to(backup_path: &str, compressed: bool, target_path: &str) -> Result<(), String> {
    let path = Path::new(backup_path);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }
    let target = Path::new(target_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create target dir: {}", e))?;
    }
    let data = read_backup_bytes(path, compressed)?;
    fs::write(target, data).map_err(|e| format!("Write error: {}", e))
}

/// Restore a Claude backup back into `~/.claude/projects/`, reconstructing the
/// target from the backup directory structure.
pub fn restore_backup(backup_path: &str, compressed: bool) -> Result<(), String> {
    let path = Path::new(backup_path);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }

    // Determine target path from backup directory structure
    // backup_dir / encoded_project / session_id / timestamp.jsonl[.zst]
    let session_id = path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("Invalid backup path structure")?;
    let encoded_project = path.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("Invalid backup path structure")?;

    let claude_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let target_dir = claude_dir.join("projects").join(&encoded_project);
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create target dir: {}", e))?;

    let target_path = target_dir.join(format!("{}.jsonl", session_id));

    let data = read_backup_bytes(path, compressed)?;
    fs::write(&target_path, data).map_err(|e| format!("Write error: {}", e))?;

    // Restore subagents if present
    let backup_subagent_dir = path.parent().unwrap().join("subagents");
    if backup_subagent_dir.exists() {
        let target_subagent_dir = target_dir.join(&session_id).join("subagents");
        fs::create_dir_all(&target_subagent_dir).ok();
        for entry in fs::read_dir(&backup_subagent_dir).map_err(|e| format!("Read error: {}", e))? {
            let entry = entry.map_err(|e| format!("Read error: {}", e))?;
            let src = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.ends_with(".meta.json") {
                fs::copy(&src, target_subagent_dir.join(&filename)).ok();
            } else if filename.ends_with(".jsonl.zst") {
                let mut d = Vec::new();
                fs::File::open(&src).ok().map(|mut f| f.read_to_end(&mut d));
                if let Ok(decompressed) = zstd::decode_all(d.as_slice()) {
                    let target_name = filename.trim_end_matches(".zst");
                    fs::write(target_subagent_dir.join(target_name), decompressed).ok();
                }
            } else if filename.ends_with(".jsonl") {
                fs::copy(&src, target_subagent_dir.join(&filename)).ok();
            }
        }
    }

    Ok(())
}
