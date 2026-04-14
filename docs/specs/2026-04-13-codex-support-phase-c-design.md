# Codex Session Browser (Phase C) Design

## Goal

Add read-only browsing of OpenAI Codex CLI sessions. Users can browse Codex projects (grouped by cwd), view session lists, and read conversation history — all in a separate "Codex" section of the sidebar.

## Data Sources

**Metadata:** Read directly from `~/.codex/state_5.sqlite` (read-only). The `threads` table has: id, rollout_path, cwd, title, model, model_provider, tokens_used, git_branch, cli_version, approval_mode, source, archived, created_at, updated_at, first_user_message, agent_nickname.

**Conversations:** Parse `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`. Format:
- `session_meta` — session metadata (one per file)
- `response_item` with `payload.type`:
  - `message` (role: user/assistant/developer) with `content[].type`: `input_text` or `output_text`
  - `function_call` with name, arguments, call_id
  - `function_call_output` with call_id, output
  - `reasoning` with summary, encrypted_content
- `event_msg` with `payload.type`: token_count, exec_command_end, task_started, task_complete

**Subagents:** `thread_spawn_edges` table links parent → child threads. Child threads have JSON `source` field with `agent_nickname`.

## Backend Design

### New files in `src-tauri/src/codex/`

- `db.rs` — Open `~/.codex/state_5.sqlite` read-only, query threads, group by cwd into projects
- `parser.rs` — Parse Codex JSONL into raw event types
- `converter.rs` — Convert Codex raw events → ViewMessage/ViewContentBlock
- `commands.rs` — Tauri commands: `codex_list_projects`, `codex_list_sessions`, `codex_get_messages`, `codex_get_subagents`

### Codex types (Rust, internal only)

```rust
struct CodexThread {
    id: String,
    rollout_path: String,
    cwd: String,
    title: String,
    model: Option<String>,
    model_provider: String,
    tokens_used: i64,
    git_branch: Option<String>,
    cli_version: String,
    approval_mode: String,
    source: String,
    archived: bool,
    created_at: i64,
    updated_at: i64,
    first_user_message: String,
}
```

### View types returned to frontend

Reuse existing `ViewMessage` for conversation display. New types for Codex project/session lists:

```rust
struct CodexProject {
    cwd: String,
    display_name: String,
    session_count: i64,
    last_active: i64,
    total_tokens: i64,
}

struct CodexSession {
    id: String,
    title: String,
    cwd: String,
    model: Option<String>,
    tokens_used: i64,
    git_branch: Option<String>,
    cli_version: String,
    approval_mode: String,
    source: String,  // "cli" | "vscode" | "exec"
    archived: bool,
    created_at: i64,
    updated_at: i64,
    first_user_message: String,
    subagent_count: i64,
}
```

### Codex JSONL → ViewMessage mapping

| Codex event | ViewMessage |
|---|---|
| response_item/message role=user, content[].input_text | ViewMessage::User with ViewContentBlock::Text |
| response_item/message role=assistant, content[].output_text | ViewMessage::Assistant with ViewContentBlock::Text |
| response_item/message role=developer | ViewMessage::System (subtype="developer") |
| response_item/function_call | ViewContentBlock::ToolCall (name, input=parsed arguments, id=call_id) |
| response_item/function_call_output | ViewContentBlock::ToolResult (toolCallId=call_id, content=output) |
| response_item/reasoning | ViewContentBlock::Thinking (thinking=summary text or "[encrypted]") |
| event_msg/exec_command_end | Skipped (output captured via function_call_output) |
| event_msg/token_count | Skipped (metadata only) |
| session_meta, turn_context | Skipped |

### Codex state_5.sqlite path

`~/.codex/state_5.sqlite` — opened read-only, not managed by our app.

## Frontend Design

### New views in appStore

Add to View type: `"codexProjects"`, `"codexSessions"`, `"codexConversation"`

### New sidebar entry

A "Codex" button in the navigation section (below "Favorites", above tags). Clicking it navigates to `codexProjects` view.

### New components

- `src/components/codex/CodexProjectList.tsx` — Lists Codex projects grouped by cwd
- `src/components/codex/CodexSessionList.tsx` — Lists sessions for a Codex project
- `src/components/codex/CodexConversationView.tsx` — Renders conversation using shared MessageBubble

### Shared components (no changes needed)

MessageBubble, ToolCallBlock, ThinkingBlock, CodeBlock, DiffView — all consume ViewMessage/ViewContentBlock already.

## Scope

**In:** Read-only browsing, conversation viewer, subagent count display, sort by time/tokens, show/hide archived
**Out:** Favorites, tags, backup, live monitoring, search, usage tracking for Codex
