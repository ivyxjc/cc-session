use serde::Deserialize;
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

