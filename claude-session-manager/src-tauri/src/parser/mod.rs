pub mod content;
pub mod messages;

use messages::{RawMessage, ParsedMessage};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct SessionParseResult {
    pub slug: Option<String>,
    pub version: Option<String>,
    pub permission_mode: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub last_active: Option<String>,
    pub message_count: i64,
    pub user_msg_count: i64,
    pub assistant_msg_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
}

/// Parse a session JSONL file and extract metadata for indexing.
/// Does NOT store full messages — those are loaded on demand.
pub fn parse_session_metadata(path: &Path) -> Result<SessionParseResult, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut result = SessionParseResult {
        slug: None,
        version: None,
        permission_mode: None,
        git_branch: None,
        started_at: None,
        last_active: None,
        message_count: 0,
        user_msg_count: 0,
        assistant_msg_count: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
    };

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Extract metadata from first user message
        if raw.msg_type == "user" {
            if result.slug.is_none() {
                result.slug = raw.slug.clone();
            }
            if result.version.is_none() {
                result.version = raw.version.clone();
            }
            if result.git_branch.is_none() {
                result.git_branch = raw.git_branch.clone();
            }
            // Update slug if later messages have it (slug appears after first turn)
            if raw.slug.is_some() {
                result.slug = raw.slug.clone();
            }
        }

        if raw.msg_type == "permission-mode" {
            result.permission_mode = raw.permission_mode.clone();
        }

        // Track timestamps
        if let Some(ref ts) = raw.timestamp {
            if result.started_at.is_none() {
                result.started_at = Some(ts.clone());
            }
            result.last_active = Some(ts.clone());
        }

        // Count messages and tokens
        match raw.msg_type.as_str() {
            "user" => {
                result.message_count += 1;
                result.user_msg_count += 1;
            }
            "assistant" => {
                result.message_count += 1;
                result.assistant_msg_count += 1;
                if let Some(ref msg) = raw.message {
                    if let Some(usage) = msg.get("usage") {
                        result.total_input_tokens += usage.get("input_tokens")
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                        result.total_output_tokens += usage.get("output_tokens")
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Load all messages from a session JSONL for display.
/// Returns parsed messages with offset/limit pagination.
pub fn load_messages(path: &Path, offset: usize, limit: usize) -> Result<Vec<ParsedMessage>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut display_index: usize = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Only count displayable messages for pagination
        let dominated = matches!(raw.msg_type.as_str(), "user" | "assistant" | "system");
        if !dominated {
            continue;
        }

        if display_index >= offset {
            if let Some(parsed) = ParsedMessage::from_raw(&raw) {
                messages.push(parsed);
            }
        }

        display_index += 1;
        if messages.len() >= limit {
            break;
        }
    }

    Ok(messages)
}
