// @ts-expect-error: the viewer has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

import type { SetReplay, Tick } from "@ucbc/viewer";
import { expect, it } from "vitest";

import type { Snapshot } from "./api.gen.ts";
import { BUILDINGS, CROPS, sample, Timeline, type World } from "./world.ts";

const SIZE = 4;
const ZERO = { buildings: 0, stored: 0, units: 0 };

function snap(tick: number, s: Partial<Snapshot> = {}): Snapshot {
  return {
    tick,
    keyframe: false,
    units: [],
    carrying: [],
    removed: [],
    tiles: [],
    buildings: [],
    stored: [],
    owed: [],
    jobs: [],
    demolished: [],
    piles: [],
    crops: [],
    cleared: [],
    signs: [],
    points: [ZERO, ZERO],
    ...s,
  };
}

function set(snapshots: Snapshot[]): SetReplay {
  const ticks: Tick[] = snapshots.slice(1).map((state_after, i) => ({ number: i, state_after, steps: [] }));
  return {
    index: 0,
    first_team: 0,
    initial_state: snapshots[0],
    ticks,
    result: { index: 0, first_team: 0, reason: "draw", ticks: ticks.length },
  };
}

// Six ticks on a 4x4 map: a unit walks right and leaves; forest tiles are gathered out;
// a pile of two kinds is taken from; a crop is planted and cleared; the base falls; a
// sign goes up and comes down; the base stores wheat, then starts a job. Tick 3 is a
// keyframe.
const replay = set([
  snap(0, {
    keyframe: true,
    map: { size: SIZE, vision: 2, tiles: btoa(String.fromCharCode(...new Uint8Array(SIZE * SIZE).fill(1))), storage: [] },
    units: [7, 0, 1, 0, 0, 100, 0],
    buildings: [3, 1, 0, 3, 3, 500, 0],
    stored: [3, 17, 40],
  }),
  snap(1, { units: [7, 0, 1, 1, 0, 100, 1], carrying: [7, 19, 1], tiles: [2, 2, 4], piles: [0, 3, 13, 2, 0, 3, 19, 1] }),
  snap(2, { piles: [0, 3, 19, 1], crops: [1, 1, 1, 50], buildings: [3, 1, 0, 3, 3, 500, 0], stored: [3, 17, 38], jobs: [3, 2, 60] }),
  snap(3, {
    keyframe: true,
    units: [7, 0, 1, 2, 0, 90, 1],
    carrying: [7, 19, 1],
    buildings: [3, 1, 0, 3, 3, 500, 0],
    stored: [3, 17, 38],
    jobs: [3, 2, 60],
    piles: [0, 3, 19, 1],
    crops: [1, 1, 1, 50],
  }),
  snap(4, { removed: [7], cleared: [1, 1], piles: [0, 3, 0, 0] }),
  snap(5, { demolished: [3], signs: [{ pos: { x: 1, y: 0 }, data: "aGk=" }] }),
  snap(6, { signs: [{ pos: { x: 1, y: 0 }, data: "" }], tiles: [0, 0, 4] }),
]);

/** What a world holds, comparable. */
function summary(w: World) {
  return JSON.stringify({
    tick: w.tick,
    tiles: [...w.tiles],
    units: [...w.units.values()],
    structures: [...w.structures.values()],
    piles: [...w.piles].sort(),
    crops: [...w.crops].sort(),
    signs: [...w.signs].sort(),
    carrying: [...w.carrying],
    contents: [...w.contents],
  });
}

it("plays forward one delta at a time", () => {
  const t = new Timeline(replay);
  let w = t.worldAt(0);
  expect(w.units.get(7)).toMatchObject({ x: 0, commander: false });
  expect(w.structures.get(3)?.building).toBe("base");
  expect(w.contents.get(3)).toEqual({ stored: [["wheat", 40]], owed: [], job: undefined });
  expect(w.carrying.get(7)).toEqual([]);
  w = t.worldAt(1);
  expect(w.units.get(7)).toMatchObject({ x: 1, carried: 1 });
  expect(w.carrying.get(7)).toEqual([["wood", 1]]);
  expect(w.tiles[2 * SIZE + 2]).toBe(4);
  expect([...w.piles]).toEqual([3 * SIZE]);
  w = t.worldAt(2);
  expect(w.crops.get(SIZE + 1)).toEqual({ kind: "wheat", ripeAt: 50 });
  expect(w.contents.get(3)).toEqual({ stored: [["wheat", 38]], owed: [], job: { recipe: "copper_ingot", doneAt: 60 } });
  w = t.worldAt(4);
  expect(w.units.size).toBe(0);
  expect(w.carrying.size).toBe(0);
  expect(w.crops.size).toBe(0);
  expect(w.piles.size).toBe(0);
  expect(w.structures.size).toBe(1);
});

it("seeks back and from keyframes to the same world", () => {
  const t = new Timeline(replay);
  t.worldAt(4);
  // Back before the tile changed: it's forest again.
  const w = t.worldAt(0);
  expect(w.tiles[2 * SIZE + 2]).toBe(1);
  expect(w.units.get(7)?.x).toBe(0);
  // A jump that crosses keyframe 3.
  expect(t.worldAt(3).units.get(7)).toMatchObject({ x: 2, hp: 90 });
  expect(t.worldAt(3).tiles[2 * SIZE + 2]).toBe(4);
});

it("reaches the same world from any frame as from the start", () => {
  const frames = replay.ticks.length + 1;
  const fresh = Array.from({ length: frames }, (_, i) => summary(new Timeline(replay).worldAt(i)));
  for (let from = 0; from < frames; from++) {
    for (let to = 0; to < frames; to++) {
      const t = new Timeline(replay);
      t.worldAt(from);
      expect(summary(t.worldAt(to)), `${from} -> ${to}`).toBe(fresh[to]);
    }
  }
});

it("takes buildings down, and signs up and down", () => {
  const t = new Timeline(replay);
  expect(t.worldAt(5).structures.size).toBe(0);
  expect([...t.worldAt(5).signs]).toEqual([1]);
  expect(t.worldAt(6).signs.size).toBe(0);
  expect(t.worldAt(6).tiles[0]).toBe(4);
});

it("tracks the tiles to redraw", () => {
  const t = new Timeline(replay);
  const w = t.worldAt(0);
  expect(w.dirty).toBeNull();
  w.dirty = new Set();
  t.worldAt(1);
  expect([...w.dirty]).toEqual([2 * SIZE + 2]);
  t.worldAt(0);
  expect(w.dirty).toBeNull();
});

it("samples evenly from the first frame to the last", () => {
  expect(sample(4, 400)).toEqual([0, 1, 2, 3]);
  expect(sample(8, 3)).toEqual([0, 4, 7]);
  expect(sample(10_001, 400)).toHaveLength(400);
});

it("keeps each team's points over the set", () => {
  const points = [
    { buildings: 1, stored: 2, units: 3 },
    { buildings: 4, stored: 5, units: 6 },
  ];
  const h = new Timeline(set([replay.initial_state as Snapshot, snap(1, { points })])).history();
  expect(h.ticks).toEqual([0, 1]);
  expect(h.points.map((team) => team[1])).toEqual(points);
});

it("finds when commanders fell", () => {
  const fell = snap(1, { removed: [7] });
  const replay2 = set([{ ...(replay.initial_state as Snapshot), units: [7, 1, 0, 0, 0, 100, 0] }, fell]);
  expect(new Timeline(replay2).history().fallen).toEqual([{ team: 1, tick: 1 }]);
});

it("names buildings in id order, which is alphabetical", () => {
  expect([...BUILDINGS]).toEqual([...BUILDINGS].sort());
});

it("rejects a state it does not recognise", () => {
  expect(() => new Timeline(set([{ tick: 0 } as unknown as Snapshot]))).toThrow("not a frontier state");
});

it("names crops in the order the engine numbers them, by their content keys", () => {
  const toml = readFileSync(new URL("../../content/crops.toml", import.meta.url), "utf8");
  const keys = [...toml.matchAll(/^\[(\w+)\]/gm)].map((m) => m[1]).sort();
  expect([...CROPS]).toEqual(keys);
});
