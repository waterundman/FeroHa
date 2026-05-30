import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    chunkSizeWarningLimit: 700,
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom"],
          codemirror: [
            "codemirror",
            "@codemirror/autocomplete",
            "@codemirror/commands",
            "@codemirror/lang-markdown",
            "@codemirror/language",
            "@codemirror/search",
            "@codemirror/state",
            "@codemirror/view",
          ],
          graph: ["d3-force"],
        },
      },
    },
  },
});
