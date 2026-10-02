import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

import { games } from "./vite/games.ts";

export default defineConfig({
  // Relative asset paths, so the built page works under any URL prefix.
  base: "./",
  plugins: [react(), games()],
});
