import { create } from "zustand";
import type { ContentSearchResult } from "../lib/types";

type View = "projects" | "sessions" | "conversation" | "favorites" | "backups" | "settings" | "search" | "projectGroup" | "live" | "liveConversation" | "usage" | "codexProjects" | "codexSessions" | "codexConversation";
type Provider = "claude" | "codex";

interface AppState {
  provider: Provider;
  view: View;
  selectedProjectId: number | null;
  selectedSessionId: number | null;
  selectedProjectGroup: string | null; // displayName for grouped projects
  searchQuery: string;
  sidebarCollapsed: boolean;
  refreshCounter: number;
  // View to return to when leaving a conversation (e.g. "search" if entered from search)
  conversationFromView: View | null;
  // Content search state (persisted across navigation)
  contentSearchQuery: string;        // the query these results correspond to
  contentSearchResults: ContentSearchResult[];
  contentSearchError: string | null;
  // Codex
  selectedCodexCwd: string | null;
  selectedCodexThreadId: string | null;
  setProvider: (provider: Provider) => void;
  setView: (view: View) => void;
  selectProject: (id: number | null) => void;
  selectSession: (id: number | null) => void;
  selectProjectGroup: (displayName: string) => void;
  selectCodexProject: (cwd: string) => void;
  selectCodexSession: (threadId: string | null) => void;
  setSearchQuery: (query: string) => void;
  setContentSearch: (query: string, results: ContentSearchResult[], error: string | null) => void;
  clearContentSearch: () => void;
  toggleSidebar: () => void;
  triggerRefresh: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  provider: "claude",
  view: "projects",
  selectedProjectId: null,
  selectedSessionId: null,
  selectedProjectGroup: null,
  searchQuery: "",
  sidebarCollapsed: false,
  refreshCounter: 0,
  conversationFromView: null,
  contentSearchQuery: "",
  contentSearchResults: [],
  contentSearchError: null,
  selectedCodexCwd: null,
  selectedCodexThreadId: null,
  setProvider: (provider) => set({ provider, view: provider === "codex" ? "codexProjects" : "projects", searchQuery: "" }),
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
  selectCodexProject: (cwd) => set({ selectedCodexCwd: cwd, selectedCodexThreadId: null, view: "codexSessions" }),
  selectCodexSession: (threadId) => set((s) => {
    if (threadId === null) {
      return {
        selectedCodexThreadId: null,
        view: s.conversationFromView || "codexSessions",
        conversationFromView: null,
      };
    }
    return {
      selectedCodexThreadId: threadId,
      view: "codexConversation" as View,
      conversationFromView: s.view === "codexConversation" ? s.conversationFromView : s.view,
    };
  }),
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
  clearContentSearch: () => set({ contentSearchQuery: "", contentSearchResults: [], contentSearchError: null }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  triggerRefresh: () => set((s) => ({ refreshCounter: s.refreshCounter + 1 })),
}));
