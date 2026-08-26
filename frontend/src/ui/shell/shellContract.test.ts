import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("desktop shell contract", () => {
  it("freezes the approved desktop shell dimensions in token authority", () => {
    const tokens = read("src/ui/foundation/openlife.tokens.css");
    expect(tokens).toContain("--ol-shell-sidebar-width: 232px");
    expect(tokens).toContain("--ol-shell-inspector-width: 460px");
    expect(tokens).toContain("--ol-shell-context-height: 56px");
    expect(tokens).toContain("--ol-shell-min-width: 0px");
  });

  it("keeps literal colors and sub-12px type out of shell consumers", () => {
    for (const path of ["src/ui/shell/openlife.shell.css"]) {
      const source = read(path);
      expect(source, path).not.toMatch(/#[0-9a-f]{3,8}|(?:rgb|hsl)a?\(/i);
      for (const match of source.matchAll(/font-size:\s*([0-9.]+)px/g)) {
        expect(Number(match[1]), `${path}: ${match[0]}`).toBeGreaterThanOrEqual(12);
      }
    }
  });

  it("uses one responsive shell without a second mobile route authority", () => {
    const shellCss = read("src/ui/shell/openlife.shell.css");

    expect(shellCss).toMatch(/@media\s*\(max-width:\s*860px\)/);
    expect(shellCss).toMatch(/@media\s*\(max-width:\s*560px\)/);
    expect(shellCss).not.toMatch(/mobile-route|mobile-shell|drawer-route/);
  });

  it("makes the workbench shell the only production shell authority", () => {
    const app = read("src/App.tsx");

    expect(app).toContain("ProductWorkbench");
    expect(app).not.toMatch(/src\/dev\//);
    expect(existsSync(join(process.cwd(), "src/components/ProductShell.tsx"))).toBe(false);
    expect(existsSync(join(process.cwd(), "src/productShellContract.ts"))).toBe(false);
  });

  it("keeps the skip-link target focusable and release guards on a stable marker", () => {
    const shell = read("src/ui/shell/OpenLifeWorkbenchShell.tsx");
    const absenceGuard = read("scripts/verify-production-absence.mjs");

    expect(shell).toMatch(/id="ol-shell-main"[\s\S]*?tabIndex=\{-1\}/);
    expect(absenceGuard).toContain('"ol-workbench-shell"');
  });
});
