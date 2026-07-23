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
const artifactDir = path.resolve(repoRoot, "docs/phase4d_governed_action_spine/artifacts");
const baseUrl = process.env.OPENLIFE_PHASE4D_URL || "http://127.0.0.1:4186/dev/phase4d/";
const rejectedUrls = ["/", "/index.html", "/phase4d/"].map(pathname =>
  new URL(pathname, baseUrl).toString()
);
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
    `Unable to start Phase 4D QA server.${startupError ? ` ${startupError.message}` : ""}\n${output}`
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

async function reachWithKeyboard(page, locator, label) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
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
        `${label}: keyboard focus ring is not visibly at least 2px.`
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

  for (const rejectedUrl of rejectedUrls) {
    const response = await fetch(rejectedUrl, {
      headers: { Accept: "text/html" },
      redirect: "manual",
    });
    const body = await response.text();
    check(response.status === 404, `${rejectedUrl} must return 404, got ${response.status}.`);
    check(!body.includes("/src/main.tsx"), `${rejectedUrl} must not load the product App.`);
    observations.push({ rejectedUrl, status: response.status });
  }

  for (const viewport of viewports) {
    const label = `${viewport.width}x${viewport.height}`;
    const page = await browser.newPage({ viewport });
    watchPage(page, label);
    await page.goto(baseUrl, { waitUntil: "networkidle" });
    await page.getByText("整理下周客户访谈要验证的三个问题").waitFor();

    check(
      (await page
        .locator('[data-harness-marker="OPENLIFE_PHASE4D_GOVERNED_ACTION_HARNESS"]')
        .count()) === 1,
      `${label}: Phase 4D marker is missing.`
    );
    check(
      (await page.locator('.ol-nav-row[aria-current="page"]').count()) === 1,
      `${label}: exactly one product navigation item must be current.`
    );
    check(
      (await page.locator(".phase4d-qa-toolbar .phase4d-source-select").count()) === 1 &&
        (await page.locator(".ol-workbench-shell .phase4d-source-select").count()) === 0,
      `${label}: source selector must stay outside the product shell.`
    );

    const layout = await page.evaluate(() => {
      const sidebar = document.querySelector(".ol-shell-sidebar").getBoundingClientRect();
      const context = document.querySelector(".ol-shell-context-bar").getBoundingClientRect();
      const toolbar = document.querySelector(".phase4d-qa-toolbar").getBoundingClientRect();
      const shell = document.querySelector(".ol-workbench-shell").getBoundingClientRect();
      return {
        overflow:
          Math.max(document.documentElement.scrollWidth, document.body.scrollWidth) -
          window.innerWidth,
        sidebarWidth: sidebar.width,
        contextHeight: context.height,
        toolbarGap: shell.top - toolbar.bottom,
        bodyFont: Number.parseFloat(
          getComputedStyle(document.querySelector(".ol-readonly-reading")).fontSize
        ),
        metadataFont: Number.parseFloat(
          getComputedStyle(document.querySelector(".ol-readonly-page-heading > span")).fontSize
        ),
      };
    });
    check(layout.overflow <= 1, `${label}: horizontal overflow is ${layout.overflow}px.`);
    check(Math.abs(layout.sidebarWidth - 232) <= 0.5, `${label}: sidebar must be 232px.`);
    check(Math.abs(layout.contextHeight - 56) <= 0.5, `${label}: context bar must be 56px.`);
    check(Math.abs(layout.toolbarGap) <= 0.5, `${label}: QA toolbar overlaps the shell.`);
    check(layout.bodyFont >= 14, `${label}: reading text must be at least 14px.`);
    check(layout.metadataFont >= 12, `${label}: metadata must be at least 12px.`);

    const todayScreenshot = path.join(artifactDir, `phase4d_${label}_today_ready.png`);
    await page.screenshot({ path: todayScreenshot, type: "png" });

    await page.getByRole("button", { name: /^任务\s+队列与连续性/ }).click();
    await page.getByText("共 5 项，当前显示 5 项").waitFor();
    await page.getByRole("button", { name: /整理三次客户访谈，归纳下周要验证的问题/ }).click();
    await page
      .getByRole("heading", { name: "整理三次客户访谈，归纳下周要验证的问题", level: 2 })
      .waitFor();

    const inspectorLayout = await page.evaluate(() => {
      const main = document.querySelector(".ol-shell-main").getBoundingClientRect();
      const inspector = document.querySelector(".ol-shell-inspector").getBoundingClientRect();
      const taskRows = Array.from(document.querySelectorAll(".ol-readonly-task-row"));
      return {
        inspectorWidth: inspector.width,
        mainInspectorGap: inspector.left - main.right,
        inspectorRightGap: window.innerWidth - inspector.right,
        taskRowsOverflow: taskRows.some(row => row.scrollWidth > row.clientWidth + 1),
      };
    });
    check(
      Math.abs(inspectorLayout.inspectorWidth - 344) <= 0.5,
      `${label}: Inspector must be 344px.`
    );
    check(
      Math.abs(inspectorLayout.mainInspectorGap) <= 0.5,
      `${label}: main work overlaps Inspector.`
    );
    check(Math.abs(inspectorLayout.inspectorRightGap) <= 0.5, `${label}: Inspector is clipped.`);
    check(!inspectorLayout.taskRowsOverflow, `${label}: a task row overflows its stable width.`);
    check(
      await page
        .getByRole("heading", {
          name: "整理三次客户访谈，归纳下周要验证的问题",
          level: 2,
        })
        .evaluate(node => document.activeElement === node),
      `${label}: task selection must focus the Inspector heading.`
    );

    const tasksScreenshot = path.join(artifactDir, `phase4d_${label}_tasks_inspector.png`);
    await page.screenshot({ path: tasksScreenshot, type: "png" });

    await page.getByRole("button", { name: "关闭证据检查器" }).click();
    await page.getByRole("button", { name: /^工作区\s+当前执行/ }).click();
    await page.getByRole("heading", { name: "整理三次客户访谈，归纳下周要验证的问题" }).waitFor();
    check(
      await page.getByText("任务暂停在一个动作之前").isVisible(),
      `${label}: Workspace must expose the current blocker before actions.`
    );
    const workspaceScreenshot = path.join(
      artifactDir,
      `phase4d_${label}_workspace_permission_pending.png`
    );
    await page.screenshot({ path: workspaceScreenshot, type: "png" });

    await page.getByRole("button", { name: "查看权限请求" }).click();
    await page.getByRole("heading", { name: "读取本地客户访谈记录", level: 2 }).waitFor();
    const governedLayout = await page.evaluate(() => {
      const review = document.querySelector(".ol-review-layout");
      const queue = document.querySelector(".ol-review-queue");
      const detail = document.querySelector(".ol-review-detail");
      const main = document.querySelector(".ol-shell-main").getBoundingClientRect();
      const approve = Array.from(document.querySelectorAll("button"))
        .find(button => button.textContent?.trim() === "仅允许本次")
        ?.getBoundingClientRect();
      return {
        reviewOverflow: review.scrollWidth - review.clientWidth,
        queueWidth: queue.getBoundingClientRect().width,
        detailOverflow: detail.scrollWidth - detail.clientWidth,
        approvalVisible:
          Boolean(approve) && approve.top >= main.top && approve.bottom <= main.bottom + 1,
      };
    });
    check(
      governedLayout.reviewOverflow <= 1,
      `${label}: Review layout overflows its work surface.`
    );
    check(
      Math.abs(governedLayout.queueWidth - 248) <= 0.5,
      `${label}: Review queue must be 248px.`
    );
    check(governedLayout.detailOverflow <= 1, `${label}: Review detail content overflows.`);
    check(
      governedLayout.approvalVisible,
      `${label}: primary review decision must remain visible in the sticky action area.`
    );
    check(
      (await page.getByText("已允许一次").count()) === 0,
      `${label}: opening a permission request must not approve it.`
    );
    const reviewPendingScreenshot = path.join(
      artifactDir,
      `phase4d_${label}_review_permission_pending.png`
    );
    await page.screenshot({ path: reviewPendingScreenshot, type: "png" });
    observations.push({
      viewport: label,
      todayScreenshot: path.relative(repoRoot, todayScreenshot),
      tasksScreenshot: path.relative(repoRoot, tasksScreenshot),
      workspaceScreenshot: path.relative(repoRoot, workspaceScreenshot),
      reviewPendingScreenshot: path.relative(repoRoot, reviewPendingScreenshot),
      ...layout,
      ...inspectorLayout,
      ...governedLayout,
    });
    await page.close();
  }

  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  watchPage(page, "interaction");
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await page.getByText("整理下周客户访谈要验证的三个问题").waitFor();

  const tasksNav = page.getByRole("button", { name: /^任务\s+队列与连续性/ });
  if (await reachWithKeyboard(page, tasksNav, "Tasks navigation")) {
    await page.keyboard.press("Enter");
  }
  await page.getByText("共 5 项，当前显示 5 项").waitFor();
  check((await tasksNav.getAttribute("aria-current")) === "page", "Tasks must become current.");

  await page.getByLabel("筛选任务").selectOption("attention");
  check(
    await page.getByText("共 5 项，当前显示 3 项").isVisible(),
    "Attention filter must produce a visible result."
  );
  await page.getByLabel("筛选任务").selectOption("all");
  await page.getByLabel("搜索任务").fill("周报");
  check(
    await page.getByText("共 5 项，当前显示 1 项").isVisible(),
    "Task search must produce a visible result."
  );

  await page.getByRole("button", { name: /^工作区\s+当前执行/ }).click();
  await page.getByRole("heading", { name: "整理三次客户访谈，归纳下周要验证的问题" }).waitFor();
  await page.getByRole("button", { name: "查看权限请求" }).click();
  await page.getByRole("heading", { name: "读取本地客户访谈记录", level: 2 }).waitFor();
  const permissionReviewDetail = page.locator(
    '[data-review-item-id="review-permission-interview-notes"]'
  );
  check(
    (await permissionReviewDetail.getByText("已允许一次", { exact: true }).count()) === 0,
    "View action must not approve a review."
  );
  check(
    (await permissionReviewDetail.getByText("已应用", { exact: true }).count()) === 0,
    "View action must not apply a review."
  );

  const reviewActions = await page.locator('[data-action-category="review"]').evaluateAll(nodes =>
    nodes.map(node => ({
      id: node.getAttribute("data-action-id"),
      kind: node.getAttribute("data-action-kind"),
      effect: node.getAttribute("data-action-effect"),
      enabled: node.getAttribute("data-action-enabled"),
      disabledReason: node.getAttribute("data-action-disabled-reason"),
      targetRef: node.getAttribute("data-action-target-ref"),
      requiresConfirmation: node.getAttribute("data-action-requires-confirmation"),
      expectedMaterializationStatus: node.getAttribute(
        "data-action-expected-materialization-status"
      ),
      completionProofAfterDispatch: node.getAttribute(
        "data-action-completion-proof-after-dispatch"
      ),
    }))
  );
  check(reviewActions.length === 4, "Pending permission must expose four typed ReviewActions.");
  for (const action of reviewActions) {
    check(Boolean(action.id), "ReviewAction id is missing.");
    check(Boolean(action.kind), `${action.id}: ReviewAction kind is missing.`);
    check(Boolean(action.effect), `${action.id}: ReviewAction effect is missing.`);
    check(["true", "false"].includes(action.enabled), `${action.id}: enabled is invalid.`);
    check(action.disabledReason !== null, `${action.id}: disabledReason attribute is missing.`);
    check(
      action.targetRef === "review-permission-interview-notes",
      `${action.id}: target mismatch.`
    );
    check(
      Boolean(action.expectedMaterializationStatus),
      `${action.id}: expected materialization status is missing.`
    );
    check(
      action.completionProofAfterDispatch === "false",
      `${action.id}: command dispatch must not claim completion.`
    );
  }

  const approve = page.getByRole("button", { name: "仅允许本次" });
  await approve.click();
  const approvalDialog = page.getByRole("dialog", { name: "仅允许这一次？" });
  await approvalDialog.waitFor();
  check(
    (await page.getByText("已允许一次").count()) === 0,
    "Opening confirmation must not record approval."
  );
  check(
    await approvalDialog
      .getByRole("heading", { name: "仅允许这一次？" })
      .evaluate(node => document.activeElement === node),
    "Approval dialog must focus its title before presenting a high-impact decision."
  );
  const confirmApproval = approvalDialog.getByRole("button", { name: "确认仅允许本次" });
  await reachWithKeyboard(page, confirmApproval, "Permission approval confirmation");
  await confirmApproval.click();
  await page.getByText("决定已记录，尚未继续任务").waitFor();
  check(
    (await page.getByText("任务已完成").count()) === 0,
    "Approved permission must not be presented as task completion."
  );
  check(
    await page.getByText("已允许一次，尚未继续任务").isVisible(),
    "Approved permission must stay distinct from task resume."
  );
  const approvedScreenshot = path.join(
    artifactDir,
    "phase4d_1440x900_review_approved_not_resumed.png"
  );
  await page.screenshot({ path: approvedScreenshot, type: "png" });

  await page.getByRole("button", { name: "返回工作区" }).click();
  const resume = page.getByRole("button", { name: "继续任务" });
  await resume.waitFor();
  const resumeContract = await resume.evaluate(node => ({
    category: node.getAttribute("data-action-category"),
    id: node.getAttribute("data-action-id"),
    kind: node.getAttribute("data-action-kind"),
    effect: node.getAttribute("data-action-effect"),
    enabled: node.getAttribute("data-action-enabled"),
    targetRef: node.getAttribute("data-action-target-ref"),
    completionProofAfterDispatch: node.getAttribute("data-action-completion-proof-after-dispatch"),
  }));
  check(resumeContract.category === "task-control", "Resume must remain a TaskControl action.");
  check(resumeContract.id === "task-interview-notes:resume", "Resume control id mismatch.");
  check(resumeContract.kind === "resume", "Resume control kind mismatch.");
  check(resumeContract.effect === "task_resume_request", "Resume control effect mismatch.");
  check(resumeContract.enabled === "true", "Exact resume control must be enabled after refresh.");
  check(resumeContract.targetRef === "task-interview-notes", "Resume target mismatch.");
  check(
    resumeContract.completionProofAfterDispatch === "false",
    "Resume command must not claim completion."
  );
  await resume.click();
  const resumeDialog = page.getByRole("dialog", { name: "确认继续这项任务？" });
  await resumeDialog.waitFor();
  await resumeDialog.getByRole("button", { name: "确认继续" }).click();
  await page.getByText("任务已继续，正在处理").waitFor();
  check(
    await page.getByText("任务正在处理").isVisible(),
    "Refreshed exact task must render running after resume."
  );
  check(
    (await page.getByText("任务已完成").count()) === 0,
    "Running task must not be presented as completed."
  );
  const runningScreenshot = path.join(
    artifactDir,
    "phase4d_1440x900_workspace_resumed_running.png"
  );
  await page.screenshot({ path: runningScreenshot, type: "png" });

  await page.getByRole("button", { name: "设置" }).click();
  check(
    (await page.getByRole("navigation", { name: "设置分类" }).count()) === 1,
    "Settings must use a separate navigation context."
  );
  await page.getByRole("button", { name: "返回工作台" }).click();
  check(
    await page
      .getByRole("button", { name: "设置" })
      .evaluate(node => document.activeElement === node),
    "Settings Back must restore focus to the utility trigger."
  );

  await page.getByRole("button", { name: /^今日\s+当前关注/ }).click();
  await page.getByLabel("数据来源").selectOption("fixture-stale");
  await page.getByText("当前计划已陈旧，只读且不执行").waitFor();
  check(
    (await page.locator(".ol-status-label--success").count()) === 0,
    "Stale state must not render verified green."
  );
  check(
    await page.getByRole("button", { name: "打开工作区" }).isDisabled(),
    "Stale Today must disable workspace action."
  );
  const staleScreenshot = path.join(artifactDir, "phase4d_1440x900_today_stale.png");
  await page.screenshot({ path: staleScreenshot, type: "png" });

  await page.getByRole("button", { name: /^审核中心\s+建议与权限决定/ }).click();
  await page.getByText("审核状态已陈旧", { exact: true }).waitFor();
  check(
    await page.getByRole("button", { name: "仅允许本次" }).isDisabled(),
    "Stale Review must disable approval."
  );
  check(
    await page.getByRole("button", { name: "拒绝" }).isDisabled(),
    "Stale Review must disable rejection."
  );
  check(
    await page.getByRole("button", { name: "查看访问范围" }).isEnabled(),
    "Stale Review must preserve evidence access."
  );

  await page.getByLabel("数据来源").selectOption("fixture-incomplete-permission");
  await page.getByRole("button", { name: /^审核中心\s+建议与权限决定/ }).click();
  await page.getByText("访问范围不完整").waitFor();
  check(
    await page.getByRole("button", { name: "仅允许本次" }).isDisabled(),
    "Incomplete permission scope must disable approval."
  );
  check(
    await page.getByText("缺少目标范围和有效期；不能批准。").isVisible(),
    "Incomplete permission must expose the backend disabled reason."
  );
  const incompleteScreenshot = path.join(
    artifactDir,
    "phase4d_1440x900_review_incomplete_scope_blocked.png"
  );
  await page.screenshot({ path: incompleteScreenshot, type: "png" });

  await page.getByLabel("数据来源").selectOption("fixture-error");
  await page.getByText("今日状态读取失败").waitFor();
  check(
    (await page.locator(".ol-status-label--success").count()) === 0,
    "Error state must not render verified green."
  );
  await page.getByRole("button", { name: /^任务\s+队列与连续性/ }).click();
  await page.getByText("任务状态读取失败").waitFor();
  check(
    (await page.getByText(/共 0 项/).count()) === 0 &&
      (await page.getByText("当前没有可展示的任务。").count()) === 0,
    "Tasks error payload must not be presented as a confirmed empty list."
  );
  check(
    (await page.getByLabel("搜索任务").count()) === 0 &&
      (await page.getByLabel("筛选任务").count()) === 0,
    "Tasks error state must suppress normal list controls."
  );
  const tasksErrorScreenshot = path.join(
    artifactDir,
    "phase4d_1440x900_tasks_error_fail_closed.png"
  );
  await page.screenshot({ path: tasksErrorScreenshot, type: "png" });

  await page.getByRole("button", { name: /^今日\s+当前关注/ }).click();
  await page.getByLabel("数据来源").selectOption("fixture-empty");
  await page.getByText("今天还没有明确重点").waitFor();
  check(
    await page.getByText(/没有后端提供的目标时/).isVisible(),
    "Empty Today must remain explicit without generated placeholder truth."
  );

  await page.getByLabel("数据来源").selectOption("fixture-ready");
  await page.getByText("整理下周客户访谈要验证的三个问题").waitFor();
  const actionContracts = await page.locator("[data-action-id]").evaluateAll(nodes =>
    nodes.map(node => ({
      category: node.getAttribute("data-action-category"),
      id: node.getAttribute("data-action-id"),
      kind: node.getAttribute("data-action-kind"),
      enabled: node.getAttribute("data-action-enabled"),
      disabledReason: node.getAttribute("data-action-disabled-reason"),
      targetRef: node.getAttribute("data-action-target-ref"),
    }))
  );
  check(actionContracts.length > 0, "The current product view must expose action contracts.");
  for (const action of actionContracts) {
    check(action.category === "product", `${action.id}: action category must be product.`);
    check(Boolean(action.id), "Action id is missing.");
    check(Boolean(action.kind), `${action.id}: action kind is missing.`);
    check(["true", "false"].includes(action.enabled), `${action.id}: enabled is invalid.`);
    check(action.disabledReason !== null, `${action.id}: disabledReason attribute is missing.`);
    check(Boolean(action.targetRef), `${action.id}: targetRef is missing.`);
  }
  const tokens = await page.evaluate(() => {
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
    [tokens["--ol-amber"], tokens["--ol-amber-soft"], "amber on protection"],
    [tokens["--ol-red"], tokens["--ol-red-soft"], "red on error"],
    [tokens["--ol-green"], tokens["--ol-green-soft"], "green on verified"],
  ]) {
    const ratio = contrast(foreground, background);
    check(ratio >= 4.5, `${label} contrast is ${ratio.toFixed(2)}:1, below 4.5:1.`);
    observations.push({ contrast: label, ratio: Number(ratio.toFixed(2)) });
  }
  for (const [foreground, background, label] of [
    [tokens["--ol-control-boundary"], tokens["--ol-canvas"], "control boundary"],
    [tokens["--ol-focus"], tokens["--ol-canvas"], "focus on canvas"],
  ]) {
    const ratio = contrast(foreground, background);
    check(ratio >= 3, `${label} contrast is ${ratio.toFixed(2)}:1, below 3:1.`);
    observations.push({ nonTextContrast: label, ratio: Number(ratio.toFixed(2)) });
  }

  observations.push({
    interaction: "review-approved-not-resumed",
    screenshot: path.relative(repoRoot, approvedScreenshot),
  });
  observations.push({
    interaction: "workspace-resumed-running",
    screenshot: path.relative(repoRoot, runningScreenshot),
  });
  observations.push({
    interaction: "review-incomplete-scope-blocked",
    screenshot: path.relative(repoRoot, incompleteScreenshot),
  });
  observations.push({
    interaction: "today-stale",
    screenshot: path.relative(repoRoot, staleScreenshot),
  });
  observations.push({
    interaction: "tasks-error-fail-closed",
    screenshot: path.relative(repoRoot, tasksErrorScreenshot),
  });
  await page.close();
} catch (error) {
  failures.push(error instanceof Error ? (error.stack ?? error.message) : String(error));
} finally {
  await browser?.close();
  await stopServer(qaServer);
}

failures.push(...browserErrors);
const report = {
  generatedAt: new Date().toISOString(),
  scope: "desktop_tauri_only",
  narrowScreenAcceptance: false,
  baseUrl,
  assertions,
  result: failures.length === 0 ? "PASS" : "FAIL",
  failures,
  observations,
};
const reportPath = path.join(artifactDir, "phase4d-browser-qa.json");
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

if (failures.length > 0) {
  console.error(failures.map(failure => `- ${failure}`).join("\n"));
  console.error(`QA report: ${reportPath}`);
  process.exit(1);
}

console.log(`Phase 4D desktop browser QA passed: ${assertions} assertions.`);
console.log(`QA report: ${reportPath}`);
