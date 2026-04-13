pub mod content;
pub mod messages;

use messages::{RawMessage, ParsedMessage};
use std::collections::HashMap;
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
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
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
        total_cache_creation_tokens: 0,
        total_cache_read_tokens: 0,
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
                // Only count as real user message if content has non-tool_result blocks
                let is_real_user_msg = raw.message.as_ref()
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_array())
                    .map(|arr| arr.iter().any(|b| {
                        b.get("type").and_then(|t| t.as_str()) != Some("tool_result")
                    }))
                    .unwrap_or(true); // string content = real user message
                if is_real_user_msg {
                    result.user_msg_count += 1;
                }
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
                        result.total_cache_creation_tokens += usage.get("cache_creation_input_tokens")
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                        result.total_cache_read_tokens += usage.get("cache_read_input_tokens")
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

/// Result of loading latest messages, includes total count for pagination.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestMessagesResult {
    pub messages: Vec<ParsedMessage>,
    pub total_count: usize,
}

/// Load the latest N messages from a session JSONL (from the end of the file).
/// Used for live session views where we want to see the most recent messages.
/// Returns the messages and the total displayable message count.
pub fn load_latest_messages(path: &Path, count: usize) -> Result<LatestMessagesResult, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut all_raws: Vec<RawMessage> = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if matches!(raw.msg_type.as_str(), "user" | "assistant" | "system") {
            all_raws.push(raw);
        }
    }

    let total_count = all_raws.len();
    let skip = total_count.saturating_sub(count);
    let messages = all_raws
        .into_iter()
        .skip(skip)
        .filter_map(|raw| ParsedMessage::from_raw(&raw))
        .collect();

    Ok(LatestMessagesResult {
        messages,
        total_count,
    })
}

/// Per-day token usage extracted from a JSONL file.
#[derive(Debug, Clone, Default)]
pub struct DayTokens {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub user_msg_count: i64,
}

/// Parse a JSONL file and return token usage grouped by date (YYYY-MM-DD).
/// Uses each assistant message's timestamp to determine the day.
pub fn extract_daily_tokens(path: &Path) -> Result<HashMap<String, DayTokens>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut daily: HashMap<String, DayTokens> = HashMap::new();
    let mut current_date = String::new(); // fallback date from most recent timestamp

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: serde_json::Value = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Track the most recent timestamp for date attribution (converted to local timezone)
        if let Some(ts) = raw.get("timestamp").and_then(|v| v.as_str()) {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                let local = dt.with_timezone(&chrono::Local);
                current_date = local.format("%Y-%m-%d").to_string();
            }
        }

        let msg_type = raw.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Count real user messages (not tool_result-only)
        if msg_type == "user" {
            let is_real = raw.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|b| b.get("type").and_then(|t| t.as_str()) != Some("tool_result")))
                .unwrap_or(true);
            if is_real {
                let date = if current_date.is_empty() { "unknown".to_string() } else { current_date.clone() };
                let entry = daily.entry(date).or_default();
                entry.user_msg_count += 1;
            }
        }

        if msg_type == "assistant" {
            if let Some(usage) = raw.get("message").and_then(|m| m.get("usage")) {
                let input = usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                let output = usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                let cache_creation = usage.get("cache_creation_input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);

                let date = if current_date.is_empty() { "unknown".to_string() } else { current_date.clone() };
                let entry = daily.entry(date).or_default();
                entry.input_tokens += input;
                entry.output_tokens += output;
                entry.cache_creation_tokens += cache_creation;
                entry.cache_read_tokens += cache_read;
            }
        }
    }

    Ok(daily)
}
