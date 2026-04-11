import { useEffect, useMemo, useState } from "react";
import { listProjects, refreshIndex } from "../../lib/tauri";
import type { Project } from "../../lib/types";
import { formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

export function ProjectList() {
  const { selectProject, selectProjectGroup } = useAppStore();
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    const data = await listProjects("time");
    setProjects(data);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handleRefresh = async () => {
    await refreshIndex();
    await load();
  };

  // Group by displayName
  const groups = useMemo(() => {
    const map = new Map<string, Project[]>();
    for (const p of projects) {
      if (p.sessionCount === 0) continue;
      const existing = map.get(p.displayName) || [];
      existing.push(p);
      map.set(p.displayName, existing);
    }
    const result: { displayName: string; projects: Project[]; totalSessions: number; lastActive: number | null }[] = [];
    for (const [displayName, projs] of map) {
      result.push({
        displayName,
        projects: projs,
        totalSessions: projs.reduce((sum, p) => sum + p.sessionCount, 0),
        lastActive: Math.max(...projs.map((p) => p.lastActive || 0)) || null,
      });
    }
    result.sort((a, b) => (b.lastActive || 0) - (a.lastActive || 0));
    return result;
  }, [projects]);

  const handleClick = (group: { displayName: string; projects: Project[] }) => {
    if (group.projects.length === 1) {
      selectProject(group.projects[0].id);
    } else {
      selectProjectGroup(group.displayName);
    }
  };

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold">Projects</h1>
        <button
          onClick={handleRefresh}
          className="text-sm px-3 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          Refresh
        </button>
      </div>
      {loading ? (
        <div className="text-zinc-500">Scanning sessions...</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {groups.map((g) => (
            <button
              key={g.displayName}
              onClick={() => handleClick(g)}
              className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
            >
              <div className="font-medium flex items-center gap-2">
                {g.displayName}
                {g.projects.length > 1 && (
                  <span className="text-xs text-zinc-400 bg-zinc-100 dark:bg-zinc-800 px-1.5 py-0.5 rounded">
                    {g.projects.length} locations
                  </span>
                )}
              </div>
              {g.projects.length === 1 ? (
                <div className="text-sm text-zinc-500 truncate mt-0.5">{g.projects[0].originalPath}</div>
              ) : (
                <div className="text-sm text-zinc-500 truncate mt-0.5">
                  {g.projects.map((p) => p.originalPath.split("/").slice(-2).join("/")).join(", ")}
                </div>
              )}
              <div className="text-xs text-zinc-400 mt-2">
                {g.totalSessions} sessions &middot; {formatRelativeTime(g.lastActive)}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
