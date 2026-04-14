use serde::{Deserialize, Serialize};
use super::content::{ViewContentBlock, ViewUsage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewMessage {
    User {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        timestamp: Option<String>,
        content: Vec<ViewContentBlock>,
    },
    Assistant {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        content: Vec<ViewContentBlock>,
        usage: Option<ViewUsage>,
        #[serde(rename = "stopReason")]
        stop_reason: Option<String>,
    },
    System {
        id: Option<String>,
        timestamp: Option<String>,
        subtype: Option<String>,
        content: Option<String>,
    },
}
