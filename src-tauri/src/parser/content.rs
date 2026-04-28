use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "tool_use_id")]
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Image {
        source: ImageSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(default)]
    pub media_type: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
}

#[cfg(test)]
mod thinking_roundtrip {
    use super::*;

    #[test]
    fn deserialize_thinking_block_from_jsonl_shape() {
        let raw = r#"{"type":"thinking","thinking":"reasoning text","signature":"abc"}"#;
        let parsed: ContentBlock = serde_json::from_str(raw).expect("deserialize");
        match parsed {
            ContentBlock::Thinking { thinking, .. } => {
                assert_eq!(thinking, "reasoning text", "thinking text should be preserved");
            }
            _ => panic!("expected Thinking variant, got {:?}", parsed),
        }
    }

    #[test]
    fn deserialize_thinking_block_no_signature() {
        let raw = r#"{"type":"thinking","thinking":"reasoning"}"#;
        let parsed: ContentBlock = serde_json::from_str(raw).expect("deserialize");
        match parsed {
            ContentBlock::Thinking { thinking, signature } => {
                assert_eq!(thinking, "reasoning");
                assert!(signature.is_none());
            }
            _ => panic!("expected Thinking variant"),
        }
    }

    #[test]
    fn end_to_end_thinking_roundtrip_to_view_block() {
        use crate::claude::converter::to_view_content_block;
        let raw = r#"{"type":"thinking","thinking":"abc","signature":"sig"}"#;
        let parsed: ContentBlock = serde_json::from_str(raw).expect("deserialize");
        let view = to_view_content_block(parsed);
        let serialized = serde_json::to_string(&view).expect("serialize view");
        // What the frontend actually sees
        assert!(serialized.contains("\"thinking\":\"abc\""), "got: {}", serialized);
        assert!(serialized.contains("\"type\":\"thinking\""), "got: {}", serialized);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
}
