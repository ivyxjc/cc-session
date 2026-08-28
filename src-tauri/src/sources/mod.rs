//! Provider boundary.
//!
//! Every session in the index carries a `Provider`. Code outside `claude/` and
//! `codex/` never touches a provider-specific parser directly — it asks this
//! module, which dispatches on the provider stored next to the session's file
//! path. Adding a provider means adding a variant here and an adapter module;
//! the rest of the app (DB, commands, LLM, search, UI) stays untouched.

use crate::models::ViewMessage;
use crate::parser::{DayTokens, SessionParseResult, ViewLatestMessagesResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    /// Parse the value stored in the `provider` column. Unknown values fall back
    /// to Claude, which is also the schema default.
    pub fn from_db(value: &str) -> Self {
        match value {
            "codex" => Provider::Codex,
            _ => Provider::Claude,
        }
    }
}

impl rusqlite::types::FromSql for Provider {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(Provider::from_db)
    }
}

impl rusqlite::types::ToSql for Provider {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

/// Session-level metadata used by the scanner to fill the `sessions` row.
pub fn parse_session_metadata(provider: Provider, path: &Path) -> Result<SessionParseResult, String> {
    match provider {
        Provider::Claude => crate::parser::parse_session_metadata(path),
        Provider::Codex => crate::codex::parser::parse_session_metadata(path),
    }
}

/// Per-day token usage for the `daily_token_usage` table.
pub fn extract_daily_tokens(provider: Provider, path: &Path) -> Result<HashMap<String, DayTokens>, String> {
    match provider {
        Provider::Claude => crate::parser::extract_daily_tokens(path),
        Provider::Codex => crate::codex::parser::extract_daily_tokens(path),
    }
}

/// Paged messages for the conversation view.
pub fn load_messages(provider: Provider, path: &Path, offset: usize, limit: usize) -> Result<Vec<ViewMessage>, String> {
    match provider {
        Provider::Claude => Ok(crate::parser::load_messages(path, offset, limit)?
            .into_iter()
            .map(crate::claude::converter::to_view_message)
            .collect()),
        Provider::Codex => Ok(crate::codex::parser::load_messages(path, offset, limit)?
            .into_iter()
            .map(crate::codex::converter::to_view_message)
            .collect()),
    }
}

/// Tail of the conversation plus the total displayable count.
pub fn load_latest_messages(provider: Provider, path: &Path, count: usize) -> Result<ViewLatestMessagesResult, String> {
    match provider {
        Provider::Claude => {
            let result = crate::parser::load_latest_messages(path, count)?;
            Ok(ViewLatestMessagesResult {
                messages: result.messages.into_iter().map(crate::claude::converter::to_view_message).collect(),
                total_count: result.total_count,
            })
        }
        Provider::Codex => {
            let (items, total_count) = crate::codex::parser::load_latest_messages(path, count)?;
            Ok(ViewLatestMessagesResult {
                messages: items.into_iter().map(crate::codex::converter::to_view_message).collect(),
                total_count,
            })
        }
    }
}

/// Whole session as Claude-shaped raw JSONL objects
/// (`{"type","timestamp","message":{"content":[...]}}`).
///
/// The LLM input builder and the FTS indexer were written against Claude's raw
/// line format and are deliberately left that way; other providers are projected
/// into that shape here so those consumers stay single-path.
pub fn raw_messages(provider: Provider, path: &Path) -> Result<Vec<serde_json::Value>, String> {
    match provider {
        Provider::Claude => {
            use std::io::BufRead;
            let file = std::fs::File::open(path).map_err(|e| format!("open: {}", e))?;
            let mut out = Vec::new();
            for line in std::io::BufReader::new(file).lines() {
                let line = line.map_err(|e| format!("read: {}", e))?;
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    out.push(v);
                }
            }
            Ok(out)
        }
        Provider::Codex => crate::codex::converter::to_claude_raw(path),
    }
}
