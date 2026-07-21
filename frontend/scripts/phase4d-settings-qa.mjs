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
const artifactDir = path.resolve(repoRoot, "docs/phase4d_privacy_configuration_spine/artifacts");
const baseUrl = process.env.OPENLIFE_PHASE4D_SETTINGS_URL || "http://127.0.0.1:4186/dev/phase4d/";
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
    `Unable to start Phase 4D settings QA server.${startupError ? ` ${startupError.message}` : ""}\n${output}`
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

async function openSettings(page) {
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("heading", { name: "模型与供应商", level: 1 }).waitFor();
  await page.getByRole("heading", { name: "模型与传输边界", level: 2 }).waitFor();
}

async function editModel(page, value) {
  const model = page.getByLabel("模型", { exact: true });
  await model.fill(value);
  await page.locator('[data-settings-phase="dirty"]').waitFor();
}

async function reachWithKeyboard(page, locator, label) {
  for (let attempt = 0; attempt < 140; attempt += 1) {
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
    await selectFixture(page, "fixture-settings-review-required");
    await openSettings(page);

    check(
      (await page
        .locator('[data-settings-harness-marker="OPENLIFE_PHASE4D_PRIVACY_CONFIGURATION_HARNESS"]')
        .count()) === 1,
      `${label}: settings harness marker is missing.`
    );
    check(
      (await page.locator('.ol-nav-row[aria-current="page"]').count()) === 1,
      `${label}: exactly one settings navigation row must be current.`
    );
    check(
      (await page.locator(".phase4d-qa-toolbar .phase4d-source-select").count()) === 1 &&
        (await page.locator(".ol-workbench-shell .phase4d-source-select").count()) === 0,
      `${label}: fixture selector must stay outside the product shell.`
    );

    const layout = await page.evaluate(() => {
      const sidebar = document.querySelector(".ol-shell-sidebar").getBoundingClientRect();
      const context = document.querySelector(".ol-shell-context-bar").getBoundingClientRect();
      const reading = document.querySelector(".ol-settings-boundary__summary p");
      const settingsPage = document.querySelector(".ol-settings-page");
      return {
        overflow:
          Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
          window.innerWidth,
        sidebarWidth: sidebar.width,
        contextHeight: context.height,
        bodyFont: Number.parseFloat(getComputedStyle(reading).fontSize),
        pageScrollWidth: settingsPage.scrollWidth,
        pageClientWidth: settingsPage.clientWidth,
      };
    });
    check(layout.overflow <= 1, `${label}: horizontal overflow is ${layout.overflow}px.`);
    check(Math.abs(layout.sidebarWidth - 232) <= 0.5, `${label}: sidebar must be 232px.`);
    check(Math.abs(layout.contextHeight - 56) <= 0.5, `${label}: context bar must be 56px.`);
    check(layout.bodyFont >= 15, `${label}: settings reading text must be at least 15px.`);
    check(
      layout.pageScrollWidth <= layout.pageClientWidth + 1,
      `${label}: settings page content overflows its work surface.`
    );
    check(
      (await page.locator(".ol-settings-page .ol-status-label--success").count()) === 0,
      `${label}: a possible external route must not be green.`
    );

    const testAction = page.getByRole("button", { name: "测试连接" });
    const saveAction = page.getByRole("button", { name: "保存设置" });
    check(await testAction.isEnabled(), `${label}: connection test must be available.`);
    check(await saveAction.isDisabled(), `${label}: unchanged settings must not be saveable.`);
    check(
      (await testAction.getAttribute("data-action-id")) === "settings.provider.test_connection" &&
        (await testAction.getAttribute("data-action-kind")) === "configure" &&
        (await testAction.getAttribute("data-target-ref")) === "settings-draft:0",
      `${label}: connection test Action Contract is incomplete.`
    );

    await testAction.click();
    const confirmation = page.getByRole("dialog", { name: "确认本次外部连接测试" });
    await confirmation.waitFor();
    check(
      (await confirmation.getByText("api.deepseek.com", { exact: true }).count()) === 1 &&
        (await confirmation.getByText("deepseek-chat", { exact: true }).count()) === 1,
      `${label}: external confirmation must identify host and model.`
    );
    await confirmation.getByRole("button", { name: "确认并测试" }).click();
    await page.getByText("需要先确认本次外部连接", { exact: true }).waitFor();
    check(
      (await page.locator(".ol-settings-page .ol-status-label--success").count()) === 0,
      `${label}: consent-required test must stay fail-closed.`
    );
    const openReview = page.getByRole("button", { name: "查看并决定" });
    check(await openReview.isEnabled(), `${label}: exact provider ReviewItem must resolve.`);
    await openReview.evaluate(node => node.scrollIntoView({ block: "center" }));
    await page.screenshot({
      path: path.join(artifactDir, `phase4d_settings_${label}_consent_required.png`),
      type: "png",
    });

    await openReview.click();
    await page.getByRole("heading", { name: "允许一次模型连接测试", level: 2 }).waitFor();
    check(
      (await page.getByText("请求尚未发送，供应商可用性未知", { exact: true }).count()) > 0,
      `${label}: permission review must show the before state.`
    );
    check(
      (await page.getByText("HTTPS api.deepseek.com", { exact: true }).count()) > 0,
      `${label}: permission review must expose the resolved target.`
    );
    await page.getByRole("button", { name: "仅允许本次" }).click();
    await page.getByRole("dialog", { name: "仅允许这一次？" }).waitFor();
    await page.getByRole("button", { name: "确认仅允许本次" }).click();
    await page.getByRole("button", { name: "返回模型与供应商" }).click();
    await page.getByRole("heading", { name: "模型与传输边界", level: 2 }).waitFor();
    check(
      (await page.getByText("本次连接验证成功", { exact: true }).count()) === 0,
      `${label}: approval must not automatically retest or claim success.`
    );

    await page.getByRole("button", { name: "测试连接" }).click();
    await page.getByRole("dialog", { name: "确认本次外部连接测试" }).waitFor();
    await page.getByRole("button", { name: "确认并测试" }).click();
    await page.getByText("本次连接验证成功", { exact: true }).waitFor();
    check(
      (await page.locator(".ol-settings-test-result .ol-status-label--success").count()) === 1,
      `${label}: trusted receipt should verify only the test result.`
    );
    check(await saveAction.isDisabled(), `${label}: successful testing must not imply a save.`);
    await page.getByRole("heading", { name: "连接测试结果", level: 2 }).scrollIntoViewIfNeeded();
    await page.screenshot({
      path: path.join(artifactDir, `phase4d_settings_${label}_validated_not_saved.png`),
      type: "png",
    });

    observations.push({ label, layout });
    await page.close();
  }

  const statePage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(statePage, "settings-state-matrix");
  await statePage.goto(baseUrl, { waitUntil: "networkidle" });

  await selectFixture(statePage, "fixture-settings-refresh-unknown");
  await openSettings(statePage);
  await editModel(statePage, "deepseek-reasoner");
  check(
    (await statePage.locator(".ol-settings-page .ol-status-label--success").count()) === 0,
    "dirty: unsaved external configuration must not retain a green boundary."
  );
  await statePage.getByRole("button", { name: "保存设置" }).click();
  await statePage.locator('[data-settings-phase="unknown"]').waitFor();
  check(
    (await statePage.getByText("保存后的边界仍未知", { exact: true }).count()) === 1,
    "refresh unknown: save must not manufacture a known provider boundary."
  );
  await statePage.screenshot({
    path: path.join(artifactDir, "phase4d_settings_1440x900_refresh_unknown.png"),
    type: "png",
  });

  await selectFixture(statePage, "fixture-settings-save-failed");
  await openSettings(statePage);
  await editModel(statePage, "deepseek-reasoner-failed-save");
  await statePage.getByRole("button", { name: "保存设置" }).click();
  await statePage.locator('[data-settings-phase="failed"]').waitFor();
  check(
    (await statePage.getByLabel("模型", { exact: true }).inputValue()) ===
      "deepseek-reasoner-failed-save",
    "save failure: draft must remain visible for correction or retry."
  );
  check(
    (await statePage.locator(".ol-settings-page .ol-status-label--success").count()) === 0,
    "save failure: current boundary must stay fail-closed."
  );
  await statePage.screenshot({
    path: path.join(artifactDir, "phase4d_settings_1440x900_save_failed.png"),
    type: "png",
  });

  await selectFixture(statePage, "fixture-settings-local-known");
  await openSettings(statePage);
  const search = statePage.getByRole("searchbox", { name: "搜索设置" });
  await search.fill("API 凭据");
  check(
    (await statePage.getByText("找到 1 个设置分类", { exact: true }).count()) === 1,
    "settings search: help terms must produce a perceptible result count."
  );
  check(
    (await statePage.getByRole("button", { name: /^模型与供应商/ }).count()) === 1,
    "settings search: model/provider category should match its help terms."
  );
  await statePage.getByRole("button", { name: "清除设置搜索" }).click();
  check(
    (await statePage.getByText("共 7 个设置分类", { exact: true }).count()) === 1,
    "settings search: icon clear action must restore all categories."
  );
  await statePage.getByRole("button", { name: /^隐私与网络/ }).click();
  await statePage.getByRole("heading", { name: "当前传输状态", level: 2 }).waitFor();
  check(
    (await statePage.getByText("本机路由", { exact: true }).count()) === 1 &&
      (await statePage.getByText("未发送到外部", { exact: true }).count()) === 1,
    "privacy surface: product copy must translate backend enums."
  );
  await statePage.getByRole("button", { name: "打开证据检查器" }).click();
  const inspector = statePage.getByRole("complementary", { name: "隐私与网络" });
  await inspector.waitFor();
  const inspectorText = await inspector.innerText();
  check(
    inspectorText.indexOf("发生了什么") < inspectorText.indexOf("技术检查信息"),
    "Inspector: user meaning must precede technical field provenance."
  );
  const tokens = await statePage.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return Object.fromEntries(
      [
        "--ol-ink",
        "--ol-ink-secondary",
        "--ol-ink-muted",
        "--ol-canvas",
        "--ol-sidebar",
        "--ol-surface-sunken",
        "--ol-amber",
        "--ol-amber-soft",
        "--ol-red",
        "--ol-red-soft",
        "--ol-green",
        "--ol-green-soft",
        "--ol-control-boundary",
        "--ol-focus",
      ].map(name => [name, style.getPropertyValue(name).trim()])
    );
  });
  const textContrastPairs = [
    [tokens["--ol-ink"], tokens["--ol-canvas"], "ink on canvas"],
    [tokens["--ol-ink-secondary"], tokens["--ol-sidebar"], "secondary on sidebar"],
    [tokens["--ol-ink-muted"], tokens["--ol-canvas"], "muted on canvas"],
    [tokens["--ol-amber"], tokens["--ol-amber-soft"], "amber on amber soft"],
    [tokens["--ol-red"], tokens["--ol-red-soft"], "red on red soft"],
    [tokens["--ol-green"], tokens["--ol-green-soft"], "green on green soft"],
  ];
  for (const [foreground, background, contrastLabel] of textContrastPairs) {
    const ratio = contrast(foreground, background);
    check(ratio >= 4.5, `${contrastLabel} contrast is ${ratio.toFixed(2)}:1, below 4.5:1.`);
    observations.push({ contrast: contrastLabel, ratio: Number(ratio.toFixed(2)) });
  }
  const nonTextContrastPairs = [
    [tokens["--ol-control-boundary"], tokens["--ol-canvas"], "control on canvas"],
    [tokens["--ol-control-boundary"], tokens["--ol-surface-sunken"], "control on sunken"],
    [tokens["--ol-focus"], tokens["--ol-canvas"], "focus on canvas"],
    [tokens["--ol-focus"], tokens["--ol-amber-soft"], "focus on amber"],
    [tokens["--ol-focus"], tokens["--ol-red-soft"], "focus on red"],
    [tokens["--ol-focus"], tokens["--ol-green-soft"], "focus on green"],
  ];
  for (const [foreground, background, contrastLabel] of nonTextContrastPairs) {
    const ratio = contrast(foreground, background);
    check(ratio >= 3, `${contrastLabel} contrast is ${ratio.toFixed(2)}:1, below 3:1.`);
    observations.push({ nonTextContrast: contrastLabel, ratio: Number(ratio.toFixed(2)) });
  }
  await statePage.screenshot({
    path: path.join(artifactDir, "phase4d_settings_1440x900_privacy_inspector.png"),
    type: "png",
  });
  await statePage.close();

  const keyboardPage = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(keyboardPage, "settings-keyboard");
  await keyboardPage.goto(baseUrl, { waitUntil: "networkidle" });
  await selectFixture(keyboardPage, "fixture-settings-review-required");
  await keyboardPage.locator("body").click({ position: { x: 2, y: 2 } });
  const settingsButton = keyboardPage.getByRole("button", { name: "设置", exact: true });
  if (await reachWithKeyboard(keyboardPage, settingsButton, "settings entry")) {
    await keyboardPage.keyboard.press("Enter");
    await keyboardPage.getByRole("heading", { name: "模型与供应商", level: 1 }).waitFor();
    const keyboardSearch = keyboardPage.getByRole("searchbox", { name: "搜索设置" });
    await reachWithKeyboard(keyboardPage, keyboardSearch, "settings search");
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
  path.join(artifactDir, "phase4d-settings-browser-qa.json"),
  `${JSON.stringify(report, null, 2)}\n`
);

if (failures.length > 0) {
  console.error(`Phase 4D settings QA failed (${failures.length} findings).`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Phase 4D settings QA passed: ${assertions} assertions across ${viewports.length} desktop viewports.`
);
