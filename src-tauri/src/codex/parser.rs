use crate::parser::{DayTokens, SessionParseResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// A raw event from a Codex JSONL session file.
#[derive(Debug, Clone, Deserialize)]
pub struct CodexEvent {
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// Parsed content from a response_item payload.
#[derive(Debug, Clone)]
pub enum CodexResponseItem {
    UserMessage {
        timestamp: Option<String>,
        texts: Vec<String>,
    },
    AssistantMessage {
        timestamp: Option<String>,
        texts: Vec<String>,
    },
    DeveloperMessage {
        timestamp: Option<String>,
        texts: Vec<String>,
    },
    FunctionCall {
        timestamp: Option<String>,
        call_id: String,
        name: String,
        arguments: String,
    },
    FunctionCallOutput {
        timestamp: Option<String>,
        call_id: String,
        output: String,
    },
    Reasoning {
        timestamp: Option<String>,
        summary: Vec<String>,
        /// Reasoning tokens from the turn's `token_count` event, back-filled after parsing.
        reasoning_tokens: Option<i64>,
        /// True when the turn produced several reasoning items sharing this count.
        tokens_shared: bool,
    },
}

/// Back-fill `reasoning_tokens` onto the reasoning items of the turn that just ended.
/// Codex reports reasoning tokens per API response in a `token_count` event, which
/// arrives after the reasoning items it covers.
fn apply_turn_reasoning_tokens<'a>(
    turn_items: impl Iterator<Item = &'a mut CodexResponseItem>,
    tokens: i64,
    turn_reasoning_count: usize,
) {
    let mut blocks: Vec<&mut CodexResponseItem> = turn_items
        .filter(|i| matches!(i, CodexResponseItem::Reasoning { .. }))
        .collect();
    // Counted over the whole turn, not just the items still held here: paging
    // and the ring buffer can drop earlier reasoning items of the same turn.
    let shared = turn_reasoning_count > 1;
    for item in blocks.iter_mut() {
        if let CodexResponseItem::Reasoning { reasoning_tokens, tokens_shared, .. } = item {
            *reasoning_tokens = Some(tokens);
            *tokens_shared = shared;
        }
    }
}

/// Reasoning tokens from a `token_count` event's `last_token_usage`, if any.
fn last_reasoning_tokens(event: &CodexEvent) -> Option<i64> {
    if event.event_type != "event_msg" || event.payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    event.payload
        .get("info")?
        .get("last_token_usage")?
        .get("reasoning_output_tokens")?
        .as_i64()
}

/// Parse a Codex JSONL file into response items for display.
/// Filters to only displayable events (messages, tool calls, reasoning).
pub fn load_messages(path: &Path, offset: usize, limit: usize) -> Result<Vec<CodexResponseItem>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    // Only items inside the requested window are retained, and reading stops as
    // soon as the window is complete — a turn's reasoning token count arrives
    // after its reasoning items, so the scan continues just far enough to
    // back-fill the last retained turn.
    let end = offset.saturating_add(limit);
    let mut items: Vec<CodexResponseItem> = Vec::new();
    let mut turn_start = 0usize;
    let mut seen = 0usize;
    let mut turn_reasoning = 0usize;
    let mut window_done = false;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(tokens) = last_reasoning_tokens(&event) {
            apply_turn_reasoning_tokens(items[turn_start..].iter_mut(), tokens, turn_reasoning);
            turn_start = items.len();
            turn_reasoning = 0;
            if window_done {
                break;
            }
            continue;
        }

        if let Some(item) = parse_event(&event) {
            if matches!(item, CodexResponseItem::Reasoning { .. }) {
                turn_reasoning += 1;
            }
            if seen >= offset && seen < end {
                items.push(item);
            }
            seen += 1;
            if seen >= end {
                // Nothing retained is still waiting on a token count.
                if turn_start == items.len() {
                    break;
                }
                window_done = true;
            }
        }
    }

    Ok(items)
}

/// Parse a Codex JSONL file and return the latest N items + total count.
pub fn load_latest_messages(path: &Path, count: usize) -> Result<(Vec<CodexResponseItem>, usize), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    // Ring buffer of the last `count` items — keeps memory bounded for large
    // session files instead of materializing every parsed item.
    let mut tail: std::collections::VecDeque<CodexResponseItem> = std::collections::VecDeque::new();
    let mut total = 0usize;
    let mut turn_items = 0usize;
    let mut turn_reasoning = 0usize;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(tokens) = last_reasoning_tokens(&event) {
            // Items dropped from the ring buffer can no longer be back-filled; the
            // ones still buffered are exactly the ones the UI will render.
            apply_turn_reasoning_tokens(tail.iter_mut().rev().take(turn_items), tokens, turn_reasoning);
            turn_items = 0;
            turn_reasoning = 0;
            continue;
        }

        if let Some(item) = parse_event(&event) {
            if matches!(item, CodexResponseItem::Reasoning { .. }) {
                turn_reasoning += 1;
            }
            turn_items += 1;
            total += 1;
            if count > 0 {
                if tail.len() == count {
                    tail.pop_front();
                }
                tail.push_back(item);
            }
        }
    }

    Ok((tail.into_iter().collect(), total))
}

/// Token usage carried by a `token_count` event's `last_token_usage`, mapped onto the
/// Claude-style split the index uses. Codex's `input_tokens` *includes* cached
/// tokens, so the uncached share is derived.
struct TurnUsage {
    input: i64,
    output: i64,
    cache_creation: i64,
    cache_read: i64,
}

fn last_turn_usage(event: &CodexEvent) -> Option<TurnUsage> {
    if event.event_type != "event_msg" || event.payload.get("type")?.as_str()? != "token_count" {
        return None;
    }
    let usage = event.payload.get("info")?.get("last_token_usage")?;
    let get = |k: &str| usage.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    let cached = get("cached_input_tokens");
    Some(TurnUsage {
        input: (get("input_tokens") - cached).max(0),
        output: get("output_tokens"),
        cache_creation: get("cache_write_input_tokens"),
        cache_read: cached,
    })
}

/// Codex injects environment/instruction payloads as `user` messages wrapped in a
/// single XML-ish tag (`<environment_context>…</environment_context>`). Those are
/// not prompts the person typed.
pub fn is_injected_user_text(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("<environment_context>")
        || t.starts_with("<user_instructions>")
        || t.starts_with("<permissions instructions>")
        || t.starts_with("<turn_aborted>")
        || t.starts_with("<skills_instructions>")
}

/// Session-level metadata for the index, mirroring `parser::parse_session_metadata`.
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
        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(ref ts) = event.timestamp {
            if result.started_at.is_none() {
                result.started_at = Some(ts.clone());
            }
            result.last_active = Some(ts.clone());
        }

        if event.event_type == "session_meta" {
            result.version = event.payload.get("cli_version").and_then(|v| v.as_str()).map(String::from);
            result.git_branch = event.payload.get("git").and_then(|g| g.get("branch")).and_then(|v| v.as_str()).map(String::from);
            continue;
        }

        if let Some(usage) = last_turn_usage(&event) {
            result.total_input_tokens += usage.input;
            result.total_output_tokens += usage.output;
            result.total_cache_creation_tokens += usage.cache_creation;
            result.total_cache_read_tokens += usage.cache_read;
            continue;
        }

        match parse_event(&event) {
            Some(CodexResponseItem::UserMessage { texts, .. }) => {
                if texts.iter().all(|t| is_injected_user_text(t)) {
                    continue;
                }
                result.message_count += 1;
                result.user_msg_count += 1;
                if result.summary.is_none() {
                    if let Some(text) = texts.iter().find(|t| !is_injected_user_text(t)) {
                        let cleaned = crate::parser::clean_summary_text(text);
                        if !cleaned.is_empty() {
                            result.summary = Some(crate::parser::truncate_at_boundary(&cleaned, 100));
                        }
                    }
                }
            }
            Some(CodexResponseItem::AssistantMessage { .. }) => {
                result.message_count += 1;
                result.assistant_msg_count += 1;
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Per-day token usage, mirroring `parser::extract_daily_tokens`.
pub fn extract_daily_tokens(path: &Path) -> Result<HashMap<String, DayTokens>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut daily: HashMap<String, DayTokens> = HashMap::new();
    let mut current_date = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }
        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(ts) = event.timestamp.as_deref() {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                current_date = dt.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string();
            }
        }
        let date = || if current_date.is_empty() { "unknown".to_string() } else { current_date.clone() };

        if let Some(usage) = last_turn_usage(&event) {
            let entry = daily.entry(date()).or_default();
            entry.input_tokens += usage.input;
            entry.output_tokens += usage.output;
            entry.cache_creation_tokens += usage.cache_creation;
            entry.cache_read_tokens += usage.cache_read;
            continue;
        }

        if let Some(CodexResponseItem::UserMessage { texts, .. }) = parse_event(&event) {
            if !texts.iter().all(|t| is_injected_user_text(t)) {
                daily.entry(date()).or_default().user_msg_count += 1;
            }
        }
    }

    Ok(daily)
}

fn parse_event(event: &CodexEvent) -> Option<CodexResponseItem> {
    match event.event_type.as_str() {
        "response_item" => parse_response_item(event),
        _ => None, // Skip session_meta, event_msg, turn_context
    }
}

fn parse_response_item(event: &CodexEvent) -> Option<CodexResponseItem> {
    let payload = &event.payload;
    let item_type = payload.get("type")?.as_str()?;

    match item_type {
        "message" => {
            let role = payload.get("role")?.as_str()?;
            let texts = extract_texts(payload);
            if texts.is_empty() {
                return None;
            }
            match role {
                "user" => Some(CodexResponseItem::UserMessage {
                    timestamp: event.timestamp.clone(),
                    texts,
                }),
                "assistant" => Some(CodexResponseItem::AssistantMessage {
                    timestamp: event.timestamp.clone(),
                    texts,
                }),
                "developer" => Some(CodexResponseItem::DeveloperMessage {
                    timestamp: event.timestamp.clone(),
                    texts,
                }),
                _ => None,
            }
        }
        "function_call" => {
            let call_id = payload.get("call_id")?.as_str()?.to_string();
            let name = payload.get("name")?.as_str()?.to_string();
            let arguments = payload.get("arguments")?.as_str()?.to_string();
            Some(CodexResponseItem::FunctionCall {
                timestamp: event.timestamp.clone(),
                call_id,
                name,
                arguments,
            })
        }
        "function_call_output" => {
            let call_id = payload.get("call_id")?.as_str()?.to_string();
            let output = payload.get("output")?.as_str()?.to_string();
            Some(CodexResponseItem::FunctionCallOutput {
                timestamp: event.timestamp.clone(),
                call_id,
                output,
            })
        }
        "reasoning" => {
            // Summary entries are objects like {"type":"summary_text","text":"..."},
            // though bare strings are also accepted.
            let summary = payload.get("summary")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        _ => v.get("text").and_then(|t| t.as_str()).map(String::from),
                    })
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>())
                .unwrap_or_default();
            // Encrypted-only reasoning carries no readable text; emit it with an empty
            // summary so the UI renders a redacted placeholder.
            let has_encrypted = payload.get("encrypted_content").is_some();
            if summary.is_empty() && !has_encrypted {
                return None;
            }
            Some(CodexResponseItem::Reasoning {
                timestamp: event.timestamp.clone(),
                summary,
                reasoning_tokens: None,
                tokens_shared: false,
            })
        }
        _ => None,
    }
}

/// Extract text content from a message's content array.
fn extract_texts(payload: &serde_json::Value) -> Vec<String> {
    let content = match payload.get("content").and_then(|c| c.as_array()) {
        Some(arr) => arr,
        None => return vec![],
    };

    content.iter()
        .filter_map(|block| {
            let block_type = block.get("type")?.as_str()?;
            match block_type {
                "input_text" | "output_text" => {
                    block.get("text")?.as_str().map(String::from)
                }
                _ => None,
            }
        })
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reasoning_event(payload: &str) -> CodexEvent {
        CodexEvent {
            timestamp: None,
            event_type: "response_item".to_string(),
            payload: serde_json::from_str(payload).unwrap(),
        }
    }

    #[test]
    fn reasoning_summary_objects_are_extracted() {
        let event = reasoning_event(
            r#"{"type":"reasoning","summary":[{"type":"summary_text","text":"**Planning**"}],"encrypted_content":"gAAA"}"#,
        );
        match parse_response_item(&event) {
            Some(CodexResponseItem::Reasoning { summary, .. }) => {
                assert_eq!(summary, vec!["**Planning**".to_string()]);
            }
            other => panic!("expected reasoning, got {:?}", other),
        }
    }

    #[test]
    fn encrypted_only_reasoning_has_empty_summary() {
        let event = reasoning_event(r#"{"type":"reasoning","summary":[],"encrypted_content":"gAAA"}"#);
        match parse_response_item(&event) {
            Some(CodexResponseItem::Reasoning { summary, .. }) => assert!(summary.is_empty()),
            other => panic!("expected reasoning, got {:?}", other),
        }
    }

    #[test]
    fn reasoning_without_summary_or_encryption_is_skipped() {
        let event = reasoning_event(r#"{"type":"reasoning","summary":[]}"#);
        assert!(parse_response_item(&event).is_none());
    }

    fn write_session(lines: &[&str]) -> std::path::PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cc-session-test-{}-{}", std::process::id(), seq));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rollout.jsonl");
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    const REASONING: &str = r#"{"timestamp":"t","type":"response_item","payload":{"type":"reasoning","summary":[{"type":"summary_text","text":"step"}],"encrypted_content":"gAAA"}}"#;

    fn token_count(reasoning: i64) -> String {
        format!(
            r#"{{"timestamp":"t","type":"event_msg","payload":{{"type":"token_count","info":{{"last_token_usage":{{"output_tokens":100,"reasoning_output_tokens":{}}}}}}}}}"#,
            reasoning
        )
    }

    fn tokens_of(item: &CodexResponseItem) -> (Option<i64>, bool) {
        match item {
            CodexResponseItem::Reasoning { reasoning_tokens, tokens_shared, .. } => {
                (*reasoning_tokens, *tokens_shared)
            }
            other => panic!("expected reasoning, got {:?}", other),
        }
    }

    #[test]
    fn turn_reasoning_tokens_are_back_filled() {
        let tc = token_count(468);
        let path = write_session(&[REASONING, &tc]);
        let items = load_messages(&path, 0, 10).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(tokens_of(&items[0]), (Some(468), false));
    }

    #[test]
    fn multiple_reasoning_items_share_the_turn_count() {
        let tc = token_count(480);
        let path = write_session(&[REASONING, REASONING, &tc]);
        let items = load_messages(&path, 0, 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(tokens_of(&items[0]), (Some(480), true));
        assert_eq!(tokens_of(&items[1]), (Some(480), true));
    }

    #[test]
    fn tokens_do_not_leak_across_turns() {
        let first = token_count(10);
        let second = token_count(20);
        let path = write_session(&[REASONING, &first, REASONING, &second, REASONING]);
        let items = load_messages(&path, 0, 10).unwrap();
        assert_eq!(tokens_of(&items[0]), (Some(10), false));
        assert_eq!(tokens_of(&items[1]), (Some(20), false));
        // Trailing turn has no token_count yet (live session).
        assert_eq!(tokens_of(&items[2]), (None, false));
    }

    #[test]
    fn paged_window_still_back_fills_shared_flag() {
        // One turn with two reasoning items; the page starts at the second, so the
        // first is never retained — the shared flag must still be true.
        let tc = token_count(90);
        let path = write_session(&[REASONING, REASONING, &tc]);
        let items = load_messages(&path, 1, 1).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(tokens_of(&items[0]), (Some(90), true));
    }

    #[test]
    fn paging_stops_at_the_window() {
        let first = token_count(10);
        let second = token_count(20);
        let path = write_session(&[REASONING, &first, REASONING, &second, REASONING]);
        let items = load_messages(&path, 0, 2).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(tokens_of(&items[0]), (Some(10), false));
        assert_eq!(tokens_of(&items[1]), (Some(20), false));
    }

    const META: &str = r#"{"timestamp":"2026-08-09T16:15:29.208Z","type":"session_meta","payload":{"id":"t1","cwd":"/w","cli_version":"0.147.0","git":{"branch":"main"}}}"#;
    const ENV_CTX: &str = r#"{"timestamp":"2026-08-09T16:15:30.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>\n  <cwd>/w</cwd>\n</environment_context>"}]}}"#;
    const USER: &str = r#"{"timestamp":"2026-08-09T16:15:31.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"fix the login bug"}]}}"#;
    const ASSISTANT: &str = r#"{"timestamp":"2026-08-09T16:15:40.000Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done"}]}}"#;
    const TOKENS: &str = r#"{"timestamp":"2026-08-09T16:15:41.000Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":23986,"cached_input_tokens":11008,"cache_write_input_tokens":0,"output_tokens":275,"reasoning_output_tokens":126}}}}"#;

    #[test]
    fn session_metadata_mirrors_the_claude_index_fields() {
        let path = write_session(&[META, ENV_CTX, USER, ASSISTANT, TOKENS]);
        let r = parse_session_metadata(&path).unwrap();
        assert_eq!(r.version.as_deref(), Some("0.147.0"));
        assert_eq!(r.git_branch.as_deref(), Some("main"));
        assert_eq!(r.started_at.as_deref(), Some("2026-08-09T16:15:29.208Z"));
        assert_eq!(r.last_active.as_deref(), Some("2026-08-09T16:15:41.000Z"));
        // The injected <environment_context> message is not a user turn.
        assert_eq!(r.user_msg_count, 1);
        assert_eq!(r.assistant_msg_count, 1);
        assert_eq!(r.message_count, 2);
        assert_eq!(r.summary.as_deref(), Some("fix the login bug"));
        // Codex input_tokens includes cached tokens; the index stores them split.
        assert_eq!(r.total_input_tokens, 23986 - 11008);
        assert_eq!(r.total_cache_read_tokens, 11008);
        assert_eq!(r.total_output_tokens, 275);
    }

    #[test]
    fn daily_tokens_attribute_usage_to_the_turn_date() {
        let path = write_session(&[META, USER, ASSISTANT, TOKENS]);
        let daily = extract_daily_tokens(&path).unwrap();
        assert_eq!(daily.len(), 1);
        let day = daily.values().next().unwrap();
        assert_eq!(day.user_msg_count, 1);
        assert_eq!(day.output_tokens, 275);
        assert_eq!(day.cache_read_tokens, 11008);
    }

    #[test]
    fn claude_raw_projection_drops_injected_context_and_keeps_tool_shape() {
        let path = write_session(&[META, ENV_CTX, USER, REASONING, ASSISTANT]);
        let raw = super::super::converter::to_claude_raw(&path).unwrap();
        let types: Vec<&str> = raw.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["user", "assistant", "assistant"]);
        assert_eq!(raw[0]["message"]["content"][0]["text"], "fix the login bug");
        assert_eq!(raw[1]["message"]["content"][0]["type"], "thinking");
    }

    #[test]
    fn latest_messages_also_back_fill() {
        let tc = token_count(77);
        let path = write_session(&[REASONING, REASONING, &tc, REASONING]);
        let (items, total) = load_latest_messages(&path, 2).unwrap();
        assert_eq!(total, 3);
        assert_eq!(items.len(), 2);
        // The turn had two reasoning items, so the count is marked as shared even
        // though the first item has already fallen out of the ring buffer.
        assert_eq!(tokens_of(&items[0]), (Some(77), true));
        assert_eq!(tokens_of(&items[1]), (None, false));
    }
}

