import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../../stores/appStore";
import { useFilterStore } from "../../stores/filterStore";
import { listProjects, listTags } from "../../lib/tauri";
import type { Project, Tag } from "../../lib/types";

function longestCommonPrefix(paths: string[]): string {
  if (paths.length <= 1) return "";
  let prefix = paths[0];
  for (let i = 1; i < paths.length; i++) {
    while (!paths[i].startsWith(prefix)) {
      prefix = prefix.substring(0, prefix.lastIndexOf("/", prefix.length - 2) + 1);
      if (!prefix) return "";
    }
  }
  if (!prefix.endsWith("/")) prefix = prefix.substring(0, prefix.lastIndexOf("/") + 1);
  return prefix;
}

interface ProjectGroup {
  displayName: string;
  projects: Project[];
  totalSessions: number;
  commonPrefix: string;
}

export function Sidebar() {
  const { view, setView, selectedProjectId, selectProject, selectedProjectGroup, selectProjectGroup, searchQuery, setSearchQuery } = useAppStore();
  const { selectedTagId, setSelectedTagId } = useFilterStore();
  const [projects, setProjects] = useState<Project[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());

  useEffect(() => {
    listProjects("time").then(setProjects).catch(console.error);
    listTags().then(setTags).catch(console.error);
  }, []);

  // Group projects by displayName
  const projectGroups = useMemo(() => {
    const map = new Map<string, Project[]>();
    for (const p of projects) {
      if (p.sessionCount === 0) continue;
      const existing = map.get(p.displayName) || [];
      existing.push(p);
      map.set(p.displayName, existing);
    }
    const groups: ProjectGroup[] = [];
    for (const [displayName, projs] of map) {
      groups.push({
        displayName,
        projects: projs,
        totalSessions: projs.reduce((sum, p) => sum + p.sessionCount, 0),
        commonPrefix: longestCommonPrefix(projs.map((p) => p.originalPath)),
      });
    }
    // Sort by most recent activity
    groups.sort((a, b) => {
      const aMax = Math.max(...a.projects.map((p) => p.lastActive || 0));
      const bMax = Math.max(...b.projects.map((p) => p.lastActive || 0));
      return bMax - aMax;
    });
    return groups;
  }, [projects]);

  const toggleGroup = (name: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const handleProjectGroupClick = (group: ProjectGroup) => {
    if (group.projects.length === 1) {
      // Single project — go directly to sessions
      selectProject(group.projects[0].id);
    } else {
      // Multiple projects — show group view in main area, toggle expand in sidebar
      selectProjectGroup(group.displayName);
      toggleGroup(group.displayName);
    }
  };

  return (
    <aside className="h-full bg-zinc-50 dark:bg-zinc-900 flex flex-col overflow-hidden">
      {/* Search */}
      <div className="p-3 pb-0">
        <input
          type="text"
          placeholder="Search sessions..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full px-3 py-1.5 rounded border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm placeholder-zinc-400 focus:outline-none focus:border-zinc-500"
        />
      </div>

      {/* Navigation */}
      <nav className="p-3 space-y-1">
        <button
          onClick={() => { setView("projects"); selectProject(null); setSearchQuery(""); }}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "projects" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          All Projects
        </button>
        <button
          onClick={() => { setView("favorites"); selectProject(null); setSearchQuery(""); }}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "favorites" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Favorites
        </button>
      </nav>

      {/* Tags */}
      {tags.length > 0 && (
        <div className="px-3 py-2">
          <h3 className="text-xs font-medium text-zinc-500 uppercase tracking-wide mb-1">Tags</h3>
          <div className="space-y-0.5">
            {tags.map((tag) => (
              <button
                key={tag.id}
                onClick={() => {
                  setSelectedTagId(selectedTagId === tag.id ? null : tag.id);
                  setView("sessions");
                  setSearchQuery("");
                }}
                className={`w-full text-left px-3 py-1 rounded text-sm flex items-center gap-2 ${
                  selectedTagId === tag.id ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
                }`}
              >
                <span className="w-2 h-2 rounded-full" style={{ backgroundColor: tag.color }} />
                {tag.name}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Projects (grouped by displayName) */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <h3 className="text-xs font-medium text-zinc-500 uppercase tracking-wide mb-1">Projects</h3>
        <div className="space-y-0.5">
          {projectGroups.map((group) => {
            const isMulti = group.projects.length > 1;
            const isExpanded = expandedGroups.has(group.displayName);
            const isGroupSelected = selectedProjectGroup === group.displayName;

            return (
              <div key={group.displayName}>
                <button
                  onClick={() => handleProjectGroupClick(group)}
                  className={`w-full text-left px-3 py-1 rounded text-sm flex items-center gap-1 ${
                    isGroupSelected || (!isMulti && selectedProjectId === group.projects[0].id)
                      ? "bg-zinc-200 dark:bg-zinc-800"
                      : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
                  }`}
                  title={isMulti ? `${group.projects.length} locations` : group.projects[0].originalPath}
                >
                  {isMulti && (
                    <span className="text-xs text-zinc-400 w-3 shrink-0">{isExpanded ? "▼" : "▶"}</span>
                  )}
                  <span className="truncate flex-1">{group.displayName}</span>
                  <span className="text-zinc-400 text-xs shrink-0">
                    {isMulti && `${group.projects.length}× `}{group.totalSessions}
                  </span>
                </button>

                {/* Expanded sub-projects */}
                {isMulti && isExpanded && (
                  <div className="ml-4 space-y-0.5 mt-0.5">
                    {group.commonPrefix && (
                      <div className="px-3 py-0.5 text-xs text-zinc-400 truncate" title={group.commonPrefix}>
                        {group.commonPrefix}
                      </div>
                    )}
                    {group.projects.map((p) => {
                      const display = group.commonPrefix
                        ? p.originalPath.substring(group.commonPrefix.length)
                        : p.originalPath;
                      return (
                        <button
                          key={p.id}
                          onClick={() => selectProject(p.id)}
                          className={`w-full text-left px-3 py-1 rounded text-xs truncate ${
                            selectedProjectId === p.id ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
                          }`}
                          title={p.originalPath}
                        >
                          <span className="text-zinc-500">{display}</span>
                          <span className="text-zinc-400 ml-1">{p.sessionCount}</span>
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Bottom */}
      <div className="p-3 border-t border-zinc-200 dark:border-zinc-800 space-y-1">
        <button
          onClick={() => { setView("backups"); setSearchQuery(""); }}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "backups" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Backups
        </button>
        <button
          onClick={() => { setView("settings"); setSearchQuery(""); }}
          className={`w-full text-left px-3 py-1.5 rounded text-sm ${view === "settings" ? "bg-zinc-200 dark:bg-zinc-800" : "hover:bg-zinc-100 dark:hover:bg-zinc-800"}`}
        >
          Settings
        </button>
      </div>
    </aside>
  );
}
