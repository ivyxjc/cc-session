# View Model Decoupling Design

## Goal

Introduce a provider-agnostic view model layer between the raw Claude parser types and the frontend. The existing app continues to work identically, but the Tauri commands and frontend consume unified `View*` types instead of Claude-specific `ParsedMessage`/`ContentBlock`. This prepares the codebase for adding Codex support without any provider-specific knowledge leaking into the UI layer.

## Problem

Currently, Claude-specific types flow directly from parser → Tauri commands → frontend:

- `ContentBlock` uses `#[serde(rename_all = "snake_case")]` for JSONL parsing compatibility, so the frontend receives `tool_use_id`, `is_error` in snake_case — inconsistent with the app's camelCase convention.
- `ParsedMessage` variants like `Attachment`, `PermissionMode`, `FileHistorySnapshot` are Claude-specific concepts baked into the frontend type system.
- Adding Codex support would require the frontend to know about two different message formats.

## Design

### Three-Layer Type Flow

```
Layer 1 (Raw):     Claude JSONL → ParsedMessage / ContentBlock  (existing, unchanged)
Layer 2 (Convert): ParsedMessage → ViewMessage / ViewContentBlock  (new converter)
Layer 3 (View):    Tauri commands return View* types → frontend consumes View* types
```

### New Rust Types: `src-tauri/src/models/`

#### `models/message.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewMessage {
    User {
        id: String,
        parent_id: Option<String>,
        timestamp: Option<String>,
        content: Vec<ViewContentBlock>,
    },
    Assistant {
        id: String,
        parent_id: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        content: Vec<ViewContentBlock>,
        usage: Option<ViewUsage>,
        stop_reason: Option<String>,
    },
    System {
        id: Option<String>,
        timestamp: Option<String>,
        subtype: Option<String>,
        content: Option<String>,
    },
}
```

Note: Claude's `Attachment`, `PermissionMode`, `FileHistorySnapshot` map to `System` with appropriate subtypes (`"attachment"`, `"permissionMode"`, `"fileHistorySnapshot"`). The frontend already renders System messages by subtype, so no visual change.

#### `models/content.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Image {
        source: ViewImageSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewImageSource {
    pub source_type: String,
    pub media_type: Option<String>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
}
```

Key difference from current `ContentBlock`: the serde tag values use **camelCase** (`toolCall`, `toolResult`) since these types are only for Tauri IPC, not for JSONL parsing. This makes the frontend type system consistent.

#### `models/session.rs`

No changes to `SessionSummary` or `Project` in this phase. These types are already database-model types, not parser types. They will get a `provider` field when Codex support is added.

#### `models/mod.rs`

```rust
pub mod content;
pub mod message;

pub use content::{ViewContentBlock, ViewImageSource, ViewUsage};
pub use message::ViewMessage;
```

### New Converter: `src-tauri/src/claude/converter.rs`

Converts `ParsedMessage` → `ViewMessage`:

```rust
pub fn to_view_message(msg: ParsedMessage) -> ViewMessage { ... }
pub fn to_view_content_block(block: ContentBlock) -> ViewContentBlock { ... }
```

Mapping:
| ParsedMessage variant | ViewMessage variant | Notes |
|---|---|---|
| User | User | uuid → id, parentUuid → parentId |
| Assistant | Assistant | uuid → id, parentUuid → parentId |
| System | System | Direct map |
| Attachment | System | subtype = "attachment", content = attachment_type |
| PermissionMode | System | subtype = "permissionMode", content = mode |
| FileHistorySnapshot | System | subtype = "fileHistorySnapshot", content = None |

| ContentBlock variant | ViewContentBlock variant | Notes |
|---|---|---|
| Text | Text | Direct map |
| Thinking | Thinking | Drop signature field (not displayed) |
| ToolUse | ToolCall | Rename: id/name/input unchanged |
| ToolResult | ToolResult | Rename: tool_use_id → toolCallId |
| Image | Image | Wrap source fields |

### Updated Tauri Commands

Commands that currently return `ParsedMessage` will return `ViewMessage` instead:

- `get_messages()` → `Vec<ViewMessage>` (was `Vec<ParsedMessage>`)
- `get_latest_messages()` → `ViewLatestMessagesResult` with `messages: Vec<ViewMessage>`
- `get_subagent_messages()` → `Vec<ViewMessage>`
- `get_backup_messages()` → `Vec<ViewMessage>`
- Live monitor `session-messages-update` event → `Vec<ViewMessage>`

Commands that return `SessionSummary`, `Project`, etc. are unchanged in this phase.

### Updated Frontend Types (`src/lib/types.ts`)

```typescript
// Replaces ContentBlock
export interface ViewContentBlock {
  type: "text" | "thinking" | "toolCall" | "toolResult" | "image";
  text?: string;
  thinking?: string;
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  toolCallId?: string;      // was tool_use_id
  content?: unknown;
  isError?: boolean;         // was is_error
  source?: {
    sourceType: string;      // was type
    mediaType?: string;      // was media_type
    data?: string;
  };
}

// Replaces Usage
export interface ViewUsage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

// Replaces ParsedMessage
export type ViewMessage =
  | { type: "user"; id: string; parentId: string | null; timestamp: string | null; content: ViewContentBlock[] }
  | { type: "assistant"; id: string; parentId: string | null; timestamp: string | null; model: string | null; content: ViewContentBlock[]; usage: ViewUsage | null; stopReason: string | null }
  | { type: "system"; id: string | null; timestamp: string | null; subtype: string | null; content: string | null };
```

Key changes for frontend:
- `ContentBlock.type` values change: `tool_use` → `toolCall`, `tool_result` → `toolResult`
- `ContentBlock.tool_use_id` → `ViewContentBlock.toolCallId`
- `ContentBlock.is_error` → `ViewContentBlock.isError`
- `ContentBlock.source.media_type` → `ViewContentBlock.source.mediaType`
- `ParsedMessage` union loses `attachment`, `permissionMode`, `fileHistorySnapshot` variants — they become `system` subtype messages
- `ParsedMessage.uuid` → `ViewMessage.id`
- `ParsedMessage.parentUuid` → `ViewMessage.parentId`

### Updated Frontend Components

Components that import `ParsedMessage` / `ContentBlock`:

1. **`MessageBubble.tsx`** — Update to use `ViewMessage` / `ViewContentBlock`. Switch on `type: "toolCall"` instead of `type: "tool_use"`. Use `toolCallId` instead of `tool_use_id`. Handle `attachment`/`permissionMode`/`fileHistorySnapshot` as system subtypes.
2. **`ToolCallBlock.tsx`** — Update `ContentBlock` references to `ViewContentBlock`. Use camelCase field names.
3. **`SubagentView.tsx`** — Update `ParsedMessage` references to `ViewMessage`.
4. **`ConversationView.tsx`** (and `LiveConversationView.tsx`) — Update type references.

Components that don't import these types (CodeBlock, DiffView, ThinkingBlock, ImageFromPath) need no changes.

### What Does NOT Change

- `src-tauri/src/parser/` — Raw Claude parser, untouched. Still produces `ParsedMessage`/`ContentBlock` with snake_case serde for JSONL parsing.
- `src-tauri/src/scanner/` — Still reads Claude JSONL, stores in DB. Untouched.
- `src-tauri/src/db/models.rs` — `SessionSummary`, `Project`, `Tag` etc. Untouched in this phase.
- Session list, project list, sidebar — No type changes.

### File Changes Summary

| File | Action |
|---|---|
| `src-tauri/src/models/mod.rs` | Create |
| `src-tauri/src/models/message.rs` | Create |
| `src-tauri/src/models/content.rs` | Create |
| `src-tauri/src/claude/mod.rs` | Create |
| `src-tauri/src/claude/converter.rs` | Create |
| `src-tauri/src/lib.rs` | Add `mod models; mod claude;` |
| `src-tauri/src/commands/sessions.rs` | Return `ViewMessage` instead of `ParsedMessage` |
| `src-tauri/src/commands/backups.rs` | Return `ViewMessage` instead of `ParsedMessage` |
| `src-tauri/src/monitor/mod.rs` | Emit `ViewMessage` in events |
| `src/lib/types.ts` | Replace `ParsedMessage`/`ContentBlock`/`Usage` with `View*` types |
| `src/components/message/MessageBubble.tsx` | Update type refs + field names |
| `src/components/message/ToolCallBlock.tsx` | Update type refs + field names |
| `src/components/message/SubagentView.tsx` | Update type refs |
| `src/components/session/ConversationView.tsx` | Update type refs |
| `src/components/live/LiveConversationView.tsx` | Update type refs |

### Testing

After refactoring:
1. `cargo check` — Rust compiles
2. `npx tsc --noEmit` — TypeScript compiles
3. `cargo test` — existing tests pass
4. Delete DB, run `pnpm run tauri dev` — app starts, sessions load, messages render correctly
5. Verify: tool call blocks display, thinking blocks expand, images render, subagent views work
