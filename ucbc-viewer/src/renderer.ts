import type { ComponentType } from "react";

import type { BotId, SetReplay, TeamInfo, Tick } from "./replay.gen.ts";

/** One point of a set: the game state and the tick that produced it. */
export interface Frame {
  state: unknown;
  /** `null` for the state before the first tick. */
  tick: Tick | null;
  set: SetReplay;
  teams: TeamInfo[];
}

export interface FrameProps {
  frame: Frame;
  /** The bot the viewer has selected; it may not be in this frame. */
  selected: BotId | null;
}

export interface BoardProps extends FrameProps {
  /** Selects a bot, or clears the selection with `null`. */
  onSelect(bot: BotId | null): void;
}

/** What a game provides to the viewer. Both components throw on state they do not
 * recognise, e.g. a replay from an older engine. */
export interface GameRenderer {
  /** Matches `replay.config.game`. */
  game: string;
  /** Drawn in the zoomable view. */
  Board: ComponentType<BoardProps>;
  /** A panel beside the board for text, such as scores. */
  Info: ComponentType<FrameProps>;
}
