# Live Session Monitoring — V2 Design Spec

## Overview

Add real-time monitoring of running Claude Code sessions to the Session Manager app. A new "Live Dashboard" page shows all active sessions, their status, subagents, and latest messages. Clicking into a live session provides a real-time conversation view with messages appearing as they stream in.

## Scope

### In Scope (V2)
- Live Dashboard page showing all running/recently-ended sessions
- Real-time conversation view with incremental JSONL tailing
- Subagent detection and live tracking
- Running/ended status indicators with PID-based liveness checks

### Out of Scope
- Background process stdout/stderr capture (V3)
- Sending messages to running sessions (V3)
- CPU/memory resource monitoring (V3)
- fs-notify watcher for session discovery in `~/.claude/projects/` (separate V2 item)

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  React Frontend                       │
│   Live Dashboard · Live ConversationView              │
│   listen: live-sessions-update                        │
│   listen: session-messages-update                     │
│   listen: session-subagent-update                     │
├──────────────────────────────────────────────────────┤
│                Tauri IPC + Events                      │
├──────────────────────────────────────────────────────┤
│                  Rust Backend                          │
│   monitor/mod.rs (NEW)                                │
│   ├─ Poll ~/.claude/sessions/*.json  (10s interval)   │
│   ├─ PID liveness check: kill(pid, 0)                 │
│   └─ fs-notify watch on individual JSONL files        │
├──────────────────────────────────────────────────────┤
│            ~/.claude/ (read-only)                      │
│   sessions/{pid}.json  — live process registry        │
│   projects/{path}/{id}.jsonl — conversation logs      │
└──────────────────────────────────────────────────────┘
```

The new `monitor` module is independent from the existing `scanner` module. Scanner handles static JSONL indexing; monitor handles live process tracking.

## Data Source: ~/.claude/sessions/{pid}.json

Each file represents a running (or recently-exited) Claude Code process:

```json
{
  "pid": 31497,
  "sessionId": "2258455a-8e72-4410-95d3-e73ebaab01a2",
  "cwd": "/Users/ivyxjc/Zeta/SideProjects/claude-exts",
  "startedAt": 1775853776985,
  "kind": "interactive",
  "entrypoint": "cli"
}
```

## Rust Backend

### New Module: `src-tauri/src/monitor/mod.rs`

**LiveSession struct:**

```rust
pub struct LiveSession {
    pub pid: u32,
    pub session_id: String,
    pub cwd: String,
    pub started_at: i64,
    pub kind: String,           // "interactive" | "batch"
    pub entrypoint: String,     // "cli" | "sdk" | etc.
    pub is_alive: bool,         // kill(pid, 0)
    pub ended_at: Option<i64>,  // timestamp when detected as ended
    pub db_session_id: Option<i64>,  // FK to sessions table if scanned
    pub slug: Option<String>,        // from DB if available
    pub project_name: Option<String>,
    pub git_branch: Option<String>,
    pub message_count: Option<i32>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    pub last_message_preview: Option<String>,
    pub active_subagent_count: Option<i32>,
}
```

### Dashboard Polling (10s interval)

1. Scan `~/.claude/sessions/*.json`, parse each file
2. For each PID, check liveness via `kill(pid, 0)` (libc)
3. Match `session_id` against DB `sessions` table to enrich with slug, project, branch, token counts
4. For newly-dead PIDs, record `ended_at = now()`
5. Remove entries where `ended_at` is older than 5 minutes
6. Emit `live-sessions-update` Tauri event with full `Vec<LiveSession>`

### JSONL Tail (fs-notify, per-session)

When a user opens a live session's conversation view:

1. Record current file size as the read offset
2. Set up `notify` crate watcher on the specific JSONL file
3. On file change event:
   - Read from last offset to current EOF
   - Parse new lines using existing `parser` module
   - Emit `session-messages-update` event with new messages
   - Update offset
4. Fallback: every 30 seconds, compare file size to detect missed events (macOS fs-notify can drop events)

### Subagent Detection

During incremental JSONL parsing:

1. When a `tool_use` with `name: "Agent"` is encountered, note it
2. Check the session's subagent directory for new `agent-{id}.meta.json` files
3. Parse meta and emit `session-subagent-update` event
4. If the user has expanded a subagent view, watch that subagent's JSONL file with the same tail mechanism

## Tauri IPC Commands

| Command | Parameters | Returns | Description |
|---------|-----------|---------|-------------|
| `get_live_sessions` | — | `Vec<LiveSession>` | One-shot fetch of all active/recently-ended sessions |
| `start_live_monitor` | — | — | Start 10s polling, begin emitting events |
| `stop_live_monitor` | — | — | Stop polling, clean up all watchers |
| `watch_session` | `session_id: String` | — | Enable fs-notify on session's JSONL, push new messages |
| `unwatch_session` | `session_id: String` | — | Stop watching session's JSONL |

## Tauri Events (Backend → Frontend)

| Event | Payload | Trigger |
|-------|---------|---------|
| `live-sessions-update` | `Vec<LiveSession>` | Every 10s poll cycle, or on detected change |
| `session-messages-update` | `{ session_id: String, new_messages: Vec<Message> }` | fs-notify detects JSONL write |
| `session-subagent-update` | `{ session_id: String, subagent: Subagent }` | New subagent discovered |

## React Frontend

### Sidebar Change

Add "Live" navigation item above "All Sessions" with an active session count badge:

```
Live (3)          ← new
All Sessions
Favorites
...
```

### Live Dashboard Page

```
┌─────────────────────────────────────────────────┐
│ Live Sessions (3 running · 1 ended)             │
├─────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────┐ │
│ │ 🟢 hidden-puzzling-floyd        12m ago     │ │
│ │ claude-exts · master · interactive          │ │
│ │ 45 msgs · 18.2K tokens                     │ │
│ │ 2 subagents active                          │ │
│ │ > "Let me check the test results..."        │ │
│ └─────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────┐ │
│ │ ⚫ brave-silent-turing    ended 2m ago      │ │
│ │ my-api · feature/auth · interactive         │ │
│ │ 120 msgs · 45.1K tokens                    │ │
│ └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

**Card fields:**
- Status indicator: 🟢 running / ⚫ ended
- Session slug
- Running duration (live timer) or "ended Xm ago"
- Project name + git branch + kind
- Message count + token usage (real-time)
- Active subagent count
- Last message preview (truncated)

### Live Conversation View

Reuses existing `ConversationView` with live mode additions:

- New messages append at the bottom with auto-scroll
- Running duration timer in the header
- Status badge (running/ended) in the header
- Subagent section showing active subagents, expandable to view their conversation
- Back button returns to Live Dashboard

### Data Flow

```
Enter Dashboard
  → start_live_monitor()
  → listen(live-sessions-update) → update session list

Click session card
  → watch_session(session_id)
  → load existing messages via get_messages()
  → listen(session-messages-update) → append new messages
  → listen(session-subagent-update) → update subagent list

Leave conversation view
  → unwatch_session(session_id)

Leave Dashboard
  → stop_live_monitor()
```

### New Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `LiveDashboard.tsx` | `src/components/live/` | Dashboard page, session list |
| `LiveSessionCard.tsx` | `src/components/live/` | Card with status indicator + live stats |
| `LiveConversationView.tsx` | `src/components/live/` | Wrapper around ConversationView with live mode |
| `LiveStatusBadge.tsx` | `src/components/live/` | 🟢/⚫ status indicator |
| `RunningTimer.tsx` | `src/components/live/` | Live elapsed time display |

### New Store

```typescript
// src/stores/liveStore.ts
interface LiveStore {
  liveSessions: LiveSession[];
  watchedSessionId: string | null;
  newMessages: Message[];
  setLiveSessions(sessions: LiveSession[]): void;
  appendMessages(sessionId: string, messages: Message[]): void;
  clearNewMessages(): void;
}
```

## Edge Cases

| Scenario | Handling |
|----------|----------|
| App starts with running sessions | First `get_live_sessions()` discovers them |
| Session ends while Dashboard is closed | Not shown on next open (PID dead + no ended_at record) |
| Stale `{pid}.json` (process dead, file remains) | PID liveness check returns false → mark ended |
| Same session resumed with new PID | Old entry ended, new entry running, same session_id |
| JSONL compact (compact_boundary written) | Incremental parse handles normally, shown as system message |
| fs-notify event missed (macOS) | 30s fallback file size check triggers re-read |
| Multiple sessions for same project | Each shown independently on Dashboard |

## Dependencies

### New Rust Crates
- `notify` (6.x) — fs-notify for JSONL file watching
- `libc` — for `kill(pid, 0)` liveness check (may already be available via Tauri)

### No New Frontend Dependencies
- Tauri event API already available via `@tauri-apps/api/event`
- All UI built with existing shadcn/ui + Tailwind stack
