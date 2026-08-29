import { create } from "zustand";
import type { ContentSearchResult, Provider } from "../lib/types";
import { useFilterStore } from "./filterStore";

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
  contentSearchResults: ContentSearchResult[];
  contentSearchError: string | null;
  setProvider: (provider: Provider) => void;
  setView: (view: View) => void;
  selectProject: (id: number | null) => void;
  selectSession: (id: number | null) => void;
  selectProjectGroup: (displayName: string) => void;
  setSearchQuery: (query: string) => void;
  setContentSearch: (query: string, results: ContentSearchResult[], error: string | null) => void;
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
  contentSearchResults: [],
  contentSearchError: null,
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
      : { contentSearchQuery: "", contentSearchResults: [], contentSearchError: null };
    return {
      searchQuery: query,
      ...contentReset,
      ...(query ? { view: "search" as View } : s.view === "search" ? { view: "projects" as View } : {}),
    };
  }),
  setContentSearch: (query, results, error) => set({
    contentSearchQuery: query.trim(),
    contentSearchResults: results,
    contentSearchError: error,
  }),
  triggerRefresh: () => set((s) => ({ refreshCounter: s.refreshCounter + 1 })),
}));
