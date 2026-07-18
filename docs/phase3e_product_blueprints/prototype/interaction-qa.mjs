import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(directory, "../../..");
const requireFromFrontend = createRequire(path.join(repoRoot, "frontend/package.json"));
const { chromium } = requireFromFrontend("@playwright/test");

const baseUrl =
  process.env.OPENLIFE_BLUEPRINT_URL ||
  "http://127.0.0.1:4183/docs/phase3e_product_blueprints/prototype/index.html";

const failures = [];
let assertions = 0;

function check(condition, message) {
  assertions += 1;
  if (!condition) failures.push(message);
}

function channel(value) {
  const normalized = value / 255;
  return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
}

function luminance(hex) {
  const value = hex.replace("#", "");
  return (
    0.2126 * channel(Number.parseInt(value.slice(0, 2), 16)) +
    0.7152 * channel(Number.parseInt(value.slice(2, 4), 16)) +
    0.0722 * channel(Number.parseInt(value.slice(4, 6), 16))
  );
}

function contrast(foreground, background) {
  const light = Math.max(luminance(foreground), luminance(background));
  const dark = Math.min(luminance(foreground), luminance(background));
  return (light + 0.05) / (dark + 0.05);
}

const browser = await chromium.launch({ headless: true });

try {
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await desktop.goto(`${baseUrl}?screen=today-ready`, { waitUntil: "networkidle" });

  await desktop.click('[data-action-id="today:view-pending-review"]');
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-pending",
    "Today review action must open the pending decision screen."
  );
  check(
    (await desktop.locator("#contextTitle").textContent())?.includes("审核"),
    "Review navigation must provide visible page feedback."
  );

  await desktop.click('[data-action-id="review:approve"]');
  check(
    (await desktop.locator("#feedbackDialog").getAttribute("open")) !== null,
    "Approve must open confirmation."
  );
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-pending",
    "Opening approval confirmation must not change product state."
  );
  await desktop.click("[data-dialog-cancel]");
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-pending",
    "Cancelling approval must keep the item pending."
  );

  await desktop.click('[data-action-id="review:approve"]');
  await desktop.click("[data-dialog-confirm]");
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-approved",
    "Confirmed approval must enter approved-not-applied state."
  );
  check(
    await desktop.locator('[data-action-id="review-approved:apply"]').isDisabled(),
    "Apply action must stay disabled without refreshed read-model proof."
  );
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("尚未应用"),
    "Approved state must explicitly say the change is not applied."
  );

  await desktop.goto(`${baseUrl}?screen=workspace`, { waitUntil: "networkidle" });
  check(
    await desktop.locator('[data-action-id="workspace:allow-once"]').isDisabled(),
    "Permission action must be disabled while scope or transmission is unproven."
  );
  await desktop.click('[data-action-id="workspace:view-scope"]');
  check(
    (await desktop.locator("#evidenceInspector").getAttribute("aria-hidden")) === "false",
    "Permission scope action must open the evidence inspector."
  );
  const permissionText = await desktop.locator("#inspectorBody").textContent();
  for (const label of ["工具", "能力", "目标", "数据", "外传", "时效", "撤销"]) {
    check(permissionText?.includes(label), `Permission inspector must include ${label}.`);
  }
  await desktop.click("#closeInspector");
  check(
    await desktop
      .locator('[data-action-id="workspace:view-scope"]')
      .evaluate(node => document.activeElement === node),
    "Closing the inspector must restore focus to its trigger."
  );

  await desktop.goto(`${baseUrl}?screen=today-stale`, { waitUntil: "networkidle" });
  check(
    (await desktop.locator("#primaryStatus").textContent())?.includes("陈旧"),
    "Stale Today must present an unknown/fail-closed primary status."
  );
  check(
    !(await desktop
      .locator("#primaryStatus")
      .evaluate(node => node.classList.contains("is-success"))),
    "Stale Today must not use a green success status."
  );

  const liveRegion = desktop.locator("#liveRegion");
  await desktop.selectOption("#blueprintSelect", "tasks");
  await desktop.waitForTimeout(60);
  check(
    (await liveRegion.textContent())?.includes("任务"),
    "Screen changes must be announced through aria-live."
  );
  check(
    (await desktop.locator('[data-nav-key="tasks"][aria-current="page"]').count()) > 0,
    "Current navigation must expose aria-current=page."
  );

  await desktop.close();

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await mobile.goto(`${baseUrl}?screen=workspace`, { waitUntil: "networkidle" });
  const fixedDecision = mobile.locator(".layout-workspace .decision-inline:visible");
  const bottomNav = mobile.locator("#mobileBottomNav");
  const [decisionBox, navBox] = await Promise.all([
    fixedDecision.boundingBox(),
    bottomNav.boundingBox(),
  ]);
  check(
    Boolean(decisionBox),
    "Workspace must expose a mobile decision bar above the bottom navigation."
  );
  check(
    Boolean(decisionBox && navBox && decisionBox.y + decisionBox.height <= navBox.y + 1),
    "Mobile decision bar must not overlap the bottom navigation."
  );
  const mobileActionHeights = await fixedDecision
    .locator("button")
    .evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().height));
  check(
    mobileActionHeights.every(height => height >= 40),
    "Mobile decision targets must be at least 40px tall."
  );

  await mobile.click("#openInspector");
  await mobile.waitForTimeout(50);
  check(
    (await mobile.locator("#evidenceInspector").getAttribute("aria-hidden")) === "false",
    "Evidence must open as an accessible mobile sheet."
  );
  check(
    await mobile.locator("#closeInspector").evaluate(node => document.activeElement === node),
    "Mobile evidence sheet must move focus to its close control."
  );
  await mobile.keyboard.press("Escape");
  check(
    await mobile.locator("#openInspector").evaluate(node => document.activeElement === node),
    "Closing the mobile evidence sheet must restore focus."
  );

  await mobile.click("#openMobileMenu");
  check(
    (await mobile.locator("#mobileDrawer").getAttribute("open")) !== null,
    "Mobile menu must open as a drawer."
  );
  await mobile.click('#mobileDrawer [data-nav-key="tasks"]');
  check(
    (await mobile.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "tasks",
    "Tasks must remain reachable from the mobile drawer."
  );
  await mobile.close();

  const contrastPairs = [
    ["#666666", "#ffffff", "muted text on surface"],
    ["#666666", "#fafafa", "muted text on subtle surface"],
    ["#4f4f4f", "#f5f5f5", "secondary text on sidebar"],
    ["#805b10", "#fffaf0", "warning text on warning surface"],
    ["#9f3a35", "#fff7f6", "error text on error surface"],
  ];
  for (const [foreground, background, label] of contrastPairs) {
    const ratio = contrast(foreground, background);
    check(ratio >= 4.5, `${label} contrast is ${ratio.toFixed(2)}:1, below 4.5:1.`);
  }
} finally {
  await browser.close();
}

if (failures.length) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  process.exit(1);
}

console.log(`Interaction QA passed: ${assertions} assertions.`);
