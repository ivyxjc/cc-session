import { create } from "zustand";

type View = "projects" | "sessions" | "conversation" | "favorites" | "backups" | "settings" | "search" | "projectGroup" | "live" | "liveConversation";

interface AppState {
  view: View;
  selectedProjectId: number | null;
  selectedSessionId: number | null;
  selectedProjectGroup: string | null; // displayName for grouped projects
  searchQuery: string;
  sidebarCollapsed: boolean;
  refreshCounter: number;
  setView: (view: View) => void;
  selectProject: (id: number | null) => void;
  selectSession: (id: number | null) => void;
  selectProjectGroup: (displayName: string) => void;
  setSearchQuery: (query: string) => void;
  toggleSidebar: () => void;
  triggerRefresh: () => void;
}

export const useAppStore = create<AppState>((set) => ({
  view: "projects",
  selectedProjectId: null,
  selectedSessionId: null,
  selectedProjectGroup: null,
  searchQuery: "",
  sidebarCollapsed: false,
  refreshCounter: 0,
  setView: (view) => set({ view }),
  selectProject: (id) => set((s) => ({ selectedProjectId: id, selectedSessionId: null, ...(id !== null ? { view: "sessions" as View } : s.view === "sessions" || s.view === "conversation" ? { view: "sessions" as View } : {}) })),
  selectSession: (id) => set({ selectedSessionId: id, view: id ? "conversation" : "sessions" }),
  selectProjectGroup: (displayName) => set({ selectedProjectGroup: displayName, selectedProjectId: null, selectedSessionId: null, view: "projectGroup" }),
  setSearchQuery: (query) => set((s) => ({ searchQuery: query, ...(query ? { view: "search" as View } : s.view === "search" ? { view: "projects" as View } : {}) })),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  triggerRefresh: () => set((s) => ({ refreshCounter: s.refreshCounter + 1 })),
}));
