import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const frontendRoot = process.cwd();
const distRoot = join(frontendRoot, "dist");
const appPath = join(frontendRoot, "src", "App.tsx");
const shellPath = join(frontendRoot, "src", "components", "ProductShell.tsx");
const routeContractPath = join(frontendRoot, "src", "productShellContract.ts");
const retiredPreviewPath = join(frontendRoot, "src", "pages", "TodayV2PreviewPage.tsx");

if (!existsSync(distRoot)) {
  throw new Error("Production dist is missing; run the normal frontend build first.");
}

if (existsSync(retiredPreviewPath)) {
  throw new Error("Retired TodayV2PreviewPage must stay absent from product source.");
}

const forbiddenSourceMarkers = [
  "/today-v2-preview",
  "TodayV2PreviewPage",
  "src/dev/phase4b",
  "OPENLIFE_PHASE4B_DEV_HARNESS",
  "src/dev/phase4c",
  "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS",
  "OpenLifeWorkbenchShell",
  "src/dev/phase4d",
  "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
  "ReadOnlySpineJourney",
];

for (const sourcePath of [appPath, shellPath, routeContractPath]) {
  const source = readFileSync(sourcePath, "utf8");
  for (const marker of forbiddenSourceMarkers) {
    if (source.includes(marker)) {
      throw new Error(`${relative(frontendRoot, sourcePath)} contains dev-only marker ${marker}`);
    }
  }
}

function releaseFiles(directory) {
  return readdirSync(directory).flatMap(entry => {
    const fullPath = join(directory, entry);
    return statSync(fullPath).isDirectory() ? releaseFiles(fullPath) : [fullPath];
  });
}

const forbiddenBundleMarkers = [
  "/today-v2-preview",
  "TodayV2PreviewPage",
  "OPENLIFE_PHASE4B_DEV_HARNESS",
  "LAYOUT_FIXTURE",
  "dev/phase4b/index.html",
  "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS",
  "OpenLifeWorkbenchShell",
  "ol-workbench-shell",
  "dev/phase4c/index.html",
  "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
  "ReadOnlySpineJourney",
  "ol-readonly-page",
  "dev/phase4d/index.html",
];

for (const filePath of releaseFiles(distRoot)) {
  if (!/\.(?:css|html|js|json|map|txt)$/.test(filePath)) continue;
  const content = readFileSync(filePath, "utf8");
  for (const marker of forbiddenBundleMarkers) {
    if (content.includes(marker)) {
      throw new Error(`${relative(frontendRoot, filePath)} contains dev-only marker ${marker}`);
    }
  }
}

console.log(
  "Production absence guard passed: Phase 4B/4C/4D harnesses, new desktop journeys, and retired preview are absent."
);
