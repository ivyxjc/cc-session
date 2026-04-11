# Claude Session Manager — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Tauri 2 desktop app that browses Claude Code sessions, supports favorites/tags, and provides persistent backup.

**Architecture:** Tauri 2 (Rust backend + React frontend). Rust reads `~/.claude/` read-only, indexes into SQLite, serves data via IPC commands. React renders conversations with Markdown/code highlighting/diff views.

**Tech Stack:** Tauri 2, Rust (rusqlite, serde, zstd, chrono, dirs), React 19, TypeScript, Tailwind CSS v4, shadcn/ui, shiki, react-markdown, zustand, @tanstack/react-virtual, @tanstack/react-router

---

## File Map

### Rust Backend (`src-tauri/src/`)

| File | Responsibility |
|------|---------------|
| `main.rs` | Entry point |
| `lib.rs` | Tauri builder, command registration |
| `config.rs` | App config (claude dir path, backup dir, etc.) |
| `db/mod.rs` | SQLite connection, migrations |
| `db/models.rs` | Rust structs: Project, Session, Tag, Backup, Subagent |
| `scanner/mod.rs` | Discover projects and sessions, incremental scan |
| `parser/mod.rs` | JSONL parse entry point |
| `parser/messages.rs` | Message type definitions and deserialization |
| `parser/content.rs` | ContentBlock enum (Text, Thinking, ToolUse, ToolResult) |
| `backup/mod.rs` | Backup/restore engine |
| `commands/mod.rs` | Re-export all command modules |
| `commands/projects.rs` | list_projects, get_project |
| `commands/sessions.rs` | list_sessions, get_session, get_messages, get_subagents, get_subagent_messages |
| `commands/favorites.rs` | toggle_favorite, list_favorites |
| `commands/tags.rs` | create_tag, delete_tag, tag_session, untag_session |
| `commands/backups.rs` | backup_session, backup_all, restore_session, list_backups, delete_backup, get/set_backup_config |
| `commands/scan.rs` | refresh_index |

### React Frontend (`src/`)

| File | Responsibility |
|------|---------------|
| `main.tsx` | React entry point |
| `App.tsx` | Router setup, layout |
| `lib/tauri.ts` | IPC invoke wrappers (typed) |
| `lib/types.ts` | TypeScript types matching Rust structs |
| `lib/format.ts` | Date, token count, file size formatters |
| `stores/appStore.ts` | Global app state (selected project, sidebar) |
| `stores/filterStore.ts` | Filter/sort state |
| `hooks/useProjects.ts` | Project data fetching |
| `hooks/useSessions.ts` | Session list fetching |
| `hooks/useMessages.ts` | Message loading with pagination |
| `hooks/useBackups.ts` | Backup operations |
| `components/layout/Sidebar.tsx` | Navigation sidebar |
| `components/layout/MainContent.tsx` | Content area wrapper |
| `components/project/ProjectList.tsx` | Project list view |
| `components/project/ProjectCard.tsx` | Single project card |
| `components/session/SessionList.tsx` | Session card list |
| `components/session/SessionCard.tsx` | Single session card |
| `components/session/ConversationView.tsx` | Full conversation renderer |
| `components/session/SessionHeader.tsx` | Session metadata bar |
| `components/message/MessageBubble.tsx` | User/assistant message container |
| `components/message/ToolCallBlock.tsx` | Tool use + result rendering |
| `components/message/ThinkingBlock.tsx` | Collapsible thinking section |
| `components/message/DiffView.tsx` | Edit tool diff rendering |
| `components/message/SubagentView.tsx` | Subagent card + expandable conversation |
| `components/message/CodeBlock.tsx` | Shiki-highlighted code block |
| `components/common/TagBadge.tsx` | Tag display/create |
| `components/common/FavoriteButton.tsx` | Star toggle |
| `components/common/FilterBar.tsx` | Sort/filter controls |
| `components/backup/BackupManager.tsx` | Backup list + actions |
| `components/backup/BackupConfigPanel.tsx` | Backup settings |
| `components/settings/SettingsPage.tsx` | App settings |

---

## Task 1: Project Scaffolding

**Files:**
- Create: entire project structure via `create-tauri-app`
- Modify: `src-tauri/Cargo.toml` (add dependencies)
- Modify: `vite.config.ts` (add Tailwind)
- Create: `src/styles/globals.css`

- [ ] **Step 1: Create Tauri project**

```bash
cd /Users/ivyxjc/Zeta/SideProjects/claude-exts
npm create tauri-app@latest -- --template react-ts claude-session-manager
```

When prompted:
- Project name: `claude-session-manager`
- Identifier: `com.ivyxjc.claude-session-manager`
- Frontend: TypeScript / JavaScript
- Package manager: npm
- UI template: React
- UI flavor: TypeScript

- [ ] **Step 2: Install frontend dependencies**

```bash
cd claude-session-manager
npm install
npm install tailwindcss @tailwindcss/vite
npm install react-markdown remark-gfm
npm install shiki
npm install react-diff-viewer-continued
npm install @tanstack/react-virtual
npm install @tanstack/react-router
npm install zustand
npm install lucide-react
npm install clsx tailwind-merge
```

- [ ] **Step 3: Add Rust dependencies to `src-tauri/Cargo.toml`**

Add to `[dependencies]`:

```toml
rusqlite = { version = "0.39", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "6.0"
zstd = "0.13"
walkdir = "2"
```

- [ ] **Step 4: Configure Tailwind v4**

Update `vite.config.ts`:

```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
```

Replace `src/App.css` contents with:

```css
@import "tailwindcss";
```

- [ ] **Step 5: Update window config in `src-tauri/tauri.conf.json`**

Set window size and title:

```json
{
  "app": {
    "windows": [
      {
        "title": "Claude Session Manager",
        "width": 1280,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600
      }
    ]
  }
}
```

- [ ] **Step 6: Verify build**

```bash
npm run tauri dev
```

Expected: Window opens with default React template. Close it.

- [ ] **Step 7: Commit**

```bash
git add claude-session-manager/
git commit -m "feat: scaffold Tauri 2 + React + Tailwind project"
```

---

## Task 2: Rust — SQLite Database Layer

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create db module directory**

```bash
mkdir -p claude-session-manager/src-tauri/src/db
```

- [ ] **Step 2: Write `src-tauri/src/db/models.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub encoded_path: String,
    pub original_path: String,
    pub display_name: String,
    pub session_count: i64,
    pub last_active: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: i64,
    pub session_id: String,
    pub project_id: i64,
    pub project_name: String,
    pub slug: Option<String>,
    pub version: Option<String>,
    pub permission_mode: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub last_active: Option<i64>,
    pub message_count: i64,
    pub user_msg_count: i64,
    pub assistant_msg_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub file_size: i64,
    pub is_favorited: bool,
    pub is_backed_up: bool,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: i64,
    pub session_id: i64,
    pub backup_path: String,
    pub backup_type: String,
    pub original_size: i64,
    pub compressed: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSummary {
    pub id: i64,
    pub session_id: i64,
    pub agent_id: String,
    pub agent_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    pub enabled: bool,
    pub backup_dir: String,
    pub auto_backup: bool,
    pub auto_backup_interval_hours: u32,
    pub compress: bool,
    pub max_backup_copies: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        let default_dir = dirs::data_dir()
            .unwrap_or_default()
            .join("claude-session-manager")
            .join("backups");
        Self {
            enabled: true,
            backup_dir: default_dir.to_string_lossy().to_string(),
            auto_backup: true,
            auto_backup_interval_hours: 24,
            compress: true,
            max_backup_copies: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub projects_found: usize,
    pub sessions_found: usize,
    pub sessions_updated: usize,
    pub sessions_removed: usize,
    pub duration_ms: u64,
}
```

- [ ] **Step 3: Write `src-tauri/src/db/mod.rs`**

```rust
pub mod models;

use rusqlite::{Connection, Result, params};
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
        Ok(())
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}
```

- [ ] **Step 4: Update `src-tauri/src/lib.rs` to initialize database**

```rust
mod db;

use db::Database;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("Failed to initialize database"));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(database)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Compiles and window opens. Check that `~/Library/Application Support/claude-session-manager/index.db` was created.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/ src-tauri/src/lib.rs
git commit -m "feat: add SQLite database layer with schema and models"
```

---

## Task 3: Rust — JSONL Parser

**Files:**
- Create: `src-tauri/src/parser/mod.rs`
- Create: `src-tauri/src/parser/messages.rs`
- Create: `src-tauri/src/parser/content.rs`

- [ ] **Step 1: Create parser module directory**

```bash
mkdir -p claude-session-manager/src-tauri/src/parser
```

- [ ] **Step 2: Write `src-tauri/src/parser/content.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(default)]
        signature: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "tool_use_id")]
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: i64,
    #[serde(default)]
    pub output_tokens: i64,
    #[serde(default)]
    pub cache_creation_input_tokens: i64,
    #[serde(default)]
    pub cache_read_input_tokens: i64,
}
```

- [ ] **Step 3: Write `src-tauri/src/parser/messages.rs`**

```rust
use serde::{Deserialize, Serialize};
use super::content::{ContentBlock, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMessage {
    #[serde(rename = "type")]
    pub msg_type: String,

    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,

    // For user/assistant messages
    #[serde(default)]
    pub message: Option<serde_json::Value>,

    // For system messages
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub content: Option<String>,

    // For attachment messages
    #[serde(default)]
    pub attachment: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ParsedMessage {
    User {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        content: Vec<ContentBlock>,
    },
    Assistant {
        uuid: String,
        parent_uuid: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        content: Vec<ContentBlock>,
        usage: Option<Usage>,
        stop_reason: Option<String>,
    },
    System {
        uuid: Option<String>,
        timestamp: Option<String>,
        subtype: Option<String>,
        content: Option<String>,
    },
    Attachment {
        attachment_type: String,
    },
    PermissionMode {
        mode: String,
    },
    FileHistorySnapshot,
}

impl ParsedMessage {
    pub fn from_raw(raw: &RawMessage) -> Option<Self> {
        match raw.msg_type.as_str() {
            "permission-mode" => {
                Some(ParsedMessage::PermissionMode {
                    mode: raw.permission_mode.clone().unwrap_or_default(),
                })
            }
            "file-history-snapshot" => {
                Some(ParsedMessage::FileHistorySnapshot)
            }
            "user" => {
                let content = Self::extract_content(raw.message.as_ref()?);
                Some(ParsedMessage::User {
                    uuid: raw.uuid.clone().unwrap_or_default(),
                    parent_uuid: raw.parent_uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    content,
                })
            }
            "assistant" => {
                let msg = raw.message.as_ref()?;
                let content = Self::extract_content(Some(msg));
                let model = msg.get("model").and_then(|v| v.as_str()).map(String::from);
                let stop_reason = msg.get("stop_reason").and_then(|v| v.as_str()).map(String::from);
                let usage = msg.get("usage").and_then(|v| serde_json::from_value::<Usage>(v.clone()).ok());
                Some(ParsedMessage::Assistant {
                    uuid: raw.uuid.clone().unwrap_or_default(),
                    parent_uuid: raw.parent_uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    model,
                    content,
                    usage,
                    stop_reason,
                })
            }
            "system" => {
                Some(ParsedMessage::System {
                    uuid: raw.uuid.clone(),
                    timestamp: raw.timestamp.clone(),
                    subtype: raw.subtype.clone(),
                    content: raw.content.clone(),
                })
            }
            "attachment" => {
                let att_type = raw.attachment.as_ref()
                    .and_then(|a| a.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Some(ParsedMessage::Attachment {
                    attachment_type: att_type,
                })
            }
            _ => None,
        }
    }

    fn extract_content(msg: Option<&serde_json::Value>) -> Vec<ContentBlock> {
        let Some(msg) = msg else { return vec![] };

        // content can be a string or an array of content blocks
        if let Some(content) = msg.get("content") {
            if let Some(text) = content.as_str() {
                return vec![ContentBlock::Text { text: text.to_string() }];
            }
            if let Some(arr) = content.as_array() {
                return arr.iter()
                    .filter_map(|v| serde_json::from_value::<ContentBlock>(v.clone()).ok())
                    .collect();
            }
        }
        vec![]
    }
}
```

- [ ] **Step 4: Write `src-tauri/src/parser/mod.rs`**

```rust
pub mod content;
pub mod messages;

use messages::{RawMessage, ParsedMessage};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct SessionParseResult {
    pub slug: Option<String>,
    pub version: Option<String>,
    pub permission_mode: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<String>,
    pub last_active: Option<String>,
    pub message_count: i64,
    pub user_msg_count: i64,
    pub assistant_msg_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
}

/// Parse a session JSONL file and extract metadata for indexing.
/// Does NOT store full messages — those are loaded on demand.
pub fn parse_session_metadata(path: &Path) -> Result<SessionParseResult, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut result = SessionParseResult {
        slug: None,
        version: None,
        permission_mode: None,
        git_branch: None,
        started_at: None,
        last_active: None,
        message_count: 0,
        user_msg_count: 0,
        assistant_msg_count: 0,
        total_input_tokens: 0,
        total_output_tokens: 0,
    };

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Extract metadata from first user message
        if raw.msg_type == "user" {
            if result.slug.is_none() {
                result.slug = raw.slug.clone();
            }
            if result.version.is_none() {
                result.version = raw.version.clone();
            }
            if result.git_branch.is_none() {
                result.git_branch = raw.git_branch.clone();
            }
            // Update slug if later messages have it (slug appears after first turn)
            if raw.slug.is_some() {
                result.slug = raw.slug.clone();
            }
        }

        if raw.msg_type == "permission-mode" {
            result.permission_mode = raw.permission_mode.clone();
        }

        // Track timestamps
        if let Some(ref ts) = raw.timestamp {
            if result.started_at.is_none() {
                result.started_at = Some(ts.clone());
            }
            result.last_active = Some(ts.clone());
        }

        // Count messages and tokens
        match raw.msg_type.as_str() {
            "user" => {
                result.message_count += 1;
                result.user_msg_count += 1;
            }
            "assistant" => {
                result.message_count += 1;
                result.assistant_msg_count += 1;
                if let Some(ref msg) = raw.message {
                    if let Some(usage) = msg.get("usage") {
                        result.total_input_tokens += usage.get("input_tokens")
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                        result.total_output_tokens += usage.get("output_tokens")
                            .and_then(|v| v.as_i64()).unwrap_or(0);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(result)
}

/// Load all messages from a session JSONL for display.
/// Returns parsed messages with offset/limit pagination.
pub fn load_messages(path: &Path, offset: usize, limit: usize) -> Result<Vec<ParsedMessage>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut display_index: usize = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Read error: {}", e))?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Only count displayable messages for pagination
        let dominated = matches!(raw.msg_type.as_str(), "user" | "assistant" | "system");
        if !dominated {
            continue;
        }

        if display_index >= offset {
            if let Some(parsed) = ParsedMessage::from_raw(&raw) {
                messages.push(parsed);
            }
        }

        display_index += 1;
        if messages.len() >= limit {
            break;
        }
    }

    Ok(messages)
}
```

- [ ] **Step 5: Register module in `lib.rs`**

Add `mod parser;` to the top of `src-tauri/src/lib.rs`.

- [ ] **Step 6: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/parser/
git commit -m "feat: add JSONL parser for Claude Code session files"
```

---

## Task 4: Rust — Scanner (Project & Session Discovery)

**Files:**
- Create: `src-tauri/src/scanner/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create scanner module directory**

```bash
mkdir -p claude-session-manager/src-tauri/src/scanner
```

- [ ] **Step 2: Write `src-tauri/src/scanner/mod.rs`**

```rust
use crate::db::Database;
use crate::db::models::ScanResult;
use crate::parser;
use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use walkdir::WalkDir;

fn get_claude_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude")
}

fn decode_project_path(encoded: &str) -> String {
    // "-Users-ivyxjc-myproject" -> "/Users/ivyxjc/myproject"
    encoded.replacen('-', "/", 1).replace('-', "/")
}

fn display_name_from_path(original_path: &str) -> String {
    original_path.rsplit('/').next().unwrap_or(original_path).to_string()
}

pub fn scan_all(db: &Arc<Database>) -> Result<ScanResult, String> {
    let start = Instant::now();
    let claude_dir = get_claude_dir();
    let projects_dir = claude_dir.join("projects");

    if !projects_dir.exists() {
        return Err(format!("Claude projects dir not found: {}", projects_dir.display()));
    }

    let mut projects_found: usize = 0;
    let mut sessions_found: usize = 0;
    let mut sessions_updated: usize = 0;
    let mut sessions_removed: usize = 0;

    let conn = db.conn();

    // Track which session jsonl_paths we see on disk
    let mut seen_paths: Vec<String> = Vec::new();

    // Iterate project directories
    let entries: Vec<_> = std::fs::read_dir(&projects_dir)
        .map_err(|e| format!("Failed to read projects dir: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();

    for entry in entries {
        let encoded_path = entry.file_name().to_string_lossy().to_string();
        let original_path = decode_project_path(&encoded_path);
        let display_name = display_name_from_path(&original_path);
        let project_dir = entry.path();

        // Upsert project
        conn.execute(
            "INSERT INTO projects (encoded_path, original_path, display_name, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(encoded_path) DO UPDATE SET
                original_path = excluded.original_path,
                display_name = excluded.display_name",
            params![encoded_path, original_path, display_name, chrono::Utc::now().timestamp_millis()],
        ).map_err(|e| format!("DB error: {}", e))?;

        let project_id: i64 = conn.query_row(
            "SELECT id FROM projects WHERE encoded_path = ?1",
            params![encoded_path],
            |row| row.get(0),
        ).map_err(|e| format!("DB error: {}", e))?;

        projects_found += 1;

        // Find .jsonl files in project directory (not recursive into subagent dirs)
        let jsonl_files: Vec<_> = std::fs::read_dir(&project_dir)
            .map_err(|e| format!("Read dir error: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().extension().map(|ext| ext == "jsonl").unwrap_or(false)
            })
            .collect();

        let mut project_last_active: Option<i64> = None;
        let mut project_session_count: i64 = 0;

        for jsonl_entry in jsonl_files {
            let jsonl_path = jsonl_entry.path();
            let jsonl_path_str = jsonl_path.to_string_lossy().to_string();
            seen_paths.push(jsonl_path_str.clone());

            // Session ID = filename without .jsonl
            let session_id = jsonl_path.file_stem()
                .unwrap_or_default().to_string_lossy().to_string();

            let metadata = jsonl_entry.metadata().ok();
            let file_size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let file_mtime = metadata.as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
                .unwrap_or(0);

            // Check if we need to re-parse
            let existing: Option<(i64, i64)> = conn.query_row(
                "SELECT file_size, file_mtime FROM sessions WHERE jsonl_path = ?1",
                params![jsonl_path_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).ok();

            let needs_parse = match existing {
                Some((old_size, old_mtime)) => old_size != file_size || old_mtime != file_mtime,
                None => true,
            };

            if needs_parse {
                let parse_result = match parser::parse_session_metadata(&jsonl_path) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let started_at = parse_result.started_at.as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis());
                let last_active = parse_result.last_active.as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.timestamp_millis());

                conn.execute(
                    "INSERT INTO sessions (session_id, project_id, jsonl_path, slug, version,
                        permission_mode, git_branch, started_at, last_active,
                        message_count, user_msg_count, assistant_msg_count,
                        total_input_tokens, total_output_tokens,
                        file_size, file_mtime, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                     ON CONFLICT(session_id) DO UPDATE SET
                        slug = excluded.slug,
                        version = excluded.version,
                        permission_mode = excluded.permission_mode,
                        git_branch = excluded.git_branch,
                        started_at = excluded.started_at,
                        last_active = excluded.last_active,
                        message_count = excluded.message_count,
                        user_msg_count = excluded.user_msg_count,
                        assistant_msg_count = excluded.assistant_msg_count,
                        total_input_tokens = excluded.total_input_tokens,
                        total_output_tokens = excluded.total_output_tokens,
                        file_size = excluded.file_size,
                        file_mtime = excluded.file_mtime",
                    params![
                        session_id, project_id, jsonl_path_str,
                        parse_result.slug, parse_result.version,
                        parse_result.permission_mode, parse_result.git_branch,
                        started_at, last_active,
                        parse_result.message_count, parse_result.user_msg_count,
                        parse_result.assistant_msg_count,
                        parse_result.total_input_tokens, parse_result.total_output_tokens,
                        file_size, file_mtime, chrono::Utc::now().timestamp_millis(),
                    ],
                ).map_err(|e| format!("DB error: {}", e))?;

                sessions_updated += 1;

                // Scan subagents
                let subagent_dir = project_dir.join(&session_id).join("subagents");
                if subagent_dir.exists() {
                    scan_subagents(&conn, &subagent_dir, &session_id)?;
                }

                if let Some(la) = last_active {
                    if project_last_active.map_or(true, |pla| la > pla) {
                        project_last_active = Some(la);
                    }
                }
            }

            sessions_found += 1;
            project_session_count += 1;
        }

        // Update project stats
        conn.execute(
            "UPDATE projects SET session_count = ?1, last_active = COALESCE(?2, last_active) WHERE id = ?3",
            params![project_session_count, project_last_active, project_id],
        ).map_err(|e| format!("DB error: {}", e))?;
    }

    // Remove sessions whose files no longer exist
    let mut stmt = conn.prepare("SELECT id, jsonl_path FROM sessions")
        .map_err(|e| format!("DB error: {}", e))?;
    let orphans: Vec<i64> = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let path: String = row.get(1)?;
        Ok((id, path))
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .filter(|(_, path)| !seen_paths.contains(path))
    .map(|(id, _)| id)
    .collect();

    for id in &orphans {
        conn.execute("DELETE FROM session_tags WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM favorites WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM subagents WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).ok();
        sessions_removed += 1;
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(ScanResult {
        projects_found,
        sessions_found,
        sessions_updated,
        sessions_removed,
        duration_ms,
    })
}

fn scan_subagents(conn: &rusqlite::Connection, subagent_dir: &Path, session_id: &str) -> Result<(), String> {
    let db_session_id: i64 = conn.query_row(
        "SELECT id FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("DB error: {}", e))?;

    for entry in std::fs::read_dir(subagent_dir).map_err(|e| format!("Read error: {}", e))? {
        let entry = entry.map_err(|e| format!("Read error: {}", e))?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false)
            && path.to_string_lossy().contains(".meta.json")
        {
            let meta_content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Read error: {}", e))?;
            let meta: serde_json::Value = serde_json::from_str(&meta_content)
                .map_err(|e| format!("Parse error: {}", e))?;

            let agent_id = path.file_stem()
                .unwrap_or_default().to_string_lossy()
                .replace(".meta", "");
            let agent_type = meta.get("agentType")
                .and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let description = meta.get("description")
                .and_then(|v| v.as_str()).unwrap_or("").to_string();

            let jsonl_path = subagent_dir.join(format!("{}.jsonl", agent_id));
            let jsonl_path_str = jsonl_path.to_string_lossy().to_string();

            conn.execute(
                "INSERT OR REPLACE INTO subagents (session_id, agent_id, agent_type, description, jsonl_path, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    db_session_id, agent_id, agent_type, description,
                    jsonl_path_str, chrono::Utc::now().timestamp_millis()
                ],
            ).map_err(|e| format!("DB error: {}", e))?;
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Register module in `lib.rs`**

Add `mod scanner;` to the top of `src-tauri/src/lib.rs`.

- [ ] **Step 4: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scanner/
git commit -m "feat: add scanner for discovering projects and sessions from ~/.claude/"
```

---

## Task 5: Rust — Tauri IPC Commands (Projects, Sessions, Messages)

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/projects.rs`
- Create: `src-tauri/src/commands/sessions.rs`
- Create: `src-tauri/src/commands/scan.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create commands module directory**

```bash
mkdir -p claude-session-manager/src-tauri/src/commands
```

- [ ] **Step 2: Write `src-tauri/src/commands/projects.rs`**

```rust
use crate::db::Database;
use crate::db::models::Project;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_projects(
    db: State<'_, Arc<Database>>,
    sort_by: Option<String>,
) -> Result<Vec<Project>, String> {
    let conn = db.conn();
    let order = match sort_by.as_deref() {
        Some("name") => "display_name ASC",
        Some("sessions") => "session_count DESC",
        _ => "last_active DESC NULLS LAST",
    };

    let query = format!(
        "SELECT id, encoded_path, original_path, display_name, session_count, last_active
         FROM projects ORDER BY {}",
        order
    );

    let mut stmt = conn.prepare(&query).map_err(|e| format!("DB error: {}", e))?;
    let projects = stmt.query_map([], |row| {
        Ok(Project {
            id: row.get(0)?,
            encoded_path: row.get(1)?,
            original_path: row.get(2)?,
            display_name: row.get(3)?,
            session_count: row.get(4)?,
            last_active: row.get(5)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(projects)
}
```

- [ ] **Step 3: Write `src-tauri/src/commands/sessions.rs`**

```rust
use crate::db::Database;
use crate::db::models::{SessionSummary, Tag, SubagentSummary};
use crate::parser;
use crate::parser::messages::ParsedMessage;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_sessions(
    db: State<'_, Arc<Database>>,
    project_id: Option<i64>,
    tag_id: Option<i64>,
    favorited: Option<bool>,
    sort_by: Option<String>,
) -> Result<Vec<SessionSummary>, String> {
    let conn = db.conn();

    let mut conditions: Vec<String> = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(pid) = project_id {
        conditions.push(format!("s.project_id = ?{}", param_values.len() + 1));
        param_values.push(Box::new(pid));
    }
    if let Some(true) = favorited {
        conditions.push("f.id IS NOT NULL".to_string());
    }
    if let Some(tid) = tag_id {
        conditions.push(format!("st.tag_id = ?{}", param_values.len() + 1));
        param_values.push(Box::new(tid));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let order = match sort_by.as_deref() {
        Some("size") => "s.file_size DESC",
        Some("messages") => "s.message_count DESC",
        Some("tokens") => "(s.total_input_tokens + s.total_output_tokens) DESC",
        _ => "s.last_active DESC NULLS LAST",
    };

    let query = format!(
        "SELECT DISTINCT s.id, s.session_id, s.project_id, p.display_name,
                s.slug, s.version, s.permission_mode, s.git_branch,
                s.started_at, s.last_active, s.message_count,
                s.user_msg_count, s.assistant_msg_count,
                s.total_input_tokens, s.total_output_tokens, s.file_size,
                CASE WHEN f.id IS NOT NULL THEN 1 ELSE 0 END as is_favorited,
                s.is_backed_up
         FROM sessions s
         JOIN projects p ON s.project_id = p.id
         LEFT JOIN favorites f ON s.id = f.session_id
         LEFT JOIN session_tags st ON s.id = st.session_id
         {} ORDER BY {}",
        where_clause, order
    );

    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&query).map_err(|e| format!("DB error: {}", e))?;

    let session_rows: Vec<(i64, String, i64, String, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<i64>, Option<i64>,
        i64, i64, i64, i64, i64, i64, bool, bool)> = stmt.query_map(
        params_ref.as_slice(),
        |row| {
            Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
                row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                row.get(12)?, row.get(13)?, row.get(14)?, row.get(15)?,
                row.get(16)?, row.get(17)?,
            ))
        },
    )
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    let mut sessions = Vec::new();
    for row in session_rows {
        let tags = get_session_tags(&conn, row.0)?;
        sessions.push(SessionSummary {
            id: row.0,
            session_id: row.1,
            project_id: row.2,
            project_name: row.3,
            slug: row.4,
            version: row.5,
            permission_mode: row.6,
            git_branch: row.7,
            started_at: row.8,
            last_active: row.9,
            message_count: row.10,
            user_msg_count: row.11,
            assistant_msg_count: row.12,
            total_input_tokens: row.13,
            total_output_tokens: row.14,
            file_size: row.15,
            is_favorited: row.16,
            is_backed_up: row.17,
            tags,
        });
    }

    Ok(sessions)
}

fn get_session_tags(conn: &rusqlite::Connection, session_id: i64) -> Result<Vec<Tag>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color FROM tags t
         JOIN session_tags st ON t.id = st.tag_id
         WHERE st.session_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;

    let tags = stmt.query_map(params![session_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(tags)
}

#[tauri::command]
pub fn get_messages(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ParsedMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )
}

#[tauri::command]
pub fn get_subagents(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<Vec<SubagentSummary>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare(
        "SELECT id, session_id, agent_id, agent_type, description
         FROM subagents WHERE session_id = ?1"
    ).map_err(|e| format!("DB error: {}", e))?;

    let subagents = stmt.query_map(params![session_id], |row| {
        Ok(SubagentSummary {
            id: row.get(0)?,
            session_id: row.get(1)?,
            agent_id: row.get(2)?,
            agent_type: row.get(3)?,
            description: row.get(4)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(subagents)
}

#[tauri::command]
pub fn get_subagent_messages(
    db: State<'_, Arc<Database>>,
    subagent_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ParsedMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM subagents WHERE id = ?1",
        params![subagent_id],
        |row| row.get(0),
    ).map_err(|e| format!("Subagent not found: {}", e))?;

    parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )
}
```

- [ ] **Step 4: Write `src-tauri/src/commands/scan.rs`**

```rust
use crate::db::Database;
use crate::db::models::ScanResult;
use crate::scanner;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn refresh_index(db: State<'_, Arc<Database>>) -> Result<ScanResult, String> {
    scanner::scan_all(&db)
}
```

- [ ] **Step 5: Write `src-tauri/src/commands/mod.rs`**

```rust
pub mod projects;
pub mod sessions;
pub mod scan;
```

- [ ] **Step 6: Update `lib.rs` to register commands**

```rust
mod db;
mod parser;
mod scanner;
mod commands;

use db::Database;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let database = Arc::new(Database::new().expect("Failed to initialize database"));

    // Run initial scan
    let _ = scanner::scan_all(&database);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(database)
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::sessions::list_sessions,
            commands::sessions::get_messages,
            commands::sessions::get_subagents,
            commands::sessions::get_subagent_messages,
            commands::scan::refresh_index,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Compiles. The initial scan runs on startup, indexing your `~/.claude/projects/`.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs
git commit -m "feat: add Tauri IPC commands for projects, sessions, and messages"
```

---

## Task 6: Rust — Favorites & Tags Commands

**Files:**
- Create: `src-tauri/src/commands/favorites.rs`
- Create: `src-tauri/src/commands/tags.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write `src-tauri/src/commands/favorites.rs`**

```rust
use crate::db::Database;
use crate::db::models::SessionSummary;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn toggle_favorite(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    note: Option<String>,
) -> Result<bool, String> {
    let conn = db.conn();

    let exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM favorites WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, i64>(0),
    ).map_err(|e| format!("DB error: {}", e))? > 0;

    if exists {
        conn.execute("DELETE FROM favorites WHERE session_id = ?1", params![session_id])
            .map_err(|e| format!("DB error: {}", e))?;
        Ok(false)
    } else {
        conn.execute(
            "INSERT INTO favorites (session_id, note, created_at) VALUES (?1, ?2, ?3)",
            params![session_id, note, chrono::Utc::now().timestamp_millis()],
        ).map_err(|e| format!("DB error: {}", e))?;
        Ok(true)
    }
}
```

- [ ] **Step 2: Write `src-tauri/src/commands/tags.rs`**

```rust
use crate::db::Database;
use crate::db::models::Tag;
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn create_tag(
    db: State<'_, Arc<Database>>,
    name: String,
    color: String,
) -> Result<Tag, String> {
    let conn = db.conn();
    conn.execute(
        "INSERT INTO tags (name, color) VALUES (?1, ?2)",
        params![name, color],
    ).map_err(|e| format!("DB error: {}", e))?;

    let id = conn.last_insert_rowid();
    Ok(Tag { id, name, color })
}

#[tauri::command]
pub fn delete_tag(
    db: State<'_, Arc<Database>>,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute("DELETE FROM session_tags WHERE tag_id = ?1", params![tag_id])
        .map_err(|e| format!("DB error: {}", e))?;
    conn.execute("DELETE FROM tags WHERE id = ?1", params![tag_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn list_tags(db: State<'_, Arc<Database>>) -> Result<Vec<Tag>, String> {
    let conn = db.conn();
    let mut stmt = conn.prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(|e| format!("DB error: {}", e))?;

    let tags = stmt.query_map([], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })
    .map_err(|e| format!("DB error: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(tags)
}

#[tauri::command]
pub fn tag_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR IGNORE INTO session_tags (session_id, tag_id) VALUES (?1, ?2)",
        params![session_id, tag_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn untag_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    tag_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM session_tags WHERE session_id = ?1 AND tag_id = ?2",
        params![session_id, tag_id],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}
```

- [ ] **Step 3: Update `src-tauri/src/commands/mod.rs`**

```rust
pub mod projects;
pub mod sessions;
pub mod scan;
pub mod favorites;
pub mod tags;
```

- [ ] **Step 4: Register new commands in `lib.rs`**

Add to the `generate_handler!` macro:

```rust
commands::favorites::toggle_favorite,
commands::tags::create_tag,
commands::tags::delete_tag,
commands::tags::list_tags,
commands::tags::tag_session,
commands::tags::untag_session,
```

- [ ] **Step 5: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/
git commit -m "feat: add favorites and tags IPC commands"
```

---

## Task 7: Rust — Backup Engine

**Files:**
- Create: `src-tauri/src/backup/mod.rs`
- Create: `src-tauri/src/commands/backups.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create backup module directory**

```bash
mkdir -p claude-session-manager/src-tauri/src/backup
```

- [ ] **Step 2: Write `src-tauri/src/backup/mod.rs`**

```rust
use crate::db::Database;
use crate::db::models::BackupConfig;
use rusqlite::params;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn get_backup_config(db: &Arc<Database>) -> BackupConfig {
    let conn = db.conn();
    let json: Option<String> = conn.query_row(
        "SELECT value FROM app_config WHERE key = 'backup_config'",
        [],
        |row| row.get(0),
    ).ok();

    json.and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

pub fn save_backup_config(db: &Arc<Database>, config: &BackupConfig) -> Result<(), String> {
    let conn = db.conn();
    let json = serde_json::to_string(config).map_err(|e| format!("Serialize error: {}", e))?;
    conn.execute(
        "INSERT INTO app_config (key, value) VALUES ('backup_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    ).map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

pub fn backup_session_file(
    db: &Arc<Database>,
    session_db_id: i64,
    config: &BackupConfig,
) -> Result<crate::db::models::Backup, String> {
    let conn = db.conn();

    let (jsonl_path, session_id, project_encoded): (String, String, String) = conn.query_row(
        "SELECT s.jsonl_path, s.session_id, p.encoded_path
         FROM sessions s JOIN projects p ON s.project_id = p.id
         WHERE s.id = ?1",
        params![session_db_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let source = Path::new(&jsonl_path);
    if !source.exists() {
        return Err(format!("Source file not found: {}", jsonl_path));
    }

    let original_size = source.metadata()
        .map_err(|e| format!("Metadata error: {}", e))?.len() as i64;

    let timestamp = chrono::Utc::now().timestamp_millis();
    let backup_dir = PathBuf::from(&config.backup_dir)
        .join(&project_encoded)
        .join(&session_id);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup dir: {}", e))?;

    let backup_filename = if config.compress {
        format!("{}.jsonl.zst", timestamp)
    } else {
        format!("{}.jsonl", timestamp)
    };
    let backup_path = backup_dir.join(&backup_filename);

    // Read source
    let mut source_data = Vec::new();
    fs::File::open(source)
        .map_err(|e| format!("Read error: {}", e))?
        .read_to_end(&mut source_data)
        .map_err(|e| format!("Read error: {}", e))?;

    // Write backup (optionally compressed)
    if config.compress {
        let compressed = zstd::encode_all(source_data.as_slice(), 3)
            .map_err(|e| format!("Compression error: {}", e))?;
        fs::write(&backup_path, compressed)
            .map_err(|e| format!("Write error: {}", e))?;
    } else {
        fs::write(&backup_path, &source_data)
            .map_err(|e| format!("Write error: {}", e))?;
    }

    // Also backup subagent files
    let subagent_dir = Path::new(&jsonl_path)
        .parent().unwrap_or(Path::new(""))
        .join(&session_id).join("subagents");
    if subagent_dir.exists() {
        let backup_subagent_dir = backup_dir.join("subagents");
        fs::create_dir_all(&backup_subagent_dir).ok();
        for entry in fs::read_dir(&subagent_dir).map_err(|e| format!("Read error: {}", e))? {
            let entry = entry.map_err(|e| format!("Read error: {}", e))?;
            let src_path = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.ends_with(".meta.json") {
                fs::copy(&src_path, backup_subagent_dir.join(&filename)).ok();
            } else if filename.ends_with(".jsonl") {
                let mut data = Vec::new();
                fs::File::open(&src_path).ok()
                    .map(|mut f| f.read_to_end(&mut data));
                if config.compress {
                    let compressed = zstd::encode_all(data.as_slice(), 3).ok();
                    if let Some(c) = compressed {
                        fs::write(backup_subagent_dir.join(format!("{}.zst", filename)), c).ok();
                    }
                } else {
                    fs::write(backup_subagent_dir.join(&filename), &data).ok();
                }
            }
        }
    }

    let backup_path_str = backup_path.to_string_lossy().to_string();

    // Record in DB
    conn.execute(
        "INSERT INTO backups (session_id, backup_path, backup_type, original_size, compressed, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![session_db_id, backup_path_str, "manual", original_size, config.compress, timestamp],
    ).map_err(|e| format!("DB error: {}", e))?;

    conn.execute(
        "UPDATE sessions SET is_backed_up = 1 WHERE id = ?1",
        params![session_db_id],
    ).ok();

    // Enforce max_backup_copies
    let backups: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, backup_path FROM backups WHERE session_id = ?1 ORDER BY created_at DESC"
        ).map_err(|e| format!("DB error: {}", e))?;
        stmt.query_map(params![session_db_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("DB error: {}", e))?
            .filter_map(|r| r.ok())
            .collect()
    };

    if backups.len() > config.max_backup_copies as usize {
        for (id, path) in backups.iter().skip(config.max_backup_copies as usize) {
            fs::remove_file(path).ok();
            conn.execute("DELETE FROM backups WHERE id = ?1", params![id]).ok();
        }
    }

    let backup_id = conn.last_insert_rowid();
    Ok(crate::db::models::Backup {
        id: backup_id,
        session_id: session_db_id,
        backup_path: backup_path_str,
        backup_type: "manual".to_string(),
        original_size,
        compressed: config.compress,
        created_at: timestamp,
    })
}

pub fn restore_backup(backup_path: &str, compressed: bool) -> Result<(), String> {
    let path = Path::new(backup_path);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }

    // Determine target path from backup directory structure
    // backup_dir / encoded_project / session_id / timestamp.jsonl[.zst]
    let session_id = path.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("Invalid backup path structure")?;
    let encoded_project = path.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .ok_or("Invalid backup path structure")?;

    let claude_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let target_dir = claude_dir.join("projects").join(&encoded_project);
    fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Failed to create target dir: {}", e))?;

    let target_path = target_dir.join(format!("{}.jsonl", session_id));

    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| format!("Read error: {}", e))?
        .read_to_end(&mut data)
        .map_err(|e| format!("Read error: {}", e))?;

    if compressed {
        let decompressed = zstd::decode_all(data.as_slice())
            .map_err(|e| format!("Decompression error: {}", e))?;
        fs::write(&target_path, decompressed)
            .map_err(|e| format!("Write error: {}", e))?;
    } else {
        fs::write(&target_path, &data)
            .map_err(|e| format!("Write error: {}", e))?;
    }

    // Restore subagents if present
    let backup_subagent_dir = path.parent().unwrap().join("subagents");
    if backup_subagent_dir.exists() {
        let target_subagent_dir = target_dir.join(&session_id).join("subagents");
        fs::create_dir_all(&target_subagent_dir).ok();
        for entry in fs::read_dir(&backup_subagent_dir).map_err(|e| format!("Read error: {}", e))? {
            let entry = entry.map_err(|e| format!("Read error: {}", e))?;
            let src = entry.path();
            let filename = entry.file_name().to_string_lossy().to_string();

            if filename.ends_with(".meta.json") {
                fs::copy(&src, target_subagent_dir.join(&filename)).ok();
            } else if filename.ends_with(".jsonl.zst") {
                let mut d = Vec::new();
                fs::File::open(&src).ok().map(|mut f| f.read_to_end(&mut d));
                if let Ok(decompressed) = zstd::decode_all(d.as_slice()) {
                    let target_name = filename.trim_end_matches(".zst");
                    fs::write(target_subagent_dir.join(target_name), decompressed).ok();
                }
            } else if filename.ends_with(".jsonl") {
                fs::copy(&src, target_subagent_dir.join(&filename)).ok();
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Write `src-tauri/src/commands/backups.rs`**

```rust
use crate::backup;
use crate::db::Database;
use crate::db::models::{Backup, BackupConfig};
use rusqlite::params;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn backup_session(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<Backup, String> {
    let config = backup::get_backup_config(&db);
    backup::backup_session_file(&db, session_id, &config)
}

#[tauri::command]
pub fn backup_all_sessions(db: State<'_, Arc<Database>>) -> Result<Vec<Backup>, String> {
    let config = backup::get_backup_config(&db);
    let conn = db.conn();

    let session_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT id FROM sessions")
            .map_err(|e| format!("DB error: {}", e))?;
        stmt.query_map([], |row| row.get(0))
            .map_err(|e| format!("DB error: {}", e))?
            .filter_map(|r| r.ok())
            .collect()
    };
    drop(conn);

    let mut results = Vec::new();
    for sid in session_ids {
        match backup::backup_session_file(&db, sid, &config) {
            Ok(b) => results.push(b),
            Err(e) => eprintln!("Backup failed for session {}: {}", sid, e),
        }
    }
    Ok(results)
}

#[tauri::command]
pub fn restore_session_backup(
    db: State<'_, Arc<Database>>,
    backup_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    let (backup_path, compressed): (String, bool) = conn.query_row(
        "SELECT backup_path, compressed FROM backups WHERE id = ?1",
        params![backup_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| format!("Backup not found: {}", e))?;

    backup::restore_backup(&backup_path, compressed)
}

#[tauri::command]
pub fn list_backups(
    db: State<'_, Arc<Database>>,
    session_id: Option<i64>,
) -> Result<Vec<Backup>, String> {
    let conn = db.conn();
    let query = if session_id.is_some() {
        "SELECT id, session_id, backup_path, backup_type, original_size, compressed, created_at
         FROM backups WHERE session_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, session_id, backup_path, backup_type, original_size, compressed, created_at
         FROM backups ORDER BY created_at DESC"
    };

    let mut stmt = conn.prepare(query).map_err(|e| format!("DB error: {}", e))?;

    let backups = if let Some(sid) = session_id {
        stmt.query_map(params![sid], |row| {
            Ok(Backup {
                id: row.get(0)?,
                session_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_type: row.get(3)?,
                original_size: row.get(4)?,
                compressed: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect()
    } else {
        stmt.query_map([], |row| {
            Ok(Backup {
                id: row.get(0)?,
                session_id: row.get(1)?,
                backup_path: row.get(2)?,
                backup_type: row.get(3)?,
                original_size: row.get(4)?,
                compressed: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).map_err(|e| format!("DB error: {}", e))?
        .filter_map(|r| r.ok())
        .collect()
    };

    Ok(backups)
}

#[tauri::command]
pub fn delete_backup(
    db: State<'_, Arc<Database>>,
    backup_id: i64,
) -> Result<(), String> {
    let conn = db.conn();
    let backup_path: String = conn.query_row(
        "SELECT backup_path FROM backups WHERE id = ?1",
        params![backup_id],
        |row| row.get(0),
    ).map_err(|e| format!("Backup not found: {}", e))?;

    std::fs::remove_file(&backup_path).ok();
    conn.execute("DELETE FROM backups WHERE id = ?1", params![backup_id])
        .map_err(|e| format!("DB error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_backup_config_cmd(db: State<'_, Arc<Database>>) -> Result<BackupConfig, String> {
    Ok(backup::get_backup_config(&db))
}

#[tauri::command]
pub fn set_backup_config_cmd(
    db: State<'_, Arc<Database>>,
    config: BackupConfig,
) -> Result<(), String> {
    backup::save_backup_config(&db, &config)
}
```

- [ ] **Step 4: Update `src-tauri/src/commands/mod.rs`**

```rust
pub mod projects;
pub mod sessions;
pub mod scan;
pub mod favorites;
pub mod tags;
pub mod backups;
```

- [ ] **Step 5: Register module and commands in `lib.rs`**

Add `mod backup;` to the top. Add to `generate_handler!`:

```rust
commands::backups::backup_session,
commands::backups::backup_all_sessions,
commands::backups::restore_session_backup,
commands::backups::list_backups,
commands::backups::delete_backup,
commands::backups::get_backup_config_cmd,
commands::backups::set_backup_config_cmd,
```

- [ ] **Step 6: Verify compilation**

```bash
cd claude-session-manager
npm run tauri dev
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/backup/ src-tauri/src/commands/backups.rs
git commit -m "feat: add backup engine with compress, restore, and config"
```

---

## Task 8: React — TypeScript Types & Tauri IPC Wrappers

**Files:**
- Create: `src/lib/types.ts`
- Create: `src/lib/tauri.ts`
- Create: `src/lib/format.ts`

- [ ] **Step 1: Write `src/lib/types.ts`**

```typescript
export interface Project {
  id: number;
  encodedPath: string;
  originalPath: string;
  displayName: string;
  sessionCount: number;
  lastActive: number | null;
}

export interface SessionSummary {
  id: number;
  sessionId: string;
  projectId: number;
  projectName: string;
  slug: string | null;
  version: string | null;
  permissionMode: string | null;
  gitBranch: string | null;
  startedAt: number | null;
  lastActive: number | null;
  messageCount: number;
  userMsgCount: number;
  assistantMsgCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  fileSize: number;
  isFavorited: boolean;
  isBackedUp: boolean;
  tags: Tag[];
}

export interface Tag {
  id: number;
  name: string;
  color: string;
}

export interface Backup {
  id: number;
  sessionId: number;
  backupPath: string;
  backupType: string;
  originalSize: number;
  compressed: boolean;
  createdAt: number;
}

export interface BackupConfig {
  enabled: boolean;
  backupDir: string;
  autoBackup: boolean;
  autoBackupIntervalHours: number;
  compress: boolean;
  maxBackupCopies: number;
}

export interface SubagentSummary {
  id: number;
  sessionId: number;
  agentId: string;
  agentType: string;
  description: string;
}

export interface ScanResult {
  projectsFound: number;
  sessionsFound: number;
  sessionsUpdated: number;
  sessionsRemoved: number;
  durationMs: number;
}

// Message types from parser
export interface ContentBlock {
  type: "text" | "thinking" | "tool_use" | "tool_result";
  // text
  text?: string;
  // thinking
  thinking?: string;
  // tool_use
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  // tool_result
  toolUseId?: string;
  content?: unknown;
  isError?: boolean;
}

export interface Usage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

export type ParsedMessage =
  | { type: "user"; uuid: string; parentUuid: string | null; timestamp: string | null; content: ContentBlock[] }
  | { type: "assistant"; uuid: string; parentUuid: string | null; timestamp: string | null; model: string | null; content: ContentBlock[]; usage: Usage | null; stopReason: string | null }
  | { type: "system"; uuid: string | null; timestamp: string | null; subtype: string | null; content: string | null }
  | { type: "attachment"; attachmentType: string }
  | { type: "permissionMode"; mode: string }
  | { type: "fileHistorySnapshot" };
```

- [ ] **Step 2: Write `src/lib/tauri.ts`**

```typescript
import { invoke } from "@tauri-apps/api/core";
import type {
  Project, SessionSummary, ParsedMessage, SubagentSummary,
  Tag, Backup, BackupConfig, ScanResult,
} from "./types";

// Projects
export const listProjects = (sortBy?: string) =>
  invoke<Project[]>("list_projects", { sortBy });

// Sessions
export const listSessions = (params: {
  projectId?: number;
  tagId?: number;
  favorited?: boolean;
  sortBy?: string;
}) => invoke<SessionSummary[]>("list_sessions", params);

export const getMessages = (sessionId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_messages", { sessionId, offset, limit });

export const getSubagents = (sessionId: number) =>
  invoke<SubagentSummary[]>("get_subagents", { sessionId });

export const getSubagentMessages = (subagentId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_subagent_messages", { subagentId, offset, limit });

// Favorites
export const toggleFavorite = (sessionId: number, note?: string) =>
  invoke<boolean>("toggle_favorite", { sessionId, note });

// Tags
export const createTag = (name: string, color: string) =>
  invoke<Tag>("create_tag", { name, color });

export const deleteTag = (tagId: number) =>
  invoke<void>("delete_tag", { tagId });

export const listTags = () =>
  invoke<Tag[]>("list_tags");

export const tagSession = (sessionId: number, tagId: number) =>
  invoke<void>("tag_session", { sessionId, tagId });

export const untagSession = (sessionId: number, tagId: number) =>
  invoke<void>("untag_session", { sessionId, tagId });

// Backups
export const backupSession = (sessionId: number) =>
  invoke<Backup>("backup_session", { sessionId });

export const backupAllSessions = () =>
  invoke<Backup[]>("backup_all_sessions");

export const restoreSessionBackup = (backupId: number) =>
  invoke<void>("restore_session_backup", { backupId });

export const listBackups = (sessionId?: number) =>
  invoke<Backup[]>("list_backups", { sessionId });

export const deleteBackup = (backupId: number) =>
  invoke<void>("delete_backup", { backupId });

export const getBackupConfig = () =>
  invoke<BackupConfig>("get_backup_config_cmd");

export const setBackupConfig = (config: BackupConfig) =>
  invoke<void>("set_backup_config_cmd", { config });

// Scanning
export const refreshIndex = () =>
  invoke<ScanResult>("refresh_index");
```

- [ ] **Step 3: Write `src/lib/format.ts`**

```typescript
export function formatRelativeTime(timestampMs: number | null): string {
  if (!timestampMs) return "Unknown";
  const diff = Date.now() - timestampMs;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(timestampMs).toLocaleDateString();
}

export function formatTokens(count: number): string {
  if (count < 1000) return `${count}`;
  if (count < 1_000_000) return `${(count / 1000).toFixed(1)}K`;
  return `${(count / 1_000_000).toFixed(1)}M`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

export function formatDateTime(timestampMs: number | null): string {
  if (!timestampMs) return "Unknown";
  return new Date(timestampMs).toLocaleString();
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/
git commit -m "feat: add TypeScript types, Tauri IPC wrappers, and formatters"
```

---

## Task 9: React — Zustand Stores & Layout Shell

**Files:**
- Create: `src/stores/appStore.ts`
- Create: `src/stores/filterStore.ts`
- Create: `src/components/layout/Sidebar.tsx`
- Create: `src/components/layout/MainContent.tsx`
- Modify: `src/App.tsx`
- Modify: `src/main.tsx`

- [ ] **Step 1: Write `src/stores/appStore.ts`**

```typescript
import { create } from "zustand";

type View = "projects" | "sessions" | "conversation" | "favorites" | "backups" | "settings";

interface AppState {
  view: View;
  selectedProjectId: number | null;
  selectedSessionId: number | null;
  sidebarCollapsed: boolean;
  setView: (view: View) => void;
  selectProject: (id: number | null) => void;
  selectSession: (id: number | null) => void;
  toggleSidebar: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  view: "projects",
  selectedProjectId: null,
  selectedSessionId: null,
  sidebarCollapsed: false,
  setView: (view) => set({ view }),
  selectProject: (id) => set({ selectedProjectId: id, selectedSessionId: null, view: "sessions" }),
  selectSession: (id) => set({ selectedSessionId: id, view: id ? "conversation" : "sessions" }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
```

- [ ] **Step 2: Write `src/stores/filterStore.ts`**

```typescript
import { create } from "zustand";

interface FilterState {
  sortBy: string;
  selectedTagId: number | null;
  setSortBy: (sort: string) => void;
  setSelectedTagId: (id: number | null) => void;
}

export const useFilterStore = create<FilterState>((set) => ({
  sortBy: "time",
  selectedTagId: null,
  setSortBy: (sortBy) => set({ sortBy }),
  setSelectedTagId: (selectedTagId) => set({ selectedTagId }),
}));
```

- [ ] **Step 3: Write `src/components/layout/Sidebar.tsx`**

```tsx
import { useEffect, useState } from "react";
import { useAppStore } from "../../stores/appStore";
import { useFilterStore } from "../../stores/filterStore";
import { listProjects, listTags } from "../../lib/tauri";
import type { Project, Tag } from "../../lib/types";

export function Sidebar() {
  const { view, setView, selectedProjectId, selectProject } = useAppStore();
  const { selectedTagId, setSelectedTagId } = useFilterStore();
  const [projects, setProjects] = useState<Project[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);

  useEffect(() => {
    listProjects("time").then(setProjects).catch(console.error);
    listTags().then(setTags).catch(console.error);
  }, []);

  return (
    <aside className="w-60 h-full border-r border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900 flex flex-col overflow-hidden">
      {/* Navigation */}
      <nav className="p-3 space-y-1">
        <button
          onClick={() => { setView("projects"); selectProject(null); }}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "projects" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          All Projects
        </button>
        <button
          onClick={() => setView("favorites")}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "favorites" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Favorites
        </button>
      </nav>

      {/* Tags */}
      {tags.length > 0 && (
        <div className="px-3 py-2">
          <h3 className="text-xs font-medium text-zinc-500 uppercase tracking-wide mb-1">Tags</h3>
          <div className="space-y-0.5">
            {tags.map((tag) => (
              <button
                key={tag.id}
                onClick={() => {
                  setSelectedTagId(selectedTagId === tag.id ? null : tag.id);
                  setView("sessions");
                }}
                className={`w-full text-left px-3 py-1 rounded text-sm flex items-center gap-2 ${
                  selectedTagId === tag.id ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
                }`}
              >
                <span className="w-2 h-2 rounded-full" style={{ backgroundColor: tag.color }} />
                {tag.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Projects */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <h3 className="text-xs font-medium text-zinc-500 uppercase tracking-wide mb-1">Projects</h3>
        <div className="space-y-0.5">
          {projects.map((p) => (
            <button
              key={p.id}
              onClick={() => selectProject(p.id)}
              className={`w-full text-left px-3 py-1 rounded text-sm truncate ${
                selectedProjectId === p.id ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
              }`}
              title={p.originalPath}
            >
              {p.displayName}
              <span className="text-zinc-400 ml-1 text-xs">{p.sessionCount}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Bottom */}
      <div className="p-3 border-t border-zinc-200 dark:border-zinc-800 space-y-1">
        <button
          onClick={() => setView("backups")}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "backups" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Backups
        </button>
        <button
          onClick={() => setView("settings")}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "settings" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Settings
        </button>
      </div>
    </aside>
  );
}
```

- [ ] **Step 4: Write `src/components/layout/MainContent.tsx`**

```tsx
import { useAppStore } from "../../stores/appStore";
import { ProjectList } from "../project/ProjectList";
import { SessionList } from "../session/SessionList";
import { ConversationView } from "../session/ConversationView";
import { BackupManager } from "../backup/BackupManager";

export function MainContent() {
  const { view } = useAppStore();

  switch (view) {
    case "projects":
      return <ProjectList />;
    case "sessions":
      return <SessionList />;
    case "conversation":
      return <ConversationView />;
    case "favorites":
      return <SessionList favoritesOnly />;
    case "backups":
      return <BackupManager />;
    case "settings":
      return <div className="p-6 text-zinc-500">Settings — coming soon</div>;
    default:
      return <ProjectList />;
  }
}
```

- [ ] **Step 5: Update `src/App.tsx`**

```tsx
import { Sidebar } from "./components/layout/Sidebar";
import { MainContent } from "./components/layout/MainContent";

export default function App() {
  return (
    <div className="flex h-screen bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      <Sidebar />
      <main className="flex-1 overflow-hidden">
        <MainContent />
      </main>
    </div>
  );
}
```

- [ ] **Step 6: Update `src/main.tsx`**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
```

- [ ] **Step 7: Create stub components so it compiles**

Create empty placeholder files that will be fleshed out in subsequent tasks:

`src/components/project/ProjectList.tsx`:
```tsx
export function ProjectList() {
  return <div className="p-6">Projects loading...</div>;
}
```

`src/components/session/SessionList.tsx`:
```tsx
export function SessionList({ favoritesOnly }: { favoritesOnly?: boolean }) {
  return <div className="p-6">Sessions loading...</div>;
}
```

`src/components/session/ConversationView.tsx`:
```tsx
export function ConversationView() {
  return <div className="p-6">Conversation loading...</div>;
}
```

`src/components/backup/BackupManager.tsx`:
```tsx
export function BackupManager() {
  return <div className="p-6">Backups loading...</div>;
}
```

Create the directories:
```bash
mkdir -p src/components/{layout,project,session,backup,message,common}
```

- [ ] **Step 8: Verify it runs**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Window opens with sidebar showing projects from your `~/.claude/`. Clicking items changes the view (stub text).

- [ ] **Step 9: Commit**

```bash
git add src/
git commit -m "feat: add layout shell with sidebar, stores, and navigation"
```

---

## Task 10: React — Project List & Session List

**Files:**
- Modify: `src/components/project/ProjectList.tsx`
- Create: `src/components/project/ProjectCard.tsx`
- Modify: `src/components/session/SessionList.tsx`
- Create: `src/components/session/SessionCard.tsx`
- Create: `src/components/common/TagBadge.tsx`
- Create: `src/components/common/FavoriteButton.tsx`

- [ ] **Step 1: Write `src/components/common/TagBadge.tsx`**

```tsx
import type { Tag } from "../../lib/types";

export function TagBadge({ tag }: { tag: Tag }) {
  return (
    <span
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium"
      style={{
        backgroundColor: tag.color + "20",
        color: tag.color,
      }}
    >
      <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: tag.color }} />
      {tag.name}
    </span>
  );
}
```

- [ ] **Step 2: Write `src/components/common/FavoriteButton.tsx`**

```tsx
import { useState } from "react";
import { toggleFavorite } from "../../lib/tauri";

export function FavoriteButton({
  sessionId,
  initialFavorited,
  onToggle,
}: {
  sessionId: number;
  initialFavorited: boolean;
  onToggle?: (favorited: boolean) => void;
}) {
  const [favorited, setFavorited] = useState(initialFavorited);

  const handleClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const result = await toggleFavorite(sessionId);
    setFavorited(result);
    onToggle?.(result);
  };

  return (
    <button onClick={handleClick} className="text-lg hover:scale-110 transition-transform" title={favorited ? "Remove from favorites" : "Add to favorites"}>
      {favorited ? "★" : "☆"}
    </button>
  );
}
```

- [ ] **Step 3: Write `src/components/project/ProjectCard.tsx`**

```tsx
import type { Project } from "../../lib/types";
import { formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

export function ProjectCard({ project }: { project: Project }) {
  const selectProject = useAppStore((s) => s.selectProject);

  return (
    <button
      onClick={() => selectProject(project.id)}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="font-medium">{project.displayName}</div>
      <div className="text-sm text-zinc-500 truncate mt-0.5">{project.originalPath}</div>
      <div className="text-xs text-zinc-400 mt-2">
        {project.sessionCount} sessions · {formatRelativeTime(project.lastActive)}
      </div>
    </button>
  );
}
```

- [ ] **Step 4: Update `src/components/project/ProjectList.tsx`**

```tsx
import { useEffect, useState } from "react";
import { listProjects, refreshIndex } from "../../lib/tauri";
import type { Project } from "../../lib/types";
import { ProjectCard } from "./ProjectCard";

export function ProjectList() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    const data = await listProjects("time");
    setProjects(data);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleRefresh = async () => {
    await refreshIndex();
    await load();
  };

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold">Projects</h1>
        <button
          onClick={handleRefresh}
          className="text-sm px-3 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          Refresh
        </button>
      </div>
      {loading ? (
        <div className="text-zinc-500">Scanning sessions...</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {projects.map((p) => (
            <ProjectCard key={p.id} project={p} />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Write `src/components/session/SessionCard.tsx`**

```tsx
import type { SessionSummary } from "../../lib/types";
import { formatRelativeTime, formatTokens, formatFileSize } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";
import { FavoriteButton } from "../common/FavoriteButton";
import { TagBadge } from "../common/TagBadge";

export function SessionCard({ session }: { session: SessionSummary }) {
  const selectSession = useAppStore((s) => s.selectSession);

  return (
    <button
      onClick={() => selectSession(session.id)}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="flex items-start justify-between">
        <div className="font-medium truncate flex-1">
          {session.slug || session.sessionId.slice(0, 8)}
        </div>
        <div className="flex items-center gap-2 ml-2 shrink-0">
          <span className="text-xs text-zinc-400">{formatRelativeTime(session.lastActive)}</span>
          <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
        </div>
      </div>
      <div className="text-sm text-zinc-500 mt-0.5">
        {session.projectName} · {session.gitBranch || "—"} · {session.version || "—"}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {session.messageCount} messages · {formatTokens(session.totalInputTokens + session.totalOutputTokens)} tokens · {formatFileSize(session.fileSize)}
      </div>
      {session.tags.length > 0 && (
        <div className="flex gap-1 mt-2 flex-wrap">
          {session.tags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} />
          ))}
        </div>
      )}
    </button>
  );
}
```

- [ ] **Step 6: Update `src/components/session/SessionList.tsx`**

```tsx
import { useEffect, useState } from "react";
import { listSessions } from "../../lib/tauri";
import type { SessionSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { useFilterStore } from "../../stores/filterStore";
import { SessionCard } from "./SessionCard";

export function SessionList({ favoritesOnly }: { favoritesOnly?: boolean }) {
  const { selectedProjectId } = useAppStore();
  const { sortBy, selectedTagId } = useFilterStore();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    listSessions({
      projectId: selectedProjectId ?? undefined,
      tagId: selectedTagId ?? undefined,
      favorited: favoritesOnly || undefined,
      sortBy,
    })
      .then(setSessions)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [selectedProjectId, selectedTagId, favoritesOnly, sortBy]);

  const title = favoritesOnly ? "Favorites" : "Sessions";

  return (
    <div className="p-6 h-full overflow-y-auto">
      <h1 className="text-xl font-semibold mb-4">{title}</h1>
      {loading ? (
        <div className="text-zinc-500">Loading...</div>
      ) : sessions.length === 0 ? (
        <div className="text-zinc-500">No sessions found.</div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <SessionCard key={s.id} session={s} />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 7: Verify it runs**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Project grid shows your projects with session counts. Clicking a project shows session cards with slug, branch, token counts. Favorite star toggles.

- [ ] **Step 8: Commit**

```bash
git add src/components/
git commit -m "feat: add project list and session list with favorites and tags"
```

---

## Task 11: React — Conversation View (Message Rendering)

**Files:**
- Modify: `src/components/session/ConversationView.tsx`
- Create: `src/components/session/SessionHeader.tsx`
- Create: `src/components/message/MessageBubble.tsx`
- Create: `src/components/message/ToolCallBlock.tsx`
- Create: `src/components/message/ThinkingBlock.tsx`
- Create: `src/components/message/CodeBlock.tsx`
- Create: `src/components/message/DiffView.tsx`
- Create: `src/components/message/SubagentView.tsx`

- [ ] **Step 1: Write `src/components/message/CodeBlock.tsx`**

```tsx
import { useEffect, useState } from "react";
import { codeToHtml } from "shiki";

export function CodeBlock({ code, language }: { code: string; language?: string }) {
  const [html, setHtml] = useState<string>("");

  useEffect(() => {
    codeToHtml(code, {
      lang: language || "text",
      theme: "github-dark",
    })
      .then(setHtml)
      .catch(() => setHtml(`<pre><code>${escapeHtml(code)}</code></pre>`));
  }, [code, language]);

  if (!html) {
    return <pre className="p-3 bg-zinc-900 rounded text-sm text-zinc-300 overflow-x-auto"><code>{code}</code></pre>;
  }

  return (
    <div
      className="rounded overflow-x-auto text-sm [&_pre]:p-3 [&_pre]:!bg-zinc-900"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
```

- [ ] **Step 2: Write `src/components/message/ThinkingBlock.tsx`**

```tsx
import { useState } from "react";

export function ThinkingBlock({ thinking }: { thinking: string }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-3 py-2 text-xs text-zinc-500 bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-1"
      >
        <span className="font-mono">{expanded ? "▼" : "▶"}</span>
        Thinking ({thinking.length} chars)
      </button>
      {expanded && (
        <div className="p-3 text-sm text-zinc-600 dark:text-zinc-400 whitespace-pre-wrap max-h-96 overflow-y-auto bg-zinc-50 dark:bg-zinc-900/50">
          {thinking}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Write `src/components/message/ToolCallBlock.tsx`**

```tsx
import { useState } from "react";
import { CodeBlock } from "./CodeBlock";
import type { ContentBlock } from "../../lib/types";

export function ToolCallBlock({ block }: { block: ContentBlock }) {
  const [expanded, setExpanded] = useState(false);

  const toolName = block.name || "Unknown";
  const input = block.input || {};

  // Extract display info based on tool type
  let summary = "";
  let codeContent = "";
  let language = "text";

  switch (toolName) {
    case "Bash": {
      summary = (input as { command?: string }).command?.split("\n")[0] || "";
      codeContent = (input as { command?: string }).command || "";
      language = "bash";
      break;
    }
    case "Read": {
      summary = (input as { file_path?: string }).file_path || "";
      break;
    }
    case "Edit": {
      summary = (input as { file_path?: string }).file_path || "";
      break;
    }
    case "Write": {
      summary = (input as { file_path?: string }).file_path || "";
      codeContent = (input as { content?: string }).content || "";
      break;
    }
    case "Grep": {
      summary = `"${(input as { pattern?: string }).pattern || ""}"`;
      break;
    }
    case "Glob": {
      summary = (input as { pattern?: string }).pattern || "";
      break;
    }
    case "Agent": {
      summary = (input as { description?: string }).description || "";
      break;
    }
    default: {
      summary = JSON.stringify(input).slice(0, 100);
    }
  }

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-3 py-2 text-sm bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "▼" : "▶"}</span>
        <span className="font-medium text-blue-600 dark:text-blue-400">{toolName}</span>
        <span className="text-zinc-500 truncate">{summary}</span>
      </button>
      {expanded && (
        <div className="p-3 space-y-2">
          {codeContent && <CodeBlock code={codeContent} language={language} />}
          {!codeContent && (
            <pre className="text-xs text-zinc-500 overflow-x-auto">
              {JSON.stringify(input, null, 2)}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Write `src/components/message/DiffView.tsx`**

```tsx
import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";

export function DiffView({
  oldString,
  newString,
  filePath,
}: {
  oldString: string;
  newString: string;
  filePath: string;
}) {
  return (
    <div className="rounded-lg overflow-hidden border border-zinc-200 dark:border-zinc-800 my-1">
      <div className="px-3 py-1.5 text-xs text-zinc-500 bg-zinc-50 dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800">
        {filePath}
      </div>
      <ReactDiffViewer
        oldValue={oldString}
        newValue={newString}
        splitView={false}
        compareMethod={DiffMethod.WORDS}
        useDarkTheme={true}
        styles={{
          contentText: { fontSize: "0.8rem", lineHeight: "1.4" },
        }}
      />
    </div>
  );
}
```

- [ ] **Step 5: Write `src/components/message/SubagentView.tsx`**

```tsx
import { useState } from "react";
import { getSubagentMessages } from "../../lib/tauri";
import type { ParsedMessage, SubagentSummary } from "../../lib/types";
import { MessageBubble } from "./MessageBubble";

export function SubagentView({ subagent }: { subagent: SubagentSummary }) {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [loaded, setLoaded] = useState(false);

  const handleExpand = async () => {
    if (!loaded) {
      const msgs = await getSubagentMessages(subagent.id, 0, 200);
      setMessages(msgs);
      setLoaded(true);
    }
    setExpanded(!expanded);
  };

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden my-2">
      <button
        onClick={handleExpand}
        className="w-full text-left px-3 py-2 text-sm bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "▼" : "▶"}</span>
        <span className="font-medium text-purple-600 dark:text-purple-400">Agent</span>
        <span className="text-zinc-500">[{subagent.agentType}]</span>
        <span className="text-zinc-400 truncate">{subagent.description}</span>
      </button>
      {expanded && (
        <div className="p-3 space-y-3 max-h-96 overflow-y-auto bg-zinc-50/50 dark:bg-zinc-900/50">
          {messages.map((msg, i) => (
            <MessageBubble key={i} message={msg} />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 6: Write `src/components/message/MessageBubble.tsx`**

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ParsedMessage, ContentBlock } from "../../lib/types";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallBlock } from "./ToolCallBlock";
import { DiffView } from "./DiffView";
import { CodeBlock } from "./CodeBlock";

function renderContentBlock(block: ContentBlock, index: number) {
  switch (block.type) {
    case "text":
      return (
        <div key={index} className="prose dark:prose-invert prose-sm max-w-none">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children, ...props }) {
                const match = /language-(\w+)/.exec(className || "");
                const code = String(children).replace(/\n$/, "");
                if (match) {
                  return <CodeBlock code={code} language={match[1]} />;
                }
                return <code className="bg-zinc-100 dark:bg-zinc-800 px-1 py-0.5 rounded text-sm" {...props}>{children}</code>;
              },
            }}
          >
            {block.text || ""}
          </ReactMarkdown>
        </div>
      );

    case "thinking":
      return <ThinkingBlock key={index} thinking={block.thinking || ""} />;

    case "tool_use": {
      // Special case: Edit tool — show diff
      if (block.name === "Edit" && block.input) {
        const input = block.input as { file_path?: string; old_string?: string; new_string?: string };
        if (input.old_string && input.new_string) {
          return (
            <DiffView
              key={index}
              filePath={input.file_path || ""}
              oldString={input.old_string}
              newString={input.new_string}
            />
          );
        }
      }
      return <ToolCallBlock key={index} block={block} />;
    }

    default:
      return null;
  }
}

export function MessageBubble({ message }: { message: ParsedMessage }) {
  if (message.type === "permissionMode" || message.type === "fileHistorySnapshot" || message.type === "attachment") {
    return null;
  }

  if (message.type === "system") {
    if (!message.content) return null;
    return (
      <div className="text-xs text-zinc-400 italic py-1">
        {message.subtype && <span className="font-medium">[{message.subtype}]</span>} {message.content}
      </div>
    );
  }

  const isUser = message.type === "user";

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[85%] rounded-lg p-3 space-y-2 ${
          isUser
            ? "bg-blue-600 text-white prose-invert"
            : "bg-zinc-100 dark:bg-zinc-800"
        }`}
      >
        <div className="text-xs font-medium opacity-60 mb-1">
          {isUser ? "You" : `Claude${message.type === "assistant" && message.model ? ` (${message.model})` : ""}`}
        </div>
        {message.content.map((block, i) => renderContentBlock(block, i))}
      </div>
    </div>
  );
}
```

- [ ] **Step 7: Write `src/components/session/SessionHeader.tsx`**

```tsx
import type { SessionSummary } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize } from "../../lib/format";
import { FavoriteButton } from "../common/FavoriteButton";
import { TagBadge } from "../common/TagBadge";
import { useAppStore } from "../../stores/appStore";

export function SessionHeader({ session }: { session: SessionSummary }) {
  const selectSession = useAppStore((s) => s.selectSession);

  return (
    <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
      <div className="flex items-center gap-2">
        <button
          onClick={() => selectSession(null)}
          className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          ← Back
        </button>
        <div className="flex-1" />
        <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
      </div>
      <h1 className="text-lg font-semibold mt-2">
        {session.slug || session.sessionId.slice(0, 8)}
      </h1>
      <div className="text-sm text-zinc-500 mt-0.5">
        {session.projectName} · {session.gitBranch || "—"} · {session.version || "—"} · {session.permissionMode || "default"}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {formatDateTime(session.startedAt)} · {session.messageCount} msgs · {formatTokens(session.totalInputTokens + session.totalOutputTokens)} tokens · {formatFileSize(session.fileSize)}
        {session.isBackedUp && " · Backed up"}
      </div>
      {session.tags.length > 0 && (
        <div className="flex gap-1 mt-2">
          {session.tags.map((tag) => <TagBadge key={tag.id} tag={tag} />)}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 8: Update `src/components/session/ConversationView.tsx`**

```tsx
import { useEffect, useState } from "react";
import { getMessages, getSubagents, listSessions } from "../../lib/tauri";
import type { ParsedMessage, SessionSummary, SubagentSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { SessionHeader } from "./SessionHeader";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";

export function ConversationView() {
  const { selectedSessionId } = useAppStore();
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  useEffect(() => {
    if (!selectedSessionId) return;
    setLoading(true);
    setOffset(0);
    setMessages([]);

    Promise.all([
      listSessions({ projectId: undefined }).then((sessions) =>
        sessions.find((s) => s.id === selectedSessionId) || null
      ),
      getMessages(selectedSessionId, 0, 50),
      getSubagents(selectedSessionId),
    ]).then(([sess, msgs, subs]) => {
      setSession(sess);
      setMessages(msgs);
      setSubagents(subs);
      setHasMore(msgs.length === 50);
      setOffset(50);
      setLoading(false);
    });
  }, [selectedSessionId]);

  const loadMore = async () => {
    if (!selectedSessionId || !hasMore) return;
    const more = await getMessages(selectedSessionId, offset, 50);
    setMessages((prev) => [...prev, ...more]);
    setHasMore(more.length === 50);
    setOffset((prev) => prev + 50);
  };

  if (loading || !session) {
    return <div className="p-6 text-zinc-500">Loading conversation...</div>;
  }

  return (
    <div className="h-full flex flex-col">
      <SessionHeader session={session} />
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.map((msg, i) => (
          <MessageBubble key={i} message={msg} />
        ))}
        {hasMore && (
          <button
            onClick={loadMore}
            className="w-full py-2 text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
          >
            Load more messages...
          </button>
        )}
        {subagents.length > 0 && (
          <div className="mt-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
            <h3 className="text-sm font-medium text-zinc-500 mb-2">Subagents ({subagents.length})</h3>
            {subagents.map((sa) => (
              <SubagentView key={sa.id} subagent={sa} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 9: Verify it runs**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Click a project → see sessions → click a session → see full conversation with user/assistant bubbles, thinking blocks (collapsible), tool calls, code highlighting, diffs for Edit tools.

- [ ] **Step 10: Commit**

```bash
git add src/components/
git commit -m "feat: add conversation view with message rendering, tool calls, thinking, diffs"
```

---

## Task 12: React — Backup Manager UI

**Files:**
- Modify: `src/components/backup/BackupManager.tsx`
- Create: `src/components/backup/BackupConfigPanel.tsx`

- [ ] **Step 1: Write `src/components/backup/BackupConfigPanel.tsx`**

```tsx
import { useState, useEffect } from "react";
import { getBackupConfig, setBackupConfig } from "../../lib/tauri";
import type { BackupConfig } from "../../lib/types";

export function BackupConfigPanel() {
  const [config, setConfig] = useState<BackupConfig | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getBackupConfig().then(setConfig);
  }, []);

  const save = async () => {
    if (!config) return;
    setSaving(true);
    await setBackupConfig(config);
    setSaving(false);
  };

  if (!config) return <div className="text-zinc-500">Loading config...</div>;

  return (
    <div className="space-y-4 p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg">
      <h3 className="font-medium">Backup Configuration</h3>

      <label className="flex items-center gap-2">
        <input type="checkbox" checked={config.autoBackup} onChange={(e) => setConfig({ ...config, autoBackup: e.target.checked })} />
        <span className="text-sm">Auto-backup every {config.autoBackupIntervalHours}h</span>
      </label>

      <label className="flex items-center gap-2">
        <input type="checkbox" checked={config.compress} onChange={(e) => setConfig({ ...config, compress: e.target.checked })} />
        <span className="text-sm">Compress backups (zstd)</span>
      </label>

      <div>
        <label className="text-sm text-zinc-500">Backup directory</label>
        <input
          type="text"
          value={config.backupDir}
          onChange={(e) => setConfig({ ...config, backupDir: e.target.value })}
          className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-transparent text-sm"
        />
      </div>

      <div>
        <label className="text-sm text-zinc-500">Max backup copies per session</label>
        <input
          type="number"
          value={config.maxBackupCopies}
          onChange={(e) => setConfig({ ...config, maxBackupCopies: parseInt(e.target.value) || 3 })}
          className="w-24 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-transparent text-sm"
          min={1}
          max={99}
        />
      </div>

      <button
        onClick={save}
        disabled={saving}
        className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
      >
        {saving ? "Saving..." : "Save Config"}
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Update `src/components/backup/BackupManager.tsx`**

```tsx
import { useEffect, useState } from "react";
import { listBackups, backupAllSessions, restoreSessionBackup, deleteBackup } from "../../lib/tauri";
import type { Backup } from "../../lib/types";
import { formatDateTime, formatFileSize } from "../../lib/format";
import { BackupConfigPanel } from "./BackupConfigPanel";

export function BackupManager() {
  const [backups, setBackups] = useState<Backup[]>([]);
  const [loading, setLoading] = useState(true);
  const [backing, setBacking] = useState(false);

  const load = async () => {
    setLoading(true);
    const data = await listBackups();
    setBackups(data);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleBackupAll = async () => {
    setBacking(true);
    await backupAllSessions();
    await load();
    setBacking(false);
  };

  const handleRestore = async (backupId: number) => {
    if (!confirm("Restore this backup? It will copy the session back to ~/.claude/projects/.")) return;
    await restoreSessionBackup(backupId);
    alert("Restored successfully. Use `claude -c` in the project directory to resume.");
  };

  const handleDelete = async (backupId: number) => {
    if (!confirm("Delete this backup permanently?")) return;
    await deleteBackup(backupId);
    await load();
  };

  return (
    <div className="p-6 h-full overflow-y-auto space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Backups</h1>
        <button
          onClick={handleBackupAll}
          disabled={backing}
          className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {backing ? "Backing up..." : "Backup All Sessions"}
        </button>
      </div>

      <BackupConfigPanel />

      <div>
        <h2 className="font-medium mb-2">Backup History ({backups.length})</h2>
        {loading ? (
          <div className="text-zinc-500">Loading...</div>
        ) : backups.length === 0 ? (
          <div className="text-zinc-500">No backups yet.</div>
        ) : (
          <div className="space-y-2">
            {backups.map((b) => (
              <div key={b.id} className="flex items-center justify-between p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg text-sm">
                <div>
                  <div className="font-medium truncate max-w-lg">{b.backupPath.split("/").slice(-3).join("/")}</div>
                  <div className="text-xs text-zinc-400">
                    {formatDateTime(b.createdAt)} · {formatFileSize(b.originalSize)} original · {b.compressed ? "compressed" : "raw"} · {b.backupType}
                  </div>
                </div>
                <div className="flex gap-2 shrink-0">
                  <button onClick={() => handleRestore(b.id)} className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800">
                    Restore
                  </button>
                  <button onClick={() => handleDelete(b.id)} className="px-2 py-1 text-xs border border-red-300 dark:border-red-700 text-red-600 rounded hover:bg-red-50 dark:hover:bg-red-900/20">
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Verify it runs**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: Backups page shows config panel and backup history. "Backup All Sessions" creates compressed backups.

- [ ] **Step 4: Commit**

```bash
git add src/components/backup/
git commit -m "feat: add backup manager UI with config, backup all, restore, delete"
```

---

## Task 13: Integration — Tag Management in Session View

**Files:**
- Create: `src/components/common/TagManager.tsx`
- Modify: `src/components/session/SessionHeader.tsx`

- [ ] **Step 1: Write `src/components/common/TagManager.tsx`**

```tsx
import { useEffect, useState } from "react";
import { listTags, createTag, tagSession, untagSession } from "../../lib/tauri";
import type { Tag } from "../../lib/types";

const PRESET_COLORS = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#3b82f6", "#8b5cf6", "#ec4899"];

export function TagManager({
  sessionId,
  currentTags,
  onUpdate,
}: {
  sessionId: number;
  currentTags: Tag[];
  onUpdate: () => void;
}) {
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newColor, setNewColor] = useState(PRESET_COLORS[0]);

  useEffect(() => {
    listTags().then(setAllTags);
  }, []);

  const currentIds = new Set(currentTags.map((t) => t.id));

  const handleToggle = async (tag: Tag) => {
    if (currentIds.has(tag.id)) {
      await untagSession(sessionId, tag.id);
    } else {
      await tagSession(sessionId, tag.id);
    }
    onUpdate();
  };

  const handleCreate = async () => {
    if (!newName.trim()) return;
    const tag = await createTag(newName.trim(), newColor);
    await tagSession(sessionId, tag.id);
    setNewName("");
    setShowCreate(false);
    setAllTags((prev) => [...prev, tag]);
    onUpdate();
  };

  return (
    <div className="p-2 space-y-2 min-w-48">
      {allTags.map((tag) => (
        <button
          key={tag.id}
          onClick={() => handleToggle(tag)}
          className="w-full text-left flex items-center gap-2 px-2 py-1 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          <span className="w-3 h-3 rounded-full" style={{ backgroundColor: tag.color }} />
          <span className="flex-1">{tag.name}</span>
          {currentIds.has(tag.id) && <span>✓</span>}
        </button>
      ))}
      {showCreate ? (
        <div className="space-y-2 pt-2 border-t border-zinc-200 dark:border-zinc-800">
          <input
            type="text"
            placeholder="Tag name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            className="w-full px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm bg-transparent"
            autoFocus
          />
          <div className="flex gap-1">
            {PRESET_COLORS.map((c) => (
              <button
                key={c}
                onClick={() => setNewColor(c)}
                className={`w-5 h-5 rounded-full ${newColor === c ? "ring-2 ring-offset-1 ring-blue-500" : ""}`}
                style={{ backgroundColor: c }}
              />
            ))}
          </div>
          <div className="flex gap-1">
            <button onClick={handleCreate} className="px-2 py-1 bg-blue-600 text-white rounded text-xs">Create</button>
            <button onClick={() => setShowCreate(false)} className="px-2 py-1 text-xs text-zinc-500">Cancel</button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setShowCreate(true)}
          className="w-full text-left px-2 py-1 text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          + New tag
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Update `src/components/session/SessionHeader.tsx` to include TagManager**

Add a tag button that opens the TagManager as a popover. Add state and import:

```tsx
import { useState } from "react";
import { TagManager } from "../common/TagManager";
```

Add tag management toggle button after the FavoriteButton, and a conditional popover:

```tsx
const [showTagManager, setShowTagManager] = useState(false);

// In the header, after FavoriteButton:
<div className="relative">
  <button
    onClick={() => setShowTagManager(!showTagManager)}
    className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
  >
    Tags
  </button>
  {showTagManager && (
    <div className="absolute right-0 top-8 z-10 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-lg">
      <TagManager
        sessionId={session.id}
        currentTags={session.tags}
        onUpdate={() => setShowTagManager(false)}
      />
    </div>
  )}
</div>
```

- [ ] **Step 3: Verify it runs**

```bash
cd claude-session-manager
npm run tauri dev
```

Expected: In conversation view, clicking "Tags" shows a dropdown to add/remove/create tags.

- [ ] **Step 4: Commit**

```bash
git add src/components/
git commit -m "feat: add tag management UI in session header"
```

---

## Task 14: Final Integration & Polish

**Files:**
- Various components for final wiring

- [ ] **Step 1: Add backup button to SessionHeader**

In `src/components/session/SessionHeader.tsx`, add a backup button:

```tsx
import { backupSession } from "../../lib/tauri";

// In the header actions:
<button
  onClick={async (e) => {
    e.stopPropagation();
    await backupSession(session.id);
    alert("Session backed up!");
  }}
  className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
>
  Backup
</button>
```

- [ ] **Step 2: Add dark mode support in `index.html`**

Ensure `<html>` tag in `index.html` has `class="dark"` based on system preference. Update `index.html`:

```html
<script>
  if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    document.documentElement.classList.add('dark');
  }
</script>
```

- [ ] **Step 3: Ensure `src/App.css` has dark mode config for Tailwind**

```css
@import "tailwindcss";

@custom-variant dark (&:where(.dark, .dark *));
```

- [ ] **Step 4: Full end-to-end test**

```bash
cd claude-session-manager
npm run tauri dev
```

Test the complete flow:
1. Projects page loads with all your projects
2. Click a project → see session list with cards
3. Click a session → see full conversation with formatted messages
4. Toggle favorites
5. Create a tag and assign it to a session
6. Filter sessions by tag in sidebar
7. Go to Backups → Backup All Sessions → verify backups created
8. Check config changes persist

- [ ] **Step 5: Commit**

```bash
git add .
git commit -m "feat: final integration - backup button, dark mode, polish"
```

- [ ] **Step 6: Build release**

```bash
cd claude-session-manager
npm run tauri build
```

Expected: Produces a `.dmg` file in `src-tauri/target/release/bundle/dmg/`.

- [ ] **Step 7: Commit release config**

```bash
git add .
git commit -m "chore: verify release build"
```

---

Plan complete and saved to `docs/plans/2026-04-09-claude-session-manager.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session, batch execution with checkpoints

Which approach?