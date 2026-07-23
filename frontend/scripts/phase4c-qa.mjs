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
const artifactDir = path.resolve(frontendRoot, "../docs/phase4c_desktop_shell_harness/artifacts");
const baseUrl = process.env.OPENLIFE_PHASE4C_URL || "http://127.0.0.1:4185/dev/phase4c/";
const rejectedUiUrls = ["/", "/index.html", "/phase4c/"].map(pathname =>
  new URL(pathname, baseUrl).toString()
);
const viewports = [
  { width: 1440, height: 900 },
  { width: 1280, height: 800 },
];

const failures = [];
const browserErrors = [];
const observations = [];
let assertions = 0;

function delay(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

function check(condition, message) {
  assertions += 1;
  if (!condition) failures.push(message);
}

async function endpointAvailable() {
  try {
    const response = await fetch(baseUrl, { signal: AbortSignal.timeout(1000) });
    return response.ok;
  } catch {
    return false;
  }
}

async function startQaServer() {
  if (await endpointAvailable()) return null;

  const url = new URL(baseUrl);
  const viteEntry = path.join(frontendRoot, "node_modules/vite/bin/vite.js");
  const server = spawn(
    process.execPath,
    [
      viteEntry,
      "--config",
      "vite.phase4c.config.ts",
      "--host",
      url.hostname,
      "--port",
      url.port || "4185",
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

  await stopQaServer(server);
  throw new Error(
    `Unable to start the Phase 4C QA server.${startupError ? ` ${startupError.message}` : ""}\n${output}`
  );
}

async function stopQaServer(server) {
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

async function reachWithKeyboard(page, locator, label) {
  let reached = false;
  for (let attempt = 0; !reached && attempt < 100; attempt += 1) {
    await page.keyboard.press("Tab");
    reached = await locator.evaluate(node => document.activeElement === node);
  }

  check(reached, `${label}: target is not reachable in the forward keyboard tab order.`);
  if (!reached) return false;

  const focusStyle = await locator.evaluate(node => {
    const style = getComputedStyle(node);
    return {
      active: document.activeElement === node,
      visible: node.matches(":focus-visible"),
      outlineStyle: style.outlineStyle,
      outlineWidth: Number.parseFloat(style.outlineWidth),
    };
  });
  check(focusStyle.active, `${label}: target did not retain keyboard focus.`);
  check(
    focusStyle.visible && focusStyle.outlineStyle !== "none" && focusStyle.outlineWidth >= 2,
    `${label}: target does not expose the required visible focus ring.`
  );
  return true;
}

async function activateWithKeyboard(page, locator, key, label) {
  if (!(await reachWithKeyboard(page, locator, label))) return;
  await page.keyboard.press(key);
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

mkdirSync(artifactDir, { recursive: true });
const qaServer = await startQaServer();
let browser;

try {
  browser = await chromium.launch({ headless: true });

  for (const rejectedUiUrl of rejectedUiUrls) {
    const response = await fetch(rejectedUiUrl, {
      headers: { Accept: "text/html" },
      redirect: "manual",
    });
    const body = await response.text();
    check(response.status === 404, `${rejectedUiUrl} must return 404, got ${response.status}.`);
    check(!body.includes("/src/main.tsx"), `${rejectedUiUrl} must not load the product App.`);
    observations.push({ rejectedUiUrl, status: response.status, productEntryPresent: false });
  }

  for (const viewport of viewports) {
    const label = `${viewport.width}x${viewport.height}`;
    const page = await browser.newPage({ viewport });
    watchPage(page, label);
    await page.goto(baseUrl, { waitUntil: "networkidle" });

    check(
      (await page
        .locator('[data-harness-marker="OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS"]')
        .count()) === 1,
      `${label}: Phase 4C harness marker is missing.`
    );
    const dimensions = await page.evaluate(() => {
      const sidebar = document.querySelector(".ol-shell-sidebar").getBoundingClientRect();
      const context = document.querySelector(".ol-shell-context-bar").getBoundingClientRect();
      const shell = document.querySelector(".ol-workbench-shell").getBoundingClientRect();
      const toolbar = document.querySelector(".phase4c-qa-toolbar").getBoundingClientRect();
      return {
        overflow:
          Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
          window.innerWidth,
        bodyFont: Number.parseFloat(
          getComputedStyle(document.querySelector(".ol-workbench-shell")).fontSize
        ),
        metadataFont: Number.parseFloat(
          getComputedStyle(document.querySelector(".phase4c-page-heading > span")).fontSize
        ),
        sidebarWidth: sidebar.width,
        contextHeight: context.height,
        toolbarShellGap: shell.top - toolbar.bottom,
      };
    });
    check(dimensions.overflow <= 1, `${label}: horizontal overflow is ${dimensions.overflow}px.`);
    check(dimensions.bodyFont === 14, `${label}: body type must resolve to 14px.`);
    check(dimensions.metadataFont >= 12, `${label}: metadata must be at least 12px.`);
    check(Math.abs(dimensions.sidebarWidth - 232) <= 0.5, `${label}: sidebar must be 232px.`);
    check(Math.abs(dimensions.contextHeight - 56) <= 0.5, `${label}: context bar must be 56px.`);
    check(Math.abs(dimensions.toolbarShellGap) <= 0.5, `${label}: QA toolbar overlaps the shell.`);
    check(
      (await page.locator('.ol-nav-row[aria-current="page"]').count()) === 1,
      `${label}: exactly one navigation item must expose aria-current=page.`
    );

    const screenshotPath = path.join(artifactDir, `phase4c_${label}_today.png`);
    await page.screenshot({ path: screenshotPath, type: "png" });

    await page.getByRole("button", { name: "打开证据检查器" }).click();
    const openLayout = await page.evaluate(() => {
      const sidebar = document.querySelector(".ol-shell-sidebar").getBoundingClientRect();
      const main = document.querySelector(".ol-shell-main").getBoundingClientRect();
      const inspector = document.querySelector(".ol-shell-inspector").getBoundingClientRect();
      return {
        inspectorWidth: inspector.width,
        sidebarMainGap: main.left - sidebar.right,
        mainInspectorGap: inspector.left - main.right,
        inspectorRightGap: window.innerWidth - inspector.right,
      };
    });
    check(
      Math.abs(openLayout.inspectorWidth - 344) <= 0.5,
      `${label}: open Inspector must be 344px.`
    );
    check(Math.abs(openLayout.sidebarMainGap) <= 0.5, `${label}: sidebar overlaps main work.`);
    check(Math.abs(openLayout.mainInspectorGap) <= 0.5, `${label}: main overlaps Inspector.`);
    check(Math.abs(openLayout.inspectorRightGap) <= 0.5, `${label}: Inspector is clipped.`);
    check(
      await page
        .getByRole("heading", { name: "今日计划依据" })
        .evaluate(node => document.activeElement === node),
      `${label}: opening Inspector must focus its heading.`
    );

    const inspectorPath = path.join(artifactDir, `phase4c_${label}_today_inspector.png`);
    await page.screenshot({ path: inspectorPath, type: "png" });
    observations.push({
      viewport: label,
      screenshot: path.relative(repoRoot, screenshotPath),
      inspectorScreenshot: path.relative(repoRoot, inspectorPath),
      ...dimensions,
      ...openLayout,
    });
    await page.close();
  }

  const interaction = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(interaction, "interaction");
  await interaction.goto(baseUrl, { waitUntil: "networkidle" });

  const skipLink = interaction.getByRole("link", { name: "跳到主工作区" });
  await activateWithKeyboard(interaction, skipLink, "Enter", "Skip link");
  check(
    await interaction.locator("#ol-shell-main").evaluate(node => document.activeElement === node),
    "Activating the skip link must move focus to the main work surface."
  );

  check(
    (await interaction.locator('[aria-live="polite"], [role="status"]').count()) === 1,
    "Dynamic shell feedback must have exactly one polite live region."
  );
  check(
    (await interaction.locator(".phase4c-qa-feedback[role=status]").count()) === 0,
    "Visible feedback must not duplicate the live-region announcement."
  );
  check(
    (await interaction
      .locator(".mobile-nav, .bottom-nav, .mobile-drawer, .bottom-sheet")
      .count()) === 0,
    "Desktop Shell must not render mobile navigation or sheet controls."
  );

  await activateWithKeyboard(
    interaction,
    interaction.getByRole("button", { name: "打开证据检查器" }),
    "Enter",
    "Inspector before navigation"
  );
  const tasksNav = interaction.getByRole("button", { name: /^任务\s+队列与连续性/ });
  await activateWithKeyboard(interaction, tasksNav, "Enter", "Tasks navigation");
  check(
    (await interaction.locator(".ol-shell-inspector").count()) === 0,
    "Product navigation must close the previous Inspector."
  );
  check(
    (await tasksNav.getAttribute("aria-current")) === "page",
    "Tasks navigation must become current even while the page is unavailable."
  );
  check(
    await interaction.getByRole("heading", { name: "任务页面尚未迁移" }).isVisible(),
    "Tasks must show an explicit unavailable page."
  );
  check(
    await interaction
      .getByRole("heading", { name: "任务", exact: true })
      .evaluate(node => document.activeElement === node),
    "Product navigation must move focus to the context heading."
  );

  const settingsTrigger = interaction.getByRole("button", { name: "设置" });
  await activateWithKeyboard(interaction, settingsTrigger, "Enter", "Settings utility");
  check(
    (await interaction.getByRole("navigation", { name: "设置分类" }).count()) === 1,
    "Settings must replace product navigation with a dedicated context."
  );
  const settingsScreenshot = path.join(artifactDir, "phase4c_1440x900_settings.png");
  await interaction.screenshot({ path: settingsScreenshot, type: "png" });

  const settingsSearch = interaction.getByRole("searchbox", { name: "搜索设置" });
  await settingsSearch.fill("隐私");
  check(
    (await interaction.locator(".ol-shell-settings-navigation .ol-nav-row").count()) === 1,
    "Settings search must visibly filter the category list."
  );
  await settingsSearch.fill("");
  const backButton = interaction.getByRole("button", { name: "返回工作台" });
  await activateWithKeyboard(interaction, backButton, "Enter", "Settings Back");
  check(
    await interaction
      .getByRole("button", { name: "设置" })
      .evaluate(node => document.activeElement === node),
    "Settings Back must restore focus to its utility trigger."
  );

  await interaction.getByRole("button", { name: /^今日\s+每日关注/ }).click();
  const reviewTrigger = interaction.getByRole("button", { name: "查看待审核建议" });
  await activateWithKeyboard(interaction, reviewTrigger, "Enter", "Pending review entry");
  check(
    await interaction.getByRole("heading", { name: "出差前保留准备时间" }).isVisible(),
    "Viewing a pending suggestion must open the pending decision state."
  );
  check(
    (await interaction.getByRole("heading", { name: "已批准，尚未应用" }).count()) === 0,
    "Viewing a suggestion must not approve it."
  );
  await interaction.getByRole("button", { name: "打开证据检查器" }).click();
  await interaction.getByRole("button", { name: /建议来源样例/ }).click();
  check(
    await interaction.getByText(/已选择证据 evidence_review_proposal_fixture/).isVisible(),
    "Pending review evidence selection must be visible before a decision."
  );
  await activateWithKeyboard(
    interaction,
    interaction.getByRole("button", { name: "批准变更" }),
    "Enter",
    "Review approval"
  );
  check(
    await interaction.getByRole("dialog", { name: "确认批准这条建议" }).isVisible(),
    "Review approval must require confirmation."
  );
  await activateWithKeyboard(
    interaction,
    interaction.getByRole("button", { name: "确认批准" }),
    "Enter",
    "Review confirmation"
  );
  check(
    await interaction.getByRole("heading", { name: "已批准，尚未应用" }).isVisible(),
    "Approval must remain visibly distinct from application."
  );
  check(
    await interaction.getByRole("button", { name: "应用变更" }).isDisabled(),
    "Unsupported materialization must stay disabled."
  );
  check(
    (await interaction.getByText("已完成").count()) === 0,
    "Approved fixture must never show completed."
  );
  check(
    (await interaction.getByText(/evidence_review_proposal_fixture/).count()) === 0,
    "Approval must clear evidence selected from the previous pending state."
  );
  await interaction.getByRole("button", { name: "关闭证据检查器" }).click();
  const reviewScreenshot = path.join(artifactDir, "phase4c_1440x900_review_approved.png");
  await interaction.screenshot({ path: reviewScreenshot, type: "png" });

  const inspectorTrigger = interaction.getByRole("button", { name: "打开证据检查器" });
  await activateWithKeyboard(interaction, inspectorTrigger, "Enter", "Inspector trigger");
  check(
    await interaction
      .getByRole("heading", { name: "批准与应用状态" })
      .evaluate(node => document.activeElement === node),
    "Inspector must move focus to its heading."
  );
  await interaction.getByRole("button", { name: /批准决定样例/ }).click();
  check(
    await interaction.getByText(/已选择证据 evidence_approval_fixture/).isVisible(),
    "Evidence rows must produce visible structured feedback."
  );
  await activateWithKeyboard(
    interaction,
    interaction.getByRole("button", { name: "关闭证据检查器" }),
    "Enter",
    "Inspector close"
  );
  check(
    await interaction
      .getByRole("button", { name: "打开证据检查器" })
      .evaluate(node => document.activeElement === node),
    "Inspector close must restore focus to its trigger."
  );

  await interaction.getByLabel("布局状态").selectOption("safe-mode");
  check(
    (await interaction.locator(".ol-status-label--success").count()) === 0,
    "Unknown safe mode must not use verified-success green."
  );
  check(
    await interaction.getByRole("button", { name: "执行外部动作" }).isDisabled(),
    "Unknown privacy evidence must fail closed."
  );
  const safeModeScreenshot = path.join(artifactDir, "phase4c_1440x900_safe_mode.png");
  await interaction.screenshot({ path: safeModeScreenshot, type: "png" });

  const actionContracts = await interaction.locator("[data-action-id]").evaluateAll(nodes =>
    nodes.map(node => ({
      id: node.getAttribute("data-action-id"),
      kind: node.getAttribute("data-action-kind"),
      enabled: node.getAttribute("data-action-enabled"),
      disabledReason: node.getAttribute("data-action-disabled-reason"),
      targetRef: node.getAttribute("data-action-target-ref"),
      confirmation: node.getAttribute("data-action-confirmation"),
      materialization: node.getAttribute("data-action-materialization"),
    }))
  );
  check(actionContracts.length >= 2, "Safe mode must expose fixture action contracts.");
  for (const contract of actionContracts) {
    check(Boolean(contract.id), "Action Contract is missing id.");
    check(["product", "review", "debug"].includes(contract.kind), `${contract.id}: invalid kind.`);
    check(["true", "false"].includes(contract.enabled), `${contract.id}: invalid enabled value.`);
    check(contract.disabledReason !== null, `${contract.id}: disabledReason attribute is missing.`);
    check(Boolean(contract.targetRef), `${contract.id}: targetRef is missing.`);
    check(Boolean(contract.confirmation), `${contract.id}: confirmation semantics are missing.`);
    check(
      Boolean(contract.materialization),
      `${contract.id}: materialization semantics are missing.`
    );
  }

  const tokens = await interaction.evaluate(() => {
    const style = getComputedStyle(document.documentElement);
    return Object.fromEntries(
      [
        "--ol-ink",
        "--ol-ink-secondary",
        "--ol-ink-muted",
        "--ol-canvas",
        "--ol-sidebar",
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
  for (const [foreground, background, label] of [
    [tokens["--ol-ink"], tokens["--ol-canvas"], "ink on canvas"],
    [tokens["--ol-ink-secondary"], tokens["--ol-sidebar"], "secondary on sidebar"],
    [tokens["--ol-ink-muted"], tokens["--ol-canvas"], "muted on canvas"],
    [tokens["--ol-amber"], tokens["--ol-amber-soft"], "amber on amber soft"],
    [tokens["--ol-red"], tokens["--ol-red-soft"], "red on red soft"],
    [tokens["--ol-green"], tokens["--ol-green-soft"], "green on green soft"],
  ]) {
    const ratio = contrast(foreground, background);
    check(ratio >= 4.5, `${label} contrast is ${ratio.toFixed(2)}:1, below 4.5:1.`);
    observations.push({ contrast: label, ratio: Number(ratio.toFixed(2)) });
  }
  for (const [foreground, background, label] of [
    [tokens["--ol-control-boundary"], tokens["--ol-canvas"], "control boundary"],
    [tokens["--ol-focus"], tokens["--ol-canvas"], "focus on canvas"],
    [tokens["--ol-focus"], tokens["--ol-amber-soft"], "focus on amber"],
  ]) {
    const ratio = contrast(foreground, background);
    check(ratio >= 3, `${label} contrast is ${ratio.toFixed(2)}:1, below 3:1.`);
    observations.push({ nonTextContrast: label, ratio: Number(ratio.toFixed(2)) });
  }

  observations.push({
    interaction: "settings",
    screenshot: path.relative(repoRoot, settingsScreenshot),
  });
  observations.push({
    interaction: "review-approved",
    screenshot: path.relative(repoRoot, reviewScreenshot),
  });
  observations.push({
    interaction: "safe-mode",
    screenshot: path.relative(repoRoot, safeModeScreenshot),
  });
  await interaction.close();
} finally {
  await browser?.close();
  await stopQaServer(qaServer);
}

if (browserErrors.length) failures.push(...browserErrors);

const report = {
  generatedAt: new Date().toISOString(),
  scope: "desktop_tauri_only",
  mobileAcceptance: false,
  baseUrl,
  assertions,
  result: failures.length === 0 ? "PASS" : "FAIL",
  failures,
  observations,
};
const reportPath = path.join(artifactDir, "phase4c-browser-qa.json");
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

if (failures.length) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  console.error(`QA report: ${reportPath}`);
  process.exit(1);
}

console.log(`Phase 4C desktop browser QA passed: ${assertions} assertions.`);
console.log(`QA report: ${reportPath}`);
