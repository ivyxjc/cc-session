import { create } from "zustand";

interface FilterState {
  sortBy: string;
  selectedTagId: number | null;
  setSortBy: (sort: string) => void;
  setSelectedTagId: (id: number | null) => void;
}

export const useFilterStore = create<FilterState>((set) => ({
  sortBy: "time",
  selectedTagId: null,
  setSortBy: (sortBy) => set({ sortBy }),
  setSelectedTagId: (selectedTagId) => set({ selectedTagId }),
}));
