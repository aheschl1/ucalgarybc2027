// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react";
import { ViewerContext, type BotId, type Frame, type SetReplay } from "@ucbc/viewer";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import { renderer } from "./index.tsx";

const { Board, Info } = renderer;
afterEach(cleanup);

const SIZE = 8;
const set: SetReplay = {
  index: 0,
  first_team: 0,
  initial_state: {
    tick: 0,
    keyframe: true,
    map: { size: SIZE, vision: 3, tiles: btoa("\0".repeat(SIZE * SIZE)) },
    units: [0, 0, 0, 1, 1, 100, 0, 1, 1, 0, 6, 6, 70, 0, 2, 0, 1, 2, 1, 100, 4],
    removed: [],
    tiles: [],
    buildings: [5, 0, 0, 1, 2, 500, 0, 6, 1, 1, 5, 5, 50, 3],
    demolished: [],
    piles: [],
    crops: [],
    cleared: [],
    signs: [],
    points: [120, 80],
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
    ["Team", "Points", "Units", "Buildings", "Commander"],
    ["alpha", "120", "2", "1", "100"],
    ["beta", "80", "1", "0", "70"],
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
  const detail = [...info.querySelectorAll<HTMLTableRowElement>(".fr-detail tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(detail).toEqual([
    ["Unit", "worker"],
    ["At", "2, 1"],
    ["Health", "100 / 100"],
    ["Carrying", "4 of 20 items"],
  ]);
});
