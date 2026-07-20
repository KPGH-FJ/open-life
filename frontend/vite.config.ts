import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const vitePort = Number(process.env.PORT || process.env.VITE_PORT || 5173);

export default defineConfig(async ({ command }) => ({
  base: command === "build" ? "./" : "/",
  plugins: [react()],
  define: {
    __OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false),
    __OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(false),
    __OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(false),
  },
  resolve: {
    alias: {
      "@": new URL("./src", import.meta.url).pathname,
    },
  },
  clearScreen: false,
  server: {
    port: vitePort,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    minify: "terser",
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
  },
}));
