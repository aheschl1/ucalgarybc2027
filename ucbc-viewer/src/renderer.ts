import type { ComponentType } from "react";

import type { SetReplay, TeamInfo, Tick } from "./replay.gen.ts";

/** One point of a set: the game state and the tick that produced it. */
export interface Frame {
  state: unknown;
  /** `null` for the state before the first tick. */
  tick: Tick | null;
  set: SetReplay;
  teams: TeamInfo[];
}

/** What a game provides to the viewer. Both components read the frame and the selected
 * bot with `useFrame` and `useSelection`, and throw on state they do not recognise, e.g.
 * a replay from an older engine. */
export interface GameRenderer {
  /** Matches `replay.config.game`. */
  game: string;
  /** Drawn in the zoomable view. */
  Board: ComponentType;
  /** A panel beside the board for text, such as scores. */
  Info: ComponentType;
}
