import { useEffect, useMemo, useState } from "react";
import { listProjects } from "../../lib/tauri";
import type { Project } from "../../lib/types";
import { formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

function longestCommonPrefix(paths: string[]): string {
  if (paths.length === 0) return "";
  if (paths.length === 1) return paths[0].substring(0, paths[0].lastIndexOf("/") + 1);

  let prefix = paths[0];
  for (let i = 1; i < paths.length; i++) {
    while (!paths[i].startsWith(prefix)) {
      prefix = prefix.substring(0, prefix.lastIndexOf("/", prefix.length - 2) + 1);
      if (!prefix) return "";
    }
  }
  // Ensure prefix ends at a directory boundary
  if (!prefix.endsWith("/")) {
    prefix = prefix.substring(0, prefix.lastIndexOf("/") + 1);
  }
  return prefix;
}

export function ProjectGroupView() {
  const { selectedProjectGroup, selectProject } = useAppStore();
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!selectedProjectGroup) return;
    setLoading(true);
    listProjects("time").then((all) => {
      const grouped = all.filter((p) => p.displayName === selectedProjectGroup && p.sessionCount > 0);
      setProjects(grouped);
      setLoading(false);
    });
  }, [selectedProjectGroup]);

  const commonPrefix = useMemo(
    () => longestCommonPrefix(projects.map((p) => p.originalPath)),
    [projects]
  );

  if (loading) {
    return <div className="p-6 text-zinc-500">Loading...</div>;
  }

  return (
    <div className="p-6 h-full overflow-y-auto">
      <h1 className="text-xl font-semibold mb-1">{selectedProjectGroup}</h1>
      {commonPrefix && (
        <p className="text-sm text-zinc-500 mb-1 font-mono">{commonPrefix}</p>
      )}
      <p className="text-xs text-zinc-400 mb-4">{projects.length} locations</p>
      <div className="space-y-2">
        {projects.map((p) => {
          const relativePath = commonPrefix
            ? p.originalPath.substring(commonPrefix.length)
            : p.originalPath;

          return (
            <button
              key={p.id}
              onClick={() => selectProject(p.id)}
              className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
            >
              <div className="font-medium font-mono text-sm">{relativePath || p.originalPath}</div>
              <div className="text-xs text-zinc-400 mt-1">
                {p.sessionCount} sessions &middot; {formatRelativeTime(p.lastActive)}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
