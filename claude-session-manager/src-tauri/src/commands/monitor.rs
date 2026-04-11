use crate::db::Database;
use crate::monitor::LiveMonitor;
use crate::monitor::LiveSession;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_live_sessions(
    db: State<'_, Arc<Database>>,
    monitor: State<'_, Arc<LiveMonitor>>,
) -> Result<Vec<LiveSession>, String> {
    Ok(monitor.get_live_sessions(&db))
}

#[tauri::command]
pub fn start_live_monitor(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    monitor: State<'_, Arc<LiveMonitor>>,
) -> Result<(), String> {
    monitor.start(app, db.inner().clone());
    Ok(())
}

#[tauri::command]
pub fn stop_live_monitor(
    monitor: State<'_, Arc<LiveMonitor>>,
) -> Result<(), String> {
    monitor.stop();
    Ok(())
}

#[tauri::command]
pub fn watch_session(
    app: tauri::AppHandle,
    db: State<'_, Arc<Database>>,
    monitor: State<'_, Arc<LiveMonitor>>,
    session_id: String,
) -> Result<(), String> {
    monitor.watch_session(app, &db, session_id)
}

#[tauri::command]
pub fn unwatch_session(
    monitor: State<'_, Arc<LiveMonitor>>,
    session_id: String,
) -> Result<(), String> {
    monitor.unwatch_session(&session_id);
    Ok(())
}
