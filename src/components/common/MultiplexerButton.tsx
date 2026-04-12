import { useEffect, useRef, useState } from "react";
import { detectMultiplexerSessions, getMultiplexerConfig } from "../../lib/tauri";
import type { MultiplexerDetectionResult, MultiplexerSession } from "../../lib/types";

export function MultiplexerButton({ path }: { path: string }) {
  const [config, setConfig] = useState<string>("none");
  const [showDropdown, setShowDropdown] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<MultiplexerDetectionResult | null>(null);
  const [copiedCmd, setCopiedCmd] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getMultiplexerConfig().then((c) => setConfig(c.multiplexer)).catch(console.error);
  }, []);

  // Close dropdown on outside click
  useEffect(() => {
    if (!showDropdown) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showDropdown]);

  if (config === "none") return null;

  const label = config === "zellij" ? "z" : "μ";

  const handleClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (showDropdown) {
      setShowDropdown(false);
      return;
    }
    setShowDropdown(true);
    setLoading(true);
    setResult(null);
    try {
      const r = await detectMultiplexerSessions(path, config);
      setResult(r);
    } catch (err) {
      console.error("detect_multiplexer_sessions failed:", err);
    }
    setLoading(false);
  };

  const copyCmd = (cmd: string, e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(cmd);
    setCopiedCmd(cmd);
    setTimeout(() => setCopiedCmd(null), 1500);
  };

  return (
    <div className="relative" ref={dropdownRef} onClick={(e) => e.stopPropagation()}>
      <button
        onClick={handleClick}
        className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 font-mono"
        title={`${config} sessions`}
      >
        {label}
      </button>

      {showDropdown && (
        <div className="absolute right-0 top-8 z-20 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-lg min-w-72 max-w-96 max-h-80 overflow-y-auto">
          {loading ? (
            <div className="px-4 py-3 text-xs text-zinc-400">Detecting sessions...</div>
          ) : result ? (
            <>
              {/* Matched sessions */}
              {result.sessions.filter((s) => s.matchesPath).length > 0 && (
                <div>
                  <div className="px-3 py-1.5 text-xs text-zinc-400 font-medium border-b border-zinc-100 dark:border-zinc-800">
                    Matched
                  </div>
                  {result.sessions
                    .filter((s) => s.matchesPath)
                    .map((s) => (
                      <SessionRow key={s.name} session={s} copiedCmd={copiedCmd} onCopy={copyCmd} />
                    ))}
                </div>
              )}

              {/* New session */}
              <div className="border-t border-zinc-100 dark:border-zinc-800">
                <button
                  onClick={(e) => copyCmd(result.newSessionCmd, e)}
                  className="w-full text-left px-3 py-2 hover:bg-zinc-50 dark:hover:bg-zinc-800"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-green-600 dark:text-green-400">+</span>
                    <span className="text-sm">New session</span>
                    {copiedCmd === result.newSessionCmd && (
                      <span className="text-xs text-green-500 ml-auto">Copied!</span>
                    )}
                  </div>
                  <div className="text-xs text-zinc-400 font-mono mt-0.5 truncate">
                    {result.newSessionCmd}
                  </div>
                </button>
              </div>

              {/* Other sessions */}
              {result.sessions.filter((s) => !s.matchesPath).length > 0 && (
                <div>
                  <div className="px-3 py-1.5 text-xs text-zinc-400 font-medium border-b border-zinc-100 dark:border-zinc-800">
                    All Sessions
                  </div>
                  {result.sessions
                    .filter((s) => !s.matchesPath)
                    .map((s) => (
                      <SessionRow key={s.name} session={s} copiedCmd={copiedCmd} onCopy={copyCmd} />
                    ))}
                </div>
              )}
            </>
          ) : (
            <div className="px-4 py-3 text-xs text-red-400">Failed to detect sessions</div>
          )}
        </div>
      )}
    </div>
  );
}

function SessionRow({
  session,
  copiedCmd,
  onCopy,
}: {
  session: MultiplexerSession;
  copiedCmd: string | null;
  onCopy: (cmd: string, e: React.MouseEvent) => void;
}) {
  const isActive = session.status === "active";
  return (
    <button
      onClick={(e) => onCopy(session.attachCmd, e)}
      className="w-full text-left px-3 py-2 hover:bg-zinc-50 dark:hover:bg-zinc-800"
    >
      <div className="flex items-center gap-2">
        <span className={`w-2 h-2 rounded-full shrink-0 ${isActive ? "bg-green-500" : "bg-zinc-400"}`} />
        <span className="text-sm font-medium truncate">{session.name}</span>
        {session.matchesPath && (
          <span className="text-xs text-green-600 dark:text-green-400 shrink-0">matched</span>
        )}
        {!isActive && (
          <span className="text-xs text-zinc-400 shrink-0">exited</span>
        )}
        {copiedCmd === session.attachCmd && (
          <span className="text-xs text-green-500 ml-auto shrink-0">Copied!</span>
        )}
      </div>
      <div className="text-xs text-zinc-400 font-mono mt-0.5 truncate">{session.attachCmd}</div>
      {session.cwd && !session.matchesPath && (
        <div className="text-xs text-zinc-400 mt-0.5 truncate">{session.cwd}</div>
      )}
    </button>
  );
}
