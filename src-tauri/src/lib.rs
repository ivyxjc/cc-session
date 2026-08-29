mod db;
mod parser;
mod activity;
mod scanner;
mod commands;
mod backup;
mod monitor;
mod models;
mod claude;
mod codex;
mod sources;
mod search;
mod ignore;
mod llm;
mod pty;

use db::Database;
use monitor::LiveMonitor;
use pty::PtyState;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("Failed to initialize database"));

    // Run initial scan
    let _ = scanner::scan_all(&database);

    let live_monitor = Arc::new(LiveMonitor::new());
    let pty_state = Arc::new(PtyState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(database)
        .manage(live_monitor)
        .manage(pty_state)
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::sessions::list_sessions,
            commands::sessions::get_messages,
            commands::sessions::get_latest_messages,
            commands::sessions::get_subagents,
            commands::sessions::get_subagent_messages,
            commands::scan::refresh_index,
            commands::favorites::toggle_favorite,
            commands::favorites::toggle_hide_session,
            commands::favorites::get_auto_hide_config,
            commands::favorites::set_auto_hide_config,
            commands::projects::toggle_star_project,
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
            commands::multiplexer::get_multiplexer_config,
            commands::multiplexer::set_multiplexer_config,
            commands::multiplexer::detect_multiplexer_sessions,
            commands::multiplexer::find_session_for_pid,
            commands::multiplexer::get_external_client_size,
            commands::settings_io::export_settings_to_file,
            commands::settings_io::import_settings_from_file,
            commands::copy_session::copy_session_to_path,
            commands::usage::get_daily_usage,
            commands::export::export_session,
            commands::search::search_message_content,
            commands::ignore::get_ignore_config,
            commands::ignore::set_ignore_config,
            commands::ai_summary::get_ai_summary_config,
            commands::ai_summary::set_ai_summary_config,
            commands::ai_summary::test_ai_summary_connection,
            commands::ai_summary::generate_ai_summary,
            commands::ai_summary::generate_ai_summaries_batch,
            commands::pty::pty_attach_multiplexer,
            commands::pty::pty_create_multiplexer,
            commands::pty::pty_write,
            commands::pty::pty_resize,
            commands::pty::pty_detach,
            commands::day_planner::get_day_planner,
            commands::day_planner::generate_daily_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
