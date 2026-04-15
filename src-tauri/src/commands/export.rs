use crate::db::Database;
use crate::scanner::encode_path;
use rusqlite::params;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use zip::write::{FileOptions, SimpleFileOptions};
use zip::ZipWriter;

/// Export a Claude session to a zip file.
/// `project_path` is the target project path (e.g., "/Users/bob/myproject") — it gets
/// encoded into the Claude format and used as the directory inside the zip.
/// Unzipping into `~/.claude/projects/` on the target machine restores the session.
#[tauri::command]
pub fn export_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    project_path: String,
    target_path: String,
) -> Result<(), String> {
    let conn = db.conn();

    // Get main session JSONL path
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    // Get subagent JSONL paths
    let mut stmt = conn.prepare(
        "SELECT jsonl_path FROM subagents WHERE session_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;

    let subagent_paths: Vec<String> = stmt.query_map(params![session_id], |row| {
        row.get(0)
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let encoded_dir = encode_path(&project_path);

    // Create zip
    let zip_file = File::create(&target_path)
        .map_err(|e| format!("Failed to create zip file: {}", e))?;
    let mut zip = ZipWriter::new(zip_file);
    let options: SimpleFileOptions = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut seen_entries = std::collections::HashSet::new();

    // Add main session JSONL: {encoded_dir}/{filename}
    let main_filename = Path::new(&jsonl_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "session.jsonl".to_string());
    let zip_entry = format!("{}/{}", encoded_dir, main_filename);
    seen_entries.insert(zip_entry.clone());
    add_file_to_zip(&mut zip, &jsonl_path, &zip_entry, options)?;

    // Add subagent JSONLs
    for sa_path in &subagent_paths {
        let rel = relative_to_parent(&jsonl_path, sa_path);
        let mut zip_entry = format!("{}/{}", encoded_dir, rel);
        // Deduplicate: append _N if entry already exists
        if seen_entries.contains(&zip_entry) {
            let stem = zip_entry.trim_end_matches(".jsonl");
            let mut n = 1;
            loop {
                let candidate = format!("{}_{}.jsonl", stem, n);
                if !seen_entries.contains(&candidate) {
                    zip_entry = candidate;
                    break;
                }
                n += 1;
            }
        }
        seen_entries.insert(zip_entry.clone());
        add_file_to_zip(&mut zip, sa_path, &zip_entry, options)?;
    }

    zip.finish().map_err(|e| format!("Failed to finalize zip: {}", e))?;
    Ok(())
}

/// Export a Codex session to a zip file.
/// Codex sessions use date-based paths, so we preserve the relative path from ~/.codex/.
#[tauri::command]
pub fn export_codex_session(
    thread_id: String,
    target_path: String,
) -> Result<(), String> {
    let rollout_path = crate::codex::db::get_thread_rollout_path(&thread_id)?;

    let subagents = crate::codex::db::get_subagents(&thread_id)?;
    let mut subagent_rollout_paths: Vec<String> = Vec::new();
    for sa in &subagents {
        if let Ok(sa_path) = crate::codex::db::get_thread_rollout_path(&sa.id) {
            subagent_rollout_paths.push(sa_path);
        }
    }

    let zip_file = File::create(&target_path)
        .map_err(|e| format!("Failed to create zip file: {}", e))?;
    let mut zip = ZipWriter::new(zip_file);
    let options: SimpleFileOptions = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Use path relative to ~ for Codex
    add_file_to_zip(&mut zip, &rollout_path, &relative_to_home(&rollout_path), options)?;

    for sa_path in &subagent_rollout_paths {
        add_file_to_zip(&mut zip, sa_path, &relative_to_home(sa_path), options)?;
    }

    zip.finish().map_err(|e| format!("Failed to finalize zip: {}", e))?;
    Ok(())
}

/// Get path relative to the parent directory of `base_path`.
/// e.g., base="/a/b/session.jsonl", target="/a/b/.subagents/x/agent.jsonl"
/// → ".subagents/x/agent.jsonl"
fn relative_to_parent(base_path: &str, target_path: &str) -> String {
    let base_dir = Path::new(base_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if !base_dir.is_empty() && target_path.starts_with(&base_dir) {
        target_path[base_dir.len()..].trim_start_matches('/').to_string()
    } else {
        Path::new(target_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| target_path.to_string())
    }
}

fn relative_to_home(abs_path: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    if !home.is_empty() && abs_path.starts_with(&home) {
        abs_path[home.len()..].trim_start_matches('/').to_string()
    } else {
        Path::new(abs_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_path.to_string())
    }
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    source_path: &str,
    zip_entry_name: &str,
    options: SimpleFileOptions,
) -> Result<(), String> {
    let path = Path::new(source_path);
    if !path.exists() {
        return Ok(());
    }

    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open {}: {}", source_path, e))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read {}: {}", source_path, e))?;

    zip.start_file(zip_entry_name, options)
        .map_err(|e| format!("Failed to add {} to zip: {}", zip_entry_name, e))?;
    zip.write_all(&buf)
        .map_err(|e| format!("Failed to write {} to zip: {}", zip_entry_name, e))?;

    Ok(())
}
