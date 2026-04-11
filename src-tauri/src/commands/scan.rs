use crate::db::Database;
use crate::db::models::ScanResult;
use crate::scanner;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn refresh_index(db: State<'_, Arc<Database>>) -> Result<ScanResult, String> {
    scanner::scan_all(&db)
}
