use super::converter::to_view_message;
use super::db::{self, CodexProject, CodexSession, CodexSubagent};
use crate::models::ViewMessage;
use crate::parser::ViewLatestMessagesResult;
use std::path::Path;

#[tauri::command]
pub fn codex_get_session(thread_id: String) -> Result<CodexSession, String> {
    db::get_session(&thread_id)
}

#[tauri::command]
pub fn codex_list_projects(sort_by: Option<String>) -> Result<Vec<CodexProject>, String> {
    db::list_projects(sort_by.as_deref())
}

#[tauri::command]
pub fn codex_list_sessions(
    cwd: Option<String>,
    sort_by: Option<String>,
    show_archived: Option<bool>,
) -> Result<Vec<CodexSession>, String> {
    db::list_sessions(cwd.as_deref(), sort_by.as_deref(), show_archived)
}

#[tauri::command]
pub fn codex_get_messages(
    thread_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let rollout_path = db::get_thread_rollout_path(&thread_id)?;
    let path = Path::new(&rollout_path);
    if !path.exists() {
        return Err(format!("Codex session file not found: {}", rollout_path));
    }
    let items = super::parser::load_messages(path, offset.unwrap_or(0), limit.unwrap_or(50))?;
    Ok(items.into_iter().map(to_view_message).collect())
}

#[tauri::command]
pub fn codex_get_latest_messages(
    thread_id: String,
    count: Option<usize>,
) -> Result<ViewLatestMessagesResult, String> {
    let rollout_path = db::get_thread_rollout_path(&thread_id)?;
    let path = Path::new(&rollout_path);
    if !path.exists() {
        return Err(format!("Codex session file not found: {}", rollout_path));
    }
    let (items, total_count) = super::parser::load_latest_messages(path, count.unwrap_or(50))?;
    Ok(ViewLatestMessagesResult {
        messages: items.into_iter().map(to_view_message).collect(),
        total_count,
    })
}

#[tauri::command]
pub fn codex_get_subagents(thread_id: String) -> Result<Vec<CodexSubagent>, String> {
    db::get_subagents(&thread_id)
}

#[tauri::command]
pub fn codex_get_subagent_messages(
    thread_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let rollout_path = db::get_thread_rollout_path(&thread_id)?;
    let path = Path::new(&rollout_path);
    if !path.exists() {
        return Err(format!("Codex session file not found: {}", rollout_path));
    }
    let items = super::parser::load_messages(path, offset.unwrap_or(0), limit.unwrap_or(200))?;
    Ok(items.into_iter().map(to_view_message).collect())
}
