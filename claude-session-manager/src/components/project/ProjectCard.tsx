import type { Project } from "../../lib/types";
import { formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";

export function ProjectCard({ project }: { project: Project }) {
  const selectProject = useAppStore((s) => s.selectProject);

  return (
    <button
      onClick={() => selectProject(project.id)}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="font-medium">{project.displayName}</div>
      <div className="text-sm text-zinc-500 truncate mt-0.5">{project.originalPath}</div>
      <div className="text-xs text-zinc-400 mt-2">
        {project.sessionCount} sessions &middot; {formatRelativeTime(project.lastActive)}
      </div>
    </button>
  );
}
