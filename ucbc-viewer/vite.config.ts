import { defineConfig } from "vite";

import { games } from "./vite/games.ts";

export default defineConfig({
  // Relative asset paths, so the built page works under any URL prefix.
  base: "./",
  // `UCBC_GAME` (from `make ... GAME=`) narrows the bundled renderers to one game.
  plugins: [games(process.env.UCBC_GAME || undefined)],
});
