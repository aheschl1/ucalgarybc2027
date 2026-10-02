import "./board.css";

import type { GameRenderer } from "@ucbc/viewer";

import type { Action, Board } from "./api.gen.ts";

const SYMBOL = { empty: "", x: "X", o: "O" } as const;

/** The first team of a set plays X. Cells placed during the drawn tick are highlighted.
 * Every game package exports its renderer under this name; the viewer's build finds it. */
export const renderer: GameRenderer = {
  game: "tictactoe",
  mount(el, info) {
    const grid = document.createElement("div");
    grid.className = "ttt-board";
    const cells = Array.from({ length: 9 }, () => grid.appendChild(document.createElement("div")));
    const legend = document.createElement("div");
    legend.className = "ucbc-muted";
    el.append(grid);
    info.append(legend);

    return {
      draw({ state, tick, set, teams }) {
        const board = state as Board;
        if (board?.cells?.length !== 9 || !board.cells.every((c) => Object.hasOwn(SYMBOL, c))) {
          throw new Error(`not a tic-tac-toe board: ${JSON.stringify(state)}`);
        }
        const placed = new Set(
          (tick?.steps ?? [])
            .flatMap((s) => s.actions as Action[])
            .map((a) => a.row * 3 + a.col),
        );
        board.cells.forEach((cell, i) => {
          const div = cells[i]!;
          div.textContent = SYMBOL[cell];
          div.className = `ttt-cell ttt-${cell}${placed.has(i) ? " ttt-placed" : ""}`;
        });
        const x = teams[set.first_team]?.name ?? "";
        const o = teams.find((t) => t.id !== set.first_team)?.name ?? "";
        legend.textContent = `X ${x} · O ${o}`;
      },
      destroy() {
        grid.remove();
        legend.remove();
      },
    };
  },
};
