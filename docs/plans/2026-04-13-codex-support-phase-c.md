# Codex Support (Phase C) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add read-only Codex session browsing with project list, session list, and conversation viewer — all using the unified ViewMessage types.

**Architecture:** New `codex/` Rust module reads `~/.codex/state_5.sqlite` for metadata and parses Codex JSONL for conversations. Frontend gets new Codex views in sidebar, sharing existing message rendering components.

**Tech Stack:** Rust (rusqlite, serde), TypeScript, React

---

### Task 1: Codex backend — db.rs and types

Create Codex database reader and view types.

**Files:**
- Modify: `src-tauri/src/codex/mod.rs`
- Create: `src-tauri/src/codex/db.rs`

### Task 2: Codex backend — parser.rs and converter.rs

Parse Codex JSONL and convert to ViewMessage.

**Files:**
- Create: `src-tauri/src/codex/parser.rs`
- Create: `src-tauri/src/codex/converter.rs`

### Task 3: Codex backend — commands.rs

Tauri commands for frontend.

**Files:**
- Create: `src-tauri/src/codex/commands.rs`
- Modify: `src-tauri/src/lib.rs`

### Task 4: Frontend types and IPC

Add Codex types and Tauri wrappers.

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/tauri.ts`

### Task 5: Frontend views — appStore, sidebar, routing

Add Codex navigation.

**Files:**
- Modify: `src/stores/appStore.ts`
- Modify: `src/components/layout/Sidebar.tsx`
- Modify: `src/components/layout/MainContent.tsx`

### Task 6: Frontend components — Codex views

Create project list, session list, conversation view.

**Files:**
- Create: `src/components/codex/CodexProjectList.tsx`
- Create: `src/components/codex/CodexSessionList.tsx`
- Create: `src/components/codex/CodexConversationView.tsx`

### Task 7: Verification

Build and test.
