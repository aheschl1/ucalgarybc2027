import { strToU8, zipSync } from "fflate";
import { expect, it } from "vitest";

import { fromZip, isPng, matchIdFrom, toDataUrl, toZip } from "./skinFiles";

const png = (...rest: number[]) => new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, ...rest]);

it("tells a PNG from anything else", () => {
  expect(isPng(png(1))).toBe(true);
  expect(isPng(strToU8("GIF89a"))).toBe(false);
  expect(isPng(new Uint8Array())).toBe(false);
});

it("reads back the zip it writes, at the same paths", () => {
  const overrides = { "map/environment/dirt.png": toDataUrl(png(1, 2, 255)), "dino/dino_a/a01.png": toDataUrl(png(0)) };
  const known = new Set(Object.keys(overrides));
  expect(fromZip(toZip(overrides), known)).toEqual({ found: overrides, skipped: [] });
});

it("skips what isn't a known PNG in a zip", () => {
  const zip = zipSync({
    "wall.png": png(1),
    "stranger.png": png(2),
    "fossil.png": strToU8("not a png"),
    __MACOSX: { "wall.png": png(3) },
  });
  const { found, skipped } = fromZip(zip, new Set(["wall.png", "fossil.png"]));
  expect(Object.keys(found)).toEqual(["wall.png"]);
  expect(skipped.sort()).toEqual(["__MACOSX/wall.png", "fossil.png", "stranger.png"]);
});

it("throws on a file that isn't a zip", () => {
  expect(() => fromZip(strToU8("hello"), new Set())).toThrow();
});

it("finds the match ID in a pasted ID or link", () => {
  const id = "0d9e4c2a-7b1f-4e6a-9c3d-5f8a2b1c0e7d";
  expect(matchIdFrom(`  ${id}\n`)).toBe(id);
  expect(matchIdFrom(`https://site/platform/matches/${id.toUpperCase()}?x=1`)).toBe(id);
  expect(matchIdFrom("not a match")).toBeNull();
});
