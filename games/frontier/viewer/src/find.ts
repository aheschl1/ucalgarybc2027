// What the search box asks for, shared by the panel and the board.

import { useSyncExternalStore } from "react";

let query = "";
const listeners = new Set<() => void>();

export function setFind(q: string) {
  query = q;
  for (const l of listeners) l();
}

export function useFind(): string {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => query,
  );
}

/** Whether a building or crop's id, such as `research_desk`, matches the search. */
export function matches(query: string, id: string): boolean {
  const q = query.trim().toLowerCase().replace(/[\s_]+/g, " ");
  return q !== "" && id.replace(/_/g, " ").includes(q);
}
