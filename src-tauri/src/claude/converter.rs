use crate::models::{ViewContentBlock, ViewImageSource, ViewMessage, ViewUsage};
use crate::parser::content::{ContentBlock, Usage};
use crate::parser::messages::ParsedMessage;

pub fn to_view_message(msg: ParsedMessage) -> ViewMessage {
    match msg {
        ParsedMessage::User { uuid, parent_uuid, timestamp, content } => {
            ViewMessage::User {
                id: uuid,
                parent_id: parent_uuid,
                timestamp,
                content: content.into_iter().map(to_view_content_block).collect(),
            }
        }
        ParsedMessage::Assistant { uuid, parent_uuid, timestamp, model, content, usage, stop_reason } => {
            ViewMessage::Assistant {
                id: uuid,
                parent_id: parent_uuid,
                timestamp,
                model,
                content: content.into_iter().map(to_view_content_block).collect(),
                usage: usage.map(to_view_usage),
                stop_reason,
            }
        }
        ParsedMessage::System { uuid, timestamp, subtype, content } => {
            ViewMessage::System {
                id: uuid,
                timestamp,
                subtype,
                content,
            }
        }
        ParsedMessage::Attachment { attachment_type } => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("attachment".to_string()),
                content: Some(attachment_type),
            }
        }
        ParsedMessage::PermissionMode { mode } => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("permissionMode".to_string()),
                content: Some(mode),
            }
        }
        ParsedMessage::FileHistorySnapshot => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("fileHistorySnapshot".to_string()),
                content: None,
            }
        }
    }
}

pub fn to_view_content_block(block: ContentBlock) -> ViewContentBlock {
    match block {
        ContentBlock::Text { text } => ViewContentBlock::Text { text },
        ContentBlock::Thinking { thinking, .. } => ViewContentBlock::Thinking { thinking },
        ContentBlock::ToolUse { id, name, input } => ViewContentBlock::ToolCall { id, name, input },
        ContentBlock::ToolResult { tool_use_id, content, is_error } => {
            ViewContentBlock::ToolResult {
                tool_call_id: tool_use_id,
                content,
                is_error,
            }
        }
        ContentBlock::Image { source } => {
            ViewContentBlock::Image {
                source: ViewImageSource {
                    source_type: source.source_type,
                    media_type: source.media_type,
                    data: source.data,
                },
            }
        }
    }
}

fn to_view_usage(usage: Usage) -> ViewUsage {
    ViewUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}
