// Per-project (cwd) terminal display overrides — font size, line height,
// letter spacing. Lets the user tune a TerminalPane's grid (cols×rows) so
// attaching to a shared zellij/tmux session doesn't shrink it for clients
// in larger external terminals. Only fields the user actually adjusted are
// stored; untouched fields keep following the global Settings values.

import { getTerminalFontSize, getTerminalLineHeight } from "./fonts";

export interface TerminalDisplayOverride {
  fontSize?: number;
  lineHeight?: number;
  letterSpacing?: number;
}

export interface TerminalDisplay {
  fontSize: number;
  lineHeight: number;
  letterSpacing: number;
}

export const DISPLAY_FONT_SIZE_MIN = 6;
export const DISPLAY_FONT_SIZE_MAX = 24;
export const DISPLAY_FONT_SIZE_STEP = 0.25;
export const DISPLAY_LINE_HEIGHT_MIN = 0.8;
export const DISPLAY_LINE_HEIGHT_MAX = 2.0;
export const DISPLAY_LINE_HEIGHT_STEP = 0.05;
export const DISPLAY_LETTER_SPACING_MIN = 0;
export const DISPLAY_LETTER_SPACING_MAX = 4;
export const DISPLAY_LETTER_SPACING_STEP = 0.5;

const KEY_PREFIX = "terminal-display:";

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

export function getDisplayOverride(cwd: string): TerminalDisplayOverride {
  try {
    const raw = localStorage.getItem(KEY_PREFIX + cwd);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: TerminalDisplayOverride = {};
    if (typeof parsed.fontSize === "number" && Number.isFinite(parsed.fontSize)) {
      out.fontSize = clamp(parsed.fontSize, DISPLAY_FONT_SIZE_MIN, DISPLAY_FONT_SIZE_MAX);
    }
    if (typeof parsed.lineHeight === "number" && Number.isFinite(parsed.lineHeight)) {
      out.lineHeight = clamp(parsed.lineHeight, DISPLAY_LINE_HEIGHT_MIN, DISPLAY_LINE_HEIGHT_MAX);
    }
    if (typeof parsed.letterSpacing === "number" && Number.isFinite(parsed.letterSpacing)) {
      out.letterSpacing = clamp(parsed.letterSpacing, DISPLAY_LETTER_SPACING_MIN, DISPLAY_LETTER_SPACING_MAX);
    }
    return out;
  } catch {
    return {};
  }
}

export function saveDisplayOverride(cwd: string, override: TerminalDisplayOverride) {
  if (Object.keys(override).length === 0) {
    localStorage.removeItem(KEY_PREFIX + cwd);
    return;
  }
  localStorage.setItem(KEY_PREFIX + cwd, JSON.stringify(override));
}

export function clearDisplayOverride(cwd: string) {
  localStorage.removeItem(KEY_PREFIX + cwd);
}

/** Override merged over the global terminal settings. Letter spacing has no
 *  global setting; it defaults to 0. */
export function resolveDisplay(cwd: string): TerminalDisplay {
  const o = getDisplayOverride(cwd);
  return {
    fontSize: o.fontSize ?? getTerminalFontSize(),
    lineHeight: o.lineHeight ?? getTerminalLineHeight(),
    letterSpacing: o.letterSpacing ?? 0,
  };
}
