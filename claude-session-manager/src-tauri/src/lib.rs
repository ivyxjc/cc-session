mod db;
mod parser;
mod scanner;
mod commands;
mod backup;

use db::Database;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("Failed to initialize database"));

    // Run initial scan
    let _ = scanner::scan_all(&database);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(database)
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::sessions::list_sessions,
            commands::sessions::get_messages,
            commands::sessions::get_subagents,
            commands::sessions::get_subagent_messages,
            commands::scan::refresh_index,
            commands::favorites::toggle_favorite,
            commands::tags::create_tag,
            commands::tags::delete_tag,
            commands::tags::list_tags,
            commands::tags::tag_session,
            commands::tags::untag_session,
            commands::backups::backup_session,
            commands::backups::backup_all_sessions,
            commands::backups::restore_session_backup,
            commands::backups::list_backups,
            commands::backups::delete_backup,
            commands::backups::get_backup_config_cmd,
            commands::backups::set_backup_config_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
