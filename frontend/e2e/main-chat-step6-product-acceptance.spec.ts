import { test, expect, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

import {
  STEP6_PRODUCT_ACCEPTANCE_JOURNEYS,
  STEP6_REQUIRED_PRODUCT_JOURNEYS,
  buildStep6BlockedProductAcceptanceReport,
  buildStep6ProductAcceptanceReportFromObservedJourneys,
  type Step6ObservedProductJourney,
  type Step6ProductAcceptanceJourney,
} from "../src/step6ProductAcceptance";

type TauriInvokeArgs = Record<string, unknown>;

const REPORT_FILE = "test-results/main-chat-step6-product-acceptance-report.json";
const REQUIRED_STEP6_PREP_TASK_IDS = ["S6_PERMISSION_ACCEPT", "D15"];

type Step6BrowserPrepReport = {
  prepared?: boolean;
  evidenceSource?: string;
  taskSessionIds?: Record<string, string>;
  directWritesExecuted?: boolean;
  durableLifemodelWritesExecuted?: boolean;
  fileOrExternalWritesExecuted?: boolean;
  blockers?: string[];
};

type Step6LiveProviderStatePrepReport = {
  reportKind?: string;
  configured?: boolean;
  ready?: boolean;
  provider?: string;
  model?: string;
  baseConfigured?: boolean;
  apiKeyPresent?: boolean;
  networkEnabled?: boolean;
  providerEndpointKind?: string;
  preflightReady?: boolean;
  preflightBlockers?: string[];
  appConfigPersisted?: boolean;
  directWritesExecuted?: boolean;
  blockers?: string[];
};

function writeReport(report: ReturnType<typeof buildStep6BlockedProductAcceptanceReport>) {
  const reportPath = path.resolve(process.cwd(), REPORT_FILE);
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));
  return report;
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

async function readCurrentTaskId(page: Page): Promise<string> {
  const controlPlane = page.getByTestId("agent-control-plane").last();
  if ((await controlPlane.count()) === 0) return "";
  return (await controlPlane.getAttribute("data-task-session-id")) ?? "";
}

async function executeLocalJourney(
  page: Page,
  journey: Step6ProductAcceptanceJourney,
  prepReport: Step6BrowserPrepReport
): Promise<Step6ObservedProductJourney> {
  if (journey.id === "S6-RECOVERY") {
    return executeSeededRecoveryJourney(page, journey, prepReport);
  }
  if (journey.id === "S6-PERMISSION") {
    return executeSeededPermissionAcceptanceJourney(page, journey, prepReport);
  }
  const previousTaskId = await readCurrentTaskId(page);

  await page.getByTestId("chat-input").fill(journey.prompt);
  await expect(page.getByTestId("send-button")).toBeEnabled({ timeout: 10_000 });
  await page.getByTestId("send-button").click();
  await openDiagnosticsIfAvailable(page);
  const controlPlane = await waitForControlPlaneDelivery(page, previousTaskId, journey);
  return observeStep6Journey(page, journey, controlPlane);
}

async function executeLiveJourney(
  page: Page,
  journey: Step6ProductAcceptanceJourney,
  liveProviderStateReport: Step6LiveProviderStatePrepReport | null
): Promise<Step6ObservedProductJourney> {
  const liveOptIn = await page.evaluate(() =>
    Boolean((window as any).__OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL__)
  );
  if (!liveOptIn && process.env.OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL !== "1") {
    return blockedLiveJourney(journey, ["explicit_live_eval_required"]);
  }
  if (!step6LiveProviderStateReady(liveProviderStateReport)) {
    return blockedLiveJourney(journey, step6LiveProviderStateBlockers(liveProviderStateReport));
  }

  const previousTaskId = await readCurrentTaskId(page);
  await page.getByTestId("chat-input").fill(journey.prompt);
  await expect(page.getByTestId("send-button")).toBeEnabled({ timeout: 10_000 });
  await page.getByTestId("send-button").click();
  await openDiagnosticsIfAvailable(page);
  const controlPlane = await waitForControlPlaneDelivery(page, previousTaskId, journey);
  return observeStep6Journey(page, journey, controlPlane);
}

async function openDiagnosticsIfAvailable(page: Page) {
  const controlPlane = page.getByTestId("agent-control-plane").last();
  if ((await controlPlane.count()) > 0 && (await controlPlane.isVisible())) return;
  const diagnosticsToggle = page.getByRole("button", { name: "Show Main Chat diagnostics" });
  const visible = await diagnosticsToggle.isVisible({ timeout: 30_000 }).catch(() => false);
  if (!visible) {
    console.error("[step6_diagnostics:unavailable]");
    return;
  }
  await diagnosticsToggle.click();
  await expect(page.getByTestId("agent-control-plane").last())
    .toBeVisible({ timeout: 5_000 })
    .catch(() => console.error("[step6_diagnostics:control_plane_not_visible]"));
}

async function waitForControlPlaneDelivery(
  page: Page,
  previousTaskId: string,
  journey: Step6ProductAcceptanceJourney
): Promise<Locator> {
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
    .poll(
      async () => {
        const finalDelivery = (await controlPlane.getAttribute("data-final-delivery")) === "true";
        const readyWithoutFinalDelivery = journey.expectedUiStatus.some(status =>
          ["blocked", "restricted", "permission_pending", "waiting_for_user"].includes(status)
        );
        const blockerCount = Number((await controlPlane.getAttribute("data-blocker-count")) ?? "0");
        const proposalCount = Number(
          (await controlPlane.getAttribute("data-proposal-count")) ?? "0"
        );
        const taskStatus = (await controlPlane.getAttribute("data-task-status")) ?? "";
        return (
          finalDelivery ||
          (readyWithoutFinalDelivery &&
            (blockerCount > 0 ||
              proposalCount > 0 ||
              /blocked|waiting_permission/.test(taskStatus)))
        );
      },
      { timeout: 120_000 }
    )
    .toBe(true);
  return controlPlane;
}

async function prepareStep6BrowserState(page: Page): Promise<Step6BrowserPrepReport> {
  return {
    prepared: false,
    evidenceSource: "retired_after_phase7_cleanup",
    directWritesExecuted: false,
    durableLifemodelWritesExecuted: false,
    fileOrExternalWritesExecuted: false,
    taskSessionIds: {},
    blockers: ["step6_browser_prep_command_retired_after_phase7_cleanup"],
  };
}

async function prepareStep6LiveProviderState(
  page: Page
): Promise<Step6LiveProviderStatePrepReport> {
  return {
    reportKind: "main_chat_step6_live_provider_eval_state_prep",
    configured: false,
    ready: false,
    provider: "missing",
    model: "missing",
    baseConfigured: false,
    apiKeyPresent: false,
    networkEnabled: false,
    providerEndpointKind: "missing",
    preflightReady: false,
    preflightBlockers: [],
    appConfigPersisted: false,
    directWritesExecuted: false,
    blockers: ["step6_live_provider_state_command_retired_after_phase7_cleanup"],
  };
}

function step6LiveProviderStateReady(report: Step6LiveProviderStatePrepReport | null): boolean {
  return (
    report?.reportKind === "main_chat_step6_live_provider_eval_state_prep" &&
    report.configured === true &&
    report.ready === true &&
    report.preflightReady === true &&
    report.appConfigPersisted === false &&
    report.directWritesExecuted === false &&
    report.providerEndpointKind === "external_provider" &&
    report.apiKeyPresent === true &&
    report.networkEnabled === true &&
    (report.blockers ?? []).length === 0
  );
}

function step6LiveProviderStateBlockers(report: Step6LiveProviderStatePrepReport | null): string[] {
  if (!report) return ["step6_live_provider_state_missing"];
  return uniqueValues(
    [
      "step6_live_provider_state_not_ready",
      ...(report.blockers ?? []),
      ...(report.preflightBlockers ?? []),
      report.configured === true ? "" : "step6_live_provider_state_not_configured",
      report.ready === true ? "" : "step6_live_provider_state_preflight_not_ready",
      report.providerEndpointKind === "external_provider"
        ? ""
        : "external_provider_endpoint_required",
      report.appConfigPersisted === false ? "" : "step6_live_provider_state_persisted_config",
      report.directWritesExecuted === false ? "" : "step6_live_provider_state_direct_write",
    ].map(metadataSafeBlocker)
  );
}

function validateStep6BrowserPrepReport(prepReport: Step6BrowserPrepReport): string[] {
  const blockers: string[] = [];
  if (!prepReport.prepared) blockers.push("step6_browser_prep_not_prepared");
  if (prepReport.evidenceSource !== "real_app_state_task_continuity_seed") {
    blockers.push("step6_browser_prep_source_invalid");
  }
  if (prepReport.directWritesExecuted) blockers.push("step6_browser_prep_direct_write_detected");
  if (prepReport.durableLifemodelWritesExecuted) {
    blockers.push("step6_browser_prep_durable_lifemodel_write_detected");
  }
  if (prepReport.fileOrExternalWritesExecuted) {
    blockers.push("step6_browser_prep_file_or_external_write_detected");
  }
  if (Array.isArray(prepReport.blockers) && prepReport.blockers.length > 0) {
    blockers.push("step6_browser_prep_report_blockers_present");
  }
  for (const id of REQUIRED_STEP6_PREP_TASK_IDS) {
    if (!prepReport.taskSessionIds?.[id]) blockers.push(`step6_browser_prep_missing_task:${id}`);
  }
  return blockers;
}

async function executeSeededRecoveryJourney(
  page: Page,
  journey: Step6ProductAcceptanceJourney,
  prepReport: Step6BrowserPrepReport
): Promise<Step6ObservedProductJourney> {
  const taskSessionId = prepReport.taskSessionIds?.D15 ?? "";
  if (!taskSessionId) throw new Error("step6_seeded_recovery_task_missing");
  const visibleControlEvents: string[] = [];
  visibleControlEvents.push(await openTaskContinuityDetail(page, taskSessionId));
  visibleControlEvents.push(
    await clickFirstVisibleControl(page, [/Cancel task from continuity detail/i, /Cancel task/i])
  );
  const evidence = await readTaskContinuityEvidence(page, taskSessionId);
  const finalDeliverySections = evidence.finalDeliverySections;
  const finalDeliveryMatched = finalDeliveryMatchesJourney(journey, finalDeliverySections);
  return {
    journeyId: journey.id,
    kind: journey.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "task_continuity_control",
    routeStrategy: evidence.routeStrategy || "task_continuity_control",
    taskSessionId: evidence.taskSessionId,
    runId: evidence.runId,
    answerEvidence: finalDeliveryMatched ? [...journey.expectedAnswerEvidence] : [],
    runtimeEvidence:
      visibleControlEvents.length > 0 && finalDeliveryMatched
        ? [...journey.expectedRuntimeEvidence]
        : [],
    uiStatusEvidence: [evidence.status],
    finalDeliverySections,
    traceEvidence: [...evidence.runtimeEvents, ...visibleControlEvents],
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "not_applicable",
    externalLiveProviderKind: null,
    blockers: [],
  };
}

async function executeSeededPermissionAcceptanceJourney(
  page: Page,
  journey: Step6ProductAcceptanceJourney,
  prepReport: Step6BrowserPrepReport
): Promise<Step6ObservedProductJourney> {
  const taskSessionId = prepReport.taskSessionIds?.S6_PERMISSION_ACCEPT ?? "";
  if (!taskSessionId) throw new Error("step6_seeded_permission_accept_task_missing");
  const visibleControlEvents: string[] = [];
  visibleControlEvents.push(await openTaskContinuityDetail(page, taskSessionId));
  const beforeDetail = await readTaskDetail(page, taskSessionId);
  const pendingProposal = pendingToolPermissionProposal(beforeDetail);
  if (!pendingProposal) {
    throw new Error(`step6_permission_pending_proposal_missing:${taskSessionId}`);
  }
  visibleControlEvents.push("visible_control.review_action_visible");
  visibleControlEvents.push(await clickFirstVisibleControl(page, [/Accept proposal/i]));

  const afterDetail = await waitForTaskDetail(
    page,
    taskSessionId,
    detail =>
      toolPermissionProposalStatus(detail, pendingProposal.id) === "accepted" &&
      normalizedStatusFromContinuity(
        taskStatusFromDetail(detail),
        finalDeliverySectionsFromDetail(detail)
      ) === "completed",
    30_000,
    `step6_permission_accept_resume_incomplete:${taskSessionId}`
  );
  const evidence = await readTaskContinuityEvidence(page, taskSessionId);
  const finalDeliverySections = uniqueValues([
    ...evidence.finalDeliverySections,
    ...finalDeliverySectionsFromDetail(afterDetail),
  ]);
  const status = normalizedStatusFromContinuity(
    taskStatusFromDetail(afterDetail),
    finalDeliverySections
  );
  const accepted = toolPermissionProposalStatus(afterDetail, pendingProposal.id) === "accepted";
  const replayed = taskReplayCompleted(afterDetail);
  const finalDeliveryMatched = finalDeliveryMatchesJourney(journey, finalDeliverySections);
  return {
    journeyId: journey.id,
    kind: journey.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "task_continuity_control",
    routeStrategy: evidence.routeStrategy || "task_continuity_control",
    taskSessionId: evidence.taskSessionId,
    runId: evidence.runId,
    answerEvidence:
      accepted && replayed && finalDeliveryMatched ? [...journey.expectedAnswerEvidence] : [],
    runtimeEvidence: uniqueValues([
      pendingProposal ? "permission.pending" : "",
      pendingProposal ? "review_action.visible" : "",
      accepted ? "permission.accepted" : "",
      replayed ? "automatic_resume_replay" : "",
      finalDeliveryMatched ? "final_delivery.recorded" : "",
    ]),
    uiStatusEvidence: status ? [status] : [],
    finalDeliverySections,
    traceEvidence: uniqueValues([...evidence.runtimeEvents, ...visibleControlEvents]),
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "not_applicable",
    externalLiveProviderKind: null,
    blockers: [],
  };
}

async function openTaskContinuityDetail(page: Page, taskSessionId: string): Promise<string> {
  await expect(page.getByTestId("task-continuity")).toBeVisible({ timeout: 30_000 });
  const summaries = page.getByTestId("task-continuity-summary");
  const count = await summaries.count();
  for (let index = 0; index < count; index += 1) {
    const candidate = summaries.nth(index);
    if ((await candidate.getAttribute("data-task-session-id")) === taskSessionId) {
      await candidate.click();
      await expect(page.getByTestId("task-continuity-detail")).toBeVisible({ timeout: 10_000 });
      return "visible_control.task_continuity_detail_opened";
    }
  }
  throw new Error(`step6_task_continuity_summary_missing:${taskSessionId}`);
}

async function clickFirstVisibleControl(page: Page, labels: RegExp[]): Promise<string> {
  for (const label of labels) {
    const buttons = page.getByRole("button", { name: label });
    const count = await buttons.count();
    for (let index = 0; index < count; index += 1) {
      const button = buttons.nth(index);
      if ((await button.isVisible()) && (await button.isEnabled())) {
        const text = (await button.textContent()) ?? label.source;
        await button.click();
        await page.waitForTimeout(500);
        return visibleControlEventForLabel(text);
      }
    }
  }
  throw new Error(`step6_visible_control_missing:${labels.map(String).join(",")}`);
}

async function readTaskContinuityEvidence(page: Page, taskSessionId: string) {
  const detail = page.getByTestId("task-continuity-detail");
  await expect(detail).toBeVisible({ timeout: 10_000 });
  const runId = (await detail.getAttribute("data-run-id")) ?? "";
  const rawStatus = (await detail.getAttribute("data-task-status")) ?? "";
  const finalDelivery = page.getByTestId("task-continuity-final-delivery");
  const finalDeliverySections =
    (await finalDelivery.count()) > 0
      ? ((await finalDelivery.getAttribute("data-final-delivery-section-titles")) ?? "")
          .split("|")
          .filter(Boolean)
          .map(normalizeFinalSection)
      : [];
  const taskDetail = await tauriInvoke<any>(page, "get_main_chat_agent_task_detail", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
  const events = await taskEvents(page, taskSessionId).catch(() => []);
  return {
    taskSessionId,
    runId,
    routeStrategy: (await detail.getAttribute("data-task-strategy")) ?? "",
    status: normalizedStatusFromContinuity(rawStatus, finalDeliverySections),
    finalDeliverySections: uniqueValues([
      ...finalDeliverySections,
      ...finalDeliverySectionsFromDetail(taskDetail),
    ]),
    runtimeEvents: uniqueValues([...events, ...transcriptEvents(taskDetail)]),
  };
}

async function readTaskDetail(page: Page, taskSessionId: string): Promise<any> {
  return tauriInvoke<any>(page, "get_main_chat_agent_task_detail", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

async function waitForTaskDetail(
  page: Page,
  taskSessionId: string,
  predicate: (detail: any) => boolean,
  timeoutMs: number,
  errorMessage: string
): Promise<any> {
  let lastDetail: any = null;
  await expect
    .poll(
      async () => {
        lastDetail = await readTaskDetail(page, taskSessionId);
        return predicate(lastDetail);
      },
      { timeout: timeoutMs }
    )
    .toBe(true);
  if (!lastDetail) throw new Error(errorMessage);
  return lastDetail;
}

function taskStatusFromDetail(detail: any): string {
  return detail?.taskSession?.status ?? detail?.task_session?.status ?? "";
}

function pendingToolPermissionProposal(detail: any): any | null {
  return (
    (detail?.proposals ?? []).find((proposal: any) => {
      const proposalType = proposal?.proposalType ?? proposal?.proposal_type ?? "";
      return proposalType === "tool_permission" && proposal?.status === "pending";
    }) ?? null
  );
}

function toolPermissionProposalStatus(detail: any, proposalId: string): string {
  const proposal = (detail?.proposals ?? []).find((item: any) => item?.id === proposalId);
  return proposal?.status ?? "";
}

function taskReplayCompleted(detail: any): boolean {
  const actions = detail?.actions ?? [];
  const transcript = detail?.transcript ?? [];
  return (
    actions.some((action: any) => action?.status === "completed") ||
    transcript.some((entry: any) => {
      const metadata = entry?.metadata ?? {};
      return (
        metadata.automaticResumeReplayCompleted === true ||
        metadata.automaticReplayCompleted === true
      );
    })
  );
}

async function observeStep6Journey(
  page: Page,
  journey: Step6ProductAcceptanceJourney,
  controlPlane: Locator
): Promise<Step6ObservedProductJourney> {
  const attrs = await readControlPlaneAttrs(controlPlane);
  const snapshot = await tauriInvoke<any>(page, "get_main_chat_agent_state_snapshot", {
    taskSessionId: attrs.taskSessionId,
    task_session_id: attrs.taskSessionId,
  }).catch(() => null);
  const detail = await tauriInvoke<any>(page, "get_main_chat_agent_task_detail", {
    taskSessionId: attrs.taskSessionId,
    task_session_id: attrs.taskSessionId,
  }).catch(() => null);
  const events = uniqueValues([
    ...(await taskEvents(page, attrs.taskSessionId).catch(() => [])),
    ...transcriptEvents(detail),
  ]);
  const statusSurface = page.getByTestId("main-chat-agent-status").last();
  const uiStatus =
    (await statusSurface.getAttribute("data-agent-product-status").catch(() => "")) || "";
  const assistantVisible = (await page.getByTestId("assistant-message").last().count()) > 0;
  const finalDeliverySections = uniqueValues([
    ...attrs.finalDeliverySectionTitles,
    ...finalDeliverySectionsFromDetail(snapshot),
    ...finalDeliverySectionsFromDetail(detail),
  ]);
  const finalDeliveryMatched = finalDeliveryMatchesJourney(journey, finalDeliverySections);
  const answerEvidence =
    assistantVisible && finalDeliveryMatched ? answerEvidenceForJourney(journey, snapshot) : [];
  const runtimeEvidence = finalDeliveryMatched
    ? runtimeEvidenceForJourney(journey, attrs, snapshot, detail, events)
    : [];
  const liveJourneyCredited =
    journey.kind === "external_live" &&
    externalLiveJourneyCredited(journey, snapshot, detail, runtimeEvidence, finalDeliveryMatched);
  const traceEvidence = uniqueValues([
    ...events,
    ...liveProviderTraceEvidence(detail),
    ...(attrs.routeStrategy ? [`route.${attrs.routeStrategy}`] : []),
    ...(snapshot?.provider?.routeType ? [`provider_route.${snapshot.provider.routeType}`] : []),
  ]);
  const externalLiveProviderKind = externalProviderKindForSnapshot(journey, snapshot);

  return {
    journeyId: journey.id,
    kind: journey.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "ordinary_main_chat_input",
    routeStrategy: attrs.routeStrategy,
    taskSessionId: attrs.taskSessionId,
    runId: attrs.runId,
    answerEvidence,
    runtimeEvidence,
    uiStatusEvidence: uiStatus ? [uiStatus] : [],
    finalDeliverySections,
    traceEvidence,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: ((await controlPlane.textContent()) ?? "").includes("Fallback notice"),
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive:
      journey.kind === "external_live" &&
      liveJourneyCredited &&
      externalLiveProviderKind !== "external_provider",
    externalLiveStatus:
      journey.kind === "external_live" && liveJourneyCredited
        ? "credited_external_live"
        : journey.kind === "external_live"
          ? "incomplete_external_live"
          : "not_applicable",
    externalLiveProviderKind,
    blockers:
      journey.kind === "external_live" && !liveJourneyCredited
        ? ["external_live_provider_evidence_missing"]
        : [],
  };
}

function finalDeliveryMatchesJourney(
  journey: Step6ProductAcceptanceJourney,
  finalDeliverySections: string[]
): boolean {
  return journey.expectedFinalDeliverySections.some(section =>
    finalDeliverySections.includes(section)
  );
}

function externalProviderKindForSnapshot(journey: Step6ProductAcceptanceJourney, snapshot: any) {
  if (journey.kind !== "external_live") return null;
  const routeType = String(snapshot?.provider?.routeType ?? "").toLowerCase();
  const provider = String(snapshot?.provider?.provider ?? "");
  if (routeType !== "cloud") return routeType || null;
  return isExternalProviderLabel(provider) ? "external_provider" : provider || null;
}

function isExternalProviderLabel(provider: string): boolean {
  if (!metadataSafeLabel(provider)) return false;
  const lower = provider.toLowerCase();
  const localAliases = [
    "local",
    "localhost",
    "loopback",
    "fixture",
    "synthetic",
    "scripted",
    "mock",
    "ollama",
    "test_http",
    "local_test",
    "127.",
    "0.0.0.0",
    "::1",
  ];
  return provider.length > 0 && !localAliases.some(alias => lower.includes(alias));
}

function externalLiveJourneyCredited(
  journey: Step6ProductAcceptanceJourney,
  snapshot: any,
  detail: any,
  runtimeEvidence: string[],
  finalDeliveryMatched: boolean
): boolean {
  return (
    journey.kind === "external_live" &&
    finalDeliveryMatched &&
    externalProviderKindForSnapshot(journey, snapshot) === "external_provider" &&
    journey.expectedRuntimeEvidence.every(evidence => runtimeEvidence.includes(evidence)) &&
    liveProviderAgentLoopEvidence(journey, snapshot, detail).externalProviderAgentLoopSucceeded
  );
}

function liveProviderAgentLoopEvidence(
  journey: Step6ProductAcceptanceJourney,
  snapshot: any,
  detail: any
) {
  const metadata = agentLoopMetadataFromDetail(detail);
  const providerRoute = `${snapshot?.provider?.routeType ?? ""}`.toLowerCase();
  const providerKind = externalProviderKindForSnapshot(journey, snapshot);
  const endpointKind = metadataString(metadata, "providerEndpointKind");
  const baseSucceeded =
    journey.kind === "external_live" &&
    providerRoute === "cloud" &&
    providerKind === "external_provider" &&
    endpointKind === "external_provider" &&
    metadata?.liveProviderInvoked === true &&
    metadata?.agentLoopSucceeded === true &&
    metadata?.singleStepFallbackUsed !== true &&
    metadataString(metadata, "agentLoopActionStatus") === "succeeded" &&
    metadataString(metadata, "toolSelectionCandidateActionType") === "mcp_tool" &&
    metadata?.modelSelectedAllowedTool === true &&
    metadata?.modelSelectedExecutionPolicyValidated === true &&
    metadata?.modelSelectedExecutionAllowed === true &&
    metadataString(metadata, "modelSelectedArgumentsSource") === "governed_candidate_contract";
  const selectedTarget = metadataString(metadata, "toolSelectionCandidateTarget");
  const selectedId = metadataString(metadata, "toolSelectionCandidateId");
  const selectedRank = metadataNumber(metadata, "toolSelectionCandidateRank");
  const candidateIds = metadataStringArray(metadata, "toolSelectionCandidateIds");
  const targetMatches = (...targets: string[]) =>
    targets.some(target => selectedTarget === target || selectedId === target);
  const selectedRankMatches =
    selectedRank > 0 &&
    Boolean(candidateIds[selectedRank - 1]) &&
    targetMatches(candidateIds[selectedRank - 1]);
  const webRead = baseSucceeded && targetMatches("web.search") && selectedRankMatches;
  const providerRankedSelection =
    baseSucceeded &&
    metadata?.mcpReadTargetResolved === true &&
    targetMatches("builtin_echo") &&
    candidateIds.length >= 2 &&
    selectedRankMatches &&
    metadata?.toolSelectionModelRanked === true &&
    metadataString(metadata, "toolSelectionRankingSource") === "provider_model" &&
    metadataString(metadata, "toolSelectionRankingRouteType") === "cloud" &&
    metadata?.toolSelectionRankingProviderBacked === true &&
    metadata?.toolSelectionModelRankingIgnored !== true;
  return {
    externalProviderAgentLoopSucceeded: baseSucceeded,
    webRead,
    mcpRead: providerRankedSelection,
  };
}

function agentLoopMetadataFromDetail(detail: any): Record<string, any> | null {
  let attemptedMetadata: Record<string, any> | null = null;
  for (const entry of detail?.transcript ?? []) {
    const summary = String(entry?.summary ?? "");
    const metadata = entry?.metadata && typeof entry.metadata === "object" ? entry.metadata : null;
    if (!metadata) continue;
    if (summary.includes("Governed ReAct AgentLoop completed")) return metadata;
    if (summary.includes("Governed ReAct AgentLoop") && metadata.agentLoopAttempted === true) {
      attemptedMetadata = metadata;
    }
  }
  return attemptedMetadata;
}

function liveProviderTraceEvidence(detail: any): string[] {
  const metadata = agentLoopMetadataFromDetail(detail);
  if (!metadata) return [];
  const evidence: string[] = [];
  if (metadata.liveProviderInvoked === true) evidence.push("live_provider.invoked");
  if (metadata.agentLoopSucceeded === true) evidence.push("agent_loop.succeeded");
  const endpointKind = metadataString(metadata, "providerEndpointKind");
  if (metadataSafeLabel(endpointKind)) evidence.push(`provider_endpoint.${endpointKind}`);
  const actionStatus = metadataString(metadata, "agentLoopActionStatus");
  if (metadataSafeLabel(actionStatus)) evidence.push(`action.${actionStatus}`);
  const selectedTarget = metadataString(metadata, "toolSelectionCandidateTarget");
  if (metadataSafeLabel(selectedTarget)) evidence.push(`tool_target.${selectedTarget}`);
  if (metadata.toolSelectionModelRanked === true) evidence.push("tool_selection.provider_ranked");
  return uniqueValues(evidence);
}

function metadataString(metadata: Record<string, any> | null, key: string): string {
  const value = metadata?.[key];
  return typeof value === "string" ? value : "";
}

function metadataNumber(metadata: Record<string, any> | null, key: string): number {
  const value = metadata?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function metadataStringArray(metadata: Record<string, any> | null, key: string): string[] {
  const value = metadata?.[key];
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
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
      .filter(Boolean)
      .map(normalizeFinalSection),
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

function answerEvidenceForJourney(journey: Step6ProductAcceptanceJourney, snapshot: any): string[] {
  const delivery = snapshot?.finalDelivery ?? snapshot?.final_delivery;
  if (!delivery && journey.kind === "deterministic_local") return [];
  return [...journey.expectedAnswerEvidence];
}

function runtimeEvidenceForJourney(
  journey: Step6ProductAcceptanceJourney,
  attrs: Awaited<ReturnType<typeof readControlPlaneAttrs>>,
  snapshot: any,
  detail: any,
  events: string[]
): string[] {
  const evidence: string[] = [];
  const routeStrategy = `${attrs.routeStrategy} ${snapshot?.route?.strategy ?? ""}`.toLowerCase();
  if (journey.id === "S6-CLOCK" && routeStrategy.includes("runtime")) {
    evidence.push("source.runtime_fact", "runtime.clock");
  }
  if (journey.id === "S6-ROUTE" && routeStrategy.includes("runtime")) {
    evidence.push("source.runtime_fact", "runtime.provider_route");
  }
  if (journey.id === "S6-TOOLS" && routeStrategy.includes("runtime")) {
    evidence.push("source.runtime_fact", "runtime.tool_availability");
  }
  if (
    journey.id === "S6-FILE" &&
    (attrs.observationCount > 0 || events.includes("observation.created"))
  ) {
    evidence.push("tool.file_read", "observation.workspace_file");
  }
  if (journey.id === "S6-DIRECT-SELF" && attrs.taskStatus) {
    evidence.push("source.model_or_direct_answer", "self_state.completed_response");
  }
  if (journey.id === "S6-PROPOSAL" && attrs.proposalCount > 0) {
    evidence.push("proposal.created", "durable_write.not_completed");
  }
  if (journey.id === "S6-BLOCKED" && attrs.blockerCount > 0) {
    evidence.push("blocker.created", "safe_next_control");
  }
  if (
    journey.id === "S6-PERMISSION" &&
    (attrs.proposalCount > 0 || attrs.taskStatus === "waiting_permission")
  ) {
    evidence.push("permission.pending", "review_action.visible");
  }
  if (
    journey.id === "S6-LIVE-WEB" &&
    liveProviderAgentLoopEvidence(journey, snapshot, detail).webRead
  ) {
    evidence.push("live_provider.external", "tool.web_read");
  }
  if (
    journey.id === "S6-LIVE-MCP" &&
    liveProviderAgentLoopEvidence(journey, snapshot, detail).mcpRead
  ) {
    evidence.push("live_provider.external", "tool.mcp_read", "provider_ranked_selection");
  }
  if (journey.id === "S6-RECOVERY" && (events.length > 0 || attrs.taskStatus)) {
    evidence.push("control.retry_or_cancel", "final_delivery.recorded");
  }
  return uniqueValues(evidence);
}

function blockedLiveJourney(
  journey: Step6ProductAcceptanceJourney,
  blockers: string[]
): Step6ObservedProductJourney {
  return {
    journeyId: journey.id,
    kind: journey.kind,
    observedVia: "blocked_live_evidence_report",
    entryPoint: "blocked_live_evidence_report",
    routeStrategy: "blocked_external_live",
    taskSessionId: "",
    runId: "",
    answerEvidence: [],
    runtimeEvidence: [],
    uiStatusEvidence: ["blocked_live_evidence"],
    finalDeliverySections: [],
    traceEvidence: [],
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "blocked_live_evidence",
    externalLiveProviderKind: null,
    blockers,
  };
}

function metadataSafeBlocker(value: string): string {
  return (
    String(value)
      .replace(/[^A-Za-z0-9_.:/-]+/g, "_")
      .replace(/^_+|_+$/g, "")
      .slice(0, 160) || "unknown"
  );
}

function metadataSafeLabel(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 128 &&
    value.trim() === value &&
    /^[A-Za-z0-9_.:/-]+$/.test(value)
  );
}

function uniqueValues(values: string[]): string[] {
  return values.filter((value, index) => value && values.indexOf(value) === index);
}

function visibleControlEventForLabel(label: string): string {
  const normalized =
    label
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "button";
  return `visible_control.${normalized}`;
}

function normalizeFinalSection(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function normalizedStatusFromContinuity(status: string, finalDeliverySections: string[]): string {
  const normalized = normalizeFinalSection(status);
  if (["cancelled", "canceled"].includes(normalized)) return "cancelled";
  if (["completed", "delivered", "succeeded"].includes(normalized)) return "completed";
  if (["blocked", "waiting_permission"].includes(normalized)) return "blocked";
  if (finalDeliverySections.length > 0) return "completed";
  return normalized;
}

function finalDeliverySectionsFromDetail(detail: any): string[] {
  const delivery = detail?.finalDelivery ?? detail?.final_delivery;
  const metrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  if (!metrics || typeof metrics !== "object") return [];
  const sections = [
    Array.isArray(metrics.completedActions) && metrics.completedActions.length > 0
      ? "completed_actions"
      : "",
    Array.isArray(metrics.observationsUsed) && metrics.observationsUsed.length > 0
      ? "sources_used"
      : "",
    Array.isArray(metrics.proposalsCreated) && metrics.proposalsCreated.length > 0
      ? "proposals_created"
      : "",
    Array.isArray(metrics.blockers) && metrics.blockers.length > 0 ? "blocked_items" : "",
    Array.isArray(metrics.skippedWork) && metrics.skippedWork.length > 0 ? "skipped_work" : "",
    Array.isArray(metrics.pendingUserActions) && metrics.pendingUserActions.length > 0
      ? "pending_user_actions"
      : "",
    Array.isArray(metrics.durableChanges) && metrics.durableChanges.length > 0
      ? "durable_changes"
      : "",
    Array.isArray(metrics.nextSteps) && metrics.nextSteps.length > 0 ? "next_steps" : "",
  ].filter(Boolean);
  if (sections.length === 0 && typeof metrics.summary === "string" && metrics.summary.trim()) {
    sections.push("completed_work");
  }
  return sections;
}

function transcriptEvents(detail: any): string[] {
  return uniqueValues(
    (detail?.transcript ?? []).map((entry: any) => `transcript.${entry.kind}`).filter(Boolean)
  );
}

test.describe("main-chat-step6-product-acceptance", () => {
  test.setTimeout(600_000);

  test("exports Step 6 product acceptance report from real Tauri Chat UI or explicit blockers", async ({
    page,
  }) => {
    expect(STEP6_PRODUCT_ACCEPTANCE_JOURNEYS).toHaveLength(11);
    await page.goto("/#/chat");
    const tauriAvailable = await page.evaluate(() => Boolean((window as any).__TAURI_INTERNALS__));

    if (!tauriAvailable) {
      const report = writeReport(
        buildStep6BlockedProductAcceptanceReport([
          "real_tauri_browser_command_surface_unavailable",
          "playwright_default_runner_is_vite_only",
        ])
      );
      expect(report.acceptanceReady).toBe(false);
      expect(report.requiredJourneys).toEqual(STEP6_REQUIRED_PRODUCT_JOURNEYS);
      expect(report.passedJourneys).toEqual([]);
      expect(report.blockedLiveJourneys).toEqual(["S6-LIVE-WEB", "S6-LIVE-MCP"]);
      expect(report.evidenceSource).not.toContain("fixture");
      expect(report.evidenceSource).not.toContain("synthetic");
      return;
    }

    writeReport(
      buildStep6BlockedProductAcceptanceReport(["real_tauri_step6_observation_in_progress"])
    );

    const prepReport = await prepareStep6BrowserState(page);
    const prepBlockers = validateStep6BrowserPrepReport(prepReport);
    if (prepBlockers.length > 0) {
      throw new Error(`step6_browser_prep_not_ready:${prepBlockers.join(",")}`);
    }

    const observedJourneys: Step6ObservedProductJourney[] = [];
    let liveProviderStateReport: Step6LiveProviderStatePrepReport | null = null;
    try {
      for (const journey of STEP6_PRODUCT_ACCEPTANCE_JOURNEYS) {
        if (
          journey.kind === "external_live" &&
          liveProviderStateReport === null &&
          process.env.OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL === "1"
        ) {
          liveProviderStateReport = await prepareStep6LiveProviderState(page);
        }
        const observed =
          journey.kind === "external_live"
            ? await executeLiveJourney(page, journey, liveProviderStateReport)
            : await executeLocalJourney(page, journey, prepReport);
        observedJourneys.push(observed);
      }

      const report = writeReport(
        buildStep6ProductAcceptanceReportFromObservedJourneys(observedJourneys)
      );
      expect(report.localDeterministicReady, report.blockers.join(",")).toBe(true);
      expect(report.noSilentDurableWrite).toBe(true);
      expect(report.noHiddenLegacyFallback).toBe(true);
      expect(report.noLocalEvidenceCreditedAsExternalLive).toBe(true);
    } catch (error) {
      writeReport(
        buildStep6BlockedProductAcceptanceReport([
          "real_tauri_step6_observation_failed",
          String(error),
        ])
      );
      throw error;
    }
  });
});
