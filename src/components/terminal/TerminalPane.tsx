import { useEffect, useRef, useState, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import {
  detectMultiplexerSessions,
  findSessionForPid,
  getMultiplexerConfig,
  ptyAttachMultiplexer,
  ptyCreateMultiplexer,
  ptyDetach,
} from "../../lib/tauri";
import { getTerminalFontFamily, onTerminalSettingsChange } from "../../lib/fonts";
import {
  clearDisplayOverride,
  DISPLAY_FONT_SIZE_MAX,
  DISPLAY_FONT_SIZE_MIN,
  DISPLAY_FONT_SIZE_STEP,
  DISPLAY_LETTER_SPACING_MAX,
  DISPLAY_LETTER_SPACING_MIN,
  DISPLAY_LETTER_SPACING_STEP,
  DISPLAY_LINE_HEIGHT_MAX,
  DISPLAY_LINE_HEIGHT_MIN,
  DISPLAY_LINE_HEIGHT_STEP,
  getDisplayOverride,
  resolveDisplay,
  saveDisplayOverride,
  type TerminalDisplay,
  type TerminalDisplayOverride,
} from "../../lib/terminalDisplay";
import type { MultiplexerSession } from "../../lib/types";

interface OutputPayload {
  data: string;
}

interface Props {
  cwd: string;
  /** Live process PID — when provided, used to precisely detect the multiplexer session
   *  that contains this process (via ZELLIJ_SESSION_NAME / TMUX_PANE in its env). */
  livePid?: number;
  /** Called when user clicks the close button. */
  onClose?: () => void;
}

/**
 * Module-level cache: remember the last successfully attached session per cwd.
 * Lets the second open of the panel skip the ~1s detection step and attach immediately.
 * Wiped on full page reload (acceptable; detection rebuilds it).
 */
const lastSessionByCwd = new Map<string, { multiplexer: string; name: string }>();

function utf8ToBase64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function TerminalPane({ cwd, livePid, onClose }: Props) {
  const hostRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const attachedRef = useRef<boolean>(false);
  const autoAttachedRef = useRef<boolean>(false);

  const [multiplexer, setMultiplexer] = useState<string | null>(null);
  const [sessions, setSessions] = useState<MultiplexerSession[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "attached" | "error">("loading");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [createName, setCreateName] = useState<string>("");

  // Per-cwd display tuning (font size / line height / letter spacing) so the
  // pane's grid can be made ≥ the external terminal's and a shared zellij
  // session isn't shrunk by attaching here.
  const [display, setDisplay] = useState<TerminalDisplay>(() => resolveDisplay(cwd));
  const [grid, setGrid] = useState<{ cols: number; rows: number } | null>(null);
  const [showDisplaySettings, setShowDisplaySettings] = useState(false);
  // Fullscreen overlays the whole app window (more pixels → larger grid).
  // The terminal stays mounted; the ResizeObserver refits and resizes the PTY.
  const [fullscreen, setFullscreen] = useState(false);
  const displayPopoverRef = useRef<HTMLDivElement>(null);
  const overrideRef = useRef<TerminalDisplayOverride>(getDisplayOverride(cwd));
  const applyTimerRef = useRef<number | null>(null);

  // Debounced apply: sliders update state instantly; the terminal (and the
  // pty_resize that follows fit()) only sees the value 120ms after the last
  // change, so the shared session doesn't bounce while dragging.
  const updateDisplay = useCallback((patch: Partial<TerminalDisplay>) => {
    setDisplay((prev) => {
      const next = { ...prev, ...patch };
      overrideRef.current = { ...overrideRef.current, ...patch };
      if (applyTimerRef.current != null) window.clearTimeout(applyTimerRef.current);
      applyTimerRef.current = window.setTimeout(() => {
        applyTimerRef.current = null;
        const term = termRef.current;
        const fit = fitRef.current;
        if (term && fit) {
          try {
            term.options.fontSize = next.fontSize;
            term.options.lineHeight = next.lineHeight;
            term.options.letterSpacing = next.letterSpacing;
            fit.fit();
          } catch {
            // term disposed or host not sized — ignore
          }
        }
        saveDisplayOverride(cwd, overrideRef.current);
      }, 120);
      return next;
    });
  }, [cwd]);

  const resetDisplay = useCallback(() => {
    clearDisplayOverride(cwd);
    overrideRef.current = {};
    const d = resolveDisplay(cwd);
    setDisplay(d);
    const term = termRef.current;
    const fit = fitRef.current;
    if (term && fit) {
      try {
        term.options.fontSize = d.fontSize;
        term.options.lineHeight = d.lineHeight;
        term.options.letterSpacing = d.letterSpacing;
        fit.fit();
      } catch {
        // term disposed or host not sized — ignore
      }
    }
  }, [cwd]);

  // Close the display popover on outside click.
  useEffect(() => {
    if (!showDisplaySettings) return;
    const handler = (e: MouseEvent) => {
      if (displayPopoverRef.current && !displayPopoverRef.current.contains(e.target as Node)) {
        setShowDisplaySettings(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showDisplaySettings]);

  // Set up xterm.js once.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let cancelled = false;
    let unlistenOutput: UnlistenFn | null = null;
    let unlistenExit: UnlistenFn | null = null;

    const initial = resolveDisplay(cwd);
    const term = new Terminal({
      fontSize: initial.fontSize,
      fontFamily: getTerminalFontFamily(),
      lineHeight: initial.lineHeight,
      letterSpacing: initial.letterSpacing,
      cursorBlink: true,
      allowProposedApi: true,
      theme: {
        background: "#1e1e1e",
        foreground: "#dcdcdc",
        cursor: "#dcdcdc",
      },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(host);
    termRef.current = term;
    fitRef.current = fit;

    // Register Tauri listeners FIRST (synchronously kicked off) — don't gate on fonts.
    // If we're already cancelled (StrictMode double-mount cleanup) when listen resolves,
    // unsubscribe immediately to avoid leaked listeners that would write to a disposed term.
    listen<OutputPayload>("pty://output", (e) => {
      if (cancelled) return;
      const bytes = base64ToBytes(e.payload.data);
      try { term.write(bytes); } catch { /* term disposed */ }
    }).then((un) => {
      if (cancelled) un();
      else unlistenOutput = un;
    }).catch(() => {});

    listen("pty://exit", () => {
      if (cancelled) return;
      try { term.write("\r\n\x1b[33m[session ended]\x1b[0m\r\n"); } catch {}
      attachedRef.current = false;
      setStatus("ready");
    }).then((un) => {
      if (cancelled) un();
      else unlistenExit = un;
    }).catch(() => {});

    // Fit once layout settles. Avoid `document.fonts.ready` (can hang in webview).
    // After fit, run the fast-path attach so cached zellij sessions appear without the
    // ~1s detection step. We do this *after* fit() so the initial PTY size matches the
    // actual cell grid — otherwise zellij would briefly broadcast 80x24 to all clients.
    requestAnimationFrame(() => {
      if (cancelled) return;
      try { fit.fit(); } catch { /* host not yet sized */ }
      setGrid({ cols: term.cols, rows: term.rows });

      // Fast path priority:
      //   1. PID-based exact match (most accurate — finds the actual session containing
      //      the live Claude process via ZELLIJ_SESSION_NAME / TMUX_PANE in its env)
      //   2. Last-attached cache for this cwd
      //   3. Fall through to detection-based auto-attach (slower)
      const tryAttach = (mp: string, name: string) => {
        if (cancelled || autoAttachedRef.current || !termRef.current) return;
        autoAttachedRef.current = true;
        setMultiplexer(mp);
        const t = termRef.current;
        const cols = t.cols;
        const rows = t.rows;
        ptyAttachMultiplexer(mp, name, cwd, cols, rows)
          .then(() => {
            if (cancelled) return;
            attachedRef.current = true;
            setStatus("attached");
            lastSessionByCwd.set(cwd, { multiplexer: mp, name });
          })
          .catch(() => {
            // Session disappeared — let the normal flow retry with detection results.
            lastSessionByCwd.delete(cwd);
            autoAttachedRef.current = false;
          });
      };

      if (livePid != null) {
        getMultiplexerConfig().then((cfg) => {
          if (cancelled || !cfg.multiplexer || cfg.multiplexer === "none") return;
          findSessionForPid(livePid, cfg.multiplexer)
            .then((name) => {
              if (cancelled) return;
              if (name) {
                tryAttach(cfg.multiplexer, name);
              } else {
                // No PID-based match — fall back to cache, else wait for detection.
                const cached = lastSessionByCwd.get(cwd);
                if (cached) tryAttach(cached.multiplexer, cached.name);
              }
            })
            .catch(() => {
              const cached = lastSessionByCwd.get(cwd);
              if (cached) tryAttach(cached.multiplexer, cached.name);
            });
        }).catch(() => {});
      } else {
        const cached = lastSessionByCwd.get(cwd);
        if (cached) tryAttach(cached.multiplexer, cached.name);
      }
    });

    // Keep IDisposables so we can release them explicitly on cleanup. term.dispose()
    // does dispose registered listeners transitively, but being explicit makes the
    // lifecycle obvious and prevents leaks if a future change disposes earlier.
    const onDataDisposable = term.onData((data) => {
      if (!attachedRef.current) return;
      const b64 = utf8ToBase64(data);
      invoke("pty_write", { data: b64 }).catch(() => {});
    });

    const onResizeDisposable = term.onResize(({ cols, rows }) => {
      setGrid({ cols, rows });
      if (!attachedRef.current) return;
      invoke("pty_resize", { cols, rows }).catch(() => {});
    });

    const ro = new ResizeObserver(() => {
      try { fit.fit(); } catch { /* not yet sized */ }
    });
    ro.observe(host);

    // Live-update terminal font / size / line-height when user saves settings.
    // Mutating term.options triggers an internal re-render; we then refit so the
    // PTY size matches the new cell grid.
    const unlistenSettings = onTerminalSettingsChange(() => {
      if (cancelled) return;
      // Re-resolve so per-cwd overridden fields keep winning while
      // un-overridden ones follow the new global values.
      const d = resolveDisplay(cwd);
      setDisplay(d);
      try {
        term.options.fontFamily = getTerminalFontFamily();
        term.options.fontSize = d.fontSize;
        term.options.lineHeight = d.lineHeight;
        term.options.letterSpacing = d.letterSpacing;
        fit.fit();
      } catch {
        // term disposed or host not sized — ignore
      }
    });

    return () => {
      cancelled = true;
      if (applyTimerRef.current != null) {
        window.clearTimeout(applyTimerRef.current);
        applyTimerRef.current = null;
      }
      ro.disconnect();
      unlistenSettings();
      onDataDisposable.dispose();
      onResizeDisposable.dispose();
      if (unlistenOutput) unlistenOutput();
      if (unlistenExit) unlistenExit();
      try { term.dispose(); } catch {}
      termRef.current = null;
      fitRef.current = null;
      // Detach the PTY when component unmounts (multiplexer keeps running).
      ptyDetach().catch(() => {});
      attachedRef.current = false;
      autoAttachedRef.current = false;
    };
  }, []);

  // Detect multiplexer + matching sessions for this cwd.
  // The fast-path attach (PID/cache based) runs concurrently with this — once
  // it wins, detection results must not clobber the "attached" status, or the
  // selector overlay would reappear over a live terminal.
  const refreshSessions = useCallback(async () => {
    if (attachedRef.current) return;
    setStatus("loading");
    setErrorMsg(null);
    try {
      const cfg = await getMultiplexerConfig();
      if (!cfg.multiplexer || cfg.multiplexer === "none") {
        setMultiplexer(null);
        setStatus("error");
        setErrorMsg("Multiplexer not configured. Set zellij or tmux in Settings.");
        return;
      }
      setMultiplexer(cfg.multiplexer);
      const result = await detectMultiplexerSessions(cwd, cfg.multiplexer);
      setSessions(result.sessions);
      if (!attachedRef.current) setStatus("ready");
    } catch (e) {
      if (attachedRef.current) return;
      setStatus("error");
      setErrorMsg(String(e));
    }
  }, [cwd]);

  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  const attachTo = useCallback(async (name: string) => {
    if (!multiplexer || !termRef.current) return;
    try {
      const term = termRef.current;
      // Reset visible buffer so old multiplexer paint doesn't linger.
      term.reset();
      const cols = term.cols;
      const rows = term.rows;
      await ptyAttachMultiplexer(multiplexer, name, cwd, cols, rows);
      attachedRef.current = true;
      setStatus("attached");
      lastSessionByCwd.set(cwd, { multiplexer, name });
    } catch (e) {
      setStatus("error");
      setErrorMsg(String(e));
    }
  }, [multiplexer, cwd]);

  const createNew = useCallback(async () => {
    if (!multiplexer || !termRef.current) return;
    const name = (createName.trim() || `cc-${Date.now().toString(36)}`).replace(/\s+/g, "-");
    try {
      const term = termRef.current;
      term.reset();
      const cols = term.cols;
      const rows = term.rows;
      await ptyCreateMultiplexer(multiplexer, name, cwd, cols, rows);
      attachedRef.current = true;
      setStatus("attached");
      lastSessionByCwd.set(cwd, { multiplexer, name });
    } catch (e) {
      setStatus("error");
      setErrorMsg(String(e));
    }
  }, [multiplexer, cwd, createName]);

  const detach = useCallback(async () => {
    try { await ptyDetach(); } catch {}
    attachedRef.current = false;
    setStatus("ready");
    refreshSessions();
  }, [refreshSessions]);

  // Auto-attach to first matching session if exactly one matches the cwd.
  useEffect(() => {
    if (autoAttachedRef.current) return;
    if (status !== "ready") return;
    const matching = sessions.filter((s) => s.matchesPath && s.status !== "exited");
    if (matching.length === 1) {
      autoAttachedRef.current = true;
      attachTo(matching[0].name);
    }
  }, [status, sessions, attachTo]);

  return (
    <div
      className={`flex flex-col bg-[#1e1e1e] border-t border-zinc-800 ${
        fullscreen ? "fixed inset-0 z-40" : "h-full"
      }`}
    >
      <div className="flex items-center gap-2 px-3 py-1.5 bg-zinc-900 border-b border-zinc-800 text-xs text-zinc-300 shrink-0">
        <span className="font-medium">Terminal</span>
        {multiplexer && (
          <span className="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400 font-mono text-[10px] uppercase">
            {multiplexer}
          </span>
        )}
        {status === "attached" && (
          <span className="text-emerald-400 text-[10px]">● attached</span>
        )}
        {grid && (
          <span
            className="text-zinc-500 font-mono text-[10px]"
            title="Terminal grid (cols×rows). Keep it ≥ your external terminal's grid so attaching here doesn't shrink the shared session."
          >
            {grid.cols}×{grid.rows}
          </span>
        )}
        <div className="flex-1" />
        <button
          onClick={() => setFullscreen((v) => !v)}
          className="px-2 py-0.5 text-xs border border-zinc-700 rounded hover:bg-zinc-800"
          title={fullscreen ? "Exit fullscreen" : "Fullscreen (cover the whole window for a larger grid)"}
        >
          {fullscreen ? "⤡" : "⤢"}
        </button>
        <div className="relative" ref={displayPopoverRef}>
          <button
            onClick={() => setShowDisplaySettings((v) => !v)}
            className="px-2 py-0.5 text-xs border border-zinc-700 rounded hover:bg-zinc-800"
            title="Display settings (font size / line height / letter spacing, per project)"
          >
            Aa
          </button>
          {showDisplaySettings && (
            <div className="absolute right-0 top-7 z-30 w-64 p-3 bg-zinc-900 border border-zinc-700 rounded-lg shadow-lg space-y-3">
              <DisplaySlider
                label="Font size"
                value={display.fontSize}
                min={DISPLAY_FONT_SIZE_MIN}
                max={DISPLAY_FONT_SIZE_MAX}
                step={DISPLAY_FONT_SIZE_STEP}
                format={(v) => `${v}px`}
                onChange={(v) => updateDisplay({ fontSize: v })}
              />
              <DisplaySlider
                label="Line height"
                value={display.lineHeight}
                min={DISPLAY_LINE_HEIGHT_MIN}
                max={DISPLAY_LINE_HEIGHT_MAX}
                step={DISPLAY_LINE_HEIGHT_STEP}
                format={(v) => v.toFixed(2)}
                onChange={(v) => updateDisplay({ lineHeight: v })}
              />
              <DisplaySlider
                label="Letter spacing"
                value={display.letterSpacing}
                min={DISPLAY_LETTER_SPACING_MIN}
                max={DISPLAY_LETTER_SPACING_MAX}
                step={DISPLAY_LETTER_SPACING_STEP}
                format={(v) => `${v}px`}
                onChange={(v) => updateDisplay({ letterSpacing: v })}
              />
              <div className="flex items-center justify-between pt-1 border-t border-zinc-800">
                <span className="text-zinc-500 font-mono text-[10px]">
                  {grid ? `${grid.cols}×${grid.rows}` : "—"}
                </span>
                <button
                  onClick={resetDisplay}
                  className="px-2 py-0.5 text-[10px] border border-zinc-700 rounded hover:bg-zinc-800 text-zinc-400"
                >
                  Reset to defaults
                </button>
              </div>
            </div>
          )}
        </div>
        {status === "attached" && (
          <button
            onClick={detach}
            className="px-2 py-0.5 text-xs border border-zinc-700 rounded hover:bg-zinc-800"
          >
            Detach
          </button>
        )}
        {onClose && (
          <button
            onClick={onClose}
            className="px-2 py-0.5 text-xs border border-zinc-700 rounded hover:bg-zinc-800"
          >
            Close
          </button>
        )}
      </div>

      {/* Selector overlay (only shown when not yet attached) */}
      {status !== "attached" && (
        <div className="px-3 py-2 bg-zinc-900 border-b border-zinc-800 text-xs text-zinc-300 shrink-0 space-y-2">
          {status === "loading" && <div className="text-zinc-400">Loading multiplexer sessions...</div>}
          {status === "error" && (
            <div className="text-red-400">
              {errorMsg}
              <button onClick={refreshSessions} className="ml-2 underline">retry</button>
            </div>
          )}
          {status === "ready" && multiplexer && (
            <>
              {sessions.length > 0 ? (
                <div>
                  <div className="text-zinc-400 mb-1">Sessions:</div>
                  <div className="flex flex-wrap gap-1">
                    {sessions.map((s) => {
                      const isExited = s.status === "exited";
                      return (
                        <button
                          key={s.name}
                          onClick={() => !isExited && attachTo(s.name)}
                          disabled={isExited}
                          className={`px-2 py-0.5 rounded border text-xs inline-flex items-center gap-1 ${
                            isExited
                              ? "border-zinc-800 text-zinc-500 line-through cursor-not-allowed opacity-60"
                              : s.matchesPath
                              ? "border-emerald-700 text-emerald-300 hover:bg-emerald-900/30"
                              : "border-zinc-700 text-zinc-400 hover:bg-zinc-800"
                          }`}
                          title={
                            isExited
                              ? `${s.name} — exited (cannot attach)`
                              : s.cwd || s.name
                          }
                        >
                          <span>{s.name}</span>
                          {s.matchesPath && !isExited && <span>★</span>}
                          {isExited && (
                            <span className="px-1 py-0 rounded bg-zinc-800 text-zinc-400 text-[9px] font-mono uppercase tracking-wide no-underline">
                              exited
                            </span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                </div>
              ) : (
                <div className="text-zinc-400">No existing {multiplexer} sessions.</div>
              )}
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  value={createName}
                  onChange={(e) => setCreateName(e.target.value)}
                  placeholder="new session name"
                  className="px-2 py-0.5 bg-zinc-800 border border-zinc-700 rounded text-xs text-zinc-200 font-mono w-48"
                />
                <button
                  onClick={createNew}
                  className="px-2 py-0.5 border border-zinc-700 rounded text-xs hover:bg-zinc-800"
                >
                  + Create new
                </button>
              </div>
            </>
          )}
        </div>
      )}

      <div ref={hostRef} className="flex-1 min-h-0 overflow-hidden" />
    </div>
  );
}

function DisplaySlider({
  label,
  value,
  min,
  max,
  step,
  format,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  format: (v: number) => string;
  onChange: (v: number) => void;
}) {
  return (
    <div>
      <div className="flex items-center justify-between text-[10px] text-zinc-400 mb-1">
        <span>{label}</span>
        <span className="font-mono">{format(value)}</span>
      </div>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1 accent-emerald-500 cursor-pointer"
      />
    </div>
  );
}
