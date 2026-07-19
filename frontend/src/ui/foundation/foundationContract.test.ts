import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("Phase 4B token and isolation contract", () => {
  it("keeps the approved fixed visual values in the single token authority", () => {
    const tokens = read("src/ui/foundation/openlife.tokens.css");
    for (const expected of [
      "--ol-type-caption: 12px",
      "--ol-type-body: 14px",
      "--ol-type-reading: 15px",
      "--ol-space-1: 4px",
      "--ol-space-12: 48px",
      "--ol-radius-1: 4px",
      "--ol-radius-3: 8px",
      "--ol-canvas: #ffffff",
      "--ol-sidebar: #f5f5f5",
      "--ol-ink: #111111",
      "--ol-ink-muted: #666666",
      "--ol-amber: #805b10",
      "--ol-red: #9f3a35",
      "--ol-green: #2e7d4f",
      "--ol-focus: #2563eb",
    ]) {
      expect(tokens).toContain(expected);
    }
  });

  it("keeps hard-coded colors and arbitrary Tailwind values out of V2 React source", () => {
    for (const path of [
      "src/ui/foundation/foundation.tsx",
      "src/dev/phase4b/FoundationHarness.tsx",
    ]) {
      const source = read(path);
      expect(source, path).not.toMatch(/#[0-9a-f]{3,8}/i);
      expect(source, path).not.toMatch(/(?:bg|text|border|p|m|gap|w|h)-\[[^\]]+\]/);
      expect(source, path).not.toMatch(/tracking-(?:tight|wide|wider)/);
    }
  });

  it("maps Tailwind semantic aliases to CSS variables instead of second token values", () => {
    const tailwind = read("tailwind.config.js");
    expect(tailwind).toContain('"ol-canvas": "var(--ol-canvas)"');
    expect(tailwind).toContain('"ol-ink-muted": "var(--ol-ink-muted)"');
    expect(tailwind).toContain('"ol-4": "var(--ol-space-4)"');
    expect(tailwind).toContain('"ol-2": "var(--ol-radius-2)"');
  });

  it("keeps the dev entry compile-time false in the production Vite config", () => {
    const productVite = read("vite.config.ts");
    const harnessVite = read("vite.phase4b.config.ts");
    const app = read("src/App.tsx");

    expect(productVite).toContain("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(false)");
    expect(harnessVite).toContain("__OPENLIFE_PHASE4B_HARNESS__: JSON.stringify(true)");
    expect(app).not.toMatch(/TodayV2PreviewPage|\/today-v2-preview|src\/dev\/phase4b/);
  });
});
