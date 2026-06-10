# Terminal Display Tuning (per-cwd) — Design

Date: 2026-06-11
Status: approved

## Problem

zellij sizes a session to the smallest attached client. When cc-session's
TerminalPane attaches to the same zellij session the user has open in a real
terminal, the pane's grid (cols×rows, derived from panel pixels ÷ font metrics)
is usually smaller than the external terminal's — so attaching shrinks the
user's live Claude/Codex session.

The TerminalPane already takes the full main area to mitigate this, but the
app window is still typically smaller than a full-screen terminal. The
remaining lever is font metrics: smaller font / tighter cell aspect → more
cols×rows in the same pixels.

## Goal

Stepless (fine-grained) per-pane adjustment of terminal font size, line
height, and letter spacing, with a live cols×rows readout, so the user can
tune the pane's grid to be ≥ the external terminal's grid. Settings are
remembered per project (cwd).

Non-goals (deliberately out of scope): Cmd+scroll zoom, target-grid lock
(enter cols×rows, auto-compute font), read-only mirror mode via
`zellij action dump-screen`. All three can layer on this data model later.

## Design

### Data layer — `src/lib/terminalDisplay.ts` (new)

- `TerminalDisplayOverride = { fontSize?: number; lineHeight?: number; letterSpacing?: number }`
  — only fields the user has actually moved are stored (field-level merge,
  untouched fields keep following the global Settings values).
- `TerminalDisplay` — the fully resolved triple.
- Storage: `localStorage["terminal-display:<cwd>"]` as JSON. Malformed or
  out-of-range values are clamped/ignored.
- `getDisplayOverride(cwd)` / `saveDisplayOverride(cwd, o)` / `clearDisplayOverride(cwd)`
- `resolveDisplay(cwd)` — override merged over the global terminal settings
  from `lib/fonts.ts` (`getTerminalFontSize` / `getTerminalLineHeight`;
  letter spacing has no global setting, default 0).
- Ranges: fontSize 6–24 step 0.25; lineHeight 0.8–2.0 step 0.05 (matches the
  existing global range; xterm.js cannot compress below what the glyph allows);
  letterSpacing 0–4px step 0.5.

### UI — TerminalPane header

- An `Aa` button in the header opens a small popover (outside-click to close,
  same pattern as MultiplexerButton):
  - three stepless range sliders (font size / line height / letter spacing)
    with the current numeric value shown,
  - a live **cols×rows** readout, updated on every fit/resize,
  - a **Reset** button that clears the cwd override and re-applies the global
    defaults.

### Apply path

slider change → state update (sliders track instantly) → **120ms debounce** →
`term.options.{fontSize,lineHeight,letterSpacing}` + `fit.fit()` → existing
`term.onResize` hook sends `pty_resize` to the PTY → override persisted to
localStorage. The debounce keeps the shared zellij session from bouncing in
the external terminal while a slider is being dragged.

- Mount: Terminal is constructed with `resolveDisplay(cwd)` values instead of
  raw globals.
- Global settings change (`onTerminalSettingsChange`): re-resolve via
  `resolveDisplay(cwd)` so per-cwd overridden fields win, un-overridden fields
  follow the new global value; sync the slider state.
- Cleanup: pending debounce timer cleared on unmount; existing
  term-disposed try/catch guards reused.

## Verification

- `pnpm exec tsc --noEmit` passes.
- Manual (`pnpm tauri dev`): attach the same zellij session from a large
  external terminal and from cc-session; drag font size down until the pane's
  cols×rows readout ≥ the external grid; confirm the external session is no
  longer shrunk. Reset restores global defaults. Reopening the pane for the
  same project restores the tuned values; a different project starts from
  global defaults.
