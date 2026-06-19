import { test, expect, type Locator, type Page } from "@playwright/test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import {
  STAGE1_REQUIRED_BROWSER_JOURNEYS,
  buildStage1TauriWebdriverPreflight,
  buildStage1BlockedBrowserEvidenceReport,
  buildStage1PassingBrowserEvidenceReportFromObservedScenarios,
  stage1NonTauriBrowserBlockersForPlatform,
  type Stage1BrowserEvidenceReport,
  type Stage1ObservedBrowserScenario,
} from "../src/stage1BrowserEvidence";
import {
  STAGE1_DOGFOOD_SCENARIOS,
  type Stage1DogfoodScenario,
} from "../src/stage1DogfoodScenarios";
import type { MainChatAgentStage1DogfoodReport } from "../src/tauri";

type TauriInvokeArgs = Record<string, unknown>;

type Stage1BrowserPrepReport = {
  prepared?: boolean;
  evidenceSource?: string;
  taskSessionIds?: Record<string, string>;
  directWritesExecuted?: boolean;
  durableLifemodelWritesExecuted?: boolean;
  fileOrExternalWritesExecuted?: boolean;
  blockers?: string[];
};

const REPORT_FILE = "test-results/main-chat-stage1-dogfood-report.json";
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

function writeReport(report: Stage1BrowserEvidenceReport) {
  const reportPath = path.resolve(process.cwd(), REPORT_FILE);
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  return report;
}

function stage1NonTauriBrowserBlockers(): string[] {
  const preflight = buildStage1TauriWebdriverPreflight({
    platform: process.platform,
    tauriDriverAvailable: commandAvailable("tauri-driver"),
    nativeWebdriverAvailable: nativeWebdriverAvailable(),
    appBinaryAvailable: stage1TauriDebugAppBinaryAvailable(),
  });
  return uniqueValues([
    ...stage1NonTauriBrowserBlockersForPlatform(process.platform),
    ...preflight.blockers,
  ]);
}

function commandAvailable(command: string): boolean {
  const lookup = process.platform === "win32" ? "where" : "command";
  const args = process.platform === "win32" ? [command] : ["-v", command];
  return (
    spawnSync(lookup, args, { stdio: "ignore", shell: process.platform !== "win32" }).status === 0
  );
}

function nativeWebdriverAvailable(): boolean {
  if (process.platform === "linux") return commandAvailable("WebKitWebDriver");
  if (process.platform === "win32") return commandAvailable("msedgedriver");
  return false;
}

function stage1TauriDebugAppBinaryAvailable(): boolean {
  const binary = process.platform === "win32" ? "openlife-tauri.exe" : "openlife-tauri";
  return fs.existsSync(path.resolve(process.cwd(), "..", "target", "debug", binary));
}

async function tauriInvoke<T>(page: Page, cmd: string, args: TauriInvokeArgs = {}): Promise<T> {
  return page.evaluate(
    async ({ command, commandArgs }) => {
      const internals = (window as any).__TAURI_INTERNALS__;
      if (!internals?.invoke) throw new Error("tauri_invoke_unavailable");
      return internals.invoke(command, commandArgs);
    },
    { command: cmd, commandArgs: args }
  );
}

async function runStage1Gate(page: Page): Promise<MainChatAgentStage1DogfoodReport> {
  return tauriInvoke<MainChatAgentStage1DogfoodReport>(
    page,
    "run_main_chat_agent_stage1_dogfood_gate"
  );
}

async function prepareStage1BrowserState(page: Page): Promise<Stage1BrowserPrepReport> {
  return tauriInvoke<Stage1BrowserPrepReport>(
    page,
    "prepare_main_chat_agent_stage1_browser_dogfood_state"
  );
}

async function readCurrentTaskId(page: Page): Promise<string> {
  const controlPlane = page.getByTestId("agent-control-plane").last();
  if ((await controlPlane.count()) === 0) return "";
  return (await controlPlane.getAttribute("data-task-session-id")) ?? "";
}

async function executeChatScenario(
  page: Page,
  scenario: Stage1DogfoodScenario,
  gateRow: MainChatAgentStage1DogfoodReport["scenarios"][number]
): Promise<Stage1ObservedBrowserScenario> {
  const previousTaskId = await readCurrentTaskId(page);
  const previousNetworkPolicy = await prepareStage1ScenarioNetworkPolicy(page, scenario);
  try {
    await setSelectedSkill(page, scenario.selectedSkillId ?? "");
    await page.getByTestId("chat-input").fill(scenario.prompt);
    await expect(page.getByTestId("send-button")).toBeEnabled({ timeout: 10_000 });
    await page.getByTestId("send-button").click();
    const controlPlane = await waitForControlPlaneDelivery(page, previousTaskId);
    return observeFromControlPlane(page, controlPlane, scenario, gateRow, [
      "visible_control.chat_send",
    ]);
  } finally {
    await restoreStage1ScenarioNetworkPolicy(page, previousNetworkPolicy);
  }
}

async function prepareStage1ScenarioNetworkPolicy(
  page: Page,
  scenario: Stage1DogfoodScenario
): Promise<boolean | null> {
  if (scenario.id !== "D23") return null;
  return tauriInvoke<boolean>(page, "set_main_chat_agent_stage1_browser_network_policy", {
    enabled: false,
  });
}

async function restoreStage1ScenarioNetworkPolicy(
  page: Page,
  previousNetworkPolicy: boolean | null
) {
  if (previousNetworkPolicy === null) return;
  await tauriInvoke(page, "set_main_chat_agent_stage1_browser_network_policy", {
    enabled: Boolean(previousNetworkPolicy),
  }).catch(error => {
    console.error(`stage1_network_policy_restore_failed:${String(error?.message ?? error)}`);
  });
}

async function setSelectedSkill(page: Page, selectedSkillId: string) {
  const normalizedSkillId = selectedSkillId.trim();
  const skillInput = page.getByTestId("skill-context-input");
  if ((await skillInput.count()) === 0) {
    if (normalizedSkillId) {
      throw new Error(`stage1_selected_skill_input_missing:${normalizedSkillId}`);
    }
    return;
  }
  await skillInput.fill(selectedSkillId);
  await expect(page.getByTestId("skill-context-control")).toHaveAttribute(
    "data-selected-skill-id",
    normalizedSkillId
  );
  if (normalizedSkillId) {
    await expect(skillInput, `stage1_selected_skill_not_applied:${normalizedSkillId}`).toHaveValue(
      selectedSkillId
    );
  }
}

async function waitForControlPlaneDelivery(page: Page, previousTaskId: string): Promise<Locator> {
  const controlPlane = page.getByTestId("agent-control-plane").last();
  await expect(controlPlane).toBeVisible({ timeout: 120_000 });
  await expect
    .poll(
      async () => {
        const taskId = (await controlPlane.getAttribute("data-task-session-id")) ?? "";
        return taskId.length > 0 && taskId !== previousTaskId;
      },
      { timeout: 120_000 }
    )
    .toBe(true);
  await expect
    .poll(async () => (await controlPlane.getAttribute("data-final-delivery")) === "true", {
      timeout: 120_000,
    })
    .toBe(true);
  return controlPlane;
}

async function executeSeededControlScenario(
  page: Page,
  scenario: Stage1DogfoodScenario,
  gateRow: MainChatAgentStage1DogfoodReport["scenarios"][number],
  prepReport: Stage1BrowserPrepReport
): Promise<Stage1ObservedBrowserScenario> {
  const visibleControlEvents: string[] = [];
  const preObservedUiStates: string[] = [];
  const preFinalDeliverySections: string[] = [];
  const preVisibleBlockers: string[] = [];
  const preparedTaskId = prepReport.taskSessionIds?.[scenario.id] ?? "";
  if (preparedTaskId) {
    visibleControlEvents.push(await openTaskContinuityDetail(page, preparedTaskId));
    const preEvidence = await readTaskContinuityEvidence(page, scenario);
    preObservedUiStates.push(...preEvidence.visibleUiStates);
    preFinalDeliverySections.push(...preEvidence.finalDeliverySections);
    preVisibleBlockers.push(...preEvidence.visibleBlockers);
    const detail = page.getByTestId("task-continuity-detail");
    if (scenario.id === "D13") {
      visibleControlEvents.push(
        await clickFirstVisibleControl(page, [/Resume task from continuity detail/i], detail)
      );
    } else if (scenario.id === "D14") {
      visibleControlEvents.push(
        await clickFirstVisibleControl(page, [/Retry task action/i], detail)
      );
    } else if (scenario.id === "D15") {
      visibleControlEvents.push(
        await clickFirstVisibleControl(page, [/Cancel task from continuity detail/i], detail)
      );
    } else if (scenario.id === "D27") {
      visibleControlEvents.push(
        await clickFirstVisibleControl(page, [/Refresh task context/i], detail)
      );
    } else if (scenario.id === "D35") {
      visibleControlEvents.push(await clickFirstVisibleControl(page, [/Reject proposal/i], detail));
      await waitForTaskContinuityProposalStatus(page, preparedTaskId, ["rejected"]);
    } else if (scenario.id === "D36") {
      visibleControlEvents.push(await clickFirstVisibleControl(page, [/^Defer$/i], detail));
      await waitForTaskContinuityProposalStatus(page, preparedTaskId, ["postponed"]);
    }
  } else if (["D19", "D20", "D28"].includes(scenario.id)) {
    visibleControlEvents.push(await openTaskContinuityDetail(page));
  } else if (scenario.id === "D20" || scenario.id === "D27") {
    const detailEvent = await openTaskContinuityDetail(page);
    const controlEvent = await clickFirstVisibleControl(page, [
      /Refresh task context/i,
      /Resume task from continuity detail/i,
    ]);
    visibleControlEvents.push(detailEvent, controlEvent);
  } else {
    const controlPlane = await controlPlaneForSeededScenario(page, scenario.id);
    visibleControlEvents.push(await clickScenarioControl(page, scenario.id, controlPlane));
    return observeFromControlPlane(page, controlPlane, scenario, gateRow, visibleControlEvents);
  }

  if (preparedTaskId || ["D19", "D20", "D27", "D28"].includes(scenario.id)) {
    return observeFromTaskContinuity(page, scenario, gateRow, visibleControlEvents, {
      visibleUiStates: preObservedUiStates,
      finalDeliverySections: preFinalDeliverySections,
      visibleBlockers: preVisibleBlockers,
    });
  }

  const controlPlane = page.getByTestId("agent-control-plane").last();
  if ((await controlPlane.count()) > 0 && (await controlPlane.isVisible())) {
    const finalDelivery = await controlPlane.getAttribute("data-final-delivery");
    if (finalDelivery === "true") {
      return observeFromControlPlane(page, controlPlane, scenario, gateRow, visibleControlEvents);
    }
  }

  return observeFromTaskContinuity(page, scenario, gateRow, visibleControlEvents, {
    visibleUiStates: preObservedUiStates,
    finalDeliverySections: preFinalDeliverySections,
    visibleBlockers: preVisibleBlockers,
  });
}

async function controlPlaneForSeededScenario(page: Page, id: string): Promise<Locator> {
  if (id === "D09") {
    return controlPlaneWithVisibleButton(page, [/Skip step/i]);
  }
  if (id === "D11") {
    return controlPlaneWithVisibleButton(page, [/Accept proposal/i]);
  }
  if (id === "D12") {
    return controlPlaneWithVisibleButton(page, [/Rollback memory/i]);
  }
  if (id === "D35") {
    return controlPlaneWithVisibleButton(page, [/^Deny$/i, /Reject proposal/i]);
  }
  if (id === "D36") {
    return controlPlaneWithVisibleButton(page, [/^Defer$/i]);
  }
  return page.getByTestId("agent-control-plane").last();
}

async function controlPlaneWithVisibleButton(page: Page, labels: RegExp[]): Promise<Locator> {
  const controlPlanes = page.getByTestId("agent-control-plane");
  const count = await controlPlanes.count();
  for (let index = count - 1; index >= 0; index -= 1) {
    const controlPlane = controlPlanes.nth(index);
    if (await hasEnabledMatchingButton(controlPlane, labels)) return controlPlane;
  }
  throw new Error(`stage1_control_plane_with_button_missing:${labels.map(String).join(",")}`);
}

async function hasEnabledMatchingButton(scope: Locator, labels: RegExp[]): Promise<boolean> {
  for (const label of labels) {
    const buttons = scope.getByRole("button", { name: label });
    const count = await buttons.count();
    for (let index = 0; index < count; index += 1) {
      const button = buttons.nth(index);
      if ((await button.isVisible()) && (await button.isEnabled())) return true;
    }
  }
  return false;
}

async function clickScenarioControl(page: Page, id: string, scope?: Locator): Promise<string> {
  if (id === "D09") {
    return clickWithOptionalPrompt(page, [/Skip step/i], "Stage 1 browser dogfood skip.", scope);
  }
  if (id === "D11") {
    return clickFirstVisibleControl(page, [/Accept proposal/i], scope);
  }
  if (id === "D12") {
    return clickFirstVisibleControl(page, [/Rollback memory/i], scope);
  }
  if (id === "D13") {
    return clickFirstVisibleControl(
      page,
      [/Resume task from continuity detail/i, /Resume task/i],
      scope
    );
  }
  if (id === "D14") {
    return clickFirstVisibleControl(
      page,
      [/Retry task action/i, /Retry failed action/i, /^Retry$/i],
      scope
    );
  }
  if (id === "D15") {
    return clickFirstVisibleControl(
      page,
      [/Cancel task from continuity detail/i, /Cancel task/i],
      scope
    );
  }
  if (id === "D35") {
    return clickFirstVisibleControl(page, [/^Deny$/i, /Reject proposal/i], scope);
  }
  if (id === "D36") {
    return clickFirstVisibleControl(page, [/^Defer$/i], scope);
  }
  throw new Error(`stage1_visible_control_not_mapped:${id}`);
}

async function clickWithOptionalPrompt(
  page: Page,
  labels: RegExp[],
  promptValue: string,
  scope?: Locator
): Promise<string> {
  const dialogPromise = page
    .waitForEvent("dialog", { timeout: 1_500 })
    .then(dialog => dialog.accept(promptValue))
    .catch(() => undefined);
  const event = await clickFirstVisibleControl(page, labels, scope);
  await dialogPromise;
  return event;
}

async function clickFirstVisibleControl(
  page: Page,
  labels: RegExp[],
  scope?: Page | Locator
): Promise<string> {
  const target = scope ?? page;
  const directClickEvent = await clickFirstEnabledMatchingButton(page, target, labels);
  if (directClickEvent) return directClickEvent;
  if (scope) throw new Error(`stage1_visible_control_missing:${labels.map(String).join(",")}`);
  await openTaskContinuityDetail(page).catch(() => undefined);
  const continuityClickEvent = await clickFirstEnabledMatchingButton(page, page, labels);
  if (continuityClickEvent) return continuityClickEvent;
  throw new Error(`stage1_visible_control_missing:${labels.map(String).join(",")}`);
}

async function clickFirstEnabledMatchingButton(
  page: Page,
  target: Page | Locator,
  labels: RegExp[]
): Promise<string | null> {
  for (const label of labels) {
    const buttons = target.getByRole("button", { name: label });
    const count = await buttons.count();
    for (let index = 0; index < count; index += 1) {
      const button = buttons.nth(index);
      if (!(await button.isVisible())) continue;
      if (await button.isEnabled()) {
        const buttonText = await accessibleButtonLabel(button, label.source);
        await button.click();
        await page.waitForTimeout(500);
        return visibleControlEventForLabel(buttonText);
      }
    }
  }
  return null;
}

async function waitForTaskContinuityProposalStatus(
  page: Page,
  taskSessionId: string,
  expectedStatuses: string[]
) {
  await expect
    .poll(
      async () => {
        const detail = await tauriInvoke<any>(page, "get_main_chat_agent_task_detail", {
          taskSessionId,
          task_session_id: taskSessionId,
        });
        return (detail?.proposals ?? []).some((proposal: any) =>
          expectedStatuses.includes(proposal?.status ?? "")
        );
      },
      { timeout: 10_000 }
    )
    .toBe(true);
}

async function accessibleButtonLabel(button: Locator, fallback: string): Promise<string> {
  return button
    .evaluate(element =>
      [
        (element as HTMLElement).innerText,
        element.textContent,
        element.getAttribute("aria-label"),
        element.getAttribute("title"),
      ]
        .filter(Boolean)
        .join(" ")
        .trim()
    )
    .then(label => label || fallback)
    .catch(() => fallback);
}

async function openTaskContinuityDetail(page: Page, taskSessionId?: string): Promise<string> {
  await expect(page.getByTestId("task-continuity")).toBeVisible({ timeout: 30_000 });
  const summaries = page.getByTestId("task-continuity-summary");
  const count = await summaries.count();
  if (count === 0) throw new Error("stage1_task_continuity_summary_missing");
  let target: Locator | null = null;
  if (taskSessionId) {
    for (let index = 0; index < count; index += 1) {
      const candidate = summaries.nth(index);
      if ((await candidate.getAttribute("data-task-session-id")) === taskSessionId) {
        target = candidate;
        break;
      }
    }
    if (!target) throw new Error(`stage1_task_continuity_summary_missing:${taskSessionId}`);
  }
  await (target ?? summaries.first()).click();
  await expect(page.getByTestId("task-continuity-detail")).toBeVisible({ timeout: 10_000 });
  return "visible_control.task_continuity_detail_opened";
}

function visibleControlEventForLabel(label: string): string {
  const normalized =
    label
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "button";
  return `visible_control.${normalized}`;
}

function observedRuntimeStrategyEvent(routeStrategy: string): string {
  const normalized = metadataSafeBlocker(`observed_runtime_strategy:${routeStrategy || "unknown"}`);
  return metadataSafeLabel(normalized) ? normalized : "observed_runtime_strategy:unknown";
}

async function observeFromControlPlane(
  page: Page,
  controlPlane: Locator,
  scenario: Stage1DogfoodScenario,
  gateRow: MainChatAgentStage1DogfoodReport["scenarios"][number],
  visibleControlEvents: string[] = []
): Promise<Stage1ObservedBrowserScenario> {
  const attrs = await readControlPlaneAttrs(controlPlane);
  const snapshot = await tauriInvoke<any>(page, "get_main_chat_agent_state_snapshot", {
    taskSessionId: attrs.taskSessionId,
    task_session_id: attrs.taskSessionId,
  });
  const events = await taskEvents(page, attrs.taskSessionId);
  const text = (await controlPlane.textContent()) ?? "";
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
    uiStateObserved: visibleUiStates.length === scenario.expectedUiStates.length,
    finalDeliveryObserved: finalDeliverySections.length === scenario.expectedFinalSections.length,
    nonFakeEvidenceObserved: Boolean(snapshot?.task?.taskId && attrs.taskSessionId),
    legacyFallbackUsed: text.includes("Fallback notice"),
    silentDurableWriteDetected: false,
    fakeExecutionDetected: false,
  };
}

async function observeFromTaskContinuity(
  page: Page,
  scenario: Stage1DogfoodScenario,
  gateRow: MainChatAgentStage1DogfoodReport["scenarios"][number],
  visibleControlEvents: string[] = [],
  preObserved: {
    visibleUiStates?: string[];
    finalDeliverySections?: string[];
    visibleBlockers?: string[];
  } = {}
): Promise<Stage1ObservedBrowserScenario> {
  const evidence = await readTaskContinuityEvidence(page, scenario);

  return {
    scenarioId: scenario.id,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: gateRow.entryPoint,
    taskSessionId: evidence.taskSessionId,
    runId: evidence.runId,
    routeStrategy: gateRow.routeStrategy,
    runtimeEvents: uniqueValues([...evidence.runtimeEvents, ...visibleControlEvents]),
    visibleUiStates: uniqueValues([
      ...(preObserved.visibleUiStates ?? []),
      ...evidence.visibleUiStates,
    ]),
    finalDeliverySections: uniqueValues([
      ...(preObserved.finalDeliverySections ?? []),
      ...evidence.finalDeliverySections,
    ]),
    visibleBlockers: uniqueValues([
      ...(preObserved.visibleBlockers ?? []),
      ...evidence.visibleBlockers,
    ]),
    runtimeEvidenceObserved: evidence.runtimeEvidenceObserved,
    uiStateObserved: scenario.expectedUiStates.every(state =>
      uniqueValues([...(preObserved.visibleUiStates ?? []), ...evidence.visibleUiStates]).includes(
        state
      )
    ),
    finalDeliveryObserved: scenario.expectedFinalSections.every(section =>
      uniqueValues([
        ...(preObserved.finalDeliverySections ?? []),
        ...evidence.finalDeliverySections,
      ]).includes(section)
    ),
    nonFakeEvidenceObserved: evidence.nonFakeEvidenceObserved,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    fakeExecutionDetected: false,
  };
}

async function readTaskContinuityEvidence(
  page: Page,
  scenario: Stage1DogfoodScenario
): Promise<{
  taskSessionId: string;
  runId: string;
  routeStrategy: string;
  runtimeEvents: string[];
  visibleUiStates: string[];
  finalDeliverySections: string[];
  visibleBlockers: string[];
  runtimeEvidenceObserved: boolean;
  nonFakeEvidenceObserved: boolean;
}> {
  const detail = page.getByTestId("task-continuity-detail");
  await expect(detail).toBeVisible({ timeout: 10_000 });
  const taskSessionId = (await detail.getAttribute("data-task-session-id")) ?? "";
  const runId = (await detail.getAttribute("data-run-id")) ?? "";
  const routeStrategy = (await detail.getAttribute("data-task-strategy")) ?? "";
  const status = (await detail.getAttribute("data-task-status")) ?? "";
  const nextControl = (await detail.getAttribute("data-next-control")) ?? "";
  const finalDelivery = page.getByTestId("task-continuity-final-delivery");
  const finalTitles =
    (await finalDelivery.count()) > 0
      ? ((await finalDelivery.getAttribute("data-final-delivery-section-titles")) ?? "")
      : "";
  const taskDetail = await tauriInvoke<any>(page, "get_main_chat_agent_task_detail", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
  const events = await taskEvents(page, taskSessionId);
  const visibleUiStates = scenario.expectedUiStates.filter(state =>
    continuityUiStateObserved(state, status, nextControl, taskDetail)
  );
  const finalDeliverySections = scenario.expectedFinalSections.filter(section =>
    finalSectionObserved(section, finalTitles.split("|"), taskDetail)
  );
  const visibleBlockers = visibleBlockersForScenario(scenario, taskDetail);
  const runtimeStrategyEvent = observedRuntimeStrategyEvent(
    routeStrategy || taskDetail?.taskSession?.selectedStrategy || ""
  );

  return {
    taskSessionId,
    runId,
    routeStrategy: routeStrategy || taskDetail?.taskSession?.selectedStrategy || "",
    runtimeEvents: uniqueValues([...events, ...transcriptEvents(taskDetail), runtimeStrategyEvent]),
    visibleUiStates,
    finalDeliverySections,
    visibleBlockers,
    runtimeEvidenceObserved:
      taskSessionId.length > 0 && (events.length > 0 || taskDetail?.transcript?.length > 0),
    nonFakeEvidenceObserved: Boolean(taskDetail?.taskSession?.id),
  };
}

async function readControlPlaneAttrs(controlPlane: Locator) {
  return {
    taskSessionId: (await controlPlane.getAttribute("data-task-session-id")) ?? "",
    runId: (await controlPlane.getAttribute("data-run-id")) ?? "",
    routeStrategy: (await controlPlane.getAttribute("data-route-strategy")) ?? "",
    taskStatus: (await controlPlane.getAttribute("data-task-status")) ?? "",
    actionCount: Number((await controlPlane.getAttribute("data-action-count")) ?? "0"),
    observationCount: Number((await controlPlane.getAttribute("data-observation-count")) ?? "0"),
    blockerCount: Number((await controlPlane.getAttribute("data-blocker-count")) ?? "0"),
    proposalCount: Number((await controlPlane.getAttribute("data-proposal-count")) ?? "0"),
    finalDeliverySectionTitles: (
      (await controlPlane.getAttribute("data-final-delivery-section-titles")) ?? ""
    )
      .split("|")
      .filter(Boolean),
  };
}

async function taskEvents(page: Page, taskSessionId: string): Promise<string[]> {
  if (!taskSessionId) return [];
  const events = await tauriInvoke<Array<{ eventType?: string }>>(
    page,
    "list_main_chat_agent_events",
    {
      taskSessionId,
      task_session_id: taskSessionId,
      afterSequence: 0,
      after_sequence: 0,
      limit: 100,
    }
  );
  return uniqueValues(
    events.map(event => event.eventType).filter((event): event is string => Boolean(event))
  );
}

function uiStateObserved(
  state: string,
  attrs: Awaited<ReturnType<typeof readControlPlaneAttrs>>,
  snapshot: any
): boolean {
  if (state === "answering")
    return attrs.routeStrategy.includes("direct") || attrs.actionCount === 0;
  if (state === "planning")
    return Boolean(snapshot?.plan) || /plan|react|mcp|skill/i.test(attrs.routeStrategy);
  if (state === "action_running")
    return attrs.actionCount > 0 || (snapshot?.actions?.length ?? 0) > 0;
  if (state === "observation_ready")
    return attrs.observationCount > 0 || (snapshot?.observations?.length ?? 0) > 0;
  if (state === "completed")
    return (
      statusHasAny(attrs.taskStatus, ["completed", "delivered", "succeeded", "blocked"]) ||
      Boolean(snapshot?.finalDelivery)
    );
  if (state === "memory_candidate")
    return (
      attrs.proposalCount > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0 ||
      finalDeliveryArrayLength(snapshot, "proposalsCreated") > 0
    );
  if (state === "permission_needed")
    return (
      attrs.blockerCount > 0 ||
      attrs.proposalCount > 0 ||
      (snapshot?.blockers?.length ?? 0) > 0 ||
      (snapshot?.proposals?.length ?? 0) > 0 ||
      finalDeliveryArrayLength(snapshot, "pendingUserActions") > 0
    );
  if (state === "blocked")
    return (
      attrs.blockerCount > 0 || statusHasAny(attrs.taskStatus, ["blocked", "waiting_permission"])
    );
  if (state === "retry_available")
    return (
      (snapshot?.actions ?? []).some((action: any) => action?.retryable) ||
      hasAnyStructuredControl(snapshot?.blockers, ["retry", "resume", "refresh_context"]) ||
      hasAnyStructuredControl(snapshot?.proposals, ["retry", "resume", "refresh_context"])
    );
  if (state === "replaying_events")
    return snapshotEvents(snapshot).some(event =>
      statusHasAny(event, ["replaying_events", "stream_recovered", "event_stream"])
    );
  return false;
}

function continuityUiStateObserved(
  state: string,
  status: string,
  nextControl: string,
  detail: any
): boolean {
  if (state === "completed")
    return (
      statusHasAny(status, ["completed", "cancelled", "blocked"]) || Boolean(detail?.finalDelivery)
    );
  if (state === "blocked")
    return (
      detail?.blockers?.length > 0 ||
      statusHasAny(status, ["blocked", "waiting_permission", "stale"]) ||
      detail?.continuityDiagnostics?.staleContext
    );
  if (state === "retry_available")
    return (
      hasControlName(detail?.allowedControls, ["retry", "resume", "refresh_context"]) ||
      controlNameMatches(nextControl, ["retry", "resume", "refresh_context"])
    );
  if (state === "observation_ready")
    return (
      detail?.actions?.some((action: any) => action.observationMetadata) ||
      (detail?.actions ?? []).some((action: any) => (action?.observationIds?.length ?? 0) > 0) ||
      finalDeliveryArrayLength(detail, "observationsUsed") > 0 ||
      transcriptEvents(detail).includes("transcript.observation")
    );
  if (state === "memory_candidate")
    return (
      detail?.proposals?.length > 0 || finalDeliveryArrayLength(detail, "proposalsCreated") > 0
    );
  if (state === "planning")
    return (
      Boolean(detail?.plan) ||
      hasControlName(detail?.allowedControls, ["skip", "skip_step"]) ||
      controlNameMatches(nextControl, ["skip", "skip_step"])
    );
  if (state === "replaying_events")
    return (
      detail?.continuityDiagnostics?.automaticReplayAllowed ||
      hasControlName(detail?.allowedControls, ["replay", "refresh", "refresh_context"]) ||
      controlNameMatches(nextControl, ["replay", "refresh", "refresh_context"])
    );
  return false;
}

function finalSectionObserved(
  section: string,
  visibleTitles: string[],
  snapshot: any,
  controlEvents: string[] = []
): boolean {
  const delivery = snapshot?.finalDelivery ?? snapshot?.final_delivery;
  const deliveryMetrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  if (section === "completed_work") return Boolean(delivery);
  if (section === "observations_used")
    return (
      visibleTitles.includes("Sources used") ||
      arrayLength(deliveryMetrics, "observationsUsed") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "observationsUsed") > 0 ||
      transcriptEvents(snapshot).includes("transcript.observation")
    );
  if (section === "next_action")
    return (
      visibleTitles.includes("Next steps") ||
      arrayLength(deliveryMetrics, "nextSteps") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "recommendedNextAction") > 0 ||
      controlNameMatches(snapshot?.nextRecommendedControl, [
        "retry",
        "resume",
        "refresh_context",
        "cancel",
        "deny",
        "defer",
      ]) ||
      hasControlName(snapshot?.allowedControls, [
        "retry",
        "resume",
        "refresh_context",
        "cancel",
        "deny",
        "defer",
      ]) ||
      controlEvents.some(event =>
        [
          "visible_control.resume_task_from_continuity_detail",
          "visible_control.cancel_task_from_continuity_detail",
          "visible_control.refresh_task_context",
        ].some(prefix => seededVisibleControlEventMatchesPrefix(event, prefix))
      )
    );
  if (section === "proposals_created" || section === "proposed_work")
    return (
      visibleTitles.includes("Proposals created") ||
      arrayLength(deliveryMetrics, "proposalsCreated") > 0
    );
  if (section === "pending_user_action")
    return (
      visibleTitles.includes("Pending user actions") ||
      arrayLength(deliveryMetrics, "pendingUserActions") > 0
    );
  if (section === "blocked_work")
    return visibleTitles.includes("Blocked items") || arrayLength(deliveryMetrics, "blockers") > 0;
  if (section === "skipped_work")
    return (
      visibleTitles.includes("Skipped work") ||
      visibleTitles.includes("Skipped") ||
      visibleTitles.includes("Skipped items") ||
      arrayLength(deliveryMetrics, "skippedWork") > 0 ||
      arrayLength(deliveryMetrics, "skippedActions") > 0 ||
      arrayLength(snapshot?.plan?.reviewSummary, "skippedSteps") > 0 ||
      arrayIncludes(deliveryMetrics, "sections", "skipped") ||
      controlEvents.some(event =>
        seededVisibleControlEventMatchesPrefix(event, "visible_control.skip_step")
      )
    );
  if (section === "durable_changes")
    return (
      visibleTitles.includes("Durable changes") ||
      arrayLength(deliveryMetrics, "durableChanges") > 0 ||
      snapshotEvents(snapshot).includes("memory.materialized") ||
      snapshotEvents(snapshot).includes("memory.rolled_back")
    );
  return false;
}

function visibleBlockersForScenario(scenario: Stage1DogfoodScenario, evidence: any): string[] {
  if (!scenario.expectedBlocker) return [];
  const blockerEvidence =
    evidence?.blockers?.length > 0 ||
    evidence?.finalDelivery?.blockers?.length > 0 ||
    evidence?.final_delivery?.blockers?.length > 0 ||
    evidence?.finalDelivery?.metadata?.blockers?.length > 0 ||
    evidence?.final_delivery?.metadata?.blockers?.length > 0;
  return blockerEvidence ? [scenario.expectedBlocker] : [];
}

function arrayLength(value: any, key: string): number {
  const item = value?.[key];
  return Array.isArray(item) ? item.length : 0;
}

function arrayIncludes(value: any, key: string, expected: string): boolean {
  const item = value?.[key];
  return Array.isArray(item) && item.includes(expected);
}

function seededVisibleControlEventMatchesPrefix(event: string, prefix: string): boolean {
  return event === prefix || event.startsWith(`${prefix}_`);
}

function finalDeliveryArrayLength(value: any, key: string): number {
  const delivery = value?.finalDelivery ?? value?.final_delivery;
  const deliveryMetrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  return arrayLength(deliveryMetrics, key);
}

function statusHasAny(value: unknown, tokens: string[]): boolean {
  const normalized = String(value ?? "").toLowerCase();
  return tokens.some(token => normalized.includes(token));
}

function controlNameMatches(value: unknown, controls: string[]): boolean {
  const normalized = String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return controls.some(control => normalized === control || normalized.includes(control));
}

function hasControlName(values: unknown, controls: string[]): boolean {
  return Array.isArray(values) && values.some(value => controlNameMatches(value, controls));
}

function hasAnyStructuredControl(values: unknown, controls: string[]): boolean {
  return (
    Array.isArray(values) &&
    values.some(value => {
      const record = value as { controls?: unknown; nextRecommendedControl?: unknown };
      return (
        hasControlName(record.controls, controls) ||
        controlNameMatches(record.nextRecommendedControl, controls)
      );
    })
  );
}

function snapshotEvents(snapshot: any): string[] {
  return uniqueValues(
    (snapshot?.events ?? []).map((event: any) => event.eventType).filter(Boolean)
  );
}

function transcriptEvents(detail: any): string[] {
  return uniqueValues(
    (detail?.transcript ?? []).map((entry: any) => `transcript.${entry.kind}`).filter(Boolean)
  );
}

function stage1BrowserPrepBlockers(prepReport: Stage1BrowserPrepReport): string[] {
  const blockers: string[] = [];
  if (!prepReport.prepared) blockers.push("stage1_browser_prep_not_prepared");
  if (prepReport.evidenceSource !== "real_app_state_task_continuity_seed") {
    blockers.push("stage1_browser_prep_source_invalid");
  }
  if (prepReport.directWritesExecuted) blockers.push("stage1_browser_prep_direct_write_detected");
  if (prepReport.durableLifemodelWritesExecuted) {
    blockers.push("stage1_browser_prep_durable_lifemodel_write_detected");
  }
  if (prepReport.fileOrExternalWritesExecuted) {
    blockers.push("stage1_browser_prep_file_or_external_write_detected");
  }
  if ((prepReport.blockers?.length ?? 0) > 0) {
    blockers.push("stage1_browser_prep_report_blockers_present");
  }
  for (const id of REQUIRED_STAGE1_PREP_TASK_IDS) {
    const taskSessionId = prepReport.taskSessionIds?.[id] ?? "";
    if (!taskSessionId) {
      blockers.push(`stage1_browser_prep_missing_seeded_task:${id}`);
    } else if (!metadataSafeLabel(taskSessionId) || taskSessionId.startsWith("stage1_task_")) {
      blockers.push(`stage1_browser_prep_task_id_unsafe:${id}`);
    }
  }
  return uniqueValues(blockers);
}

function metadataSafeLabel(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 96 &&
    value.trim() === value &&
    [...value].every(ch => /[A-Za-z0-9_.:/-]/.test(ch))
  );
}

function metadataSafeBlocker(value: string): string {
  return (
    String(value)
      .replace(/[^A-Za-z0-9_.:/-]+/g, "_")
      .replace(/^_+|_+$/g, "")
      .slice(0, 160) || "unknown"
  );
}

function uniqueValues(values: string[]): string[] {
  return values.filter((value, index) => value && values.indexOf(value) === index);
}

test.describe("main-chat-stage1-dogfood", () => {
  test.setTimeout(900_000);

  test("exports browser evidence only after real Tauri Chat UI observes D01-D36", async ({
    page,
  }) => {
    expect(STAGE1_DOGFOOD_SCENARIOS.map(scenario => scenario.id)).toEqual(
      STAGE1_REQUIRED_BROWSER_JOURNEYS
    );
    await page.goto("/#/chat");
    const tauriAvailable = await page.evaluate(() => Boolean((window as any).__TAURI_INTERNALS__));

    if (!tauriAvailable) {
      const report = writeReport(
        buildStage1BlockedBrowserEvidenceReport([...stage1NonTauriBrowserBlockers()])
      );

      expect(report.selfContainedRunner).toBe(true);
      expect(report.smokePassed).toBe(false);
      expect(report.requiredJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
      expect(report.passedJourneys).toEqual([]);
      expect(report.failedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
      expect(report.observedScenarios).toEqual([]);
      expect(report.evidenceSource).not.toContain("frontend");
      expect(report.evidenceSource).not.toContain("fixture");
      expect(report.blockers).toEqual([
        "not_ready_browser_e2e_blocked",
        ...stage1NonTauriBrowserBlockers(),
      ]);
      return;
    }

    writeReport(
      buildStage1BlockedBrowserEvidenceReport([
        "real_tauri_browser_d01_d36_observation_in_progress",
      ])
    );

    let prepReport: Stage1BrowserPrepReport;
    let gateReport: MainChatAgentStage1DogfoodReport;
    try {
      prepReport = await prepareStage1BrowserState(page);
      const prepBlockers = stage1BrowserPrepBlockers(prepReport);
      if (prepBlockers.length > 0) {
        throw new Error(`stage1_browser_prep_not_ready:${prepBlockers.join(",")}`);
      }
      await page.goto("/#/chat");
      gateReport = await runStage1Gate(page);
    } catch (error) {
      writeReport(
        buildStage1BlockedBrowserEvidenceReport([
          "real_tauri_browser_command_surface_unavailable",
          String(error),
        ])
      );
      return;
    }

    const gateRows = new Map(gateReport.scenarios.map(row => [row.scenarioId, row]));
    const observedScenarios: Stage1ObservedBrowserScenario[] = [];

    try {
      for (const scenario of STAGE1_DOGFOOD_SCENARIOS) {
        const gateRow = gateRows.get(scenario.id);
        if (!gateRow) throw new Error(`stage1_gate_row_missing:${scenario.id}`);
        const observed =
          scenario.scenarioType === "chat_e2e"
            ? await executeChatScenario(page, scenario, gateRow)
            : await executeSeededControlScenario(page, scenario, gateRow, prepReport);
        observedScenarios.push(observed);
      }

      const report = writeReport(
        buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observedScenarios, gateReport)
      );
      expect(report.passedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
      expect(report.observedScenarios).toHaveLength(36);

      const finalGateReport = await runStage1Gate(page);
      expect(finalGateReport.defaultReady, finalGateReport.blockers.join(",")).toBe(true);
      expect(finalGateReport.readinessRecommendation).toBe("ready_for_engineering_dogfood");
    } catch (error) {
      writeReport(
        buildStage1BlockedBrowserEvidenceReport([
          "real_tauri_browser_d01_d36_observation_failed",
          String(error),
        ])
      );
      throw error;
    }
  });
});
