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
