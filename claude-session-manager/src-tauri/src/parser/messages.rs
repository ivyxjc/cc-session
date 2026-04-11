use serde::{Deserialize, Serialize};
use super::content::{ContentBlock, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMessage {
    #[serde(rename = "type")]
    pub msg_type: String,

    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,

    // For user/assistant messages
    #[serde(default)]
    pub message: Option<serde_json::Value>,

    // For system messages
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub content: Option<String>,

    // For attachment messages
    #[serde(default)]
    pub attachment: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ParsedMessage {
    User {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        content: Vec<ContentBlock>,
    },
    Assistant {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        content: Vec<ContentBlock>,
        usage: Option<Usage>,
        stop_reason: Option<String>,
    },
    System {
        uuid: Option<String>,
        timestamp: Option<String>,
        subtype: Option<String>,
        content: Option<String>,
    },
    Attachment {
        attachment_type: String,
    },
    PermissionMode {
        mode: String,
    },
    FileHistorySnapshot,
}

impl ParsedMessage {
    pub fn from_raw(raw: &RawMessage) -> Option<Self> {
        match raw.msg_type.as_str() {
            "permission-mode" => {
                Some(ParsedMessage::PermissionMode {
                    mode: raw.permission_mode.clone().unwrap_or_default(),
                })
            }
            "file-history-snapshot" => {
                Some(ParsedMessage::FileHistorySnapshot)
            }
            "user" => {
                let content = Self::extract_content(Some(raw.message.as_ref()?));
                Some(ParsedMessage::User {
                    uuid: raw.uuid.clone().unwrap_or_default(),
                    parent_uuid: raw.parent_uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    content,
                })
            }
            "assistant" => {
                let msg = raw.message.as_ref()?;
                let content = Self::extract_content(Some(msg));
                let model = msg.get("model").and_then(|v| v.as_str()).map(String::from);
                let stop_reason = msg.get("stop_reason").and_then(|v| v.as_str()).map(String::from);
                let usage = msg.get("usage").and_then(|v| serde_json::from_value::<Usage>(v.clone()).ok());
                Some(ParsedMessage::Assistant {
                    uuid: raw.uuid.clone().unwrap_or_default(),
                    parent_uuid: raw.parent_uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    model,
                    content,
                    usage,
                    stop_reason,
                })
            }
            "system" => {
                Some(ParsedMessage::System {
                    uuid: raw.uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    subtype: raw.subtype.clone(),
                    content: raw.content.clone(),
                })
            }
            "attachment" => {
                let att_type = raw.attachment.as_ref()
                    .and_then(|a| a.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Some(ParsedMessage::Attachment {
                    attachment_type: att_type,
                })
            }
            _ => None,
        }
    }

    fn extract_content(msg: Option<&serde_json::Value>) -> Vec<ContentBlock> {
        let Some(msg) = msg else { return vec![] };

        // content can be a string or an array of content blocks
        if let Some(content) = msg.get("content") {
            if let Some(text) = content.as_str() {
                return vec![ContentBlock::Text { text: text.to_string() }];
            }
            if let Some(arr) = content.as_array() {
                return arr.iter()
                    .filter_map(|v| serde_json::from_value::<ContentBlock>(v.clone()).ok())
                    .collect();
            }
        }
        vec![]
    }
}
