import { create } from "zustand";
import type { LiveSession, ParsedMessage } from "../lib/types";

interface LiveState {
  liveSessions: LiveSession[];
  watchedSessionId: string | null;
  newMessages: ParsedMessage[];
  setLiveSessions: (sessions: LiveSession[]) => void;
  setWatchedSessionId: (id: string | null) => void;
  appendMessages: (messages: ParsedMessage[]) => void;
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
