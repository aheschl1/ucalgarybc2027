// @vitest-environment jsdom
/// <reference types="node" />

import { cleanup, render } from "@testing-library/react";
import { frameAt, frameCount, ViewerContext, type Frame, type Replay } from "@ucbc/viewer";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { gunzipSync } from "node:zlib";
import { afterEach, expect, it } from "vitest";

import { renderer } from "./index.tsx";

const { Board, Info } = renderer;
const DIR = resolve(import.meta.dirname, "../replays");
const samples = readdirSync(DIR).filter((f) => f.endsWith(".json.gz"));
afterEach(cleanup);

function both(frame: Frame) {
  return (
    <ViewerContext value={{ frame, selected: null, select: () => {} }}>
      <Board />
      <Info />
    </ViewerContext>
  );
}

it("has sample replays", () => {
  expect(samples).not.toHaveLength(0);
});

// The samples are what `make replays` plays now (tests/test_viewer.py), so this is the
// renderer against the engine's real output.
it.each(samples)("draws every frame of %s", (file) => {
  const replay: Replay = JSON.parse(gunzipSync(readFileSync(resolve(DIR, file))).toString());
  for (const set of replay.sets) {
    const view = render(both(frameAt(set, replay.teams, 0)));
    for (let i = 1; i < frameCount(set); i++) view.rerender(both(frameAt(set, replay.teams, i)));
    expect(view.container.querySelectorAll(".u27-bot").length).toBeGreaterThan(0);
  }
});
