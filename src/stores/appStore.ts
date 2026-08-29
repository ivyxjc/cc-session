import { create } from "zustand";
import type { ContentSearchResult, Provider } from "../lib/types";
import { useFilterStore } from "./filterStore";

const SCOPE_HISTORY_KEY = "cc-session.searchScopeHistory";
const SCOPE_HISTORY_MAX = 10;

/** Reading it can throw outright (private mode, blocked site data), so every
 *  access is guarded and an unreadable store simply means "no history". */
function loadScopeHistory(): string[] {
  try {
    const raw = localStorage.getItem(SCOPE_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((v): v is string => typeof v === "string").slice(0, SCOPE_HISTORY_MAX);
  } catch {
    return [];
  }
}

/** Most recent first, de-duplicated, capped. */
function pushScopeHistory(history: string[], prefix: string): string[] {
  const next = [prefix, ...history.filter((h) => h !== prefix)].slice(0, SCOPE_HISTORY_MAX);
  try {
    localStorage.setItem(SCOPE_HISTORY_KEY, JSON.stringify(next));
  } catch {
    // A full or unavailable store costs the history, not the search.
  }
  return next;
}

type View = "projects" | "sessions" | "conversation" | "favorites" | "backups" | "settings" | "search" | "projectGroup" | "live" | "liveConversation" | "usage" | "dayPlanner";

interface AppState {
  /** Active provider filter — every list view scopes to it. */
  provider: Provider;
  view: View;
  selectedProjectId: number | null;
  selectedSessionId: number | null;
  selectedProjectGroup: string | null; // displayName for grouped projects
  searchQuery: string;
  refreshCounter: number;
  // View to return to when leaving a conversation (e.g. "search" if entered from search)
  conversationFromView: View | null;
  // Content search state (persisted across navigation)
  contentSearchQuery: string;        // the query these results correspond to
  contentSearchPathPrefix: string;   // the scope those results were taken under
  contentSearchResults: ContentSearchResult[];
  contentSearchError: string | null;
  // Scope for the next content search. Applied in SQL, not to the results:
  // the query is limited by rank, so narrowing afterwards would only shrink an
  // already-truncated set instead of surfacing the scope's own best hits.
  searchPathPrefix: string;
  /** Recently used scopes, most recent first. Client-side only. */
  scopeHistory: string[];
  setProvider: (provider: Provider) => void;
  setView: (view: View) => void;
  selectProject: (id: number | null) => void;
  selectSession: (id: number | null) => void;
  selectProjectGroup: (displayName: string) => void;
  setSearchQuery: (query: string) => void;
  setContentSearch: (query: string, pathPrefix: string, results: ContentSearchResult[], error: string | null) => void;
  setSearchPathPrefix: (prefix: string) => void;
  triggerRefresh: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  provider: "claude",
  view: "projects",
  selectedProjectId: null,
  selectedSessionId: null,
  selectedProjectGroup: null,
  searchQuery: "",
  refreshCounter: 0,
  conversationFromView: null,
  contentSearchQuery: "",
  contentSearchPathPrefix: "",
  contentSearchResults: [],
  contentSearchError: null,
  searchPathPrefix: "",
  scopeHistory: loadScopeHistory(),
  setProvider: (provider) => {
    // Tags live in the shared index but the selection is a filter over the
    // list; carrying it across a provider switch silently hides sessions with
    // no visible indication of why.
    useFilterStore.getState().setSelectedTagId(null);
    set({
      provider,
      view: "projects",
      selectedProjectId: null,
      selectedSessionId: null,
      selectedProjectGroup: null,
      searchQuery: "",
      searchPathPrefix: "",
    });
  },
  setView: (view) => set({ view }),
  selectProject: (id) => set((s) => ({ selectedProjectId: id, selectedSessionId: null, ...(id !== null ? { view: "sessions" as View } : s.view === "sessions" || s.view === "conversation" ? { view: "sessions" as View } : {}) })),
  selectSession: (id) => set((s) => {
    if (id === null) {
      // Leaving conversation — restore source view if we recorded one
      return {
        selectedSessionId: null,
        view: s.conversationFromView || "sessions",
        conversationFromView: null,
      };
    }
    // Entering conversation — remember where we came from (don't overwrite if already in conversation)
    return {
      selectedSessionId: id,
      view: "conversation" as View,
      conversationFromView: s.view === "conversation" ? s.conversationFromView : s.view,
    };
  }),
  selectProjectGroup: (displayName) => set({ selectedProjectGroup: displayName, selectedProjectId: null, selectedSessionId: null, view: "projectGroup" }),
  setSearchQuery: (query) => set((s) => {
    const trimmed = query.trim();
    // Invalidate cached content search if the query no longer matches
    const contentReset = trimmed === s.contentSearchQuery
      ? {}
      : { contentSearchQuery: "", contentSearchPathPrefix: "", contentSearchResults: [], contentSearchError: null };
    return {
      searchQuery: query,
      ...contentReset,
      ...(query ? { view: "search" as View } : s.view === "search" ? { view: "projects" as View } : {}),
    };
  }),
  setContentSearch: (query, pathPrefix, results, error) => set({
    contentSearchQuery: query.trim(),
    contentSearchPathPrefix: pathPrefix.trim(),
    contentSearchResults: results,
    contentSearchError: error,
  }),
  // Changing the scope invalidates the cached results the same way changing the
  // query does — they were taken under the old scope.
  setSearchPathPrefix: (prefix) => set((s) => {
    const trimmed = prefix.trim();
    // Clearing the scope is not a scope worth remembering.
    const history = trimmed === "" ? s.scopeHistory : pushScopeHistory(s.scopeHistory, trimmed);
    return trimmed === s.contentSearchPathPrefix
      ? { searchPathPrefix: prefix, scopeHistory: history }
      : {
          searchPathPrefix: prefix,
          scopeHistory: history,
          contentSearchQuery: "",
          contentSearchPathPrefix: "",
          contentSearchResults: [],
          contentSearchError: null,
        };
  }),
  triggerRefresh: () => set((s) => ({ refreshCounter: s.refreshCounter + 1 })),
}));
