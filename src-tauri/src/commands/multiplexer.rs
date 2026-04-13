use crate::db::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

// --- Config ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplexerConfig {
    /// "none" | "zellij" | "tmux"
    pub multiplexer: String,
}

impl Default for MultiplexerConfig {
    fn default() -> Self {
        Self {
            multiplexer: "none".to_string(),
        }
    }
}

#[tauri::command]
pub fn get_multiplexer_config(db: State<'_, Arc<Database>>) -> Result<MultiplexerConfig, String> {
    let conn = db.conn();
    let json: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'multiplexer_config'",
            [],
            |row| row.get(0),
        )
        .ok();
    Ok(json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn set_multiplexer_config(
    db: State<'_, Arc<Database>>,
    config: MultiplexerConfig,
) -> Result<(), String> {
    let conn = db.conn();
    let json = serde_json::to_string(&config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('multiplexer_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )
    .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

// --- Detection ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplexerSession {
    pub name: String,
    pub status: String, // "active" | "exited"
    pub cwd: Option<String>,
    pub matches_path: bool,
    pub attach_cmd: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplexerDetectionResult {
    pub multiplexer: String,
    pub sessions: Vec<MultiplexerSession>,
    pub new_session_cmd: String,
}

#[tauri::command]
pub fn detect_multiplexer_sessions(
    path: String,
    multiplexer: String,
) -> Result<MultiplexerDetectionResult, String> {
    match multiplexer.as_str() {
        "zellij" => detect_zellij(&path),
        "tmux" => detect_tmux(&path),
        _ => Err(format!("Unknown multiplexer: {}", multiplexer)),
    }
}

fn run_cmd(cmd: &str, args: &[&str], timeout_secs: u64) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // Wait with timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let mut output = String::new();
                    if let Some(mut stdout) = child.stdout.take() {
                        use std::io::Read;
                        stdout.read_to_string(&mut output).ok()?;
                    }
                    return Some(output);
                } else {
                    return None;
                }
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(timeout_secs) {
                    let _ = child.kill();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

fn shell_escape(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '\\' || c == '$') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

fn basename(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

// --- Zellij ---

fn find_binary(name: &str) -> Option<String> {
    // Try direct command first
    if Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        return Some(name.to_string());
    }
    // Search common paths
    let home = dirs::home_dir().unwrap_or_default();
    let candidates = [
        home.join(".cargo/bin").join(name),
        home.join(".local/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
        PathBuf::from("/opt/homebrew/bin").join(name),
    ];
    for p in &candidates {
        if p.exists() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

fn detect_zellij(project_path: &str) -> Result<MultiplexerDetectionResult, String> {
    let bin = find_binary("zellij").ok_or_else(|| "zellij not found".to_string())?;

    let output = run_cmd(&bin, &["list-sessions", "-n"], 3)
        .unwrap_or_default();

    let mut sessions = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse: "name [Created Xm ago] (status)"
        let name = line.split_whitespace().next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }

        let is_exited = line.contains("EXITED");
        let is_current = line.contains("(current)");
        let status = if is_exited {
            "exited"
        } else {
            "active"
        };

        // Try to get cwd for active sessions (not current, not exited)
        let cwd = if !is_exited && !is_current {
            get_zellij_cwd(&name)
        } else if is_current {
            // Current session — can still query
            get_zellij_cwd(&name)
        } else {
            None
        };

        let matches_path = cwd
            .as_ref()
            .map(|c| c.trim_end_matches('/') == project_path.trim_end_matches('/'))
            .unwrap_or(false);

        let attach_cmd = format!("zellij attach {}", shell_escape(&name));

        sessions.push(MultiplexerSession {
            name,
            status: status.to_string(),
            cwd,
            matches_path,
            attach_cmd,
        });
    }

    // Sort: matched first, then active, then exited
    sessions.sort_by(|a, b| {
        b.matches_path
            .cmp(&a.matches_path)
            .then_with(|| {
                let a_active = a.status == "active";
                let b_active = b.status == "active";
                b_active.cmp(&a_active)
            })
    });

    let escaped_path = shell_escape(project_path);
    let base = shell_escape(basename(project_path));
    let new_session_cmd = format!(
        "zellij -s {} options --default-cwd {}",
        base, escaped_path
    );

    Ok(MultiplexerDetectionResult {
        multiplexer: "zellij".to_string(),
        sessions,
        new_session_cmd,
    })
}

fn get_zellij_cwd(session_name: &str) -> Option<String> {
    let bin = find_binary("zellij")?;
    let output = run_cmd(
        &bin,
        &["-s", session_name, "action", "dump-layout"],
        2,
    )?;

    // Parse: layout { cwd "/path/to/project"
    for line in output.lines().take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with("cwd ") {
            let cwd = trimmed
                .trim_start_matches("cwd ")
                .trim_matches('"')
                .to_string();
            return Some(cwd);
        }
    }
    None
}

// --- tmux ---

fn detect_tmux(project_path: &str) -> Result<MultiplexerDetectionResult, String> {
    let bin = find_binary("tmux").ok_or_else(|| "tmux not found".to_string())?;

    let output = run_cmd(
        &bin,
        &[
            "list-sessions",
            "-F",
            "#{session_name}\t#{pane_current_path}",
        ],
        3,
    );

    let mut sessions = Vec::new();

    if let Some(out) = output {
        for line in out.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.is_empty() {
                continue;
            }
            let name = parts[0].to_string();
            let cwd = parts.get(1).map(|s| s.to_string());

            let matches_path = cwd
                .as_ref()
                .map(|c| c.trim_end_matches('/') == project_path.trim_end_matches('/'))
                .unwrap_or(false);

            let attach_cmd = format!("tmux attach -t {}", shell_escape(&name));

            sessions.push(MultiplexerSession {
                name,
                status: "active".to_string(),
                cwd,
                matches_path,
                attach_cmd,
            });
        }
    }

    sessions.sort_by(|a, b| b.matches_path.cmp(&a.matches_path));

    let escaped_path = shell_escape(project_path);
    let base = shell_escape(basename(project_path));
    let new_session_cmd = format!("tmux new-session -s {} -c {}", base, escaped_path);

    Ok(MultiplexerDetectionResult {
        multiplexer: "tmux".to_string(),
        sessions,
        new_session_cmd,
    })
}
