import { useState, useEffect } from "react";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { toast } from "../../stores/toastStore";
import {
  getBackupConfig, setBackupConfig, migrateBackups, getTerminalConfig, setTerminalConfig, testTerminalCommand,
  getMultiplexerConfig, setMultiplexerConfig, getAutoHideConfig, setAutoHideConfig,
  exportSettingsToFile, importSettingsFromFile,
  getAiSummaryConfig, setAiSummaryConfig, testAiSummaryConnection, generateAiSummariesBatch,
  getLiveSessions,
} from "../../lib/tauri";
import { setLocale as setGlobalLocale } from "../../lib/format";
import {
  getUiFont, getCodeFont, getFontSize, setUiFont, setCodeFont, setFontSize,
  getTerminalFont, getTerminalFontSize, getTerminalLineHeight, getTerminalLetterSpacing,
  setTerminalFont, setTerminalFontSize, setTerminalLineHeight, setTerminalLetterSpacing,
  TERMINAL_FONT_SIZE_DEFAULT, TERMINAL_LINE_HEIGHT_DEFAULT, TERMINAL_LETTER_SPACING_DEFAULT,
  TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX,
  TERMINAL_LINE_HEIGHT_MIN, TERMINAL_LINE_HEIGHT_MAX,
  TERMINAL_LETTER_SPACING_MIN, TERMINAL_LETTER_SPACING_MAX,
} from "../../lib/fonts";
import type { BackupConfig, TerminalConfig, MultiplexerConfig, AutoHideConfig, AiSummaryConfig, AiSummaryProgress } from "../../lib/types";

type SettingsCategory = "appearance" | "sessions" | "backup" | "integrations" | "ai";

const CATEGORIES: { id: SettingsCategory; label: string; sub: string }[] = [
  { id: "appearance", label: "Appearance", sub: "Fonts, locale, terminal" },
  { id: "sessions", label: "Sessions", sub: "Visibility & filters" },
  { id: "backup", label: "Backup", sub: "Where & how often" },
  { id: "integrations", label: "Integrations", sub: "Terminal & multiplexer" },
  { id: "ai", label: "AI Summaries", sub: "LLM provider" },
];

export function SettingsPage() {
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>("appearance");
  const [config, setConfig] = useState<BackupConfig | null>(null);
  const [originalDir, setOriginalDir] = useState<string>("");
  const [termConfig, setTermConfig] = useState<TerminalConfig | null>(null);
  const [muxConfig, setMuxConfig] = useState<MultiplexerConfig | null>(null);
  const [autoHideConfig, setAutoHideConfigState] = useState<AutoHideConfig | null>(null);
  const [locale, setLocale] = useState<string>(localStorage.getItem("locale") || "");
  const [uiFont, setUiFontState] = useState(getUiFont());
  const [codeFont, setCodeFontState] = useState(getCodeFont());
  const [fontSize, setFontSizeState] = useState(getFontSize());
  const [terminalFont, setTerminalFontState] = useState(getTerminalFont());
  const [terminalFontSize, setTerminalFontSizeState] = useState<number>(getTerminalFontSize());
  const [terminalLineHeight, setTerminalLineHeightState] = useState<number>(getTerminalLineHeight());
  const [terminalLetterSpacing, setTerminalLetterSpacingState] = useState<number>(getTerminalLetterSpacing());
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [migrating, setMigrating] = useState(false);

  // AI summary
  const [aiCfg, setAiCfg] = useState<AiSummaryConfig>({ baseUrl: "", apiKey: "", model: "" });
  const [aiTesting, setAiTesting] = useState(false);
  const [aiTestResult, setAiTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [aiBatchRunning, setAiBatchRunning] = useState(false);
  const [aiBatchProgress, setAiBatchProgress] = useState<{ current: number; total: number; ok: number; skipped: number; errors: number } | null>(null);
  const [aiBatchForce, setAiBatchForce] = useState(false);

  useEffect(() => {
    getBackupConfig().then((c) => {
      setConfig(c);
      setOriginalDir(c.backupDir);
    });
    getTerminalConfig().then(setTermConfig);
    getMultiplexerConfig().then(setMuxConfig);
    getAutoHideConfig().then(setAutoHideConfigState);
    getAiSummaryConfig().then(setAiCfg);
  }, []);

  // Listen to AI batch progress events
  useEffect(() => {
    const unlistenProgress = listen<AiSummaryProgress>("ai-summary-progress", (e) => {
      const p = e.payload;
      setAiBatchProgress((prev) => ({
        current: p.current,
        total: p.total,
        ok: (prev?.ok ?? 0) + (p.status === "ok" ? 1 : 0),
        skipped: (prev?.skipped ?? 0) + (p.status === "skipped" ? 1 : 0),
        errors: (prev?.errors ?? 0) + (p.status === "error" ? 1 : 0),
      }));
    });
    const unlistenDone = listen("ai-summary-done", () => {
      setAiBatchRunning(false);
      toast.success("Batch generation finished. Refresh the session list to see updates.");
    });
    return () => {
      unlistenProgress.then((f) => f());
      unlistenDone.then((f) => f());
    };
  }, []);

  const handleSave = async () => {
    if (!config) return;
    setSaving(true);

    // Migrate backups if directory changed
    if (config.backupDir !== originalDir) {
      setMigrating(true);
      try {
        await migrateBackups(originalDir, config.backupDir);
      } catch (e) {
        console.error("Migration failed:", e);
      }
      setMigrating(false);
      setOriginalDir(config.backupDir);
    }

    await setBackupConfig(config);
    if (termConfig) await setTerminalConfig(termConfig);
    if (muxConfig) await setMultiplexerConfig(muxConfig);
    if (autoHideConfig) await setAutoHideConfig(autoHideConfig);
    await setAiSummaryConfig(aiCfg);

    // Save fonts
    setUiFont(uiFont);
    setCodeFont(codeFont);
    setFontSize(fontSize);
    setTerminalFont(terminalFont);
    setTerminalFontSize(terminalFontSize === TERMINAL_FONT_SIZE_DEFAULT ? null : terminalFontSize);
    setTerminalLineHeight(terminalLineHeight === TERMINAL_LINE_HEIGHT_DEFAULT ? null : terminalLineHeight);
    setTerminalLetterSpacing(terminalLetterSpacing === TERMINAL_LETTER_SPACING_DEFAULT ? null : terminalLetterSpacing);

    // Save locale
    if (locale) {
      localStorage.setItem("locale", locale);
      setGlobalLocale(locale);
    } else {
      localStorage.removeItem("locale");
      setGlobalLocale(undefined);
    }

    setSaving(false);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!config) return <div className="p-6 text-zinc-500">Loading...</div>;

  const activeMeta = CATEGORIES.find((c) => c.id === activeCategory)!;

  return (
    <div className="h-full flex bg-zinc-50 dark:bg-zinc-950">
      {/* Sidebar nav */}
      <nav className="w-56 shrink-0 border-r border-zinc-200 dark:border-zinc-800 p-3 flex flex-col gap-0.5 overflow-y-auto">
        <h1 className="text-base font-semibold px-2 pt-1 pb-3">Settings</h1>
        {CATEGORIES.map((c) => (
          <button
            key={c.id}
            onClick={() => setActiveCategory(c.id)}
            className={`text-left px-2.5 py-1.5 rounded text-sm transition-colors ${
              activeCategory === c.id
                ? "bg-zinc-200 dark:bg-zinc-800 font-medium text-zinc-900 dark:text-zinc-100"
                : "hover:bg-zinc-100 dark:hover:bg-zinc-900 text-zinc-600 dark:text-zinc-400"
            }`}
          >
            <div>{c.label}</div>
            <div className={`text-[11px] mt-0.5 ${activeCategory === c.id ? "text-zinc-500 dark:text-zinc-500" : "text-zinc-400 dark:text-zinc-600"}`}>{c.sub}</div>
          </button>
        ))}
      </nav>

      {/* Content panel */}
      <div className="flex-1 min-w-0 flex flex-col bg-white dark:bg-zinc-900">
        <div className="flex-1 overflow-y-auto">
          <div className="px-8 py-6 max-w-2xl">
            <h2 className="text-2xl font-semibold mb-1">{activeMeta.label}</h2>
            <p className="text-sm text-zinc-500 mb-6">{activeMeta.sub}</p>

      {/* Backup Settings */}
      {activeCategory === "backup" && (
      <section className="space-y-4">

        <div>
          <label className="text-sm font-medium">Backup directory</label>
          <div className="flex gap-2 mt-1">
            <input
              type="text"
              value={config.backupDir}
              onChange={(e) => setConfig({ ...config, backupDir: e.target.value })}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
            />
            <button
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: "Select backup directory",
                  defaultPath: config.backupDir,
                });
                if (selected) {
                  setConfig({ ...config, backupDir: selected });
                }
              }}
              className="px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 shrink-0"
            >
              Browse
            </button>
          </div>
          <p className="text-xs text-zinc-400 mt-1">Where session backups are stored.</p>
        </div>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={config.autoBackup}
            onChange={(e) => setConfig({ ...config, autoBackup: e.target.checked })}
          />
          <span className="text-sm">Auto-backup on favorite</span>
        </label>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={config.compress}
            onChange={(e) => setConfig({ ...config, compress: e.target.checked })}
          />
          <span className="text-sm">Compress backups (zstd)</span>
        </label>

        <div>
          <label className="text-sm font-medium">Max backup copies per session</label>
          <input
            type="number"
            value={config.maxBackupCopies}
            onChange={(e) => setConfig({ ...config, maxBackupCopies: parseInt(e.target.value) || 3 })}
            className="w-20 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            min={1}
            max={99}
          />
        </div>

        <div>
          <label className="text-sm font-medium">Auto-backup interval (hours)</label>
          <input
            type="number"
            value={config.autoBackupIntervalHours}
            onChange={(e) => setConfig({ ...config, autoBackupIntervalHours: parseInt(e.target.value) || 24 })}
            className="w-20 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            min={1}
          />
        </div>

      </section>
      )}

      {/* Session Visibility */}
      {activeCategory === "sessions" && autoHideConfig && (
        <section className="space-y-4">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={autoHideConfig.enabled}
              onChange={(e) => setAutoHideConfigState({ ...autoHideConfig, enabled: e.target.checked })}
            />
            <span className="text-sm">Auto-hide small sessions</span>
          </label>
          {autoHideConfig.enabled && (
            <div>
              <label className="text-sm font-medium">Minimum message count</label>
              <input
                type="number"
                value={autoHideConfig.minMessageCount}
                onChange={(e) => setAutoHideConfigState({ ...autoHideConfig, minMessageCount: parseInt(e.target.value) || 3 })}
                className="w-20 mt-1 ml-2 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
                min={1}
              />
              <p className="text-xs text-zinc-400 mt-1">Sessions with fewer messages will be hidden (starred sessions are always shown).</p>
            </div>
          )}
        </section>
      )}

      {/* Display Settings */}
      {activeCategory === "appearance" && (
      <section className="space-y-5">
        <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 border-b border-zinc-200 dark:border-zinc-800 pb-1">Display</h3>
        <div>
          <label className="text-sm font-medium">Date/time locale</label>
          <div className="flex gap-2 mt-1 items-center">
            <select
              value={locale}
              onChange={(e) => setLocale(e.target.value)}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="">System default ({navigator.language})</option>
              <option value="zh-CN">zh-CN (2026/04/10 16:44:28)</option>
              <option value="en-US">en-US (04/10/2026, 4:44:28 PM)</option>
              <option value="en-GB">en-GB (10/04/2026, 16:44:28)</option>
              <option value="ja-JP">ja-JP (2026/04/10 16:44:28)</option>
              <option value="de-DE">de-DE (10.04.2026, 16:44:28)</option>
            </select>
            <input
              type="text"
              value={locale}
              onChange={(e) => setLocale(e.target.value)}
              placeholder="or type locale code"
              className="w-40 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
          </div>
          <p className="text-xs text-zinc-400 mt-1">Leave empty for system default. Or enter any BCP 47 locale code (e.g. zh-TW, ko-KR).</p>
        </div>

        <div>
          <label className="text-sm font-medium">UI Font</label>
          <div className="flex gap-2 mt-1 items-center">
            <select
              value={uiFont}
              onChange={(e) => setUiFontState(e.target.value)}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="">System default</option>
              <option value="Inter">Inter</option>
              <option value="SF Pro Display">SF Pro Display</option>
              <option value="Helvetica Neue">Helvetica Neue</option>
              <option value="PingFang SC">PingFang SC</option>
              <option value="Microsoft YaHei">Microsoft YaHei</option>
              <option value="Noto Sans SC">Noto Sans SC</option>
            </select>
            <input
              type="text"
              value={uiFont}
              onChange={(e) => setUiFontState(e.target.value)}
              placeholder="or type font name"
              className="w-40 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
          </div>
          <p className="text-xs text-zinc-400 mt-1">Font for UI elements. Leave empty for system default.</p>
        </div>

        <div>
          <label className="text-sm font-medium">Code Font</label>
          <div className="flex gap-2 mt-1 items-center">
            <select
              value={codeFont}
              onChange={(e) => setCodeFontState(e.target.value)}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="">System default</option>
              <option value="JetBrains Mono">JetBrains Mono</option>
              <option value="Fira Code">Fira Code</option>
              <option value="Source Code Pro">Source Code Pro</option>
              <option value="SF Mono">SF Mono</option>
              <option value="Cascadia Code">Cascadia Code</option>
              <option value="Monaco">Monaco</option>
              <option value="Menlo">Menlo</option>
            </select>
            <input
              type="text"
              value={codeFont}
              onChange={(e) => setCodeFontState(e.target.value)}
              placeholder="or type font name"
              className="w-40 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
          </div>
          <p className="text-xs text-zinc-400 mt-1">Font for code blocks and monospace text. Leave empty for system default.</p>
        </div>

        <div>
          <label className="text-sm font-medium">Font Size</label>
          <div className="flex gap-2 mt-1 items-center">
            <select
              value={fontSize}
              onChange={(e) => setFontSizeState(e.target.value)}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="">Default (16px)</option>
              <option value="12px">12px</option>
              <option value="13px">13px</option>
              <option value="14px">14px</option>
              <option value="15px">15px</option>
              <option value="16px">16px</option>
              <option value="18px">18px</option>
            </select>
          </div>
          <p className="text-xs text-zinc-400 mt-1">Base font size for the entire app.</p>
        </div>
      </section>
      )}

      {/* Embedded Terminal Font */}
      {activeCategory === "appearance" && (
      <section className="space-y-5 mt-8">
        <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 border-b border-zinc-200 dark:border-zinc-800 pb-1">Embedded Terminal</h3>
        <p className="text-xs text-zinc-500">
          Font settings for the embedded zellij / tmux terminal pane (the one in live session view).
          Independent from the app-wide Code Font; falls back to Code Font when left empty.
        </p>

        <div>
          <label className="text-sm font-medium">Terminal Font</label>
          <div className="flex gap-2 mt-1 items-center">
            <select
              value={terminalFont}
              onChange={(e) => setTerminalFontState(e.target.value)}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="">Fall back to Code Font / system mono</option>
              <option value="JetBrains Mono">JetBrains Mono</option>
              <option value="JetBrainsMono Nerd Font Mono">JetBrainsMono Nerd Font Mono</option>
              <option value="Maple Mono NF CN">Maple Mono NF CN</option>
              <option value="Fira Code">Fira Code</option>
              <option value="SF Mono">SF Mono</option>
              <option value="Menlo">Menlo</option>
              <option value="Monaco">Monaco</option>
              <option value="Cascadia Code">Cascadia Code</option>
            </select>
            <input
              type="text"
              value={terminalFont}
              onChange={(e) => setTerminalFontState(e.target.value)}
              placeholder="or custom font name"
              className="w-40 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
          </div>
          <p className="text-xs text-zinc-400 mt-1">
            Monospace font for the terminal grid. Tip: a Nerd Font lets zellij status icons render correctly.
          </p>
        </div>

        <div>
          <label className="text-sm font-medium">Terminal Font Size (px)</label>
          <input
            type="number"
            min={TERMINAL_FONT_SIZE_MIN}
            max={TERMINAL_FONT_SIZE_MAX}
            step={0.5}
            value={terminalFontSize}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              if (Number.isFinite(v)) {
                setTerminalFontSizeState(Math.max(TERMINAL_FONT_SIZE_MIN, Math.min(TERMINAL_FONT_SIZE_MAX, v)));
              }
            }}
            className="w-24 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
          />
          <span className="ml-2 text-xs text-zinc-400">Default {TERMINAL_FONT_SIZE_DEFAULT}px</span>
        </div>

        <div>
          <label className="text-sm font-medium">Terminal Line Height</label>
          <div className="flex items-center gap-2 mt-1">
            <input
              type="number"
              min={TERMINAL_LINE_HEIGHT_MIN}
              max={TERMINAL_LINE_HEIGHT_MAX}
              step={0.05}
              value={terminalLineHeight.toFixed(2)}
              onChange={(e) => {
                const v = parseFloat(e.target.value);
                if (Number.isFinite(v)) {
                  setTerminalLineHeightState(Math.max(TERMINAL_LINE_HEIGHT_MIN, Math.min(TERMINAL_LINE_HEIGHT_MAX, v)));
                }
              }}
              className="w-24 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
            <span className="text-xs text-zinc-400">
              Default {TERMINAL_LINE_HEIGHT_DEFAULT.toFixed(2)} · range {TERMINAL_LINE_HEIGHT_MIN}–{TERMINAL_LINE_HEIGHT_MAX}
            </span>
          </div>
          <p className="text-xs text-zinc-400 mt-1">
            xterm cell-height multiplier. Bump up if cells feel cramped, down to match another terminal's grid (zellij broadcasts the smallest client size to all).
          </p>
        </div>

        <div>
          <label className="text-sm font-medium">Terminal Letter Spacing (px)</label>
          <div className="flex items-center gap-2 mt-1">
            <input
              type="number"
              min={TERMINAL_LETTER_SPACING_MIN}
              max={TERMINAL_LETTER_SPACING_MAX}
              step={0.5}
              value={terminalLetterSpacing}
              onChange={(e) => {
                const v = parseFloat(e.target.value);
                if (Number.isFinite(v)) {
                  setTerminalLetterSpacingState(Math.max(TERMINAL_LETTER_SPACING_MIN, Math.min(TERMINAL_LETTER_SPACING_MAX, v)));
                }
              }}
              className="w-24 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            />
            <span className="text-xs text-zinc-400">
              Default {TERMINAL_LETTER_SPACING_DEFAULT} · range {TERMINAL_LETTER_SPACING_MIN}–{TERMINAL_LETTER_SPACING_MAX}
            </span>
          </div>
          <p className="text-xs text-zinc-400 mt-1">
            Extra pixels between cells. These three values are the defaults for the terminal pane's per-project "Aa" tuning; projects you've tuned keep their own values.
          </p>
        </div>
      </section>
      )}

      {/* Terminal launcher */}
      {activeCategory === "integrations" && termConfig && (
        <section className="space-y-5">
          <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 border-b border-zinc-200 dark:border-zinc-800 pb-1">Terminal Launcher</h3>

          <div>
            <label className="text-sm font-medium">Default terminal</label>
            <select
              value={termConfig.defaultTerminal}
              onChange={(e) => setTermConfig({ ...termConfig, defaultTerminal: e.target.value })}
              className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              {termConfig.terminals.map((t) => (
                <option key={t.name} value={t.name}>{t.name}</option>
              ))}
            </select>
          </div>

          <div className="space-y-3">
            <label className="text-sm font-medium">Terminals</label>
            {termConfig.terminals.map((t, i) => (
              <div key={i} className="flex gap-2 items-start">
                <div className="flex-1 space-y-1">
                  <input
                    type="text"
                    value={t.name}
                    onChange={(e) => {
                      const updated = [...termConfig.terminals];
                      updated[i] = { ...updated[i], name: e.target.value };
                      setTermConfig({ ...termConfig, terminals: updated });
                    }}
                    placeholder="Name"
                    className="w-full px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
                  />
                  <input
                    type="text"
                    value={t.command}
                    onChange={(e) => {
                      const updated = [...termConfig.terminals];
                      updated[i] = { ...updated[i], command: e.target.value };
                      setTermConfig({ ...termConfig, terminals: updated });
                    }}
                    placeholder="Command (use {path} as placeholder)"
                    className="w-full px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
                  />
                </div>
                <div className="flex flex-col gap-1 mt-0.5">
                  <button
                    onClick={() => testTerminalCommand(t.command).catch(console.error)}
                    className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
                  >
                    Test
                  </button>
                  <button
                    onClick={() => {
                      const updated = termConfig.terminals.filter((_, j) => j !== i);
                      setTermConfig({ ...termConfig, terminals: updated });
                    }}
                    className="px-2 py-1 text-xs text-red-500 hover:text-red-700"
                  >
                    Remove
                  </button>
                </div>
              </div>
            ))}
            <button
              onClick={() => {
                setTermConfig({
                  ...termConfig,
                  terminals: [...termConfig.terminals, { name: "", command: "" }],
                });
              }}
              className="text-sm text-blue-600 hover:text-blue-700"
            >
              + Add terminal
            </button>
          </div>
          <p className="text-xs text-zinc-400">Use <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">{"{path}"}</code> in the command as a placeholder for the project directory.</p>
        </section>
      )}

      {/* Multiplexer Integration */}
      {activeCategory === "integrations" && muxConfig && (
        <section className="space-y-5 mt-8">
          <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 border-b border-zinc-200 dark:border-zinc-800 pb-1">Multiplexer</h3>
          <div>
            <label className="text-sm font-medium">Terminal multiplexer</label>
            <select
              value={muxConfig.multiplexer}
              onChange={(e) => setMuxConfig({ ...muxConfig, multiplexer: e.target.value })}
              className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            >
              <option value="none">None</option>
              <option value="zellij">Zellij</option>
              <option value="tmux">tmux</option>
            </select>
            <p className="text-xs text-zinc-400 mt-1">
              When enabled, a multiplexer button appears on session cards. Click it to see attach commands for existing sessions.
            </p>
          </div>
        </section>
      )}

      {/* AI Summaries */}
      {activeCategory === "ai" && (
      <section className="space-y-4">
        <p className="text-xs text-zinc-500">
          Use any OpenAI-compatible endpoint to auto-generate session titles and tags.
          Examples:
          {" "}
          <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">https://api.openai.com/v1</code>
          {" / "}
          <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">https://api.deepseek.com/v1</code>
          {" / "}
          <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">https://dashscope.aliyuncs.com/compatible-mode/v1</code>
          {" / "}
          <code className="bg-zinc-100 dark:bg-zinc-800 px-1 rounded">http://localhost:11434/v1</code>
        </p>

        <div>
          <label className="text-sm font-medium">Base URL</label>
          <input
            type="text"
            value={aiCfg.baseUrl}
            onChange={(e) => { setAiCfg({ ...aiCfg, baseUrl: e.target.value }); setAiTestResult(null); }}
            placeholder="https://api.openai.com/v1"
            className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
          />
        </div>
        <div>
          <label className="text-sm font-medium">API Key</label>
          <input
            type="password"
            value={aiCfg.apiKey}
            onChange={(e) => { setAiCfg({ ...aiCfg, apiKey: e.target.value }); setAiTestResult(null); }}
            placeholder="sk-..."
            className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
          />
          <p className="text-xs text-zinc-400 mt-1">Stored locally. Only sent to the configured Base URL.</p>
        </div>
        <div>
          <label className="text-sm font-medium">Model</label>
          <input
            type="text"
            value={aiCfg.model}
            onChange={(e) => { setAiCfg({ ...aiCfg, model: e.target.value }); setAiTestResult(null); }}
            placeholder="gpt-4o-mini / deepseek-chat / qwen-flash / claude-haiku-4-5-20251001"
            className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
          />
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={async () => {
              setAiTesting(true);
              setAiTestResult(null);
              try {
                const r = await testAiSummaryConnection(aiCfg);
                setAiTestResult({ ok: true, message: `OK · ${r.slice(0, 60)}` });
              } catch (e) {
                setAiTestResult({ ok: false, message: String(e) });
              } finally {
                setAiTesting(false);
              }
            }}
            disabled={aiTesting || !aiCfg.baseUrl || !aiCfg.apiKey || !aiCfg.model}
            className="px-3 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
          >
            {aiTesting ? "Testing..." : "Test connection"}
          </button>
          {aiTestResult && (
            <span className={`text-xs ${aiTestResult.ok ? "text-emerald-600 dark:text-emerald-400" : "text-red-600 dark:text-red-400"}`}>
              {aiTestResult.message}
            </span>
          )}
        </div>

        <div className="border-t border-zinc-200 dark:border-zinc-800 pt-3">
          <div className="text-sm font-medium mb-1">Generate for all sessions</div>
          <p className="text-xs text-zinc-500 mb-2">
            Walks every session and calls the LLM. Sessions whose content hasn't changed since the last run are skipped via a content-hash cache (free).
          </p>
          <label className="flex items-center gap-2 mb-2">
            <input type="checkbox" checked={aiBatchForce} onChange={(e) => setAiBatchForce(e.target.checked)} />
            <span className="text-xs">Force regenerate (ignore cache)</span>
          </label>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={async () => {
                if (!confirm("This will call the LLM for every session that has changed since last run. Continue?")) return;
                setAiBatchRunning(true);
                setAiBatchProgress({ current: 0, total: 0, ok: 0, skipped: 0, errors: 0 });
                try {
                  const total = await generateAiSummariesBatch(aiBatchForce);
                  if (total === 0) {
                    setAiBatchRunning(false);
                    toast.success("No sessions to process.");
                  }
                } catch (e) {
                  setAiBatchRunning(false);
                  toast.error(`Batch failed to start: ${e}`);
                }
              }}
              disabled={aiBatchRunning || !aiCfg.baseUrl || !aiCfg.apiKey || !aiCfg.model}
              className="px-3 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
            >
              {aiBatchRunning ? "Running..." : "Generate for all sessions"}
            </button>
            <button
              onClick={async () => {
                // Live sessions only — pull current live monitor state and filter to those
                // with a DB-backed session id (the ones that have actually been indexed).
                let liveIds: number[] = [];
                try {
                  const live = await getLiveSessions();
                  liveIds = live
                    .map((s) => s.dbSessionId)
                    .filter((id): id is number => id != null);
                } catch (e) {
                  toast.error(`Failed to fetch live sessions: ${e}`);
                  return;
                }
                if (liveIds.length === 0) {
                  toast.success("No live sessions right now.");
                  return;
                }
                if (!confirm(`This will call the LLM for ${liveIds.length} currently live session(s). Continue?`)) return;
                setAiBatchRunning(true);
                setAiBatchProgress({ current: 0, total: 0, ok: 0, skipped: 0, errors: 0 });
                try {
                  const total = await generateAiSummariesBatch(aiBatchForce, liveIds);
                  if (total === 0) {
                    setAiBatchRunning(false);
                    toast.success("No sessions to process.");
                  }
                } catch (e) {
                  setAiBatchRunning(false);
                  toast.error(`Batch failed to start: ${e}`);
                }
              }}
              disabled={aiBatchRunning || !aiCfg.baseUrl || !aiCfg.apiKey || !aiCfg.model}
              className="px-3 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
              title="Only process sessions whose Claude Code process is currently running"
            >
              {aiBatchRunning ? "Running..." : "Generate for all live sessions"}
            </button>
          </div>
          {aiBatchProgress && aiBatchProgress.total > 0 && (
            <div className="mt-2 text-xs text-zinc-500">
              {aiBatchProgress.current} / {aiBatchProgress.total}
              {" · "}{aiBatchProgress.ok} generated
              {" · "}{aiBatchProgress.skipped} skipped
              {aiBatchProgress.errors > 0 && <> · <span className="text-red-500">{aiBatchProgress.errors} errors</span></>}
            </div>
          )}
        </div>
      </section>
      )}

          </div>
        </div>

        {/* Sticky bottom action bar */}
        <div className="border-t border-zinc-200 dark:border-zinc-800 px-8 py-3 flex items-center gap-3 bg-zinc-50 dark:bg-zinc-950">
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
          >
            {migrating ? "Migrating backups..." : saving ? "Saving..." : saved ? "Saved!" : "Save"}
          </button>
          <div className="flex-1" />
          <button
            onClick={async () => {
              const filePath = await saveDialog({
                defaultPath: "cc-session-settings.json",
                filters: [{ name: "JSON", extensions: ["json"] }],
              });
              if (filePath) {
                await exportSettingsToFile(filePath);
              }
            }}
            className="px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 bg-white dark:bg-zinc-900"
          >
            Export
          </button>
          <button
            onClick={async () => {
              const filePath = await open({
                multiple: false,
                filters: [{ name: "JSON", extensions: ["json"] }],
              });
              if (filePath) {
                try {
                  await importSettingsFromFile(filePath as string);
                  getBackupConfig().then((c) => { setConfig(c); setOriginalDir(c.backupDir); });
                  getTerminalConfig().then(setTermConfig);
                  getMultiplexerConfig().then(setMuxConfig);
                  getAutoHideConfig().then(setAutoHideConfigState);
                } catch (e) {
                  toast.error(`Import failed: ${e}`);
                }
              }
            }}
            className="px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 bg-white dark:bg-zinc-900"
          >
            Import
          </button>
        </div>
      </div>
    </div>
  );
}
