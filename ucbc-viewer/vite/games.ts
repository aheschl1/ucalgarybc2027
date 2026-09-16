// Which game renderers a build bundles. `src/games.ts` is replaced at build time with an
// import of every `@ucbc/viewer-<game>` this package depends on, or of the one game asked for.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Plugin } from "vite";

const PREFIX = "@ucbc/viewer-";
const MANIFEST = new URL("../package.json", import.meta.url);
const TARGET = fileURLToPath(new URL("../src/games.ts", import.meta.url));

/** Every game with a renderer: the `@ucbc/viewer-<game>` dependencies of the viewer. */
export function gamePackages(): string[] {
  const { dependencies = {} } = JSON.parse(readFileSync(MANIFEST, "utf8"));
  return Object.keys(dependencies)
    .filter((name) => name.startsWith(PREFIX))
    .map((name) => name.slice(PREFIX.length));
}

/** The module: each game's `renderer` export, in order. */
export function gamesSource(games: string[]): string {
  const imports = games.map((game, i) => `import { renderer as g${i} } from "${PREFIX}${game}";`);
  const names = games.map((_, i) => `g${i}`);
  return `${imports.join("\n")}\nexport const renderers = [${names.join(", ")}];\n`;
}

/** Bundles every game, or `only`, which must be one of them. */
export function games(only?: string): Plugin {
  const all = gamePackages();
  if (only !== undefined && !all.includes(only)) {
    throw new Error(`no viewer for game "${only}"; games with one: ${all.join(", ")}`);
  }
  const list = only === undefined ? all : [only];
  return {
    name: "ucbc-games",
    enforce: "pre",
    load: (id) => (id === TARGET ? gamesSource(list) : null),
  };
}
