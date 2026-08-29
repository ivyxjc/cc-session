//! Index Codex threads into the shared `projects` / `sessions` / `subagents` tables.
//!
//! Metadata comes from Codex's own SQLite store; message counts, token usage and
//! the heuristic summary come from the rollout JSONL, exactly like the Claude scan.

use super::db::{self, ThreadRow};
use crate::search;
use crate::sources::{self, Provider};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Default)]
pub struct CodexScanStats {
    pub projects_found: usize,
    pub sessions_found: usize,
    pub sessions_updated: usize,
}

/// Scan every Codex thread. Rollout paths that were seen are appended to
/// `seen_paths` so the caller can remove orphaned rows in one pass.
pub fn scan(conn: &Connection, seen_paths: &mut HashSet<String>) -> Result<CodexScanStats, String> {
    let mut stats = CodexScanStats::default();
    let Some(codex) = db::open_codex_db()? else { return Ok(stats) };

    let threads = db::list_threads(&codex)?;
    let edges = db::spawn_edges(&codex)?;
    let children: HashSet<&str> = edges.values().flatten().map(String::as_str).collect();
    let by_id: HashMap<&str, &ThreadRow> = threads.iter().map(|t| (t.id.as_str(), t)).collect();

    let mut project_ids: HashMap<&str, i64> = HashMap::new();
    let now_ms = chrono::Utc::now().timestamp_millis();

    for thread in threads.iter().filter(|t| !children.contains(t.id.as_str())) {
        let path = Path::new(&thread.rollout_path);
        let Ok(metadata) = path.metadata() else { continue }; // rollout gone — orphan cleanup drops it
        seen_paths.insert(thread.rollout_path.clone());

        let project_id = match project_ids.get(thread.cwd.as_str()) {
            Some(id) => *id,
            None => {
                let id = upsert_project(conn, &thread.cwd, now_ms)?;
                project_ids.insert(thread.cwd.as_str(), id);
                stats.projects_found += 1;
                id
            }
        };

        let file_size = metadata.len() as i64;
        let file_mtime = metadata.modified().ok()
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
            .unwrap_or(0);

        let existing: Option<(i64, i64, i64)> = conn.query_row(
            "SELECT file_size, file_mtime, COALESCE(content_indexed_at, 0) FROM sessions WHERE jsonl_path = ?1",
            params![thread.rollout_path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok();
        let needs_parse = existing.is_none_or(|(size, mtime, _)| size != file_size || mtime != file_mtime);
        let needs_index = existing.is_none_or(|(_, _, indexed_at)| indexed_at < file_mtime);

        if needs_parse {
            let Ok(parsed) = sources::parse_session_metadata(Provider::Codex, path) else { continue };
            let started_at = parsed.started_at.as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(thread.created_at_ms);
            let last_active = parsed.last_active.as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(thread.updated_at_ms);
            // Codex keeps its own title; fall back to the first-prompt heuristic.
            let summary = Some(thread.title.trim())
                .filter(|t| !t.is_empty())
                .map(String::from)
                .or(parsed.summary);

            conn.execute(
                "INSERT INTO sessions (provider, session_id, project_id, jsonl_path, version,
                    permission_mode, git_branch, started_at, last_active,
                    message_count, user_msg_count, assistant_msg_count,
                    total_input_tokens, total_output_tokens,
                    total_cache_creation_tokens, total_cache_read_tokens,
                    file_size, file_mtime, is_hidden,
                    summary, summary_source, summary_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
                 ON CONFLICT(session_id) DO UPDATE SET
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
                    file_mtime = excluded.file_mtime,
                    summary = CASE WHEN sessions.summary_source = 'llm'
                                   THEN sessions.summary ELSE excluded.summary END,
                    summary_source = CASE WHEN sessions.summary_source = 'llm'
                                          THEN 'llm' ELSE excluded.summary_source END,
                    summary_at = CASE WHEN sessions.summary_source = 'llm'
                                      THEN sessions.summary_at ELSE excluded.summary_at END",
                params![
                    Provider::Codex, thread.id, project_id, thread.rollout_path,
                    thread.cli_version, thread.approval_mode,
                    thread.git_branch.clone().or(parsed.git_branch),
                    started_at, last_active,
                    parsed.message_count, parsed.user_msg_count, parsed.assistant_msg_count,
                    parsed.total_input_tokens, parsed.total_output_tokens,
                    parsed.total_cache_creation_tokens, parsed.total_cache_read_tokens,
                    file_size, file_mtime,
                    thread.archived, // only applied on first insert; the user owns is_hidden afterwards
                    summary,
                    summary.as_ref().map(|_| "heuristic"),
                    summary.as_ref().map(|_| now_ms),
                    now_ms,
                ],
            ).map_err(|e| format!("DB error: {}", e))?;
            stats.sessions_updated += 1;

            if let Ok(daily) = sources::extract_daily_tokens(Provider::Codex, path) {
                for (date, tokens) in &daily {
                    conn.execute(
                        "INSERT INTO daily_token_usage (date, session_id, input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, user_msg_count)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                         ON CONFLICT(date, session_id) DO UPDATE SET
                            input_tokens = excluded.input_tokens,
                            output_tokens = excluded.output_tokens,
                            cache_creation_tokens = excluded.cache_creation_tokens,
                            cache_read_tokens = excluded.cache_read_tokens,
                            user_msg_count = excluded.user_msg_count",
                        params![date, thread.id, tokens.input_tokens, tokens.output_tokens, tokens.cache_creation_tokens, tokens.cache_read_tokens, tokens.user_msg_count],
                    ).ok();
                }
            }

            if let Some(child_ids) = edges.get(&thread.id) {
                sync_subagents(conn, &thread.id, child_ids, &by_id, now_ms)?;
            }
        }

        if needs_index {
            if let Ok(session_db_id) = conn.query_row(
                "SELECT id FROM sessions WHERE jsonl_path = ?1",
                params![thread.rollout_path],
                |row| row.get::<_, i64>(0),
            ) {
                if search::index_session_content(conn, session_db_id, Provider::Codex, path).is_ok() {
                    let _ = conn.execute(
                        "UPDATE sessions SET content_indexed_at = ?1 WHERE id = ?2",
                        params![file_mtime, session_db_id],
                    );
                }
            }
        }

        stats.sessions_found += 1;
    }

    // Project stats derive from the sessions actually indexed.
    conn.execute(
        "UPDATE projects SET
            session_count = (SELECT COUNT(*) FROM sessions s WHERE s.project_id = projects.id),
            last_active   = (SELECT MAX(last_active) FROM sessions s WHERE s.project_id = projects.id)
         WHERE provider = ?1",
        params![Provider::Codex],
    ).map_err(|e| format!("DB error: {}", e))?;

    Ok(stats)
}

/// Codex has no encoded project directory; the cwd itself is the stable key.
fn upsert_project(conn: &Connection, cwd: &str, now_ms: i64) -> Result<i64, String> {
    let display_name = cwd.rsplit('/').next().unwrap_or(cwd).to_string();
    conn.execute(
        "INSERT INTO projects (provider, encoded_path, original_path, display_name, created_at)
         VALUES (?1, ?2, ?2, ?3, ?4)
         ON CONFLICT(encoded_path) DO UPDATE SET display_name = excluded.display_name",
        params![Provider::Codex, cwd, display_name, now_ms],
    ).map_err(|e| format!("DB error: {}", e))?;
    conn.query_row(
        "SELECT id FROM projects WHERE encoded_path = ?1",
        params![cwd],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))
}

fn sync_subagents(
    conn: &Connection,
    parent_id: &str,
    child_ids: &[String],
    by_id: &HashMap<&str, &ThreadRow>,
    now_ms: i64,
) -> Result<(), String> {
    let db_session_id: i64 = conn.query_row(
        "SELECT id FROM sessions WHERE session_id = ?1",
        params![parent_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    conn.execute("DELETE FROM subagents WHERE session_id = ?1", params![db_session_id])
        .map_err(|e| format!("DB error: {}", e))?;

    for child in child_ids.iter().filter_map(|id| by_id.get(id.as_str())) {
        let description = match (&child.agent_nickname, child.title.trim()) {
            (Some(nick), "") => nick.clone(),
            (Some(nick), title) => format!("{} — {}", nick, title),
            (None, title) => title.to_string(),
        };
        conn.execute(
            "INSERT INTO subagents (session_id, agent_id, agent_type, description, jsonl_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                db_session_id, child.id,
                child.agent_role.clone().unwrap_or_else(|| "codex".to_string()),
                description, child.rollout_path, now_ms,
            ],
        ).map_err(|e| format!("DB error: {}", e))?;
    }
    Ok(())
}
