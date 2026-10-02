import { createContext, useContext } from "react";

import type { Frame } from "./renderer.ts";
import type { BotId } from "./replay.gen.ts";

/** What the viewer shares with a game's components. */
export interface ViewerState {
  frame: Frame;
  /** The selected bot; it may not be in this frame. */
  selected: BotId | null;
  /** Selects a bot, or clears the selection with `null`. */
  select(bot: BotId | null): void;
}

/** Provided by `Viewer`; a renderer's tests provide it themselves. */
export const ViewerContext = createContext<ViewerState | null>(null);

function useViewer(): ViewerState {
  const state = useContext(ViewerContext);
  if (!state) throw new Error("a viewer hook was used outside a ViewerContext");
  return state;
}

/** The frame on show. */
export function useFrame(): Frame {
  return useViewer().frame;
}

/** The selected bot, and a function to change it. */
export function useSelection(): Pick<ViewerState, "selected" | "select"> {
  const { selected, select } = useViewer();
  return { selected, select };
}
