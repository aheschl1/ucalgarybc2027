// @vitest-environment jsdom

import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useFrame, useSelection, type ViewerState } from "./hooks.ts";
import type { Frame, GameRenderer } from "./renderer.ts";
import type { Replay } from "./replay.gen.ts";
import { formatAction, Viewer } from "./viewer.tsx";

const replay: Replay = {
  match_id: "m",
  engine_version: "0.1.0",
  config: {
    game: "counter",
    sets: 2,
    seed: 0,
    teams: 2,
    limits: { step_ms: 500, memory_bytes: 1 << 30 },
  },
  teams: [
    { id: 0, name: "alpha" },
    { id: 1, name: "beta" },
  ],
  sets: [0, 1].map((index) => ({
    index,
    first_team: index,
    initial_state: 0,
    ticks: [1, 2].map((n) => ({
      number: n - 1,
      state_after: n,
      steps: [
        {
          bot: 0,
          team: 0,
          actions: [{ type: "add" }],
          stdout: "hello",
          usage: { time_us: 1500 },
        },
        {
          bot: 1,
          team: 1,
          actions: [],
          failure: { kind: "Exception", message: "boom", traceback: "Traceback" },
          usage: { time_us: 10 },
        },
      ],
    })),
    result: { index, first_team: index, winner_team: 1, reason: "win", ticks: 2 },
  })),
  result: { sets: [], set_wins: [0, 2], winner_team: 1 },
};

afterEach(cleanup);

function mount(renderers: GameRenderer[], game = replay.config.game) {
  const frames: Frame[] = [];
  let board: Pick<ViewerState, "selected" | "select"> | undefined;
  const counter: GameRenderer = {
    game: "counter",
    Board: () => {
      frames.push(useFrame());
      board = useSelection();
      return null;
    },
    Info: () => null,
  };
  const { container } = render(
    <Viewer renderers={[...renderers, counter]} replay={{ ...replay, config: { ...replay.config, game } }} />,
  );
  const root = container.querySelector<HTMLElement>(".ucbc-viewer")!;
  const key = (k: string) => fireEvent.keyDown(root, { key: k });
  const text = (selector: string) => root.querySelector(selector)?.textContent;
  // What the board shows selected, and clicking a bot on it.
  const selected = () => board?.selected;
  const select = (bot: number | null) => act(() => board?.select(bot));
  return { root, frames, key, text, selected, select };
}

describe("formatAction", () => {
  it("puts the type first, then each other field", () => {
    expect(formatAction({ x: 9, type: "move", y: 10 })).toBe("move x=9 y=10");
    expect(formatAction({ row: 1, col: 2 })).toBe("row=1 col=2");
    expect(formatAction({ type: "drop", at: { x: 1 } })).toBe('drop at={"x":1}');
    expect(formatAction(3)).toBe("3");
  });
});

describe("Viewer", () => {
  it("shows the match result and starts at the first frame", () => {
    const { frames, text } = mount([]);
    expect(text(".ucbc-title")).toBe("alpha vs beta");
    expect(text(".ucbc-result")).toBe("beta wins (alpha 0, beta 2)");
    expect(text(".ucbc-set-result")).toBe("Set 1: beta wins by win after 2 ticks");
    expect(frames.at(-1)?.state).toBe(0);
    expect(text(".ucbc-position")).toBe("start / 2");
  });

  it("steps with the keyboard and lists the tick's steps", () => {
    const { root, frames, key, text } = mount([]);
    key("ArrowRight");
    expect(frames.at(-1)?.state).toBe(1);
    expect(text(".ucbc-position")).toBe("tick 1 / 2");
    const steps = root.querySelectorAll(".ucbc-step");
    expect(steps).toHaveLength(2);
    expect(steps[0]!.querySelector(".ucbc-action")?.textContent).toBe("add");
    expect(steps[0]!.querySelector(".ucbc-stdout")?.textContent).toBe("hello");
    expect(steps[1]!.querySelector(".ucbc-error")?.textContent).toBe("Exception: boom");
    key("End");
    expect(frames.at(-1)?.state).toBe(2);
    key("ArrowRight");
    expect(frames.at(-1)?.state).toBe(2);
    key("Home");
    expect(frames.at(-1)?.state).toBe(0);
  });

  it("switches sets from the start", () => {
    const { root, frames, key, text } = mount([]);
    key("End");
    fireEvent.click(root.querySelectorAll(".ucbc-sets button")[1]!);
    expect(frames.at(-1)?.set.index).toBe(1);
    expect(frames.at(-1)?.tick).toBeNull();
    expect(text(".ucbc-set-result")).toBe("Set 2: beta wins by win after 2 ticks");
  });

  it("shows a renderer's error instead of a silently wrong board", () => {
    // React logs what a component throws.
    vi.spyOn(console, "error").mockImplementation(() => {});
    const picky: GameRenderer = {
      game: "counter",
      Board: () => {
        if (useFrame().state === 2) throw new Error("bad state");
        return null;
      },
      Info: () => null,
    };
    const { key, text } = mount([picky]);
    expect(text(".ucbc-draw-error")).toBeUndefined();
    key("End");
    expect(text(".ucbc-draw-error")).toBe("Cannot draw this frame: bad state");
    key("Home");
    expect(text(".ucbc-draw-error")).toBeUndefined();
    vi.restoreAllMocks();
  });

  it("zooms the board with the keyboard", () => {
    const { root, key } = mount([]);
    const board = root.querySelector<HTMLElement>(".ucbc-board")!;
    key("+");
    expect(board.style.transform).toBe("translate(0px, 0px) scale(1.25)");
    key("-");
    key("-");
    expect(board.style.transform).toBe("translate(0px, 0px) scale(0.8)");
    key("0");
    expect(board.style.transform).toBe("translate(0px, 0px) scale(1)");
  });

  it("falls back to raw JSON for an unknown game", () => {
    const { text } = mount([], "chess");
    expect(text(".ucbc-info .ucbc-error")).toBe('No renderer for game "chess".');
    expect(text(".ucbc-board .ucbc-raw")).toBe("0");
  });

  it("selects a bot from the steps table and logs its output over the set", () => {
    const { root, key, text, selected } = mount([]);
    key("ArrowRight");
    fireEvent.click(root.querySelectorAll(".ucbc-step")[0]!);
    expect(selected()).toBe(0);
    expect(root.querySelector(".ucbc-step")?.classList).toContain("ucbc-selected");
    expect(text(".ucbc-bot-header")).toBe("Bot 0 · alpha×");
    const entries = () => [...root.querySelectorAll(".ucbc-entry")].map((e) => [e.className, e.textContent]);
    expect(entries()).toEqual([
      ["ucbc-entry ucbc-current", "tick 1hello"],
      ["ucbc-entry ucbc-future", "tick 2hello"],
    ]);
    fireEvent.click(root.querySelectorAll(".ucbc-link")[1]!);
    expect(text(".ucbc-position")).toBe("tick 2 / 2");
    expect(entries().map(([c]) => c)).toEqual(["ucbc-entry", "ucbc-entry ucbc-current"]);
    key("Escape");
    expect(selected()).toBeNull();
    expect(root.querySelector(".ucbc-bot")).toBeNull();
  });

  it("selects from the board, and a new set clears the selection", () => {
    const { root, select, selected, text } = mount([]);
    select(1);
    expect(selected()).toBe(1);
    expect(text(".ucbc-bot-header")).toBe("Bot 1 · beta×");
    expect(text(".ucbc-entry .ucbc-error")).toBe("Exception: boom");
    expect(text(".ucbc-entry .ucbc-traceback")).toBe("Traceback");
    fireEvent.click(root.querySelectorAll(".ucbc-sets button")[1]!);
    expect(selected()).toBeNull();
    select(5);
    expect(text(".ucbc-bot")).toBe("Bot 5×No output in this set.");
  });
});
