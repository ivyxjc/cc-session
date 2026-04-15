import { useEffect, useState } from "react";
import { codexListSessions } from "../../lib/tauri";
import type { CodexSession } from "../../lib/types";
import { formatRelativeTime, formatTokens } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

export function CodexSessionList() {
  const selectedCodexCwd = useAppStore((s) => s.selectedCodexCwd);
  const selectCodexSession = useAppStore((s) => s.selectCodexSession);
  const setView = useAppStore((s) => s.setView);
  const [sessions, setSessions] = useState<CodexSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [showArchived, setShowArchived] = useState(false);

  useEffect(() => {
    setLoading(true);
    codexListSessions({ cwd: selectedCodexCwd ?? undefined, showArchived }).then((data) => {
      setSessions(data);
      setLoading(false);
    }).catch((e) => {
      console.error(e);
      setLoading(false);
    });
  }, [selectedCodexCwd, showArchived]);

  const isAllSessions = !selectedCodexCwd;
  const displayName = isAllSessions ? "All Codex Sessions" : selectedCodexCwd?.split("/").pop() || "Sessions";

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center gap-2 mb-4">
        {!isAllSessions && (
          <button
            onClick={() => setView("codexProjects")}
            className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
          >
            &larr; Projects
          </button>
        )}
        <h1 className="text-xl font-semibold">{displayName}</h1>
        <span className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950 px-2 py-0.5 rounded-full font-medium">
          Codex
        </span>
        <div className="flex-1" />
        <label className="flex items-center gap-1 text-xs text-zinc-500">
          <input
            type="checkbox"
            checked={showArchived}
            onChange={(e) => setShowArchived(e.target.checked)}
            className="rounded border-zinc-300"
          />
          Show archived
        </label>
      </div>
      {selectedCodexCwd && <div className="text-xs text-zinc-400 mb-3 truncate">{selectedCodexCwd}</div>}

      {loading ? (
        <div className="text-zinc-500">Loading sessions...</div>
      ) : sessions.length === 0 ? (
        <div className="text-zinc-500">No sessions found.</div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <button
              key={s.id}
              onClick={() => selectCodexSession(s.id)}
              className={`w-full text-left p-4 rounded-lg border transition-colors ${
                s.archived
                  ? "border-zinc-200 dark:border-zinc-800 opacity-60"
                  : "border-zinc-200 dark:border-zinc-800 hover:border-emerald-400 dark:hover:border-emerald-600"
              }`}
            >
              <div className="flex items-center gap-2">
                <div className="font-medium text-sm truncate flex-1">
                  {s.title || s.firstUserMessage.slice(0, 80) || "Untitled"}
                </div>
                <SourceBadge source={s.source} />
                {s.archived && (
                  <span className="text-xs text-zinc-400 bg-zinc-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">
                    archived
                  </span>
                )}
              </div>
              {s.firstUserMessage && s.title !== s.firstUserMessage && (
                <div className="text-xs text-zinc-500 mt-1 truncate">
                  {s.firstUserMessage.slice(0, 120)}
                </div>
              )}
              <div className="text-xs text-zinc-400 mt-2 flex items-center gap-2 flex-wrap">
                {s.model && <span>{s.model}</span>}
                <span>{formatTokens(s.tokensUsed)} tokens</span>
                {s.gitBranch && <span>branch: {s.gitBranch}</span>}
                {s.subagentCount > 0 && <span>{s.subagentCount} subagents</span>}
                <span>{s.approvalMode}</span>
                <span>{formatRelativeTime(s.updatedAt * 1000)}</span>
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function SourceBadge({ source }: { source: string }) {
  const colors: Record<string, string> = {
    cli: "text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-950",
    vscode: "text-purple-600 dark:text-purple-400 bg-purple-50 dark:bg-purple-950",
    exec: "text-orange-600 dark:text-orange-400 bg-orange-50 dark:bg-orange-950",
  };
  return (
    <span className={`text-xs px-1.5 py-0.5 rounded font-medium ${colors[source] || "text-zinc-500 bg-zinc-100 dark:bg-zinc-800"}`}>
      {source}
    </span>
  );
}
