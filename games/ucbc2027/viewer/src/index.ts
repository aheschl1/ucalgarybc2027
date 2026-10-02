import "./state.css";

import type { GameRenderer } from "@ucbc/viewer";

import type { Snapshot } from "./api.gen.ts";

function cell(tag: "td" | "th", text: string, className = ""): HTMLTableCellElement {
  const el = document.createElement(tag);
  el.className = className;
  el.textContent = text;
  return el;
}

function row(...cells: HTMLTableCellElement[]): HTMLTableRowElement {
  const tr = document.createElement("tr");
  tr.append(...cells);
  return tr;
}

/** Draws the board, and a table of each team's bones, fossils, and dinos with the set's
 * step order. Every game package exports its renderer under this name; the viewer's
 * build finds it. */
export const renderer: GameRenderer = {
  game: "ucbc2027",
  mount(el, info) {
    const board = document.createElement("div");
    board.className = "u27-board";
    const table = document.createElement("table");
    table.className = "ucbc-table";
    const head = document.createElement("thead");
    head.append(row(cell("th", "Team"), ...["Bones", "Fossils", "Dinos"].map((c) => cell("th", c, "ucbc-num"))));
    const body = document.createElement("tbody");
    table.append(head, body);
    const legend = document.createElement("div");
    legend.className = "ucbc-muted";
    el.append(board);
    info.append(table, legend);

    return {
      draw({ state: s, set, teams }) {
        const current = s as Snapshot;
        if (typeof current?.tick !== "number" || !Array.isArray(current.environment)) {
          throw new Error(`not a ucbc2027 state: ${JSON.stringify(s)}`);
        }
        const dinos = current.teams.map((_, i) => current.units.filter((u) => u.type === "dino" && u.team === i).length);
        body.replaceChildren(
          ...current.teams.map((t, i) => {
            const name = cell("td", teams[i]?.name ?? String(i), "ucbc-team");
            const swatch = document.createElement("span");
            swatch.className = "ucbc-swatch";
            swatch.style.background = `var(--ucbc-team-${i})`;
            name.prepend(swatch);
            return row(name, ...[t.bones, t.fossils, dinos[i]!].map((n) => cell("td", String(n), "ucbc-num")));
          }),
        );

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
        board.remove();
        table.remove();
        legend.remove();
      },
    };
  },
};
