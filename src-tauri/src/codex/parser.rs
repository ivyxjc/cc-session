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
    },
}

/// Parse a Codex JSONL file into response items for display.
/// Filters to only displayable events (messages, tool calls, reasoning).
pub fn load_messages(path: &Path, offset: usize, limit: usize) -> Result<Vec<CodexResponseItem>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut items = Vec::new();
    let mut display_index: usize = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(item) = parse_event(&event) {
            if display_index >= offset {
                items.push(item);
            }
            display_index += 1;
            if items.len() >= limit {
                break;
            }
        }
    }

    Ok(items)
}

/// Parse a Codex JSONL file and return the latest N items + total count.
pub fn load_latest_messages(path: &Path, count: usize) -> Result<(Vec<CodexResponseItem>, usize), String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut all_items = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let event: CodexEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(item) = parse_event(&event) {
            all_items.push(item);
        }
    }

    let total = all_items.len();
    let skip = total.saturating_sub(count);
    let messages = all_items.into_iter().skip(skip).collect();

    Ok((messages, total))
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
            let summary = payload.get("summary")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>())
                .unwrap_or_default();
            // If summary is empty and content is encrypted, show placeholder
            let has_encrypted = payload.get("encrypted_content").is_some();
            if summary.is_empty() && !has_encrypted {
                return None;
            }
            Some(CodexResponseItem::Reasoning {
                timestamp: event.timestamp.clone(),
                summary: if summary.is_empty() { vec!["[encrypted reasoning]".to_string()] } else { summary },
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
