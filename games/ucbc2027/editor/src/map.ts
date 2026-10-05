// A map file being edited: the generated `Map` from map.proto, changed in place.

import { create, fromBinary, toBinary } from "@bufbuild/protobuf";

import { Environment, Item, type Map, MapSchema, TileSchema } from "./map_pb.ts";

/** What a click paints. A lab is a 2x2 stamp whose top-left is the clicked tile. */
export type Brush = "empty" | "wall" | "lab" | "fossil";

/** Paint the mirrored tile too: across the vertical centre line, or the horizontal one. */
export type Mirror = "none" | "left-right" | "top-bottom";

const PAINT: Record<Brush, [Environment, Item]> = {
  empty: [Environment.EMPTY, Item.NONE],
  wall: [Environment.WALL, Item.NONE],
  lab: [Environment.LAB, Item.NONE],
  fossil: [Environment.EMPTY, Item.FOSSIL],
};

/** A `width` by `height` map, every tile empty. */
export function blank(width: number, height: number): Map {
  const tiles = Array.from({ length: width * height }, () => create(TileSchema));
  return create(MapSchema, { width, height, tiles });
}

export function decode(bytes: Uint8Array): Map {
  return fromBinary(MapSchema, bytes);
}

export function encode(map: Map): Uint8Array {
  return toBinary(MapSchema, map);
}

/** Paints `brush` at (x, y), and at the mirrored tiles. Returns the indices of the tiles
 * painted; none if a lab stamp would hang off the board. */
export function paint(map: Map, x: number, y: number, brush: Brush, mirror: Mirror): number[] {
  const size = brush === "lab" ? 2 : 1;
  if (x + size > map.width || y + size > map.height) return [];
  const at: [number, number][] = [];
  for (let dy = 0; dy < size; dy++) {
    for (let dx = 0; dx < size; dx++) at.push([x + dx, y + dy]);
  }
  const flip = ([tx, ty]: [number, number]): [number, number] =>
    mirror === "left-right" ? [map.width - 1 - tx, ty] : [tx, map.height - 1 - ty];
  if (mirror !== "none") at.push(...at.map(flip));

  const [environment, item] = PAINT[brush];
  const painted = new Set(at.map(([tx, ty]) => ty * map.width + tx));
  for (const i of painted) {
    map.tiles[i] = create(TileSchema, { environment, item });
  }
  return [...painted];
}

/** Where a lab tile sits in its lab: its team, and its quarter of the 2x2 square. */
export interface LabTile {
  team: number;
  quarter: "top-left" | "top-right" | "bottom-left" | "bottom-right";
}

const QUARTERS: LabTile["quarter"][] = ["top-left", "top-right", "bottom-left", "bottom-right"];

/** Each lab tile's place in its lab by tile index, read as the engine does (`Map::new` in
 * games/ucbc2027/src/map.rs): in reading order, the first lab tile not yet in a lab is a
 * new lab's top-left, and the order gives its team. A lab tile left out of every lab, as
 * in a broken square, has none. */
export function labs(map: Map): (LabTile | undefined)[] {
  const isLab = (x: number, y: number) =>
    x < map.width && y < map.height && map.tiles[y * map.width + x]!.environment === Environment.LAB;
  const placed = Array.from<LabTile | undefined>({ length: map.tiles.length });
  let team = 0;
  map.tiles.forEach((_, i) => {
    const x = i % map.width;
    const y = Math.floor(i / map.width);
    if (!isLab(x, y) || placed[i]) return;
    const square = [i, i + 1, i + map.width, i + map.width + 1];
    const whole = isLab(x + 1, y) && isLab(x, y + 1) && isLab(x + 1, y + 1);
    if (!whole || square.some((j) => placed[j])) return;
    square.forEach((j, q) => (placed[j] = { team, quarter: QUARTERS[q]! }));
    team++;
  });
  return placed;
}
