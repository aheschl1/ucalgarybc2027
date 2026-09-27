// Which game renderers a build bundles. `src/games.ts` is replaced at build time with an
// import of every game's renderer, or of the games UCBC_GAMES names.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const PREFIX = "@ucbc/viewer-";
const GAMES = new URL("../../games/", import.meta.url);
const TARGET = fileURLToPath(new URL("../src/games.ts", import.meta.url));

/** Every game with a renderer: the `@ucbc/viewer-<game>` package in `games/*\/viewer`. */
export function gamePackages(): string[] {
  return readdirSync(GAMES)
    .map((dir) => new URL(`${dir}/viewer/package.json`, GAMES))
    .filter((manifest) => existsSync(manifest))
    .map((manifest) => JSON.parse(readFileSync(manifest, "utf8")).name as string)
    .filter((name) => name.startsWith(PREFIX))
    .map((name) => name.slice(PREFIX.length))
    .sort();
}

/** The module: each game's `renderer` export, in order. */
export function gamesSource(games: string[]): string {
  const imports = games.map((game, i) => `import { renderer as g${i} } from "${PREFIX}${game}";`);
  const names = games.map((_, i) => `g${i}`);
  return `${imports.join("\n")}\nexport const renderers = [${names.join(", ")}];\n`;
}

/** Bundles the games `selection` names, space-separated; every game when it is unset or
 * `all`. Each named game must have a renderer. */
export function games(selection?: string): Plugin {
  const all = gamePackages();
  const named = selection?.split(/\s+/).filter(Boolean) ?? [];
  const list = named.length === 0 || named.includes("all") ? all : named;
  const missing = list.filter((game) => !all.includes(game));
  if (missing.length > 0) {
    throw new Error(`no viewer for game "${missing.join('", "')}"; games with one: ${all.join(", ")}`);
  }
  return {
    name: "ucbc-games",
    enforce: "pre",
    load: (id) => (id === TARGET ? gamesSource(list) : null),
  };
}
