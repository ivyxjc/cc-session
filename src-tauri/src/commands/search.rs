use crate::db::Database;
use crate::search::{search_messages, ContentSearchResult};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn search_message_content(
    db: State<'_, Arc<Database>>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ContentSearchResult>, String> {
    let conn = db.conn();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    search_messages(&conn, &query, limit)
}
