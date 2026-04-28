use rusqlite::{params, Connection};
use serde::Serialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Index all text/thinking content of a session's JSONL into messages_fts.
/// Replaces any existing rows for this session_db_id.
pub fn index_session_content(
    conn: &Connection,
    session_db_id: i64,
    jsonl_path: &Path,
) -> Result<(), String> {
    // Drop stale rows for this session first
    conn.execute(
        "DELETE FROM messages_fts WHERE session_db_id = ?1",
        params![session_db_id],
    )
    .map_err(|e| format!("FTS delete error: {}", e))?;

    let file = match File::open(jsonl_path) {
        Ok(f) => f,
        Err(_) => return Ok(()), // Missing file isn't fatal — silently skip
    };
    let reader = BufReader::new(file);

    let mut stmt = conn
        .prepare(
            "INSERT INTO messages_fts (content, session_db_id, message_uuid, role, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .map_err(|e| format!("FTS prepare error: {}", e))?;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let raw: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "user" && msg_type != "assistant" {
            continue;
        }

        let uuid = raw.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp_ms = raw
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0);

        let content_arr = raw
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array());

        // String-form content (legacy user messages where content is a plain string)
        if content_arr.is_none() {
            if let Some(s) = raw
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
            {
                if !s.trim().is_empty() {
                    stmt.execute(params![s, session_db_id, uuid, msg_type, timestamp_ms])
                        .map_err(|e| format!("FTS insert error: {}", e))?;
                }
            }
            continue;
        }

        for block in content_arr.unwrap() {
            let bt = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let (text, role): (Option<&str>, &str) = match bt {
                "text" => (block.get("text").and_then(|v| v.as_str()), msg_type),
                "thinking" => (
                    block.get("thinking").and_then(|v| v.as_str()),
                    "thinking",
                ),
                _ => (None, ""),
            };
            if let Some(t) = text {
                if !t.trim().is_empty() {
                    stmt.execute(params![t, session_db_id, uuid, role, timestamp_ms])
                        .map_err(|e| format!("FTS insert error: {}", e))?;
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSearchResult {
    pub session_db_id: i64,
    pub session_id: String,
    pub project_name: String,
    pub project_path: String,
    pub message_uuid: String,
    pub role: String,
    pub timestamp_ms: i64,
    pub snippet: String,
}

/// Search message content using FTS5 + BM25 with linear time decay.
/// `query` is treated as a plain phrase — special FTS chars are escaped.
pub fn search_messages(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<ContentSearchResult>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    // Trigram tokenizer needs at least 3 chars to use the index
    if q.chars().count() < 3 {
        return search_messages_like(conn, q, limit);
    }

    // Escape and quote as a phrase so FTS5 doesn't interpret operators (AND/OR/NOT/quotes)
    let escaped = q.replace('"', "\"\"");
    let phrase = format!("\"{}\"", escaped);

    let now_ms = chrono::Utc::now().timestamp_millis();
    // Linear decay: every 30 days adds +1 to bm25 (worse rank, since smaller = better)
    let decay_per_ms: f64 = 1.0 / (30.0 * 86_400_000.0);

    let sql = "
        SELECT
            f.session_db_id,
            s.session_id,
            p.display_name,
            p.original_path,
            f.message_uuid,
            f.role,
            f.timestamp_ms,
            snippet(messages_fts, 0, char(1), char(2), '…', 24) AS snip,
            bm25(messages_fts) + (?2 - f.timestamp_ms) * ?3 AS weighted
        FROM messages_fts f
        JOIN sessions s ON s.id = f.session_db_id
        JOIN projects p ON p.id = s.project_id
        WHERE messages_fts MATCH ?1
        ORDER BY weighted ASC
        LIMIT ?4
    ";

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("FTS query prepare error: {}", e))?;

    let rows = stmt
        .query_map(
            params![phrase, now_ms, decay_per_ms, limit as i64],
            |row| {
                Ok(ContentSearchResult {
                    session_db_id: row.get(0)?,
                    session_id: row.get(1)?,
                    project_name: row.get(2)?,
                    project_path: row.get(3)?,
                    message_uuid: row.get(4)?,
                    role: row.get(5)?,
                    timestamp_ms: row.get(6)?,
                    snippet: row.get(7)?,
                })
            },
        )
        .map_err(|e| format!("FTS query error: {}", e))?;

    let mut results = Vec::new();
    for r in rows {
        if let Ok(r) = r {
            results.push(r);
        }
    }
    Ok(results)
}

/// Fallback for short queries (<3 chars) where trigram tokenizer can't index.
/// Linear scan via LIKE — slower but correct.
fn search_messages_like(
    conn: &Connection,
    q: &str,
    limit: usize,
) -> Result<Vec<ContentSearchResult>, String> {
    let pattern = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
    let sql = "
        SELECT
            f.session_db_id,
            s.session_id,
            p.display_name,
            p.original_path,
            f.message_uuid,
            f.role,
            f.timestamp_ms,
            substr(f.content, max(1, instr(lower(f.content), lower(?1)) - 24), 80) AS snip
        FROM messages_fts f
        JOIN sessions s ON s.id = f.session_db_id
        JOIN projects p ON p.id = s.project_id
        WHERE f.content LIKE ?2 ESCAPE '\\'
        ORDER BY f.timestamp_ms DESC
        LIMIT ?3
    ";
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("LIKE query prepare error: {}", e))?;
    let rows = stmt
        .query_map(params![q, pattern, limit as i64], |row| {
            Ok(ContentSearchResult {
                session_db_id: row.get(0)?,
                session_id: row.get(1)?,
                project_name: row.get(2)?,
                project_path: row.get(3)?,
                message_uuid: row.get(4)?,
                role: row.get(5)?,
                timestamp_ms: row.get(6)?,
                snippet: row.get(7)?,
            })
        })
        .map_err(|e| format!("LIKE query error: {}", e))?;

    let mut results = Vec::new();
    for r in rows {
        if let Ok(r) = r {
            results.push(r);
        }
    }
    Ok(results)
}
