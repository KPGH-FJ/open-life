import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("Phase 4C desktop shell contract", () => {
  it("freezes the approved desktop shell dimensions in token authority", () => {
    const tokens = read("src/ui/foundation/openlife.tokens.css");
    expect(tokens).toContain("--ol-shell-sidebar-width: 232px");
    expect(tokens).toContain("--ol-shell-inspector-width: 344px");
    expect(tokens).toContain("--ol-shell-context-height: 56px");
    expect(tokens).toContain("--ol-shell-min-width: 1024px");
  });

  it("keeps literal colors and sub-12px type out of shell consumers", () => {
    for (const path of ["src/ui/shell/openlife.shell.css", "src/dev/phase4c/phase4c-harness.css"]) {
      const source = read(path);
      expect(source, path).not.toMatch(/#[0-9a-f]{3,8}|(?:rgb|hsl)a?\(/i);
      for (const match of source.matchAll(/font-size:\s*([0-9.]+)px/g)) {
        expect(Number(match[1]), `${path}: ${match[0]}`).toBeGreaterThanOrEqual(12);
      }
    }
  });

  it("does not introduce a mobile shell, bottom navigation, or responsive route authority", () => {
    const shellCss = read("src/ui/shell/openlife.shell.css");
    const harnessCss = read("src/dev/phase4c/phase4c-harness.css");
    const harness = read("src/dev/phase4c/DesktopShellHarness.tsx");

    expect(shellCss).not.toMatch(/@media\s*\(max-width/);
    expect(harnessCss).not.toMatch(/@media\s*\(max-width/);
    expect(harness).not.toMatch(/bottom[- ]?(?:nav|sheet)|mobile|drawer/i);
  });

  it("keeps the new shell and harness out of current production authorities", () => {
    const app = read("src/App.tsx");
    const productShell = read("src/components/ProductShell.tsx");
    const routeContract = read("src/productShellContract.ts");

    for (const source of [app, productShell, routeContract]) {
      expect(source).not.toMatch(/OpenLifeWorkbenchShell|src\/dev\/phase4c|PHASE4C_DESKTOP/);
    }
  });

  it("keeps the skip-link target focusable and release guards on a stable marker", () => {
    const shell = read("src/ui/shell/OpenLifeWorkbenchShell.tsx");
    const absenceGuard = read("scripts/verify-production-absence.mjs");

    expect(shell).toMatch(/id="ol-shell-main"[\s\S]*?tabIndex=\{-1\}/);
    expect(absenceGuard).toContain('"ol-workbench-shell"');
  });

  it("typechecks the dedicated Vite configuration", () => {
    expect(read("tsconfig.node.json")).toContain('"vite.phase4c.config.ts"');
  });

  it("uses real current contract fields or explicit layout_fixture sources", () => {
    const fixtures = read("src/dev/phase4c/phase4c-fixtures.ts");

    for (const hallucinatedField of [
      "TodayViewModel.focusItems",
      "TodayViewModel.reviewAttention",
      "WorkspaceViewModel.waitingPermission.scope",
      "LifeStateProjection.applicationStatus",
    ]) {
      expect(fixtures).not.toContain(hallucinatedField);
    }
    for (const currentSource of [
      "layout_fixture.today.focusList",
      "TodayViewModel.pendingReviewCount + reviewCenterLink",
      "WorkspaceViewModel.pendingReviewItems[].decisionContext.permission",
      "ReviewItem.materializationStatus",
      "LifeModelViewModel.currentViewSummary + provenanceRefs",
      "ProviderPrivacyBoundarySummary",
    ]) {
      expect(fixtures).toContain(currentSource);
    }
  });
});
