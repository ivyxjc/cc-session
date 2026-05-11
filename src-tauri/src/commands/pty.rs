use crate::pty::{PtySession, PtyState};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Attach to (or create) a multiplexer session by name and stream it to the frontend.
/// `kind` is "zellij" or "tmux"; `name` is the existing session name.
/// `cwd` is used as the spawn directory (for `tmux new-session`-style fallbacks).
#[tauri::command]
pub fn pty_attach_multiplexer(
    app: AppHandle,
    state: State<'_, Arc<PtyState>>,
    kind: String,
    name: String,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let argv = match kind.as_str() {
        "tmux" => vec![
            "tmux".into(),
            "attach-session".into(),
            "-t".into(),
            name,
        ],
        "zellij" => vec!["zellij".into(), "attach".into(), name],
        other => return Err(format!("unsupported multiplexer: {}", other)),
    };
    let session = PtySession::spawn(app, argv, cwd, cols.max(20), rows.max(5))?;
    state.replace(session);
    Ok(())
}

/// Create a new multiplexer session in the given cwd and attach to it.
#[tauri::command]
pub fn pty_create_multiplexer(
    app: AppHandle,
    state: State<'_, Arc<PtyState>>,
    kind: String,
    name: String,
    cwd: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let argv = match kind.as_str() {
        // tmux -A: attach if exists, else create
        "tmux" => vec![
            "tmux".into(),
            "new-session".into(),
            "-A".into(),
            "-s".into(),
            name,
        ],
        // zellij --create: same idea
        "zellij" => vec![
            "zellij".into(),
            "attach".into(),
            "--create".into(),
            name,
        ],
        other => return Err(format!("unsupported multiplexer: {}", other)),
    };
    let session = PtySession::spawn(app, argv, cwd, cols.max(20), rows.max(5))?;
    state.replace(session);
    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<'_, Arc<PtyState>>, data: String) -> Result<(), String> {
    let bytes = B64.decode(data.as_bytes()).map_err(|e| format!("decode: {}", e))?;
    state.write(&bytes)
}

#[tauri::command]
pub fn pty_resize(state: State<'_, Arc<PtyState>>, cols: u16, rows: u16) -> Result<(), String> {
    state.resize(cols.max(20), rows.max(5))
}

#[tauri::command]
pub fn pty_detach(state: State<'_, Arc<PtyState>>) -> Result<(), String> {
    state.detach();
    Ok(())
}
