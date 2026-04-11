mod db;
mod parser;
mod scanner;
mod commands;
mod backup;
mod monitor;

use db::Database;
use monitor::LiveMonitor;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("Failed to initialize database"));

    // Run initial scan
    let _ = scanner::scan_all(&database);

    let live_monitor = Arc::new(LiveMonitor::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(database)
        .manage(live_monitor)
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
            commands::backups::get_backup_messages,
            commands::backups::migrate_backups_cmd,
            commands::backups::get_backup_config_cmd,
            commands::backups::set_backup_config_cmd,
            commands::terminal::get_terminal_config,
            commands::terminal::set_terminal_config,
            commands::terminal::open_terminal,
            commands::terminal::test_terminal_command,
            commands::monitor::get_live_sessions,
            commands::monitor::start_live_monitor,
            commands::monitor::stop_live_monitor,
            commands::monitor::watch_session,
            commands::monitor::unwatch_session,
            commands::images::read_image_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
