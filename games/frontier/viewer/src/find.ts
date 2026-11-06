// What the search box asks for, shared by the panel and the board.

import { store } from "./store.ts";
import { BUILDINGS, CROPS } from "./world.ts";

const query = store("");
export const setFind = query.set;
export const useFind = query.use;

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
