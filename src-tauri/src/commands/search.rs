use crate::db::Database;
use crate::search::{search_messages, ContentSearchResult};
use crate::sources::Provider;
use std::sync::Arc;
use tauri::State;

/// `provider` of `None` searches every provider; the UI passes the active one
/// so content hits match the provider-scoped project/session lists beside them.
/// `path_prefix` of `None` searches every project.
#[tauri::command]
pub fn search_message_content(
    db: State<'_, Arc<Database>>,
    query: String,
    provider: Option<Provider>,
    path_prefix: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ContentSearchResult>, String> {
    let conn = db.conn();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    search_messages(&conn, &query, provider, path_prefix.as_deref(), limit)
}
