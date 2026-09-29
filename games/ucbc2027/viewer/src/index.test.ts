// @vitest-environment jsdom

import type { SetReplay, Tick } from "@ucbc/viewer";
import { expect, it } from "vitest";

import type { State } from "./api.gen.ts";
import { renderer } from "./index.ts";

const set: SetReplay = {
  index: 1,
  first_team: 1,
  initial_state: { tick: 0, board: [] },
  ticks: [],
  result: { index: 1, first_team: 1, reason: "draw", ticks: 0 },
};
const teams = [
  { id: 0, name: "alpha" },
  { id: 1, name: "beta" },
];

it("draws the tick, the board, and the set's step order", () => {
  const state: State = {
    tick: 3,
    board: [
      [
        { environment: "Empty", item: { type: "player", team: 1 } },
        { environment: "Wall", item: null },
      ],
    ],
  };
  const tick: Tick = { number: 2, state_after: state, steps: [] };
  const el = document.createElement("div");
  const instance = renderer.mount(el);
  instance.draw({ state, tick, set, teams });

  expect(el.querySelector(".u27-state")?.textContent).toBe("tick 3");
  const tiles = el.querySelectorAll(".u27-tile");
  expect(tiles).toHaveLength(2);
  expect(tiles[0]?.className).toBe("u27-tile u27-empty u27-team-1");
  expect(tiles[0]?.textContent).toBe("1");
  expect(tiles[1]?.className).toBe("u27-tile u27-wall");
  expect(el.querySelector(".u27-legend")?.textContent).toBe("beta steps first, then alpha");

  instance.destroy();
  expect(el.children).toHaveLength(0);
});

it("rejects a state it does not recognise", () => {
  const instance = renderer.mount(document.createElement("div"));
  expect(() => instance.draw({ state: null, tick: null, set, teams })).toThrow("not a ucbc2027 state");
  expect(() => instance.draw({ state: { tick: 1 }, tick: null, set, teams })).toThrow("not a ucbc2027 state");
});
