# Schema Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate all incremental ALTER TABLE migrations into the base CREATE TABLE definitions, remove the dead `favorites` table and all migration logic, and clean up dead code references across Rust and TypeScript.

**Architecture:** The app's SQLite schema in `db/mod.rs` evolved incrementally via ALTER TABLE statements and versioned migration markers. Since the project is pre-release, we flatten everything into clean CREATE TABLE statements and drop all migration code. The `favorites` table is fully superseded by `sessions.is_favorited` and is removed along with all references.

**Tech Stack:** Rust (Tauri 2, rusqlite), TypeScript, SQLite

---

### Task 1: Consolidate database schema

**Files:**
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Add missing columns to CREATE TABLE sessions**

In `src-tauri/src/db/mod.rs`, inside the `CREATE TABLE IF NOT EXISTS sessions` block, add these four columns after `is_backed_up`:

```rust
                is_favorited        INTEGER DEFAULT 0,
                is_hidden           INTEGER DEFAULT 0,
                copied_from_session_id TEXT,
                copied_at           INTEGER,
```

The full `sessions` table columns should end up as:

```sql
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
    is_favorited        INTEGER DEFAULT 0,
    is_hidden           INTEGER DEFAULT 0,
    copied_from_session_id TEXT,
    copied_at           INTEGER,
    created_at          INTEGER
);
```

- [ ] **Step 2: Add missing column to CREATE TABLE projects**

In the same file, add `is_starred` to the `projects` table definition, after `created_at`:

```sql
CREATE TABLE IF NOT EXISTS projects (
    id            INTEGER PRIMARY KEY,
    encoded_path  TEXT UNIQUE,
    original_path TEXT,
    display_name  TEXT,
    session_count INTEGER DEFAULT 0,
    last_active   INTEGER,
    created_at    INTEGER,
    is_starred    INTEGER DEFAULT 0
);
```

- [ ] **Step 3: Remove the favorites table and its index**

Delete the entire `CREATE TABLE IF NOT EXISTS favorites` block (current lines 61-67):

```sql
            CREATE TABLE IF NOT EXISTS favorites (
                id         INTEGER PRIMARY KEY,
                session_id INTEGER REFERENCES sessions(id),
                note       TEXT,
                created_at INTEGER,
                UNIQUE(session_id)
            );
```

And delete the index line (current line 120):

```sql
            CREATE INDEX IF NOT EXISTS idx_favorites_session     ON favorites(session_id);
```

- [ ] **Step 4: Remove all migration code**

Delete everything after the `CREATE INDEX` block and before `Ok(())`. Specifically, remove all of these sections:

1. The 8 `ALTER TABLE` lines (current lines 127-134):
```rust
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_creation_tokens INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN total_cache_read_tokens INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN is_favorited INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN is_hidden INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE projects ADD COLUMN is_starred INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE daily_token_usage ADD COLUMN user_msg_count INTEGER DEFAULT 0", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN copied_from_session_id TEXT", []).ok();
        conn.execute("ALTER TABLE sessions ADD COLUMN copied_at INTEGER", []).ok();
```

2. The favorites migration block (current lines 136-140):
```rust
        // Migrate existing favorites table data into sessions.is_favorited
        conn.execute(
            "UPDATE sessions SET is_favorited = 1 WHERE id IN (SELECT session_id FROM favorites)",
            [],
        ).ok();
```

3. The entire `user_msg_reparse_v3` block (current lines 142-161):
```rust
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
```

After this step, `run_migrations()` should end with:

```rust
            CREATE INDEX IF NOT EXISTS idx_subagents_session     ON subagents(session_id);
        ")?;

        Ok(())
    }
```

- [ ] **Step 5: Verify the final state of run_migrations()**

The complete `run_migrations` method should now be:

```rust
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
                created_at    INTEGER,
                is_starred    INTEGER DEFAULT 0
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
                is_favorited        INTEGER DEFAULT 0,
                is_hidden           INTEGER DEFAULT 0,
                copied_from_session_id TEXT,
                copied_at           INTEGER,
                created_at          INTEGER
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
            CREATE INDEX IF NOT EXISTS idx_session_tags_tag      ON session_tags(tag_id);
            CREATE INDEX IF NOT EXISTS idx_backups_session       ON backups(session_id);
            CREATE INDEX IF NOT EXISTS idx_subagents_session     ON subagents(session_id);
        ")?;

        Ok(())
    }
```

- [ ] **Step 6: Build to verify compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && pnpm run tauri build --no-bundle 2>&1 | tail -20`

Expected: Compilation succeeds with no errors related to `db/mod.rs`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db/mod.rs
git commit -m "refactor(db): consolidate schema — fold ALTER TABLE columns into CREATE TABLE, remove favorites table and migration logic"
```

---

### Task 2: Remove favorites reference from scanner

**Files:**
- Modify: `src-tauri/src/scanner/mod.rs:345`

- [ ] **Step 1: Remove the DELETE FROM favorites line**

In `src-tauri/src/scanner/mod.rs`, in the orphan cleanup loop (around line 343-348), delete line 345:

```rust
        conn.execute("DELETE FROM favorites WHERE session_id = ?1", params![id]).ok();
```

The loop should become:

```rust
    for id in &orphans {
        conn.execute("DELETE FROM session_tags WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM subagents WHERE session_id = ?1", params![id]).ok();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).ok();
        sessions_removed += 1;
    }
```

- [ ] **Step 2: Build to verify**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10`

Expected: No compilation errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scanner/mod.rs
git commit -m "refactor(scanner): remove dead favorites table reference from orphan cleanup"
```

---

### Task 3: Remove unused note parameter from toggle_favorite

**Files:**
- Modify: `src-tauri/src/commands/favorites.rs:45-48`
- Modify: `src/lib/tauri.ts:44-45`

- [ ] **Step 1: Remove _note parameter from Rust command**

In `src-tauri/src/commands/favorites.rs`, change the `toggle_favorite` function signature from:

```rust
#[tauri::command]
pub fn toggle_favorite(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    _note: Option<String>,
) -> Result<bool, String> {
```

To:

```rust
#[tauri::command]
pub fn toggle_favorite(
    db: State<'_, Arc<Database>>,
    session_id: i64,
) -> Result<bool, String> {
```

- [ ] **Step 2: Remove note parameter from TypeScript wrapper**

In `src/lib/tauri.ts`, change line 44-45 from:

```typescript
export const toggleFavorite = (sessionId: number, note?: string) =>
  invoke<boolean>("toggle_favorite", { sessionId, note });
```

To:

```typescript
export const toggleFavorite = (sessionId: number) =>
  invoke<boolean>("toggle_favorite", { sessionId });
```

- [ ] **Step 3: Build to verify both Rust and TypeScript compile**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && npx tsc --noEmit 2>&1 | tail -10`

Expected: Both pass with no errors. The frontend caller `FavoriteButton.tsx` already calls `toggleFavorite(sessionId)` without `note`, so no change needed there.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/favorites.rs src/lib/tauri.ts
git commit -m "refactor(favorites): remove unused note parameter from toggle_favorite"
```

---

### Task 4: Delete old database and smoke test

- [ ] **Step 1: Delete the old database**

```bash
rm ~/Library/Application\ Support/claude-session-manager/index.db
```

- [ ] **Step 2: Run the app and verify it starts cleanly**

```bash
cd /Users/ivyxjc/Zeta/SideProjects/cc-session && pnpm run tauri dev
```

Expected: App starts, creates a fresh database, scans sessions from `~/.claude/`, and displays them. Favorites (star toggle), hide, and project starring should all work as before.

- [ ] **Step 3: Verify favorites functionality**

In the running app:
1. Click the star on any session → it should toggle to favorited
2. Navigate to Favorites view in sidebar → the session should appear
3. Click star again → should unfavorite

- [ ] **Step 4: Final commit (if any fixups needed)**

If no fixups were needed, skip this step.
