# Terminal Multiplexer Integration — Design Spec

## Overview

Enhance the session view with an optional multiplexer button (alongside existing "Open Terminal"). When enabled, the button detects existing Zellij/tmux sessions and lists recommended commands. User clicks a command to copy it to clipboard, then pastes in their terminal.

Goal: simplify the workflow of attaching to existing multiplexer sessions for a project without trying to automate the interactive terminal attachment.

## Scope

### In Scope
- Settings toggle to enable Zellij and/or tmux integration (independent toggles)
- New button in session card/header next to "Open Terminal"
- Backend detection of multiplexer sessions + cwd matching
- Dropdown listing: matched sessions, all sessions, and "new session" command
- Click-to-copy commands to clipboard

### Out of Scope
- Directly executing attach commands (interactive, requires terminal)
- Managing multiplexer sessions (kill/rename)
- Custom layout creation
- Other multiplexers (screen, etc.)

## Detection Capabilities

### Zellij

```bash
# List all sessions with status
zellij list-sessions -n
# Output per line: "<name> [Created <time> ago] (<status>)"
# Status: "(current)" | "(EXITED - attach to resurrect)" | no marker (detached active)

# Get cwd for ACTIVE sessions only
zellij -s <name> action dump-layout | head -3
# Output: layout { cwd "/path/to/project"
# NOTE: Fails for EXITED sessions — returns "Session not found"
```

### tmux

```bash
# List all sessions with cwd in one call
tmux list-sessions -F '#{session_name}\t#{pane_current_path}'
# Output: "main\t/Users/x/projects/my-app"
```

## Command Generation

### For a given project path, generate these command groups:

**Group 1: Matched sessions (cwd == project path)**
- Only for active sessions where cwd can be queried
- Zellij: `zellij attach <name>`
- tmux: `tmux attach -t <name>`
- Shown with a green indicator and "Matches this project" label

**Group 2: All other sessions**
- List all remaining sessions (active + EXITED for zellij, all for tmux)
- User picks manually — solves the EXITED session problem without heuristic matching
- Zellij: `zellij attach <name>`
- tmux: `tmux attach -t <name>`
- Shown with session name + status (active/exited)

**Group 3: New session**
- Always shown at the bottom as a fallback
- Zellij: `zellij -s <basename(path)> options --default-cwd <path>`
- tmux: `tmux new-session -s <basename(path)> -c <path>`

## Rust Backend

### New Command: `detect_multiplexer_sessions`

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MultiplexerSession {
    name: String,
    status: String,          // "active" | "exited" (zellij) / "active" (tmux)
    cwd: Option<String>,     // None for EXITED zellij sessions
    matches_path: bool,      // true if cwd == requested path
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MultiplexerDetectionResult {
    multiplexer: String,     // "zellij" | "tmux"
    sessions: Vec<MultiplexerSession>,
    new_session_cmd: String, // Command to create a new named session
}

#[tauri::command]
fn detect_multiplexer_sessions(
    path: String,
    multiplexer: String,     // "zellij" | "tmux"
) -> Result<MultiplexerDetectionResult, String>
```

**Implementation:**

For Zellij:
1. Run `zellij list-sessions -n`, parse each line for name + status
2. For each active (non-EXITED) session, run `zellij -s <name> action dump-layout`, extract `cwd "..."` from first few lines
3. Compare cwd with requested path, set `matches_path`
4. 2-second timeout per subprocess call
5. Generate `new_session_cmd`: `zellij -s <basename(path)> options --default-cwd <path>`

For tmux:
1. Run `tmux list-sessions -F '#{session_name}\t#{pane_current_path}'`
2. Parse tab-separated output
3. Compare cwd with requested path, set `matches_path`
4. Generate `new_session_cmd`: `tmux new-session -s <basename(path)> -c <path>`

Edge cases:
- Binary not found → return error "zellij/tmux not installed"
- tmux server not running → return empty sessions list
- No sessions → return empty sessions list with `new_session_cmd`
- Subprocess timeout → skip that session, continue with others

### Settings Storage

Reuse existing `app_config` table:

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MultiplexerConfig {
    zellij_enabled: bool,    // default: false
    tmux_enabled: bool,      // default: false
}
```

New commands: `get_multiplexer_config`, `set_multiplexer_config`.

## Frontend

### New Component: `MultiplexerButton`

Located next to `OpenTerminalButton` in session cards and headers. Only rendered when at least one multiplexer is enabled.

```
┌─────────────────────────────────────────────────────────┐
│ ⭐ hidden-puzzling-floyd       2h ago   >_  [z]  ♡     │
│ claude-exts · master · v2.1.98                          │
└─────────────────────────────────────────────────────────┘
```

`[z]` for Zellij, `[μ]` for tmux, or both if both enabled.

**Dropdown on click:**

```
┌──────────────────────────────────────┐
│ Zellij Sessions                      │
├──────────────────────────────────────┤
│ ● claude-exts          [matched]     │
│   zellij attach claude-exts    📋   │
├──────────────────────────────────────┤
│ ○ simora.cfd           (exited)      │
│   zellij attach simora.cfd     📋   │
│ ○ litellm              (exited)      │
│   zellij attach litellm        📋   │
│ ... (scrollable if many)             │
├──────────────────────────────────────┤
│ + New session                        │
│   zellij -s claude-exts ...    📋   │
└──────────────────────────────────────┘
```

- `●` green dot = active session
- `○` grey dot = exited session
- `[matched]` = cwd matches current project path
- `📋` = click copies command to clipboard, shows "Copied!" toast
- Matched sessions sorted to top
- Dropdown is scrollable for many sessions
- Detection runs on dropdown open (not eagerly), cached for 10 seconds

**If both multiplexers enabled:** Show two sections in the dropdown, one per multiplexer.

### Settings Page Addition

Under existing terminal settings:

```
Multiplexer Integration
  Zellij    [toggle]
  tmux      [toggle]
```

No other config needed — the app auto-detects binary paths and session states.

### Components

| Component | File | Purpose |
|-----------|------|---------|
| `MultiplexerButton` | `src/components/common/MultiplexerButton.tsx` | Button + dropdown |
| Settings section | Added to existing `SettingsPage.tsx` | Toggles |

## Data Flow

```
User clicks [z] button
  → detect_multiplexer_sessions(project_path, "zellij")
  → Rust spawns: zellij list-sessions -n
  → Rust spawns: zellij -s <active_name> action dump-layout (per active session)
  → Returns: MultiplexerDetectionResult { sessions, new_session_cmd }
  → Frontend renders dropdown with commands
  → User clicks 📋 on a command
  → Command copied to clipboard
  → User pastes in their terminal
```

## Edge Cases

| Scenario | Handling |
|----------|----------|
| Multiplexer not installed | Button not shown (check on settings toggle, warn user) |
| No sessions exist | Dropdown shows only "New session" command |
| Many sessions (>20) | Dropdown scrollable, max-height limited |
| Session name has special chars | Shell-escape in generated commands |
| Both multiplexers enabled | Two sections in dropdown |
| Path with spaces | Properly quoted in generated commands |
| Detection takes >2s | Show loading spinner in dropdown |
| Same basename, different paths | New session command uses full basename; matched sessions show cwd to disambiguate |

## Implementation Priority

1. `MultiplexerConfig` in settings (get/set commands + UI toggle)
2. `detect_multiplexer_sessions` backend command
3. `MultiplexerButton` frontend component
4. Wire into SessionCard and SessionHeader
