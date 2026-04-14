# View Model Decoupling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a provider-agnostic view model layer (`ViewMessage`, `ViewContentBlock`) between the raw Claude parser and the frontend, so the UI never touches Claude-specific types.

**Architecture:** New `models/` module defines unified view types. New `claude/converter.rs` converts `ParsedMessage` → `ViewMessage`. Tauri commands and the live monitor emit `ViewMessage`. Frontend types and components switch from `ParsedMessage`/`ContentBlock` to `ViewMessage`/`ViewContentBlock`. The existing `parser/` module stays untouched.

**Tech Stack:** Rust (Tauri 2, serde), TypeScript, React

---

### Task 1: Create unified view model types (Rust)

**Files:**
- Create: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/models/content.rs`
- Create: `src-tauri/src/models/message.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/models/content.rs`**

```rust
use serde::{Deserialize, Serialize};

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
        #[serde(rename = "isError")]
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

- [ ] **Step 2: Create `src-tauri/src/models/message.rs`**

```rust
use serde::{Deserialize, Serialize};
use super::content::{ViewContentBlock, ViewUsage};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ViewMessage {
    User {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        timestamp: Option<String>,
        content: Vec<ViewContentBlock>,
    },
    Assistant {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        content: Vec<ViewContentBlock>,
        usage: Option<ViewUsage>,
        #[serde(rename = "stopReason")]
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

- [ ] **Step 3: Create `src-tauri/src/models/mod.rs`**

```rust
pub mod content;
pub mod message;

pub use content::{ViewContentBlock, ViewImageSource, ViewUsage};
pub use message::ViewMessage;
```

- [ ] **Step 4: Register module in `src-tauri/src/lib.rs`**

Add `mod models;` after the existing `mod monitor;` line (line 6):

```rust
mod db;
mod parser;
mod scanner;
mod commands;
mod backup;
mod monitor;
mod models;
```

- [ ] **Step 5: Verify compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Expected: Compiles with no errors (the types are defined but not yet used).

---

### Task 2: Create Claude converter

**Files:**
- Create: `src-tauri/src/claude/mod.rs`
- Create: `src-tauri/src/claude/converter.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/claude/converter.rs`**

```rust
use crate::models::{ViewContentBlock, ViewImageSource, ViewMessage, ViewUsage};
use crate::parser::content::{ContentBlock, Usage};
use crate::parser::messages::ParsedMessage;

pub fn to_view_message(msg: ParsedMessage) -> ViewMessage {
    match msg {
        ParsedMessage::User { uuid, parent_uuid, timestamp, content } => {
            ViewMessage::User {
                id: uuid,
                parent_id: parent_uuid,
                timestamp,
                content: content.into_iter().map(to_view_content_block).collect(),
            }
        }
        ParsedMessage::Assistant { uuid, parent_uuid, timestamp, model, content, usage, stop_reason } => {
            ViewMessage::Assistant {
                id: uuid,
                parent_id: parent_uuid,
                timestamp,
                model,
                content: content.into_iter().map(to_view_content_block).collect(),
                usage: usage.map(to_view_usage),
                stop_reason,
            }
        }
        ParsedMessage::System { uuid, timestamp, subtype, content } => {
            ViewMessage::System {
                id: uuid,
                timestamp,
                subtype,
                content,
            }
        }
        ParsedMessage::Attachment { attachment_type } => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("attachment".to_string()),
                content: Some(attachment_type),
            }
        }
        ParsedMessage::PermissionMode { mode } => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("permissionMode".to_string()),
                content: Some(mode),
            }
        }
        ParsedMessage::FileHistorySnapshot => {
            ViewMessage::System {
                id: None,
                timestamp: None,
                subtype: Some("fileHistorySnapshot".to_string()),
                content: None,
            }
        }
    }
}

pub fn to_view_content_block(block: ContentBlock) -> ViewContentBlock {
    match block {
        ContentBlock::Text { text } => ViewContentBlock::Text { text },
        ContentBlock::Thinking { thinking, .. } => ViewContentBlock::Thinking { thinking },
        ContentBlock::ToolUse { id, name, input } => ViewContentBlock::ToolCall { id, name, input },
        ContentBlock::ToolResult { tool_use_id, content, is_error } => {
            ViewContentBlock::ToolResult {
                tool_call_id: tool_use_id,
                content,
                is_error,
            }
        }
        ContentBlock::Image { source } => {
            ViewContentBlock::Image {
                source: ViewImageSource {
                    source_type: source.source_type,
                    media_type: source.media_type,
                    data: source.data,
                },
            }
        }
    }
}

fn to_view_usage(usage: Usage) -> ViewUsage {
    ViewUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}
```

- [ ] **Step 2: Create `src-tauri/src/claude/mod.rs`**

```rust
pub mod converter;
```

- [ ] **Step 3: Register module in `src-tauri/src/lib.rs`**

Add `mod claude;` after `mod models;`:

```rust
mod models;
mod claude;
```

- [ ] **Step 4: Verify compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Expected: Compiles with no errors.

---

### Task 3: Update Tauri commands to return ViewMessage

**Files:**
- Modify: `src-tauri/src/commands/sessions.rs`
- Modify: `src-tauri/src/commands/backups.rs`
- Modify: `src-tauri/src/parser/mod.rs` (add `ViewLatestMessagesResult`)

- [ ] **Step 1: Add `ViewLatestMessagesResult` to `src-tauri/src/parser/mod.rs`**

Add this struct after the existing `LatestMessagesResult` (around line 174):

```rust
/// View-layer result with ViewMessage instead of ParsedMessage.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewLatestMessagesResult {
    pub messages: Vec<crate::models::ViewMessage>,
    pub total_count: usize,
}
```

- [ ] **Step 2: Update `src-tauri/src/commands/sessions.rs`**

Replace the imports at the top (lines 1-8):

```rust
use crate::db::Database;
use crate::db::models::{SessionSummary, Tag, SubagentSummary};
use crate::parser;
use crate::claude::converter::to_view_message;
use crate::models::ViewMessage;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
```

Change `get_messages` return type and add conversion (lines 164-183):

```rust
#[tauri::command]
pub fn get_messages(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let messages = parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )?;
    Ok(messages.into_iter().map(to_view_message).collect())
}
```

Change `get_latest_messages` return type and add conversion (lines 185-202):

```rust
#[tauri::command]
pub fn get_latest_messages(
    db: State<'_, Arc<Database>>,
    session_id: i64,
    count: Option<usize>,
) -> Result<parser::ViewLatestMessagesResult, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM sessions WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    ).map_err(|e| format!("Session not found: {}", e))?;

    let result = parser::load_latest_messages(
        Path::new(&jsonl_path),
        count.unwrap_or(50),
    )?;
    Ok(parser::ViewLatestMessagesResult {
        messages: result.messages.into_iter().map(to_view_message).collect(),
        total_count: result.total_count,
    })
}
```

Change `get_subagent_messages` return type and add conversion (lines 231-250):

```rust
#[tauri::command]
pub fn get_subagent_messages(
    db: State<'_, Arc<Database>>,
    subagent_id: i64,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let conn = db.conn();
    let jsonl_path: String = conn.query_row(
        "SELECT jsonl_path FROM subagents WHERE id = ?1",
        params![subagent_id],
        |row| row.get(0),
    ).map_err(|e| format!("Subagent not found: {}", e))?;

    let messages = parser::load_messages(
        Path::new(&jsonl_path),
        offset.unwrap_or(0),
        limit.unwrap_or(50),
    )?;
    Ok(messages.into_iter().map(to_view_message).collect())
}
```

- [ ] **Step 3: Update `src-tauri/src/commands/backups.rs`**

Replace the imports at the top (lines 1-9):

```rust
use crate::backup;
use crate::db::Database;
use crate::db::models::{Backup, BackupConfig};
use crate::parser;
use crate::claude::converter::to_view_message;
use crate::models::ViewMessage;
use rusqlite::params;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
```

Change `get_backup_messages` return type and add conversion (lines 128-139):

```rust
#[tauri::command]
pub fn get_backup_messages(
    backup_path: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<Vec<ViewMessage>, String> {
    let path = Path::new(&backup_path);
    if !path.exists() {
        return Err(format!("Backup file not found: {}", backup_path));
    }
    let messages = parser::load_messages(path, offset.unwrap_or(0), limit.unwrap_or(200))?;
    Ok(messages.into_iter().map(to_view_message).collect())
}
```

- [ ] **Step 4: Verify compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Expected: Compiles with no errors.

---

### Task 4: Update live monitor to emit ViewMessage

**Files:**
- Modify: `src-tauri/src/monitor/mod.rs`

- [ ] **Step 1: Update imports and SessionMessagesUpdate struct**

At the top of `src-tauri/src/monitor/mod.rs`, change line 2 from:

```rust
use crate::parser::messages::{ParsedMessage, RawMessage};
```

To:

```rust
use crate::parser::messages::{ParsedMessage, RawMessage};
use crate::claude::converter::to_view_message;
use crate::models::ViewMessage;
```

Change the `SessionMessagesUpdate` struct (around line 47) from:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagesUpdate {
    pub session_id: String,
    pub new_messages: Vec<ParsedMessage>,
}
```

To:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessagesUpdate {
    pub session_id: String,
    pub new_messages: Vec<ViewMessage>,
}
```

- [ ] **Step 2: Update `read_new_lines` to return `ViewMessage`**

Change the function signature (around line 345) from:

```rust
fn read_new_lines(path: &Path, offset: &Mutex<u64>) -> Vec<ParsedMessage> {
```

To:

```rust
fn read_new_lines(path: &Path, offset: &Mutex<u64>) -> Vec<ViewMessage> {
```

And at line 399 where `ParsedMessage::from_raw` is called, change:

```rust
        if matches!(raw.msg_type.as_str(), "user" | "assistant" | "system") {
            if let Some(parsed) = ParsedMessage::from_raw(&raw) {
                messages.push(parsed);
            }
        }
```

To:

```rust
        if matches!(raw.msg_type.as_str(), "user" | "assistant" | "system") {
            if let Some(parsed) = ParsedMessage::from_raw(&raw) {
                messages.push(to_view_message(parsed));
            }
        }
```

- [ ] **Step 3: Verify compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Expected: Compiles with no errors.

- [ ] **Step 4: Run tests**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10`

Expected: All existing tests pass.

---

### Task 5: Update frontend types

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/toolResults.ts`

- [ ] **Step 1: Replace message types in `src/lib/types.ts`**

Replace the `ContentBlock`, `Usage`, and `ParsedMessage` types (lines 159-195) with:

```typescript
// View model types — provider-agnostic
export interface ViewContentBlock {
  type: "text" | "thinking" | "toolCall" | "toolResult" | "image";
  // text
  text?: string;
  // thinking
  thinking?: string;
  // toolCall
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  // toolResult
  toolCallId?: string;
  content?: unknown;
  isError?: boolean;
  // image
  source?: {
    sourceType: string;
    mediaType?: string;
    data?: string;
  };
}

export interface ViewUsage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

export type ViewMessage =
  | { type: "user"; id: string; parentId: string | null; timestamp: string | null; content: ViewContentBlock[] }
  | { type: "assistant"; id: string; parentId: string | null; timestamp: string | null; model: string | null; content: ViewContentBlock[]; usage: ViewUsage | null; stopReason: string | null }
  | { type: "system"; id: string | null; timestamp: string | null; subtype: string | null; content: string | null };
```

Update `SessionMessagesUpdate` (line 107-110) to use `ViewMessage`:

```typescript
export interface SessionMessagesUpdate {
  sessionId: string;
  newMessages: ViewMessage[];
}
```

Update `LatestMessagesResult` (line 112-115) to use `ViewMessage`:

```typescript
export interface LatestMessagesResult {
  messages: ViewMessage[];
  totalCount: number;
}
```

- [ ] **Step 2: Update imports in `src/lib/tauri.ts`**

Change the import (line 2-7) to replace `ParsedMessage` with `ViewMessage`:

```typescript
import type {
  Project, SessionSummary, ViewMessage, SubagentSummary,
  Tag, Backup, BackupConfig, TerminalConfig, ScanResult, LiveSession,
  LatestMessagesResult,
  MultiplexerConfig, MultiplexerDetectionResult,
} from "./types";
```

Update the function return types:

Line 31-32 `getMessages`:
```typescript
export const getMessages = (sessionId: number, offset = 0, limit = 50) =>
  invoke<ViewMessage[]>("get_messages", { sessionId, offset, limit });
```

Line 34-35 `getLatestMessages`:
```typescript
export const getLatestMessages = (sessionId: number, count = 50) =>
  invoke<LatestMessagesResult>("get_latest_messages", { sessionId, count });
```

Line 40-41 `getSubagentMessages`:
```typescript
export const getSubagentMessages = (subagentId: number, offset = 0, limit = 50) =>
  invoke<ViewMessage[]>("get_subagent_messages", { subagentId, offset, limit });
```

Line 91-92 `getBackupMessages`:
```typescript
export const getBackupMessages = (backupPath: string, offset = 0, limit = 200) =>
  invoke<ViewMessage[]>("get_backup_messages", { backupPath, offset, limit });
```

- [ ] **Step 3: Update `src/lib/toolResults.ts`**

Replace the import (line 1):

```typescript
import type { ViewMessage, ViewContentBlock } from "./types";
```

Update `buildToolResultsMap` (lines 12-29):

```typescript
export function buildToolResultsMap(messages: ViewMessage[]): Map<string, ToolResult> {
  const map = new Map<string, ToolResult>();

  for (const msg of messages) {
    if (msg.type !== "user") continue;
    for (const block of msg.content) {
      if (block.type === "toolResult" && block.toolCallId) {
        const content = extractToolResultContent(block);
        map.set(block.toolCallId, {
          content,
          isError: block.isError ?? false,
        });
      }
    }
  }

  return map;
}
```

Update `extractToolResultContent` (lines 31-42):

```typescript
function extractToolResultContent(block: ViewContentBlock): string {
  const raw = block.content;
  if (typeof raw === "string") return raw;
  if (Array.isArray(raw)) {
    return raw
      .filter((b: Record<string, unknown>) => b.type === "text")
      .map((b: Record<string, unknown>) => b.text || "")
      .join("\n");
  }
  return String(raw ?? "");
}
```

- [ ] **Step 4: Verify TypeScript compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && npx tsc --noEmit 2>&1 | head -30`

Expected: Will show errors in component files — those are fixed in Task 6.

---

### Task 6: Update frontend components

**Files:**
- Modify: `src/components/message/MessageBubble.tsx`
- Modify: `src/components/message/ToolCallBlock.tsx`
- Modify: `src/components/message/SubagentView.tsx`
- Modify: `src/components/session/ConversationView.tsx`
- Modify: `src/components/live/LiveConversationView.tsx`
- Modify: `src/components/backup/BackupManager.tsx`
- Modify: `src/stores/liveStore.ts`

- [ ] **Step 1: Update `src/components/message/MessageBubble.tsx`**

Replace import (line 4):

```typescript
import type { ViewMessage, ViewContentBlock, SubagentSummary } from "../../lib/types";
```

Update `renderContentBlock` signature (line 12-17):

```typescript
function renderContentBlock(
  block: ViewContentBlock,
  index: number,
  subagents?: SubagentSummary[],
  toolResults?: Map<string, ToolResult>,
) {
```

In the `"image"` case (line 54-69), change `src?.type` to `src?.sourceType` and `src.media_type` to `src.mediaType`:

```typescript
    case "image": {
      const src = block.source;
      if (src?.sourceType === "base64" && src.data && src.mediaType) {
        return (
          <div key={index} className="my-1">
            <img
              src={`data:${src.mediaType};base64,${src.data}`}
              alt="User image"
              className="max-w-full max-h-96 rounded border border-zinc-200 dark:border-zinc-700"
              loading="lazy"
            />
          </div>
        );
      }
      return null;
    }
```

Change the `"tool_use"` case (line 71) to `"toolCall"`:

```typescript
    case "toolCall": {
```

Update the Props interface (line 95-99):

```typescript
interface Props {
  message: ViewMessage;
  subagents?: SubagentSummary[];
  toolResults?: Map<string, ToolResult>;
}
```

Update the component function (line 101-102). Remove the check for `permissionMode`, `fileHistorySnapshot`, `attachment` since they are now `system` subtypes:

```typescript
export const MessageBubble = memo(function MessageBubble({ message, subagents, toolResults }: Props) {
  if (message.type === "system") {
    // Skip attachment, permissionMode, fileHistorySnapshot subtypes
    if (message.subtype === "attachment" || message.subtype === "permissionMode" || message.subtype === "fileHistorySnapshot") {
      return null;
    }
    if (!message.content) return null;
    return (
      <div className="text-xs text-zinc-400 italic py-1">
        {message.subtype && <span className="font-medium">[{message.subtype}]</span>} {message.content}
      </div>
    );
  }

  const isUser = message.type === "user";

  // Skip user messages that only contain toolResult blocks
  if (isUser && message.content.length > 0 && message.content.every((b) => b.type === "toolResult")) {
    return null;
  }
```

- [ ] **Step 2: Update `src/components/message/ToolCallBlock.tsx`**

Replace import (line 4):

```typescript
import type { ViewContentBlock, ViewMessage, SubagentSummary } from "../../lib/types";
```

Update Props interface (line 8-12):

```typescript
interface Props {
  block: ViewContentBlock;
  subagents?: SubagentSummary[];
  toolResult?: ToolResult;
}
```

Update state type (line 16):

```typescript
  const [agentMessages, setAgentMessages] = useState<ViewMessage[]>([]);
```

- [ ] **Step 3: Update `src/components/message/SubagentView.tsx`**

Replace import (line 3):

```typescript
import type { ViewMessage, SubagentSummary } from "../../lib/types";
```

Update state type (line 13):

```typescript
  const [messages, setMessages] = useState<ViewMessage[]>([]);
```

- [ ] **Step 4: Update `src/components/session/ConversationView.tsx`**

Replace import (line 4):

```typescript
import type { ViewMessage, SessionSummary, SubagentSummary } from "../../lib/types";
```

Update `useIncrementalToolResults` parameter type (line 12):

```typescript
function useIncrementalToolResults(messages: ViewMessage[]) {
```

In the body of `useIncrementalToolResults` (lines 20-24), change `tool_result` to `toolResult` and `tool_use_id` to `toolCallId` and `is_error` to `isError`:

```typescript
  if (messages.length > processedRef.current) {
    for (let i = processedRef.current; i < messages.length; i++) {
      const msg = messages[i];
      if (msg.type !== "user") continue;
      for (const block of msg.content) {
        if (block.type === "toolResult" && block.toolCallId) {
          const content = extractToolResultContent(block);
          mapRef.current.set(block.toolCallId, { content, isError: block.isError ?? false });
        }
      }
    }
    processedRef.current = messages.length;
  }
```

Update `getMessageKey` (lines 45-49) to use `id` instead of `uuid`:

```typescript
function getMessageKey(msg: ViewMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.id || `msg-${index}`;
  if (msg.type === "system") return msg.id || `sys-${index}`;
  return `msg-${index}`;
}
```

Update `findSubagentMessageIndex` (lines 52-67) to use `toolCall` instead of `tool_use`:

```typescript
function findSubagentMessageIndex(messages: ViewMessage[], description: string): number {
  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    if (msg.type !== "assistant") continue;
    for (const block of msg.content) {
      if (
        block.type === "toolCall" &&
        block.name === "Agent" &&
        (block.input as { description?: string })?.description === description
      ) {
        return i;
      }
    }
  }
  return -1;
}
```

Update state types (lines 75-76):

```typescript
  const [messages, setMessages] = useState<ViewMessage[]>([]);
```

- [ ] **Step 5: Update `src/components/live/LiveConversationView.tsx`**

Replace import (line 5):

```typescript
import type { ViewMessage, SubagentSummary, SessionMessagesUpdate } from "../../lib/types";
```

Update `useIncrementalToolResults` parameter type (line 24):

```typescript
function useIncrementalToolResults(messages: ViewMessage[]) {
```

In the body (lines 30-37), change `tool_result` → `toolResult`, `tool_use_id` → `toolCallId`, `is_error` → `isError`:

```typescript
      for (const block of msg.content) {
        if (block.type === "toolResult" && block.toolCallId) {
          const content = extractToolResultContent(block);
          mapRef.current.set(block.toolCallId, { content, isError: block.isError ?? false });
        }
      }
```

Update `getMessageKey` (lines 63-67) to use `id` instead of `uuid`:

```typescript
function getMessageKey(msg: ViewMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.id || `msg-${index}`;
  if (msg.type === "system") return msg.id || `sys-${index}`;
  return `msg-${index}`;
}
```

Update `findSubagentMessageIndex` (lines 69-84) to use `toolCall` instead of `tool_use`:

```typescript
function findSubagentMessageIndex(messages: ViewMessage[], description: string): number {
  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    if (msg.type !== "assistant") continue;
    for (const block of msg.content) {
      if (
        block.type === "toolCall" &&
        block.name === "Agent" &&
        (block.input as { description?: string })?.description === description
      ) {
        return i;
      }
    }
  }
  return -1;
}
```

Update state types (line 97):

```typescript
  const [messages, setMessages] = useState<ViewMessage[]>([]);
```

- [ ] **Step 6: Update `src/stores/liveStore.ts`**

Replace import (line 2):

```typescript
import type { LiveSession, ViewMessage } from "../lib/types";
```

Update the interface and all `ParsedMessage` references to `ViewMessage` (lines 4-12):

```typescript
interface LiveState {
  liveSessions: LiveSession[];
  watchedSessionId: string | null;
  newMessages: ViewMessage[];
  setLiveSessions: (sessions: LiveSession[]) => void;
  setWatchedSessionId: (id: string | null) => void;
  appendMessages: (messages: ViewMessage[]) => void;
  clearNewMessages: () => void;
}
```

- [ ] **Step 7: Update `src/components/backup/BackupManager.tsx`**

Replace import (line 3):

```typescript
import type { Backup, ViewMessage, SessionSummary } from "../../lib/types";
```

Update state type (line 21):

```typescript
  const [viewMessages, setViewMessages] = useState<ViewMessage[]>([]);
```

- [ ] **Step 8: Verify TypeScript compilation**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && npx tsc --noEmit 2>&1 | tail -10`

Expected: No errors.

---

### Task 7: Final verification

- [ ] **Step 1: Full Rust build**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | tail -5`

Expected: No errors.

- [ ] **Step 2: Full TypeScript check**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && npx tsc --noEmit 2>&1 | tail -10`

Expected: No errors.

- [ ] **Step 3: Rust tests**

Run: `cd /Users/ivyxjc/Zeta/SideProjects/cc-session && cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | tail -10`

Expected: All tests pass.

- [ ] **Step 4: Delete old database and run dev**

```bash
rm ~/Library/Application\ Support/claude-session-manager/index.db
cd /Users/ivyxjc/Zeta/SideProjects/cc-session && pnpm run tauri dev
```

Expected: App starts, scans sessions, displays them. Click a session → conversation view renders messages, tool calls, thinking blocks, and subagents correctly.
