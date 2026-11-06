// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react";
import { ViewerContext, type BotId, type Frame, type SetReplay, type Tick } from "@ucbc/viewer";
import type { ReactNode } from "react";
import { afterEach, expect, it, vi } from "vitest";

import type { Snapshot } from "./api.gen.ts";
import { renderer } from "./index.tsx";

const { Board, Info } = renderer;
afterEach(cleanup);

function at(frame: Frame, node: ReactNode, selected: BotId | null = null, select = (_: BotId | null) => {}) {
  return <ViewerContext value={{ frame, selected, select }}>{node}</ViewerContext>;
}

const state: Snapshot = {
  tick: 7,
  maze: ["#.o G#"],
  agents: [
    { id: 0, role: "pacman", pos: { x: 3, y: 0 }, frightened: 0 },
    { id: 1, role: "ghost", pos: { x: 4, y: 0 }, frightened: 0 },
    { id: 2, role: "ghost", pos: { x: 1, y: 0 }, frightened: 3 },
  ],
  pacman_team: 1,
  ghost_team: 0,
  pellets_left: 2,
  ghosts_eaten: 1,
};
const set: SetReplay = {
  index: 1,
  first_team: 1,
  initial_state: state,
  ticks: [],
  result: { index: 1, first_team: 1, reason: "draw", ticks: 0 },
};
const teams = [
  { id: 0, name: "alpha" },
  { id: 1, name: "beta" },
];
const tick: Tick = {
  number: 6,
  state_after: state,
  steps: [{ bot: 0, team: 1, actions: [{ type: "move", dir: "left" }], usage: { time_us: 1 } }],
};

it("draws the maze, the agents, and who plays which side", () => {
  const frame = { state, tick, set, teams };
  const el = render(at(frame, <Board />)).container;
  const info = render(at(frame, <Info />)).container;

  expect(el.querySelectorAll(".pm-wall")).toHaveLength(2);
  expect(el.querySelectorAll(".pm-pellet")).toHaveLength(1);
  expect(el.querySelectorAll(".pm-power")).toHaveLength(1);
  expect(el.querySelectorAll(".pm-house")).toHaveLength(1);
  const placed = [...el.querySelectorAll("[data-bot]")].map((a) => [a.getAttribute("data-bot"), a.getAttribute("transform")]);
  expect(placed).toEqual([
    ["1", "translate(64 0)"],
    ["2", "translate(16 0)"],
    ["0", "translate(48 0)"],
  ]);
  // Pac-Man faces the way it moved; the frightened ghost is blue.
  expect(el.querySelector(".pm-pacman")?.getAttribute("transform")).toBe("rotate(180 8 8)");
  expect(el.querySelectorAll(".pm-frightened")).toHaveLength(1);
  expect(el.querySelectorAll(".pm-stack")).toHaveLength(0);

  const rows = [...info.querySelectorAll("tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(rows).toEqual([
    ["Plays", "Team"],
    ["Pac-Man", "beta"],
    ["2 ghosts", "alpha"],
    ["Pellets left", "2"],
    ["Ghosts eaten", "1"],
  ]);
});

it("selects an agent on click and shows it", () => {
  const select = vi.fn();
  const frame = { state, tick: null, set, teams };
  const el = render(at(frame, <Board />, null, select)).container;
  fireEvent.click(el.querySelector('[data-bot="2"] .pm-hit')!);
  fireEvent.click(el.querySelector(".pm-floor")!);
  expect(select.mock.calls).toEqual([[2], [null]]);
  // Without a tick, Pac-Man faces right.
  expect(el.querySelector(".pm-pacman")?.getAttribute("transform")).toBe("rotate(0 8 8)");

  const info = render(at(frame, <Info />, 2)).container;
  const detail = [...info.querySelectorAll<HTMLTableRowElement>(".pm-detail tr")].map((r) => [...r.cells].map((c) => c.textContent));
  expect(detail).toEqual([
    ["Agent", "ghost 2"],
    ["At", "1, 0"],
    ["Frightened", "3 ticks"],
  ]);
  const gone = render(at(frame, <Info />, 9)).container;
  expect(gone.textContent).toContain("Bot 9 is not on the board.");
});

it("counts ghosts that share a tile", () => {
  const ghost = { role: "ghost", pos: { x: 4, y: 0 }, frightened: 0 } as const;
  const stacked: Snapshot = {
    ...state,
    agents: [state.agents[0]!, { ...ghost, id: 1 }, { ...ghost, id: 2 }, { ...ghost, id: 3 }],
  };
  const el = render(at({ state: stacked, tick: null, set, teams }, <Board />)).container;
  const badges = [...el.querySelectorAll(".pm-stack")];
  expect(badges.map((b) => [b.parentElement?.getAttribute("transform"), b.textContent])).toEqual([
    ["translate(64 0)", "3"],
  ]);
});

it("rejects a state it does not recognise", () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  for (const bad of [null, { tick: 0, environment: [] }]) {
    expect(() => render(at({ state: bad, tick: null, set, teams }, <Board />))).toThrow("not a pacman state");
  }
  const tile = { ...state, maze: ["#x#"] };
  expect(() => render(at({ state: tile, tick: null, set, teams }, <Board />))).toThrow("not a pacman tile");
  vi.restoreAllMocks();
});
