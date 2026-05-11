//! PTY session backend — spawns a child process inside a pseudo-terminal,
//! relays its output to the frontend via Tauri events, and accepts input
//! through commands. Adapted from the tauri-zellij POC.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct PtySession {
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

#[derive(Serialize, Clone)]
struct OutputPayload {
    data: String, // base64-encoded raw bytes
}

impl PtySession {
    pub fn spawn(
        app: AppHandle,
        argv: Vec<String>,
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<Self, String> {
        if argv.is_empty() {
            return Err("empty argv".into());
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty: {}", e))?;

        let mut builder = CommandBuilder::new(&argv[0]);
        for a in argv.iter().skip(1) {
            builder.arg(a);
        }

        // Inherit a sane env so multiplexer binaries are findable and render correctly.
        for key in ["HOME", "USER", "LANG", "LC_ALL", "SHELL"] {
            if let Ok(v) = std::env::var(key) {
                builder.env(key, v);
            }
        }
        // Carry the user's PATH (homebrew, cargo, etc.).
        let path = std::env::var("PATH")
            .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".into());
        builder.env("PATH", path);
        builder.env(
            "TERM",
            std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into()),
        );
        // Clear multiplexer env vars inherited from the launching shell, so
        // `zellij attach` / `tmux attach` don't think they are running nested.
        // (Without this, zellij panics when we attach to "the current session",
        // and tmux refuses to nest by default.)
        for key in [
            "ZELLIJ",
            "ZELLIJ_SESSION_NAME",
            "ZELLIJ_PANE_ID",
            "TMUX",
            "TMUX_PANE",
            "TMUX_PLUGIN_MANAGER_PATH",
        ] {
            // CommandBuilder builds env from scratch by default for these keys
            // since we only `env()` what we explicitly want — but be defensive in
            // case a future portable-pty version inherits the parent env.
            builder.env_remove(key);
        }
        let cwd_to_use = cwd
            .filter(|c| !c.is_empty())
            .or_else(|| std::env::var("HOME").ok());
        if let Some(c) = cwd_to_use {
            builder.cwd(c);
        }

        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| format!("spawn: {}", e))?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| e.to_string())?;

        let app_for_thread = app.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let payload = OutputPayload {
                            data: B64.encode(&buf[..n]),
                        };
                        if app_for_thread.emit("pty://output", payload).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = app_for_thread.emit("pty://exit", ());
        });

        Ok(PtySession {
            master: Arc::new(Mutex::new(pair.master)),
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
        })
    }

    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        let mut w = self.writer.lock();
        w.write_all(data).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let m = self.master.lock();
        m.resize(PtySize {
            cols,
            rows,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("resize: {}", e))
    }

    pub fn kill(&self) {
        let mut child = self.child.lock();
        let _ = child.kill();
    }
}

/// Holds the single active PTY session. Lives inside Tauri state.
pub struct PtyState {
    inner: Mutex<Option<PtySession>>,
}

impl PtyState {
    pub fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }

    /// Replace any existing session with a new one (kills the previous).
    pub fn replace(&self, session: PtySession) {
        let mut g = self.inner.lock();
        if let Some(prev) = g.take() {
            prev.kill();
        }
        *g = Some(session);
    }

    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        let g = self.inner.lock();
        match g.as_ref() {
            Some(s) => s.write(data),
            None => Err("no active pty session".into()),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let g = self.inner.lock();
        match g.as_ref() {
            Some(s) => s.resize(cols, rows),
            None => Err("no active pty session".into()),
        }
    }

    pub fn detach(&self) {
        let mut g = self.inner.lock();
        if let Some(s) = g.take() {
            s.kill();
        }
    }
}
