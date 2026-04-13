# Schema Consolidation: Remove v1/v2/v3 Migration Layers

**Date:** 2026-04-13
**Status:** Approved

## Goal

Consolidate the incremental database schema migrations into a single clean `CREATE TABLE` definition. Remove all legacy migration code, the dead `favorites` table, and associated dead code. The project is pre-release — no backwards compatibility required.

## Breaking Change

After applying these changes, users must delete the existing SQLite database:

```
rm ~/Library/Application\ Support/claude-session-manager/index.db
```

The app will recreate it on next launch and re-scan all sessions from `~/.claude/` JSONL files. No data is lost.

## Changes

### 1. `src-tauri/src/db/mod.rs` — Schema consolidation

**Add to `CREATE TABLE sessions`:**
- `is_favorited INTEGER DEFAULT 0`
- `is_hidden INTEGER DEFAULT 0`
- `copied_from_session_id TEXT`
- `copied_at INTEGER`

**Add to `CREATE TABLE projects`:**
- `is_starred INTEGER DEFAULT 0`

**Remove entirely:**
- `CREATE TABLE IF NOT EXISTS favorites` (lines 61-67) — superseded by `sessions.is_favorited`
- `CREATE INDEX IF NOT EXISTS idx_favorites_session` (line 120)
- All 8 `ALTER TABLE` lines (lines 127-134) — columns now in base schema
- Favorites migration logic (lines 136-140)
- `user_msg_reparse_v3` migration logic (lines 142-161)

### 2. `src-tauri/src/scanner/mod.rs` — Remove favorites cleanup

- Remove `DELETE FROM favorites WHERE session_id = ?1` in orphan session cleanup (line 345)

### 3. `src-tauri/src/commands/favorites.rs` — Remove unused parameter

- Remove `_note: Option<String>` parameter from `toggle_favorite` command signature

### 4. `src-tauri/src/lib.rs` — Update command registration

- Update `toggle_favorite` registration if signature changes require it

### 5. `src/lib/tauri.ts` — Remove unused parameter

- Remove `note?: string` parameter from `toggleFavorite` function

## Scope

This is a schema-only cleanup. No functional changes to the application behavior. The favorites feature continues to work via `sessions.is_favorited`. Tags, backups, subagents, daily token usage, and all other features remain unchanged.
