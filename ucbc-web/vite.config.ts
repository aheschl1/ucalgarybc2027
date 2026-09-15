import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The app calls /api/*; this forwards it to the API so the browser stays same-origin.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: process.env.UCBC_API_PROXY ?? "http://127.0.0.1:8000",
        rewrite: (path) => path.replace(/^\/api/, ""),
        configure: (proxy) => {
          // Without this a 401 opens the browser's own Basic auth dialog.
          proxy.on("proxyRes", (res) => {
            delete res.headers["www-authenticate"];
          });
        },
      },
    },
  },
});
