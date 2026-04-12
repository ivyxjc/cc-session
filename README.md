# Claude Session Manager

A native desktop app for browsing, monitoring, and managing local Claude Code sessions. Built with Tauri 2 + React + TypeScript.

The app reads `~/.claude/` data read-only, providing a rich browsing experience for conversation history that Claude Code itself does not offer.

## Features

### Session Browser
- Browse all sessions organized by project, with search and tag filtering
- Full conversation rendering: Markdown (GFM), syntax-highlighted code blocks, diff views, thinking blocks, tool call expansion with output, image display
- Subagent conversations viewable inline (expand Agent tool calls) or via the bottom Subagents panel with locate-to-source navigation
- Pagination with virtual scrolling (react-virtuoso) for smooth performance on long sessions
- Latest messages loaded first, scroll up to load history

### Live Session Monitoring
- Real-time dashboard showing all running Claude Code processes (detected via `~/.claude/sessions/{pid}.json`)
- PID liveness checking, 10-second polling with ended session retention (5 minutes)
- Click into a live session for real-time conversation streaming (fs-notify on JSONL files, 30s fallback)
- New messages appear automatically with follow-scroll; batched event processing for performance
- Filter live sessions by ID, slug, project name, or PID

### Favorites & Tags
- Star sessions, create custom tags with colors, filter by tag
- Cross-project favorited session list

### Persistent Backup
- Auto/manual backup to prevent Claude Code's 30-day cleanup
- zstd compression (typically 5-10x ratio)
- Configurable backup directory, interval, and max copies per session
- Backup viewer with message browsing

### Settings
- Backup configuration (directory, auto-backup, compression)
- Terminal integration (configurable terminal apps, open project directories)
- Locale settings

## Architecture

```
┌─────────────────────────────────────────────────┐
│                React Frontend                    │
│  Live Dashboard · Conversation View · Backups    │
│  react-virtuoso · react-markdown · shiki · zustand│
├─────────────────────────────────────────────────┤
│              Tauri IPC + Events                   │
├─────────────────────────────────────────────────┤
│                Rust Backend                       │
│  scanner · parser · monitor · backup · commands   │
│                  SQLite                            │
├─────────────────────────────────────────────────┤
│          ~/.claude/ (read-only)                   │
└─────────────────────────────────────────────────┘
```

### Backend (Rust)

| Module | Purpose |
|--------|---------|
| `scanner` | Discovers projects and sessions from `~/.claude/projects/`, incremental re-scan based on file mtime/size |
| `parser` | JSONL parsing: message types (user, assistant, system, attachment, image), content blocks (text, thinking, tool_use, tool_result, image), token usage extraction |
| `monitor` | Live session monitoring: polls `~/.claude/sessions/*.json`, PID liveness via `kill(pid, 0)`, fs-notify JSONL tail for real-time message streaming |
| `backup` | zstd compression/decompression, backup scheduling, restore to `~/.claude/` |
| `db` | SQLite: projects, sessions, favorites, tags, backups, subagents |
| `commands` | Tauri IPC command handlers for all frontend operations |

### Frontend (React + TypeScript)

| Area | Components |
|------|------------|
| Layout | Sidebar (navigation, search, live badge), MainContent (view router) |
| Session | SessionList, SessionCard, ConversationView (virtualized), SessionHeader |
| Live | LiveDashboard, LiveSessionCard, LiveConversationView (virtualized + streaming) |
| Message | MessageBubble (memo), ToolCallBlock (with output), ThinkingBlock, DiffView, CodeBlock, ImageFromPath |
| Backup | BackupManager, BackupConfig |
| Stores | appStore (navigation), filterStore (tags), liveStore (live sessions) |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop framework | Tauri 2 |
| Frontend | React 19, TypeScript 5.8, Vite 7 |
| Styling | Tailwind CSS 4, shadcn/ui |
| Markdown | react-markdown + remark-gfm |
| Code highlighting | shiki |
| Virtual scrolling | react-virtuoso |
| State management | zustand |
| Backend | Rust, rusqlite (SQLite), notify 7 (fs events), zstd, chrono |
| Platform | macOS (primary), Linux/Windows possible via Tauri |

## Development

```bash
# Install dependencies
npm install

# Development (hot-reload frontend + debug Rust backend)
npm run tauri dev

# Production build
npm run tauri build
```

Output: `src-tauri/target/release/bundle/macos/claude-session-manager.app`

## Core Constraint

The app **never writes to `~/.claude/`**. All application state (favorites, tags, backup records, metadata cache) is stored in its own SQLite database under `~/Library/Application Support/claude-session-manager/`.
