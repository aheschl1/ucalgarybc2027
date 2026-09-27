import { expect, it } from "vitest";

import { gamePackages, games, gamesSource } from "./games.ts";

it("finds the renderer of each game under games/", () => {
  expect(gamePackages()).toContain("tictactoe");
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

it("rejects a game without a renderer", () => {
  expect(() => games("nope")).toThrow('no viewer for game "nope"');
  expect(() => games("tictactoe nope")).toThrow('no viewer for game "nope"');
  expect(games("tictactoe").name).toBe("ucbc-games");
  expect(games("all").name).toBe("ucbc-games");
});
