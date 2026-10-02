// @vitest-environment jsdom

import type { SetReplay, Tick } from "@ucbc/viewer";
import { expect, it } from "vitest";

import type { Snapshot } from "./api.gen.ts";
import { renderer } from "./index.ts";

const set: SetReplay = {
  index: 1,
  first_team: 1,
  initial_state: { tick: 0, environment: [], units: [], fossils: [], teams: [] },
  ticks: [],
  result: { index: 1, first_team: 1, reason: "draw", ticks: 0 },
};
const teams = [
  { id: 0, name: "alpha" },
  { id: 1, name: "beta" },
];

it("draws the board, a table of team counts, and the set's step order", () => {
  const state: Snapshot = {
    tick: 3,
    environment: [
      [{ type: "lab", team: 1 }, { type: "wall" }, { type: "empty" }, { type: "empty" }],
    ],
    units: [
      { id: 0, team: 1, type: "lab", origin: { x: 0, y: 0 }, health: 100 },
      { id: 2, team: 0, type: "dino", pos: { x: 2, y: 0 }, level: 1, health: 10, held: { type: "fossil" } },
    ],
    fossils: [{ x: 3, y: 0 }],
    teams: [{ bones: 50, fossils: 0 }, { bones: 40, fossils: 2 }],
  };
  const tick: Tick = { number: 2, state_after: state, steps: [] };
  const el = document.createElement("div");
  const info = document.createElement("div");
  const instance = renderer.mount(el, info);
  instance.draw({ state, tick, set, teams });

  const rows = [...info.querySelectorAll("tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(rows).toEqual([
    ["Team", "Bones", "Fossils", "Dinos"],
    ["alpha", "50", "0", "1"],
    ["beta", "40", "2", "0"],
  ]);
  const tiles = el.querySelectorAll(".u27-tile");
  expect(tiles).toHaveLength(4);
  expect(tiles[0]?.className).toBe("u27-tile u27-lab u27-team-1");
  expect(tiles[1]?.className).toBe("u27-tile u27-wall");
  expect(tiles[2]?.className).toBe("u27-tile u27-empty u27-dino u27-team-0 u27-held");
  expect(tiles[2]?.textContent).toBe("1");
  expect(tiles[3]?.className).toBe("u27-tile u27-empty u27-fossil");
  expect(info.querySelector(".ucbc-muted")?.textContent).toBe("beta steps first, then alpha");

  instance.destroy();
  expect(el.children).toHaveLength(0);
  expect(info.children).toHaveLength(0);
});

it("rejects a state it does not recognise", () => {
  const instance = renderer.mount(document.createElement("div"), document.createElement("div"));
  expect(() => instance.draw({ state: null, tick: null, set, teams })).toThrow("not a ucbc2027 state");
  expect(() => instance.draw({ state: { tick: 1 }, tick: null, set, teams })).toThrow("not a ucbc2027 state");
});
