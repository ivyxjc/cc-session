use crate::db::Database;
use crate::db::models::ScanResult;
use crate::parser;
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;


fn get_claude_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude")
}

fn decode_project_path(encoded: &str) -> String {
    // "-Users-ivyxjc-myproject" -> "/Users/ivyxjc/myproject"
    encoded.replacen('-', "/", 1).replace('-', "/")
}

fn display_name_from_path(original_path: &str) -> String {
    original_path.rsplit('/').next().unwrap_or(original_path).to_string()
}

pub fn scan_all(db: &Arc<Database>) -> Result<ScanResult, String> {
    let start = Instant::now();
    let claude_dir = get_claude_dir();
    let projects_dir = claude_dir.join("projects");

    if !projects_dir.exists() {
        return Err(format!("Claude projects dir not found: {}", projects_dir.display()));
    }

    let mut projects_found: usize = 0;
    let mut sessions_found: usize = 0;
    let mut sessions_updated: usize = 0;
    let mut sessions_removed: usize = 0;

    let conn = db.conn();

    // Track which session jsonl_paths we see on disk
    let mut seen_paths: Vec<String> = Vec::new();

    // Iterate project directories
    let entries: Vec<_> = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();

    for entry in entries {
        let encoded_path = entry.file_name().to_string_lossy().to_string();
        let original_path = decode_project_path(&encoded_path);
        let display_name = display_name_from_path(&original_path);
        let project_dir = entry.path();

        // Upsert project
        conn.execute(
            "INSERT INTO projects (encoded_path, original_path, display_name, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(encoded_path) DO UPDATE SET
                original_path = excluded.original_path,
                display_name = excluded.display_name",
            params![encoded_path, original_path, display_name, chrono::Utc::now().timestamp_millis()],
        ).map_err(|e| format!("DB error: {}", e))?;

        let project_id: i64 = conn.query_row(
            "SELECT id FROM projects WHERE encoded_path = ?1",
            params![encoded_path],
            |row| row.get(0),
        ).map_err(|e| format!("DB error: {}", e))?;

        projects_found += 1;

        // Find .jsonl files in project directory (not recursive into subagent dirs)
        let jsonl_files: Vec<_> = std::fs::read_dir(&project_dir)
            .map_err(|e| format!("Read dir error: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().map(|ext| ext == "jsonl").unwrap_or(false)
            })
            .collect();

        let mut project_last_active: Option<i64> = None;
        let mut project_session_count: i64 = 0;

        for jsonl_entry in jsonl_files {
            let jsonl_path = jsonl_entry.path();
            let jsonl_path_str = jsonl_path.to_string_lossy().to_string();
            seen_paths.push(jsonl_path_str.clone());

            // Session ID = filename without .jsonl
            let session_id = jsonl_path.file_stem()
                .unwrap_or_default().to_string_lossy().to_string();

            let metadata = jsonl_entry.metadata().ok();
            let file_size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let file_mtime = metadata.as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
                .unwrap_or(0);

            // Check if we need to re-parse
            let existing: Option<(i64, i64)> = conn.query_row(
                "SELECT file_size, file_mtime FROM sessions WHERE jsonl_path = ?1",
                params![jsonl_path_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).ok();

            let needs_parse = match existing {
                Some((old_size, old_mtime)) => old_size != file_size || old_mtime != file_mtime,
                None => true,
            };

            if needs_parse {
                let parse_result = match parser::parse_session_metadata(&jsonl_path) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let started_at = parse_result.started_at.as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis());
                let last_active = parse_result.last_active.as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis());

                conn.execute(
                    "INSERT INTO sessions (session_id, project_id, jsonl_path, slug, version,
                        permission_mode, git_branch, started_at, last_active,
                        message_count, user_msg_count, assistant_msg_count,
                        total_input_tokens, total_output_tokens,
                        file_size, file_mtime, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                     ON CONFLICT(session_id) DO UPDATE SET
                        slug = excluded.slug,
                        version = excluded.version,
                        permission_mode = excluded.permission_mode,
                        git_branch = excluded.git_branch,
                        started_at = excluded.started_at,
                        last_active = excluded.last_active,
                        message_count = excluded.message_count,
                        user_msg_count = excluded.user_msg_count,
                        assistant_msg_count = excluded.assistant_msg_count,
                        total_input_tokens = excluded.total_input_tokens,
                        total_output_tokens = excluded.total_output_tokens,
                        file_size = excluded.file_size,
                        file_mtime = excluded.file_mtime",
                    params![
                        session_id, project_id, jsonl_path_str,
                        parse_result.slug, parse_result.version,
                        parse_result.permission_mode, parse_result.git_branch,
                        started_at, last_active,
                        parse_result.message_count, parse_result.user_msg_count,
                        parse_result.assistant_msg_count,
                        parse_result.total_input_tokens, parse_result.total_output_tokens,
                        file_size, file_mtime, chrono::Utc::now().timestamp_millis(),
                    ],
                ).map_err(|e| format!("DB error: {}", e))?;

                sessions_updated += 1;

                // Scan subagents
                let subagent_dir = project_dir.join(&session_id).join("subagents");
                if subagent_dir.exists() {
                    scan_subagents(&conn, &subagent_dir, &session_id)?;
                }

                if let Some(la) = last_active {
                    if project_last_active.map_or(true, |pla| la > pla) {
                        project_last_active = Some(la);
                    }
                }
            }

            sessions_found += 1;
            project_session_count += 1;
        }

        // Update project stats
        conn.execute(
            "UPDATE projects SET session_count = ?1, last_active = COALESCE(?2, last_active) WHERE id = ?3",
            params![project_session_count, project_last_active, project_id],
        ).map_err(|e| format!("DB error: {}", e))?;
    }

    // Remove sessions whose files no longer exist
    let mut stmt = conn.prepare("SELECT id, jsonl_path FROM sessions")
        .map_err(|e| format!("DB error: {}", e))?;
    let orphans: Vec<i64> = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((id, path))
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .filter(|(_, path)| !seen_paths.contains(path))
    .map(|(id, _)| id)
    .collect();

    for id in &orphans {
        conn.execute("DELETE FROM session_tags WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM favorites WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM subagents WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).ok();
        sessions_removed += 1;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(ScanResult {
        projects_found,
        sessions_found,
        sessions_updated,
        sessions_removed,
        duration_ms,
    })
}

fn scan_subagents(conn: &rusqlite::Connection, subagent_dir: &Path, session_id: &str) -> Result<(), String> {
    let db_session_id: i64 = conn.query_row(
        "SELECT id FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    for entry in std::fs::read_dir(subagent_dir).map_err(|e| format!("Read error: {}", e))? {
        let entry = entry.map_err(|e| format!("Read error: {}", e))?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false)
            && path.to_string_lossy().contains(".meta.json")
        {
            let meta_content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Read error: {}", e))?;
            let meta: serde_json::Value = serde_json::from_str(&meta_content)
                .map_err(|e| format!("Parse error: {}", e))?;

            let agent_id = path.file_stem()
                .unwrap_or_default().to_string_lossy()
                .replace(".meta", "");
            let agent_type = meta.get("agentType")
                .and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let description = meta.get("description")
                .and_then(|v| v.as_str()).unwrap_or("").to_string();

            let jsonl_path = subagent_dir.join(format!("{}.jsonl", agent_id));
            let jsonl_path_str = jsonl_path.to_string_lossy().to_string();

            conn.execute(
                "INSERT OR REPLACE INTO subagents (session_id, agent_id, agent_type, description, jsonl_path, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    db_session_id, agent_id, agent_type, description,
                    jsonl_path_str, chrono::Utc::now().timestamp_millis()
                ],
            ).map_err(|e| format!("DB error: {}", e))?;
        }
    }

    Ok(())
}
