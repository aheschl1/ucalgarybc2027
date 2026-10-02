// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react";
import type { SetReplay, Tick } from "@ucbc/viewer";
import { afterEach, expect, it, vi } from "vitest";

import type { Snapshot } from "./api.gen.ts";
import { renderer } from "./index.tsx";

const { Board, Info } = renderer;
afterEach(cleanup);

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
    environment: [[{ type: "lab", team: 1 }, { type: "wall" }, { type: "empty" }, { type: "empty" }]],
    units: [
      { id: 0, team: 1, type: "lab", origin: { x: 0, y: 0 }, health: 100 },
      { id: 2, team: 0, type: "dino", pos: { x: 2, y: 0 }, level: 1, health: 10, held: { type: "fossil" } },
    ],
    fossils: [{ x: 3, y: 0 }],
    teams: [
      { bones: 50, fossils: 0 },
      { bones: 40, fossils: 2 },
    ],
  };
  const tick: Tick = { number: 2, state_after: state, steps: [] };
  const frame = { state, tick, set, teams };
  const el = render(<Board frame={frame} selected={null} onSelect={() => {}} />).container;
  const info = render(<Info frame={frame} selected={null} />).container;

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
});

it("selects a dino or a lab by its tiles, and shows the selected unit", () => {
  const lab = { type: "lab", team: 0 } as const;
  const empty = { type: "empty" } as const;
  const state: Snapshot = {
    tick: 3,
    environment: [
      [lab, lab, empty, empty],
      [lab, lab, empty, empty],
    ],
    units: [
      { id: 4, team: 0, type: "lab", origin: { x: 0, y: 0 }, health: 90 },
      { id: 7, team: 1, type: "dino", pos: { x: 3, y: 1 }, level: 2, health: 10, held: { type: "fossil" } },
    ],
    fossils: [],
    teams: [
      { bones: 0, fossils: 0 },
      { bones: 0, fossils: 0 },
    ],
  };
  const frame = { state, tick: null, set, teams };
  const onSelect = vi.fn();
  const board = render(<Board frame={frame} selected={7} onSelect={onSelect} />);
  const tiles = board.container.querySelectorAll(".u27-tile");
  for (const i of [5, 7, 2]) fireEvent.click(tiles[i]!);
  expect(onSelect.mock.calls).toEqual([[4], [7], [null]]);
  const selectedTiles = () => [...board.container.querySelectorAll(".u27-selected")].map((t) => [...tiles].indexOf(t));
  expect(selectedTiles()).toEqual([7]);
  board.rerender(<Board frame={frame} selected={4} onSelect={onSelect} />);
  expect(selectedTiles()).toEqual([0, 1, 4, 5]);

  const info = render(<Info frame={frame} selected={7} />);
  const rows = [...info.container.querySelectorAll(".u27-unit tr")].map((r) => r.textContent);
  expect(rows).toEqual(["Unitdino", "Level2", "At3, 1", "Health10", "Holdsa fossil"]);
  info.rerender(<Info frame={frame} selected={9} />);
  expect(info.container.textContent).toContain("Bot 9 is not on the board.");
});

it("rejects a state it does not recognise", () => {
  // React logs what a component throws.
  vi.spyOn(console, "error").mockImplementation(() => {});
  for (const state of [null, { tick: 1 }]) {
    const frame = { state, tick: null, set, teams };
    expect(() => render(<Board frame={frame} selected={null} onSelect={() => {}} />)).toThrow("not a ucbc2027 state");
    expect(() => render(<Info frame={frame} selected={null} />)).toThrow("not a ucbc2027 state");
  }
  vi.restoreAllMocks();
});
