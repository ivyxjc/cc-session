# Claude Session Manager — V1 Design Spec

## Overview

A native desktop client (Tauri 2 + React) for managing local Claude Code sessions. The app reads `~/.claude/` data read-only, provides a rich browsing experience for conversation history, and adds favorites, tags, and persistent backup capabilities that Claude Code itself lacks.

## V1 Scope

| Feature | Description |
|---------|-------------|
| **Session Browser** | Browse all sessions by project/time/name. Full conversation rendering with Markdown, code highlighting, tool call folding, thinking blocks, diff view. |
| **Favorites & Tags** | Star sessions, create custom tags with colors, filter by tag. |
| **Persistent Backup** | Auto/manual backup to prevent Claude Code's 30-day cleanup. Compress with zstd. Configurable backup directory. Restore sessions back to `~/.claude/`. |

**Out of V1 scope (V2):**
- Real-time monitoring of running Claude Code processes
- Sending messages to running sessions from the client

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 React Frontend                   │
│   SessionExplorer · ConversationView · Backups   │
├─────────────────────────────────────────────────┤
│                Tauri IPC (invoke)                 │
├─────────────────────────────────────────────────┤
│                  Rust Backend                     │
│   Scanner · Parser · BackupEngine · Commands      │
│                    SQLite                          │
├─────────────────────────────────────────────────┤
│            ~/.claude/ (read-only)                 │
└─────────────────────────────────────────────────┘
```

**Core constraint:** The app never writes to `~/.claude/`. All application state (favorites, tags, backup records, metadata cache) is stored in its own SQLite database under `~/Library/Application Support/claude-session-manager/`.

## Claude Code Local Data Format

### Directory Structure

```
~/.claude/
├── projects/                    # Session transcripts by project
│   └── {encoded-path}/         # e.g. -Users-ivyxjc-myproject
│       ├── {sessionId}.jsonl   # Conversation log (append-only)
│       └── {sessionId}/
│           └── subagents/
│               ├── agent-{id}.jsonl
│               └── agent-{id}.meta.json
├── sessions/                    # Active session registry
│   └── {pid}.json              # {pid, sessionId, cwd, startedAt, kind, entrypoint}
├── history.jsonl                # Global prompt history
├── .claude.json                 # Global state (per-project stats, identity)
└── file-history/                # File backups per session
```

### Path Encoding

Project paths are encoded by replacing `/` with `-`:
- `/Users/ivyxjc/myproject` -> `-Users-ivyxjc-myproject`

### Session JSONL Format

Each line is a self-contained JSON object. Key message types:

**`permission-mode`** — First line of every session:
```json
{"type": "permission-mode", "permissionMode": "default", "sessionId": "uuid"}
```

**`user`** — User messages:
```json
{
  "type": "user",
  "uuid": "...",
  "parentUuid": "...",
  "message": {"role": "user", "content": "..." | [/* content blocks */]},
  "timestamp": "ISO-8601",
  "sessionId": "uuid",
  "version": "2.1.98",
  "gitBranch": "master",
  "cwd": "/path/to/project",
  "slug": "hidden-puzzling-floyd"
}
```

**`assistant`** — Claude responses:
```json
{
  "type": "assistant",
  "uuid": "...",
  "parentUuid": "...",
  "message": {
    "model": "claude-opus-4-6",
    "role": "assistant",
    "content": [
      {"type": "thinking", "thinking": "..."},
      {"type": "text", "text": "..."},
      {"type": "tool_use", "id": "toolu_...", "name": "Bash", "input": {...}}
    ],
    "stop_reason": "end_turn" | "tool_use",
    "usage": {
      "input_tokens": 1234,
      "output_tokens": 567,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 890
    }
  }
}
```

**`system`** — System events:
- `subtype: "turn_duration"` — End-of-turn metrics
- `subtype: "compact_boundary"` — Context compaction marker
- `subtype: "stop_hook_summary"` — Hook execution results

**`attachment`** — Injected context (deferred_tools_delta, skill_listing, task_reminder, etc.)

**`file-history-snapshot`** — File modification tracking for undo/rewind

### Subagent Format

- `agent-{id}.meta.json`: `{"agentType": "Explore", "description": "..."}`
- `agent-{id}.jsonl`: Same JSONL format as main session, with `isSidechain: true`

### Session Lifecycle

- **Creation:** UUID generated, `.jsonl` created, `{pid}.json` written to `sessions/`
- **Resumption:** `claude -c` (continue latest), `claude -r` (interactive picker)
- **Compaction:** At ~167K tokens, a `compact_boundary` is written followed by a summary
- **Cleanup:** Sessions older than 30 days auto-deleted on startup (configurable via `cleanupPeriodDays`)

## Data Model (SQLite)

```sql
CREATE TABLE projects (
  id            INTEGER PRIMARY KEY,
  encoded_path  TEXT UNIQUE,
  original_path TEXT,
  display_name  TEXT,
  session_count INTEGER DEFAULT 0,
  last_active   INTEGER,
  created_at    INTEGER
);

CREATE TABLE sessions (
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
  message_count       INTEGER,
  user_msg_count      INTEGER,
  assistant_msg_count INTEGER,
  total_input_tokens  INTEGER,
  total_output_tokens INTEGER,
  file_size           INTEGER,
  file_mtime          INTEGER,
  is_backed_up        BOOLEAN DEFAULT 0,
  created_at          INTEGER
);

CREATE TABLE favorites (
  id         INTEGER PRIMARY KEY,
  session_id INTEGER REFERENCES sessions(id),
  note       TEXT,
  created_at INTEGER,
  UNIQUE(session_id)
);

CREATE TABLE tags (
  id    INTEGER PRIMARY KEY,
  name  TEXT UNIQUE,
  color TEXT
);

CREATE TABLE session_tags (
  session_id INTEGER REFERENCES sessions(id),
  tag_id     INTEGER REFERENCES tags(id),
  PRIMARY KEY (session_id, tag_id)
);

CREATE TABLE backups (
  id            INTEGER PRIMARY KEY,
  session_id    INTEGER REFERENCES sessions(id),
  backup_path   TEXT,
  backup_type   TEXT,
  original_size INTEGER,
  compressed    BOOLEAN DEFAULT 1,
  created_at    INTEGER
);

CREATE TABLE subagents (
  id          INTEGER PRIMARY KEY,
  session_id  INTEGER REFERENCES sessions(id),
  agent_id    TEXT,
  agent_type  TEXT,
  description TEXT,
  jsonl_path  TEXT,
  created_at  INTEGER
);

-- Indexes
CREATE INDEX idx_sessions_project     ON sessions(project_id, last_active DESC);
CREATE INDEX idx_sessions_last_active ON sessions(last_active DESC);
CREATE INDEX idx_sessions_slug        ON sessions(slug);
CREATE INDEX idx_favorites_session    ON favorites(session_id);
CREATE INDEX idx_session_tags_tag     ON session_tags(tag_id);
CREATE INDEX idx_backups_session      ON backups(session_id);
CREATE INDEX idx_subagents_session    ON subagents(session_id);
```

### Incremental Scanning

1. On startup, scan `~/.claude/projects/` directory tree
2. For each `.jsonl` file, compare `file_size` + `file_mtime` against SQLite cache
3. Only re-parse changed files (new = full parse, size changed = incremental append parse)
4. Remove SQLite records for files that no longer exist on disk

## Rust Backend

### Module Structure

```
src-tauri/src/
├── main.rs
├── db/
│   ├── mod.rs              -- SQLite connection pool, migrations
│   └── models.rs           -- Rust structs (Project, Session, Tag, Backup...)
├── scanner/
│   ├── mod.rs              -- Project/session discovery, incremental scan
│   └── watcher.rs          -- (V2) fs notify
├── parser/
│   ├── mod.rs              -- JSONL parse entry
│   ├── messages.rs         -- Message type parsing
│   └── content.rs          -- Content block parsing (text/thinking/tool_use)
├── backup/
│   ├── mod.rs              -- Backup/restore engine
│   └── scheduler.rs        -- Auto-backup policy
├── commands/
│   ├── mod.rs              -- Tauri command registration
│   ├── projects.rs
│   ├── sessions.rs
│   ├── favorites.rs
│   ├── tags.rs
│   └── backups.rs
└── config.rs
```

### Key Data Structures

```rust
// Message types parsed from JSONL
enum MessageType {
    User { content: Vec<ContentBlock> },
    Assistant {
        content: Vec<ContentBlock>,
        model: String,
        usage: Usage,
        stop_reason: String,
    },
    System { subtype: String, content: String },
    Attachment { attachment_type: String },
    PermissionMode { mode: String },
    FileHistorySnapshot { ... },
}

enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}
```

### Tauri IPC Commands

```
// Projects
list_projects(filter, sort) -> Vec<Project>
get_project(id) -> ProjectDetail

// Sessions
list_sessions(project_id?, tag_id?, favorited?, sort) -> Vec<SessionSummary>
get_session(id) -> SessionDetail
get_messages(session_id, offset, limit) -> Vec<Message>
get_subagents(session_id) -> Vec<Subagent>
get_subagent_messages(subagent_id, offset, limit) -> Vec<Message>

// Favorites & Tags
toggle_favorite(session_id, note?) -> bool
list_favorites() -> Vec<SessionSummary>
create_tag(name, color) -> Tag
delete_tag(id)
tag_session(session_id, tag_id)
untag_session(session_id, tag_id)

// Backups
backup_session(session_id) -> Backup
backup_all_sessions() -> Vec<Backup>
restore_session(backup_id)
list_backups(session_id?) -> Vec<Backup>
delete_backup(backup_id)
get_backup_config() -> BackupConfig
set_backup_config(config)

// Scanning
refresh_index() -> ScanResult
```

### Backup Strategy

```rust
struct BackupConfig {
    enabled: bool,
    backup_dir: PathBuf,              // Default: ~/Library/Application Support/claude-session-manager/backups/
    auto_backup: bool,
    auto_backup_interval_hours: u32,  // Default: 24
    compress: bool,                   // zstd, default true
    max_backup_copies: u32,           // Per session, default 3
}
```

**Backup directory layout:**
```
{backup_dir}/
└── {encoded_project_path}/
    └── {session_id}/
        ├── {timestamp}.jsonl.zst
        └── subagents/
            ├── agent-{id}.jsonl.zst
            └── agent-{id}.meta.json
```

**Auto-backup flow:**
1. On app startup, check time since last backup
2. Scan all sessions, back up those that are new or modified since last backup
3. Respect `max_backup_copies` — delete oldest when exceeded

**Restore flow:**
1. Decompress `.jsonl.zst` back to `.jsonl`
2. Copy to `~/.claude/projects/{encoded_path}/`
3. `claude -c` or `claude -r` in that project will pick it up

## React Frontend

### Tech Stack

| Purpose | Library |
|---------|---------|
| UI components | shadcn/ui + Tailwind CSS |
| Markdown | react-markdown + remark-gfm |
| Code highlighting | shiki |
| Diff view | react-diff-viewer-continued |
| Virtual scrolling | @tanstack/react-virtual |
| State management | zustand |
| Routing | @tanstack/react-router |
| Tauri bridge | @tauri-apps/api |

### Layout

```
┌────────────────────────────────────────────────┐
│ Sidebar (240px)      │ Main Content             │
│                      │                          │
│ Search/Filter        │ Session List              │
│ All Sessions         │   or                     │
│ Favorites            │ Conversation View         │
│ Tags (expandable)    │   or                     │
│ Projects (tree)      │ Backup Manager            │
│ ---                  │   or                     │
│ Settings             │ Settings                  │
│ Backups              │                          │
└────────────────────────────────────────────────┘
```

### Pages

| Page | Content |
|------|---------|
| **Projects** | Project grid/list with session count, last active time |
| **Session List** | Session cards for selected project/tag/favorites. Sort by time/size. |
| **Conversation** | Full conversation rendering with metadata header |
| **Favorites** | Cross-project favorited session list |
| **Backups** | Backup config, backup list, manual backup/restore actions |
| **Settings** | Claude directory path, backup config, theme (light/dark) |

### Session Card

```
┌────────────────────────────────────────┐
│ ⭐ hidden-puzzling-floyd       2h ago  │
│ claude-exts · master · v2.1.98        │
│ 33 messages · 12.5K tokens · 260KB    │
│ [bugfix] [review]                     │
└────────────────────────────────────────┘
```

### Conversation Rendering

| Content Block | Rendering |
|---------------|-----------|
| `text` | Markdown with GFM, code blocks highlighted by shiki |
| `thinking` | Collapsible section, collapsed by default, muted background |
| `tool_use: Bash` | Terminal-style command display, output collapsible |
| `tool_use: Read` | File path header + syntax-highlighted content preview |
| `tool_use: Edit` | Side-by-side or unified diff view (old_string -> new_string) |
| `tool_use: Write` | File path + full content with syntax highlighting |
| `tool_use: Grep/Glob` | Search result list |
| `tool_use: Agent` | Subagent card with description, click to expand subagent conversation |
| `tool_result` | Nested under corresponding tool_use, collapsible |
| `system` | Small muted text (compaction boundary, turn duration) |
| `attachment` | Collapsed by default, expandable |

### Component Structure

```
src/
├── main.tsx
├── App.tsx
├── components/
│   ├── layout/
│   │   ├── Sidebar.tsx
│   │   ├── MainContent.tsx
│   │   └── Header.tsx
│   ├── session/
│   │   ├── SessionCard.tsx
│   │   ├── SessionList.tsx
│   │   └── ConversationView.tsx
│   ├── message/
│   │   ├── MessageBubble.tsx
│   │   ├── ToolCallBlock.tsx
│   │   ├── ThinkingBlock.tsx
│   │   ├── DiffView.tsx
│   │   └── SubagentView.tsx
│   ├── project/
│   │   ├── ProjectGrid.tsx
│   │   └── ProjectCard.tsx
│   ├── backup/
│   │   ├── BackupManager.tsx
│   │   └── BackupConfig.tsx
│   └── common/
│       ├── TagBadge.tsx
│       ├── FavoriteButton.tsx
│       └── FilterBar.tsx
├── hooks/
│   ├── useProjects.ts
│   ├── useSessions.ts
│   ├── useMessages.ts
│   └── useBackups.ts
├── stores/
│   ├── appStore.ts
│   └── filterStore.ts
├── lib/
│   ├── tauri.ts            -- IPC call wrappers
│   ├── format.ts           -- Date, token count, file size formatting
│   └── types.ts            -- TypeScript types matching Rust structs
└── styles/
```

## Non-Functional Requirements

- **Startup time:** Index scan should complete within 2 seconds for ~200MB of data (incremental scan after first run)
- **Message loading:** Paginated loading (50 messages per page) with virtual scrolling for smooth performance
- **Backup size:** zstd compression typically achieves 5-10x on JSONL text, so 222MB source -> ~30-40MB compressed
- **Theme:** Respect system light/dark preference, with manual override
- **Platform:** macOS first (primary user environment). Tauri supports Linux/Windows for future expansion.
