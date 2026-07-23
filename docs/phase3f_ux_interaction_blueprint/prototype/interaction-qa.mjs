import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(directory, "../../..");
const requireFromFrontend = createRequire(path.join(repoRoot, "frontend/package.json"));
const { chromium } = requireFromFrontend("@playwright/test");

const baseUrl =
  process.env.OPENLIFE_PHASE3F_URL ||
  "http://127.0.0.1:4184/docs/phase3f_ux_interaction_blueprint/prototype/index.html";

const failures = [];
const browserErrors = [];
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

function watchPage(page, label) {
  page.on("console", message => {
    if (["error", "warning"].includes(message.type())) {
      browserErrors.push(`${label} console ${message.type()}: ${message.text()}`);
    }
  });
  page.on("pageerror", error => browserErrors.push(`${label} pageerror: ${error.message}`));
}

const browser = await chromium.launch({ headless: true });

try {
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(desktop, "desktop");
  await desktop.goto(`${baseUrl}?screen=today-ready`, { waitUntil: "networkidle" });

  await desktop.click('[data-action-id="today:view-pending-review"]');
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-pending",
    "Viewing a Today review item must open the pending decision, not an approved state."
  );
  check(
    (await desktop.locator('[data-nav-key="review"][aria-current="page"]').count()) > 0,
    "Current product navigation must expose aria-current=page."
  );

  await desktop.click('[data-action-id="review:approve"]');
  check(
    (await desktop.locator("#feedbackDialog").getAttribute("open")) !== null,
    "Approve must open confirmation."
  );
  check(
    await desktop.locator("[data-dialog-cancel]").evaluate(node => document.activeElement === node),
    "Confirmation must place focus on the safe cancel action."
  );
  await desktop.click("[data-dialog-cancel]");
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-pending",
    "Cancelling approval must keep the item pending."
  );

  await desktop.click('[data-action-id="review:edit"]');
  await desktop.fill("#reviewEditValue", "工作日 09:30—11:00 优先安排深度工作。");
  await desktop.click("[data-dialog-confirm]");
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("09:30—11:00"),
    "Editing must produce a visible pending-edited result."
  );
  check(
    (await desktop.locator("#primaryStatus").textContent())?.includes("仍待决定"),
    "Editing must not approve or apply the proposal."
  );

  await desktop.selectOption("#blueprintSelect", "review-pending");
  await desktop.click('[data-action-id="review:reject"]');
  check(
    (await desktop.locator(".review-result-banner").textContent())?.includes("已拒绝"),
    "Reject must produce a visible rejected result."
  );
  check(
    !(await desktop.locator(".review-result-banner").textContent())?.includes("已应用"),
    "Rejected state must not imply materialization."
  );

  await desktop.selectOption("#blueprintSelect", "review-pending");
  await desktop.click('[data-action-id="review:later"]');
  check(
    (await desktop.locator(".review-result-banner").textContent())?.includes("稍后处理"),
    "Later must produce a visible postponed result."
  );

  await desktop.selectOption("#blueprintSelect", "review-pending");
  await desktop.click('[data-action-id="review:approve"]');
  await desktop.click("[data-dialog-confirm]");
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "review-approved",
    "Confirmed approval must enter approved-not-applied state."
  );
  check(
    await desktop.locator('[data-action-id="review-approved:apply"]').isDisabled(),
    "Apply must stay disabled without a backend materialization request action."
  );
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("尚未应用"),
    "Approval must remain distinct from applied/completed."
  );

  await desktop.selectOption("#blueprintSelect", "workspace-unknown");
  check(
    await desktop.locator('[data-action-id="workspace:allow-once"]').isDisabled(),
    "Unknown permission scope must fail closed."
  );
  check(
    !(await desktop
      .locator("#primaryStatus")
      .evaluate(node => node.classList.contains("is-success"))),
    "Unknown permission must not render a green status."
  );

  await desktop.selectOption("#blueprintSelect", "workspace");
  check(
    !(await desktop.locator('[data-action-id="workspace:allow-once"]').isDisabled()),
    "Known exact action-bound permission fixture must expose allow-once."
  );
  await desktop.click('[data-action-id="workspace:view-scope"]');
  const permissionText = await desktop.locator("#inspectorBody").textContent();
  for (const label of ["工具", "能力", "目标", "数据", "外传", "时效", "撤销"]) {
    check(permissionText?.includes(label), `Permission inspector must include ${label}.`);
  }
  await desktop.click("#closeInspector");
  check(
    await desktop
      .locator('[data-action-id="workspace:view-scope"]')
      .evaluate(node => document.activeElement === node),
    "Closing Inspector must restore focus to the permission scope trigger."
  );

  await desktop.click('[data-action-id="workspace:allow-once"]');
  check(
    (await desktop.locator("#dialogBody").textContent())?.includes("只对当前阻塞动作生效一次"),
    "Permission confirmation must explain exact one-time scope."
  );
  await desktop.click("[data-dialog-cancel]");
  check(
    (await desktop.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "workspace",
    "Cancelling permission must keep the task waiting."
  );

  await desktop.click('[data-action-id="workspace:allow-once"]');
  await desktop.click("[data-dialog-confirm]");
  await desktop.waitForFunction(
    () => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey === "workspace-running",
    null,
    { timeout: 3000 }
  );
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("一次性授权已匹配当前动作"),
    "Permission flow must visibly reach the refreshed running state."
  );

  await desktop.click('[data-static-feedback="attach"]');
  check(
    (await desktop.locator(".resource-chip.is-importing").count()) === 1,
    "Attachment action must show a verifiable importing state."
  );
  await desktop.waitForFunction(
    () => document.querySelector(".resource-tray")?.textContent?.includes("静态回执已提交"),
    null,
    { timeout: 1600 }
  );
  check(
    (await desktop.locator(".resource-tray").textContent())?.includes("静态回执已提交"),
    "Attachment import must end in an explicit static receipt state."
  );
  const resourceCount = await desktop.locator(".resource-chip").count();
  await desktop.locator(".resource-remove:not(:disabled)").last().click();
  check(
    (await desktop.locator(".resource-chip").count()) === resourceCount - 1,
    "Detach must visibly remove one turn binding."
  );

  await desktop.selectOption("#blueprintSelect", "workspace-resources-web");
  check(
    (await desktop.locator("#sidebarPrivacyTitle").textContent())?.includes("外部网络"),
    "Web scenario must disclose its external network boundary."
  );
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("不可信外部数据"),
    "Web action must disclose untrusted external evidence semantics."
  );

  await desktop.selectOption("#blueprintSelect", "today-stale");
  check(
    (await desktop.locator("#primaryStatus").textContent())?.includes("陈旧"),
    "Stale Today must present a fail-closed status."
  );
  check(
    !(await desktop
      .locator("#primaryStatus")
      .evaluate(node => node.classList.contains("is-success"))),
    "Stale Today must not use a green success status."
  );

  await desktop.selectOption("#blueprintSelect", "settings");
  check(
    (await desktop.locator('[data-settings-category="models"][aria-current="page"]').count()) === 1,
    "Settings must use a dedicated current category navigation."
  );
  await desktop.fill("[data-settings-search]", "隐私");
  check(
    (await desktop.locator("[data-settings-search-status]").textContent())?.includes("1"),
    "Settings search must announce filtered category count."
  );
  await desktop.fill("[data-settings-search]", "");
  await desktop.fill('[data-settings-field="model"]', "deepseek-reasoner");
  await desktop.press('[data-settings-field="model"]', "Tab");
  check(
    (await desktop.locator("#primaryStatus").textContent())?.includes("边界待重新确认"),
    "Editing provider config must make boundary truth unknown/pending."
  );
  await desktop.click('[data-action-id="settings:test-provider"]');
  check(
    (await desktop.locator("#dialogBody").textContent())?.includes("不会保存配置"),
    "Connection test confirmation must say it does not save."
  );
  await desktop.click("[data-dialog-confirm]");
  await desktop.waitForSelector(".connection-result", { timeout: 1800 });
  check(
    (await desktop.locator(".connection-result").textContent())?.includes("设置尚未保存"),
    "Successful connection test must remain distinct from saved config."
  );
  await desktop.click('[data-action-id="settings:save-provider"]');
  await desktop.waitForFunction(
    () => document.querySelector("#primaryStatus")?.textContent?.includes("仍为未知"),
    null,
    { timeout: 2500 }
  );
  check(
    await desktop.locator("#primaryStatus").evaluate(node => node.classList.contains("is-warning")),
    "Saved config with unknown refreshed boundary must stay warning/fail closed."
  );
  await desktop.click('[data-settings-category="privacy"]');
  check(
    (await desktop.locator("#workSurface").textContent())?.includes("本轮只冻结信息架构与入口"),
    "Unimplemented settings categories must show explicit unavailable content."
  );

  for (const [width, height] of [[1280, 800]]) {
    await desktop.setViewportSize({ width, height });
    await desktop.selectOption("#blueprintSelect", "workspace");
    const overflow = await desktop.evaluate(
      () =>
        Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
        window.innerWidth
    );
    check(overflow <= 1, `${width}x${height} Workspace must not overflow horizontally.`);
  }
  await desktop.close();

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  watchPage(mobile, "mobile");
  await mobile.goto(`${baseUrl}?screen=workspace-unknown`, { waitUntil: "networkidle" });
  const decision = mobile.locator(".layout-workspace .decision-inline:visible");
  const bottomNav = mobile.locator("#mobileBottomNav");
  const [decisionBox, navBox] = await Promise.all([
    decision.boundingBox(),
    bottomNav.boundingBox(),
  ]);
  check(Boolean(decisionBox), "Workspace must expose its current decision on mobile.");
  check(
    Boolean(decisionBox && navBox && decisionBox.y + decisionBox.height <= navBox.y + 1),
    "Mobile permission decision must not overlap bottom navigation."
  );
  const mobileTargets = await decision
    .locator("button")
    .evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().height));
  check(
    mobileTargets.every(height => height >= 40),
    "Mobile decision targets must be at least 40px tall."
  );

  await mobile.click("#openInspector");
  await mobile.waitForFunction(() => document.activeElement?.id === "closeInspector");
  check(
    (await mobile.locator("#evidenceInspector").getAttribute("aria-hidden")) === "false",
    "Evidence must open as an accessible mobile sheet."
  );
  check(
    await mobile.locator("#closeInspector").evaluate(node => document.activeElement === node),
    "Mobile evidence sheet must move focus to Close."
  );
  await mobile.keyboard.press("Escape");
  check(
    await mobile.locator("#openInspector").evaluate(node => document.activeElement === node),
    "Closing mobile evidence must restore focus."
  );

  await mobile.click("#openMobileMenu");
  await mobile.click('#mobileDrawer [data-nav-key="tasks"]');
  check(
    (await mobile.evaluate(() => window.__OPENLIFE_BLUEPRINT__.getState().currentScreenKey)) ===
      "tasks",
    "Tasks must remain reachable from the mobile drawer."
  );
  await mobile.selectOption("#blueprintSelect", "settings");
  check(
    (await mobile
      .locator("[data-mobile-settings-category]")
      .evaluate(node => node.getBoundingClientRect().height)) >= 44,
    "Mobile settings category control must be at least 44px tall."
  );
  await mobile.selectOption("[data-mobile-settings-category]", "privacy");
  check(
    (await mobile.locator("#workSurface").textContent())?.includes("隐私与网络"),
    "Mobile settings category selector must provide visible navigation feedback."
  );
  const mobileOverflow = await mobile.evaluate(
    () =>
      Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) - window.innerWidth
  );
  check(mobileOverflow <= 1, "390x844 Settings must not overflow horizontally.");
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

if (browserErrors.length) failures.push(...browserErrors);

if (failures.length) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  process.exit(1);
}

console.log(`Phase 3F interaction QA passed: ${assertions} assertions.`);
