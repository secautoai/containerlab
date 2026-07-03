import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// ClabStudio frontend build config. Output goes to dist/ which is embedded into
// the Go binary via studio/frontend/embed.go.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // During `npm run dev`, proxy API + WS calls to a running `clab studio`.
      "/api": {
        target: "http://127.0.0.1:8080",
        changeOrigin: true,
        ws: true,
      },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
