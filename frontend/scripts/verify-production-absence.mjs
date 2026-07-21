import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const frontendRoot = process.cwd();
const distRoot = join(frontendRoot, "dist");
const sourceRoot = join(frontendRoot, "src");
const appPath = join(sourceRoot, "App.tsx");
const routeContractPath = join(sourceRoot, "ui", "productRouteContract.ts");

if (!existsSync(distRoot)) {
  throw new Error("Production dist is missing; run the normal frontend build first.");
}

const forbiddenOldOwners = [
  "src/components/ProductShell.tsx",
  "src/components/product/ProductPrimitives.tsx",
  "src/productShellContract.ts",
  "src/pages/TodayPage.tsx",
  "src/pages/CompanionPage.tsx",
  "src/pages/ChatPage.tsx",
  "src/pages/RunsPage.tsx",
  "src/pages/MailboxPage.tsx",
  "src/pages/LifeModelPage.tsx",
  "src/pages/MemorySearch.tsx",
  "src/pages/SettingsPage.tsx",
  "src/pages/BuilderPage.tsx",
  "src/pages/AgentRunDetail.tsx",
  "src/pages/TodayV2PreviewPage.tsx",
];

for (const directory of ["src/pages", "src/components"]) {
  if (existsSync(join(frontendRoot, directory))) {
    throw new Error(`Retired frontend owner directory must stay absent: ${directory}`);
  }
}

for (const owner of forbiddenOldOwners) {
  if (existsSync(join(frontendRoot, owner))) {
    throw new Error(`Retired frontend owner must stay absent: ${owner}`);
  }
}

const appSource = readFileSync(appPath, "utf8");
for (const requiredOwner of [
  "ReadOnlySpineJourney",
  "tauriReadOnlySpineDataSource",
  "tauriGovernedActionDataSource",
  "tauriDurableTruthDataSource",
  "tauriSettingsPrivacyDataSource",
  "tauriWorkspaceConversationDataSource",
  "tauriLifeModelBuilderDataSource",
]) {
  if (!appSource.includes(requiredOwner)) {
    throw new Error(`src/App.tsx is missing production owner ${requiredOwner}`);
  }
}
if (/ProductShell|productShellContract|LEGACY_PRODUCT_REDIRECTS/.test(appSource)) {
  throw new Error("src/App.tsx still references a retired shell or redirect authority.");
}

const routeSource = readFileSync(routeContractPath, "utf8");
for (const canonicalPath of [
  'today: "/today"',
  'workspace: "/workspace"',
  'tasks: "/tasks"',
  'review: "/review"',
  '"life-model": "/life-model"',
  'SETTINGS_ROUTE_PATH = "/settings"',
]) {
  if (!routeSource.includes(canonicalPath)) {
    throw new Error(`Production route contract is missing ${canonicalPath}`);
  }
}
if (/Navigate|Redirect|LEGACY_PRODUCT_REDIRECTS/.test(routeSource)) {
  throw new Error("Production route contract must not authorize compatibility redirects.");
}

function sourceFiles(directory) {
  return readdirSync(directory).flatMap(entry => {
    if (entry === "dev" || entry === "test") return [];
    const fullPath = join(directory, entry);
    return statSync(fullPath).isDirectory() ? sourceFiles(fullPath) : [fullPath];
  });
}

const forbiddenSourceMarkers = [
  "src/dev/phase4b",
  "OPENLIFE_PHASE4B_DEV_HARNESS",
  "src/dev/phase4c",
  "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS",
  "src/dev/phase4d",
  "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
  "OPENLIFE_PHASE4D_GOVERNED_ACTION_HARNESS",
  "OPENLIFE_PHASE4D_DURABLE_TRUTH_HARNESS",
  "OPENLIFE_PHASE4D_PRIVACY_CONFIGURATION_HARNESS",
  "OPENLIFE_PHASE4D_REAL_TAURI_PROBE",
];

for (const filePath of sourceFiles(sourceRoot)) {
  if (!/\.(?:ts|tsx|css)$/.test(filePath) || /\.test\.(?:ts|tsx)$/.test(filePath)) continue;
  const content = readFileSync(filePath, "utf8");
  for (const marker of forbiddenSourceMarkers) {
    if (content.includes(marker)) {
      throw new Error(`${relative(frontendRoot, filePath)} contains dev-only marker ${marker}`);
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
  "dev/phase4c/index.html",
  "OPENLIFE_PHASE4D_READ_ONLY_SPINE_HARNESS",
  "OPENLIFE_PHASE4D_GOVERNED_ACTION_HARNESS",
  "OPENLIFE_PHASE4D_DURABLE_TRUTH_HARNESS",
  "OPENLIFE_PHASE4D_PRIVACY_CONFIGURATION_HARNESS",
  "OPENLIFE_PHASE4D_REAL_TAURI_PROBE",
  "dev/phase4d/index.html",
];
let workbenchCssPresent = false;

for (const filePath of releaseFiles(distRoot)) {
  if (!/\.(?:css|html|js|json|map|txt)$/.test(filePath)) continue;
  const content = readFileSync(filePath, "utf8");
  if (content.includes("ol-workbench-shell")) workbenchCssPresent = true;
  for (const marker of forbiddenBundleMarkers) {
    if (content.includes(marker)) {
      throw new Error(`${relative(frontendRoot, filePath)} contains dev-only marker ${marker}`);
    }
  }
}

if (!workbenchCssPresent) {
  throw new Error("Production bundle does not contain the OpenLife Workbench shell owner.");
}

console.log(
  "Production authority guard passed: Workbench journeys are shipped; old shell/pages, compatibility redirects, and Phase 4 dev harnesses are absent."
);
