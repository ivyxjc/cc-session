use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        /// Reasoning tokens reported for the turn this block belongs to.
        /// Codex only — Claude does not break thinking out of `output_tokens`.
        #[serde(rename = "reasoningTokens", skip_serializing_if = "Option::is_none")]
        reasoning_tokens: Option<i64>,
        /// True when several thinking blocks in the same turn share `reasoning_tokens`.
        #[serde(rename = "tokensShared", default)]
        tokens_shared: bool,
    },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        content: serde_json::Value,
        #[serde(default)]
        #[serde(rename = "isError")]
        is_error: bool,
    },
    Image {
        source: ViewImageSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewImageSource {
    pub source_type: String,
    pub media_type: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewUsage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
}
