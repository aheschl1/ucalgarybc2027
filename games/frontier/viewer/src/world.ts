// The set at a frame, rebuilt from the replay's deltas. Units, buildings, piles, crops,
// and signs restart from the nearest keyframe; tiles replay every change from the
// initial map. Playing forward applies one delta per frame.

import type { SetReplay } from "@ucbc/viewer";

import type { Building, Points, Snapshot } from "./api.gen.ts";

/** In id order, as observations and snapshots index them. */
export const BUILDINGS = [
  "base",
  "chest",
  "gate",
  "mech_drill",
  "monument",
  "radio_tower",
  "research_desk",
  "smelter",
  "spike_trap",
  "steam_drill",
  "stone_wall",
  "tower",
  "well",
  "wood_wall",
  "workbench",
] as const satisfies readonly Building[];

/** In id order, as content/crops.toml's crops sort. */
export const CROPS = ["apple_tree", "wheat"] as const;

// Every building is named: this fails to compile if one is missing.
const _all: Exclude<Building, (typeof BUILDINGS)[number]> extends never ? true : never = true;

export interface Unit {
  id: number;
  team: number;
  commander: boolean;
  x: number;
  y: number;
  hp: number;
  carried: number;
}

export interface Structure {
  id: number;
  team: number;
  building: Building;
  x: number;
  y: number;
  hp: number;
  workLeft: number;
}

export interface Crop {
  kind: (typeof CROPS)[number];
  ripeAt: number;
}

export interface World {
  size: number;
  vision: number;
  tick: number;
  /** One byte per tile, row by row. */
  tiles: Uint8Array;
  /** Tiles changed since the board last drew the terrain; null for all of them. */
  dirty: Set<number> | null;
  units: Map<number, Unit>;
  structures: Map<number, Structure>;
  /** Tiles with things on the ground, as `y * size + x`. */
  piles: Set<number>;
  /** Crops by tile, `y * size + x`. */
  crops: Map<number, Crop>;
  signs: Set<number>;
  /** Each team's points, as the tick limit would count them. */
  points: Points[];
}

/** `state` as a snapshot. Throws if it is not one, e.g. from an older engine. */
export function snapshot(state: unknown): Snapshot {
  const s = state as Snapshot;
  if (typeof s?.tick !== "number" || !Array.isArray(s.units) || !Array.isArray(s.tiles)) {
    throw new Error(`not a frontier state: ${JSON.stringify(state)}`);
  }
  return s;
}

function rows(flat: number[], width: number): number[][] {
  const out = [];
  for (let i = 0; i + width <= flat.length; i += width) out.push(flat.slice(i, i + width));
  return out;
}

/** Applies one snapshot's entity changes to `w`; a keyframe replaces them all. */
function applyEntities(w: World, s: Snapshot) {
  if (s.keyframe) {
    w.units.clear();
    w.structures.clear();
    w.piles.clear();
    w.crops.clear();
    w.signs.clear();
  }
  for (const [id, team, kind, x, y, hp, carried] of rows(s.units, 7)) {
    w.units.set(id!, { id: id!, team: team!, commander: kind === 0, x: x!, y: y!, hp: hp!, carried: carried! });
  }
  for (const id of s.removed) w.units.delete(id);
  for (const [id, team, b, x, y, hp, workLeft] of rows(s.buildings, 7)) {
    const building = BUILDINGS[b!];
    if (!building) throw new Error(`unknown building ${b}`);
    w.structures.set(id!, { id: id!, team: team!, building, x: x!, y: y!, hp: hp!, workLeft: workLeft! });
  }
  for (const id of s.demolished) w.structures.delete(id);
  // A tile's pile rows replace what was there; a count of 0 means nothing is.
  const piled = new Map<number, number>();
  for (const [x, y, , count] of rows(s.piles, 4)) {
    const at = y! * w.size + x!;
    piled.set(at, (piled.get(at) ?? 0) + count!);
  }
  for (const [at, count] of piled) (count > 0 ? w.piles.add(at) : w.piles.delete(at));
  for (const [x, y, crop, ripeAt] of rows(s.crops, 4)) {
    const kind = CROPS[crop!];
    if (!kind) throw new Error(`unknown crop ${crop}`);
    w.crops.set(y! * w.size + x!, { kind, ripeAt: ripeAt! });
  }
  for (const [x, y] of rows(s.cleared, 2)) w.crops.delete(y! * w.size + x!);
  for (const { pos, data } of s.signs) {
    const at = pos.y * w.size + pos.x;
    if (data) w.signs.add(at);
    else w.signs.delete(at);
  }
  w.tick = s.tick;
  w.points = s.points;
}

function applyTiles(w: World, s: Snapshot) {
  for (const [x, y, tile] of rows(s.tiles, 3)) {
    w.tiles[y! * w.size + x!] = tile!;
    w.dirty?.add(y! * w.size + x!);
  }
}

/** A set's snapshots in order, the initial state first, and the world at any of them. */
export class Timeline {
  private readonly snapshots: Snapshot[];
  private readonly keyframes: number[] = [];
  private readonly initialTiles: Uint8Array;
  private readonly world: World;
  /** The index `world`'s entities and tiles are at. */
  private at = 0;

  constructor(set: SetReplay) {
    this.snapshots = [set.initial_state, ...set.ticks.map((t) => t.state_after)].map(snapshot);
    const map = this.snapshots[0]!.map;
    if (!map) throw new Error("the initial frontier state has no map");
    this.initialTiles = Uint8Array.from(atob(map.tiles), (c) => c.charCodeAt(0));
    this.snapshots.forEach((s, i) => s.keyframe && this.keyframes.push(i));
    this.world = {
      size: map.size,
      vision: map.vision,
      tick: 0,
      tiles: this.initialTiles.slice(),
      dirty: null,
      units: new Map(),
      structures: new Map(),
      piles: new Set(),
      crops: new Map(),
      signs: new Set(),
      points: [],
    };
    applyEntities(this.world, this.snapshots[0]!);
  }

  get length(): number {
    return this.snapshots.length;
  }

  private sampled?: History;

  /** Every team's points over the set, at up to `SAMPLES` frames from first to last. */
  history(): History {
    if (!this.sampled) {
      const picked = sample(this.snapshots.length, SAMPLES).map((i) => this.snapshots[i]!);
      const first = this.snapshots[0]!.units;
      const commanders = new Map<number, number>();
      for (let i = 0; i < first.length; i += 7) if (first[i + 2] === 0) commanders.set(first[i]!, first[i + 1]!);
      this.sampled = {
        ticks: picked.map((s) => s.tick),
        points: [0, 1].map((t) => picked.map((s) => s.points[t] ?? { buildings: 0, stored: 0, units: 0 })),
        fallen: this.snapshots.flatMap((s) =>
          s.removed.filter((id) => commanders.has(id)).map((id) => ({ team: commanders.get(id)!, tick: s.tick })),
        ),
      };
    }
    return this.sampled;
  }

  /** The world after the `index`th snapshot. Valid until the next call. */
  worldAt(index: number): World {
    const target = Math.max(0, Math.min(index, this.snapshots.length - 1));
    const w = this.world;
    if (target === this.at) return w;
    if (target < this.at) {
      w.tiles.set(this.initialTiles);
      w.dirty = null;
      for (let i = 1; i <= target; i++) applyTiles(w, this.snapshots[i]!);
    } else {
      for (let i = this.at + 1; i <= target; i++) applyTiles(w, this.snapshots[i]!);
    }
    // From the last keyframe at or before the target, unless going forward past none.
    let keyframe = 0;
    for (const k of this.keyframes) if (k <= target) keyframe = k;
    const from = target > this.at && keyframe <= this.at ? this.at + 1 : keyframe;
    for (let i = from; i <= target; i++) applyEntities(w, this.snapshots[i]!);
    this.at = target;
    return w;
  }
}

const SAMPLES = 400;

/** `samples` indices spread evenly over `length` items, the first and last included. */
export function sample(length: number, samples: number): number[] {
  if (length <= samples) return Array.from({ length }, (_, i) => i);
  return Array.from({ length: samples }, (_, i) => Math.round((i * (length - 1)) / (samples - 1)));
}

export interface History {
  ticks: number[];
  /** By team, then by sample. */
  points: Points[][];
  /** Commanders that died, and the tick. */
  fallen: { team: number; tick: number }[];
}

const timelines = new WeakMap<SetReplay, Timeline>();

/** The set's timeline, built once. */
export function timeline(set: SetReplay): Timeline {
  let t = timelines.get(set);
  if (!t) timelines.set(set, (t = new Timeline(set)));
  return t;
}
