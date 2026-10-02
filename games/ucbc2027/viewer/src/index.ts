import "./state.css";

import type { GameRenderer } from "@ucbc/viewer";

import type { Snapshot } from "./api.gen.ts";

/** Draws the tick, each team's bones and fossils, the board, and the set's step order.
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
        const current = s as Snapshot;
        if (typeof current?.tick !== "number" || !Array.isArray(current.environment)) {
          throw new Error(`not a ucbc2027 state: ${JSON.stringify(s)}`);
        }
        const scores = current.teams.map((t, i) => `${teams[i]?.name ?? i} ${t.bones} bones ${t.fossils} fossils`);
        state.textContent = [`tick ${current.tick}`, ...scores].join(" · ");

        const width = current.environment[0]?.length ?? 0;
        board.style.gridTemplateColumns = `repeat(${width}, auto)`;
        const tiles = current.environment.flat().map((env) => {
          const tile = document.createElement("span");
          tile.className = `u27-tile u27-${env.type}`;
          if (env.type === "lab") tile.classList.add(`u27-team-${env.team}`);
          return tile;
        });
        const at = ({ x, y }: { x: number; y: number }) => tiles[y * width + x];
        for (const fossil of current.fossils) {
          const tile = at(fossil);
          if (tile) {
            tile.classList.add("u27-fossil");
            tile.textContent = "◆";
          }
        }
        for (const unit of current.units) {
          if (unit.type !== "dino") continue;
          const tile = at(unit.pos);
          if (!tile) continue;
          tile.classList.add("u27-dino", `u27-team-${unit.team}`);
          if (unit.held) tile.classList.add("u27-held");
          tile.textContent = String(unit.level);
        }
        board.replaceChildren(...tiles);

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
