// The game renderers a build bundles. `src/games.ts` is replaced at build time with an
// import of the renderer of each game ucbc-games builds by default.

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const PREFIX = "@ucbc/viewer-";
const GAMES = new URL("../../games/", import.meta.url);
const FEATURES = new URL("../../ucbc-games/Cargo.toml", import.meta.url);
const TARGET = fileURLToPath(new URL("../src/games.ts", import.meta.url));

/** The games in `default` in ucbc-games' Cargo.toml. */
export function defaultGames(manifest: string): string[] {
  const list = manifest.match(/^default = \[(.*)\]$/m)?.[1];
  if (list === undefined) throw new Error("no `default` feature in ucbc-games/Cargo.toml");
  return [...list.matchAll(/"([^"]+)"/g)].map(([, game]) => game as string);
}

/** Every default game with a renderer: `games/<game>/viewer`, the package `@ucbc/viewer-<game>`. */
export function gamePackages(): string[] {
  return defaultGames(readFileSync(FEATURES, "utf8"))
    .filter((game) => existsSync(new URL(`${game}/viewer/package.json`, GAMES)))
    .sort();
}

/** The module: each game's `renderer` export, in order. */
export function gamesSource(games: string[]): string {
  const imports = games.map((game, i) => `import { renderer as g${i} } from "${PREFIX}${game}";`);
  const names = games.map((_, i) => `g${i}`);
  return `${imports.join("\n")}\nexport const renderers = [${names.join(", ")}];\n`;
}

/** Bundles the default games' renderers. */
export function games(): Plugin {
  const list = gamePackages();
  return {
    name: "ucbc-games",
    enforce: "pre",
    load: (id) => (id === TARGET ? gamesSource(list) : null),
    // The dev server watches its own root; this adds the renderers, so a changed picture
    // in a game's assets/ reaches an open page.
    configureServer: (server) => {
      server.watcher.add(list.map((game) => fileURLToPath(new URL(`${game}/viewer`, GAMES))));
    },
  };
}
