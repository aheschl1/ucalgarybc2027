import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

import { games } from "./vite/games.ts";
import { replays } from "./vite/replays.ts";

export default defineConfig({
  // Relative asset paths, so the built page works under any URL prefix.
  base: "./",
  plugins: [react(), games(), replays()],
});
