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
- `docs/specs/` — Design specs

## Architecture
- One index for every provider: `projects` / `sessions` / `subagents` rows carry a `provider` column (`claude` | `codex`). Favorites, tags, hide, FTS search, backups, export, AI summaries and daily activity all work off that index and are provider-agnostic.
- `sources/` is the provider boundary (`Provider` enum + dispatch: `parse_session_metadata`, `load_messages`, `load_latest_messages`, `extract_daily_tokens`, `raw_messages`). Code outside `claude/` and `codex/` never calls a provider parser directly.
- Claude: `scanner` reads `~/.claude/projects/` read-only, `parser` extracts messages, `monitor` handles live tracking (Claude-only).
- Codex: `codex/scanner` reads `~/.codex/state_5.sqlite` read-only for thread metadata and parses the rollout JSONL for counts/tokens/summary; `codex/parser` + `codex/converter` produce `ViewMessage`s. Codex `encoded_path` is the cwd itself.
- Unified view model (`ViewMessage`/`ViewContentBlock`) — provider-specific converters produce shared types. Consumers written against Claude's raw line shape (LLM input builder, FTS indexer) get other providers projected into that shape via `sources::raw_messages`.
- Frontend: the provider switch is a filter passed to `list_projects` / `list_sessions`; the same components render both providers.
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
