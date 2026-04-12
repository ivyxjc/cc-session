use crate::db::Database;
use crate::scanner;
use rusqlite::params;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

fn encode_path(path: &str) -> String {
    path.replace('/', "-").replace('.', "-")
}

#[tauri::command]
pub fn copy_session_to_path(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    target_path: String,
) -> Result<String, String> {
    let conn = db.conn();

    // 1. Look up source session
    let (source_uuid, source_jsonl): (String, String) = conn
        .query_row(
            "SELECT session_id, jsonl_path FROM sessions WHERE id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Session not found: {}", e))?;

    let source_path = PathBuf::from(&source_jsonl);
    if !source_path.exists() {
        return Err(format!("Source JSONL not found: {}", source_jsonl));
    }

    // 2. Generate new UUID
    let new_uuid = uuid::Uuid::new_v4().to_string();

    // 3. Compute target directory
    let encoded_target = encode_path(&target_path);
    let claude_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects")
        .join(&encoded_target);

    fs::create_dir_all(&claude_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // 4. Rewrite JSONL with new sessionId
    let target_jsonl = claude_dir.join(format!("{}.jsonl", new_uuid));
    let source_file = fs::File::open(&source_path)
        .map_err(|e| format!("Failed to open source: {}", e))?;
    let reader = BufReader::new(source_file);

    let mut target_file = fs::File::create(&target_jsonl)
        .map_err(|e| format!("Failed to create target: {}", e))?;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            writeln!(target_file).ok();
            continue;
        }

        // Parse, replace sessionId, write back
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(mut val) => {
                if let Some(obj) = val.as_object_mut() {
                    if obj.get("sessionId").and_then(|v| v.as_str()) == Some(&source_uuid) {
                        obj.insert(
                            "sessionId".to_string(),
                            serde_json::Value::String(new_uuid.clone()),
                        );
                    }
                    // Also update session_id if present (some message types use this)
                    if obj.get("session_id").and_then(|v| v.as_str()) == Some(&source_uuid) {
                        obj.insert(
                            "session_id".to_string(),
                            serde_json::Value::String(new_uuid.clone()),
                        );
                    }
                }
                let rewritten = serde_json::to_string(&val)
                    .map_err(|e| format!("Serialize error: {}", e))?;
                writeln!(target_file, "{}", rewritten)
                    .map_err(|e| format!("Write error: {}", e))?;
            }
            Err(_) => {
                // Can't parse — write as-is
                writeln!(target_file, "{}", line)
                    .map_err(|e| format!("Write error: {}", e))?;
            }
        }
    }

    // 5. Copy subagent directory if exists
    let source_subagent_dir = source_path
        .parent()
        .unwrap_or(source_path.as_ref())
        .join(&source_uuid)
        .join("subagents");

    if source_subagent_dir.exists() {
        let target_subagent_dir = claude_dir.join(&new_uuid).join("subagents");
        fs::create_dir_all(&target_subagent_dir)
            .map_err(|e| format!("Failed to create subagent dir: {}", e))?;

        for entry in fs::read_dir(&source_subagent_dir)
            .map_err(|e| format!("Read dir error: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
            let dest = target_subagent_dir.join(entry.file_name());
            fs::copy(entry.path(), dest)
                .map_err(|e| format!("Copy error: {}", e))?;
        }
    }

    // 6. Re-scan to pick up the new session
    drop(conn); // Release DB lock before scanning
    let _ = scanner::scan_all(&db);

    // 7. Set copy metadata on the new session
    let conn = db.conn();
    conn.execute(
        "UPDATE sessions SET copied_from_session_id = ?1, copied_at = ?2 WHERE session_id = ?3",
        params![source_uuid, chrono::Utc::now().timestamp_millis(), new_uuid],
    )
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(new_uuid)
}
