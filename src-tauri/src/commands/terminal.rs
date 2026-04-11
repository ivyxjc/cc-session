use crate::db::Database;
use crate::db::models::TerminalConfig;
use rusqlite::params;
use std::process::Command;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_terminal_config(db: State<'_, Arc<Database>>) -> Result<TerminalConfig, String> {
    let conn = db.conn();
    let json: Option<String> = conn.query_row(
        "SELECT value FROM app_config WHERE key = 'terminal_config'",
        [],
        |row| row.get(0),
    ).ok();

    Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
}

#[tauri::command]
pub fn set_terminal_config(
    db: State<'_, Arc<Database>>,
    config: TerminalConfig,
) -> Result<(), String> {
    let conn = db.conn();
    let json = serde_json::to_string(&config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('terminal_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn open_terminal(
    db: State<'_, Arc<Database>>,
    path: String,
    terminal_name: Option<String>,
) -> Result<(), String> {
    let config = get_terminal_config_inner(&db)?;

    let entry = if let Some(ref name) = terminal_name {
        // Explicit selection from dropdown
        config.terminals.iter()
            .find(|t| t.name == *name)
            .or_else(|| config.terminals.first())
    } else {
        // Default: always use first terminal in list
        config.terminals.first()
    }.ok_or_else(|| "No terminals configured".to_string())?;

    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let cmd = entry.command.replace("{path}", &path);

    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Terminal command failed (exit {}): {}", output.status.code().unwrap_or(-1), stderr.trim()));
    }

    Ok(())
}

#[tauri::command]
pub fn test_terminal_command(command: String) -> Result<(), String> {
    let home = dirs::home_dir().unwrap_or_default();
    let path = home.to_string_lossy().to_string();
    let cmd = command.replace("{path}", &path);

    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Command failed (exit {}): {}", output.status.code().unwrap_or(-1), stderr.trim()));
    }

    Ok(())
}

fn get_terminal_config_inner(db: &Arc<Database>) -> Result<TerminalConfig, String> {
    let conn = db.conn();
    let json: Option<String> = conn.query_row(
        "SELECT value FROM app_config WHERE key = 'terminal_config'",
        [],
        |row| row.get(0),
    ).ok();

    let mut config: TerminalConfig = json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();

    // Ensure there's always at least one terminal
    if config.terminals.is_empty() {
        config = TerminalConfig::default();
    }

    Ok(config)
}
