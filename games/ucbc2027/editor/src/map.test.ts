import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { blank, decode, encode, labs, paint } from "./map.ts";
import { Environment, Item, type Map } from "./map_pb.ts";

const SYMBOL = { [Environment.EMPTY]: ".", [Environment.WALL]: "#", [Environment.LAB]: "L" };

/** The map as rows: `.` empty, `#` wall, `L` lab, `f` fossil. */
function rows(map: Map): string[] {
  const symbol = ({ environment, item }: Map["tiles"][number]) =>
    item === Item.FOSSIL ? "f" : SYMBOL[environment];
  return Array.from({ length: map.height }, (_, y) =>
    map.tiles.slice(y * map.width, (y + 1) * map.width).map(symbol).join(""),
  );
}

describe("paint", () => {
  it("paints one tile, or a 2x2 lab from its top-left", () => {
    const map = blank(4, 3);
    expect(paint(map, 0, 0, "wall", "none")).toEqual([0]);
    expect(paint(map, 2, 1, "lab", "none")).toEqual([6, 7, 10, 11]);
    paint(map, 1, 2, "fossil", "none");
    expect(rows(map)).toEqual(["#...", "..LL", ".fLL"]);
    paint(map, 0, 0, "empty", "none");
    paint(map, 1, 2, "wall", "none");
    expect(rows(map)).toEqual(["....", "..LL", ".#LL"]);
  });

  it("leaves a lab that would hang off the board unpainted", () => {
    const map = blank(3, 3);
    expect(paint(map, 2, 0, "lab", "none")).toEqual([]);
    expect(paint(map, 0, 2, "lab", "left-right")).toEqual([]);
    expect(rows(map)).toEqual(["...", "...", "..."]);
  });

  it("paints the mirrored tiles too", () => {
    const map = blank(5, 4);
    paint(map, 0, 1, "lab", "left-right");
    paint(map, 1, 0, "fossil", "left-right");
    paint(map, 2, 3, "wall", "left-right"); // on the centre line: one tile
    expect(rows(map)).toEqual([".f.f.", "LL.LL", "LL.LL", "..#.."]);
    paint(map, 2, 0, "wall", "top-bottom");
    expect(rows(map)).toEqual([".f#f.", "LL.LL", "LL.LL", "..#.."]);
  });
});

describe("encode and decode", () => {
  it("round-trip a map", () => {
    const map = blank(3, 2);
    paint(map, 0, 0, "lab", "none");
    paint(map, 2, 1, "fossil", "none");
    const back = decode(encode(map));
    expect([back.width, back.height]).toEqual([3, 2]);
    expect(rows(back)).toEqual(["LL.", "LLf"]);
  });

  it("read the standard map the engine ships", () => {
    const map = decode(readFileSync(new URL("../../maps/standard.map", import.meta.url)));
    expect([map.width, map.height]).toEqual([16, 16]);
    expect(rows(map)[7]).toBe(".LL...f..f...LL.");
    expect(rows(map).join("").split("f")).toHaveLength(7);
  });
});

describe("labs", () => {
  it("reads labs as the engine does, leaving a broken square's tiles out", () => {
    const map = blank(5, 4);
    paint(map, 3, 0, "lab", "none");
    paint(map, 0, 1, "lab", "none");
    paint(map, 4, 3, "wall", "none");
    map.tiles[3 * 5 + 4]!.environment = Environment.LAB;
    expect(rows(map)).toEqual(["...LL", "LL.LL", "LL...", "....L"]);
    const at = labs(map);
    const short = (i: number) => {
      const lab = at[i];
      return lab ? `${lab.team} ${lab.quarter}` : undefined;
    };
    expect([3, 4, 8, 9].map(short)).toEqual(["0 top-left", "0 top-right", "0 bottom-left", "0 bottom-right"]);
    expect([5, 6, 10, 11].map(short)).toEqual(["1 top-left", "1 top-right", "1 bottom-left", "1 bottom-right"]);
    expect(short(19)).toBeUndefined();
    expect(at.filter(Boolean)).toHaveLength(8);
  });
});
