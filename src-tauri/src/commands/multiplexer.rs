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

    // Drain stdout on a separate thread while we poll for exit — if we only
    // read after exit, a child producing more than the OS pipe buffer (~64KB)
    // blocks forever on write and we'd kill it at the timeout.
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });

    // Wait with timeout
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = reader.join().ok()?;
                return if status.success() { Some(output) } else { None };
            }
            Ok(None) => {
                if start.elapsed() > Duration::from_secs(timeout_secs) {
                    let _ = child.kill();
                    let _ = child.wait(); // reap, don't leave a zombie
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
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

/// Resolve a multiplexer binary to an invocable path. Successful lookups are
/// cached for the app's lifetime — probing spawns a `--version` subprocess,
/// and detection used to re-probe once per session. Misses are not cached so
/// installing the binary while the app runs is picked up.
pub fn find_binary(name: &str) -> Option<String> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, String>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(name) {
        return Some(hit.clone());
    }
    let resolved = find_binary_uncached(name)?;
    cache
        .lock()
        .unwrap()
        .insert(name.to_string(), resolved.clone());
    Some(resolved)
}

fn find_binary_uncached(name: &str) -> Option<String> {
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

    // First pass: collect raw entries (cheap, no subprocess).
    struct Entry {
        name: String,
        is_exited: bool,
        age_secs: u64, // newer (smaller age) first; u64::MAX if unparseable
    }
    let mut entries: Vec<Entry> = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let name = line.split_whitespace().next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        entries.push(Entry {
            name,
            is_exited: line.contains("EXITED"),
            age_secs: parse_zellij_age_seconds(line).unwrap_or(u64::MAX),
        });
    }

    // Second pass: parallel `zellij action dump-layout` per active session to fetch cwd.
    // Sequential blew up the UX (N × 1-2s); thread::scope keeps it under 1.5s.
    let cwds: Vec<Option<String>> = std::thread::scope(|s| {
        let handles: Vec<_> = entries
            .iter()
            .map(|e| {
                if e.is_exited {
                    None
                } else {
                    let name = e.name.clone();
                    let bin = bin.clone();
                    Some(s.spawn(move || get_zellij_cwd(&bin, &name)))
                }
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.and_then(|h| h.join().ok()).flatten())
            .collect()
    });

    // Build (age, session) pairs so we can sort by recency as a tertiary key.
    let mut pairs: Vec<(u64, MultiplexerSession)> = Vec::new();
    for (i, e) in entries.into_iter().enumerate() {
        let status = if e.is_exited { "exited" } else { "active" };
        let cwd = cwds.get(i).cloned().flatten();
        let matches_path = cwd
            .as_ref()
            .map(|c| c.trim_end_matches('/') == project_path.trim_end_matches('/'))
            .unwrap_or(false);
        let attach_cmd = format!("zellij attach {}", shell_escape(&e.name));
        pairs.push((
            e.age_secs,
            MultiplexerSession {
                name: e.name,
                status: status.to_string(),
                cwd,
                matches_path,
                attach_cmd,
            },
        ));
    }

    // Sort: cwd-matched first → active before exited → newest first (smallest age_secs).
    pairs.sort_by(|(a_age, a), (b_age, b)| {
        b.matches_path
            .cmp(&a.matches_path)
            .then_with(|| {
                let a_active = a.status == "active";
                let b_active = b.status == "active";
                b_active.cmp(&a_active)
            })
            .then_with(|| a_age.cmp(b_age))
    });
    let sessions: Vec<MultiplexerSession> = pairs.into_iter().map(|(_, s)| s).collect();

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

fn get_zellij_cwd(bin: &str, session_name: &str) -> Option<String> {
    let output = run_cmd(
        bin,
        &["-s", session_name, "action", "dump-layout"],
        1,
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

/// Parse zellij's `[Created Xd Yh Zm Ws]` segment into seconds, for "newest first" sorting.
fn parse_zellij_age_seconds(line: &str) -> Option<u64> {
    let marker = "[Created ";
    let start = line.find(marker)? + marker.len();
    let rel_end = line[start..].find(']')?;
    let body = &line[start..start + rel_end];

    let mut total: u64 = 0;
    for part in body.split_whitespace() {
        if part.len() < 2 {
            return None;
        }
        let (num_part, unit) = part.split_at(part.len() - 1);
        let n: u64 = num_part.parse().ok()?;
        match unit {
            "d" => total = total.saturating_add(n.saturating_mul(86400)),
            "h" => total = total.saturating_add(n.saturating_mul(3600)),
            "m" => total = total.saturating_add(n.saturating_mul(60)),
            "s" => total = total.saturating_add(n),
            _ => return None,
        }
    }
    Some(total)
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

// --- Precise per-PID session lookup ---
//
// Given a PID (e.g. the live Claude Code process), find the multiplexer session it
// is running inside by reading the process's environment:
//   - zellij: ZELLIJ_SESSION_NAME directly names the session
//   - tmux:   TMUX_PANE gives a pane id, then `tmux display-message` resolves it to a session
//
// This is more accurate than the cwd heuristic since it follows the actual process tree.

use std::collections::HashMap;

fn read_pid_env(pid: u32) -> HashMap<String, String> {
    let mut map = HashMap::new();

    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read(format!("/proc/{}/environ", pid)) {
            for entry in content.split(|&b| b == 0) {
                if let Ok(s) = std::str::from_utf8(entry) {
                    if let Some((k, v)) = s.split_once('=') {
                        map.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // sysctl KERN_PROCARGS2 → returns argc + exec_path + argv + envp
        let mut mib: [libc::c_int; 3] = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
        let mut size: libc::size_t = 0;
        let ret = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if ret != 0 || size == 0 {
            return map;
        }
        let mut buf: Vec<u8> = vec![0u8; size];
        let ret = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                buf.as_mut_ptr() as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if ret != 0 {
            return map;
        }
        buf.truncate(size);
        if buf.len() < 4 {
            return map;
        }

        // Layout:
        //   int argc;          // 4 bytes (native endian)
        //   char exec_path[];  // null-terminated
        //   char padding[];    // null bytes until argv starts (8-byte aligned)
        //   char argv[argc][]; // each null-terminated
        //   char envp[][];     // each null-terminated, ends at buf end
        let argc = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let mut idx = 4usize;

        // Skip exec_path
        while idx < buf.len() && buf[idx] != 0 {
            idx += 1;
        }
        // Skip padding nulls
        while idx < buf.len() && buf[idx] == 0 {
            idx += 1;
        }

        // Skip argc args
        let mut count = 0;
        while count < argc && idx < buf.len() {
            while idx < buf.len() && buf[idx] != 0 {
                idx += 1;
            }
            idx += 1;
            count += 1;
        }

        // Parse envp
        while idx < buf.len() {
            let start = idx;
            while idx < buf.len() && buf[idx] != 0 {
                idx += 1;
            }
            if start == idx {
                break;
            }
            if let Ok(s) = std::str::from_utf8(&buf[start..idx]) {
                if let Some((k, v)) = s.split_once('=') {
                    map.insert(k.to_string(), v.to_string());
                }
            }
            idx += 1;
        }
    }

    let _ = pid; // suppress unused warning on platforms without an impl
    map
}

#[tauri::command]
pub fn find_session_for_pid(
    pid: u32,
    multiplexer: String,
) -> Result<Option<String>, String> {
    match multiplexer.as_str() {
        "zellij" => {
            let env = read_pid_env(pid);
            Ok(env.get("ZELLIJ_SESSION_NAME").cloned().filter(|s| !s.is_empty()))
        }
        "tmux" => {
            let env = read_pid_env(pid);
            let Some(pane) = env.get("TMUX_PANE").filter(|s| !s.is_empty()) else {
                return Ok(None);
            };
            let bin = match find_binary("tmux") {
                Some(b) => b,
                None => return Ok(None),
            };
            let output = Command::new(&bin)
                .args(["display-message", "-p", "-t", pane, "#{session_name}"])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    Ok(if name.is_empty() { None } else { Some(name) })
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}
