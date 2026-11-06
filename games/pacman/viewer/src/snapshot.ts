import { useFrame } from "@ucbc/viewer";
import { useMemo } from "react";

import type { Snapshot } from "./api.gen.ts";

/** `state` as a snapshot. Throws if it is not one, e.g. from an older engine. */
export function snapshot(state: unknown): Snapshot {
  const s = state as Snapshot;
  if (typeof s?.tick !== "number" || !Array.isArray(s.maze) || !Array.isArray(s.agents)) {
    throw new Error(`not a pacman state: ${JSON.stringify(state)}`);
  }
  return s;
}

/** The frame's snapshot. */
export function useSnapshot(): Snapshot {
  const { state } = useFrame();
  return useMemo(() => snapshot(state), [state]);
}
