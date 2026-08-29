import { useEffect, useMemo, useState } from "react";
import { listProjects, listSessions, searchMessageContent } from "../../lib/tauri";
import type { Project, SessionSummary, ContentSearchResult } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { ProjectCard } from "../project/ProjectCard";
import { SessionCard } from "../session/SessionCard";
import { formatRelativeTime } from "../../lib/format";

/** Long enough that a burst of typing produces one query, short enough to feel
 *  immediate once the user stops. */
const CONTENT_SEARCH_DEBOUNCE_MS = 250;

/** Projects shown before expanding — one row at the widest grid breakpoint. */
const PROJECT_ROW = 3;

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

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Split a query into the terms both the index and the highlighter work with. */
function queryTerms(query: string): string[] {
  return query.trim().split(/\s+/).filter(Boolean);
}

/**
 * Wrap each occurrence of a query term in <mark>. The backend returns a plain
 * context window rather than FTS5's snippet(): with the trigram tokenizer its
 * token count is in 3-character units and caps at 64, which yields only ~30
 * characters of context - too little to recognize a hit.
 */
function highlightTerms(text: string, terms: string[]): string {
  if (terms.length === 0) return escapeHtml(text);
  const re = new RegExp(`(${terms.map(escapeRegex).join("|")})`, "gi");
  // split() with one capture group alternates: [text, match, text, match, ...]
  return text
    .split(re)
    .map((part, i) =>
      i % 2 === 1
        ? `<mark class="bg-yellow-200 dark:bg-yellow-800 px-0.5 rounded">${escapeHtml(part)}</mark>`
        : escapeHtml(part),
    )
    .join("");
}

interface SessionGroup {
  sessionDbId: number;
  sessionId: string;
  slug: string | null;
  latestTimestampMs: number;
  matches: ContentSearchResult[];
}

interface ProjectGroup {
  projectName: string;
  projectPath: string;
  sessions: SessionGroup[];
  matchCount: number;
  latestTimestampMs: number;
}

/**
 * Group results by project, then by session. Insertion order is preserved at
 * both levels, so the best-ranked project comes first and within it the
 * best-ranked session — the backend already returned the rows in rank order.
 */
function groupByProject(results: ContentSearchResult[]): ProjectGroup[] {
  const projects = new Map<string, ProjectGroup>();
  const sessions = new Map<number, SessionGroup>();

  for (const r of results) {
    let p = projects.get(r.projectPath);
    if (!p) {
      p = {
        projectName: r.projectName,
        projectPath: r.projectPath,
        sessions: [],
        matchCount: 0,
        latestTimestampMs: r.timestampMs,
      };
      projects.set(r.projectPath, p);
    }
    p.matchCount++;
    if (r.timestampMs > p.latestTimestampMs) p.latestTimestampMs = r.timestampMs;

    let sess = sessions.get(r.sessionDbId);
    if (!sess) {
      sess = {
        sessionDbId: r.sessionDbId,
        sessionId: r.sessionId,
        slug: r.slug,
        latestTimestampMs: r.timestampMs,
        matches: [],
      };
      sessions.set(r.sessionDbId, sess);
      p.sessions.push(sess);
    }
    sess.matches.push(r);
    if (r.timestampMs > sess.latestTimestampMs) sess.latestTimestampMs = r.timestampMs;
  }

  return Array.from(projects.values());
}

export function SearchResults() {
  const provider = useAppStore((s) => s.provider);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const selectSession = useAppStore((s) => s.selectSession);
  const contentSearchQuery = useAppStore((s) => s.contentSearchQuery);
  const contentSearchPathPrefix = useAppStore((s) => s.contentSearchPathPrefix);
  const contentResults = useAppStore((s) => s.contentSearchResults);
  const contentError = useAppStore((s) => s.contentSearchError);
  const setContentSearch = useAppStore((s) => s.setContentSearch);
  const pathPrefix = useAppStore((s) => s.searchPathPrefix);
  const setPathPrefix = useAppStore((s) => s.setSearchPathPrefix);
  const scopeHistory = useAppStore((s) => s.scopeHistory);

  const [projects, setProjects] = useState<Project[]>([]);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [contentLoading, setContentLoading] = useState(false);
  // Projects the user has folded away, by path. Absent means expanded.
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  // The scope box is edited locally and committed on Enter or blur: committing
  // per keystroke would re-query the project and session lists on every letter.
  const [prefixDraft, setPrefixDraft] = useState(pathPrefix);
  useEffect(() => setPrefixDraft(pathPrefix), [pathPrefix]);
  const [showAllProjects, setShowAllProjects] = useState(false);

  const trimmedPrefix = pathPrefix.trim();

  // "Searched" iff the cached results were taken under the current query *and*
  // the current scope — a scope change makes them as stale as a query change.
  const contentSearched =
    contentSearchQuery !== "" &&
    contentSearchQuery === searchQuery.trim() &&
    contentSearchPathPrefix === trimmedPrefix;

  const groupedContentResults = useMemo(
    () => (contentSearched ? groupByProject(contentResults) : []),
    [contentSearched, contentResults],
  );

  const toggleProject = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(path)) next.add(path);
      return next;
    });

  useEffect(() => {
    if (!searchQuery.trim()) {
      setProjects([]);
      setSessions([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    setShowAllProjects(false);
    const q = searchQuery.trim().toLowerCase();
    const terms = queryTerms(q);
    // Typing fast fires overlapping queries; only the newest may render.
    let stale = false;

    Promise.all([
      listProjects("time", provider),
      listSessions({ provider, sortBy: "time" }),
    ])
      .then(([allProjects, allSessions]) => {
        if (stale) return;
        // Subsequence matching stays for project paths, where it usefully
        // abbreviates ("wa" → "web-app").
        const inScope = (path: string) =>
          trimmedPrefix === "" || path.startsWith(trimmedPrefix);

        const matchedProjects = allProjects.filter(
          (p) =>
            inScope(p.originalPath) &&
            (fuzzyMatch(p.displayName, q) || fuzzyMatch(p.originalPath, q))
        );

        // Sessions match on everything the card actually shows — including the
        // summary, which is the title the user reads, and the AI tags. Each
        // term must appear somewhere, so multi-word queries work without the
        // words having to be adjacent.
        const matchedSessions = allSessions.filter((s) => {
          if (!inScope(s.projectPath)) return false;
          if (s.sessionId.toLowerCase().startsWith(q)) return true;
          const haystack = [s.summary, s.slug, s.projectName, ...s.aiTags]
            .filter(Boolean)
            .join(" ")
            .toLowerCase();
          return terms.every((t) => haystack.includes(t));
        });

        setProjects(matchedProjects);
        setSessions(matchedSessions);
        setLoading(false);
      })
      .catch((e) => {
        if (stale) return;
        console.error("Search failed:", e);
        setProjects([]);
        setSessions([]);
        setLoading(false);
      });

    return () => { stale = true; };
  }, [provider, searchQuery, trimmedPrefix]);

  // Content search runs on its own. It is debounced because it is the expensive
  // half: a keystroke-per-query would hit FTS over the whole index each letter.
  // `contentSearched` in the deps makes this a no-op once the store already
  // holds results for this query and scope, including when returning from a
  // conversation.
  useEffect(() => {
    const q = searchQuery.trim();
    if (!q) {
      setContentLoading(false);
      return;
    }
    if (contentSearched) {
      setContentLoading(false);
      return;
    }

    let stale = false;
    setContentLoading(true);
    const timer = setTimeout(() => {
      searchMessageContent(q, provider, trimmedPrefix, 50)
        .then((results) => {
          if (!stale) setContentSearch(q, trimmedPrefix, results, null);
        })
        .catch((e) => {
          if (!stale) setContentSearch(q, trimmedPrefix, [], String(e));
        })
        .finally(() => {
          if (!stale) setContentLoading(false);
        });
    }, CONTENT_SEARCH_DEBOUNCE_MS);

    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [provider, searchQuery, trimmedPrefix, contentSearched, setContentSearch]);

  if (loading && projects.length === 0 && sessions.length === 0) {
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
        <span className="text-xs text-zinc-400">
          {contentLoading
            ? "Searching content..."
            : contentSearched
              ? `${contentResults.length} content match${contentResults.length === 1 ? "" : "es"}`
              : ""}
        </span>
      </div>

      <div className="flex items-center gap-2 mb-4 text-xs">
        <span className="text-zinc-500 whitespace-nowrap">Scope</span>
        <input
          value={prefixDraft}
          onChange={(e) => setPrefixDraft(e.target.value)}
          onBlur={() => setPathPrefix(prefixDraft)}
          onKeyDown={(e) => {
            if (e.key === "Enter") setPathPrefix(prefixDraft);
            if (e.key === "Escape") setPrefixDraft(pathPrefix);
          }}
          placeholder="Limit to a path prefix, e.g. /Users/me/work"
          spellCheck={false}
          list="search-scope-history"
          className="flex-1 min-w-0 px-2 py-1 font-mono rounded border border-zinc-300 dark:border-zinc-700 bg-transparent placeholder:text-zinc-400 focus:outline-none focus:border-zinc-500"
        />
        <datalist id="search-scope-history">
          {scopeHistory.map((h) => (
            <option key={h} value={h} />
          ))}
        </datalist>
        {trimmedPrefix !== "" && (
          <button
            onClick={() => setPathPrefix("")}
            className="px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 whitespace-nowrap"
          >
            Clear
          </button>
        )}
      </div>

      {projects.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide mb-2 flex items-center gap-2">
            <span>Projects ({projects.length})</span>
            {/* Project cards are tall; a broad query can push the message hits,
                which are the point of the page, off the bottom of the screen. */}
            {projects.length > PROJECT_ROW && (
              <button
                onClick={() => setShowAllProjects((v) => !v)}
                aria-expanded={showAllProjects}
                className="normal-case tracking-normal font-normal text-xs px-1.5 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                {showAllProjects ? "Show less" : `Show all ${projects.length}`}
              </button>
            )}
          </h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {(showAllProjects ? projects : projects.slice(0, PROJECT_ROW)).map((p) => (
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
        <div className="text-zinc-500 text-sm mb-6">
          No content matches
          {trimmedPrefix !== "" && <> under <span className="font-mono">{trimmedPrefix}</span></>}.
        </div>
      )}

      {contentResults.length > 0 && (
        <div className="mb-6">
          <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide mb-2">
            Message content ({contentResults.length} match{contentResults.length === 1 ? "" : "es"} in {groupedContentResults.length} project{groupedContentResults.length === 1 ? "" : "s"})
          </h2>
          <div className="space-y-3">
            {groupedContentResults.map((project) => {
              const isCollapsed = collapsed.has(project.projectPath);
              return (
                <div
                  key={project.projectPath}
                  className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden"
                >
                  <div className="flex items-center gap-2 px-3 py-2 bg-zinc-100 dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800">
                    <button
                      onClick={() => toggleProject(project.projectPath)}
                      className="flex items-center gap-2 min-w-0 flex-1 text-left hover:opacity-80"
                      aria-expanded={!isCollapsed}
                    >
                      <span className="text-zinc-400 w-3 shrink-0">{isCollapsed ? "\u25b8" : "\u25be"}</span>
                      <span className="text-sm font-medium text-zinc-800 dark:text-zinc-200 truncate">
                        {project.projectName}
                      </span>
                      <span className="text-xs text-zinc-400 font-mono truncate">
                        {project.projectPath}
                      </span>
                    </button>
                    <span className="text-xs text-zinc-500 whitespace-nowrap">
                      {project.matchCount} match{project.matchCount === 1 ? "" : "es"} · {project.sessions.length} session{project.sessions.length === 1 ? "" : "s"}
                    </span>
                    {/* Re-runs the search scoped to this project rather than
                        filtering what is already on screen: the result set is
                        capped by rank, so this can surface hits the unscoped
                        query never had room to return. */}
                    <button
                      onClick={() => setPathPrefix(project.projectPath)}
                      title="Search only this project"
                      className="text-xs px-1.5 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-200 dark:hover:bg-zinc-800 whitespace-nowrap"
                    >
                      Only this
                    </button>
                  </div>

                  {!isCollapsed && project.sessions.map((group) => (
                    <div key={group.sessionDbId} className="border-b border-zinc-200 dark:border-zinc-800 last:border-b-0">
                      <button
                        onClick={() => selectSession(group.sessionDbId)}
                        className="w-full text-left px-3 py-2 bg-zinc-50 dark:bg-zinc-900/50 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition-colors"
                      >
                        <div className="flex items-center gap-2 text-xs">
                          <span className="text-zinc-600 dark:text-zinc-300 truncate">
                            {group.slug || group.sessionId.slice(0, 8)}
                          </span>
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
                              dangerouslySetInnerHTML={{
                                __html: highlightTerms(r.snippet, queryTerms(contentSearchQuery)),
                              }}
                            />
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {!hasMetaResults && contentResults.length === 0 && !contentLoading && (
        <div className="text-zinc-500 text-sm">
          {contentLoading ? "Searching..." : "No results found."}
        </div>
      )}
    </div>
  );
}
