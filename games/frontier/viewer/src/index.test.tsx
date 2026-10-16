// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react";
import { ViewerContext, type BotId, type Frame, type SetReplay } from "@ucbc/viewer";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import { setFind } from "./find.ts";
import { picked } from "./store.ts";
import { renderer } from "./index.tsx";

const { Board, Info } = renderer;
afterEach(() => {
  cleanup();
  setFind("");
  picked.set(null);
});

const SIZE = 8;
const set: SetReplay = {
  index: 0,
  first_team: 0,
  initial_state: {
    tick: 0,
    keyframe: true,
    map: { size: SIZE, vision: 3, tiles: btoa("\0".repeat(SIZE * SIZE)), storage: [1000, 40, 0, 0, 0, 0, 0, 30, 0, 10, 0, 0, 0, 0, 30] },
    units: [0, 0, 0, 1, 1, 100, 0, 1, 1, 0, 6, 6, 70, 0, 2, 0, 1, 2, 1, 100, 4],
    carrying: [2, 15, 1, 2, 19, 3],
    removed: [],
    tiles: [],
    buildings: [5, 0, 0, 1, 2, 500, 0, 6, 1, 1, 5, 5, 50, 3],
    stored: [5, 17, 40, 5, 19, 50],
    owed: [6, 19, 5],
    jobs: [],
    demolished: [],
    piles: [],
    crops: [],
    cleared: [],
    signs: [],
    points: [
      { buildings: 100, stored: 10, units: 10 },
      { buildings: 60, stored: 15, units: 5 },
    ],
  },
  ticks: [],
  result: { index: 0, first_team: 0, reason: "draw", ticks: 0 },
};
const teams = [
  { id: 0, name: "alpha" },
  { id: 1, name: "beta" },
];
const frame: Frame = { state: set.initial_state, tick: null, set, teams };

function at(node: ReactNode, selected: BotId | null = null, select = (_: BotId | null) => {}) {
  return <ViewerContext value={{ frame, selected, select }}>{node}</ViewerContext>;
}

it("counts each team's units and finished buildings, and its commander's health", () => {
  const info = render(at(<Info />)).container;
  const rows = [...info.querySelectorAll<HTMLTableRowElement>("table:first-child tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(rows).toEqual([
    ["Team", "Points", "Units", "Buildings", "Cmdr"],
    ["alpha", "120", "2", "1", "100"],
    ["beta", "80", "1", "0", "70"],
  ]);
});

it("charts points over time, and their sources at this tick", () => {
  const info = render(at(<Info />)).container;
  expect(info.querySelectorAll(".fr-chart svg")).toHaveLength(3);
  // Two team lines; each team's stack of three sources, outlined.
  expect(info.querySelectorAll(".fr-line")).toHaveLength(2 + 2);
  expect(info.querySelectorAll(".fr-area")).toHaveLength(2 * 3);
  expect(info.querySelector(".fr-chart svg")?.getAttribute("aria-label")).toContain("alpha 120, beta 80");
  const readouts = [...info.querySelectorAll(".fr-readout")].map((r) => r.textContent);
  expect(readouts).toEqual([
    "alpha at tick 0: 100 buildings · 10 stored · 10 units",
    "beta at tick 0: 60 buildings · 15 stored · 5 units",
  ]);
});

it("selects the unit clicked and shows it", () => {
  const select = vi.fn();
  const board = render(at(<Board />, null, select)).container.querySelector("canvas")!;
  expect(board.width).toBe(SIZE * 512);
  board.getBoundingClientRect = () => ({ left: 0, top: 0, width: 80, height: 80 }) as DOMRect;
  fireEvent.click(board, { clientX: 25, clientY: 15 });
  fireEvent.click(board, { clientX: 75, clientY: 5 });
  expect(select.mock.calls).toEqual([[2], [null]]);

  const info = render(at(<Info />, 2)).container;
  expect(info.querySelector(".fr-detail-head")!.textContent).toBe("workeralpha · 2, 1 · 100 HP");
  const detail = [...info.querySelectorAll<HTMLTableRowElement>(".fr-detail tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(detail).toEqual([["Carrying", "4 of 20: 1 stone, 3 wood"]]);
});

it("counts each kind built, and searches for one when it is clicked", () => {
  const info = render(at(<Info />)).container;
  const chip = (name: string) =>
    [...info.querySelectorAll<HTMLButtonElement>(".fr-chip")].find((c) => c.querySelector(".fr-chip-name")?.textContent === name);
  const search = () => info.querySelector<HTMLInputElement>(".fr-search")!;
  expect(chip("base")!.textContent).toBe("Bbase10");
  expect(chip("chest")!.textContent).toBe("Cchest00+1");
  expect(chip("chest")!.getAttribute("aria-label")).toBe("chest: alpha 0 built, 0 being built; beta 0 built, 1 being built");
  // Kinds nobody has are left out until searched for.
  expect(chip("tower")).toBeUndefined();

  fireEvent.click(chip("chest")!);
  expect(search().value).toBe("chest");
  expect(chip("chest")!.getAttribute("aria-pressed")).toBe("true");
  expect(info.querySelector(".fr-search-row")!.textContent).toBe("1 found");
  fireEvent.click(chip("chest")!);
  expect(search().value).toBe("");

  // A kind's whole name finds only it; otherwise the start of any word.
  fireEvent.change(search(), { target: { value: "tower" } });
  expect([...info.querySelectorAll(".fr-on .fr-chip-name")].map((c) => c.textContent)).toEqual(["tower"]);
  fireEvent.change(search(), { target: { value: "dr" } });
  expect([...info.querySelectorAll(".fr-on .fr-chip-name")].map((c) => c.textContent)).toEqual(["mech drill", "steam drill"]);
  fireEvent.change(search(), { target: { value: "xyz" } });
  expect(info.querySelector(".fr-search-row")!.textContent).toBe("no matches");
});

it("shows what a building clicked holds, and what a site still needs", () => {
  const board = render(at(<Board />)).container.querySelector("canvas")!;
  board.getBoundingClientRect = () => ({ left: 0, top: 0, width: 80, height: 80 }) as DOMRect;
  const detail = () =>
    [...document.querySelectorAll<HTMLTableRowElement>(".fr-detail tr")].map((r) => [...r.cells].map((c) => c.textContent));
  const info = render(at(<Info />));
  const head = () => document.querySelector(".fr-detail-head")!.textContent;
  // The base, at (1, 2): no unit there.
  fireEvent.click(board, { clientX: 15, clientY: 25 });
  expect(head()).toBe("Bbasealpha · 1, 2 · 500 HP");
  expect(detail()).toEqual([["Storage", "90 of 1000: 40 wheat, 50 wood"]]);
  // Beta's chest site, at (5, 5).
  fireEvent.click(board, { clientX: 55, clientY: 55 });
  expect(head()).toBe("Cchestbeta · 5, 5 · 50 HP");
  expect(detail()).toEqual([["Being built", "3 work left; owed 5 wood"]]);
  info.unmount();
});
