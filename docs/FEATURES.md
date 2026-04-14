# Claude Session Manager — Feature Reference

A Tauri 2 desktop app for browsing, organizing, and monitoring local Claude Code sessions.

## Core Concepts

The app reads `~/.claude/projects/` (read-only) to discover session JSONL files. All app state lives in a SQLite database at `~/Library/Application Support/claude-session-manager/index.db`. Sessions are parsed on scan and indexed for fast browsing.

---

## Session Indexing & Scanning

- Scans `~/.claude/projects/` for JSONL session files
- Incremental: only re-parses files whose size or mtime changed
- Extracts metadata: slug, version, git branch, permission mode, timestamps
- Counts messages (total, user, assistant) and token usage
- Discovers and indexes subagent sessions, accumulates their tokens into parent
- Tracks daily token usage per session (grouped by local date)
- Cleans up orphaned sessions when source files are deleted
- Decodes encoded project paths back to original filesystem paths

## Project Browsing

- Lists all discovered projects with session count and last activity
- Groups projects by common path prefix
- Star projects for quick access
- Sort by: name, session count, last active time

## Session Browsing

- List sessions per project or across all projects
- Filter by: tag, favorited, hidden/visible
- Sort by: time, file size, message count, token usage
- Session metadata: slug, version, git branch, started/last active timestamps, message counts, token totals, file size
- Auto-hide small sessions (configurable threshold, default 3 messages)

## Conversation Viewer

- Paginated message display with offset/limit loading
- Load-latest-messages mode (jump to end of conversation)
- Message types: user, assistant, system, attachment, permission mode change
- Content rendering:
  - Markdown text with syntax-highlighted code blocks (shiki)
  - Git diff visualization
  - Collapsible thinking blocks
  - Tool call/result display with structured input/output
  - Image rendering from `~/.claude/image-cache/`
- Subagent inline viewer: expand subagent conversations within parent session

## Favorites & Visibility

- Toggle favorite (star) on any session
- Toggle hide on any session
- Auto-hide: optionally hide sessions below a message count threshold
- Favorites view: browse only favorited sessions across all projects

## Tagging

- Create tags with custom name and color
- Assign multiple tags to a session
- Filter session list by tag
- Delete tags (cascades removal from all sessions)

## Backup & Restore

- Backup individual sessions or all sessions at once
- Compression support (zstd, level 3)
- Backs up main JSONL + subagent files
- Restore: decompress and copy back to `~/.claude/projects/`
- View messages directly from backup files
- Configurable:
  - Backup directory
  - Auto-backup toggle and interval (hours)
  - Compression on/off
  - Max backup copies per session (retention)
- Migrate backups between directories (moves files + updates DB paths)

## Live Session Monitoring

- Detects currently running Claude Code sessions via `~/.claude/sessions/` registry
- Background polling (10s interval) with event emission
- Displays: PID, session ID, working directory, start time, slug, git branch, message count, tokens
- Distinguishes running vs recently ended sessions (5-minute cache)
- Search/filter live sessions by: session ID, slug, project name, PID, cwd
- Live conversation view:
  - File watcher on session JSONL for real-time message updates
  - 30s fallback polling thread
  - Auto-scroll to new messages
  - Running timer showing elapsed time

## Daily Usage Tracking

- Aggregates token usage per day from all sessions
- Metrics: input tokens, output tokens, cache creation tokens, cache read tokens, user message count, session count
- Configurable time range: 7, 30, 90, 365 days
- Summary cards + daily breakdown table

## Terminal Integration

- Open terminal at any project directory
- Configurable terminal templates with `{path}` placeholder
- Multiple terminal profiles (e.g., iTerm, Terminal.app, Warp)
- Test command before saving
- Default terminal selection

## Multiplexer Integration

- Supports: zellij, tmux, or none
- Detects running multiplexer sessions
- Matches sessions to projects by working directory
- Generates attach/create commands
- Quick-access button on sessions to jump into multiplexer

## Session Copying

- Copy a session to a new project path
- Generates new UUID, rewrites JSONL with new session ID
- Copies subagent files
- Records copy provenance (copied_from_session_id, copied_at)
- Auto-indexes the copy via re-scan

## Settings Import/Export

- Export all app configuration as JSON
- Import settings from JSON string or file
- Export settings to file

## Search

- Frontend text search across sessions
- Matches against session ID, slug, project name

## Frontend Architecture

- **State**: zustand stores (app navigation, filters, live session data)
- **Virtualization**: react-virtuoso for large message lists
- **Rendering**: react-markdown + shiki for code highlighting
- **Layout**: resizable sidebar + main content area
- **IPC**: typed wrappers in `src/lib/tauri.ts` with safe fallback for non-Tauri environments
