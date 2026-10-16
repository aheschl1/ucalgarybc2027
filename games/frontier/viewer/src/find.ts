// What the search box asks for, shared by the panel and the board.

import { useSyncExternalStore } from "react";

import { BUILDINGS, CROPS } from "./world.ts";

let query = "";
const listeners = new Set<() => void>();
const subscribe = (l: () => void) => (listeners.add(l), () => listeners.delete(l));

export function setFind(q: string) {
  query = q;
  for (const l of listeners) l();
}

export function useFind(): string {
  return useSyncExternalStore(subscribe, () => query);
}

/** A building or crop id as people write it: `research_desk` is "research desk". */
export const name = (id: string) => id.replace(/_/g, " ");

/** The buildings and crops the search finds: the one it names exactly, or else every one
 * with a word starting with it. */
export function found(query: string): Set<string> {
  const q = query.trim().toLowerCase().replace(/[\s_]+/g, " ");
  const ids: string[] = [...BUILDINGS, ...CROPS];
  if (!q) return new Set();
  const exact = ids.filter((id) => name(id) === q);
  return new Set(exact.length ? exact : ids.filter((id) => ` ${name(id)}`.includes(` ${q}`)));
}
