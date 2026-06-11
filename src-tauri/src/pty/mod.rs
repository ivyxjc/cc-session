//! PTY session backend — spawns a child process inside a pseudo-terminal,
//! relays its output to the frontend via Tauri events, and accepts input
//! through commands. Adapted from the tauri-zellij POC.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Serialize, Serializer};
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Runtime};

pub type PtyResult<T> = Result<T, PtyError>;

/// Typed errors for PTY operations. Serialized to a string when crossing the
/// Tauri IPC boundary so the frontend just sees a readable message.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("empty argv")]
    EmptyArgv,

    #[error("no active pty session")]
    NoActiveSession,

    #[error("openpty failed: {0}")]
    OpenPty(String),

    #[error("spawn failed: {0}")]
    Spawn(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("pty: {0}")]
    Generic(String),
}

impl Serialize for PtyError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

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
    pub fn spawn<R: Runtime>(
        app: AppHandle<R>,
        argv: Vec<String>,
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    ) -> PtyResult<Self> {
        if argv.is_empty() {
            return Err(PtyError::EmptyArgv);
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::OpenPty(e.to_string()))?;

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
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| PtyError::Generic(e.to_string()))?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Generic(e.to_string()))?;

        // Reader thread pushes raw chunks into a channel; the emitter thread
        // coalesces whatever queued up while it was emitting the previous
        // batch. Under heavy output (builds, `cat` of a big file) this turns
        // thousands of per-4KB IPC events into a few large ones, without
        // adding latency to the interactive keystroke-echo path.
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        const MAX_BATCH: usize = 256 * 1024;
        let app_for_thread = app.clone();
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut batch = first;
                while batch.len() < MAX_BATCH {
                    match rx.try_recv() {
                        Ok(more) => batch.extend_from_slice(&more),
                        Err(_) => break,
                    }
                }
                let payload = OutputPayload {
                    data: B64.encode(&batch),
                };
                if app_for_thread.emit("pty://output", payload).is_err() {
                    break;
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

    pub fn write(&self, data: &[u8]) -> PtyResult<()> {
        let mut w = self.writer.lock();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> PtyResult<()> {
        let m = self.master.lock();
        m.resize(PtySize {
            cols,
            rows,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| PtyError::Generic(format!("resize: {}", e)))
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

    pub fn write(&self, data: &[u8]) -> PtyResult<()> {
        let g = self.inner.lock();
        match g.as_ref() {
            Some(s) => s.write(data),
            None => Err(PtyError::NoActiveSession),
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) -> PtyResult<()> {
        let g = self.inner.lock();
        match g.as_ref() {
            Some(s) => s.resize(cols, rows),
            None => Err(PtyError::NoActiveSession),
        }
    }

    pub fn detach(&self) {
        let mut g = self.inner.lock();
        if let Some(s) = g.take() {
            s.kill();
        }
    }

    /// PID of the spawned multiplexer client process, if a session is active.
    /// Used to exclude our own client when measuring external client sizes.
    pub fn child_pid(&self) -> Option<u32> {
        let g = self.inner.lock();
        g.as_ref().and_then(|s| s.child.lock().process_id())
    }
}
