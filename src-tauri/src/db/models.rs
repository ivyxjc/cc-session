use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub encoded_path: String,
    pub original_path: String,
    pub display_name: String,
    pub session_count: i64,
    pub last_active: Option<i64>,
    pub is_starred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: i64,
    pub session_id: String,
    pub project_id: i64,
    pub project_name: String,
    pub project_path: String,
    pub slug: Option<String>,
    pub version: Option<String>,
    pub permission_mode: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub last_active: Option<i64>,
    pub message_count: i64,
    pub user_msg_count: i64,
    pub assistant_msg_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub file_size: i64,
    pub is_favorited: bool,
    pub is_hidden: bool,
    pub is_backed_up: bool,
    pub copied_from_session_id: Option<String>,
    pub copied_at: Option<i64>,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: i64,
    pub session_id: i64,
    pub backup_path: String,
    pub backup_type: String,
    pub original_size: i64,
    pub compressed: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub id: i64,
    pub session_id: i64,
    pub agent_id: String,
    pub agent_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub enabled: bool,
    pub backup_dir: String,
    pub auto_backup: bool,
    pub auto_backup_interval_hours: u32,
    pub compress: bool,
    pub max_backup_copies: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        let default_dir = dirs::data_dir()
            .unwrap_or_default()
            .join("claude-session-manager")
            .join("backups");
        Self {
            enabled: true,
            backup_dir: default_dir.to_string_lossy().to_string(),
            auto_backup: true,
            auto_backup_interval_hours: 24,
            compress: true,
            max_backup_copies: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEntry {
    pub name: String,
    pub command: String, // use {path} as placeholder
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfig {
    pub terminals: Vec<TerminalEntry>,
    pub default_terminal: String, // name of the default
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            terminals: vec![
                TerminalEntry {
                    name: "Terminal".to_string(),
                    command: "open -a Terminal {path}".to_string(),
                },
                TerminalEntry {
                    name: "iTerm2".to_string(),
                    command: "open -a iTerm {path}".to_string(),
                },
                TerminalEntry {
                    name: "Warp".to_string(),
                    command: "open -a Warp {path}".to_string(),
                },
            ],
            default_terminal: "Terminal".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub projects_found: usize,
    pub sessions_found: usize,
    pub sessions_updated: usize,
    pub sessions_removed: usize,
    pub duration_ms: u64,
}
