import { expect, it } from "vitest";

import { defaultGames, gamePackages, gamesSource, selectedGames } from "./games.ts";

it("reads the default games from ucbc-games' features", () => {
  expect(defaultGames('[features]\ndefault = ["foo", "bar"]\nall = ["foo", "bar", "baz"]\n')).toEqual([
    "foo",
    "bar",
  ]);
});

it("selects the default games unless UCBC_GAMES lists some", () => {
  expect(selectedGames(undefined)).toEqual(["ucbc2027"]);
  expect(selectedGames(" ")).toEqual(["ucbc2027"]);
  expect(selectedGames("ucbc2027 tictactoe,foo")).toEqual(["ucbc2027", "tictactoe", "foo"]);
});

it("finds the renderer of each selected game under games/", () => {
  expect(gamePackages(["ucbc2027"])).toEqual(["ucbc2027"]);
  // No renderer, no package.
  expect(gamePackages(["ucbc2027", "nonexistent"])).toEqual(["ucbc2027"]);
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
