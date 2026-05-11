//! Orchestrate generation of LLM summary + tags for a session.

use crate::db::Database;
use crate::llm::client::{parse_json_payload, LlmClient, LlmConfig, LlmError};
use crate::llm::input_builder;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;

/// Bump when the LLM input format or prompt changes — invalidates cached hashes.
pub const SCHEMA_VERSION: &str = "v1";

const SYSTEM_PROMPT: &str = "You generate concise summaries and topical tags for Claude Code sessions.

You will receive:
- INITIAL REQUEST: the user's first message (cleaned)
- RECENT CONVERSATION: the last several turns of the session, in chronological order
- TOOLS USED IN SESSION: aggregate tool-call counts

Return ONLY valid JSON, no markdown fences, no commentary, in this exact shape:
{\"summary\": \"...\", \"tags\": [\"...\", \"...\"]}

Rules:
- summary: <= 60 characters, single line, no trailing punctuation. Use the SAME language as the input. Be specific (\"Implement FTS5 search with BM25\" not \"Help with code\").
- tags: 2-4 short, lowercase English topical labels. Use hyphens for multi-word tags (e.g. \"search\", \"rust\", \"ui-fix\", \"db-schema\"). No spaces.
- If the session is too short or unclear, still produce a best-guess summary; never refuse.";

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SummaryAndTags {
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum GenError {
    NotConfigured,
    SessionNotFound,
    Build(String),
    Db(String),
    Llm(LlmError),
}

impl std::fmt::Display for GenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "AI summary not configured (set base URL, API key and model in Settings)"),
            Self::SessionNotFound => write!(f, "session not found"),
            Self::Build(s) => write!(f, "input build failed: {}", s),
            Self::Db(s) => write!(f, "db error: {}", s),
            Self::Llm(e) => write!(f, "llm error: {}", e),
        }
    }
}

impl From<GenError> for String {
    fn from(e: GenError) -> Self {
        e.to_string()
    }
}

/// Read the AI summary configuration from `app_config`.
pub fn load_config(db: &Database) -> Result<Option<LlmConfig>, String> {
    let conn = db.conn();
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'ai_summary_config'",
            [],
            |row| row.get(0),
        )
        .ok();
    let Some(json) = raw else { return Ok(None) };
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let base_url = v.get("baseUrl").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let api_key = v.get("apiKey").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let model = v.get("model").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if base_url.is_empty() || api_key.is_empty() || model.is_empty() {
        return Ok(None);
    }
    Ok(Some(LlmConfig { base_url, api_key, model }))
}

#[derive(Debug)]
pub struct GenerateOutcome {
    pub generated: bool, // true if LLM was called; false if skipped via hash cache
    pub summary: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Generate (or skip) AI summary for a single session by DB id.
/// `force=true` ignores the cached hash and always regenerates.
pub async fn generate_for_session(
    db: Arc<Database>,
    session_db_id: i64,
    force: bool,
) -> Result<GenerateOutcome, GenError> {
    // Load config first (no-op without it).
    let cfg = load_config(&db).map_err(GenError::Db)?.ok_or(GenError::NotConfigured)?;

    // Look up jsonl path + last_active + summary_at + cached hash.
    type SessionInfoRow = (String, Option<i64>, Option<i64>, Option<String>);
    let session_info: Option<SessionInfoRow> = {
        let conn = db.conn();
        conn.query_row(
            "SELECT jsonl_path, last_active, summary_at, summary_input_hash
             FROM sessions WHERE id = ?1",
            params![session_db_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok()
    };
    let (jsonl_path, last_active, summary_at, prev_hash) =
        session_info.ok_or(GenError::SessionNotFound)?;

    // Layer 1: timestamp pre-filter — skip if session unchanged since last summary.
    if !force {
        if let (Some(la), Some(sa)) = (last_active, summary_at) {
            if la <= sa {
                return Ok(GenerateOutcome { generated: false, summary: None, tags: None });
            }
        }
    }

    // Build LLM input slice (sync work).
    let path = PathBuf::from(&jsonl_path);
    let input = input_builder::build(&path).map_err(GenError::Build)?;
    let rendered = input.render();
    let new_hash = format!("{}:{:x}", SCHEMA_VERSION, Sha256::digest(rendered.as_bytes()));

    // Layer 2: hash precise check — skip if input slice unchanged.
    if !force {
        if let Some(prev) = prev_hash.as_deref() {
            if prev == new_hash {
                // touch summary_at so we don't keep re-checking
                let now = chrono::Utc::now().timestamp_millis();
                let conn = db.conn();
                let _ = conn.execute(
                    "UPDATE sessions SET summary_at = ?1 WHERE id = ?2",
                    params![now, session_db_id],
                );
                return Ok(GenerateOutcome { generated: false, summary: None, tags: None });
            }
        }
    }

    // Call LLM.
    let client = LlmClient::new(cfg);
    let raw = client
        .complete(SYSTEM_PROMPT, &rendered, 400)
        .await
        .map_err(GenError::Llm)?;
    let parsed: SummaryAndTags = parse_json_payload(&raw).map_err(GenError::Llm)?;

    // Light validation/normalization.
    let summary = clamp_summary(&parsed.summary);
    let tags = normalize_tags(parsed.tags);
    let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    let now = chrono::Utc::now().timestamp_millis();

    // Persist.
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE sessions
             SET summary = ?1, summary_source = 'llm', summary_at = ?2,
                 summary_input_hash = ?3, ai_tags = ?4
             WHERE id = ?5",
            params![summary, now, new_hash, tags_json, session_db_id],
        )
        .map_err(|e| GenError::Db(e.to_string()))?;
    }

    Ok(GenerateOutcome {
        generated: true,
        summary: Some(summary),
        tags: Some(tags),
    })
}

fn clamp_summary(s: &str) -> String {
    let cleaned: String = s.replace('\n', " ").trim().to_string();
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() <= 80 {
        return cleaned;
    }
    let mut out: String = chars[..80].iter().collect();
    out.push('…');
    out
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|t| {
            t.trim()
                .to_lowercase()
                .replace([' ', '_'], "-")
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect::<String>()
        })
        .filter(|t| !t.is_empty() && t.len() <= 30)
        .take(4)
        .collect()
}
