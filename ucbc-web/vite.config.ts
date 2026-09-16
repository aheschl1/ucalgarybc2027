import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

import { games } from "../ucbc-viewer/vite/games.ts";

// The app calls /api/*; this forwards it to the API so the browser stays same-origin.
// `UCBC_GAME` (from `make ... GAME=`) narrows the viewer's bundled renderers to one game.
export default defineConfig({
  plugins: [react(), games(process.env.UCBC_GAME || undefined)],
  server: {
    proxy: {
      "/api": {
        target: process.env.UCBC_API_PROXY ?? "http://127.0.0.1:8000",
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
});
