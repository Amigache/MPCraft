import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and does not tolerate the dev server wandering.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    // The end-to-end specs are Playwright's, not Vitest's. The two runners cannot share
    // a file, so the folder is kept out of the unit test run.
    exclude: ["e2e/**", "node_modules/**", "dist/**"],
  },
  build: {
    target: "esnext",
    sourcemap: true,
    rollupOptions: {
      output: {
        // The map library is a third of the bundle and changes far less often than
        // the interface does; a chunk of its own keeps the cache useful.
        manualChunks: {
          maplibre: ["maplibre-gl"],
        },
      },
    },
  },
});
