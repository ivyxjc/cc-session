use crate::db::Database;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;
use std::path::Path;

/// Export all app_config entries as a JSON object
#[tauri::command]
pub fn export_settings(db: State<'_, Arc<Database>>) -> Result<String, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_config")
        .map_err(|e| format!("DB error: {}", e))?;

    let entries: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut map = serde_json::Map::new();
    map.insert(
        "version".to_string(),
        serde_json::Value::Number(1.into()),
    );

    let mut settings = serde_json::Map::new();
    for (key, value) in entries {
        // Try to parse value as JSON, fallback to string
        let json_val = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
        settings.insert(key, json_val);
    }
    map.insert("settings".to_string(), serde_json::Value::Object(settings));

    serde_json::to_string_pretty(&map).map_err(|e| format!("Serialize error: {}", e))
}

/// Import settings from a JSON string, overwriting existing app_config entries
#[tauri::command]
pub fn import_settings(db: State<'_, Arc<Database>>, json: String) -> Result<(), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("Invalid JSON: {}", e))?;

    let settings = parsed
        .get("settings")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing 'settings' object in config file".to_string())?;

    let conn = db.conn();
    for (key, value) in settings {
        let value_str = if value.is_string() {
            value.as_str().unwrap().to_string()
        } else {
            serde_json::to_string(value).map_err(|e| format!("Serialize error: {}", e))?
        };

        conn.execute(
            "INSERT INTO app_config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value_str],
        )
        .map_err(|e| format!("DB error: {}", e))?;
    }

    Ok(())
}

/// Export settings to a file
#[tauri::command]
pub fn export_settings_to_file(db: State<'_, Arc<Database>>, path: String) -> Result<(), String> {
    let json = export_settings_json(&db)?;
    std::fs::write(Path::new(&path), json).map_err(|e| format!("Write error: {}", e))
}

/// Import settings from a file
#[tauri::command]
pub fn import_settings_from_file(db: State<'_, Arc<Database>>, path: String) -> Result<(), String> {
    let json = std::fs::read_to_string(Path::new(&path)).map_err(|e| format!("Read error: {}", e))?;
    import_settings_json(&db, &json)
}

fn export_settings_json(db: &Arc<Database>) -> Result<String, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare("SELECT key, value FROM app_config")
        .map_err(|e| format!("DB error: {}", e))?;

    let entries: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    let mut map = serde_json::Map::new();
    map.insert("version".to_string(), serde_json::Value::Number(1.into()));

    let mut settings = serde_json::Map::new();
    for (key, value) in entries {
        let json_val = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
        settings.insert(key, json_val);
    }
    map.insert("settings".to_string(), serde_json::Value::Object(settings));

    serde_json::to_string_pretty(&map).map_err(|e| format!("Serialize error: {}", e))
}

fn import_settings_json(db: &Arc<Database>, json: &str) -> Result<(), String> {
    let parsed: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("Invalid JSON: {}", e))?;

    let settings = parsed
        .get("settings")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing 'settings' object in config file".to_string())?;

    let conn = db.conn();
    for (key, value) in settings {
        let value_str = if value.is_string() {
            value.as_str().unwrap().to_string()
        } else {
            serde_json::to_string(value).map_err(|e| format!("Serialize error: {}", e))?
        };

        conn.execute(
            "INSERT INTO app_config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value_str],
        )
        .map_err(|e| format!("DB error: {}", e))?;
    }

    Ok(())
}
