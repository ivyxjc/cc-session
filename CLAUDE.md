In this project, you may proactively perform GIT operations.

# Project: Claude Session Manager

A Tauri 2 + React + TypeScript desktop app for browsing and managing local Claude Code sessions.

## Tech Stack
- **Backend**: Rust (Tauri 2), rusqlite, notify 7, zstd, chrono
- **Frontend**: React 19, TypeScript 5.8, Vite 7, Tailwind CSS 4, zustand, react-virtuoso, react-markdown, shiki
- **Package manager**: pnpm

## Key Directories
- `src-tauri/src/` — Rust backend (scanner, parser, monitor, backup, commands, db)
- `src/` — React frontend (components, stores, lib)
- `docs/specs/` — Design specs

## Architecture
- App reads `~/.claude/` read-only. All app state in SQLite at `~/Library/Application Support/claude-session-manager/`
- `scanner` discovers sessions from JSONL files, `parser` extracts messages/metadata
- `monitor` module handles live session tracking (PID polling + fs-notify JSONL tail)
- Frontend uses react-virtuoso for virtualized message lists, zustand for state

## Conventions
- Rust structs use `#[serde(rename_all = "camelCase")]` for Tauri IPC (frontend gets camelCase)
- Exception: `ContentBlock` enum uses `#[serde(rename_all = "snake_case")]` for JSONL compatibility — frontend types must match snake_case field names (`tool_use_id`, `is_error`, `media_type`)
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
