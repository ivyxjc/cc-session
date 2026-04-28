import { useEffect, useMemo, useState } from "react";
import { listProjects, listSessions, searchMessageContent } from "../../lib/tauri";
import type { Project, SessionSummary, ContentSearchResult } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { ProjectCard } from "../project/ProjectCard";
import { SessionCard } from "../session/SessionCard";
import { formatRelativeTime } from "../../lib/format";

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

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Convert a snippet with \x01 / \x02 boundary markers (set by SQL `char(1)/char(2)`) into safe HTML */
function snippetToHtml(snippet: string): string {
  return escapeHtml(snippet)
    .replace(//g, '<mark class="bg-yellow-200 dark:bg-yellow-800 px-0.5 rounded">')
    .replace(//g, "</mark>");
}

interface ContentGroup {
  sessionDbId: number;
  sessionId: string;
  slug: string | null;
  projectName: string;
  projectPath: string;
  latestTimestampMs: number;
  matches: ContentSearchResult[];
}

/** Group results by session, preserving the order of first appearance (best-ranked session first) */
function groupBySession(results: ContentSearchResult[]): ContentGroup[] {
  const map = new Map<number, ContentGroup>();
  for (const r of results) {
    const g = map.get(r.sessionDbId);
    if (g) {
      g.matches.push(r);
      if (r.timestampMs > g.latestTimestampMs) g.latestTimestampMs = r.timestampMs;
    } else {
      map.set(r.sessionDbId, {
        sessionDbId: r.sessionDbId,
        sessionId: r.sessionId,
        slug: r.slug,
        projectName: r.projectName,
        projectPath: r.projectPath,
        latestTimestampMs: r.timestampMs,
        matches: [r],
      });
    }
  }
  return Array.from(map.values());
}

export function SearchResults() {
  const searchQuery = useAppStore((s) => s.searchQuery);
  const selectSession = useAppStore((s) => s.selectSession);
  const contentSearchQuery = useAppStore((s) => s.contentSearchQuery);
  const contentResults = useAppStore((s) => s.contentSearchResults);
  const contentError = useAppStore((s) => s.contentSearchError);
  const setContentSearch = useAppStore((s) => s.setContentSearch);

  const [projects, setProjects] = useState<Project[]>([]);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [contentLoading, setContentLoading] = useState(false);

  // "Searched" iff the cached content-search query matches the current query and is non-empty
  const contentSearched = contentSearchQuery !== "" && contentSearchQuery === searchQuery.trim();

  const groupedContentResults = useMemo(
    () => (contentSearched ? groupBySession(contentResults) : []),
    [contentSearched, contentResults],
  );

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
      const matchedProjects = allProjects.filter(
        (p) => fuzzyMatch(p.displayName, q) || fuzzyMatch(p.originalPath, q)
      );

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

  const runContentSearch = async () => {
    const q = searchQuery.trim();
    if (!q) return;
    setContentLoading(true);
    try {
      const results = await searchMessageContent(q, 50);
      setContentSearch(q, results, null);
    } catch (e) {
      setContentSearch(q, [], String(e));
    } finally {
      setContentLoading(false);
    }
  };

  if (loading) {
    return <div className="p-6 text-zinc-500">Searching...</div>;
  }

  if (!searchQuery.trim()) {
    return <div className="p-6 text-zinc-500">Type to search...</div>;
  }

  const hasMetaResults = projects.length > 0 || sessions.length > 0;

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center gap-3 mb-4 flex-wrap">
        <h1 className="text-xl font-semibold">
          Search: "{searchQuery}"
        </h1>
        <button
          onClick={runContentSearch}
          disabled={contentLoading}
          className="text-xs px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
        >
          {contentLoading ? "Searching..." : contentSearched ? "Search content again" : "Search content"}
        </button>
        {contentSearched && !contentLoading && (
          <span className="text-xs text-zinc-400">
            {contentResults.length} content match{contentResults.length === 1 ? "" : "es"}
          </span>
        )}
      </div>

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
        <div className="mb-6">
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

      {contentError && (
        <div className="text-sm text-red-500 mb-3">Search failed: {contentError}</div>
      )}

      {contentSearched && !contentLoading && contentResults.length === 0 && !contentError && (
        <div className="text-zinc-500 text-sm mb-6">No content matches.</div>
      )}

      {contentResults.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide mb-2">
            Message content ({contentResults.length} match{contentResults.length === 1 ? "" : "es"} in {groupedContentResults.length} session{groupedContentResults.length === 1 ? "" : "s"})
          </h2>
          <div className="space-y-3">
            {groupedContentResults.map((group) => (
              <div
                key={group.sessionDbId}
                className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden"
              >
                <button
                  onClick={() => selectSession(group.sessionDbId)}
                  className="w-full text-left px-3 py-2 bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors border-b border-zinc-200 dark:border-zinc-800"
                >
                  <div className="flex items-center gap-2 text-xs">
                    <span className="font-medium text-zinc-700 dark:text-zinc-300 truncate">
                      {group.projectName}
                    </span>
                    <span className="text-zinc-400 font-mono">
                      {group.slug || group.sessionId.slice(0, 8)}
                    </span>
                    <span className="text-zinc-400 truncate">{group.projectPath}</span>
                    <span className="ml-auto text-zinc-500 whitespace-nowrap">
                      {group.matches.length} match{group.matches.length === 1 ? "" : "es"} · {formatRelativeTime(group.latestTimestampMs)}
                    </span>
                  </div>
                </button>
                <div className="divide-y divide-zinc-100 dark:divide-zinc-800">
                  {group.matches.map((r) => (
                    <button
                      key={r.messageUuid + r.timestampMs}
                      onClick={() => selectSession(r.sessionDbId)}
                      className="w-full text-left block px-3 py-2 hover:bg-zinc-50 dark:hover:bg-zinc-900 transition-colors"
                    >
                      <div className="flex items-center gap-2 text-xs text-zinc-500 mb-1">
                        <span className={`px-1.5 py-0.5 rounded font-mono ${
                          r.role === "assistant" ? "bg-blue-50 text-blue-600 dark:bg-blue-950 dark:text-blue-400" :
                          r.role === "thinking" ? "bg-purple-50 text-purple-600 dark:bg-purple-950 dark:text-purple-400" :
                          "bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400"
                        }`}>
                          {r.role}
                        </span>
                        <span className="ml-auto whitespace-nowrap">{formatRelativeTime(r.timestampMs)}</span>
                      </div>
                      <div
                        className="text-sm text-zinc-700 dark:text-zinc-300 leading-snug whitespace-pre-wrap break-words"
                        dangerouslySetInnerHTML={{ __html: snippetToHtml(r.snippet) }}
                      />
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {!hasMetaResults && contentResults.length === 0 && !contentLoading && (
        <div className="text-zinc-500 text-sm">
          {contentSearched ? "No results found." : "No project/session matches. Try \"Search content\" to look inside messages."}
        </div>
      )}
    </div>
  );
}
