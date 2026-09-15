import type { SetReplay, TeamInfo } from "./replay.gen.ts";
import type { Frame } from "./renderer.ts";

/** The initial state plus one frame per tick. */
export function frameCount(set: SetReplay): number {
  return set.ticks.length + 1;
}

/** Frame 0 is `initial_state`; frame n is the state after tick n - 1. Clamped. */
export function frameAt(set: SetReplay, teams: TeamInfo[], index: number): Frame {
  const i = Math.max(0, Math.min(index, set.ticks.length));
  const tick = i === 0 ? null : set.ticks[i - 1]!;
  return { state: tick ? tick.state_after : set.initial_state, tick, set, teams };
}
