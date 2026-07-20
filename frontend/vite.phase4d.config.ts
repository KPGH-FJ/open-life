import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

const vitePort = Number(process.env.PORT || process.env.VITE_PORT || 4186);
const harnessPaths = new Set(["/dev/phase4d/", "/dev/phase4d/index.html"]);

function phase4dEntryBoundary(): Plugin {
  return {
    name: "phase4d-entry-boundary",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        if (request.method !== "GET" || !request.url) return next();

        const pathname = new URL(request.url, "http://phase4d.local").pathname;
        const acceptsHtml = request.headers.accept?.includes("text/html") ?? false;
        const isHtmlNavigation =
          acceptsHtml || pathname === "/" || pathname.endsWith("/") || pathname.endsWith(".html");
        if (!isHtmlNavigation || harnessPaths.has(pathname)) return next();

        response.statusCode = 404;
        response.setHeader("Content-Type", "text/plain; charset=utf-8");
        response.end("Phase 4D read-only journey harness is available only at /dev/phase4d/.\n");
      });
    },
  };
}

export default defineConfig(({ command }) => ({
  appType: "mpa",
  base: command === "build" ? "./" : "/",
  plugins: [phase4dEntryBoundary(), react()],
  define: {
    __OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false),
    __OPENLIFE_PHASE4C_HARNESS__: JSON.stringify(false),
    __OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(true),
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
    outDir: "dist-phase4d",
    emptyOutDir: true,
    minify: "terser",
    rollupOptions: {
      input: new URL("./dev/phase4d/index.html", import.meta.url).pathname,
    },
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
  },
}));
