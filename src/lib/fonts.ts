const UI_FONT_KEY = "ui-font";
const CODE_FONT_KEY = "code-font";
const FONT_SIZE_KEY = "font-size";
const TERMINAL_FONT_KEY = "terminal-font";
const TERMINAL_FONT_SIZE_KEY = "terminal-font-size";
const TERMINAL_LINE_HEIGHT_KEY = "terminal-line-height";

/**
 * Default monospace stack. "Maple Mono NF CN" is first so users who installed it
 * (recommended) get the full-width CJK + Nerd Font icons that zellij uses for
 * its status bar. Falls through to common system mono fonts otherwise.
 */
const MAPLE_MONO_STACK = '"Maple Mono NF CN", "JetBrainsMono Nerd Font Mono", "JetBrains Mono", Menlo, Monaco, "Courier New", monospace';
const TERMINAL_FONT_FALLBACK = MAPLE_MONO_STACK;
export const TERMINAL_FONT_SIZE_DEFAULT = 13;
export const TERMINAL_LINE_HEIGHT_DEFAULT = 1.0;
export const TERMINAL_FONT_SIZE_MIN = 8;
export const TERMINAL_FONT_SIZE_MAX = 32;
export const TERMINAL_LINE_HEIGHT_MIN = 0.8;
export const TERMINAL_LINE_HEIGHT_MAX = 1.8;

const TERMINAL_SETTINGS_EVENT = "cc-session:terminal-settings-changed";

export function getUiFont(): string {
  return localStorage.getItem(UI_FONT_KEY) || "";
}

export function getCodeFont(): string {
  return localStorage.getItem(CODE_FONT_KEY) || "";
}

export function getFontSize(): string {
  return localStorage.getItem(FONT_SIZE_KEY) || "";
}

/** Raw stored terminal font name (no fallback chain). Empty if unset. */
export function getTerminalFont(): string {
  return localStorage.getItem(TERMINAL_FONT_KEY) || "";
}

/** Full font-family string for xterm.js: user pref → code font → system mono. */
export function getTerminalFontFamily(): string {
  const explicit = getTerminalFont();
  if (explicit) return `"${explicit}", ${TERMINAL_FONT_FALLBACK}`;
  const code = getCodeFont();
  if (code) return `"${code}", ${TERMINAL_FONT_FALLBACK}`;
  return TERMINAL_FONT_FALLBACK;
}

export function getTerminalFontSize(): number {
  const raw = parseInt(localStorage.getItem(TERMINAL_FONT_SIZE_KEY) || "", 10);
  if (!Number.isFinite(raw)) return TERMINAL_FONT_SIZE_DEFAULT;
  return Math.max(TERMINAL_FONT_SIZE_MIN, Math.min(TERMINAL_FONT_SIZE_MAX, raw));
}

export function getTerminalLineHeight(): number {
  const raw = parseFloat(localStorage.getItem(TERMINAL_LINE_HEIGHT_KEY) || "");
  if (!Number.isFinite(raw)) return TERMINAL_LINE_HEIGHT_DEFAULT;
  return Math.max(TERMINAL_LINE_HEIGHT_MIN, Math.min(TERMINAL_LINE_HEIGHT_MAX, raw));
}

export function setUiFont(font: string) {
  if (font) {
    localStorage.setItem(UI_FONT_KEY, font);
  } else {
    localStorage.removeItem(UI_FONT_KEY);
  }
  applyFonts();
}

export function setCodeFont(font: string) {
  if (font) {
    localStorage.setItem(CODE_FONT_KEY, font);
  } else {
    localStorage.removeItem(CODE_FONT_KEY);
  }
  applyFonts();
  // Terminal falls back to code font when no terminal-specific font is set —
  // notify so open terminals refresh.
  notifyTerminalSettingsChanged();
}

export function setFontSize(size: string) {
  if (size) {
    localStorage.setItem(FONT_SIZE_KEY, size);
  } else {
    localStorage.removeItem(FONT_SIZE_KEY);
  }
  applyFonts();
}

export function setTerminalFont(font: string) {
  if (font) {
    localStorage.setItem(TERMINAL_FONT_KEY, font);
  } else {
    localStorage.removeItem(TERMINAL_FONT_KEY);
  }
  notifyTerminalSettingsChanged();
}

export function setTerminalFontSize(size: number | null) {
  if (size != null && Number.isFinite(size)) {
    localStorage.setItem(TERMINAL_FONT_SIZE_KEY, String(size));
  } else {
    localStorage.removeItem(TERMINAL_FONT_SIZE_KEY);
  }
  notifyTerminalSettingsChanged();
}

export function setTerminalLineHeight(lh: number | null) {
  if (lh != null && Number.isFinite(lh)) {
    localStorage.setItem(TERMINAL_LINE_HEIGHT_KEY, String(lh));
  } else {
    localStorage.removeItem(TERMINAL_LINE_HEIGHT_KEY);
  }
  notifyTerminalSettingsChanged();
}

function notifyTerminalSettingsChanged() {
  window.dispatchEvent(new CustomEvent(TERMINAL_SETTINGS_EVENT));
}

/** Subscribe to terminal-related font / size / line-height changes. Returns unlisten. */
export function onTerminalSettingsChange(handler: () => void): () => void {
  window.addEventListener(TERMINAL_SETTINGS_EVENT, handler);
  return () => window.removeEventListener(TERMINAL_SETTINGS_EVENT, handler);
}

export function applyFonts() {
  const root = document.documentElement;
  const uiFont = getUiFont();
  const codeFont = getCodeFont();
  const fontSize = getFontSize();

  if (uiFont) {
    root.style.fontFamily = `${uiFont}, system-ui, -apple-system, sans-serif`;
  } else {
    root.style.fontFamily = "";
  }

  if (codeFont) {
    root.style.setProperty("--code-font", `"${codeFont}", ${MAPLE_MONO_STACK}`);
  } else {
    // Default code font: same Maple-first chain as the terminal.
    root.style.setProperty("--code-font", MAPLE_MONO_STACK);
  }

  if (fontSize) {
    root.style.fontSize = fontSize;
  } else {
    root.style.fontSize = "";
  }
}
