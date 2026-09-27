import { defineConfig } from "vite";

import { games } from "./vite/games.ts";

export default defineConfig({
  // Relative asset paths, so the built page works under any URL prefix.
  base: "./",
  // `UCBC_GAMES` (from `make ... GAMES=`) picks the bundled renderers.
  plugins: [games(process.env.UCBC_GAMES)],
});
