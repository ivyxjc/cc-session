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
        CodexResponseItem::Reasoning { timestamp, summary } => {
            let thinking_text = summary.join("\n");
            ViewMessage::Assistant {
                id: String::new(),
                parent_id: None,
                timestamp,
                model: None,
                content: vec![ViewContentBlock::Thinking {
                    thinking: thinking_text,
                }],
                usage: None,
                stop_reason: None,
            }
        }
    }
}
