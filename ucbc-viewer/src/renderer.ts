import type { SetReplay, TeamInfo, Tick } from "./replay.gen.ts";

/** One point of a set: the game state and the tick that produced it. */
export interface Frame {
  state: unknown;
  /** `null` for the state before the first tick. */
  tick: Tick | null;
  set: SetReplay;
  teams: TeamInfo[];
}

export interface RendererInstance {
  draw(frame: Frame): void;
  destroy(): void;
}

/** What a game provides to the viewer. */
export interface GameRenderer {
  /** Matches `replay.config.game`. */
  game: string;
  /** `board` sits in the zoomable view; `info` is a panel beside it for text, such as scores. */
  mount(board: HTMLElement, info: HTMLElement): RendererInstance;
}
