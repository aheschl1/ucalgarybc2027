// The committed sample replays, served by the dev server (`npm run dev`) so the page can
// list and open them: the list at `/replays/`, each at `/replays/<game>/<file>`. Builds
// leave them out. `make replays` writes them.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import type { Plugin } from "vite";

const GAMES = new URL("../../games/", import.meta.url);

/** Each sample replay as `<game>/<file>`, from `games/<game>/viewer/replays/*.json.gz`. */
export function sampleReplays(): string[] {
  return readdirSync(GAMES)
    .flatMap((game) => {
      const dir = new URL(`${game}/viewer/replays/`, GAMES);
      if (!existsSync(dir)) return [];
      return readdirSync(dir)
        .filter((file) => file.endsWith(".json.gz"))
        .map((file) => `${game}/${file}`);
    })
    .sort();
}

/** Serves the sample replays gzipped as they are, which the browser decompresses. */
export function replays(): Plugin {
  return {
    name: "ucbc-replays",
    apply: "serve",
    configureServer(server) {
      // Only `ucbc view` has a replay of its own; without this the page would read the
      // dev server's HTML fallback as one, instead of listing the samples.
      server.middlewares.use("/replay.json", (_req, res) => {
        res.statusCode = 404;
        res.end();
      });
      server.middlewares.use("/replays", (req, res, next) => {
        const path = decodeURIComponent((req.url ?? "/").split("?")[0]!).replace(/^\//, "");
        const list = sampleReplays();
        // Only listed files, so a path cannot reach outside the replay folders.
        if (path !== "" && !list.includes(path)) return next();
        res.setHeader("Content-Type", "application/json");
        if (path === "") return res.end(JSON.stringify(list));
        const [game, file] = path.split("/");
        res.setHeader("Content-Encoding", "gzip");
        res.end(readFileSync(new URL(`${game}/viewer/replays/${file}`, GAMES)));
      });
    },
  };
}
