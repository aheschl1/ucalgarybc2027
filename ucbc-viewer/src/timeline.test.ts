import { describe, expect, it } from "vitest";

import type { SetReplay, Tick } from "./replay.gen.ts";
import { frameAt, frameCount } from "./timeline.ts";

const tick = (number: number): Tick => ({ number, steps: [], state_after: { after: number } });

const set: SetReplay = {
  index: 0,
  first_team: 0,
  initial_state: { initial: true },
  ticks: [tick(0), tick(1)],
  result: { index: 0, first_team: 0, winner_team: null, reason: "draw", ticks: 2 },
};

describe("timeline", () => {
  it("has the initial state plus one frame per tick", () => {
    expect(frameCount(set)).toBe(3);
  });

  it("maps frame 0 to the initial state", () => {
    const f = frameAt(set, [], 0);
    expect(f.state).toEqual({ initial: true });
    expect(f.tick).toBeNull();
  });

  it("maps frame n to the state after tick n - 1", () => {
    const f = frameAt(set, [], 2);
    expect(f.state).toEqual({ after: 1 });
    expect(f.tick?.number).toBe(1);
  });

  it("clamps out-of-range frames", () => {
    expect(frameAt(set, [], -5).tick).toBeNull();
    expect(frameAt(set, [], 99).tick?.number).toBe(1);
  });
});
