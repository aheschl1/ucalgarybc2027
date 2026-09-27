// @vitest-environment jsdom

import type { SetReplay, Tick } from "@ucbc/viewer";
import { expect, it } from "vitest";

import type { State } from "./api.gen.ts";
import { renderer } from "./index.ts";

const set: SetReplay = {
  index: 1,
  first_team: 1,
  initial_state: { tick: 0 },
  ticks: [],
  result: { index: 1, first_team: 1, reason: "draw", ticks: 0 },
};
const teams = [
  { id: 0, name: "alpha" },
  { id: 1, name: "beta" },
];

it("draws the tick and the set's step order", () => {
  const state: State = { tick: 3 };
  const tick: Tick = { number: 2, state_after: state, steps: [] };
  const el = document.createElement("div");
  const instance = renderer.mount(el);
  instance.draw({ state, tick, set, teams });

  expect(el.querySelector(".u27-state")?.textContent).toBe("tick 3");
  expect(el.querySelector(".u27-legend")?.textContent).toBe("beta steps first, then alpha");

  instance.destroy();
  expect(el.children).toHaveLength(0);
});

it("rejects a state it does not recognise", () => {
  const instance = renderer.mount(document.createElement("div"));
  expect(() => instance.draw({ state: null, tick: null, set, teams })).toThrow("not a ucbc2027 state");
  expect(() => instance.draw({ state: { cells: [] }, tick: null, set, teams })).toThrow("not a ucbc2027 state");
});
