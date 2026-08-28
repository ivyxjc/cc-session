use crate::models::{ViewContentBlock, ViewMessage};
use super::parser::CodexResponseItem;

/// Convert a Codex response item into a ViewMessage.
pub fn to_view_message(item: CodexResponseItem) -> ViewMessage {
    match item {
        CodexResponseItem::UserMessage { timestamp, texts } => {
            ViewMessage::User {
                id: String::new(),
                parent_id: None,
                timestamp,
                content: texts.into_iter()
                    .map(|text| ViewContentBlock::Text { text })
                    .collect(),
            }
        }
        CodexResponseItem::AssistantMessage { timestamp, texts } => {
            ViewMessage::Assistant {
                id: String::new(),
                parent_id: None,
                timestamp,
                model: None,
                content: texts.into_iter()
                    .map(|text| ViewContentBlock::Text { text })
                    .collect(),
                usage: None,
                stop_reason: None,
            }
        }
        CodexResponseItem::DeveloperMessage { timestamp, texts } => {
            ViewMessage::System {
                id: None,
                timestamp,
                subtype: Some("developer".to_string()),
                content: Some(texts.join("\n")),
            }
        }
        CodexResponseItem::FunctionCall { timestamp, call_id, name, arguments } => {
            let input = serde_json::from_str::<serde_json::Value>(&arguments)
                .unwrap_or(serde_json::Value::String(arguments));

            // Wrap in an assistant message with a single ToolCall content block
            ViewMessage::Assistant {
                id: String::new(),
                parent_id: None,
                timestamp,
                model: None,
                content: vec![ViewContentBlock::ToolCall {
                    id: call_id,
                    name,
                    input,
                }],
                usage: None,
                stop_reason: None,
            }
        }
        CodexResponseItem::FunctionCallOutput { timestamp, call_id, output } => {
            // Wrap in a user message with a single ToolResult content block
            ViewMessage::User {
                id: String::new(),
                parent_id: None,
                timestamp,
                content: vec![ViewContentBlock::ToolResult {
                    tool_call_id: call_id,
                    content: serde_json::Value::String(output),
                    is_error: false,
                }],
            }
        }
        CodexResponseItem::Reasoning { timestamp, summary, reasoning_tokens, tokens_shared } => {
            let thinking_text = summary.join("\n");
            ViewMessage::Assistant {
                id: String::new(),
                parent_id: None,
                timestamp,
                model: None,
                content: vec![ViewContentBlock::Thinking {
                    thinking: thinking_text,
                    reasoning_tokens,
                    tokens_shared,
                }],
                usage: None,
                stop_reason: None,
            }
        }
    }
}

/// Project a whole Codex rollout into Claude-shaped raw JSONL objects so that
/// consumers written against Claude's line format (LLM input builder, FTS
/// indexer) work unchanged. Injected environment/instruction payloads are
/// dropped — they are not user prompts.
pub fn to_claude_raw(path: &std::path::Path) -> Result<Vec<serde_json::Value>, String> {
    use serde_json::json;
    use super::parser::is_injected_user_text;
    let items = super::parser::load_messages(path, 0, usize::MAX)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let value = match item {
            CodexResponseItem::UserMessage { timestamp, texts } => {
                let texts: Vec<String> = texts.into_iter().filter(|t| !is_injected_user_text(t)).collect();
                if texts.is_empty() {
                    continue;
                }
                json!({"type": "user", "timestamp": timestamp,
                       "message": {"content": [{"type": "text", "text": texts.join("\n")}]}})
            }
            CodexResponseItem::AssistantMessage { timestamp, texts } => json!({
                "type": "assistant", "timestamp": timestamp,
                "message": {"content": [{"type": "text", "text": texts.join("\n")}]}
            }),
            CodexResponseItem::Reasoning { timestamp, summary, .. } => {
                if summary.is_empty() {
                    continue;
                }
                json!({"type": "assistant", "timestamp": timestamp,
                       "message": {"content": [{"type": "thinking", "thinking": summary.join("\n")}]}})
            }
            CodexResponseItem::FunctionCall { timestamp, call_id, name, .. } => json!({
                "type": "assistant", "timestamp": timestamp,
                "message": {"content": [{"type": "tool_use", "id": call_id, "name": name}]}
            }),
            CodexResponseItem::FunctionCallOutput { timestamp, call_id, .. } => json!({
                "type": "user", "timestamp": timestamp,
                "message": {"content": [{"type": "tool_result", "tool_use_id": call_id}]}
            }),
            CodexResponseItem::DeveloperMessage { .. } => continue,
        };
        out.push(value);
    }
    Ok(out)
}
