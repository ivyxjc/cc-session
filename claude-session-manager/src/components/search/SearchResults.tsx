import { useEffect, useState } from "react";
import { listProjects, listSessions } from "../../lib/tauri";
import type { Project, SessionSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { ProjectCard } from "../project/ProjectCard";
import { SessionCard } from "../session/SessionCard";

function fuzzyMatch(text: string, query: string): boolean {
  const lower = text.toLowerCase();
  const q = query.toLowerCase();
  // Check if all characters of query appear in order in text
  let qi = 0;
  for (let i = 0; i < lower.length && qi < q.length; i++) {
    if (lower[i] === q[qi]) qi++;
  }
  return qi === q.length;
}

export function SearchResults() {
  const { searchQuery } = useAppStore();
  const [projects, setProjects] = useState<Project[]>([]);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setProjects([]);
      setSessions([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    const q = searchQuery.trim().toLowerCase();

    Promise.all([
      listProjects("time"),
      listSessions({ sortBy: "time" }),
    ]).then(([allProjects, allSessions]) => {
      // Filter projects by path fuzzy match
      const matchedProjects = allProjects.filter(
        (p) => fuzzyMatch(p.displayName, q) || fuzzyMatch(p.originalPath, q)
      );

      // Filter sessions by session ID prefix or slug match
      const matchedSessions = allSessions.filter(
        (s) =>
          s.sessionId.toLowerCase().startsWith(q) ||
          (s.slug && fuzzyMatch(s.slug, q)) ||
          fuzzyMatch(s.projectName, q)
      );

      setProjects(matchedProjects);
      setSessions(matchedSessions);
      setLoading(false);
    });
  }, [searchQuery]);

  if (loading) {
    return <div className="p-6 text-zinc-500">Searching...</div>;
  }

  if (!searchQuery.trim()) {
    return <div className="p-6 text-zinc-500">Type to search...</div>;
  }

  const hasResults = projects.length > 0 || sessions.length > 0;

  return (
    <div className="p-6 h-full overflow-y-auto">
      <h1 className="text-xl font-semibold mb-4">
        Search: "{searchQuery}"
      </h1>

      {!hasResults && (
        <div className="text-zinc-500">No results found.</div>
      )}

      {projects.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide mb-2">
            Projects ({projects.length})
          </h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {projects.map((p) => (
              <ProjectCard key={p.id} project={p} />
            ))}
          </div>
        </div>
      )}

      {sessions.length > 0 && (
        <div>
          <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide mb-2">
            Sessions ({sessions.length})
          </h2>
          <div className="space-y-2">
            {sessions.map((s) => (
              <SessionCard key={s.id} session={s} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
