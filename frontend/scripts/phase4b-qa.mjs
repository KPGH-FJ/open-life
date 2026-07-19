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
const artifactDir = path.resolve(frontendRoot, "../docs/phase4b_ui_foundation_harness/artifacts");
const baseUrl = process.env.OPENLIFE_PHASE4B_URL || "http://127.0.0.1:4184/dev/phase4b/";
const viewports = [
  { width: 1440, height: 900 },
  { width: 1280, height: 800 },
  { width: 390, height: 844 },
];

const failures = [];
const browserErrors = [];
const observations = [];
let assertions = 0;

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

async function startQaServer() {
  if (await endpointAvailable()) return null;

  const url = new URL(baseUrl);
  const viteEntry = path.join(frontendRoot, "node_modules/vite/bin/vite.js");
  const server = spawn(
    process.execPath,
    [
      viteEntry,
      "--config",
      "vite.phase4b.config.ts",
      "--host",
      url.hostname,
      "--port",
      url.port || "4184",
      "--strictPort",
    ],
    {
      cwd: frontendRoot,
      stdio: ["ignore", "pipe", "pipe"],
    }
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
    if (startupError) break;
    if (server.exitCode !== null) break;
    if (await endpointAvailable()) return server;
    await delay(100);
  }

  await stopQaServer(server);
  throw new Error(
    `Unable to start the Phase 4B QA server.${startupError ? ` ${startupError.message}` : ""}\n${output}`
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

mkdirSync(artifactDir, { recursive: true });
const qaServer = await startQaServer();
let browser;

try {
  browser = await chromium.launch({ headless: true });
  for (const viewport of viewports) {
    const label = `${viewport.width}x${viewport.height}`;
    const page = await browser.newPage({ viewport });
    watchPage(page, label);
    await page.goto(baseUrl, { waitUntil: "networkidle" });

    check(
      (await page.locator('[data-harness-marker="OPENLIFE_PHASE4B_DEV_HARNESS"]').count()) === 1,
      `${label}: dev-only harness marker is missing.`
    );

    const dimensions = await page.evaluate(() => ({
      overflow:
        Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
        window.innerWidth,
      bodyFont: Number.parseFloat(
        getComputedStyle(document.querySelector(".ol-foundation")).fontSize
      ),
      metadataFont: Number.parseFloat(
        getComputedStyle(document.querySelector(".phase4b-eyebrow")).fontSize
      ),
    }));
    check(dimensions.overflow <= 1, `${label}: horizontal overflow is ${dimensions.overflow}px.`);
    check(dimensions.bodyFont === 14, `${label}: body type must resolve to 14px.`);
    check(dimensions.metadataFont >= 12, `${label}: metadata type must be at least 12px.`);

    const currentNavCount = await page.locator('.ol-nav-row[aria-current="page"]').count();
    check(currentNavCount >= 1, `${label}: current navigation must expose aria-current=page.`);

    if (viewport.width <= 640) {
      const targetHeights = await page
        .locator(
          ".ol-action-button:visible, .ol-icon-button:visible, .ol-toggle:visible, .ol-nav-row:visible, .ol-evidence-row:visible"
        )
        .evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().height));
      check(targetHeights.length > 0, `${label}: no mobile interaction targets were found.`);
      check(
        targetHeights.every(height => height >= 44),
        `${label}: every visible interaction target must be at least 44px tall.`
      );
    }

    const screenshotPath = path.join(artifactDir, `phase4b_${label}.png`);
    await page.screenshot({ path: screenshotPath, type: "png" });
    observations.push({
      viewport: label,
      screenshot: path.relative(repoRoot, screenshotPath),
      ...dimensions,
    });
    await page.close();
  }

  const interaction = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(interaction, "interaction");
  await interaction.goto(baseUrl, { waitUntil: "networkidle" });

  const approveTrigger = interaction.getByRole("button", { name: "批准样例" });
  await approveTrigger.focus();
  await interaction.keyboard.press("Enter");
  const dialog = interaction.getByRole("dialog", { name: "确认批准布局样例" });
  check(await dialog.isVisible(), "Approval must open a visible modal dialog.");
  check(
    await interaction
      .getByRole("heading", { name: "确认批准布局样例" })
      .evaluate(node => document.activeElement === node),
    "Opening a dialog must move focus into its heading."
  );
  check(
    await interaction
      .locator("#root")
      .evaluate(node => node.inert && node.getAttribute("aria-hidden") === "true"),
    "Dialog background must be inert and hidden from the accessibility tree."
  );
  await interaction.keyboard.press("Escape");
  check((await dialog.count()) === 0, "Escape must close a non-busy dialog.");
  check(
    await approveTrigger.evaluate(node => document.activeElement === node),
    "Closing a dialog must restore focus to its trigger."
  );

  await approveTrigger.click();
  await interaction.getByRole("button", { name: "确认批准" }).click();
  check(
    (await interaction.locator(".phase4b-page-heading").textContent())?.includes(
      "已批准，尚未应用"
    ),
    "Approval must remain distinct from applied/completed."
  );
  check(
    (await interaction.locator(".phase4b-feedback").textContent())?.includes(
      "尚未应用，也未写入长期状态"
    ),
    "Approval feedback must deny materialization and durable writes."
  );

  await interaction.getByRole("button", { name: "查看依据" }).click();
  check(
    (await interaction.locator(".phase4b-feedback").textContent())?.includes(
      "没有改变任何产品状态"
    ),
    "Evidence action must produce visible feedback without changing state."
  );
  await interaction.getByRole("button", { name: /任务/ }).click();
  check(
    (await interaction.locator(".phase4b-feedback").textContent())?.includes("尚未迁移"),
    "Unavailable navigation must produce explicit visible feedback."
  );
  check(
    await interaction.getByRole("button", { name: "应用变更" }).isDisabled(),
    "Unsupported materialization must remain disabled."
  );
  check(
    (await interaction.locator(".ol-disabled-reason").textContent())?.includes(
      "批准不能显示为已完成"
    ),
    "Disabled controls must expose a visible reason."
  );
  check(
    (await interaction.locator(".ol-status-label--unknown.ol-status-label--success").count()) === 0,
    "Unknown state must never use the verified-success presentation."
  );

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
      ].map(name => [name, style.getPropertyValue(name).trim()])
    );
  });
  const contrastPairs = [
    [tokens["--ol-ink"], tokens["--ol-canvas"], "ink on canvas"],
    [tokens["--ol-ink-secondary"], tokens["--ol-sidebar"], "secondary on sidebar"],
    [tokens["--ol-ink-muted"], tokens["--ol-canvas"], "muted on canvas"],
    [tokens["--ol-amber"], tokens["--ol-amber-soft"], "amber on amber soft"],
    [tokens["--ol-red"], tokens["--ol-red-soft"], "red on red soft"],
    [tokens["--ol-green"], tokens["--ol-green-soft"], "green on green soft"],
  ];
  for (const [foreground, background, label] of contrastPairs) {
    const ratio = contrast(foreground, background);
    check(ratio >= 4.5, `${label} contrast is ${ratio.toFixed(2)}:1, below 4.5:1.`);
    observations.push({ contrast: label, ratio: Number(ratio.toFixed(2)) });
  }
  await interaction.close();

  const mobileDialog = await browser.newPage({ viewport: { width: 390, height: 844 } });
  watchPage(mobileDialog, "mobile-dialog");
  await mobileDialog.goto(baseUrl, { waitUntil: "networkidle" });
  await mobileDialog.getByRole("button", { name: "批准样例" }).click();
  const dialogBox = await mobileDialog.getByRole("dialog").boundingBox();
  check(Boolean(dialogBox), "Mobile approval must expose a dialog.");
  check(
    Boolean(dialogBox && Math.abs(dialogBox.y + dialogBox.height - 844) <= 1),
    "Mobile dialog must be anchored to the viewport bottom."
  );
  const mobileDialogPath = path.join(artifactDir, "phase4b_390x844_dialog.png");
  await mobileDialog.screenshot({ path: mobileDialogPath, type: "png" });
  observations.push({
    interaction: "mobile-dialog",
    screenshot: path.relative(repoRoot, mobileDialogPath),
  });
  await mobileDialog.close();
} finally {
  await browser?.close();
  await stopQaServer(qaServer);
}

if (browserErrors.length) failures.push(...browserErrors);

const report = {
  generatedAt: new Date().toISOString(),
  baseUrl,
  assertions,
  result: failures.length === 0 ? "PASS" : "FAIL",
  failures,
  observations,
};
const reportPath = path.join(artifactDir, "phase4b-browser-qa.json");
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

if (failures.length) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  console.error(`QA report: ${reportPath}`);
  process.exit(1);
}

console.log(`Phase 4B browser QA passed: ${assertions} assertions.`);
console.log(`QA report: ${reportPath}`);
