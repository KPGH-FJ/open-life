#!/usr/bin/env node

import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(frontendRoot, "..");
const artifactDir = path.resolve(repoRoot, "docs/phase4d_durable_truth_spine/artifacts");
const baseUrl = process.env.OPENLIFE_PHASE4D_DURABLE_URL || "http://127.0.0.1:4186/dev/phase4d/";
const viewports = [
  { width: 1440, height: 900 },
  { width: 1280, height: 800 },
  { width: 1024, height: 720 },
];

const failures = [];
const browserErrors = [];
const observations = [];
let assertions = 0;

function check(condition, message) {
  assertions += 1;
  if (!condition) failures.push(message);
}

function delay(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

async function endpointAvailable() {
  try {
    const response = await fetch(baseUrl, { signal: AbortSignal.timeout(1000) });
    return response.ok;
  } catch {
    return false;
  }
}

async function startServer() {
  if (await endpointAvailable()) return null;
  const url = new URL(baseUrl);
  const viteEntry = path.join(frontendRoot, "node_modules/vite/bin/vite.js");
  const server = spawn(
    process.execPath,
    [
      viteEntry,
      "--config",
      "vite.phase4d.config.ts",
      "--host",
      url.hostname,
      "--port",
      url.port || "4186",
      "--strictPort",
    ],
    { cwd: frontendRoot, stdio: ["ignore", "pipe", "pipe"] }
  );
  let output = "";
  let startupError;
  const recordOutput = chunk => {
    output = `${output}${chunk}`.slice(-12000);
  };
  server.stdout.on("data", recordOutput);
  server.stderr.on("data", recordOutput);
  server.on("error", error => {
    startupError = error;
  });

  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (startupError || server.exitCode !== null) break;
    if (await endpointAvailable()) return server;
    await delay(100);
  }
  await stopServer(server);
  throw new Error(
    `Unable to start Phase 4D durable QA server.${startupError ? ` ${startupError.message}` : ""}\n${output}`
  );
}

async function stopServer(server) {
  if (!server || server.exitCode !== null || server.signalCode !== null) return;
  server.kill("SIGTERM");
  await Promise.race([once(server, "exit"), delay(2000)]);
  if (server.exitCode === null && server.signalCode === null) {
    server.kill("SIGKILL");
    await once(server, "exit");
  }
}

function watchPage(page, label) {
  page.on("console", message => {
    if (["error", "warning"].includes(message.type())) {
      browserErrors.push(`${label} console ${message.type()}: ${message.text()}`);
    }
  });
  page.on("pageerror", error => browserErrors.push(`${label} pageerror: ${error.message}`));
}

async function selectFixture(page, fixtureId) {
  await page.locator(".phase4d-source-select select").selectOption(fixtureId);
  await page.locator(`.phase4d-harness[data-source-id="${fixtureId}"]`).waitFor();
}

async function openLifeModel(page) {
  await page.getByRole("button", { name: /^LifeModel\s+长期状态/ }).click();
  await page.getByRole("heading", { name: "当前有来源的长期理解" }).waitFor();
}

async function reachWithKeyboard(page, locator, label) {
  for (let attempt = 0; attempt < 120; attempt += 1) {
    await page.keyboard.press("Tab");
    if (await locator.evaluate(node => document.activeElement === node)) {
      const focus = await locator.evaluate(node => {
        const style = getComputedStyle(node);
        return {
          visible: node.matches(":focus-visible"),
          outlineStyle: style.outlineStyle,
          outlineWidth: Number.parseFloat(style.outlineWidth),
        };
      });
      check(
        focus.visible && focus.outlineStyle !== "none" && focus.outlineWidth >= 2,
        `${label}: focus ring is not visibly at least 2px.`
      );
      return true;
    }
  }
  check(false, `${label}: target is not reachable in forward keyboard order.`);
  return false;
}

mkdirSync(artifactDir, { recursive: true });
let qaServer;
let browser;

try {
  qaServer = await startServer();
  browser = await chromium.launch({ headless: true });

  for (const viewport of viewports) {
    const label = `${viewport.width}x${viewport.height}`;
    const page = await browser.newPage({ viewport });
    watchPage(page, label);
    await page.goto(baseUrl, { waitUntil: "networkidle" });
    await selectFixture(page, "fixture-ready");
    await openLifeModel(page);

    check(
      (await page
        .locator('[data-durable-harness-marker="OPENLIFE_PHASE4D_DURABLE_TRUTH_HARNESS"]')
        .count()) === 1,
      `${label}: durable harness marker is missing.`
    );
    check(
      (await page.locator('.ol-nav-row[aria-current="page"]').count()) === 1,
      `${label}: exactly one navigation row must be current.`
    );
    check(
      (await page.locator(".phase4d-qa-toolbar .phase4d-source-select").count()) === 1 &&
        (await page.locator(".ol-workbench-shell .phase4d-source-select").count()) === 0,
      `${label}: fixture selector must remain outside the product shell.`
    );

    const layout = await page.evaluate(() => {
      const sidebar = document.querySelector(".ol-shell-sidebar").getBoundingClientRect();
      const context = document.querySelector(".ol-shell-context-bar").getBoundingClientRect();
      const reading = document.querySelector(".ol-durable-current > p");
      return {
        overflow:
          Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
          window.innerWidth,
        sidebarWidth: sidebar.width,
        contextHeight: context.height,
        bodyFont: Number.parseFloat(getComputedStyle(reading).fontSize),
        pageScrollWidth: document.querySelector(".ol-durable-page").scrollWidth,
        pageClientWidth: document.querySelector(".ol-durable-page").clientWidth,
      };
    });
    check(layout.overflow <= 1, `${label}: horizontal overflow is ${layout.overflow}px.`);
    check(Math.abs(layout.sidebarWidth - 232) <= 0.5, `${label}: sidebar must be 232px.`);
    check(Math.abs(layout.contextHeight - 56) <= 0.5, `${label}: context bar must be 56px.`);
    check(layout.bodyFont >= 15, `${label}: durable reading text must be at least 15px.`);
    check(
      layout.pageScrollWidth <= layout.pageClientWidth + 1,
      `${label}: durable page content overflows its work surface.`
    );
    check(
      (await page.locator(".ol-status-label--success").count()) === 1,
      `${label}: only the verified local boundary may be green in pending state.`
    );

    const openReview = page.getByRole("button", { name: "查看并决定" });
    check(await openReview.isEnabled(), `${label}: exact review navigation must be enabled.`);
    check(
      (await openReview.getAttribute("data-action-kind")) === "open" &&
        (await openReview.getAttribute("data-action-target-ref")) ===
          "review-lifemodel-focus-preference",
      `${label}: review navigation action contract is incomplete.`
    );

    await page.screenshot({
      path: path.join(artifactDir, `phase4d_durable_${label}_pending.png`),
      type: "png",
    });

    await page.getByRole("button", { name: "查看状态依据" }).click();
    const inspector = page.getByRole("complementary", { name: "把上午作为优先深度工作时段" });
    await inspector.waitFor();
    check(
      (await inspector.getByText("proposal-focus-morning", { exact: false }).count()) > 0,
      `${label}: Inspector must expose the proposal identity.`
    );
    await page.getByRole("button", { name: "关闭证据检查器" }).click();

    await openReview.click();
    await page.getByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 }).waitFor();
    check(
      (await page.getByText("等待决定", { exact: true }).count()) > 0,
      `${label}: viewing the review must keep it pending.`
    );
    await page.getByRole("button", { name: "批准变更" }).click();
    await page.getByRole("dialog", { name: "确认批准变更？" }).waitFor();
    await page.getByRole("button", { name: "确认批准" }).click();
    await page.locator(".ol-notice__title", { hasText: "已批准，尚未应用" }).waitFor();
    check(
      (await page.locator(".ol-notice__title", { hasText: "变更已应用" }).count()) === 0,
      `${label}: approval must not be presented as applied.`
    );
    await page.getByRole("button", { name: "返回 LifeModel" }).click();
    await page.locator('[data-durable-lifecycle="approved_not_applied"]').waitFor();
    const apply = page.getByRole("button", { name: "应用变更" });
    check(await apply.isDisabled(), `${label}: unsupported Apply must stay disabled.`);
    check(
      (await apply.getAttribute("data-action-kind")) === "apply" &&
        (await apply.getAttribute("data-action-enabled")) === "false",
      `${label}: disabled Apply must preserve its backend action contract.`
    );
    await page.screenshot({
      path: path.join(artifactDir, `phase4d_durable_${label}_approved_not_applied.png`),
      type: "png",
    });

    observations.push({ label, layout });
    await page.close();
  }

  const statePage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(statePage, "state-matrix");
  await statePage.goto(baseUrl, { waitUntil: "networkidle" });
  const states = [
    ["fixture-durable-applying", "applying", "正在应用", false],
    ["fixture-durable-applied", "applied", "已应用", true],
    ["fixture-durable-failed", "failed", "应用失败", false],
    ["fixture-durable-rolled-back", "rolled_back", "已回滚", false],
  ];
  for (const [fixtureId, lifecycle, visibleLabel, verified] of states) {
    await selectFixture(statePage, fixtureId);
    await openLifeModel(statePage);
    await statePage.locator(`[data-durable-lifecycle="${lifecycle}"]`).waitFor();
    check(
      (await statePage.getByText(visibleLabel, { exact: true }).count()) > 0,
      `${fixtureId}: visible lifecycle label is missing.`
    );
    const durableSuccessCount = await statePage
      .locator(`.ol-durable-page .ol-status-label--success`)
      .count();
    check(
      verified ? durableSuccessCount === 1 : durableSuccessCount === 0,
      `${fixtureId}: green verified treatment does not match proof state.`
    );
    await statePage.screenshot({
      path: path.join(artifactDir, `phase4d_durable_1440x900_${lifecycle}.png`),
      type: "png",
    });
  }

  await selectFixture(statePage, "fixture-stale");
  await openLifeModel(statePage);
  await statePage.getByText("长期状态已陈旧").waitFor();
  check(
    (await statePage.locator(".ol-durable-page .ol-status-label--success").count()) === 0,
    "stale: durable surface must not show verified success."
  );
  await selectFixture(statePage, "fixture-error");
  await statePage.getByRole("button", { name: /^LifeModel\s+长期状态/ }).click();
  await statePage.getByText("长期状态暂时不可用").waitFor();
  check(
    (await statePage.getByRole("button", { name: "批准变更" }).count()) === 0,
    "error: review decision must not be exposed from failed durable read models."
  );
  await statePage.close();

  const keyboardPage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(keyboardPage, "keyboard");
  await keyboardPage.goto(baseUrl, { waitUntil: "networkidle" });
  await keyboardPage.locator("body").click({ position: { x: 2, y: 2 } });
  const lifeModelNavigation = keyboardPage.getByRole("button", {
    name: /^LifeModel\s+长期状态/,
  });
  if (await reachWithKeyboard(keyboardPage, lifeModelNavigation, "LifeModel navigation")) {
    await keyboardPage.keyboard.press("Enter");
    await keyboardPage.getByRole("heading", { name: "当前有来源的长期理解" }).waitFor();
    const keyboardOpenReview = keyboardPage.getByRole("button", { name: "查看并决定" });
    if (await reachWithKeyboard(keyboardPage, keyboardOpenReview, "durable review action")) {
      await keyboardPage.keyboard.press("Enter");
      await keyboardPage
        .getByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 })
        .waitFor();
    }
  }
  await keyboardPage.close();
} catch (error) {
  failures.push(error instanceof Error ? error.stack || error.message : String(error));
} finally {
  if (browser) await browser.close();
  await stopServer(qaServer);
}

for (const browserError of browserErrors) failures.push(browserError);

const report = {
  generatedAt: new Date().toISOString(),
  baseUrl,
  viewports,
  assertions,
  failures,
  browserErrors,
  observations,
};
writeFileSync(
  path.join(artifactDir, "phase4d-durable-browser-qa.json"),
  `${JSON.stringify(report, null, 2)}\n`
);

if (failures.length > 0) {
  console.error(`Phase 4D durable QA failed (${failures.length} findings).`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Phase 4D durable QA passed: ${assertions} assertions across ${viewports.length} desktop viewports.`
);
