// @vitest-environment jsdom

import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useFrame, useSelection, type ViewerState } from "./hooks.ts";
import type { Frame, GameRenderer } from "./renderer.ts";
import type { Replay } from "./replay.gen.ts";
import { actionMatches, actionParts, formatAction, searchTokens, stepsByBot, Viewer } from "./viewer.tsx";

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

describe("actionParts", () => {
  it("splits the type from the parameters", () => {
    expect(actionParts({ x: 9, type: "move", name: "a" })).toEqual({
      type: "move",
      fields: [
        ["x", "9"],
        ["name", "a"],
      ],
    });
    expect(actionParts({ type: 7 })).toEqual({ type: "7", fields: [] });
    expect(actionParts([1])).toEqual({ type: null, fields: [["", "[1]"]] });
  });
});

describe("actionMatches", () => {
  const move = { type: "move", x: 11, y: 7 };
  const hit = (q: string) => actionMatches(move, searchTokens(q));
  it("matches words anywhere and key=value as a whole parameter", () => {
    expect(hit("move")).toBe(true);
    expect(hit("MOVE  x=11")).toBe(true);
    expect(hit("x=11 y=7")).toBe(true);
    expect(hit("x=1")).toBe(false);
    expect(hit("x=110")).toBe(false);
    expect(hit("grab")).toBe(false);
    expect(hit("")).toBe(false);
  });
});

describe("stepsByBot", () => {
  it("lists each bot's ticks, collapsing runs of idle ones", () => {
    const idle = { actions: [], usage: { time_us: 0 } };
    const set = replay.sets[0]!;
    const ticks = [
      { number: 0, state_after: 1, steps: [{ ...idle, bot: 0, team: 0 }] },
      { number: 1, state_after: 2, steps: [{ ...idle, bot: 0, team: 0 }] },
      { number: 2, state_after: 3, steps: [{ ...idle, bot: 0, team: 0, actions: [{ type: "add" }] }] },
      { number: 3, state_after: 4, steps: [{ ...idle, bot: 0, team: 0, stdout: "hi" }] },
      { number: 4, state_after: 5, steps: [{ ...idle, bot: 0, team: 0 }] },
    ];
    const entries = stepsByBot({ ...set, ticks })
      .get(0)!
      .map((e) => [e.frame, e.to, e.step ? "step" : "idle"]);
    expect(entries).toEqual([
      [1, 2, "idle"],
      [3, 3, "step"],
      [4, 4, "step"],
      [5, 5, "idle"],
    ]);
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

  it("selects a bot from the steps table and shows its timeline over the set", () => {
    const { root, key, text, selected } = mount([]);
    key("ArrowRight");
    fireEvent.click(root.querySelectorAll(".ucbc-step")[0]!);
    expect(selected()).toBe(0);
    // The timeline takes the table's place until the selection is cleared.
    expect(root.querySelector(".ucbc-steps")).toBeNull();
    expect(text(".ucbc-bot-header")).toBe("Bot 0 · alpha×");
    const entries = () => [...root.querySelectorAll(".ucbc-entry")].map((e) => [e.className, e.textContent]);
    expect(entries()).toEqual([
      ["ucbc-entry ucbc-current", "tick 1addstdouthello"],
      ["ucbc-entry ucbc-future", "tick 2addstdouthello"],
    ]);
    // Each action is a node with its type, then its parameters; stdout is a node after them.
    const nodes = [...root.querySelectorAll(".ucbc-entry")[0]!.querySelectorAll(".ucbc-node")];
    expect(nodes.map((n) => n.className)).toEqual(["ucbc-node ucbc-node-action", "ucbc-node ucbc-node-stdout"]);
    expect(nodes[0]!.querySelector(".ucbc-node-type")?.textContent).toBe("add");
    expect(nodes[1]!.querySelector(".ucbc-stdout")?.textContent).toBe("hello");
    // Clicking an entry, anywhere on it, seeks to its tick.
    fireEvent.click(root.querySelectorAll(".ucbc-entry")[1]!.querySelector(".ucbc-node")!);
    expect(text(".ucbc-position")).toBe("tick 2 / 2");
    expect(entries().map(([c]) => c)).toEqual(["ucbc-entry", "ucbc-entry ucbc-current"]);
    fireEvent.click(root.querySelectorAll(".ucbc-link")[0]!);
    expect(text(".ucbc-position")).toBe("tick 1 / 2");
    key("Escape");
    expect(selected()).toBeNull();
    expect(root.querySelector(".ucbc-bot")).toBeNull();
    expect(root.querySelectorAll(".ucbc-step")).toHaveLength(2);
  });

  it("selects from the board, and a new set clears the selection", () => {
    const { root, select, selected, text } = mount([]);
    select(1);
    expect(selected()).toBe(1);
    expect(text(".ucbc-bot-header")).toBe("Bot 1 · beta×");
    expect(text(".ucbc-node-error .ucbc-node-type")).toBe("Exception");
    expect(text(".ucbc-entry .ucbc-error")).toBe("boom");
    expect(text(".ucbc-entry .ucbc-traceback")).toBe("Traceback");
    fireEvent.click(root.querySelectorAll(".ucbc-sets button")[1]!);
    expect(selected()).toBeNull();
    select(5);
    expect(text(".ucbc-bot")).toBe("Bot 5×Bot 5 did not play in set 2.");
  });

  it("shows an action's parameters as chips and collapses idle ticks", () => {
    const idle = { bot: 0, team: 0, actions: [], usage: { time_us: 0 } };
    const ticks = [
      { number: 0, state_after: 1, steps: [idle] },
      { number: 1, state_after: 2, steps: [idle] },
      { number: 2, state_after: 3, steps: [{ ...idle, actions: [{ type: "move", x: 1, y: "b" }] }] },
    ];
    const sets = replay.sets.map((s) => ({ ...s, ticks }));
    const { container } = render(<Viewer renderers={[]} replay={{ ...replay, sets }} />);
    const root = container.querySelector<HTMLElement>(".ucbc-viewer")!;
    fireEvent.keyDown(root, { key: "ArrowRight" });
    fireEvent.click(root.querySelector(".ucbc-step")!);
    const entries = [...root.querySelectorAll(".ucbc-entry")];
    expect(entries.map((e) => [e.className, e.querySelector(".ucbc-link")?.textContent])).toEqual([
      ["ucbc-entry ucbc-current", "ticks 1–2"],
      ["ucbc-entry ucbc-future", "tick 3"],
    ]);
    // The idle run is current, so its node says which of its ticks the frame is.
    expect(entries[0]!.querySelector(".ucbc-node")?.textContent).toBe("idletick 1");
    const params = [...entries[1]!.querySelectorAll(".ucbc-param")].map((p) => p.textContent);
    expect(params).toEqual(["x=1", "y=b"]);
    // The current tick is the one the frame is in, idle run included.
    fireEvent.keyDown(root, { key: "ArrowRight" });
    expect(root.querySelector(".ucbc-current .ucbc-link")?.textContent).toBe("ticks 1–2");
    expect(root.querySelector(".ucbc-current .ucbc-node")?.textContent).toBe("idletick 2");
    fireEvent.keyDown(root, { key: "ArrowRight" });
    expect(root.querySelector(".ucbc-current .ucbc-link")?.textContent).toBe("tick 3");
    expect(root.querySelector(".ucbc-future .ucbc-node")).toBeNull();
    expect(root.querySelector(".ucbc-entry .ucbc-node")?.textContent).toBe("idle");
  });

  it("searches a bot's timeline and jumps between matches", () => {
    const base = { bot: 0, team: 0, actions: [], usage: { time_us: 0 } };
    const ticks = [
      { number: 0, state_after: 1, steps: [{ ...base, actions: [{ type: "move", x: 1 }] }] },
      { number: 1, state_after: 2, steps: [{ ...base, actions: [{ type: "grab", x: 2 }], stdout: "got it" }] },
      { number: 2, state_after: 3, steps: [{ ...base, actions: [{ type: "move", x: 2 }, { type: "drop", x: 3 }] }] },
      { number: 3, state_after: 4, steps: [{ ...base, stdout: "GRAB failed" }] },
    ];
    const sets = replay.sets.map((s) => ({ ...s, ticks }));
    const { container } = render(<Viewer renderers={[]} replay={{ ...replay, sets }} />);
    const root = container.querySelector<HTMLElement>(".ucbc-viewer")!;
    fireEvent.keyDown(root, { key: "ArrowRight" });
    fireEvent.click(root.querySelector(".ucbc-step")!);
    const input = root.querySelector<HTMLInputElement>(".ucbc-search-input")!;
    const links = () => [...root.querySelectorAll(".ucbc-entry .ucbc-link")].map((l) => l.textContent);
    expect(links()).toEqual(["tick 1", "tick 2", "tick 3", "tick 4"]);

    // Matches actions, parameters and stdout, ignoring case; the matching nodes are marked.
    fireEvent.change(input, { target: { value: "grab" } });
    expect(links()).toEqual(["tick 2", "tick 4"]);
    expect(root.querySelector(".ucbc-search-count")?.textContent).toBe("2 matches");
    expect([...root.querySelectorAll(".ucbc-match .ucbc-node-type")].map((n) => n.textContent)).toEqual(["grab", "stdout"]);
    // The type is marked as the hit; so is a parameter a key=value word asks for.
    expect(root.querySelector(".ucbc-hit")?.textContent).toBe("grab");
    fireEvent.change(input, { target: { value: "x=2" } });
    expect(links()).toEqual(["tick 2", "tick 3"]);
    expect([...root.querySelectorAll(".ucbc-hit")].map((n) => n.textContent)).toEqual(["x=2", "x=2"]);
    expect([...root.querySelectorAll(".ucbc-node-action")].map((n) => n.classList.contains("ucbc-match"))).toEqual([
      true,
      true,
      false,
    ]);

    // Enter seeks to the next match after the frame, wrapping; Shift+Enter to the previous.
    const position = () => root.querySelector(".ucbc-position")?.textContent;
    expect(position()).toBe("tick 1 / 4");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(position()).toBe("tick 2 / 4");
    expect(root.querySelector(".ucbc-search-count")?.textContent).toBe("1 / 2");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(position()).toBe("tick 3 / 4");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(position()).toBe("tick 2 / 4");
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(position()).toBe("tick 3 / 4");

    // Typing in the box does not drive playback; Escape clears the search, then the selection.
    fireEvent.keyDown(input, { key: " " });
    fireEvent.keyDown(input, { key: "ArrowLeft" });
    expect(position()).toBe("tick 3 / 4");
    fireEvent.change(input, { target: { value: "nothing" } });
    expect(root.querySelector(".ucbc-bot > .ucbc-muted")?.textContent).toBe("No matches in set 1. Esc clears the search.");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(links()).toHaveLength(4);
    // Closing from the box hands focus back to the viewer, so its shortcuts keep working.
    fireEvent.keyDown(input, { key: "Escape" });
    expect(root.querySelector(".ucbc-bot")).toBeNull();
    expect(document.activeElement).toBe(root);
    fireEvent.keyDown(root, { key: "ArrowLeft" });
    expect(position()).toBe("tick 2 / 4");
  });

  it("focuses the search with / and follows playback until scrolled by hand", () => {
    const { root, key, select } = mount([]);
    select(0);
    key("/");
    const input = root.querySelector<HTMLInputElement>(".ucbc-search-input")!;
    expect(document.activeElement).toBe(input);
    const follow = () => [...root.querySelectorAll(".ucbc-bot-header button")].map((b) => b.textContent);
    expect(follow()).toEqual(["×"]);
    // A scroll the code did not ask for stops following; the button offers it back.
    const log = root.querySelector<HTMLOListElement>(".ucbc-bot-log")!;
    log.scrollTop = 40;
    fireEvent.scroll(log);
    expect(follow()).toEqual(["Follow", "×"]);
    fireEvent.click(root.querySelector(".ucbc-bot-header button")!);
    expect(follow()).toEqual(["×"]);
    log.scrollTop = 40;
    fireEvent.scroll(log);
    expect(follow()).toEqual(["Follow", "×"]);
    // Pressing play follows again; so does clicking a tick.
    key(" ");
    expect(follow()).toEqual(["×"]);
    key(" ");
    log.scrollTop = 40;
    fireEvent.scroll(log);
    fireEvent.click(root.querySelectorAll(".ucbc-entry")[1]!);
    expect(follow()).toEqual(["×"]);
  });
});
