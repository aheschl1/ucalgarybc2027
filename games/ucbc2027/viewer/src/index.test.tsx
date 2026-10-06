// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react";
import { ViewerContext, type BotId, type Frame, type SetReplay, type Tick } from "@ucbc/viewer";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import type { Snapshot } from "./api.gen.ts";
import { renderer } from "./index.tsx";
import { assets, dinoTier } from "./skin/assets.ts";

const { Board, Info } = renderer;
afterEach(cleanup);

function at(frame: Frame, node: ReactNode, selected: BotId | null = null, select = (_: BotId | null) => {}) {
  return <ViewerContext value={{ frame, selected, select }}>{node}</ViewerContext>;
}

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

/** Each element's `transform`, or another attribute. */
function attrs(els: Iterable<Element>, name = "transform") {
  return [...els].map((el) => el.getAttribute(name));
}

it("draws the board in layers, a table of team counts, and the set's step order", () => {
  const state: Snapshot = {
    tick: 3,
    environment: [[{ type: "lab", team: 1 }, { type: "wall" }, { type: "empty" }, { type: "empty" }]],
    units: [
      { id: 2, team: 0, type: "dino", pos: { x: 2, y: 0 }, level: 1, health: 10, held: { type: "fossil" }, vision: 3 },
      { id: 0, team: 1, type: "lab", origin: { x: 0, y: 0 }, health: 100, vision: 2 },
    ],
    fossils: [{ x: 3, y: 0 }],
    teams: [
      { bones: 50, fossils: 0 },
      { bones: 40, fossils: 2 },
    ],
  };
  const tick: Tick = { number: 2, state_after: state, steps: [] };
  const frame = { state, tick, set, teams };
  const el = render(at(frame, <Board />)).container;
  const info = render(at(frame, <Info />)).container;

  const rows = [...info.querySelectorAll("tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(rows).toEqual([
    ["Team", "Bones", "Fossils", "Dinos"],
    ["alpha", "50", "0", "1"],
    ["beta", "40", "2", "0"],
  ]);
  expect(info.querySelector(".ucbc-muted")?.textContent).toBe("beta steps first, then alpha");

  const svg = el.querySelector("svg.u27-board")!;
  expect([svg.getAttribute("width"), svg.getAttribute("height")]).toEqual(["64", "16"]);
  const terrain = svg.querySelectorAll(".u27-terrain > g");
  expect(attrs(terrain)).toEqual(["translate(0 0)", "translate(16 0)", "translate(32 0)", "translate(48 0)"]);
  expect(attrs(svg.querySelectorAll(".u27-terrain image"), "href")).toEqual([
    assets.labTile[1],
    assets.wall,
    assets.empty,
    assets.empty,
  ]);
  expect(attrs(svg.querySelectorAll(".u27-items > g"))).toEqual(["translate(48 0)"]);
  expect(attrs(svg.querySelectorAll(".u27-items image"), "href")).toEqual([assets.fossil]);

  // Labs under dinos, whatever the units' order.
  const bots = svg.querySelectorAll(".u27-bot");
  expect(attrs(bots, "data-bot")).toEqual(["0", "2"]);
  expect(attrs(bots)).toEqual(["translate(0 0)", "translate(32 0)"]);
  const [lab, dino] = [...bots];
  expect(attrs(lab!.querySelectorAll("image"), "href")).toEqual([assets.lab[1]]);
  expect(attrs(lab!.querySelectorAll(".u27-hit"), "width")).toEqual(["32"]);
  expect(attrs(dino!.querySelectorAll(".u27-frame"), "href")).toEqual(assets.dino[0]![0]);
  expect(dino!.querySelector(".u27-level")?.textContent).toBe("1");
  expect(attrs(dino!.querySelectorAll(".u27-held image"), "href")).toEqual([assets.fossil]);
  expect(svg.querySelector(".u27-selected")).toBeNull();
  expect(svg.querySelector(".u27-vision")).toBeNull();
});

it("selects a dino or a lab by its footprint, and shows the selected unit", () => {
  const lab = { type: "lab", team: 0 } as const;
  const empty = { type: "empty" } as const;
  const state: Snapshot = {
    tick: 3,
    environment: [
      [lab, lab, empty, empty],
      [lab, lab, empty, empty],
    ],
    units: [
      { id: 4, team: 0, type: "lab", origin: { x: 0, y: 0 }, health: 90, vision: 2 },
      {
        id: 7,
        team: 1,
        type: "dino",
        pos: { x: 3, y: 1 },
        level: 2,
        health: 10,
        held: { type: "dino", id: 5, level: 1, team: 0 },
        vision: 3,
      },
    ],
    fossils: [],
    teams: [
      { bones: 0, fossils: 0 },
      { bones: 0, fossils: 0 },
    ],
  };
  const frame = { state, tick: null, set, teams };
  const onSelect = vi.fn();
  const board = render(at(frame, <Board />, 7, onSelect));
  const bot = (id: number) => board.container.querySelector(`[data-bot="${id}"]`)!;
  fireEvent.click(bot(4).querySelector(".u27-hit")!);
  fireEvent.click(bot(7).querySelector("image")!);
  fireEvent.click(board.container.querySelectorAll(".u27-terrain image")[2]!);
  expect(onSelect.mock.calls).toEqual([[4], [7], [null]]);
  expect(attrs(bot(7).querySelectorAll(".u27-frame"), "href")).toEqual(assets.dino[1]![1]);
  expect(attrs(bot(7).querySelectorAll(".u27-held image"), "href")).toEqual([assets.dino[0]![0]![0]]);

  const ring = () => {
    const r = board.container.querySelector(".u27-selected");
    return r && [r.getAttribute("transform"), r.getAttribute("width")];
  };
  expect(ring()).toEqual(["translate(48 16)", "16"]);
  board.rerender(at(frame, <Board />, 4, onSelect));
  expect(ring()).toEqual(["translate(0 0)", "32"]);
  board.rerender(at(frame, <Board />, 9, onSelect));
  expect(ring()).toBeNull();

  const info = render(at(frame, <Info />, 7));
  const rows = [...info.container.querySelectorAll(".u27-unit tr")].map((r) => r.textContent);
  expect(rows).toEqual([
    "Unitdino",
    "Level2",
    "At3, 1",
    "Health10",
    "Holdsa level 1 dino of alpha",
    "Vision3",
  ]);
  info.rerender(at(frame, <Info />, 9));
  expect(info.container.textContent).toContain("Bot 9 is not on the board.");
});

it("outlines what only the selected unit can see, clipped to the board", () => {
  const empty = { type: "empty" } as const;
  const state: Snapshot = {
    tick: 0,
    environment: Array.from({ length: 10 }, () => Array.from({ length: 10 }, () => empty)),
    units: [
      { id: 0, team: 0, type: "lab", origin: { x: 1, y: 1 }, health: 100, vision: 2 },
      { id: 2, team: 0, type: "dino", pos: { x: 5, y: 6 }, level: 1, health: 10, held: null, vision: 3 },
    ],
    fossils: [],
    teams: [{ bones: 0, fossils: 0 }],
  };
  const frame = { state, tick: null, set, teams };
  const board = render(at(frame, <Board />));
  const sight = () =>
    [...board.container.querySelectorAll(".u27-vision")].map((r) =>
      ["transform", "width", "height"].map((a) => r.getAttribute(a)),
    );
  expect(sight()).toEqual([]);
  // Tiles (2, 3) to (8, 9): three each way, the bottom edge on the board's.
  board.rerender(at(frame, <Board />, 2));
  expect(sight()).toEqual([["translate(32 48)", "112", "112"]]);
  // Two out from the 2x2 footprint, clipped at the top and left: (0, 0) to (4, 4).
  board.rerender(at(frame, <Board />, 0));
  expect(sight()).toEqual([["translate(0 0)", "80", "80"]]);
});

it("rejects a state it does not recognise", () => {
  // React logs what a component throws.
  vi.spyOn(console, "error").mockImplementation(() => {});
  for (const state of [null, { tick: 1 }]) {
    const frame = { state, tick: null, set, teams };
    expect(() => render(at(frame, <Board />))).toThrow("not a ucbc2027 state");
    expect(() => render(at(frame, <Info />))).toThrow("not a ucbc2027 state");
  }
  vi.restoreAllMocks();
});

it("picks a dino's picture tier from its level, the top tier past the last", () => {
  expect([0, 1, 2, 3, 4, 5, 99].map(dinoTier)).toEqual([0, 0, 1, 2, 3, 3, 3]);
});
