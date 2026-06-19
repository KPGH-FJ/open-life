#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(frontendRoot, "..");
const STAGE1_DOGFOOD_SCENARIOS = loadStage1DogfoodScenarios();
const requiredJourneys = STAGE1_DOGFOOD_SCENARIOS.map(scenario => scenario.id);
const REQUIRED_STAGE1_PREP_TASK_IDS = [
  "D13",
  "D14",
  "D15",
  "D19",
  "D20",
  "D27",
  "D28",
  "D35",
  "D36",
];

const unavailableBlockers = [
  "real_tauri_browser_command_surface_unavailable",
  "tauri_webdriver_environment_not_ready",
];

const macosBlocker = "tauri_webdriver_macos_not_supported_by_tauri_driver";
const reportPath = "frontend/test-results/main-chat-stage1-dogfood-report.json";
const webdriverUrl = "http://127.0.0.1:4444";
const frontendDevUrl = "http://127.0.0.1:5173";

if (process.argv.includes("--validate-scenarios-only")) {
  console.log(`validated_stage1_dogfood_scenarios=${STAGE1_DOGFOOD_SCENARIOS.length}`);
  process.exit(0);
}

main().catch(error => {
  const blocker = metadataSafeBlocker(`tauri_webdriver_runner_error:${error?.message ?? error}`);
  writeBlockedReport([blocker]);
  console.error(`Stage 1 Tauri WebDriver E2E failed: ${blocker}`);
  process.exit(1);
});

async function main() {
  const preflight = buildPreflight();
  if (!preflight.ready) {
    const blockers = uniqueValues([...unavailableBlockers, ...preflight.blockers]);
    writeBlockedReport(blockers);
    console.error(
      [
        "Stage 1 Tauri WebDriver E2E did not run.",
        `platform=${process.platform}`,
        `supportedPlatform=${String(preflight.supportedPlatform)}`,
        `ready=${String(preflight.ready)}`,
        `blockers=${blockers.join(",")}`,
      ].join("\n")
    );
    process.exit(1);
  }

  const result = await runStage1TauriDogfood();
  if (result.ready) {
    console.error(
      [
        "Stage 1 Tauri WebDriver observed D01-D36.",
        `sessionCreated=${String(result.sessionCreated)}`,
        `observedCount=${String(result.observedCount ?? 0)}`,
        `readinessRecommendation=${String(result.readinessRecommendation ?? "")}`,
      ].join("\n")
    );
    process.exit(0);
  }

  writeBlockedReport(result.blockers, result.observedScenarios ?? []);
  console.error(
    [
      "Stage 1 Tauri WebDriver ran but D01-D36 did not complete.",
      `sessionCreated=${String(result.sessionCreated)}`,
      `observedCount=${String(result.observedCount ?? 0)}`,
      `blockers=${result.blockers.join(",")}`,
    ].join("\n")
  );
  process.exit(1);
}

function buildPreflight() {
  if (process.platform === "darwin") {
    return {
      ready: false,
      supportedPlatform: false,
      blockers: [macosBlocker],
    };
  }

  const supportedPlatform = process.platform === "linux" || process.platform === "win32";
  const blockers = [];
  if (!supportedPlatform) blockers.push("tauri_webdriver_platform_not_supported");
  if (!commandAvailable("tauri-driver")) blockers.push("tauri_driver_binary_missing");
  if (!nativeWebdriverAvailable()) blockers.push("native_webdriver_binary_missing");
  if (!stage1TauriDebugAppBinaryAvailable()) blockers.push("tauri_debug_app_binary_missing");

  return {
    ready: supportedPlatform && blockers.length === 0,
    supportedPlatform,
    blockers,
  };
}

function commandAvailable(command) {
  const lookup = process.platform === "win32" ? "where" : "command";
  const args = process.platform === "win32" ? [command] : ["-v", command];
  return (
    spawnSync(lookup, args, {
      stdio: "ignore",
      shell: process.platform !== "win32",
    }).status === 0
  );
}

function nativeWebdriverAvailable() {
  if (process.platform === "linux") return commandAvailable("WebKitWebDriver");
  if (process.platform === "win32") return commandAvailable("msedgedriver");
  return false;
}

function stage1TauriDebugAppBinaryAvailable() {
  return fs.existsSync(stage1TauriDebugAppBinaryPath());
}

function stage1TauriDebugAppBinaryPath() {
  const binary = process.platform === "win32" ? "openlife-tauri.exe" : "openlife-tauri";
  return path.resolve(repoRoot, "target", "debug", binary);
}

async function runStage1TauriDogfood() {
  let driverProcess;
  let frontendDevServer;
  let sessionId;
  try {
    frontendDevServer = await startFrontendDevServer();
    driverProcess = startTauriDriver();
    const session = await createTauriWebDriverSession(stage1TauriDebugAppBinaryPath());
    sessionId = session.sessionId;

    const prepReport = await tauriInvoke(
      sessionId,
      "prepare_main_chat_agent_stage1_browser_dogfood_state"
    );
    const prepBlockers = validateStage1BrowserPrepReport(prepReport);
    if (prepBlockers.length > 0) {
      return {
        ready: false,
        sessionCreated: true,
        observedCount: 0,
        blockers: ["tauri_webdriver_stage1_prep_not_ready", ...prepBlockers],
      };
    }

    await navigateToChat(sessionId);

    const gateReport = await tauriInvoke(sessionId, "run_main_chat_agent_stage1_dogfood_gate");
    const gateRows = new Map((gateReport?.scenarios ?? []).map(row => [row.scenarioId, row]));
    const observedScenarios = [];
    for (const scenario of STAGE1_DOGFOOD_SCENARIOS) {
      console.error(`[stage1_scenario:start] ${scenario.id}`);
      const gateRow = gateRows.get(scenario.id);
      if (!gateRow) {
        return {
          ready: false,
          sessionCreated: true,
          observedCount: observedScenarios.length,
          blockers: [metadataSafeBlocker(`tauri_webdriver_gate_row_missing:${scenario.id}`)],
          observedScenarios,
        };
      }
      try {
        const observed =
          scenario.scenarioType === "chat_e2e"
            ? await executeChatScenarioWithWebDriver(sessionId, scenario, gateRow)
            : await executeSeededControlScenarioWithWebDriver(
                sessionId,
                scenario,
                gateRow,
                prepReport
              );
        observedScenarios.push(observed);
        console.error(`[stage1_scenario:ok] ${scenario.id}`);
      } catch (error) {
        return {
          ready: false,
          sessionCreated: true,
          observedCount: observedScenarios.length,
          blockers: [metadataSafeBlocker(`scenario_${scenario.id}:${error?.message ?? error}`)],
          observedScenarios,
        };
      }
    }

    const observedBlockers = validateObservedScenariosForPassingReport(observedScenarios, gateRows);
    if (observedBlockers.length === 0) {
      if (!writePassingReport(observedScenarios, gateRows)) {
        return {
          ready: false,
          sessionCreated: true,
          observedCount: observedScenarios.length,
          blockers: ["tauri_webdriver_passing_report_validation_failed"],
          observedScenarios,
        };
      }
      let finalGateReport;
      try {
        finalGateReport = await assertFinalStage1GateReadyWithBrowserEvidence(sessionId);
      } catch (error) {
        return {
          ready: false,
          sessionCreated: true,
          observedCount: observedScenarios.length,
          blockers: ["tauri_webdriver_final_gate_rejected", finalGateBlockerFromError(error)],
          observedScenarios,
        };
      }
      return {
        ready: true,
        sessionCreated: true,
        observedCount: observedScenarios.length,
        observedScenarios,
        readinessRecommendation: finalGateReport.readinessRecommendation,
        blockers: [],
      };
    }

    return {
      ready: false,
      sessionCreated: true,
      observedCount: observedScenarios.length,
      blockers: ["tauri_webdriver_d01_d36_observation_not_completed", ...observedBlockers],
      observedScenarios,
    };
  } catch (error) {
    return {
      ready: false,
      sessionCreated: Boolean(sessionId),
      observedCount: 0,
      blockers: [metadataSafeBlocker(`tauri_webdriver_launch_failed:${error?.message ?? error}`)],
    };
  } finally {
    if (sessionId) await deleteWebDriverSession(sessionId).catch(() => undefined);
    if (driverProcess) driverProcess.kill();
    if (frontendDevServer) frontendDevServer.kill();
  }
}

function startTauriDriver() {
  const command = process.platform === "win32" ? "tauri-driver.exe" : "tauri-driver";
  const child = spawn(command, [], {
    stdio: ["ignore", "pipe", "pipe"],
    shell: process.platform === "win32",
  });
  pipeChildOutput(child, "tauri_driver");
  child.on("error", error => {
    throw error;
  });
  return child;
}

async function startFrontendDevServer() {
  if (await frontendDevServerReady()) return null;
  const child = spawn("corepack", ["pnpm", "dev", "--host", "127.0.0.1", "--port", "5173"], {
    cwd: frontendRoot,
    stdio: ["ignore", "pipe", "pipe"],
    shell: process.platform === "win32",
  });
  pipeChildOutput(child, "frontend_dev_server");
  await waitForFrontendDevServer(child, 120_000);
  return child;
}

function pipeChildOutput(child, label) {
  child.stdout?.on("data", chunk => {
    process.stderr.write(`[${label}:stdout] ${chunk}`);
  });
  child.stderr?.on("data", chunk => {
    process.stderr.write(`[${label}:stderr] ${chunk}`);
  });
}

async function waitForFrontendDevServer(child, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let exited = false;
  child.once("exit", () => {
    exited = true;
  });
  while (Date.now() < deadline) {
    if (await frontendDevServerReady()) return;
    if (exited) throw new Error("frontend_dev_server_exited_before_ready");
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  throw new Error("frontend_dev_server_ready_timeout");
}

async function frontendDevServerReady() {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 1_000);
  try {
    const response = await fetch(frontendDevUrl, {
      method: "GET",
      signal: controller.signal,
    });
    return response.ok || response.status < 500;
  } catch {
    return false;
  } finally {
    clearTimeout(timeout);
  }
}

async function createTauriWebDriverSession(application) {
  const response = await retryWebDriverRequest(
    "/session",
    {
      method: "POST",
      body: {
        capabilities: {
          alwaysMatch: {
            browserName: "wry",
            "tauri:options": { application },
          },
        },
      },
    },
    30_000
  );
  const value = response.value ?? response;
  const sessionId = value.sessionId ?? response.sessionId;
  if (!sessionId) throw new Error("webdriver_session_id_missing");
  await configureWebDriverTimeouts(sessionId);
  return { sessionId };
}

async function configureWebDriverTimeouts(sessionId) {
  await webdriverRequest(`/session/${encodeURIComponent(sessionId)}/timeouts`, {
    method: "POST",
    body: {
      implicit: 0,
      pageLoad: 60_000,
      script: 180_000,
    },
  });
}

async function executeChatScenarioWithWebDriver(sessionId, scenario, gateRow) {
  const previousTaskId = await readCurrentTaskIdWithWebDriver(sessionId);
  await setSelectedSkillWithWebDriver(sessionId, scenario.selectedSkillId ?? "");
  await fillByTestId(sessionId, "chat-input", scenario.prompt);
  await waitForElementEnabled(sessionId, "send-button", 10_000);
  await clickByTestId(sessionId, "send-button");
  const controlPlane = await waitForControlPlaneDelivery(sessionId, previousTaskId, scenario);
  return await observeFromControlPlaneWithWebDriver(
    sessionId,
    controlPlane.taskSessionId,
    scenario,
    gateRow,
    ["visible_control.chat_send"]
  );
}

async function executeSeededControlScenarioWithWebDriver(sessionId, scenario, gateRow, prepReport) {
  const visibleControlEvents = [];
  const preObserved = emptyPreObservedEvidence();
  const preparedTaskId = prepReport?.taskSessionIds?.[scenario.id] ?? "";
  if (preparedTaskId) {
    visibleControlEvents.push(
      await openTaskContinuityDetailWithWebDriver(sessionId, preparedTaskId)
    );
    const preEvidence = await readTaskContinuityEvidenceWithWebDriver(sessionId, scenario);
    preObserved.visibleUiStates.push(...preEvidence.visibleUiStates);
    preObserved.finalDeliverySections.push(...preEvidence.finalDeliverySections);
    preObserved.visibleBlockers.push(...preEvidence.visibleBlockers);
    if (scenario.id === "D13") {
      visibleControlEvents.push(
        await clickFirstVisibleControlWithWebDriver(sessionId, [
          "Resume task from continuity detail",
        ])
      );
    } else if (scenario.id === "D14") {
      visibleControlEvents.push(
        await clickFirstVisibleControlWithWebDriver(sessionId, ["Retry task action"])
      );
    } else if (scenario.id === "D15") {
      visibleControlEvents.push(
        await clickFirstVisibleControlWithWebDriver(sessionId, [
          "Cancel task from continuity detail",
        ])
      );
    } else if (scenario.id === "D27") {
      visibleControlEvents.push(
        await clickFirstVisibleControlWithWebDriver(sessionId, ["Refresh task context"])
      );
    } else if (scenario.id === "D35") {
      visibleControlEvents.push(
        await clickTaskContinuityVisibleControlWithWebDriver(sessionId, ["Reject proposal"])
      );
      await waitForTaskContinuityProposalStatusWithWebDriver(sessionId, preparedTaskId, [
        "rejected",
      ]);
    } else if (scenario.id === "D36") {
      visibleControlEvents.push(
        await clickTaskContinuityVisibleControlWithWebDriver(sessionId, ["Defer"])
      );
      await waitForTaskContinuityProposalStatusWithWebDriver(sessionId, preparedTaskId, [
        "postponed",
      ]);
    }
    const evidence = await readTaskContinuityEvidenceWithWebDriver(sessionId, scenario);
    return seededObservationFromEvidence(
      scenario,
      gateRow,
      evidence,
      visibleControlEvents,
      preObserved
    );
  }

  if (["D19", "D20", "D28"].includes(scenario.id)) {
    visibleControlEvents.push(await openTaskContinuityDetailWithWebDriver(sessionId));
    const evidence = await readTaskContinuityEvidenceWithWebDriver(sessionId, scenario);
    return seededObservationFromEvidence(scenario, gateRow, evidence, visibleControlEvents);
  }

  visibleControlEvents.push(await clickScenarioControlWithWebDriver(sessionId, scenario.id));
  const controlPlane = await readLastControlPlaneWithWebDriver(sessionId);
  return await observeFromControlPlaneWithWebDriver(
    sessionId,
    controlPlane.taskSessionId,
    scenario,
    gateRow,
    visibleControlEvents
  );
}

async function tauriInvoke(sessionId, command, commandArgs = {}) {
  const safeCommand = metadataSafeBlocker(command);
  console.error(`[tauri_invoke:start] ${safeCommand}`);
  try {
    const result = await executeAsyncScript(
      sessionId,
      `
        const done = arguments[arguments.length - 1];
        const command = arguments[0];
        const commandArgs = arguments[1];
        const internals = window.__TAURI_INTERNALS__;
        if (!internals?.invoke) {
          done({ ok: false, error: "tauri_invoke_unavailable" });
          return;
        }
        internals.invoke(command, commandArgs)
          .then(value => done({ ok: true, value }))
          .catch(error => done({ ok: false, error: String(error?.message ?? error) }));
      `,
      [command, commandArgs]
    );
    if (!result?.ok) throw new Error(result?.error ?? "tauri_invoke_failed");
    console.error(`[tauri_invoke:ok] ${safeCommand}`);
    return result.value;
  } catch (error) {
    console.error(
      `[tauri_invoke:error] ${safeCommand}:${metadataSafeBlocker(error?.message ?? error)}`
    );
    throw error;
  }
}

async function navigateToChat(sessionId) {
  await webdriverRequest(`/session/${encodeURIComponent(sessionId)}/execute/sync`, {
    method: "POST",
    body: {
      script: "window.location.hash = '#/chat'; return window.location.hash;",
      args: [],
    },
  });
}

async function readCurrentTaskIdWithWebDriver(sessionId) {
  return await executeScript(
    sessionId,
    `
      const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
      const control = controls.at(-1);
      return control?.getAttribute('data-task-session-id') ?? '';
    `
  );
}

async function setSelectedSkillWithWebDriver(sessionId, selectedSkillId) {
  const requestedSkillId = String(selectedSkillId ?? "");
  const selected = await executeScript(
    sessionId,
    `
      const input = document.querySelector('[data-testid="skill-context-input"]');
      if (!input) return false;
      const inputValueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      if (!inputValueSetter) return false;
      input.focus();
      inputValueSetter.call(input, arguments[0]);
      try {
        input.dispatchEvent(new InputEvent('input', {
          bubbles: true,
          data: arguments[0],
          inputType: arguments[0] ? 'insertText' : 'deleteContentBackward',
        }));
      } catch {
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
      input.dispatchEvent(new Event('change', { bubbles: true }));
      return input.value === arguments[0];
    `,
    [requestedSkillId]
  );
  if (requestedSkillId.trim() && !selected) {
    throw new Error(
      `webdriver_selected_skill_not_applied:${metadataSafeBlocker(requestedSkillId)}`
    );
  }
  await waitForScript(
    sessionId,
    `
      const expected = arguments[0];
      const input = document.querySelector('[data-testid="skill-context-input"]');
      const stateBackedControl = document.querySelector('[data-testid="skill-context-control"]');
      if (!input || !stateBackedControl) return false;
      return input.value === expected &&
        stateBackedControl.getAttribute('data-selected-skill-id') === expected;
    `,
    [requestedSkillId],
    10_000,
    `webdriver_selected_skill_state_not_committed:${metadataSafeBlocker(requestedSkillId)}`
  );
}

async function fillByTestId(sessionId, testId, value) {
  const filled = await executeScript(
    sessionId,
    `
      const element = document.querySelector(\`[data-testid="\${arguments[0]}"]\`);
      if (!element) return false;
      element.focus();
      const textareaValueSetter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set;
      const inputValueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      const valueSetter = element instanceof HTMLTextAreaElement
        ? textareaValueSetter
        : inputValueSetter;
      if (!valueSetter) return false;
      valueSetter.call(element, arguments[1]);
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    `,
    [testId, value]
  );
  if (!filled) throw new Error(`webdriver_element_missing:${testId}`);
}

async function clickByTestId(sessionId, testId) {
  const clicked = await executeScript(
    sessionId,
    `
      const element = document.querySelector(\`[data-testid="\${arguments[0]}"]\`);
      if (!element || element.disabled) return false;
      element.click();
      return true;
    `,
    [testId]
  );
  if (!clicked) throw new Error(`webdriver_click_failed:${testId}`);
}

async function waitForElementEnabled(sessionId, testId, timeoutMs) {
  await waitForScript(
    sessionId,
    `
      const element = document.querySelector(\`[data-testid="\${arguments[0]}"]\`);
      return Boolean(element && !element.disabled);
    `,
    [testId],
    timeoutMs,
    `webdriver_element_not_enabled:${testId}`
  );
}

async function waitForControlPlaneDelivery(sessionId, previousTaskId, scenario) {
  try {
    return await waitForScript(
      sessionId,
      `
        const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
        const control = controls.at(-1);
        if (!control) return null;
        const expectedUiStates = arguments[1] ?? [];
        const taskSessionId = control.getAttribute('data-task-session-id') ?? '';
        const finalDelivery = control.getAttribute('data-final-delivery') === 'true';
        const routeStrategy = control.getAttribute('data-route-strategy') ?? '';
        const taskStatus = control.getAttribute('data-task-status') ?? '';
        const actionCount = Number(control.getAttribute('data-action-count') ?? '0');
        const observationCount = Number(control.getAttribute('data-observation-count') ?? '0');
        const blockerCount = Number(control.getAttribute('data-blocker-count') ?? '0');
        const proposalCount = Number(control.getAttribute('data-proposal-count') ?? '0');
        const readyWithoutFinalDelivery =
          expectedUiStates.some(state =>
            ["blocked", "permission_needed", "memory_candidate"].includes(state)
          ) &&
          (blockerCount > 0 || proposalCount > 0 || /blocked|waiting_permission/.test(taskStatus));
        if (
          !taskSessionId ||
          taskSessionId === arguments[0] ||
          (!finalDelivery && !readyWithoutFinalDelivery)
        ) {
          return null;
        }
        return {
          taskSessionId,
          runId: control.getAttribute('data-run-id') ?? '',
          routeStrategy,
          taskStatus,
          actionCount,
          observationCount,
          blockerCount,
          proposalCount,
          finalDeliverySectionTitles: (
            control.getAttribute('data-final-delivery-section-titles') ?? ''
          ).split('|').filter(Boolean),
          text: control.textContent ?? '',
        };
      `,
      [previousTaskId, scenario?.expectedUiStates ?? []],
      120_000,
      `webdriver_control_plane_delivery_timeout:${scenario?.id ?? "unknown"}`
    );
  } catch (error) {
    const snapshot = await readControlPlaneTimeoutSnapshotWithWebDriver(
      sessionId,
      previousTaskId
    ).catch(
      snapshotError =>
        `snapshot_error=${metadataSafeBlocker(snapshotError?.message ?? snapshotError)}`
    );
    throw new Error(`${error?.message ?? error}:${snapshot}`);
  }
}

async function readLastControlPlaneWithWebDriver(sessionId) {
  return await waitForScript(
    sessionId,
    `
      const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
      const control = controls.at(-1);
      if (!control) return null;
      return {
        taskSessionId: control.getAttribute('data-task-session-id') ?? '',
        runId: control.getAttribute('data-run-id') ?? '',
        routeStrategy: control.getAttribute('data-route-strategy') ?? '',
        taskStatus: control.getAttribute('data-task-status') ?? '',
        actionCount: Number(control.getAttribute('data-action-count') ?? '0'),
        observationCount: Number(control.getAttribute('data-observation-count') ?? '0'),
        blockerCount: Number(control.getAttribute('data-blocker-count') ?? '0'),
        proposalCount: Number(control.getAttribute('data-proposal-count') ?? '0'),
        finalDeliverySectionTitles: (
          control.getAttribute('data-final-delivery-section-titles') ?? ''
        ).split('|').filter(Boolean),
        text: control.textContent ?? '',
      };
    `,
    [],
    30_000,
    "webdriver_control_plane_missing"
  );
}

async function readControlPlaneTimeoutSnapshotWithWebDriver(sessionId, previousTaskId) {
  const snapshot = await executeScript(
    sessionId,
    `
      const safe = value => String(value ?? '')
        .replace(/[^A-Za-z0-9_.:-]/g, '_')
        .slice(0, 96);
      const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
      const control = controls.at(-1);
      const selectedSkillControl = document.querySelector('[data-testid="skill-context-control"]');
      const sendButton = document.querySelector('[data-testid="send-button"]');
      const assistantMessages = [...document.querySelectorAll('[data-testid="chat-message-assistant"]')];
      const lastTaskSessionId = control?.getAttribute('data-task-session-id') ?? '';
      return {
        controlCount: controls.length,
        lastTaskChanged: Boolean(lastTaskSessionId && lastTaskSessionId !== arguments[0]),
        lastTaskStatus: safe(control?.getAttribute('data-task-status') ?? ''),
        lastFinalDelivery: control?.getAttribute('data-final-delivery') === 'true',
        lastRouteStrategy: safe(control?.getAttribute('data-route-strategy') ?? ''),
        actionCount: Number(control?.getAttribute('data-action-count') ?? '0'),
        observationCount: Number(control?.getAttribute('data-observation-count') ?? '0'),
        blockerCount: Number(control?.getAttribute('data-blocker-count') ?? '0'),
        proposalCount: Number(control?.getAttribute('data-proposal-count') ?? '0'),
        selectedSkillId: safe(selectedSkillControl?.getAttribute('data-selected-skill-id') ?? ''),
        sendDisabled: Boolean(sendButton?.disabled),
        assistantMessageCount: assistantMessages.length,
      };
    `,
    [previousTaskId]
  );
  if (!snapshot || typeof snapshot !== "object") return "snapshot=missing";
  return Object.entries(snapshot)
    .map(([key, value]) => `${metadataSafeBlocker(key)}=${metadataSafeBlocker(value)}`)
    .join(",");
}

async function openTaskContinuityDetailWithWebDriver(sessionId, taskSessionId = "") {
  const opened = await waitForScript(
    sessionId,
    `
      const summaries = [...document.querySelectorAll('[data-testid="task-continuity-summary"]')];
      if (summaries.length === 0) return false;
      const targetId = arguments[0];
      const target = targetId
        ? summaries.find(item => item.getAttribute('data-task-session-id') === targetId)
        : summaries[0];
      if (!target) return false;
      target.click();
      return true;
    `,
    [taskSessionId],
    30_000,
    taskSessionId
      ? `webdriver_task_continuity_summary_missing:${taskSessionId}`
      : "webdriver_task_continuity_summary_missing"
  );
  if (!opened) throw new Error("webdriver_task_continuity_detail_not_opened");
  await waitForScript(
    sessionId,
    `return Boolean(document.querySelector('[data-testid="task-continuity-detail"]'));`,
    [],
    10_000,
    "webdriver_task_continuity_detail_missing"
  );
  return "visible_control.task_continuity_detail_opened";
}

async function clickScenarioControlWithWebDriver(sessionId, id) {
  if (id === "D09") {
    await installPromptResponseWithWebDriver(sessionId, "Stage 1 browser dogfood skip.");
    return await clickFirstVisibleControlWithWebDriver(sessionId, ["Skip step"]);
  }
  if (id === "D11")
    return await clickFirstVisibleControlWithWebDriver(sessionId, ["Accept proposal"]);
  if (id === "D12")
    return await clickFirstVisibleControlWithWebDriver(sessionId, ["Rollback memory"]);
  if (id === "D35")
    return await clickFirstVisibleControlWithWebDriver(sessionId, ["Deny", "Reject proposal"]);
  if (id === "D36") return await clickFirstVisibleControlWithWebDriver(sessionId, ["Defer"]);
  throw new Error(`webdriver_visible_control_not_mapped:${id}`);
}

async function installPromptResponseWithWebDriver(sessionId, response) {
  await executeScript(
    sessionId,
    `
      window.prompt = () => arguments[0];
      return true;
    `,
    [response]
  );
}

async function clickFirstVisibleControlWithWebDriver(sessionId, labels) {
  const label = await waitForScript(
    sessionId,
    `
      const labels = arguments[0].map(label => label.toLowerCase());
      const buttons = [...document.querySelectorAll('button')];
      const labelText = item => [
        item.innerText,
        item.textContent,
        item.getAttribute('aria-label'),
        item.getAttribute('title'),
      ].filter(Boolean).join(' ').trim();
      const button = buttons.find(item => {
        const text = labelText(item).toLowerCase();
        return !item.disabled && labels.some(label => text === label || text.includes(label));
      });
      if (!button) return '';
      const text = labelText(button);
      button.click();
      return text;
    `,
    [labels],
    30_000,
    `webdriver_visible_control_missing:${labels.join("|")}`
  );
  await new Promise(resolve => setTimeout(resolve, 500));
  return visibleControlEventForLabel(label);
}

async function clickTaskContinuityVisibleControlWithWebDriver(sessionId, labels) {
  const label = await waitForScript(
    sessionId,
    `
      const detail = document.querySelector('[data-testid="task-continuity-detail"]');
      if (!detail) return '';
      const labels = arguments[0].map(label => label.toLowerCase());
      const buttons = [...detail.querySelectorAll('button')];
      const labelText = item => [
        item.innerText,
        item.textContent,
        item.getAttribute('aria-label'),
        item.getAttribute('title'),
      ].filter(Boolean).join(' ').trim();
      const button = buttons.find(item => {
        const text = labelText(item).toLowerCase();
        return !item.disabled && labels.some(label => text === label || text.includes(label));
      });
      if (!button) return '';
      const text = labelText(button);
      button.click();
      return text;
    `,
    [labels],
    30_000,
    `webdriver_task_continuity_control_missing:${labels.join("|")}`
  );
  await new Promise(resolve => setTimeout(resolve, 500));
  return visibleControlEventForLabel(label);
}

async function waitForTaskContinuityProposalStatusWithWebDriver(
  sessionId,
  taskSessionId,
  expectedStatuses
) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const detail = await tauriInvoke(sessionId, "get_main_chat_agent_task_detail", {
      taskSessionId,
      task_session_id: taskSessionId,
    });
    if (
      (detail?.proposals ?? []).some(proposal => expectedStatuses.includes(proposal?.status ?? ""))
    ) {
      return true;
    }
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw new Error(
    `webdriver_task_continuity_proposal_status_timeout:${expectedStatuses.join("|")}`
  );
}

async function observeFromControlPlaneWithWebDriver(
  sessionId,
  taskSessionId,
  scenario,
  gateRow,
  visibleControlEvents = []
) {
  const attrs = await readControlPlaneAttrsWithWebDriver(sessionId, taskSessionId);
  const snapshot = await tauriInvoke(sessionId, "get_main_chat_agent_state_snapshot", {
    taskSessionId: attrs.taskSessionId,
    task_session_id: attrs.taskSessionId,
  });
  const events = await taskEventsWithWebDriver(sessionId, attrs.taskSessionId);
  const text = attrs.text ?? "";
  const visibleBlockers = visibleBlockersForScenario(scenario, snapshot);
  const visibleUiStates = scenario.expectedUiStates.filter(state =>
    uiStateObserved(state, attrs, snapshot)
  );
  const finalDeliverySections = scenario.expectedFinalSections.filter(section =>
    finalSectionObserved(section, attrs.finalDeliverySectionTitles, snapshot, visibleControlEvents)
  );
  const runtimeStrategyEvent = observedRuntimeStrategyEvent(attrs.routeStrategy);

  return {
    scenarioId: scenario.id,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: gateRow.entryPoint,
    taskSessionId: attrs.taskSessionId,
    runId: attrs.runId,
    routeStrategy: gateRow.routeStrategy,
    runtimeEvents: uniqueValues([
      ...events,
      ...snapshotEvents(snapshot),
      ...visibleControlEvents,
      runtimeStrategyEvent,
    ]),
    visibleUiStates,
    finalDeliverySections,
    visibleBlockers,
    runtimeEvidenceObserved: attrs.taskSessionId.length > 0 && events.length > 0,
    uiStateObserved: scenario.expectedUiStates.every(state => visibleUiStates.includes(state)),
    finalDeliveryObserved: scenario.expectedFinalSections.every(section =>
      finalDeliverySections.includes(section)
    ),
    nonFakeEvidenceObserved: Boolean(snapshot?.task?.taskId && attrs.taskSessionId),
    legacyFallbackUsed: text.includes("Fallback notice"),
    silentDurableWriteDetected: false,
    fakeExecutionDetected: false,
  };
}

async function readControlPlaneAttrsWithWebDriver(sessionId, taskSessionId = "") {
  return await waitForScript(
    sessionId,
    `
      const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
      if (controls.length === 0) return null;
      const targetId = arguments[0];
      const control = targetId
        ? controls.find(item => item.getAttribute('data-task-session-id') === targetId)
        : controls.at(-1);
      if (!control) return null;
      return {
        taskSessionId: control.getAttribute('data-task-session-id') ?? '',
        runId: control.getAttribute('data-run-id') ?? '',
        routeStrategy: control.getAttribute('data-route-strategy') ?? '',
        taskStatus: control.getAttribute('data-task-status') ?? '',
        actionCount: Number(control.getAttribute('data-action-count') ?? '0'),
        observationCount: Number(control.getAttribute('data-observation-count') ?? '0'),
        blockerCount: Number(control.getAttribute('data-blocker-count') ?? '0'),
        proposalCount: Number(control.getAttribute('data-proposal-count') ?? '0'),
        finalDeliverySectionTitles: (
          control.getAttribute('data-final-delivery-section-titles') ?? ''
        ).split('|').filter(Boolean),
        text: control.textContent ?? '',
      };
    `,
    [taskSessionId],
    30_000,
    taskSessionId
      ? `webdriver_control_plane_missing:${taskSessionId}`
      : "webdriver_control_plane_missing"
  );
}

async function readTaskContinuityEvidenceWithWebDriver(sessionId, scenario) {
  const domEvidence = await executeScript(
    sessionId,
    `
      const detail = document.querySelector('[data-testid="task-continuity-detail"]');
      const finalDelivery = document.querySelector('[data-testid="task-continuity-final-delivery"]');
      if (!detail) return null;
      return {
        taskSessionId: detail.getAttribute('data-task-session-id') ?? '',
        runId: detail.getAttribute('data-run-id') ?? '',
        routeStrategy: detail.getAttribute('data-task-strategy') ?? '',
        status: detail.getAttribute('data-task-status') ?? '',
        nextControl: detail.getAttribute('data-next-control') ?? '',
        finalDeliverySectionTitles: (
          finalDelivery?.getAttribute('data-final-delivery-section-titles') ?? ''
        ).split('|').filter(Boolean),
        text: detail.textContent ?? '',
      };
    `
  );
  if (!domEvidence) throw new Error(`webdriver_task_continuity_evidence_missing:${scenario.id}`);
  const taskDetail = await tauriInvoke(sessionId, "get_main_chat_agent_task_detail", {
    taskSessionId: domEvidence.taskSessionId,
    task_session_id: domEvidence.taskSessionId,
  });
  const events = await taskEventsWithWebDriver(sessionId, domEvidence.taskSessionId);
  const visibleUiStates = scenario.expectedUiStates.filter(state =>
    continuityUiStateObserved(state, domEvidence.status, domEvidence.nextControl, taskDetail)
  );
  const finalDeliverySections = scenario.expectedFinalSections.filter(section =>
    finalSectionObserved(section, domEvidence.finalDeliverySectionTitles, taskDetail)
  );
  const visibleBlockers = visibleBlockersForScenario(scenario, taskDetail);
  const runtimeStrategyEvent = observedRuntimeStrategyEvent(
    domEvidence.routeStrategy || taskDetail?.taskSession?.selectedStrategy || ""
  );

  return {
    taskSessionId: domEvidence.taskSessionId,
    runId: domEvidence.runId,
    routeStrategy: domEvidence.routeStrategy || taskDetail?.taskSession?.selectedStrategy || "",
    runtimeEvents: uniqueValues([...events, ...transcriptEvents(taskDetail), runtimeStrategyEvent]),
    visibleUiStates,
    finalDeliverySections,
    visibleBlockers,
    runtimeEvidenceObserved:
      domEvidence.taskSessionId.length > 0 &&
      (events.length > 0 || (taskDetail?.transcript?.length ?? 0) > 0),
    nonFakeEvidenceObserved: Boolean(taskDetail?.taskSession?.id),
  };
}

function seededObservationFromEvidence(
  scenario,
  gateRow,
  evidence,
  visibleControlEvents,
  preObserved = {}
) {
  if (!evidence) throw new Error(`webdriver_task_continuity_evidence_missing:${scenario.id}`);
  const visibleUiStates = uniqueValues([
    ...(preObserved.visibleUiStates ?? []),
    ...evidence.visibleUiStates,
  ]);
  const finalDeliverySections = uniqueValues([
    ...(preObserved.finalDeliverySections ?? []),
    ...evidence.finalDeliverySections,
  ]);
  const visibleBlockers = uniqueValues([
    ...(preObserved.visibleBlockers ?? []),
    ...evidence.visibleBlockers,
  ]);
  return {
    scenarioId: scenario.id,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: gateRow.entryPoint,
    taskSessionId: evidence.taskSessionId,
    runId: evidence.runId,
    routeStrategy: gateRow.routeStrategy,
    runtimeEvents: uniqueValues([...evidence.runtimeEvents, ...visibleControlEvents]),
    visibleUiStates,
    finalDeliverySections,
    visibleBlockers,
    runtimeEvidenceObserved: evidence.runtimeEvidenceObserved,
    uiStateObserved: scenario.expectedUiStates.every(state => visibleUiStates.includes(state)),
    finalDeliveryObserved: scenario.expectedFinalSections.every(section =>
      finalDeliverySections.includes(section)
    ),
    nonFakeEvidenceObserved: evidence.nonFakeEvidenceObserved,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    fakeExecutionDetected: false,
  };
}

function visibleControlEventForLabel(label) {
  const normalized =
    label
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "button";
  return `visible_control.${normalized}`;
}

function observedRuntimeStrategyEvent(routeStrategy) {
  const normalized = metadataSafeBlocker(`observed_runtime_strategy:${routeStrategy || "unknown"}`);
  return metadataSafeLabel(normalized) ? normalized : "observed_runtime_strategy:unknown";
}

function emptyPreObservedEvidence() {
  return {
    visibleUiStates: Array.from([]),
    finalDeliverySections: Array.from([]),
    visibleBlockers: Array.from([]),
  };
}

function validateStage1BrowserPrepReport(prepReport) {
  const blockers = [];
  if (!prepReport?.prepared) {
    blockers.push("tauri_webdriver_stage1_prep_not_prepared");
  }
  if (prepReport?.evidenceSource !== "real_app_state_task_continuity_seed") {
    blockers.push("tauri_webdriver_stage1_prep_source_invalid");
  }
  if (prepReport?.directWritesExecuted) {
    blockers.push("tauri_webdriver_stage1_prep_direct_write_detected");
  }
  if (prepReport?.durableLifemodelWritesExecuted) {
    blockers.push("tauri_webdriver_stage1_prep_durable_lifemodel_write_detected");
  }
  if (prepReport?.fileOrExternalWritesExecuted) {
    blockers.push("tauri_webdriver_stage1_prep_file_or_external_write_detected");
  }
  if (Array.isArray(prepReport?.blockers) && prepReport.blockers.length > 0) {
    blockers.push(
      ...prepReport.blockers.map(blocker =>
        metadataSafeBlocker(`tauri_webdriver_stage1_prep_blocker:${blocker}`)
      )
    );
  } else if (prepReport?.blockers !== undefined && !Array.isArray(prepReport.blockers)) {
    blockers.push("tauri_webdriver_stage1_prep_blockers_invalid");
  }
  for (const id of REQUIRED_STAGE1_PREP_TASK_IDS) {
    const taskSessionId = prepReport?.taskSessionIds?.[id] ?? "";
    if (!taskSessionId) {
      blockers.push(`tauri_webdriver_stage1_prep_missing_seeded_task:${id}`);
    } else if (!metadataSafeLabel(taskSessionId) || taskSessionId.startsWith("stage1_task_")) {
      blockers.push(`tauri_webdriver_stage1_prep_task_id_unsafe:${id}`);
    }
  }
  return uniqueValues(blockers);
}

async function taskEventsWithWebDriver(sessionId, taskSessionId) {
  if (!taskSessionId) return [];
  const events = await tauriInvoke(sessionId, "list_main_chat_agent_events", {
    taskSessionId,
    task_session_id: taskSessionId,
    afterSequence: 0,
    after_sequence: 0,
    limit: 100,
  });
  return uniqueValues(
    (events ?? []).map(event => event?.eventType).filter(event => typeof event === "string")
  );
}

function uiStateObserved(state, attrs, snapshot) {
  if (state === "answering") {
    return attrs.routeStrategy.includes("direct") || attrs.actionCount === 0;
  }
  if (state === "planning") {
    return Boolean(snapshot?.plan) || /plan|react|mcp|skill/i.test(attrs.routeStrategy);
  }
  if (state === "action_running") {
    return attrs.actionCount > 0 || (snapshot?.actions?.length ?? 0) > 0;
  }
  if (state === "observation_ready") {
    return attrs.observationCount > 0 || (snapshot?.observations?.length ?? 0) > 0;
  }
  if (state === "completed") {
    return (
      /completed|delivered|succeeded|blocked/.test(attrs.taskStatus) ||
      Boolean(snapshot?.finalDelivery)
    );
  }
  if (state === "memory_candidate") {
    return (
      attrs.proposalCount > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0 ||
      finalDeliveryArrayLength(snapshot, "proposalsCreated") > 0
    );
  }
  if (state === "permission_needed") {
    return (
      attrs.blockerCount > 0 ||
      attrs.proposalCount > 0 ||
      (snapshot?.blockers?.length ?? 0) > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0 ||
      finalDeliveryArrayLength(snapshot, "pendingUserActions") > 0
    );
  }
  if (state === "blocked") {
    return attrs.blockerCount > 0 || /blocked|waiting_permission/.test(attrs.taskStatus);
  }
  if (state === "retry_available") {
    return (
      (snapshot?.actions ?? []).some(action => action?.retryable) ||
      hasAnyStructuredControl(snapshot?.blockers, ["retry", "resume", "refresh_context"]) ||
      hasAnyStructuredControl(snapshot?.proposals, ["retry", "resume", "refresh_context"]) ||
      snapshotControls(snapshot).some(control =>
        ["retry", "resume", "refresh_context"].includes(control)
      )
    );
  }
  if (state === "replaying_events") {
    return snapshotEvents(snapshot).some(event =>
      statusHasAny(event, ["replaying_events", "stream_recovered", "event_stream"])
    );
  }
  return false;
}

function continuityUiStateObserved(state, status, nextControl, detail) {
  if (state === "completed") {
    return /completed|cancelled|blocked/.test(status) || Boolean(detail?.finalDelivery);
  }
  if (state === "blocked") {
    return (
      (detail?.blockers?.length ?? 0) > 0 ||
      /blocked|waiting_permission/.test(status) ||
      Boolean(detail?.continuityDiagnostics?.staleContext)
    );
  }
  if (state === "retry_available") {
    return (
      detail?.allowedControls?.some(control =>
        ["retry", "resume", "refresh_context"].includes(control)
      ) || ["retry", "resume", "refresh_context"].includes(nextControl)
    );
  }
  if (state === "observation_ready") {
    return (
      detail?.actions?.some(action => action.observationMetadata) ||
      (detail?.actions ?? []).some(action => (action?.observationIds?.length ?? 0) > 0) ||
      finalDeliveryArrayLength(detail, "observationsUsed") > 0
    );
  }
  if (state === "memory_candidate") {
    return (
      (detail?.proposals?.length ?? 0) > 0 ||
      finalDeliveryArrayLength(detail, "proposalsCreated") > 0
    );
  }
  if (state === "planning") {
    return (
      Boolean(detail?.taskSession?.currentPlanSummary) ||
      hasControlName(detail?.allowedControls, ["skip", "skip_step"]) ||
      controlNameMatches(nextControl, ["skip", "skip_step"])
    );
  }
  if (state === "replaying_events") {
    return (
      detail?.continuityDiagnostics?.automaticReplayAllowed ||
      hasControlName(detail?.allowedControls, ["replay", "refresh", "refresh_context"]) ||
      controlNameMatches(nextControl, ["replay", "refresh", "refresh_context"]) ||
      transcriptMetadataFlag(detail, ["replayingEvents", "streamRecovered"])
    );
  }
  return false;
}

function finalSectionObserved(section, visibleTitles, snapshot, controlEvents = []) {
  const delivery = snapshot?.finalDelivery ?? snapshot?.final_delivery;
  const deliveryMetrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  if (section === "completed_work") {
    return Boolean(delivery) || visibleTitles.includes("Completed actions");
  }
  if (section === "observations_used") {
    return (
      visibleTitles.includes("Sources used") ||
      arrayLength(deliveryMetrics, "observationsUsed") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "observationsUsed") > 0
    );
  }
  if (section === "next_action") {
    return (
      visibleTitles.includes("Next steps") ||
      arrayLength(deliveryMetrics, "nextSteps") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "recommendedNextAction") > 0
    );
  }
  if (section === "proposals_created" || section === "proposed_work") {
    return (
      visibleTitles.includes("Proposals created") ||
      arrayLength(deliveryMetrics, "proposalsCreated") > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0
    );
  }
  if (section === "pending_user_action") {
    return (
      visibleTitles.includes("Pending user actions") ||
      arrayLength(deliveryMetrics, "pendingUserActions") > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0 ||
      (snapshot?.blockers?.length ?? 0) > 0
    );
  }
  if (section === "blocked_work") {
    return (
      visibleTitles.includes("Blocked items") ||
      arrayLength(deliveryMetrics, "blockers") > 0 ||
      (snapshot?.blockers?.length ?? 0) > 0
    );
  }
  if (section === "skipped_work") {
    return (
      visibleTitles.includes("Skipped") ||
      visibleTitles.includes("Skipped items") ||
      arrayLength(deliveryMetrics, "skippedActions") > 0 ||
      arrayLength(deliveryMetrics, "skippedWork") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "skippedSteps") > 0 ||
      arrayIncludes(deliveryMetrics, "sections", "skipped") ||
      controlEvents.some(event =>
        seededVisibleControlEventMatchesPrefix(event, "visible_control.skip_step")
      )
    );
  }
  if (section === "durable_changes") {
    return (
      visibleTitles.includes("Durable changes") ||
      arrayLength(deliveryMetrics, "durableChanges") > 0
    );
  }
  return false;
}

function visibleBlockersForScenario(scenario, evidence) {
  if (!scenario.expectedBlocker) return [];
  const blockerEvidence =
    (evidence?.blockers?.length ?? 0) > 0 ||
    (evidence?.finalDelivery?.blockers?.length ?? 0) > 0 ||
    (evidence?.final_delivery?.blockers?.length ?? 0) > 0 ||
    (evidence?.finalDelivery?.metadata?.blockers?.length ?? 0) > 0 ||
    (evidence?.final_delivery?.metadata?.blockers?.length ?? 0) > 0;
  return blockerEvidence ? [scenario.expectedBlocker] : [];
}

function arrayLength(value, key) {
  const item = value?.[key];
  return Array.isArray(item) ? item.length : 0;
}

function arrayIncludes(value, key, expected) {
  const item = value?.[key];
  return Array.isArray(item) && item.includes(expected);
}

function finalDeliveryArrayLength(value, key) {
  const delivery = value?.finalDelivery ?? value?.final_delivery;
  const deliveryMetrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  return arrayLength(deliveryMetrics, key);
}

function statusHasAny(value, tokens) {
  const normalized = String(value ?? "").toLowerCase();
  return tokens.some(token => normalized.includes(token));
}

function controlNameMatches(value, controls) {
  const normalized = String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return controls.some(control => normalized === control || normalized.includes(control));
}

function hasControlName(values, controls) {
  return Array.isArray(values) && values.some(value => controlNameMatches(value, controls));
}

function hasAnyStructuredControl(values, controls) {
  return (
    Array.isArray(values) &&
    values.some(value => {
      const record = value;
      return (
        hasControlName(record?.controls, controls) ||
        controlNameMatches(record?.nextRecommendedControl, controls)
      );
    })
  );
}

function snapshotControls(snapshot) {
  return uniqueValues(
    [
      ...(snapshot?.task?.controls ?? []),
      ...(snapshot?.blockers ?? []).flatMap(blocker => blocker?.controls ?? []),
      ...(snapshot?.proposals ?? []).flatMap(proposal => proposal?.controls ?? []),
    ].filter(value => typeof value === "string")
  );
}

function transcriptMetadataFlag(detail, keys) {
  return (detail?.transcript ?? []).some(entry =>
    keys.some(key => entry?.metadata?.[key] === true)
  );
}

function snapshotEvents(snapshot) {
  return uniqueValues(
    (snapshot?.events ?? [])
      .map(event => event?.eventType)
      .filter(event => typeof event === "string")
  );
}

function transcriptEvents(detail) {
  return uniqueValues(
    (detail?.transcript ?? []).map(entry => `transcript.${entry.kind}`).filter(Boolean)
  );
}

async function waitForScript(sessionId, script, args, timeoutMs, timeoutError) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await executeScript(sessionId, script, args);
    if (value) return value;
    await new Promise(resolve => setTimeout(resolve, 500));
  }
  throw new Error(timeoutError);
}

async function executeScript(sessionId, script, args = []) {
  const response = await webdriverRequest(
    `/session/${encodeURIComponent(sessionId)}/execute/sync`,
    {
      method: "POST",
      body: { script, args },
    }
  );
  return response.value;
}

async function executeAsyncScript(sessionId, script, args = []) {
  const response = await webdriverRequest(
    `/session/${encodeURIComponent(sessionId)}/execute/async`,
    {
      method: "POST",
      body: { script, args },
    }
  );
  return response.value;
}

async function deleteWebDriverSession(sessionId) {
  await webdriverRequest(`/session/${encodeURIComponent(sessionId)}`, {
    method: "DELETE",
  });
}

async function retryWebDriverRequest(endpoint, options, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      return await webdriverRequest(endpoint, options);
    } catch (error) {
      lastError = error;
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }
  throw lastError ?? new Error("webdriver_request_timeout");
}

async function webdriverRequest(endpoint, options) {
  const response = await fetch(`${webdriverUrl}${endpoint}`, {
    method: options.method,
    headers: { "content-type": "application/json" },
    body: options.body ? JSON.stringify(options.body) : undefined,
  });
  const text = await response.text();
  const data = text ? JSON.parse(text) : {};
  if (!response.ok) {
    throw new Error(
      metadataSafeBlocker(data?.value?.message ?? data?.message ?? response.statusText)
    );
  }
  return data;
}

function metadataSafeBlocker(value) {
  return (
    String(value)
      .replace(/[^A-Za-z0-9_.:/-]+/g, "_")
      .replace(/^_+|_+$/g, "")
      .slice(0, 160) || "unknown"
  );
}

function normalizeBlockers(values) {
  return uniqueValues(values.map(metadataSafeBlocker));
}

function writeBlockedReport(blockers, observedScenarios = []) {
  const runId = `stage1-browser-e2e-blocked-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const normalizedBlockers = normalizeBlockers(["not_ready_browser_e2e_blocked", ...blockers]);
  const copiedObservedScenarios = Array.isArray(observedScenarios)
    ? observedScenarios.map(copyObservedScenario)
    : [];
  const report = {
    browserE2eEnvironmentReady: false,
    selfContainedRunner: true,
    smokePassed: false,
    reportPath,
    evidenceSource: "tauri_command_surface_unavailable",
    runId,
    generatedAt,
    reportDigest: digestLabel(
      stage1BrowserReportDigestInput({
        evidenceSource: "tauri_command_surface_unavailable",
        runId,
        generatedAt,
        requiredJourneys,
        passedJourneys: [],
        failedJourneys: requiredJourneys,
        observedScenarios: copiedObservedScenarios,
        blockers: normalizedBlockers,
      })
    ),
    requiredJourneys,
    passedJourneys: [],
    failedJourneys: requiredJourneys,
    observedScenarios: copiedObservedScenarios,
    blockers: normalizedBlockers,
  };

  const outputPath = stage1BrowserReportOutputPath();
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
}

function writePassingReport(observedScenarios, gateRows = new Map()) {
  const blockers = validateObservedScenariosForPassingReport(observedScenarios, gateRows);
  if (blockers.length > 0) {
    writeBlockedReport(
      ["tauri_webdriver_passing_report_validation_failed", ...blockers],
      observedScenarios
    );
    return false;
  }

  const runId = `stage1-browser-e2e-real-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const report = {
    browserE2eEnvironmentReady: true,
    selfContainedRunner: true,
    smokePassed: true,
    reportPath,
    evidenceSource: "tauri_command_surface_browser_observed",
    runId,
    generatedAt,
    reportDigest: digestLabel(
      stage1BrowserReportDigestInput({
        evidenceSource: "tauri_command_surface_browser_observed",
        runId,
        generatedAt,
        requiredJourneys,
        passedJourneys: requiredJourneys,
        failedJourneys: [],
        observedScenarios,
        blockers: [],
      })
    ),
    requiredJourneys,
    passedJourneys: requiredJourneys,
    failedJourneys: [],
    observedScenarios: observedScenarios.map(copyObservedScenario),
    blockers: [],
  };

  const outputPath = stage1BrowserReportOutputPath();
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
  return true;
}

function stage1BrowserReportOutputPath() {
  return path.resolve(repoRoot, "frontend", "test-results", "main-chat-stage1-dogfood-report.json");
}

async function assertFinalStage1GateReadyWithBrowserEvidence(sessionId) {
  const report = await tauriInvoke(sessionId, "run_main_chat_agent_stage1_dogfood_gate");
  const blockers = [];
  if (report?.defaultReady !== true) blockers.push("tauri_webdriver_final_gate_default_not_ready");
  if (report?.readinessRecommendation !== "ready_for_engineering_dogfood") {
    blockers.push("tauri_webdriver_final_gate_recommendation_not_ready");
  }
  if (report?.browserE2eEnvironmentReady !== true) {
    blockers.push("tauri_webdriver_final_gate_browser_environment_not_ready");
  }
  if (report?.browserE2ePassedJourneyCount !== requiredJourneys.length) {
    blockers.push("tauri_webdriver_final_gate_browser_passed_count_mismatch");
  }
  if (report?.browserE2eFailedJourneyCount !== 0) {
    blockers.push("tauri_webdriver_final_gate_browser_failed_count_nonzero");
  }
  if (Array.isArray(report?.blockers) && report.blockers.length > 0) {
    blockers.push(
      ...report.blockers.map(blocker => `tauri_webdriver_final_gate_blocker:${blocker}`)
    );
  }
  if (blockers.length > 0) {
    throw new Error(uniqueValues(blockers).map(metadataSafeBlocker).join(","));
  }
  return report;
}

function finalGateBlockerFromError(error) {
  return metadataSafeBlocker(`tauri_webdriver_final_gate_error:${error?.message ?? error}`);
}

function validateObservedScenariosForPassingReport(observedScenarios, gateRows = new Map()) {
  const blockers = [];
  if (!Array.isArray(observedScenarios)) return ["tauri_webdriver_observed_scenarios_missing"];
  const observedIds = observedScenarios.map(row => row?.scenarioId ?? "");
  const scenarioById = new Map(STAGE1_DOGFOOD_SCENARIOS.map(scenario => [scenario.id, scenario]));
  if (observedIds.length !== requiredJourneys.length) {
    blockers.push("tauri_webdriver_observed_scenario_count_not_36");
  }
  if (observedIds.join(",") !== requiredJourneys.join(",")) {
    blockers.push("tauri_webdriver_observed_scenario_ids_not_exact_d01_d36");
  }
  if (uniqueValues(observedScenarios.map(row => row?.taskSessionId ?? "")).length < 20) {
    blockers.push("tauri_webdriver_observed_task_session_distinct_count_below_20");
  }
  if (uniqueValues(observedScenarios.map(row => row?.runId ?? "")).length < 20) {
    blockers.push("tauri_webdriver_observed_run_distinct_count_below_20");
  }
  for (const row of observedScenarios) {
    if (!row?.scenarioId) {
      blockers.push("tauri_webdriver_observed_scenario_id_missing");
      continue;
    }
    const scenario = scenarioById.get(row.scenarioId);
    const gateRow = gateRows.get(row.scenarioId);
    if (!scenario) {
      blockers.push(`tauri_webdriver_scenario_contract_missing:${row.scenarioId}`);
      continue;
    }
    if (row.observedVia !== "real_tauri_chat_or_control_path") {
      blockers.push(`tauri_webdriver_observed_via_invalid:${row.scenarioId}`);
    }
    const expectedEntryPoint = entryPointForScenario(scenario);
    if (row.entryPoint !== expectedEntryPoint) {
      blockers.push(`tauri_webdriver_entry_point_mismatch:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.entryPoint)) {
      blockers.push(`tauri_webdriver_entry_point_unsafe:${row.scenarioId}`);
    }
    if (!row.taskSessionId || !row.runId || !row.routeStrategy) {
      blockers.push(`tauri_webdriver_runtime_identity_missing:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.taskSessionId) || row.taskSessionId.startsWith("stage1_task_")) {
      blockers.push(`tauri_webdriver_task_session_not_observed:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.runId) || row.runId.startsWith("stage1_run_")) {
      blockers.push(`tauri_webdriver_run_not_observed:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.routeStrategy)) {
      blockers.push(`tauri_webdriver_route_unsafe:${row.scenarioId}`);
    }
    if (gateRow?.routeStrategy && row.routeStrategy !== gateRow.routeStrategy) {
      blockers.push(`tauri_webdriver_route_mismatch:${row.scenarioId}`);
    }
    if (!Array.isArray(row.runtimeEvents) || row.runtimeEvents.length === 0) {
      blockers.push(`tauri_webdriver_runtime_events_missing:${row.scenarioId}`);
    }
    if (hasUnsafeLabel(row.runtimeEvents)) {
      blockers.push(`tauri_webdriver_runtime_event_unsafe:${row.scenarioId}`);
    }
    if (!Array.isArray(row.visibleUiStates) || row.visibleUiStates.length === 0) {
      blockers.push(`tauri_webdriver_ui_states_missing:${row.scenarioId}`);
    }
    if (hasUnsafeLabel(row.visibleUiStates)) {
      blockers.push(`tauri_webdriver_visible_ui_state_unsafe:${row.scenarioId}`);
    }
    if (!Array.isArray(row.finalDeliverySections) || row.finalDeliverySections.length === 0) {
      blockers.push(`tauri_webdriver_final_delivery_missing:${row.scenarioId}`);
    }
    if (hasUnsafeLabel(row.finalDeliverySections)) {
      blockers.push(`tauri_webdriver_final_delivery_section_unsafe:${row.scenarioId}`);
    }
    for (const requiredState of scenario.expectedUiStates) {
      if (!row.visibleUiStates?.includes(requiredState)) {
        blockers.push(
          `tauri_webdriver_required_ui_state_missing:${row.scenarioId}:${requiredState}`
        );
      }
    }
    for (const requiredSection of scenario.expectedFinalSections) {
      if (!row.finalDeliverySections?.includes(requiredSection)) {
        blockers.push(
          `tauri_webdriver_required_final_section_missing:${row.scenarioId}:${requiredSection}`
        );
      }
    }
    if (
      scenario.scenarioType === "chat_e2e" &&
      !row.runtimeEvents?.includes("visible_control.chat_send")
    ) {
      blockers.push(`tauri_webdriver_chat_send_not_observed:${row.scenarioId}`);
    }
    if (
      scenario.scenarioType === "seeded_task_control_e2e" &&
      !seededVisibleControlEventObserved(row.scenarioId, row.runtimeEvents ?? [])
    ) {
      blockers.push(`tauri_webdriver_seeded_control_not_observed:${row.scenarioId}`);
    }
    if (scenario.expectedBlocker && !row.visibleBlockers?.includes(scenario.expectedBlocker)) {
      blockers.push(`tauri_webdriver_expected_blocker_not_visible:${row.scenarioId}`);
    }
    if (hasUnsafeLabel(row.visibleBlockers)) {
      blockers.push(`tauri_webdriver_visible_blocker_unsafe:${row.scenarioId}`);
    }
    if (
      !row.runtimeEvidenceObserved ||
      !row.uiStateObserved ||
      !row.finalDeliveryObserved ||
      !row.nonFakeEvidenceObserved
    ) {
      blockers.push(`tauri_webdriver_required_evidence_flags_missing:${row.scenarioId}`);
    }
    if (row.legacyFallbackUsed || row.silentDurableWriteDetected || row.fakeExecutionDetected) {
      blockers.push(`tauri_webdriver_forbidden_evidence_present:${row.scenarioId}`);
    }
  }
  return uniqueValues(blockers);
}

function entryPointForScenario(scenario) {
  return scenario.scenarioType === "chat_e2e"
    ? "ordinary_main_chat_input"
    : "seeded_visible_control_surface";
}

function metadataSafeLabel(value) {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 96 &&
    value.trim() === value &&
    [...value].every(ch => /[A-Za-z0-9_.:/-]/.test(ch))
  );
}

function hasUnsafeLabel(values) {
  return Array.isArray(values) && values.some(value => !metadataSafeLabel(value));
}

function seededVisibleControlEventObserved(scenarioId, events) {
  return seededVisibleControlEventPrefixes(scenarioId).some(prefix =>
    events.some(event => seededVisibleControlEventMatchesPrefix(event, prefix))
  );
}

function seededVisibleControlEventMatchesPrefix(event, prefix) {
  return event === prefix || event.startsWith(`${prefix}_`);
}

function seededVisibleControlEventPrefixes(scenarioId) {
  if (scenarioId === "D09") return ["visible_control.skip_step"];
  if (scenarioId === "D11") return ["visible_control.accept_proposal"];
  if (scenarioId === "D12") return ["visible_control.rollback_memory"];
  if (scenarioId === "D13") return ["visible_control.resume_task_from_continuity_detail"];
  if (scenarioId === "D14") return ["visible_control.retry_task_action"];
  if (scenarioId === "D15") return ["visible_control.cancel_task_from_continuity_detail"];
  if (scenarioId === "D19") return ["visible_control.task_continuity_detail_opened"];
  if (scenarioId === "D20") return ["visible_control.task_continuity_detail_opened"];
  if (scenarioId === "D27") return ["visible_control.refresh_task_context"];
  if (scenarioId === "D28") return ["visible_control.task_continuity_detail_opened"];
  if (scenarioId === "D35") return ["visible_control.deny", "visible_control.reject_proposal"];
  if (scenarioId === "D36") return ["visible_control.defer"];
  return [];
}

function copyObservedScenario(row) {
  return {
    ...row,
    runtimeEvents: [...row.runtimeEvents],
    visibleUiStates: [...row.visibleUiStates],
    finalDeliverySections: [...row.finalDeliverySections],
    visibleBlockers: [...row.visibleBlockers],
  };
}

function loadStage1DogfoodScenarios() {
  const sourcePath = path.resolve(frontendRoot, "src/stage1DogfoodScenarios.ts");
  const source = fs.readFileSync(sourcePath, "utf8");
  const ids = [
    ...source.matchAll(
      /s\(\s*"(?<id>D\d{2})",\s*"(?<scenarioType>[^"]+)",\s*"(?<prompt>(?:[^"\\]|\\.)*)".*?\)/gs
    ),
  ].map(match => {
    const stringArrays = stringArraysFromScenarioCall(match[0]);
    return {
      id: match.groups.id,
      scenarioType: match.groups.scenarioType,
      prompt: match.groups.prompt,
      expectedUiStates: stringArrays[0] ?? [],
      expectedFinalSections: stringArrays[1] ?? [],
      expectedBlocker: expectedBlockerFromScenarioCall(match[0]),
      selectedSkillId: selectedSkillIdFromScenarioCall(match[0]),
    };
  });
  const expectedIds = Array.from(
    { length: 36 },
    (_, index) => `D${String(index + 1).padStart(2, "0")}`
  );
  if (ids.map(item => item.id).join(",") !== expectedIds.join(",")) {
    throw new Error("stage1_dogfood_scenario_matrix_not_exact_d01_d36");
  }
  if (
    ids.some(item => item.expectedUiStates.length === 0 || item.expectedFinalSections.length === 0)
  ) {
    throw new Error("stage1_dogfood_scenario_matrix_missing_expected_observations");
  }
  return ids;
}

function stringArraysFromScenarioCall(callSource) {
  return [...callSource.matchAll(/\[(?<items>[^\]]*)\]/g)].map(match =>
    [...match.groups.items.matchAll(/"(?<item>[^"]+)"/g)].map(itemMatch => itemMatch.groups.item)
  );
}

function expectedBlockerFromScenarioCall(callSource) {
  const arrays = [...callSource.matchAll(/\[[^\]]*\]/g)];
  if (arrays.length < 2) return "";
  const afterFinalSections = callSource.slice(arrays[1].index + arrays[1][0].length);
  return (
    afterFinalSections.match(/^\s*,\s*"(?<expectedBlocker>[^"]+)"/)?.groups?.expectedBlocker ?? ""
  );
}

function selectedSkillIdFromScenarioCall(callSource) {
  return (
    callSource.match(/,\s*undefined,\s*"(?<selectedSkillId>[^"]+)"\s*\)$/)?.groups
      ?.selectedSkillId ?? ""
  );
}

function stage1BrowserReportDigestInput(input) {
  const rows = input.observedScenarios
    .map(row =>
      [
        row.scenarioId,
        row.observedVia,
        row.entryPoint,
        row.taskSessionId,
        row.runId,
        row.routeStrategy,
        digestArray(row.runtimeEvents),
        digestArray(row.visibleUiStates),
        digestArray(row.finalDeliverySections),
        digestArray(row.visibleBlockers),
        String(row.runtimeEvidenceObserved),
        String(row.uiStateObserved),
        String(row.finalDeliveryObserved),
        String(row.nonFakeEvidenceObserved),
        String(row.legacyFallbackUsed),
        String(row.silentDurableWriteDetected),
        String(row.fakeExecutionDetected),
      ]
        .map(digestPart)
        .join("|")
    )
    .join("\n");

  return [
    "stage1-browser-e2e-report-v1",
    `source=${digestPart(input.evidenceSource)}`,
    `runId=${digestPart(input.runId)}`,
    `generatedAt=${digestPart(input.generatedAt)}`,
    `required=${digestArray(input.requiredJourneys)}`,
    `passed=${digestArray(input.passedJourneys)}`,
    `failed=${digestArray(input.failedJourneys)}`,
    `blockers=${digestArray(input.blockers)}`,
    "observed:",
    rows,
  ].join("\n");
}

function digestArray(values) {
  return values.map(digestPart).join(",");
}

function digestPart(value) {
  return `${new TextEncoder().encode(value).byteLength}:${value}`;
}

function digestLabel(input) {
  const bytes = new TextEncoder().encode(input);
  return `bytes:${bytes.byteLength} hash:sha256:${createHash("sha256").update(input).digest("hex")}`;
}

function uniqueValues(values) {
  return values.filter((value, index) => value && values.indexOf(value) === index);
}
