use crate::commands::multiplexer::find_binary;
use crate::pty::{PtyError, PtyResult, PtySession, PtyState};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// Resolve the multiplexer binary to an invocable path. A GUI-launched app
/// gets a minimal PATH (no homebrew/cargo dirs), so spawning a bare "zellij"
/// or "tmux" can fail even though detection found the binary via fallbacks.
fn resolve_bin(kind: &str) -> PtyResult<String> {
    find_binary(kind).ok_or_else(|| PtyError::Generic(format!("{} not found", kind)))
}

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
) -> PtyResult<()> {
    let argv = match kind.as_str() {
        "tmux" => vec![
            resolve_bin("tmux")?,
            "attach-session".into(),
            "-t".into(),
            name,
        ],
        "zellij" => vec![resolve_bin("zellij")?, "attach".into(), name],
        other => return Err(PtyError::Generic(format!("unsupported multiplexer: {}", other))),
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
) -> PtyResult<()> {
    let argv = match kind.as_str() {
        // tmux -A: attach if exists, else create
        "tmux" => vec![
            resolve_bin("tmux")?,
            "new-session".into(),
            "-A".into(),
            "-s".into(),
            name,
        ],
        // zellij --create: same idea
        "zellij" => vec![
            resolve_bin("zellij")?,
            "attach".into(),
            "--create".into(),
            name,
        ],
        other => return Err(PtyError::Generic(format!("unsupported multiplexer: {}", other))),
    };
    let session = PtySession::spawn(app, argv, cwd, cols.max(20), rows.max(5))?;
    state.replace(session);
    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<'_, Arc<PtyState>>, data: String) -> PtyResult<()> {
    let bytes = B64
        .decode(data.as_bytes())
        .map_err(|e| PtyError::Generic(format!("base64 decode: {}", e)))?;
    state.write(&bytes)
}

#[tauri::command]
pub fn pty_resize(state: State<'_, Arc<PtyState>>, cols: u16, rows: u16) -> PtyResult<()> {
    state.resize(cols.max(20), rows.max(5))
}

#[tauri::command]
pub fn pty_detach(state: State<'_, Arc<PtyState>>) -> PtyResult<()> {
    state.detach();
    Ok(())
}
