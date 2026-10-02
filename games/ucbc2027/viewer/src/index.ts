import "./state.css";

import type { GameRenderer } from "@ucbc/viewer";

import type { State } from "./api.gen.ts";

/** Draws the tick count, the board, and the set's step order.
 * Every game package exports its renderer under this name; the viewer's build finds it. */
export const renderer: GameRenderer = {
  game: "ucbc2027",
  mount(el) {
    const state = document.createElement("div");
    state.className = "u27-state";
    const board = document.createElement("div");
    board.className = "u27-board";
    const legend = document.createElement("div");
    legend.className = "u27-legend";
    el.append(state, board, legend);

    return {
      draw({ state: s, set, teams }) {
        const current = s as State;
        if (typeof current?.tick !== "number" || !Array.isArray(current.board)) {
          throw new Error(`not a ucbc2027 state: ${JSON.stringify(s)}`);
        }
        state.textContent = `tick ${current.tick}`;
        board.style.gridTemplateColumns = `repeat(${current.board[0]?.length ?? 0}, auto)`;
        board.replaceChildren(
          ...current.board.flat().map((tile) => {
            const cell = document.createElement("span");
            cell.className = `u27-tile u27-${tile.environment.toLowerCase()}`;
            if (tile.item) {
              cell.textContent = String(tile.item.team);
              cell.classList.add(`u27-team-${tile.item.team}`);
            }
            return cell;
          }),
        );
        const first = teams[set.first_team]?.name ?? "";
        const second = teams.find((t) => t.id !== set.first_team)?.name ?? "";
        legend.textContent = `${first} steps first, then ${second}`;
      },
      destroy() {
        state.remove();
        board.remove();
        legend.remove();
      },
    };
  },
};
