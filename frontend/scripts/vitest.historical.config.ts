import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export default defineConfig({
  root: frontendRoot,
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: [
      "src/stage1BrowserEvidence.test.ts",
      "src/step6ProductAcceptance.test.ts",
      "src/dev/**/*.test.{ts,tsx}",
      "src/tauri.test.ts",
    ],
    exclude: ["**/node_modules/**", "**/dist/**"],
    env: {
      OPENLIFE_VITEST_SCOPE: "historical",
    },
    coverage: {
      enabled: false,
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(frontendRoot, "src"),
    },
  },
});
