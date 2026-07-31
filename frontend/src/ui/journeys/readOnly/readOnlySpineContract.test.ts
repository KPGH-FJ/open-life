import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("read-only journey boundaries", () => {
  it("makes the read-only journey owner production authority without importing a dev harness", () => {
    const app = read("src/App.tsx");
    expect(app).toContain("ReadOnlySpineJourney");
    expect(app).not.toMatch(/src\/dev\//);
    expect(existsSync(join(process.cwd(), "src/components/ProductShell.tsx"))).toBe(false);
    expect(existsSync(join(process.cwd(), "src/productShellContract.ts"))).toBe(false);
  });

  it("contains no narrow-screen navigation implementation or narrow viewport breakpoint", () => {
    const sources = [
      read("src/ui/journeys/readOnly/ReadOnlySpineJourney.tsx"),
      read("src/ui/journeys/readOnly/readOnlySpine.css"),
    ].join("\n");

    expect(sources).not.toMatch(/bottom[- ]?(?:nav|sheet)|drawer|@media\s*\(max-width/i);
  });

  it("keeps literal colors and sub-12px type out of new CSS consumers", () => {
    for (const path of ["src/ui/journeys/readOnly/readOnlySpine.css"]) {
      const source = read(path);
      expect(source, path).not.toMatch(/#[0-9a-f]{3,8}|(?:rgb|hsl)a?\(/i);
      for (const match of source.matchAll(/font-size:\s*([0-9.]+)px/g)) {
        expect(Number(match[1]), `${path}: ${match[0]}`).toBeGreaterThanOrEqual(12);
      }
    }
  });

  it("keeps retired shell surfaces absent from release", () => {
    const guard = read("scripts/verify-production-absence.mjs");
    expect(guard).toContain("ProductShell.tsx");
  });
});
