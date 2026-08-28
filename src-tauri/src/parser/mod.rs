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
    pub summary: Option<String>,
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
        summary: None,
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
                    // Compute heuristic summary from the first real user message
                    if result.summary.is_none() {
                        if let Some(text) = raw.message.as_ref()
                            .and_then(|m| m.get("content"))
                            .and_then(extract_first_user_text)
                        {
                            let cleaned = clean_summary_text(&text);
                            if !cleaned.is_empty() {
                                result.summary = Some(truncate_at_boundary(&cleaned, 100));
                            }
                        }
                    }
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

/// View-layer result with ViewMessage instead of ParsedMessage.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewLatestMessagesResult {
    pub messages: Vec<crate::models::ViewMessage>,
    pub total_count: usize,
}

/// Load the latest N messages from a session JSONL (from the end of the file).
/// Used for live session views where we want to see the most recent messages.
/// Returns the messages and the total displayable message count.
pub fn load_latest_messages(path: &Path, count: usize) -> Result<LatestMessagesResult, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    // Ring buffer of the last `count` raws — keeps memory bounded even for
    // multi-hundred-MB session files instead of materializing every message.
    let mut tail: std::collections::VecDeque<RawMessage> = std::collections::VecDeque::new();
    let mut total_count = 0usize;

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
            total_count += 1;
            if count > 0 {
                if tail.len() == count {
                    tail.pop_front();
                }
                tail.push_back(raw);
            }
        }
    }

    let messages = tail
        .into_iter()
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

// ===== Heuristic session summary helpers =====

/// Extract the first text content from a user message's `content` field.
/// `content` may be a JSON string (legacy) or an array of blocks.
/// Skips messages that are tool_result-only.
fn extract_first_user_text(content: &serde_json::Value) -> Option<String> {
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    let arr = content.as_array()?;
    let has_text = arr.iter().any(|b| b.get("type").and_then(|v| v.as_str()) == Some("text"));
    if !has_text {
        return None; // tool_result-only — skip and let the next message be tried
    }
    for block in arr {
        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Clean a raw user message text for summary use:
/// - strip Claude Code wrapper tags (system-reminder, local-command-stdout/stderr, command-message)
/// - if it's a slash command, prefix with "/<name>" + args
/// - normalize whitespace
pub fn clean_summary_text(text: &str) -> String {
    // Local-command echoes (/model, /login, …) are not real prompts — skip the
    // whole message so the next genuine user message becomes the summary.
    if text.contains("<local-command-caveat>") {
        return String::new();
    }
    let mut s = text.to_string();
    for tag in &[
        "system-reminder",
        "local-command-stdout",
        "local-command-stderr",
        "command-message",
    ] {
        s = strip_tag_with_content(&s, tag);
    }

    // Slash command handling
    let cmd_name = extract_tag_content(&s, "command-name");
    let cmd_args = extract_tag_content(&s, "command-args");
    s = strip_tag_with_content(&s, "command-name");
    s = strip_tag_with_content(&s, "command-args");

    // Normalize whitespace (collapse runs of whitespace incl. newlines into single space)
    let normalized: String = s.split_whitespace().collect::<Vec<_>>().join(" ");

    if let Some(name) = cmd_name {
        // JSONL sometimes records the name with a leading slash already.
        let name = name.trim().trim_start_matches('/');
        if !name.is_empty() {
            let mut out = format!("/{}", name);
            if let Some(args) = cmd_args.as_deref().map(str::trim).filter(|x| !x.is_empty()) {
                out.push(' ');
                out.push_str(args);
            }
            if !normalized.is_empty() {
                out.push(' ');
                out.push_str(&normalized);
            }
            return out.trim().to_string();
        }
    }

    normalized.trim().to_string()
}

/// Strip `<tag>...</tag>` (including content) from `s`, all occurrences.
fn strip_tag_with_content(s: &str, tag: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find(&open) {
        out.push_str(&rest[..start]);
        match rest[start..].find(&close) {
            Some(end_off) => rest = &rest[start + end_off + close.len()..],
            None => return out, // unclosed tag — drop everything from here on
        }
    }
    out.push_str(rest);
    out
}

/// Extract the content between `<tag>` and `</tag>` (first occurrence only).
fn extract_tag_content(s: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = s.find(&open)? + open.len();
    let end_off = s[start..].find(&close)?;
    Some(s[start..start + end_off].to_string())
}

/// Truncate a string to at most `max_chars` characters, preferring a sentence
/// boundary (。！？.!?\n) within the last 30 chars before the cap.
pub fn truncate_at_boundary(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return chars.into_iter().collect();
    }
    let boundary_chars = ['。', '！', '？', '.', '!', '?', '\n'];
    let search_start = max_chars.saturating_sub(30);
    for i in (search_start..max_chars).rev() {
        if boundary_chars.contains(&chars[i]) {
            let mut out: String = chars[..=i].iter().collect();
            out.push('…');
            return out;
        }
    }
    let mut out: String = chars[..max_chars].iter().collect();
    out.push('…');
    out
}

#[cfg(test)]
mod summary_tests {
    use super::*;

    #[test]
    fn plain_user_message() {
        let r = clean_summary_text("帮我加一个搜索功能");
        assert_eq!(r, "帮我加一个搜索功能");
    }

    #[test]
    fn strips_system_reminder() {
        let r = clean_summary_text("<system-reminder>noise</system-reminder>实际任务");
        assert_eq!(r, "实际任务");
    }

    #[test]
    fn slash_command_with_args() {
        let r = clean_summary_text("<command-name>init</command-name><command-args>--force</command-args>");
        assert_eq!(r, "/init --force");
    }

    #[test]
    fn slash_command_bare() {
        let r = clean_summary_text("<command-name>compact</command-name><command-args></command-args>");
        assert_eq!(r, "/compact");
    }

    #[test]
    fn slash_command_name_with_leading_slash() {
        let r = clean_summary_text("<command-name>/model</command-name><command-args></command-args>");
        assert_eq!(r, "/model");
    }

    #[test]
    fn local_command_caveat_skipped_entirely() {
        let r = clean_summary_text(
            "<local-command-caveat>Caveat: The messages below were generated by the user while running local commands.</local-command-caveat>\
             <command-name>/model</command-name><command-message>model</command-message>\
             <local-command-stdout>Set model to Fable 5</local-command-stdout>",
        );
        assert_eq!(r, "");
    }

    #[test]
    fn collapses_whitespace() {
        let r = clean_summary_text("行1\n\n  行2   行3");
        assert_eq!(r, "行1 行2 行3");
    }

    #[test]
    fn truncate_short_string_unchanged() {
        let r = truncate_at_boundary("短文本", 100);
        assert_eq!(r, "短文本");
    }

    #[test]
    fn truncate_at_period() {
        let s = "第一句话。第二句话内容很长很长很长很长。第三句也很长很长很长很长很长很长很长很长。";
        let r = truncate_at_boundary(s, 20);
        assert!(r.ends_with("…"));
        assert!(r.contains("第一句话。"));
    }
}
