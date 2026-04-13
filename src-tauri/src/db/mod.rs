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

            CREATE TABLE IF NOT EXISTS daily_token_usage (
                date          TEXT NOT NULL,
                session_id    TEXT NOT NULL,
                input_tokens  INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cache_creation_tokens INTEGER DEFAULT 0,
                cache_read_tokens INTEGER DEFAULT 0,
                user_msg_count INTEGER DEFAULT 0,
                PRIMARY KEY (date, session_id)
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

        // Migrations for existing DBs
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_creation_tokens INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_read_tokens INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN is_favorited INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN is_hidden INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE projects ADD COLUMN is_starred INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE daily_token_usage ADD COLUMN user_msg_count INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN copied_from_session_id TEXT", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN copied_at INTEGER", []).ok();

        // Migrate existing favorites table data into sessions.is_favorited
        conn.execute(
            "UPDATE sessions SET is_favorited = 1 WHERE id IN (SELECT session_id FROM favorites)",
            [],
        ).ok();

        // Force re-parse all sessions to update user_msg_count with new logic
        // (only runs once — after re-parse, file_mtime will be set correctly)
        let needs_reparse: bool = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE user_msg_count > 0 AND file_mtime > 0",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) > 0;
        // Check if we already ran this migration by looking at app_config
        let reparse_done: bool = conn.query_row(
            "SELECT COUNT(*) FROM app_config WHERE key = 'user_msg_reparse_v3'",
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) > 0;
        if needs_reparse && !reparse_done {
            conn.execute("UPDATE sessions SET file_mtime = 0", []).ok();
            conn.execute(
                "INSERT INTO app_config (key, value) VALUES ('user_msg_reparse_v3', '1')",
                [],
            ).ok();
        }

        Ok(())
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}
