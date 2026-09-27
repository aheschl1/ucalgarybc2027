import "./state.css";

import type { GameRenderer } from "@ucbc/viewer";

import type { State } from "./api.gen.ts";

/** Draws the tick count and the set's step order.
 * Every game package exports its renderer under this name; the viewer's build finds it. */
export const renderer: GameRenderer = {
  game: "ucbc2027",
  mount(el) {
    const state = document.createElement("div");
    state.className = "u27-state";
    const legend = document.createElement("div");
    legend.className = "u27-legend";
    el.append(state, legend);

    return {
      draw({ state: s, set, teams }) {
        const current = s as State;
        if (typeof current?.tick !== "number") {
          throw new Error(`not a ucbc2027 state: ${JSON.stringify(s)}`);
        }
        state.textContent = `tick ${current.tick}`;
        const first = teams[set.first_team]?.name ?? "";
        const second = teams.find((t) => t.id !== set.first_team)?.name ?? "";
        legend.textContent = `${first} steps first, then ${second}`;
      },
      destroy() {
        state.remove();
        legend.remove();
      },
    };
  },
};
