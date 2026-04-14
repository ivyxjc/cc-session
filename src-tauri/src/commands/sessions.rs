use crate::db::Database;
use crate::db::models::{SessionSummary, Tag, SubagentSummary};
use crate::parser;
use crate::claude::converter::to_view_message;
use crate::models::ViewMessage;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_sessions(
    db: State<'_, Arc<Database>>,
    project_id: Option<i64>,
    tag_id: Option<i64>,
    favorited: Option<bool>,
    show_hidden: Option<bool>,
    sort_by: Option<String>,
) -> Result<Vec<SessionSummary>, String> {
    let conn = db.conn();

    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(pid) = project_id {
        conditions.push(format!("s.project_id = ?{}", param_values.len() + 1));
        param_values.push(Box::new(pid));
    }
    if let Some(true) = favorited {
        conditions.push("s.is_favorited = 1".to_string());
    }
    if let Some(tid) = tag_id {
        conditions.push(format!("st.tag_id = ?{}", param_values.len() + 1));
        param_values.push(Box::new(tid));
    }
    // Hide hidden sessions unless explicitly requested
    if !show_hidden.unwrap_or(false) {
        conditions.push("s.is_hidden = 0".to_string());

        // Auto-hide small sessions if configured
        let auto_hide: Option<String> = conn.query_row(
            "SELECT value FROM app_config WHERE key = 'auto_hide_config'",
            [],
            |row| row.get(0),
        ).ok();
        if let Some(json) = auto_hide {
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&json) {
                if cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let min_msgs = cfg.get("minMessageCount").and_then(|v| v.as_i64()).unwrap_or(3);
                    conditions.push(format!("(s.user_msg_count >= {} OR s.is_favorited = 1)", min_msgs));
                }
            }
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let order = match sort_by.as_deref() {
        Some("size") => "s.is_favorited DESC, s.file_size DESC",
        Some("messages") => "s.is_favorited DESC, s.message_count DESC",
        Some("tokens") => "s.is_favorited DESC, (s.total_input_tokens + s.total_output_tokens + s.total_cache_creation_tokens + s.total_cache_read_tokens) DESC",
        _ => "s.is_favorited DESC, s.last_active DESC NULLS LAST",
    };

    let query = format!(
        "SELECT DISTINCT s.id, s.session_id, s.project_id, p.display_name,
                p.original_path,
                s.slug, s.version, s.permission_mode, s.git_branch,
                s.started_at, s.last_active, s.message_count,
                s.user_msg_count, s.assistant_msg_count,
                s.total_input_tokens, s.total_output_tokens,
                s.total_cache_creation_tokens, s.total_cache_read_tokens,
                s.file_size, s.is_favorited, s.is_hidden, s.is_backed_up,
                s.copied_from_session_id, s.copied_at
         FROM sessions s
         JOIN projects p ON s.project_id = p.id
         LEFT JOIN session_tags st ON s.id = st.session_id
         {} ORDER BY {}",
        where_clause, order
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&query).map_err(|e| format!("DB error: {}", e))?;

    let session_rows: Vec<(i64, String, i64, String, String, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<i64>, Option<i64>,
        i64, i64, i64, i64, i64, i64, i64, i64, bool, bool, bool,
        Option<String>, Option<i64>)> = stmt.query_map(
        params_ref.as_slice(),
        |row| {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
                row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?,
                row.get(16)?, row.get(17)?, row.get(18)?, row.get(19)?,
                row.get(20)?, row.get(21)?, row.get(22)?, row.get(23)?,
            ))
        },
    )
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut sessions = Vec::new();
    for row in session_rows {
        let tags = get_session_tags(&conn, row.0)?;
        sessions.push(SessionSummary {
            id: row.0,
            session_id: row.1,
            project_id: row.2,
            project_name: row.3,
            project_path: row.4,
            slug: row.5,
            version: row.6,
            permission_mode: row.7,
            git_branch: row.8,
            started_at: row.9,
            last_active: row.10,
            message_count: row.11,
            user_msg_count: row.12,
            assistant_msg_count: row.13,
            total_input_tokens: row.14,
            total_output_tokens: row.15,
            total_cache_creation_tokens: row.16,
            total_cache_read_tokens: row.17,
            file_size: row.18,
            is_favorited: row.19,
            is_hidden: row.20,
            is_backed_up: row.21,
            copied_from_session_id: row.22,
            copied_at: row.23,
            tags,
        });
    }

    Ok(sessions)
}

fn get_session_tags(conn: &rusqlite::Connection, session_id: i64) -> Result<Vec<Tag>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color FROM tags t
         JOIN session_tags st ON t.id = st.tag_id
         WHERE st.session_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;

    let tags = stmt.query_map(params![session_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(tags)
}

#[tauri::command]
pub fn get_messages(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let messages = parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )?;
    Ok(messages.into_iter().map(to_view_message).collect())
}

#[tauri::command]
pub fn get_latest_messages(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    count: Option<usize>,
) -> Result<parser::ViewLatestMessagesResult, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let result = parser::load_latest_messages(
        Path::new(&jsonl_path),
        count.unwrap_or(50),
    )?;
    Ok(parser::ViewLatestMessagesResult {
        messages: result.messages.into_iter().map(to_view_message).collect(),
        total_count: result.total_count,
    })
}

#[tauri::command]
pub fn get_subagents(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<Vec<SubagentSummary>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, session_id, agent_id, agent_type, description
         FROM subagents WHERE session_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;

    let subagents = stmt.query_map(params![session_id], |row| {
        Ok(SubagentSummary {
            id: row.get(0)?,
            session_id: row.get(1)?,
            agent_id: row.get(2)?,
            agent_type: row.get(3)?,
            description: row.get(4)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(subagents)
}

#[tauri::command]
pub fn get_subagent_messages(
    db: State<'_, Arc<Database>>,
    subagent_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM subagents WHERE id = ?1",
        params![subagent_id],
        |row| row.get(0),
    ).map_err(|e| format!("Subagent not found: {}", e))?;

    let messages = parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )?;
    Ok(messages.into_iter().map(to_view_message).collect())
}
