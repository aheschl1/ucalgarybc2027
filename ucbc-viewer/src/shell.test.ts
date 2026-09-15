// @vitest-environment jsdom

import { describe, expect, it } from "vitest";

import type { Frame, GameRenderer } from "./renderer.ts";
import type { Replay } from "./replay.gen.ts";
import { createViewer } from "./shell.ts";

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

function mount(renderers: GameRenderer[]) {
  const el = document.createElement("div");
  document.body.append(el);
  const frames: Frame[] = [];
  const counter: GameRenderer = {
    game: "counter",
    mount: () => ({ draw: (f) => frames.push(f), destroy: () => {} }),
  };
  const viewer = createViewer(el, { renderers: [...renderers, counter], replay });
  const root = el.querySelector<HTMLElement>(".ucbc-viewer")!;
  const key = (k: string) => root.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true }));
  const text = (selector: string) => root.querySelector(selector)?.textContent;
  return { el, root, viewer, frames, key, text };
}

describe("createViewer", () => {
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
    expect(steps[0]!.textContent).toContain('{"type":"add"}');
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
    root.querySelectorAll<HTMLButtonElement>(".ucbc-sets button")[1]!.click();
    expect(frames.at(-1)?.set.index).toBe(1);
    expect(frames.at(-1)?.tick).toBeNull();
    expect(text(".ucbc-set-result")).toBe("Set 2: beta wins by win after 2 ticks");
  });

  it("shows a renderer's error instead of a silently wrong board", () => {
    const picky: GameRenderer = {
      game: "counter",
      mount: () => ({
        draw: (f) => {
          if (f.state === 2) throw new Error("bad state");
        },
        destroy: () => {},
      }),
    };
    const { root, key } = mount([picky]);
    const error = root.querySelector<HTMLElement>(".ucbc-draw-error")!;
    expect(error.hidden).toBe(true);
    key("End");
    expect(error.hidden).toBe(false);
    expect(error.textContent).toBe("Cannot draw this frame: bad state");
    key("Home");
    expect(error.hidden).toBe(true);
  });

  it("falls back to raw JSON for an unknown game", () => {
    const { el, viewer, text } = mount([]);
    viewer.load({ ...replay, config: { ...replay.config, game: "chess" } });
    expect(text(".ucbc-stage .ucbc-error")).toBe('No renderer for game "chess".');
    expect(text(".ucbc-raw")).toBe("0");
    viewer.destroy();
    expect(el.children).toHaveLength(0);
  });
});
