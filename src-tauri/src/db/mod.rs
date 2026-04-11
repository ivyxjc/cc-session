pub mod models;

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("claude-session-manager");
        std::fs::create_dir_all(&db_dir).ok();
        let db_path = db_dir.join("index.db");
        let conn = Connection::open(db_path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS projects (
                id            INTEGER PRIMARY KEY,
                encoded_path  TEXT UNIQUE,
                original_path TEXT,
                display_name  TEXT,
                session_count INTEGER DEFAULT 0,
                last_active   INTEGER,
                created_at    INTEGER
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id                  INTEGER PRIMARY KEY,
                session_id          TEXT UNIQUE,
                project_id          INTEGER REFERENCES projects(id),
                jsonl_path          TEXT,
                slug                TEXT,
                version             TEXT,
                permission_mode     TEXT,
                git_branch          TEXT,
                started_at          INTEGER,
                last_active         INTEGER,
                message_count       INTEGER DEFAULT 0,
                user_msg_count      INTEGER DEFAULT 0,
                assistant_msg_count INTEGER DEFAULT 0,
                total_input_tokens  INTEGER DEFAULT 0,
                total_output_tokens INTEGER DEFAULT 0,
                total_cache_creation_tokens INTEGER DEFAULT 0,
                total_cache_read_tokens INTEGER DEFAULT 0,
                file_size           INTEGER DEFAULT 0,
                file_mtime          INTEGER DEFAULT 0,
                is_backed_up        INTEGER DEFAULT 0,
                created_at          INTEGER
            );

            CREATE TABLE IF NOT EXISTS favorites (
                id         INTEGER PRIMARY KEY,
                session_id INTEGER REFERENCES sessions(id),
                note       TEXT,
                created_at INTEGER,
                UNIQUE(session_id)
            );

            CREATE TABLE IF NOT EXISTS tags (
                id    INTEGER PRIMARY KEY,
                name  TEXT UNIQUE,
                color TEXT
            );

            CREATE TABLE IF NOT EXISTS session_tags (
                session_id INTEGER REFERENCES sessions(id),
                tag_id     INTEGER REFERENCES tags(id),
                PRIMARY KEY (session_id, tag_id)
            );

            CREATE TABLE IF NOT EXISTS backups (
                id            INTEGER PRIMARY KEY,
                session_id    INTEGER REFERENCES sessions(id),
                backup_path   TEXT,
                backup_type   TEXT,
                original_size INTEGER,
                compressed    INTEGER DEFAULT 1,
                created_at    INTEGER
            );

            CREATE TABLE IF NOT EXISTS subagents (
                id          INTEGER PRIMARY KEY,
                session_id  INTEGER REFERENCES sessions(id),
                agent_id    TEXT,
                agent_type  TEXT,
                description TEXT,
                jsonl_path  TEXT,
                created_at  INTEGER
            );

            CREATE TABLE IF NOT EXISTS app_config (
                key   TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_project     ON sessions(project_id, last_active DESC);
            CREATE INDEX IF NOT EXISTS idx_sessions_last_active  ON sessions(last_active DESC);
            CREATE INDEX IF NOT EXISTS idx_sessions_slug         ON sessions(slug);
            CREATE INDEX IF NOT EXISTS idx_favorites_session     ON favorites(session_id);
            CREATE INDEX IF NOT EXISTS idx_session_tags_tag      ON session_tags(tag_id);
            CREATE INDEX IF NOT EXISTS idx_backups_session       ON backups(session_id);
            CREATE INDEX IF NOT EXISTS idx_subagents_session     ON subagents(session_id);
        ")?;

        // Migration: add cache token columns for existing DBs
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_creation_tokens INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_read_tokens INTEGER DEFAULT 0", []).ok();

        Ok(())
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}
