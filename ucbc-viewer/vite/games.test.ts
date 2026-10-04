import { expect, it } from "vitest";

import { defaultGames, gamePackages, gamesSource } from "./games.ts";

it("reads the default games from ucbc-games' features", () => {
  expect(defaultGames('[features]\ndefault = ["foo", "bar"]\nall = ["foo", "bar", "baz"]\n')).toEqual([
    "foo",
    "bar",
  ]);
});

it("finds the renderer of each default game under games/", () => {
  expect(gamePackages()).toEqual(["ucbc2027"]);
});

it("imports each game's renderer", () => {
  expect(gamesSource(["tictactoe", "foo"])).toBe(
    [
      'import { renderer as g0 } from "@ucbc/viewer-tictactoe";',
      'import { renderer as g1 } from "@ucbc/viewer-foo";',
      "export const renderers = [g0, g1];",
      "",
    ].join("\n"),
  );
});
