import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: [
      "src/stage1BrowserEvidence.test.ts",
      "src/step6ProductAcceptance.test.ts",
      "src/dev/**",
      "src/test/archive/**",
    ],
    coverage: {
      reporter: ["text", "json", "json-summary", "html"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/test/**/*", "src/dev/**/*", "src/tauriDev.ts"],
      thresholds: {
        lines: 60,
        functions: 40,
        branches: 50,
        statements: 60,
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
