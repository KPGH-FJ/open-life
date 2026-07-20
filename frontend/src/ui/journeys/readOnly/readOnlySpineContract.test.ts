import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("Phase 4D read-only journey boundaries", () => {
  it("keeps Phase 4D out of production route and shell authorities", () => {
    for (const path of [
      "src/App.tsx",
      "src/components/ProductShell.tsx",
      "src/productShellContract.ts",
    ]) {
      const source = read(path);
      expect(source, path).not.toMatch(/ReadOnlySpineJourney|src\/dev\/phase4d|PHASE4D/);
    }
  });

  it("contains no narrow-screen navigation implementation or narrow viewport breakpoint", () => {
    const sources = [
      read("src/ui/journeys/readOnly/ReadOnlySpineJourney.tsx"),
      read("src/ui/journeys/readOnly/readOnlySpine.css"),
      read("src/dev/phase4d/Phase4dReadOnlyHarness.tsx"),
      read("src/dev/phase4d/phase4d-harness.css"),
    ].join("\n");

    expect(sources).not.toMatch(/bottom[- ]?(?:nav|sheet)|drawer|@media\s*\(max-width/i);
  });

  it("keeps literal colors and sub-12px type out of new CSS consumers", () => {
    for (const path of [
      "src/ui/journeys/readOnly/readOnlySpine.css",
      "src/dev/phase4d/phase4d-harness.css",
    ]) {
      const source = read(path);
      expect(source, path).not.toMatch(/#[0-9a-f]{3,8}|(?:rgb|hsl)a?\(/i);
      for (const match of source.matchAll(/font-size:\s*([0-9.]+)px/g)) {
        expect(Number(match[1]), `${path}: ${match[0]}`).toBeGreaterThanOrEqual(12);
      }
    }
  });

  it("requires an isolated entry and release absence markers", () => {
    expect(read("vite.phase4d.config.ts")).toContain(
      "__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(true)"
    );
    expect(read("vite.config.ts")).toContain("__OPENLIFE_PHASE4D_HARNESS__: JSON.stringify(false)");
    const guard = read("scripts/verify-production-absence.mjs");
    expect(guard).toContain("OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS");
    expect(guard).toContain("ReadOnlySpineJourney");
    expect(guard).toContain("dev/phase4d/index.html");
  });
});
