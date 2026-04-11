use crate::db::Database;
use crate::db::models::ScanResult;
use crate::parser;
use rusqlite::params;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;


fn get_claude_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude")
}

fn encode_path(path: &str) -> String {
    // Claude Code encodes paths by replacing '/' and '.' with '-'
    // "/Users/ivyxjc/simora.main" -> "-Users-ivyxjc-simora-main"
    path.replace('/', "-").replace('.', "-")
}

/// Extract cwd from the first user message in a JSONL file
fn extract_cwd_from_jsonl(jsonl_path: &Path) -> Option<String> {
    let file = File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.ok()?;
        let raw: serde_json::Value = serde_json::from_str(&line).ok()?;
        if raw.get("type").and_then(|v| v.as_str()) == Some("user") {
            return raw.get("cwd").and_then(|v| v.as_str()).map(String::from);
        }
    }
    None
}

/// Resolve the real path for an encoded project directory.
/// Strategy:
/// 1. Read cwd from the first JSONL in the project dir — if encode(cwd) == encoded, use cwd
/// 2. Fallback: greedy filesystem matching
fn resolve_project_path(encoded: &str, project_dir: &Path) -> String {
    // Strategy 1: try cwd from JSONL
    if let Some(jsonl) = first_jsonl_in_dir(project_dir) {
        if let Some(cwd) = extract_cwd_from_jsonl(&jsonl) {
            if encode_path(&cwd) == encoded {
                return cwd;
            }
        }
    }

    // Strategy 2: greedy filesystem decode
    decode_project_path_fs(encoded)
}

fn first_jsonl_in_dir(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map(|ext| ext == "jsonl").unwrap_or(false))
        .map(|e| e.path())
}

fn decode_project_path_fs(encoded: &str) -> String {
    decode_with_checker(encoded, |p| std::path::Path::new(p).exists())
}

/// Decode an encoded project path using a checker function to validate path segments.
/// The checker takes a candidate path string and returns true if it exists.
fn decode_with_checker<F: Fn(&str) -> bool>(encoded: &str, exists: F) -> String {
    let chars: Vec<char> = encoded.chars().collect();
    if chars.is_empty() || chars[0] != '-' {
        return encoded.to_string();
    }

    let rest = &encoded[1..];
    let mut result = String::from("/");
    let segments: Vec<&str> = rest.split('-').collect();

    let mut i = 0;
    while i < segments.len() {
        let mut found = false;
        for end in (i + 1..=segments.len()).rev() {
            let candidate = segments[i..end].join("-");
            let test_path = format!("{}{}", result, candidate);
            if exists(&test_path) {
                result = format!("{}{}/", result, candidate);
                i = end;
                found = true;
                break;
            }
        }
        if !found {
            result = format!("{}{}/", result, segments[i]);
            i += 1;
        }
    }

    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
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
        let project_dir = entry.path();
        let original_path = resolve_project_path(&encoded_path, &project_dir);
        let display_name = display_name_from_path(&original_path);

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
                        total_cache_creation_tokens, total_cache_read_tokens,
                        file_size, file_mtime, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
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
                        total_cache_creation_tokens = excluded.total_cache_creation_tokens,
                        total_cache_read_tokens = excluded.total_cache_read_tokens,
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
                        parse_result.total_cache_creation_tokens, parse_result.total_cache_read_tokens,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Helper: build an exists checker from a set of known paths
    fn mock_fs(paths: &[&str]) -> impl Fn(&str) -> bool {
        let set: HashSet<String> = paths.iter().map(|s| s.to_string()).collect();
        move |p: &str| set.contains(p)
    }

    // ===== encode_path =====

    #[test]
    fn encode_simple_path() {
        assert_eq!(encode_path("/Users/alice/projects/web-app"), "-Users-alice-projects-web-app");
    }

    #[test]
    fn encode_path_with_dots() {
        // Directory names containing '.' should have dots replaced with '-'
        assert_eq!(encode_path("/Users/alice/app.v2/backend"), "-Users-alice-app-v2-backend");
    }

    #[test]
    fn encode_path_with_hyphen_and_dot() {
        assert_eq!(
            encode_path("/Users/alice/my-project/config.prod/api"),
            "-Users-alice-my-project-config-prod-api"
        );
    }

    // ===== decode_with_checker (greedy filesystem matching) =====

    #[test]
    fn decode_simple_no_ambiguity() {
        // All segments are single directories, no hyphens in names
        let exists = mock_fs(&["/Users", "/Users/alice", "/Users/alice/projects", "/Users/alice/projects/api"]);
        assert_eq!(
            decode_with_checker("-Users-alice-projects-api", &exists),
            "/Users/alice/projects/api"
        );
    }

    #[test]
    fn decode_hyphenated_directory_name() {
        // "web-app" is a single directory with a hyphen
        let exists = mock_fs(&[
            "/Users", "/Users/alice", "/Users/alice/projects",
            "/Users/alice/projects/web-app",
        ]);
        assert_eq!(
            decode_with_checker("-Users-alice-projects-web-app", &exists),
            "/Users/alice/projects/web-app"
        );
    }

    #[test]
    fn decode_multiple_hyphenated_segments() {
        // Both "my-org" and "web-app" contain hyphens
        let exists = mock_fs(&[
            "/Users", "/Users/alice", "/Users/alice/my-org",
            "/Users/alice/my-org/web-app",
        ]);
        assert_eq!(
            decode_with_checker("-Users-alice-my-org-web-app", &exists),
            "/Users/alice/my-org/web-app"
        );
    }

    #[test]
    fn decode_deeply_nested_with_hyphens() {
        // Deep path: /Users/alice/code/rust-web-demo2/backend
        let exists = mock_fs(&[
            "/Users", "/Users/alice", "/Users/alice/code",
            "/Users/alice/code/rust-web-demo2",
            "/Users/alice/code/rust-web-demo2/backend",
        ]);
        assert_eq!(
            decode_with_checker("-Users-alice-code-rust-web-demo2-backend", &exists),
            "/Users/alice/code/rust-web-demo2/backend"
        );
    }

    #[test]
    fn decode_fallback_when_path_deleted() {
        // No paths exist on filesystem — falls back to treating each segment as a directory
        let exists = mock_fs(&[]);
        assert_eq!(
            decode_with_checker("-Users-alice-projects-api", &exists),
            "/Users/alice/projects/api"
        );
    }

    #[test]
    fn decode_fallback_hyphen_splits_incorrectly_when_deleted() {
        // "web-app" directory no longer exists, so greedy can't find it
        // Falls back to splitting each segment individually
        let exists = mock_fs(&["/Users", "/Users/alice"]);
        assert_eq!(
            decode_with_checker("-Users-alice-web-app", &exists),
            "/Users/alice/web/app"
        );
        // This is expected incorrect behavior when the directory is deleted
        // — that's why we prefer the cwd strategy first
    }

    // ===== encode/decode roundtrip via cwd strategy =====

    #[test]
    fn cwd_strategy_with_dot_in_dirname() {
        // Real path has a dot: /Users/alice/app.v2/backend/api
        let cwd = "/Users/alice/app.v2/backend/api";
        let encoded = encode_path(cwd);
        assert_eq!(encoded, "-Users-alice-app-v2-backend-api");
        // cwd verification: encode(cwd) == encoded → use cwd directly
        assert_eq!(encode_path(cwd), encoded);
    }

    #[test]
    fn cwd_strategy_with_multiple_dots() {
        let cwd = "/Users/alice/org.example.app/src/main";
        let encoded = encode_path(cwd);
        assert_eq!(encoded, "-Users-alice-org-example-app-src-main");
        assert_eq!(encode_path(cwd), encoded);
    }

    #[test]
    fn cwd_strategy_with_hyphen_and_dot() {
        // Path has both hyphens and dots: auth-service.v2
        let cwd = "/Users/alice/auth-service.v2/backend";
        let encoded = encode_path(cwd);
        assert_eq!(encoded, "-Users-alice-auth-service-v2-backend");
        assert_eq!(encode_path(cwd), encoded);
    }

    // ===== display_name_from_path =====

    #[test]
    fn display_name_simple() {
        assert_eq!(display_name_from_path("/Users/alice/my-project"), "my-project");
    }

    #[test]
    fn display_name_with_dot() {
        assert_eq!(display_name_from_path("/Users/alice/app.v2"), "app.v2");
    }

    // ===== edge cases =====

    #[test]
    fn decode_empty_string() {
        let exists = mock_fs(&[]);
        assert_eq!(decode_with_checker("", &exists), "");
    }

    #[test]
    fn decode_root_only() {
        let exists = mock_fs(&[]);
        assert_eq!(decode_with_checker("-", &exists), "/");
    }

    #[test]
    fn encode_root() {
        assert_eq!(encode_path("/"), "-");
    }
}
