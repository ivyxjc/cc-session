import { create } from "zustand";
import type { LiveSession, ViewMessage } from "../lib/types";

interface LiveState {
  liveSessions: LiveSession[];
  watchedSessionId: string | null;
  newMessages: ViewMessage[];
  setLiveSessions: (sessions: LiveSession[]) => void;
  setWatchedSessionId: (id: string | null) => void;
  appendMessages: (messages: ViewMessage[]) => void;
  clearNewMessages: () => void;
}

export const useLiveStore = create<LiveState>((set) => ({
  liveSessions: [],
  watchedSessionId: null,
  newMessages: [],
  setLiveSessions: (sessions) => set({ liveSessions: sessions }),
  setWatchedSessionId: (id) => set({ watchedSessionId: id, newMessages: [] }),
  appendMessages: (messages) =>
    set((s) => ({ newMessages: [...s.newMessages, ...messages] })),
  clearNewMessages: () => set({ newMessages: [] }),
}));
