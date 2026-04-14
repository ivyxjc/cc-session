use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A Codex "project" — threads grouped by cwd.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProject {
    pub cwd: String,
    pub display_name: String,
    pub session_count: i64,
    pub last_active: i64,
    pub total_tokens: i64,
}

/// A Codex session summary for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSession {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub model: Option<String>,
    pub tokens_used: i64,
    pub git_branch: Option<String>,
    pub cli_version: String,
    pub approval_mode: String,
    pub source: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub first_user_message: String,
    pub subagent_count: i64,
}

/// A Codex subagent (child thread spawned from a parent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexSubagent {
    pub id: String,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub title: String,
    pub tokens_used: i64,
}

fn codex_db_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let path = home.join(".codex").join("state_5.sqlite");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn open_codex_db() -> Result<Connection, String> {
    let path = codex_db_path()
        .ok_or_else(|| "Codex database not found at ~/.codex/state_5.sqlite".to_string())?;
    Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Failed to open Codex DB: {}", e))
}

/// List all Codex projects (grouped by cwd), excluding subagent threads.
pub fn list_projects(sort_by: Option<&str>) -> Result<Vec<CodexProject>, String> {
    let conn = open_codex_db()?;

    let mut stmt = conn.prepare(
        "SELECT cwd, COUNT(*) as cnt, MAX(updated_at) as last_active, SUM(tokens_used) as total_tokens
         FROM threads
         WHERE source IN ('cli', 'vscode', 'exec')
         GROUP BY cwd
         ORDER BY last_active DESC"
    ).map_err(|e| format!("Query error: {}", e))?;

    let mut projects: Vec<CodexProject> = stmt.query_map([], |row| {
        let cwd: String = row.get(0)?;
        let display_name = cwd.rsplit('/').next().unwrap_or(&cwd).to_string();
        Ok(CodexProject {
            cwd,
            display_name,
            session_count: row.get(1)?,
            last_active: row.get(2)?,
            total_tokens: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        })
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    match sort_by {
        Some("name") => projects.sort_by(|a, b| a.display_name.cmp(&b.display_name)),
        Some("sessions") => projects.sort_by(|a, b| b.session_count.cmp(&a.session_count)),
        Some("tokens") => projects.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens)),
        _ => {} // already sorted by last_active DESC
    }

    Ok(projects)
}

/// List Codex sessions, optionally filtered by cwd.
pub fn list_sessions(
    cwd: Option<&str>,
    sort_by: Option<&str>,
    show_archived: Option<bool>,
) -> Result<Vec<CodexSession>, String> {
    let conn = open_codex_db()?;

    // Count subagents per parent thread
    let mut subagent_counts: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT parent_thread_id, COUNT(*) FROM thread_spawn_edges GROUP BY parent_thread_id"
        ).map_err(|e| format!("Query error: {}", e))?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }).map_err(|e| format!("Query error: {}", e))?;
        for r in rows.flatten() {
            subagent_counts.insert(r.0, r.1);
        }
    }

    let mut conditions = vec!["source IN ('cli', 'vscode', 'exec')".to_string()];
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(c) = cwd {
        conditions.push(format!("cwd = ?{}", param_values.len() + 1));
        param_values.push(Box::new(c.to_string()));
    }
    if !show_archived.unwrap_or(false) {
        conditions.push("archived = 0".to_string());
    }

    let where_clause = format!("WHERE {}", conditions.join(" AND "));
    let order = match sort_by {
        Some("tokens") => "tokens_used DESC",
        Some("name") => "title ASC",
        _ => "updated_at DESC",
    };

    let query = format!(
        "SELECT id, rollout_path, cwd, title, model, model_provider, tokens_used,
                git_branch, cli_version, approval_mode, source, archived,
                created_at, updated_at, first_user_message
         FROM threads {} ORDER BY {}",
        where_clause, order
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&query).map_err(|e| format!("Query error: {}", e))?;

    let sessions: Vec<CodexSession> = stmt.query_map(params_ref.as_slice(), |row| {
        let id: String = row.get(0)?;
        let subagent_count = subagent_counts.get(&id).copied().unwrap_or(0);
        Ok(CodexSession {
            id,
            title: row.get(3)?,
            cwd: row.get(2)?,
            model: row.get(4)?,
            tokens_used: row.get(6)?,
            git_branch: row.get(7)?,
            cli_version: row.get(8)?,
            approval_mode: row.get(9)?,
            source: row.get(10)?,
            archived: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            first_user_message: row.get(14)?,
            subagent_count,
        })
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(sessions)
}

/// Get the rollout_path (JSONL file path) for a thread.
pub fn get_thread_rollout_path(thread_id: &str) -> Result<String, String> {
    let conn = open_codex_db()?;
    conn.query_row(
        "SELECT rollout_path FROM threads WHERE id = ?1",
        params![thread_id],
        |row| row.get(0),
    ).map_err(|e| format!("Thread not found: {}", e))
}

/// List subagents (child threads) for a parent thread.
pub fn get_subagents(parent_thread_id: &str) -> Result<Vec<CodexSubagent>, String> {
    let conn = open_codex_db()?;
    let mut stmt = conn.prepare(
        "SELECT t.id, t.agent_nickname, t.agent_role, t.title, t.tokens_used
         FROM thread_spawn_edges e
         JOIN threads t ON e.child_thread_id = t.id
         WHERE e.parent_thread_id = ?1"
    ).map_err(|e| format!("Query error: {}", e))?;

    let subagents = stmt.query_map(params![parent_thread_id], |row| {
        Ok(CodexSubagent {
            id: row.get(0)?,
            nickname: row.get(1)?,
            role: row.get(2)?,
            title: row.get(3)?,
            tokens_used: row.get(4)?,
        })
    })
    .map_err(|e| format!("Query error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(subagents)
}
