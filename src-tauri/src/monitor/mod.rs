use crate::db::Database;
use crate::parser::messages::{ParsedMessage, RawMessage};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// --- Data types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_at: i64,
    pub kind: String,
    pub entrypoint: String,
    pub is_alive: bool,
    pub ended_at: Option<i64>,
    // Enrichment from DB
    pub db_session_id: Option<i64>,
    pub slug: Option<String>,
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub message_count: Option<i64>,
    pub user_msg_count: Option<i64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    pub total_cache_creation_tokens: Option<i64>,
    pub total_cache_read_tokens: Option<i64>,
    pub version: Option<String>,
    pub file_size: Option<i64>,
    pub last_message_preview: Option<String>,
    pub active_subagent_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagesUpdate {
    pub session_id: String,
    pub new_messages: Vec<ParsedMessage>,
}

// --- Raw session file from ~/.claude/sessions/{pid}.json ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionRegistryEntry {
    pid: u32,
    session_id: String,
    cwd: String,
    started_at: i64,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_entrypoint")]
    entrypoint: String,
}

fn default_kind() -> String {
    "interactive".to_string()
}
fn default_entrypoint() -> String {
    "cli".to_string()
}

// --- PID liveness check ---

/// Check if a PID is alive. Handles EPERM (process exists but owned by another user).
fn is_pid_alive(pid: u32) -> bool {
    if pid > i32::MAX as u32 {
        return false;
    }
    let ret = unsafe { libc::kill(pid as i32, 0) };
    if ret == 0 {
        return true;
    }
    // EPERM means the process exists but we lack permission to signal it
    let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    errno == libc::EPERM
}

// --- Monitor state ---

struct EndedSession {
    live: LiveSession,
    ended_at: Instant,
}

pub struct LiveMonitor {
    running: Arc<AtomicBool>,
    poll_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    watchers: Arc<Mutex<HashMap<String, SessionWatcher>>>,
    ended_sessions: Arc<Mutex<HashMap<u32, EndedSession>>>,
}

struct SessionWatcher {
    _watcher: RecommendedWatcher,
    #[allow(dead_code)]
    offset: Arc<Mutex<u64>>,
    // Signal to stop the fallback thread
    active: Arc<AtomicBool>,
}

impl LiveMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            poll_handle: Mutex::new(None),
            watchers: Arc::new(Mutex::new(HashMap::new())),
            ended_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// One-shot fetch of all live sessions
    pub fn get_live_sessions(&self, db: &Database) -> Vec<LiveSession> {
        let mut sessions = scan_session_registry(db);
        if let Ok(ended) = self.ended_sessions.lock() {
            for es in ended.values() {
                if !sessions.iter().any(|s| s.pid == es.live.pid) {
                    sessions.push(es.live.clone());
                }
            }
        }
        sessions
    }

    /// Start the 10s polling loop. Non-blocking — spawns a background thread.
    pub fn start(&self, app: AppHandle, db: Arc<Database>) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let ended_sessions = self.ended_sessions.clone();

        let handle = std::thread::spawn(move || {
            let mut prev_snapshot: Vec<LiveSession> = Vec::new();

            while running.load(Ordering::SeqCst) {
                let mut sessions = scan_session_registry(&db);

                // Track newly ended sessions
                if let Ok(mut ended) = ended_sessions.lock() {
                    for prev in &prev_snapshot {
                        if prev.is_alive
                            && !sessions.iter().any(|s| s.pid == prev.pid && s.is_alive)
                        {
                            let mut ended_live = prev.clone();
                            ended_live.is_alive = false;
                            ended_live.ended_at =
                                Some(chrono::Utc::now().timestamp_millis());
                            ended.insert(
                                prev.pid,
                                EndedSession {
                                    live: ended_live,
                                    ended_at: Instant::now(),
                                },
                            );
                        }
                    }

                    // Purge entries older than 5 minutes
                    ended.retain(|_, es| es.ended_at.elapsed() < Duration::from_secs(300));

                    // Add ended sessions to the output
                    for es in ended.values() {
                        if !sessions.iter().any(|s| s.pid == es.live.pid) {
                            sessions.push(es.live.clone());
                        }
                    }
                }

                let _ = app.emit("live-sessions-update", &sessions);
                prev_snapshot = sessions;

                // Sleep in small increments so we can stop quickly
                for _ in 0..100 {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        });

        if let Ok(mut h) = self.poll_handle.lock() {
            *h = Some(handle);
        }
    }

    /// Stop polling. Non-blocking — sets the flag and lets the thread exit on its own.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        // Don't join the poll thread here to avoid blocking the Tauri command thread.
        // The thread will exit within ~100ms after seeing the flag.
        if let Ok(mut h) = self.poll_handle.lock() {
            h.take(); // Detach the handle
        }
        // Stop all session watchers
        if let Ok(mut w) = self.watchers.lock() {
            for (_, watcher) in w.drain() {
                watcher.active.store(false, Ordering::SeqCst);
            }
        }
    }

    /// Start watching a specific session's JSONL for new messages
    pub fn watch_session(
        &self,
        app: AppHandle,
        db: &Database,
        session_id: String,
    ) -> Result<(), String> {
        // Already watching? Skip.
        if let Ok(w) = self.watchers.lock() {
            if w.contains_key(&session_id) {
                return Ok(());
            }
        }

        // Find the JSONL path from DB
        let jsonl_path: String = {
            let conn = db.conn();
            conn.query_row(
                "SELECT jsonl_path FROM sessions WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Session not found: {}", e))?
        }; // conn (MutexGuard) dropped here

        let path = PathBuf::from(&jsonl_path);
        if !path.exists() {
            return Err(format!("JSONL file not found: {}", jsonl_path));
        }

        // Get current file size as initial offset
        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let offset = Arc::new(Mutex::new(file_size));
        let active = Arc::new(AtomicBool::new(true));

        // Set up fs-notify watcher
        let watch_path = path.clone();
        let watch_session_id = session_id.clone();
        let watch_offset = offset.clone();
        let watch_app = app.clone();

        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        let new_messages = read_new_lines(&watch_path, &watch_offset);
                        if !new_messages.is_empty() {
                            let update = SessionMessagesUpdate {
                                session_id: watch_session_id.clone(),
                                new_messages,
                            };
                            let _ = watch_app.emit("session-messages-update", &update);
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to create watcher: {}", e))?;

        watcher
            .watch(path.as_ref(), RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch file: {}", e))?;

        // 30s fallback check thread — uses AtomicBool to know when to stop
        let fallback_path = path.clone();
        let fallback_offset = offset.clone();
        let fallback_session_id = session_id.clone();
        let fallback_app = app.clone();
        let fallback_active = active.clone();

        std::thread::spawn(move || {
            loop {
                // Sleep in small increments so we can stop quickly
                for _ in 0..30 {
                    if !fallback_active.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                if !fallback_active.load(Ordering::SeqCst) {
                    return;
                }
                let new_messages = read_new_lines(&fallback_path, &fallback_offset);
                if !new_messages.is_empty() {
                    let update = SessionMessagesUpdate {
                        session_id: fallback_session_id.clone(),
                        new_messages,
                    };
                    let _ = fallback_app.emit("session-messages-update", &update);
                }
            }
        });

        if let Ok(mut w) = self.watchers.lock() {
            w.insert(
                session_id,
                SessionWatcher {
                    _watcher: watcher,
                    offset,
                    active,
                },
            );
        }

        Ok(())
    }

    /// Stop watching a session
    pub fn unwatch_session(&self, session_id: &str) {
        if let Ok(mut w) = self.watchers.lock() {
            if let Some(watcher) = w.remove(session_id) {
                watcher.active.store(false, Ordering::SeqCst);
            }
        }
    }
}

impl Drop for LiveMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut w) = self.watchers.lock() {
            for (_, watcher) in w.drain() {
                watcher.active.store(false, Ordering::SeqCst);
            }
        }
    }
}

/// Read new lines from a JSONL file starting at the given offset.
/// Only advances the offset past complete lines to avoid losing partial writes.
fn read_new_lines(path: &Path, offset: &Mutex<u64>) -> Vec<ParsedMessage> {
    let mut messages = Vec::new();
    let Ok(mut current_offset) = offset.lock() else {
        return messages;
    };

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return messages,
    };

    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    if file_size <= *current_offset {
        return messages;
    }

    if file.seek(SeekFrom::Start(*current_offset)).is_err() {
        return messages;
    }

    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return messages;
    }

    // Only process complete lines (ending with \n).
    // Track how many bytes we actually consumed so partial lines are retried next time.
    let mut bytes_consumed: u64 = 0;

    let reader = BufReader::new(buf.as_slice());
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // Stop on read error (likely partial line at EOF)
        };

        // Account for the line content + newline byte
        bytes_consumed += line.len() as u64 + 1;

        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => {
                // If JSON parse fails, this could be a partial line at the end.
                // Don't advance offset past it — revert the consumed count.
                bytes_consumed -= line.len() as u64 + 1;
                break;
            }
        };

        if matches!(raw.msg_type.as_str(), "user" | "assistant" | "system") {
            if let Some(parsed) = ParsedMessage::from_raw(&raw) {
                messages.push(parsed);
            }
        }
    }

    *current_offset += bytes_consumed;
    messages
}

/// Intermediate struct for collecting registry entries before DB enrichment.
struct RegistryInfo {
    pid: u32,
    session_id: String,
    cwd: String,
    started_at: i64,
    kind: String,
    entrypoint: String,
    is_alive: bool,
}

/// Scan ~/.claude/sessions/ for active session registry files.
/// Separates filesystem I/O from DB access to minimize lock contention.
fn scan_session_registry(db: &Database) -> Vec<LiveSession> {
    let sessions_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("sessions");

    if !sessions_dir.exists() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    // Phase 1: Filesystem scan + PID check (no DB lock needed)
    let mut registry_entries: Vec<RegistryInfo> = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let registry: SessionRegistryEntry = match serde_json::from_str(&content) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let is_alive = is_pid_alive(registry.pid);
            if !is_alive {
                continue;
            }

            registry_entries.push(RegistryInfo {
                pid: registry.pid,
                session_id: registry.session_id,
                cwd: registry.cwd,
                started_at: registry.started_at,
                kind: registry.kind,
                entrypoint: registry.entrypoint,
                is_alive,
            });
        }
    }

    // Phase 2: DB enrichment (acquire lock only for queries)
    let conn = db.conn();
    let mut results = Vec::new();

    for info in registry_entries {
        let enrichment = conn
            .query_row(
                "SELECT s.id, s.slug, p.display_name, s.git_branch,
                        s.message_count, s.user_msg_count, s.total_input_tokens, s.total_output_tokens,
                        s.total_cache_creation_tokens, s.total_cache_read_tokens,
                        s.version, s.file_size
                 FROM sessions s
                 JOIN projects p ON s.project_id = p.id
                 WHERE s.session_id = ?1",
                params![info.session_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, i64>(11)?,
                    ))
                },
            )
            .ok();

        let subagent_count = enrichment.as_ref().and_then(|e| {
            conn.query_row(
                "SELECT COUNT(*) FROM subagents WHERE session_id = ?1",
                params![e.0],
                |row| row.get::<_, i64>(0),
            )
            .ok()
        });

        results.push(LiveSession {
            pid: info.pid,
            session_id: info.session_id,
            cwd: info.cwd,
            started_at: info.started_at,
            kind: info.kind,
            entrypoint: info.entrypoint,
            is_alive: info.is_alive,
            ended_at: None,
            db_session_id: enrichment.as_ref().map(|e| e.0),
            slug: enrichment.as_ref().and_then(|e| e.1.clone()),
            project_name: enrichment.as_ref().map(|e| e.2.clone()),
            git_branch: enrichment.as_ref().and_then(|e| e.3.clone()),
            message_count: enrichment.as_ref().map(|e| e.4),
            user_msg_count: enrichment.as_ref().map(|e| e.5),
            total_input_tokens: enrichment.as_ref().map(|e| e.6),
            total_output_tokens: enrichment.as_ref().map(|e| e.7),
            total_cache_creation_tokens: enrichment.as_ref().map(|e| e.8),
            total_cache_read_tokens: enrichment.as_ref().map(|e| e.9),
            version: enrichment.as_ref().and_then(|e| e.10.clone()),
            file_size: enrichment.as_ref().map(|e| e.11),
            last_message_preview: None,
            active_subagent_count: subagent_count,
        });
    }

    results
}
