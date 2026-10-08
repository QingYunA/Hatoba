import { create } from "zustand";

export type HostSort = "recent" | "name" | "address";

/** Host list view state that survives navigating to the editor and back. */
interface HostsUi {
  selectedId: string | null;
  sort: HostSort;
  select(id: string | null): void;
  setSort(sort: HostSort): void;
}

export const useHostsUi = create<HostsUi>((set) => ({
  selectedId: null,
  sort: "recent",
  select: (selectedId) => set({ selectedId }),
  setSort: (sort) => set({ sort }),
}));
