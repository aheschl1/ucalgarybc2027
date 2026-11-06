// Viewer state the board and the panel share, outside the shared viewer's own.

import { useSyncExternalStore } from "react";

export interface Store<T> {
  set(value: T): void;
  use(): T;
}

export function store<T>(initial: T): Store<T> {
  let value = initial;
  const listeners = new Set<() => void>();
  const subscribe = (l: () => void) => (listeners.add(l), () => listeners.delete(l));
  return {
    set(v) {
      value = v;
      for (const l of listeners) l();
    },
    use: () => useSyncExternalStore(subscribe, () => value),
  };
}

/** The building clicked on the board, by id. */
export const picked = store<number | null>(null);
