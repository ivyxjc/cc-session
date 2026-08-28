//! Read-only access to Codex's own metadata store (`~/.codex/state_5.sqlite`).
//! Only the scanner talks to it; everything else reads the app index.

use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::PathBuf;

/// One row of Codex's `threads` table, limited to what the index needs.
#[derive(Debug, Clone)]
pub struct ThreadRow {
    pub id: String,
    pub rollout_path: String,
    pub cwd: String,
    pub title: String,
    pub git_branch: Option<String>,
    pub cli_version: String,
    pub approval_mode: String,
    pub archived: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
}

fn codex_db_path() -> Option<PathBuf> {
    let path = dirs::home_dir()?.join(".codex").join("state_5.sqlite");
    path.exists().then_some(path)
}

/// `None` when Codex is not installed / has no state DB.
pub fn open_codex_db() -> Result<Option<Connection>, String> {
    let Some(path) = codex_db_path() else { return Ok(None) };
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Failed to open Codex DB: {}", e))?;
    // Codex writes to this DB live; without a busy timeout reads fail hard
    // with SQLITE_BUSY whenever they collide with a write or WAL checkpoint.
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| format!("Failed to set busy timeout: {}", e))?;
    Ok(Some(conn))
}

/// All interactive threads (CLI / VS Code / exec), including spawned subagents.
pub fn list_threads(conn: &Connection) -> Result<Vec<ThreadRow>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, rollout_path, cwd, title, git_branch, cli_version, approval_mode, archived,
                COALESCE(created_at_ms, created_at * 1000), COALESCE(updated_at_ms, updated_at * 1000),
                agent_nickname, agent_role
         FROM threads WHERE source IN ('cli', 'vscode', 'exec')"
    ).map_err(|e| format!("Query error: {}", e))?;
    let rows = stmt.query_map([], |row| Ok(ThreadRow {
        id: row.get(0)?,
        rollout_path: row.get(1)?,
        cwd: row.get(2)?,
        title: row.get(3)?,
        git_branch: row.get(4)?,
        cli_version: row.get(5)?,
        approval_mode: row.get(6)?,
        archived: row.get(7)?,
        created_at_ms: row.get(8)?,
        updated_at_ms: row.get(9)?,
        agent_nickname: row.get(10)?,
        agent_role: row.get(11)?,
    })).map_err(|e| format!("Query error: {}", e))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// parent thread id → child thread ids.
pub fn spawn_edges(conn: &Connection) -> Result<HashMap<String, Vec<String>>, String> {
    let mut stmt = conn.prepare("SELECT parent_thread_id, child_thread_id FROM thread_spawn_edges")
        .map_err(|e| format!("Query error: {}", e))?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| format!("Query error: {}", e))?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (parent, child) in rows.flatten() {
        map.entry(parent).or_default().push(child);
    }
    Ok(map)
}
