# Session Copy to Path — Design Spec

## Overview

Allow users to copy a session to a different project path. The copied session gets a new UUID and appears under the target project in Claude Code's session discovery, enabling `claude -c` or `claude -r` in the new directory.

## Use Cases

- Project directory was moved/renamed, need to continue a session in the new location
- Want to reuse a session's conversation context in a different project
- Fork a session to try a different approach without losing the original

## Behavior

**Copy, not move.** The original session stays untouched. A new session is created under the target path with a fresh UUID.

## Data Flow

```
User clicks "Copy to path" on a session
  → Directory picker: select target project path
  → Backend:
    1. Generate new UUID
    2. Encode target path → new encoded path
    3. Create ~/.claude/projects/{new-encoded-path}/ if needed
    4. Read source JSONL line by line
    5. Replace all sessionId occurrences with new UUID
    6. Write to ~/.claude/projects/{new-encoded-path}/{new-uuid}.jsonl
    7. Copy subagent directory if exists:
       {old-encoded-path}/{old-uuid}/subagents/
       → {new-encoded-path}/{new-uuid}/subagents/
       (subagent IDs stay the same — they're internal to the session)
    8. Refresh index to pick up the new session
  → New session appears in the target project's session list
```

## DB Schema Change

Add two columns to `sessions` table:

```sql
ALTER TABLE sessions ADD COLUMN copied_from_session_id TEXT;
ALTER TABLE sessions ADD COLUMN copied_at INTEGER;
```

- `copied_from_session_id` — the source session's UUID (not the DB id). NULL for original sessions.
- `copied_at` — timestamp (millis) of when the copy was made. NULL for original sessions.

Using the UUID (not DB id) so the link survives re-indexing and works across different machines if the DB is recreated.

## JSONL Rewriting

Each line of the JSONL is a self-contained JSON object. The `sessionId` field appears in:

- `permission-mode` lines: `{"type": "permission-mode", "sessionId": "old-uuid", ...}`
- `user` messages: `{"type": "user", "sessionId": "old-uuid", ...}`
- `assistant` messages may reference it

Strategy: parse each line as `serde_json::Value`, if it has a `sessionId` string field, replace it with the new UUID. Write the modified line to the new file. This preserves all other content exactly.

## Rust Backend

### New Command: `copy_session_to_path`

```rust
#[tauri::command]
fn copy_session_to_path(
    db: State<'_, Arc<Database>>,
    session_id: i64,       // DB id of the source session
    target_path: String,   // target project directory path
) -> Result<String, String>  // returns new session UUID
```

**Implementation:**
1. Look up source session's `jsonl_path` and `session_id` (UUID) from DB
2. Generate new UUID via `uuid::Uuid::new_v4()`
3. Compute `encoded_target = encode_path(&target_path)`
4. Create directory `~/.claude/projects/{encoded_target}/`
5. Read source JSONL, rewrite `sessionId`, write to new path
6. Copy subagent directory if exists (no rewriting needed for subagent files)
7. Record in DB: update the new session's `copied_from_session_id` and `copied_at` after next index refresh
8. Trigger `scan_all` to pick up the new session
9. After scan, update the new session record with copy metadata

### Dependencies

New crate: `uuid = { version = "1", features = ["v4"] }`

## Frontend

### Session Header / SessionCard

Add a "Copy to path" button (or menu item). When clicked:

1. Open directory picker (`@tauri-apps/plugin-dialog` `open({ directory: true })`)
2. Call `copy_session_to_path(session_id, selected_path)`
3. Show success message with the new session UUID
4. Trigger refresh

### Copy Indicator

For copied sessions, show a small "Copied from {source_project}" label in the session header or card, using `copied_from_session_id` to look up the source.

## Edge Cases

| Scenario | Handling |
|----------|----------|
| Target path same as source | Allow it — creates a duplicate session in the same project (new UUID) |
| Target path doesn't exist on disk | Create the project directory under `~/.claude/projects/`, but warn that the path itself doesn't exist |
| Source JSONL is very large | Stream line-by-line, don't load entire file into memory |
| Source has subagents | Copy the entire `{session-id}/subagents/` directory tree |
| Session was already a copy | Allow — records the immediate source in `copied_from_session_id`, not the original origin |
| Copy metadata before re-index | After `scan_all`, find the new session by UUID and set `copied_from_session_id` + `copied_at` |

## Implementation Priority

1. DB migration: add `copied_from_session_id` and `copied_at` columns
2. Backend `copy_session_to_path` command
3. Frontend button + directory picker
4. Copy indicator in session display
