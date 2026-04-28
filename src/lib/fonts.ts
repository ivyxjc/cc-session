const UI_FONT_KEY = "ui-font";
const CODE_FONT_KEY = "code-font";
const FONT_SIZE_KEY = "font-size";

export function getUiFont(): string {
  return localStorage.getItem(UI_FONT_KEY) || "";
}

export function getCodeFont(): string {
  return localStorage.getItem(CODE_FONT_KEY) || "";
}

export function getFontSize(): string {
  return localStorage.getItem(FONT_SIZE_KEY) || "";
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
}

export function setFontSize(size: string) {
  if (size) {
    localStorage.setItem(FONT_SIZE_KEY, size);
  } else {
    localStorage.removeItem(FONT_SIZE_KEY);
  }
  applyFonts();
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
    root.style.setProperty("--code-font", `${codeFont}, ui-monospace, monospace`);
  } else {
    root.style.removeProperty("--code-font");
  }

  if (fontSize) {
    root.style.fontSize = fontSize;
  } else {
    root.style.fontSize = "";
  }
}
