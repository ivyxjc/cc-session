use base64::Engine;
use std::path::Path;

#[tauri::command]
pub fn read_image_file(path: String) -> Result<String, String> {
    let p = Path::new(&path);

    // Security: only allow reading from ~/.claude/image-cache/
    let claude_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("image-cache");
    let canonical = p
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;
    if !canonical.starts_with(&claude_dir) {
        return Err("Access denied: path outside image cache".to_string());
    }

    let data =
        std::fs::read(&canonical).map_err(|e| format!("Failed to read image: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&data))
}
