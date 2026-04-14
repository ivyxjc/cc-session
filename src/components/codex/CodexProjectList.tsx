import { useEffect, useState } from "react";
import { codexListProjects } from "../../lib/tauri";
import type { CodexProject } from "../../lib/types";
import { formatRelativeTime, formatTokens } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

export function CodexProjectList() {
  const selectCodexProject = useAppStore((s) => s.selectCodexProject);
  const [projects, setProjects] = useState<CodexProject[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    codexListProjects("time").then((data) => {
      setProjects(data);
      setLoading(false);
    }).catch((e) => {
      console.error(e);
      setLoading(false);
    });
  }, []);

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center gap-3 mb-4">
        <h1 className="text-xl font-semibold">Codex Projects</h1>
        <span className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950 px-2 py-0.5 rounded-full font-medium">
          OpenAI Codex
        </span>
      </div>
      {loading ? (
        <div className="text-zinc-500">Loading Codex projects...</div>
      ) : projects.length === 0 ? (
        <div className="text-zinc-500">No Codex sessions found. Make sure Codex CLI is installed and has session data at ~/.codex/</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {projects.map((p) => (
            <button
              key={p.cwd}
              onClick={() => selectCodexProject(p.cwd)}
              className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-emerald-400 dark:hover:border-emerald-600 transition-colors"
            >
              <div className="font-medium">{p.displayName}</div>
              <div className="text-sm text-zinc-500 truncate mt-0.5">{p.cwd}</div>
              <div className="text-xs text-zinc-400 mt-2">
                {p.sessionCount} sessions &middot; {formatTokens(p.totalTokens)} tokens &middot; {formatRelativeTime(p.lastActive * 1000)}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
