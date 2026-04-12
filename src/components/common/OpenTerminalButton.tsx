import { useEffect, useState } from "react";
import { openTerminal, getTerminalConfig } from "../../lib/tauri";
import type { TerminalConfig } from "../../lib/types";

export function OpenTerminalButton({ path, sessionId }: { path: string; sessionId?: string }) {
  const [config, setConfig] = useState<TerminalConfig | null>(null);
  const [showDropdown, setShowDropdown] = useState(false);

  useEffect(() => {
    getTerminalConfig().then(setConfig).catch(console.error);
  }, []);

  const handleOpen = (e: React.MouseEvent, terminalName?: string) => {
    e.stopPropagation();
    if (sessionId) {
      navigator.clipboard.writeText(`claude --resume ${sessionId}`).catch(console.error);
    }
    openTerminal(path, terminalName).catch((err) => {
      console.error("open_terminal failed:", err);
      alert(`Failed to open terminal: ${err}`);
    });
  };

  const hasMultiple = config && config.terminals.length > 1;

  return (
    <div className="relative flex" onClick={(e) => e.stopPropagation()}>
      <button
        onClick={(e) => handleOpen(e)}
        className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded-l hover:bg-zinc-100 dark:hover:bg-zinc-800"
        title={`Open in ${config?.defaultTerminal || "Terminal"}`}
      >
        &gt;_
      </button>
      {hasMultiple && (
        <button
          onClick={(e) => { e.stopPropagation(); setShowDropdown(!showDropdown); }}
          className="px-1 py-1 text-xs border border-l-0 border-zinc-300 dark:border-zinc-700 rounded-r hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          ▾
        </button>
      )}
      {!hasMultiple && (
        <span className="px-1 py-1 text-xs border border-l-0 border-zinc-300 dark:border-zinc-700 rounded-r" />
      )}
      {showDropdown && config && (
        <div className="absolute right-0 top-7 z-10 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded shadow-lg min-w-32">
          {config.terminals.map((t) => (
            <button
              key={t.name}
              onClick={(e) => { handleOpen(e, t.name); setShowDropdown(false); }}
              className="w-full text-left px-3 py-1.5 text-xs hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
            >
              {t.name}
              {t.name === config.defaultTerminal && <span className="text-zinc-400">(default)</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
