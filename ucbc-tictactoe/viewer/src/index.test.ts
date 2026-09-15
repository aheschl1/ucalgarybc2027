// @vitest-environment jsdom

import type { SetReplay, Tick } from "@ucbc/viewer";
import { expect, it } from "vitest";

import type { Board } from "./api.gen.ts";
import { ticTacToe } from "./index.ts";

it("draws marks, highlights this tick's placements, and names X and O", () => {
  const board: Board = { cells: ["x", "empty", "empty", "empty", "empty", "empty", "empty", "empty", "o"] };
  const tick: Tick = {
    number: 0,
    state_after: board,
    steps: [
      { bot: 0, team: 1, actions: [{ type: "place", row: 0, col: 0 }], usage: { time_us: 1 } },
      { bot: 1, team: 0, actions: [{ type: "place", row: 2, col: 2 }], usage: { time_us: 1 } },
    ],
  };
  const set: SetReplay = {
    index: 1,
    first_team: 1,
    initial_state: { cells: Array(9).fill("empty") },
    ticks: [tick],
    result: { index: 1, first_team: 1, reason: "draw", ticks: 1 },
  };
  const teams = [
    { id: 0, name: "alpha" },
    { id: 1, name: "beta" },
  ];

  const el = document.createElement("div");
  const instance = ticTacToe.mount(el);
  instance.draw({ state: board, tick, set, teams });

  const cells = [...el.querySelectorAll(".ttt-cell")];
  expect(cells.map((c) => c.textContent).join(",")).toBe("X,,,,,,,,O");
  expect(cells.map((c, i) => (c.classList.contains("ttt-placed") ? i : -1)).filter((i) => i >= 0)).toEqual([0, 8]);
  expect(el.querySelector(".ttt-legend")?.textContent).toBe("X beta · O alpha");

  instance.draw({ state: set.initial_state, tick: null, set, teams });
  expect(el.querySelectorAll(".ttt-placed")).toHaveLength(0);

  instance.destroy();
  expect(el.children).toHaveLength(0);
});

it("rejects a board it does not recognise", () => {
  const set: SetReplay = {
    index: 0,
    first_team: 0,
    initial_state: null,
    ticks: [],
    result: { index: 0, first_team: 0, reason: "draw", ticks: 0 },
  };
  const instance = ticTacToe.mount(document.createElement("div"));
  // Cells as integers: replays from before `Cell` serialized as strings.
  const old = { cells: [1, 0, 0, 0, 0, 0, 2, 0, 0] };
  expect(() => instance.draw({ state: old, tick: null, set, teams: [] })).toThrow("not a tic-tac-toe board");
  expect(() => instance.draw({ state: null, tick: null, set, teams: [] })).toThrow("not a tic-tac-toe board");
});
