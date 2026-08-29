use crate::db::Database;
use crate::search::{search_messages, ContentSearchResult};
use crate::sources::Provider;
use std::sync::Arc;
use tauri::State;

/// `provider` of `None` searches every provider; the UI passes the active one
/// so content hits match the provider-scoped project/session lists beside them.
#[tauri::command]
pub fn search_message_content(
    db: State<'_, Arc<Database>>,
    query: String,
    provider: Option<Provider>,
    limit: Option<usize>,
) -> Result<Vec<ContentSearchResult>, String> {
    let conn = db.conn();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    search_messages(&conn, &query, provider, limit)
}
