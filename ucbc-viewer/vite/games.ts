// The game renderers a build bundles. `src/games.ts` is replaced at build time with an
// import of every game's renderer.

import { existsSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const PREFIX = "@ucbc/viewer-";
const GAMES = new URL("../../games/", import.meta.url);
const TARGET = fileURLToPath(new URL("../src/games.ts", import.meta.url));

/** Every game with a renderer: `games/<game>/viewer`, the package `@ucbc/viewer-<game>`. */
export function gamePackages(): string[] {
  return readdirSync(GAMES)
    .filter((game) => existsSync(new URL(`${game}/viewer/package.json`, GAMES)))
    .sort();
}

/** The module: each game's `renderer` export, in order. */
export function gamesSource(games: string[]): string {
  const imports = games.map((game, i) => `import { renderer as g${i} } from "${PREFIX}${game}";`);
  const names = games.map((_, i) => `g${i}`);
  return `${imports.join("\n")}\nexport const renderers = [${names.join(", ")}];\n`;
}

/** Bundles every game's renderer. */
export function games(): Plugin {
  const list = gamePackages();
  return {
    name: "ucbc-games",
    enforce: "pre",
    load: (id) => (id === TARGET ? gamesSource(list) : null),
  };
}
