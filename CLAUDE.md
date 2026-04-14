In this project, you may proactively perform GIT operations.

# Project: CC Session

A Tauri 2 + React + TypeScript desktop app for browsing and managing local Claude Code and OpenAI Codex sessions.

## Tech Stack
- **Backend**: Rust (Tauri 2), rusqlite, notify 7, zstd, chrono
- **Frontend**: React 19, TypeScript 5.8, Vite 7, Tailwind CSS 4, zustand, react-virtuoso, react-markdown, shiki
- **Package manager**: pnpm

## Key Directories
- `src-tauri/src/` — Rust backend (scanner, parser, monitor, backup, commands, db, models, claude, codex)
- `src/` — React frontend (components, stores, lib)
- `src/components/codex/` — Codex-specific frontend views
- `docs/specs/` — Design specs

## Architecture
- Claude: reads `~/.claude/` read-only, scans JSONL sessions, indexes into app SQLite at `~/Library/Application Support/claude-session-manager/`
- Codex: reads `~/.codex/state_5.sqlite` read-only for metadata, parses `~/.codex/sessions/` JSONL for conversations
- Unified view model (`ViewMessage`/`ViewContentBlock`) — provider-specific parsers convert to shared types
- `scanner` discovers Claude sessions, `parser` extracts messages, `monitor` handles live tracking
- `codex/` module reads Codex DB directly (no scanning needed)
- Frontend uses react-virtuoso for virtualized message lists, zustand for state

## Conventions
- Rust structs use `#[serde(rename_all = "camelCase")]` for Tauri IPC (frontend gets camelCase)
- Raw parser types (`ContentBlock`) use snake_case serde for JSONL compatibility; view model types (`ViewContentBlock`) use camelCase for frontend
- Codex-specific types live in `src-tauri/src/codex/`, Claude-specific in `src-tauri/src/claude/`
- Tauri commands registered in `src-tauri/src/lib.rs`
- Frontend IPC wrappers in `src/lib/tauri.ts`, types in `src/lib/types.ts`

## Status
- **Pre-release**: No backwards compatibility required for database schema changes. Breaking changes are acceptable — delete `~/Library/Application Support/claude-session-manager/index.db` and re-scan.

## Build
```bash
pnpm install
pnpm run tauri dev    # development
pnpm run tauri build  # release → src-tauri/target/release/bundle/
```

## currentDate
Today's date is 2026-04-12.
