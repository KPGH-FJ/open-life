import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const frontendRoot = process.cwd();
const distRoot = join(frontendRoot, "dist");
const sourceRoot = join(frontendRoot, "src");
const appPath = join(sourceRoot, "App.tsx");
const routeContractPath = join(sourceRoot, "ui", "productRouteContract.ts");
const lifeModelBuilderPath = join(
  sourceRoot,
  "ui",
  "journeys",
  "durableTruth",
  "LifeModelBuilderPanel.tsx"
);
const retiredLifeModelBuilderDataSourcePath = join(
  sourceRoot,
  "ui",
  "journeys",
  "durableTruth",
  "lifeModelBuilderDataSource.ts"
);

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
  "src/ui/journeys/durableTruth/lifeModelBuilderDataSource.ts",
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
  "ProductWorkbenchJourney",
  "tauriProductBoundaryDataSource",
  "tauriGovernedActionDataSource",
  "tauriDurableTruthDataSource",
  "tauriSettingsPrivacyDataSource",
  "tauriWorkspaceConversationDataSource",
]) {
  if (!appSource.includes(requiredOwner)) {
    throw new Error(`src/App.tsx is missing production owner ${requiredOwner}`);
  }
}
if (existsSync(retiredLifeModelBuilderDataSourcePath)) {
  throw new Error("The retired 4D LifeModel Builder data source must stay absent.");
}
const lifeModelBuilderSource = readFileSync(lifeModelBuilderPath, "utf8");
for (const requiredMarker of ["DraftLifeModelV2ChangeRequest", 'operation: "add"']) {
  if (!lifeModelBuilderSource.includes(requiredMarker)) {
    throw new Error(`The v2 LifeModel Builder is missing ${requiredMarker}`);
  }
}
for (const retiredMarker of ["BuilderSession", "goals.short_term", "state.current_focus"]) {
  if (lifeModelBuilderSource.includes(retiredMarker)) {
    throw new Error(`The v2 LifeModel Builder contains retired marker ${retiredMarker}`);
  }
}
if (/ProductShell|productShellContract|LEGACY_PRODUCT_REDIRECTS/.test(appSource)) {
  throw new Error("src/App.tsx still references a retired shell or redirect authority.");
}

const routeSource = readFileSync(routeContractPath, "utf8");
for (const canonicalPath of [
  'workspace: "/workspace"',
  '"life-model": "/life-model"',
  'SETTINGS_ROUTE_PATH = "/settings"',
]) {
  if (!routeSource.includes(canonicalPath)) {
    throw new Error(`Production route contract is missing ${canonicalPath}`);
  }
}
for (const retiredTopLevelPath of ['"/today"', '"/tasks"', '"/review"']) {
  if (!routeSource.includes(retiredTopLevelPath)) {
    throw new Error(`Production route contract must explicitly retire ${retiredTopLevelPath}`);
  }
}
if (/today:\s*"\/today"|tasks:\s*"\/tasks"|review:\s*"\/review"/.test(routeSource)) {
  throw new Error("Today, Tasks, and Review must not return as top-level product routes.");
}
if (/Navigate|Redirect|LEGACY_PRODUCT_REDIRECTS/.test(routeSource)) {
  throw new Error("Production route contract must not authorize compatibility redirects.");
}

for (const retiredFrontendOwner of [
  "src/ui/journeys/readOnly",
  "src/viewmodels/today",
  "src/utils/dailyGoalDisplayGuard.ts",
]) {
  if (existsSync(join(frontendRoot, retiredFrontendOwner))) {
    throw new Error(`Retired frontend owner must stay absent: ${retiredFrontendOwner}`);
  }
}

function sourceFiles(directory) {
  return readdirSync(directory).flatMap(entry => {
    if (entry === "dev" || entry === "test") return [];
    const fullPath = join(directory, entry);
    return statSync(fullPath).isDirectory() ? sourceFiles(fullPath) : [fullPath];
  });
}

const forbiddenSourceMarkers = [
  "src/dev/",
  "currentViewSummary",
  "dimensionSummaries",
  '"current_compatibility"',
  "recommend_mcp_manifests",
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
  "LAYOUT_FIXTURE",
  "src/dev/",
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
  "Production authority guard passed: Workbench journeys are shipped; old shell/pages, compatibility redirects, and dev harnesses are absent."
);
