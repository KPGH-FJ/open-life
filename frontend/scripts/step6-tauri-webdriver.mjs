#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(frontendRoot, "..");

const webdriverUrl = "http://127.0.0.1:4444";
const frontendDevUrl = "http://127.0.0.1:5173";
const stage1DogfoodChatHash = "#/__stage1-dogfood-chat";
const reportPath = "frontend/test-results/main-chat-step6-product-acceptance-report.json";
const schemaVersion = "step6-product-acceptance-v1";
const readinessSemantics = "step6_local_deterministic_required_external_live_opt_in_separate";
const observedSource = "tauri_command_surface_step6_browser_observed";
const blockedSource = "tauri_command_surface_unavailable";
const macosBlocker = "tauri_webdriver_macos_not_supported_by_tauri_driver";
const blockedLiveUiStatus = "blocked_live_evidence";
const unavailableBlockers = [
  "step6_product_acceptance_e2e_blocked",
  "real_tauri_browser_command_surface_unavailable",
  "tauri_webdriver_environment_not_ready",
];

const journeys = [
  journey(
    "S6-CLOCK",
    "deterministic_local",
    "chat",
    "今天星期几？现在的日期和时间是什么？",
    ["answer.clock_value"],
    ["source.runtime_fact", "runtime.clock"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-ROUTE",
    "deterministic_local",
    "chat",
    "你现在用什么模型和路线？",
    ["answer.route_summary"],
    ["source.runtime_fact", "runtime.provider_route"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-TOOLS",
    "deterministic_local",
    "chat",
    "你现在能联网、使用 MCP 或执行工具吗？只说当前可用性证据。",
    ["answer.tool_availability"],
    ["source.runtime_fact", "runtime.tool_availability"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-FILE",
    "deterministic_local",
    "chat",
    "Read file `dogfood/project_brief.md` and summarize the governed observation.",
    ["answer.file_summary"],
    ["tool.file_read", "observation.workspace_file"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-DIRECT-SELF",
    "deterministic_local",
    "chat",
    "Answer directly: what is one practical reason to keep task evidence structured?",
    ["answer.direct_complete"],
    ["source.model_or_direct_answer", "self_state.completed_response"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-PROPOSAL",
    "deterministic_local",
    "chat",
    "Remember that I prefer Step 6 evidence before durable writes.",
    ["answer.proposal_pending"],
    ["proposal.created", "durable_write.not_completed"],
    ["proposal_pending"],
    ["proposals_created", "pending_user_actions"]
  ),
  journey(
    "S6-BLOCKED",
    "deterministic_local",
    "chat",
    "Use web while network policy blocks it, then tell me the safe next action.",
    ["answer.blocked_next_action"],
    ["blocker.created", "safe_next_control"],
    ["restricted", "blocked"],
    ["blocked_items", "next_steps", "pending_user_actions"]
  ),
  journey(
    "S6-PERMISSION",
    "deterministic_local",
    "seeded_control",
    "Accept the seeded ToolPermission proposal, then resume the governed MCP read.",
    ["answer.permission_accepted"],
    [
      "permission.pending",
      "review_action.visible",
      "permission.accepted",
      "automatic_resume_replay",
      "final_delivery.recorded",
    ],
    ["completed"],
    ["completed_actions", "sources_used", "completed_work"],
    {
      prepTaskId: "S6_PERMISSION_ACCEPT",
      controlLabels: ["Accept proposal"],
    }
  ),
  journey(
    "S6-LIVE-WEB",
    "external_live",
    "chat",
    'For this live eval, call the allowed web.search candidate exactly once before answering. Return only a JSON action envelope with actions[0].name="web.search", actions[0].action_type="mcp_tool", and actions[0].arguments={}; do not answer directly.',
    ["answer.external_web_summary"],
    ["live_provider.external", "tool.web_read"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-LIVE-MCP",
    "external_live",
    "chat",
    'For candidate ranking, if the candidate ids include builtin_echo, put builtin_echo first and include every allowed candidate id exactly once. For action execution, do not explain or answer directly. Return exactly this JSON envelope and nothing else: {"final":"pending governed read","actions":[{"name":"builtin_echo","action_type":"mcp_tool","arguments":{}}],"thought_summary":"Need a governed read.","warnings":[]}. This is a governed mcp read-only utility request.',
    ["answer.external_mcp_summary"],
    ["live_provider.external", "tool.mcp_read", "provider_ranked_selection"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-RECOVERY",
    "deterministic_local",
    "seeded_control",
    "Recover or explicitly stop a blocked task using the visible task controls.",
    ["answer.recovery_or_stop"],
    ["control.retry_or_cancel", "final_delivery.recorded"],
    ["completed", "blocked", "cancelled"],
    ["blocked_items", "next_steps", "skipped_work", "completed_actions", "completed_work"],
    { prepTaskId: "D15", controlLabels: ["Cancel task from continuity detail", "Cancel task"] }
  ),
];

const requiredJourneys = journeys.map(row => row.id);
const localJourneys = journeys.filter(row => row.kind === "deterministic_local").map(row => row.id);
const externalLiveJourneys = journeys
  .filter(row => row.kind === "external_live")
  .map(row => row.id);
const requiredPrepTaskIds = uniqueValues(
  journeys.map(row => row.prepTaskId).filter(value => typeof value === "string")
);
const allowBlockedLive = process.argv.includes("--allow-blocked-live");

if (process.argv.includes("--validate-journeys-only")) {
  const blockers = validateStep6StaticJourneyMatrix();
  if (blockers.length > 0) {
    console.error(`step6_static_journey_matrix_invalid=${blockers.join(",")}`);
    process.exit(1);
  }
  console.log(`validated_step6_product_acceptance_journeys=${requiredJourneys.length}`);
  console.log(
    `validated_step6_product_acceptance_contract=${JSON.stringify(step6JourneyContract())}`
  );
  process.exit(0);
}

if (process.argv.includes("--validate-observed-rules-only")) {
  const blockers = validateStep6ObservedRuleFixtures();
  if (blockers.length > 0) {
    console.error(`step6_observed_rule_fixtures_invalid=${blockers.join(",")}`);
    process.exit(1);
  }
  console.log("validated_step6_observed_rule_fixtures=ok");
  process.exit(0);
}

main().catch(error => {
  const blocker = metadataSafeBlocker(`tauri_webdriver_runner_error:${error?.message ?? error}`);
  writeBlockedReport([blocker]);
  console.error(`Step 6 Tauri WebDriver E2E failed: ${blocker}`);
  process.exit(1);
});

async function main() {
  const preflight = buildPreflight();
  if (!preflight.ready) {
    const blockers = uniqueValues([...unavailableBlockers, ...preflight.blockers]);
    writeBlockedReport(blockers);
    console.error(
      [
        "Step 6 Tauri WebDriver E2E did not run.",
        `platform=${process.platform}`,
        `supportedPlatform=${String(preflight.supportedPlatform)}`,
        `ready=${String(preflight.ready)}`,
        `blockers=${blockers.join(",")}`,
      ].join("\n")
    );
    process.exit(1);
  }

  const result = await runStep6TauriProductAcceptance();
  if (result.ready) {
    console.error(
      [
        "Step 6 Tauri WebDriver observed all product journeys.",
        `sessionCreated=${String(result.sessionCreated)}`,
        `observedCount=${String(result.observedCount ?? 0)}`,
      ].join("\n")
    );
    process.exit(0);
  }

  if (allowBlockedLive && result.localDeterministicReady && result.blockedExternalLiveOnly) {
    console.error(
      [
        "Step 6 Tauri WebDriver observed local deterministic journeys; external live evidence is explicitly blocked.",
        `sessionCreated=${String(result.sessionCreated)}`,
        `observedCount=${String(result.observedCount ?? 0)}`,
        `blockers=${result.blockers.join(",")}`,
      ].join("\n")
    );
    process.exit(0);
  }

  if (!result.reportWritten) {
    writeBlockedReport(result.blockers, result.observedJourneys ?? []);
  }
  console.error(
    [
      "Step 6 Tauri WebDriver ran but product acceptance is not complete.",
      `sessionCreated=${String(result.sessionCreated)}`,
      `observedCount=${String(result.observedCount ?? 0)}`,
      `blockers=${result.blockers.join(",")}`,
    ].join("\n")
  );
  process.exit(1);
}

function journey(
  id,
  kind,
  executionMode,
  prompt,
  expectedAnswerEvidence,
  expectedRuntimeEvidence,
  expectedUiStatus,
  expectedFinalDeliverySections,
  options = {}
) {
  return {
    id,
    kind,
    executionMode,
    prompt,
    expectedAnswerEvidence,
    expectedRuntimeEvidence,
    expectedUiStatus,
    expectedFinalDeliverySections,
    ...options,
  };
}

function validateStep6StaticJourneyMatrix() {
  const blockers = [];
  const expectedIds = [
    "S6-CLOCK",
    "S6-ROUTE",
    "S6-TOOLS",
    "S6-FILE",
    "S6-DIRECT-SELF",
    "S6-PROPOSAL",
    "S6-BLOCKED",
    "S6-PERMISSION",
    "S6-LIVE-WEB",
    "S6-LIVE-MCP",
    "S6-RECOVERY",
  ];
  if (!arraysEqual(requiredJourneys, expectedIds)) {
    blockers.push("step6_static_journey_ids_mismatch");
  }
  if (uniqueValues(requiredJourneys).length !== requiredJourneys.length) {
    blockers.push("step6_static_journey_ids_not_unique");
  }
  if (localJourneys.length !== 9) blockers.push("step6_static_local_count_mismatch");
  if (externalLiveJourneys.length !== 2) blockers.push("step6_static_live_count_mismatch");
  if (!arraysEqual(externalLiveJourneys, ["S6-LIVE-WEB", "S6-LIVE-MCP"])) {
    blockers.push("step6_static_live_ids_mismatch");
  }
  if (!arraysEqual(requiredPrepTaskIds, ["S6_PERMISSION_ACCEPT", "D15"])) {
    blockers.push("step6_static_prep_ids_mismatch");
  }

  for (const row of journeys) {
    if (!metadataSafeLabel(row.id))
      blockers.push(`step6_static_id_unsafe:${metadataSafeBlocker(row.id)}`);
    if (!["deterministic_local", "external_live"].includes(row.kind)) {
      blockers.push(`step6_static_kind_invalid:${metadataSafeBlocker(row.id)}`);
    }
    if (!["chat", "seeded_control"].includes(row.executionMode)) {
      blockers.push(`step6_static_execution_mode_invalid:${metadataSafeBlocker(row.id)}`);
    }
    if (typeof row.prompt !== "string" || row.prompt.trim().length === 0) {
      blockers.push(`step6_static_prompt_missing:${metadataSafeBlocker(row.id)}`);
    }
    for (const [field, values] of [
      ["answer", row.expectedAnswerEvidence],
      ["runtime", row.expectedRuntimeEvidence],
      ["ui", row.expectedUiStatus],
      ["final", row.expectedFinalDeliverySections],
    ]) {
      if (!Array.isArray(values) || values.length === 0) {
        blockers.push(`step6_static_${field}_evidence_missing:${metadataSafeBlocker(row.id)}`);
      } else if (hasUnsafeLabel(values)) {
        blockers.push(`step6_static_${field}_evidence_unsafe:${metadataSafeBlocker(row.id)}`);
      } else if (uniqueValues(values).length !== values.length) {
        blockers.push(`step6_static_${field}_evidence_duplicate:${metadataSafeBlocker(row.id)}`);
      }
    }

    const seeded = row.id === "S6-PERMISSION" || row.id === "S6-RECOVERY";
    if (seeded && row.executionMode !== "seeded_control") {
      blockers.push(`step6_static_seeded_mode_mismatch:${row.id}`);
    }
    if (!seeded && row.executionMode !== "chat") {
      blockers.push(`step6_static_chat_mode_mismatch:${row.id}`);
    }
    if (row.kind === "external_live" && row.executionMode !== "chat") {
      blockers.push(`step6_static_live_mode_mismatch:${row.id}`);
    }
    if (seeded) {
      if (!metadataSafeLabel(row.prepTaskId)) {
        blockers.push(`step6_static_prep_task_missing:${row.id}`);
      }
      if (!Array.isArray(row.controlLabels) || row.controlLabels.length === 0) {
        blockers.push(`step6_static_control_labels_missing:${row.id}`);
      }
    } else if (row.prepTaskId || row.controlLabels) {
      blockers.push(`step6_static_unexpected_seed_metadata:${row.id}`);
    }
  }

  const permission = journeys.find(row => row.id === "S6-PERMISSION");
  if (
    permission?.prepTaskId !== "S6_PERMISSION_ACCEPT" ||
    !permission?.controlLabels?.includes("Accept proposal") ||
    !permission?.expectedRuntimeEvidence?.includes("permission.accepted") ||
    !permission?.expectedRuntimeEvidence?.includes("automatic_resume_replay")
  ) {
    blockers.push("step6_static_permission_contract_mismatch");
  }
  const recovery = journeys.find(row => row.id === "S6-RECOVERY");
  if (
    recovery?.prepTaskId !== "D15" ||
    !recovery?.expectedRuntimeEvidence?.includes("control.retry_or_cancel")
  ) {
    blockers.push("step6_static_recovery_contract_mismatch");
  }

  return uniqueValues(blockers);
}

function step6JourneyContract() {
  return journeys.map(row => ({
    id: row.id,
    kind: row.kind,
    executionMode: row.executionMode,
    prompt: row.prompt,
    expectedAnswerEvidence: [...row.expectedAnswerEvidence],
    expectedRuntimeEvidence: [...row.expectedRuntimeEvidence],
    expectedUiStatus: [...row.expectedUiStatus],
    expectedFinalDeliverySections: [...row.expectedFinalDeliverySections],
    prepTaskId: row.prepTaskId ?? null,
    controlLabels: row.controlLabels ? [...row.controlLabels] : [],
  }));
}

function validateStep6ObservedRuleFixtures() {
  const blockers = [];
  const validObserved = journeys.map(row =>
    row.kind === "external_live"
      ? blockedLiveJourney(row, ["explicit_live_eval_required"])
      : validObservedLocalJourneyFixture(row)
  );
  const validBlockers = validateObservedJourneysForReport(validObserved);
  if (validBlockers.length > 0) {
    blockers.push(...validBlockers.map(blocker => `valid_fixture_rejected:${blocker}`));
  }

  const localProviderMetadataObserved = validObserved.map(copyObservedJourney);
  localProviderMetadataObserved[0].externalLiveProviderKind = "external_provider";
  const localProviderMetadataBlockers = validateObservedJourneysForReport(
    localProviderMetadataObserved
  );
  if (
    !localProviderMetadataBlockers.includes(
      "tauri_webdriver_step6_local_provider_kind_invalid:S6-CLOCK"
    )
  ) {
    blockers.push("local_provider_kind_fixture_not_rejected");
  }

  return uniqueValues(blockers);
}

function validObservedLocalJourneyFixture(row) {
  return {
    journeyId: row.id,
    kind: row.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: expectedEntryPointForStep6Journey(row.id, false),
    routeStrategy:
      row.executionMode === "seeded_control" ? "task_continuity_control" : "main_chat_kernel",
    taskSessionId: `observed-task-${row.id}`,
    runId: `observed-run-${row.id}`,
    answerEvidence: [...row.expectedAnswerEvidence],
    runtimeEvidence: [...row.expectedRuntimeEvidence],
    uiStatusEvidence: [row.expectedUiStatus[0]],
    finalDeliverySections: [row.expectedFinalDeliverySections[0]],
    traceEvidence: [`trace.step6.${row.id}`],
    noInventedUnavailableEvidence: true,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "not_applicable",
    externalLiveProviderKind: null,
    blockers: [],
  };
}

function arraysEqual(left, right) {
  return (
    Array.isArray(left) &&
    Array.isArray(right) &&
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
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
  if (!tauriDebugAppBinaryAvailable()) blockers.push("tauri_debug_app_binary_missing");

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

function tauriDebugAppBinaryAvailable() {
  return fs.existsSync(tauriDebugAppBinaryPath());
}

function tauriDebugAppBinaryPath() {
  const binary = process.platform === "win32" ? "openlife-tauri.exe" : "openlife-tauri";
  return path.resolve(repoRoot, "target", "debug", binary);
}

async function runStep6TauriProductAcceptance() {
  return {
    ready: false,
    localDeterministicReady: false,
    blockedExternalLiveOnly: false,
    reportWritten: false,
    sessionCreated: false,
    observedCount: 0,
    observedJourneys: [],
    blockers: ["step6_tauri_product_acceptance_runner_retired_after_phase7_cleanup"],
  };
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
    await delay(500);
  }
  throw new Error("frontend_dev_server_ready_timeout");
}

async function frontendDevServerReady() {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 1_000);
  try {
    const response = await fetch(frontendDevUrl, { method: "GET", signal: controller.signal });
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

async function executeStep6LocalJourneyWithWebDriver(sessionId, row) {
  const previousTaskId = await readCurrentTaskIdWithWebDriver(sessionId);
  const previousNetworkPolicy = await prepareStep6NetworkPolicy(sessionId, row);
  try {
    await fillByTestId(sessionId, "chat-input", row.prompt);
    await waitForElementEnabled(sessionId, "send-button", 10_000);
    const previousUserMessageCount = await readUserMessageCountWithWebDriver(sessionId);
    await clickByTestId(sessionId, "send-button");
    await waitForChatSendStartedWithWebDriver(sessionId, previousUserMessageCount);
    await openDiagnosticsIfAvailableWithWebDriver(sessionId);
    const controlPlane = await waitForControlPlaneDelivery(sessionId, previousTaskId, row);
    return await observeStep6JourneyFromControlPlane(sessionId, row, controlPlane.taskSessionId);
  } finally {
    await restoreStep6NetworkPolicy(sessionId, previousNetworkPolicy);
  }
}

async function executeStep6LiveJourneyWithWebDriver(sessionId, row, liveProviderStateReport) {
  if (process.env.OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL !== "1") {
    return blockedLiveJourney(row, ["explicit_live_eval_required"]);
  }
  if (!step6LiveProviderStateReady(liveProviderStateReport)) {
    return blockedLiveJourney(row, step6LiveProviderStateBlockers(liveProviderStateReport));
  }
  const previousTaskId = await readCurrentTaskIdWithWebDriver(sessionId);
  await fillByTestId(sessionId, "chat-input", row.prompt);
  await waitForElementEnabled(sessionId, "send-button", 10_000);
  const previousUserMessageCount = await readUserMessageCountWithWebDriver(sessionId);
  await clickByTestId(sessionId, "send-button");
  await waitForChatSendStartedWithWebDriver(sessionId, previousUserMessageCount);
  const controlPlane = await waitForControlPlaneDelivery(sessionId, previousTaskId, row);
  return await observeStep6JourneyFromControlPlane(sessionId, row, controlPlane.taskSessionId);
}

async function prepareStep6LiveProviderEvalStateWithWebDriver(sessionId) {
  return {
    reportKind: "main_chat_step6_live_provider_eval_state_prep",
    configured: false,
    ready: false,
    debugBuild: false,
    explicitLiveEvalRequested: true,
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

function step6LiveProviderStateReady(report) {
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

function step6LiveProviderStateBlockers(report) {
  if (!report || typeof report !== "object") return ["step6_live_provider_state_missing"];
  return uniqueValues([
    "step6_live_provider_state_not_ready",
    ...(Array.isArray(report.blockers) ? report.blockers : []),
    ...(Array.isArray(report.preflightBlockers) ? report.preflightBlockers : []),
    report.configured === true ? "" : "step6_live_provider_state_not_configured",
    report.ready === true ? "" : "step6_live_provider_state_preflight_not_ready",
    report.providerEndpointKind === "external_provider"
      ? ""
      : "external_provider_endpoint_required",
    report.appConfigPersisted === false ? "" : "step6_live_provider_state_persisted_config",
    report.directWritesExecuted === false ? "" : "step6_live_provider_state_direct_write",
  ]);
}

async function executeStep6SeededControlJourneyWithWebDriver(sessionId, row, prepReport) {
  if (row.id === "S6-PERMISSION") {
    return await executeStep6PermissionAcceptanceJourneyWithWebDriver(sessionId, row, prepReport);
  }
  const taskSessionId = prepReport?.taskSessionIds?.[row.prepTaskId] ?? "";
  if (!taskSessionId) {
    throw new Error(`step6_seeded_task_missing:${row.id}:${row.prepTaskId}`);
  }
  const visibleControlEvents = [];
  visibleControlEvents.push(await openTaskContinuityDetailWithWebDriver(sessionId, taskSessionId));
  visibleControlEvents.push(
    await clickFirstVisibleControlWithWebDriver(sessionId, row.controlLabels)
  );
  const evidence = await readTaskContinuityEvidenceWithWebDriver(sessionId, row);
  return observedJourneyFromTaskContinuity(row, evidence, visibleControlEvents);
}

async function executeStep6PermissionAcceptanceJourneyWithWebDriver(sessionId, row, prepReport) {
  const taskSessionId = prepReport?.taskSessionIds?.[row.prepTaskId] ?? "";
  if (!taskSessionId) {
    throw new Error(`step6_seeded_task_missing:${row.id}:${row.prepTaskId}`);
  }
  const visibleControlEvents = [];
  visibleControlEvents.push(await openTaskContinuityDetailWithWebDriver(sessionId, taskSessionId));
  const beforeDetail = await readTaskDetailWithWebDriver(sessionId, taskSessionId);
  const pendingProposal = pendingToolPermissionProposal(beforeDetail);
  if (!pendingProposal) {
    throw new Error(`step6_permission_pending_proposal_missing:${taskSessionId}`);
  }
  visibleControlEvents.push("visible_control.review_action_visible");
  visibleControlEvents.push(
    await clickFirstVisibleControlWithWebDriver(sessionId, row.controlLabels)
  );
  const afterDetail = await waitForTaskDetailWithWebDriver(
    sessionId,
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
  const evidence = await readTaskContinuityEvidenceWithWebDriver(sessionId, row);
  const finalDeliverySections = uniqueValues([
    ...evidence.finalDeliverySectionTitles,
    ...finalDeliverySectionsFromDetail(afterDetail),
  ]);
  const status = normalizedStatusFromContinuity(
    taskStatusFromDetail(afterDetail),
    finalDeliverySections
  );
  const accepted = toolPermissionProposalStatus(afterDetail, pendingProposal.id) === "accepted";
  const replayed = taskReplayCompleted(afterDetail);
  const finalDeliveryMatched = finalDeliveryMatchesJourney(row, finalDeliverySections);
  return {
    journeyId: row.id,
    kind: row.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "task_continuity_control",
    routeStrategy: evidence.routeStrategy || "task_continuity_control",
    taskSessionId: evidence.taskSessionId || taskSessionId,
    runId: evidence.runId,
    answerEvidence:
      accepted && replayed && finalDeliveryMatched ? [...row.expectedAnswerEvidence] : [],
    runtimeEvidence: uniqueValues([
      pendingProposal ? "permission.pending" : "",
      pendingProposal ? "review_action.visible" : "",
      accepted ? "permission.accepted" : "",
      replayed ? "automatic_resume_replay" : "",
      finalDeliveryMatched ? "final_delivery.recorded" : "",
    ]),
    uiStatusEvidence: status ? [status] : [],
    finalDeliverySections,
    traceEvidence: uniqueValues([...evidence.events, ...visibleControlEvents]),
    noInventedUnavailableEvidence: true,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "not_applicable",
    externalLiveProviderKind: null,
    blockers: [],
  };
}

async function prepareStep6NetworkPolicy(sessionId, row) {
  if (row.id !== "S6-BLOCKED") return null;
  return {
    retired: true,
    blocker: "step6_network_policy_dev_command_retired_after_phase7_cleanup",
  };
}

async function restoreStep6NetworkPolicy(sessionId, previousNetworkPolicy) {
  if (previousNetworkPolicy === null || previousNetworkPolicy === undefined) return;
  if (previousNetworkPolicy.retired) return;
}

async function navigateToChat(sessionId) {
  await webdriverRequest(`/session/${encodeURIComponent(sessionId)}/execute/sync`, {
    method: "POST",
    body: {
      script: "window.location.hash = arguments[0]; return window.location.hash;",
      args: [stage1DogfoodChatHash],
    },
  });
  await waitForElementPresent(sessionId, "chat-input", 30_000);
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

async function readUserMessageCountWithWebDriver(sessionId) {
  return await executeScript(
    sessionId,
    `
      return document.querySelectorAll('[data-testid="user-message"]').length;
    `
  );
}

async function fillByTestId(sessionId, testId, value) {
  await waitForElementPresent(sessionId, testId, 30_000);
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
      try {
        element.dispatchEvent(new InputEvent('input', {
          bubbles: true,
          data: arguments[1],
          inputType: arguments[1] ? 'insertText' : 'deleteContentBackward',
        }));
      } catch {
        element.dispatchEvent(new Event('input', { bubbles: true }));
      }
      element.dispatchEvent(new Event('change', { bubbles: true }));
      return element.value === arguments[1];
    `,
    [testId, value]
  );
  if (!filled) throw new Error(`webdriver_element_missing:${testId}`);
  await waitForScript(
    sessionId,
    `
      const element = document.querySelector(\`[data-testid="\${arguments[0]}"]\`);
      const sendButton = document.querySelector('[data-testid="send-button"]');
      const expected = String(arguments[1] ?? '');
      const shouldEnableSend = arguments[0] === 'chat-input' && expected.trim().length > 0;
      return Boolean(
        element &&
        element.value === expected &&
        (!shouldEnableSend || (sendButton && !sendButton.disabled))
      );
    `,
    [testId, value],
    10_000,
    `webdriver_input_state_not_committed:${testId}`
  );
}

async function waitForElementPresent(sessionId, testId, timeoutMs) {
  await waitForScript(
    sessionId,
    `
      return Boolean(document.querySelector(\`[data-testid="\${arguments[0]}"]\`));
    `,
    [testId],
    timeoutMs,
    `webdriver_element_missing:${testId}`
  );
}

async function clickByTestId(sessionId, testId) {
  const elementId = await findElementIdByTestId(sessionId, testId);
  if (elementId) {
    try {
      await webdriverRequest(
        `/session/${encodeURIComponent(sessionId)}/element/${encodeURIComponent(elementId)}/click`,
        {
          method: "POST",
          body: {},
        }
      );
      return;
    } catch (error) {
      console.error(
        `[webdriver_native_click:error] ${metadataSafeBlocker(testId)}:${metadataSafeBlocker(
          error?.message ?? error
        )}`
      );
    }
  }
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

async function findElementIdByTestId(sessionId, testId) {
  const response = await webdriverRequest(`/session/${encodeURIComponent(sessionId)}/element`, {
    method: "POST",
    body: {
      using: "css selector",
      value: `[data-testid="${testId}"]`,
    },
  }).catch(() => null);
  const element = response?.value;
  if (!element || typeof element !== "object") return "";
  return element["element-6066-11e4-a52e-4f735466cecf"] ?? element.ELEMENT ?? "";
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

async function waitForChatSendStartedWithWebDriver(sessionId, previousUserMessageCount) {
  await waitForScript(
    sessionId,
    `
      const input = document.querySelector('[data-testid="chat-input"]');
      const sendButton = document.querySelector('[data-testid="send-button"]');
      const userMessages = [...document.querySelectorAll('[data-testid="user-message"]')];
      const previousUserMessageCount = Number(arguments[0] ?? 0);
      return Boolean(
        (input && input.value.trim().length === 0) ||
        (sendButton && sendButton.disabled) ||
        userMessages.length > previousUserMessageCount
      );
    `,
    [previousUserMessageCount],
    10_000,
    "webdriver_chat_send_not_started"
  );
}

async function openDiagnosticsIfAvailableWithWebDriver(sessionId) {
  const opened = await waitForScript(
    sessionId,
    `
      const existing = document.querySelector('[data-testid="agent-control-plane"]');
      if (existing) return true;
      const buttons = [...document.querySelectorAll('button')];
      const button = buttons.find(item =>
        item.getAttribute('aria-label') === 'Show Main Chat diagnostics' && !item.disabled
      );
      if (!button) return false;
      button.click();
      return true;
    `,
    [],
    30_000,
    "webdriver_diagnostics_toggle_missing"
  ).catch(() => false);
  if (!opened) console.error("[step6_diagnostics:unavailable]");
  return Boolean(opened);
}

async function waitForControlPlaneDelivery(sessionId, previousTaskId, row) {
  try {
    return await waitForScript(
      sessionId,
      `
        const openDiagnosticsIfPossible = () => {
          const button = [...document.querySelectorAll('button')].find(item =>
            item.getAttribute('aria-label') === 'Show Main Chat diagnostics' && !item.disabled
          );
          if (!button) return false;
          button.click();
          return true;
        };
        const controls = [...document.querySelectorAll('[data-testid="agent-control-plane"]')];
        const control = controls.at(-1);
        if (!control) {
          openDiagnosticsIfPossible();
          return null;
        }
        const taskSessionId = control.getAttribute('data-task-session-id') ?? '';
        const finalDelivery = control.getAttribute('data-final-delivery') === 'true';
        const taskStatus = control.getAttribute('data-task-status') ?? '';
        const blockerCount = Number(control.getAttribute('data-blocker-count') ?? '0');
        const proposalCount = Number(control.getAttribute('data-proposal-count') ?? '0');
        const readyWithoutFinalDelivery =
          arguments[1].includes('blocked') ||
          arguments[1].includes('restricted') ||
          arguments[1].includes('permission_pending') ||
          arguments[1].includes('waiting_for_user');
        if (
          !taskSessionId ||
          taskSessionId === arguments[0] ||
          (!finalDelivery &&
            !(readyWithoutFinalDelivery &&
              (blockerCount > 0 || proposalCount > 0 || /blocked|waiting_permission/.test(taskStatus))))
        ) {
          return null;
        }
        return {
          taskSessionId,
          runId: control.getAttribute('data-run-id') ?? '',
          routeStrategy: control.getAttribute('data-route-strategy') ?? '',
          taskStatus,
          actionCount: Number(control.getAttribute('data-action-count') ?? '0'),
          observationCount: Number(control.getAttribute('data-observation-count') ?? '0'),
          blockerCount,
          proposalCount,
          finalDeliverySectionTitles: (
            control.getAttribute('data-final-delivery-section-titles') ?? ''
          ).split('|').filter(Boolean),
          text: control.textContent ?? '',
        };
      `,
      [previousTaskId, row.expectedUiStatus],
      120_000,
      `webdriver_control_plane_delivery_timeout:${row.id}`
    );
  } catch (error) {
    const snapshot = await readControlPlaneTimeoutSnapshotWithWebDriver(
      sessionId,
      previousTaskId
    ).catch(
      snapshotError =>
        `snapshot_error=${metadataSafeBlocker(snapshotError?.message ?? snapshotError)}`
    );
    console.error(`[step6_control_plane_timeout_snapshot] ${snapshot}`);
    throw new Error(`${error?.message ?? error}:${snapshot}`);
  }
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
      const evidence = document.querySelector('[data-testid="main-chat-execution-evidence"]');
      const status = document.querySelector('[data-testid="main-chat-agent-status"]');
      const sendButton = document.querySelector('[data-testid="send-button"]');
      const chatInput = document.querySelector('[data-testid="chat-input"]');
      const assistantMessages = [...document.querySelectorAll('[data-testid="assistant-message"]')];
      const userMessages = [...document.querySelectorAll('[data-testid="user-message"]')];
      const lastTaskSessionId = control?.getAttribute('data-task-session-id') ?? '';
      return {
        controlCount: controls.length,
        evidenceVisible: Boolean(evidence),
        productStatus: safe(status?.getAttribute('data-agent-product-status') ?? ''),
        lastTaskChanged: Boolean(lastTaskSessionId && lastTaskSessionId !== arguments[0]),
        lastTaskStatus: safe(control?.getAttribute('data-task-status') ?? ''),
        lastFinalDelivery: control?.getAttribute('data-final-delivery') === 'true',
        lastRouteStrategy: safe(control?.getAttribute('data-route-strategy') ?? ''),
        actionCount: Number(control?.getAttribute('data-action-count') ?? '0'),
        observationCount: Number(control?.getAttribute('data-observation-count') ?? '0'),
        blockerCount: Number(control?.getAttribute('data-blocker-count') ?? '0'),
        proposalCount: Number(control?.getAttribute('data-proposal-count') ?? '0'),
        sendDisabled: Boolean(sendButton?.disabled),
        sendAria: safe(sendButton?.getAttribute('aria-label') ?? ''),
        chatInputLength: Number(chatInput?.value?.length ?? '0'),
        chatInputEmpty: Boolean(!chatInput?.value?.trim()),
        assistantMessageCount: assistantMessages.length,
        userMessageCount: userMessages.length,
      };
    `,
    [previousTaskId]
  );
  if (!snapshot || typeof snapshot !== "object") return "snapshot=missing";
  return Object.entries(snapshot)
    .map(([key, value]) => `${metadataSafeBlocker(key)}=${metadataSafeBlocker(value)}`)
    .join(",");
}

async function observeStep6JourneyFromControlPlane(sessionId, row, taskSessionId) {
  const attrs = await readControlPlaneAttrsWithWebDriver(sessionId, taskSessionId);
  const snapshot = await tauriInvoke(sessionId, "get_main_chat_agent_state_snapshot", {
    taskSessionId: attrs.taskSessionId,
    task_session_id: attrs.taskSessionId,
  }).catch(() => null);
  const detail = await readTaskDetailWithWebDriver(sessionId, attrs.taskSessionId).catch(
    () => null
  );
  const events = uniqueValues([
    ...(await taskEventsWithWebDriver(sessionId, attrs.taskSessionId).catch(() => [])),
    ...transcriptEvents(detail),
  ]);
  const uiStatus = await readProductStatusWithWebDriver(sessionId);
  const finalDeliverySections = uniqueValues([
    ...attrs.finalDeliverySectionTitles.map(normalizeFinalSection),
    ...finalDeliverySectionsFromDetail(snapshot),
    ...finalDeliverySectionsFromDetail(detail),
  ]);
  const finalDeliveryMatched = finalDeliveryMatchesJourney(row, finalDeliverySections);
  const answerEvidence = finalDeliveryMatched
    ? answerEvidenceForJourney(row, attrs, snapshot, detail, finalDeliverySections)
    : [];
  const runtimeEvidence = finalDeliveryMatched
    ? runtimeEvidenceForJourney(row, attrs, snapshot, detail, events, finalDeliverySections)
    : [];
  const liveJourneyCredited =
    row.kind === "external_live" &&
    externalLiveJourneyCredited(row, snapshot, detail, runtimeEvidence, finalDeliveryMatched);
  const traceEvidence = uniqueValues([
    ...events,
    ...liveProviderTraceEvidence(detail),
    ...snapshotEvents(snapshot),
    attrs.routeStrategy ? `route.${attrs.routeStrategy}` : "",
    snapshot?.provider?.routeType ? `provider_route.${snapshot.provider.routeType}` : "",
  ]);
  const externalProviderKind = externalProviderKindForSnapshot(row, snapshot);

  return {
    journeyId: row.id,
    kind: row.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "ordinary_main_chat_input",
    routeStrategy: attrs.routeStrategy,
    taskSessionId: attrs.taskSessionId,
    runId: attrs.runId,
    answerEvidence,
    runtimeEvidence,
    uiStatusEvidence: uiStatusEvidenceForJourney(row, attrs, detail, uiStatus),
    finalDeliverySections,
    traceEvidence,
    noInventedUnavailableEvidence: true,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: attrs.text.includes("Fallback notice"),
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive:
      row.kind === "external_live" &&
      liveJourneyCredited &&
      externalProviderKind !== "external_provider",
    externalLiveStatus:
      row.kind === "external_live" && liveJourneyCredited
        ? "credited_external_live"
        : row.kind === "external_live"
          ? "incomplete_external_live"
          : "not_applicable",
    externalLiveProviderKind: row.kind === "external_live" ? externalProviderKind : null,
    blockers:
      row.kind === "external_live" && !liveJourneyCredited
        ? ["external_live_provider_evidence_missing"]
        : [],
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

async function readProductStatusWithWebDriver(sessionId) {
  return await executeScript(
    sessionId,
    `
      const statuses = [...document.querySelectorAll('[data-testid="main-chat-agent-status"]')];
      return statuses.at(-1)?.getAttribute('data-agent-product-status') ?? '';
    `
  );
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

async function clickFirstVisibleControlWithWebDriver(sessionId, labels) {
  const label = await waitForScript(
    sessionId,
    `
      const labels = arguments[0].map(label => String(label).toLowerCase());
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
  await delay(500);
  return visibleControlEventForLabel(label);
}

async function readTaskContinuityEvidenceWithWebDriver(sessionId, row) {
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
  if (!domEvidence) throw new Error(`webdriver_task_continuity_evidence_missing:${row.id}`);
  const detail = await tauriInvoke(sessionId, "get_main_chat_agent_task_detail", {
    taskSessionId: domEvidence.taskSessionId,
    task_session_id: domEvidence.taskSessionId,
  });
  const events = await taskEventsWithWebDriver(sessionId, domEvidence.taskSessionId);
  return {
    ...domEvidence,
    detail,
    events: uniqueValues([...events, ...transcriptEvents(detail)]),
    finalDeliverySectionTitles: uniqueValues([
      ...domEvidence.finalDeliverySectionTitles.map(normalizeFinalSection),
      ...finalDeliverySectionsFromDetail(detail),
    ]),
  };
}

async function readTaskDetailWithWebDriver(sessionId, taskSessionId) {
  return await tauriInvoke(sessionId, "get_main_chat_agent_task_detail", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

async function waitForTaskDetailWithWebDriver(
  sessionId,
  taskSessionId,
  predicate,
  timeoutMs,
  errorMessage
) {
  const start = Date.now();
  let lastDetail = null;
  while (Date.now() - start <= timeoutMs) {
    lastDetail = await readTaskDetailWithWebDriver(sessionId, taskSessionId);
    if (predicate(lastDetail)) return lastDetail;
    await delay(250);
  }
  throw new Error(
    `${errorMessage}:${metadataSafeBlocker(JSON.stringify(taskDetailSummary(lastDetail)))}`
  );
}

function taskDetailSummary(detail) {
  return {
    status: taskStatusFromDetail(detail),
    proposals: (detail?.proposals ?? []).map(proposal => ({
      id: proposal?.id ?? "",
      type: proposal?.proposalType ?? proposal?.proposal_type ?? "",
      status: proposal?.status ?? "",
    })),
    actions: (detail?.actions ?? []).map(action => ({
      id: action?.id ?? "",
      status: action?.status ?? "",
    })),
  };
}

function taskStatusFromDetail(detail) {
  return detail?.taskSession?.status ?? detail?.task_session?.status ?? "";
}

function pendingToolPermissionProposal(detail) {
  return (detail?.proposals ?? []).find(proposal => {
    const proposalType = proposal?.proposalType ?? proposal?.proposal_type ?? "";
    return proposalType === "tool_permission" && proposal?.status === "pending";
  });
}

function toolPermissionProposalStatus(detail, proposalId) {
  const proposal = (detail?.proposals ?? []).find(item => item?.id === proposalId);
  return proposal?.status ?? "";
}

function taskReplayCompleted(detail) {
  const actions = detail?.actions ?? [];
  const transcript = detail?.transcript ?? [];
  return (
    actions.some(action => action?.status === "completed") ||
    transcript.some(entry => {
      const metadata = entry?.metadata ?? {};
      return (
        metadata.automaticResumeReplayCompleted === true ||
        metadata.automaticReplayCompleted === true
      );
    })
  );
}

function observedJourneyFromTaskContinuity(row, evidence, visibleControlEvents) {
  const finalDeliverySections = uniqueValues(evidence.finalDeliverySectionTitles);
  const status = normalizedStatusFromContinuity(evidence.status, finalDeliverySections);
  const finalDeliveryMatched = finalDeliveryMatchesJourney(row, finalDeliverySections);
  return {
    journeyId: row.id,
    kind: row.kind,
    observedVia: "real_tauri_chat_or_control_path",
    entryPoint: "task_continuity_control",
    routeStrategy: evidence.routeStrategy || "task_continuity_control",
    taskSessionId: evidence.taskSessionId,
    runId: evidence.runId,
    answerEvidence: finalDeliveryMatched ? [...row.expectedAnswerEvidence] : [],
    runtimeEvidence:
      visibleControlEvents.length > 0 && finalDeliveryMatched
        ? [...row.expectedRuntimeEvidence]
        : [],
    uiStatusEvidence: status ? [status] : [],
    finalDeliverySections,
    traceEvidence: uniqueValues([...evidence.events, ...visibleControlEvents]),
    noInventedUnavailableEvidence: true,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "not_applicable",
    externalLiveProviderKind: null,
    blockers: [],
  };
}

function answerEvidenceForJourney(row, attrs, snapshot, detail, finalDeliverySections) {
  return structuredTaskReady(row, attrs, snapshot, detail, finalDeliverySections)
    ? [...row.expectedAnswerEvidence]
    : [];
}

function runtimeEvidenceForJourney(row, attrs, snapshot, detail, events, finalDeliverySections) {
  const evidence = [];
  const routeStrategy = `${attrs.routeStrategy} ${snapshot?.route?.strategy ?? ""}`.toLowerCase();
  const providerRoute = `${snapshot?.provider?.routeType ?? ""}`.toLowerCase();
  const providerKind = externalProviderKindForSnapshot(row, snapshot);
  const taskReady = structuredTaskReady(row, attrs, snapshot, detail, finalDeliverySections);
  if (row.id === "S6-CLOCK" && taskReady) {
    evidence.push("source.runtime_fact", "runtime.clock");
  }
  if (row.id === "S6-ROUTE" && taskReady) {
    evidence.push("source.runtime_fact", "runtime.provider_route");
  }
  if (row.id === "S6-TOOLS" && taskReady) {
    evidence.push("source.runtime_fact", "runtime.tool_availability");
  }
  if (
    row.id === "S6-FILE" &&
    (attrs.observationCount > 0 || events.includes("observation.created"))
  ) {
    evidence.push("tool.file_read", "observation.workspace_file");
  }
  if (row.id === "S6-DIRECT-SELF" && taskReady) {
    evidence.push("source.model_or_direct_answer", "self_state.completed_response");
  }
  if (row.id === "S6-PROPOSAL" && structuredProposalPending(attrs, detail)) {
    evidence.push("proposal.created", "durable_write.not_completed");
  }
  if (row.id === "S6-BLOCKED" && structuredBlockerPending(attrs, detail)) {
    evidence.push("blocker.created", "safe_next_control");
  }
  if (
    row.id === "S6-PERMISSION" &&
    (attrs.proposalCount > 0 || attrs.taskStatus === "waiting_permission")
  ) {
    evidence.push("permission.pending", "review_action.visible");
  }
  if (row.id === "S6-LIVE-WEB" && liveProviderAgentLoopEvidence(row, snapshot, detail).webRead) {
    evidence.push("live_provider.external", "tool.web_read");
  }
  if (row.id === "S6-LIVE-MCP" && liveProviderAgentLoopEvidence(row, snapshot, detail).mcpRead) {
    evidence.push("live_provider.external", "tool.mcp_read", "provider_ranked_selection");
  }
  return uniqueValues(evidence);
}

function uiStatusEvidenceForJourney(row, attrs, detail, uiStatus) {
  if (row.id === "S6-PROPOSAL" && structuredProposalPending(attrs, detail)) {
    return ["proposal_pending"];
  }
  if (row.id === "S6-BLOCKED" && structuredBlockerPending(attrs, detail)) return ["restricted"];
  return uiStatus ? [uiStatus] : [];
}

function externalLiveJourneyCredited(row, snapshot, detail, runtimeEvidence, finalDeliveryMatched) {
  return (
    row.kind === "external_live" &&
    finalDeliveryMatched &&
    externalProviderKindForSnapshot(row, snapshot) === "external_provider" &&
    row.expectedRuntimeEvidence.every(evidence => runtimeEvidence.includes(evidence)) &&
    liveProviderAgentLoopEvidence(row, snapshot, detail).externalProviderAgentLoopSucceeded
  );
}

function liveProviderAgentLoopEvidence(row, snapshot, detail) {
  const metadata = agentLoopMetadataFromDetail(detail);
  const providerRoute = `${snapshot?.provider?.routeType ?? ""}`.toLowerCase();
  const providerKind = externalProviderKindForSnapshot(row, snapshot);
  const endpointKind = metadataString(metadata, "providerEndpointKind");
  const liveProviderInvoked = metadata?.liveProviderInvoked === true;
  const baseSucceeded =
    row.kind === "external_live" &&
    providerRoute === "cloud" &&
    providerKind === "external_provider" &&
    endpointKind === "external_provider" &&
    liveProviderInvoked &&
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
  const targetMatches = (...targets) =>
    targets.some(target => selectedTarget === target || selectedId === target);
  const selectedRankMatches =
    selectedRank > 0 &&
    candidateIds[selectedRank - 1] &&
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

function agentLoopMetadataFromDetail(detail) {
  let attemptedMetadata = null;
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

function liveProviderTraceEvidence(detail) {
  const metadata = agentLoopMetadataFromDetail(detail);
  if (!metadata) return [];
  const evidence = [];
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

function metadataString(metadata, key) {
  const value = metadata?.[key];
  return typeof value === "string" ? value : "";
}

function metadataNumber(metadata, key) {
  const value = metadata?.[key];
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

function metadataStringArray(metadata, key) {
  const value = metadata?.[key];
  return Array.isArray(value) ? value.filter(item => typeof item === "string") : [];
}

function structuredTaskReady(row, attrs, snapshot, detail, finalDeliverySections) {
  if (!row) return false;
  if (finalDeliverySections.length > 0) return true;
  if (structuredFinalDeliveryPresent(snapshot) || structuredFinalDeliveryPresent(detail))
    return true;
  if (attrs.taskStatus === "waiting_permission") return true;
  if (structuredProposalPending(attrs, detail)) return true;
  if (structuredBlockerPending(attrs, detail)) return true;
  return false;
}

function structuredFinalDeliveryPresent(value) {
  const delivery = value?.finalDelivery ?? value?.final_delivery;
  return Boolean(delivery && typeof delivery === "object");
}

function structuredProposalPending(attrs, detail) {
  return attrs.proposalCount > 0 || pendingProposalCount(detail) > 0;
}

function pendingProposalCount(detail) {
  return (detail?.proposals ?? []).filter(proposal => {
    const status = normalizeFinalSection(proposal?.status);
    return status === "pending" || status === "proposed" || status === "";
  }).length;
}

function structuredBlockerPending(attrs, detail) {
  const status = normalizeFinalSection(attrs.taskStatus || taskStatusFromDetail(detail));
  const allowedControls = detail?.allowedControls ?? detail?.allowed_controls;
  const nextControl = detail?.nextRecommendedControl ?? detail?.next_recommended_control;
  return (
    attrs.blockerCount > 0 ||
    detailBlockerCount(detail) > 0 ||
    status === "blocked" ||
    status === "restricted" ||
    Boolean(nextControl) ||
    (Array.isArray(allowedControls) && allowedControls.length > 0)
  );
}

function detailBlockerCount(detail) {
  return Array.isArray(detail?.blockers) ? detail.blockers.length : 0;
}

function externalProviderKindForSnapshot(row, snapshot) {
  if (row.kind !== "external_live") return null;
  const routeType = String(snapshot?.provider?.routeType ?? "").toLowerCase();
  const provider = String(snapshot?.provider?.provider ?? "");
  if (routeType !== "cloud") return routeType || null;
  return isExternalProviderLabel(provider) ? "external_provider" : provider || null;
}

function isExternalProviderLabel(provider) {
  const raw = String(provider ?? "");
  if (!metadataSafeLabel(raw)) return false;
  const lower = raw.toLowerCase();
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
  return raw.length > 0 && !localAliases.some(alias => lower.includes(alias));
}

function blockedLiveJourney(row, blockers) {
  const normalizedBlockers = normalizeBlockers(blockers);
  return {
    journeyId: row.id,
    kind: row.kind,
    observedVia: "blocked_live_evidence_report",
    entryPoint: "blocked_live_evidence_report",
    routeStrategy: "blocked_external_live",
    taskSessionId: "",
    runId: "",
    answerEvidence: [],
    runtimeEvidence: [],
    uiStatusEvidence: [blockedLiveUiStatus],
    finalDeliverySections: [],
    traceEvidence: [],
    noInventedUnavailableEvidence: true,
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "blocked_live_evidence",
    externalLiveProviderKind: null,
    blockers: normalizedBlockers,
  };
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

async function auditFinalStep6GateWithBrowserEvidence(sessionId) {
  const report = {
    browserE2eEnvironmentReady: false,
    localDeterministicReady: false,
    passedJourneyCount: 0,
    noSilentDurableWrite: true,
    noHiddenLegacyFallback: true,
    noLocalEvidenceCreditedAsExternalLive: true,
    noInventedUnavailableEvidence: true,
    uiStatusFromStructuredEvidence: false,
    overallReady: false,
    blockers: ["step6_product_acceptance_gate_command_retired_after_phase7_cleanup"],
  };
  const blockers = [];
  if (report?.browserE2eEnvironmentReady !== true) {
    blockers.push("tauri_webdriver_step6_gate_browser_environment_not_ready");
  }
  if (report?.localDeterministicReady !== true) {
    blockers.push("tauri_webdriver_step6_gate_local_not_ready");
  }
  if (report?.passedJourneyCount < localJourneys.length) {
    blockers.push("tauri_webdriver_step6_gate_local_passed_count_low");
  }
  if (report?.noSilentDurableWrite !== true) {
    blockers.push("tauri_webdriver_step6_gate_silent_write_detected");
  }
  if (report?.noHiddenLegacyFallback !== true) {
    blockers.push("tauri_webdriver_step6_gate_legacy_fallback_detected");
  }
  if (report?.noLocalEvidenceCreditedAsExternalLive !== true) {
    blockers.push("tauri_webdriver_step6_gate_fake_live_credit_detected");
  }
  if (report?.noInventedUnavailableEvidence !== true) {
    blockers.push("tauri_webdriver_step6_gate_invented_unavailable_evidence");
  }
  if (report?.uiStatusFromStructuredEvidence !== true) {
    blockers.push("tauri_webdriver_step6_gate_ui_status_unstructured");
  }
  if (report?.overallReady !== true) {
    blockers.push(
      ...(report?.blockers ?? []).map(blocker => `tauri_webdriver_step6_gate_blocker:${blocker}`)
    );
  }
  return {
    ready: report?.overallReady === true && blockers.length === 0,
    report,
    blockedExternalLiveOnly: finalGateBlockedExternalLiveOnly(report),
    blockers:
      blockers.length === 0
        ? []
        : ["tauri_webdriver_step6_final_gate_rejected", ...uniqueValues(blockers)],
  };
}

function finalGateBlockedExternalLiveOnly(report) {
  if (!report || typeof report !== "object") return false;
  const localSafetyReady =
    report.browserE2eEnvironmentReady === true &&
    report.localDeterministicReady === true &&
    report.noSilentDurableWrite === true &&
    report.noHiddenLegacyFallback === true &&
    report.noLocalEvidenceCreditedAsExternalLive === true &&
    report.noInventedUnavailableEvidence === true &&
    report.uiStatusFromStructuredEvidence === true;
  if (!localSafetyReady || report.externalLiveReady === true || report.overallReady === true) {
    return false;
  }
  const blockedLiveJourneys = new Set(
    (report.journeys ?? [])
      .filter(row => row?.blockedLiveEvidenceReport === true)
      .map(row => row?.journeyId)
  );
  const liveRowsBlocked = externalLiveJourneys.every(id => blockedLiveJourneys.has(id));
  const localRowsCredited = localJourneys.every(id =>
    (report.journeys ?? []).some(row => row?.journeyId === id && row?.credited === true)
  );
  const allowedBlockers = new Set([
    "step6_external_live_evidence_blocked_or_incomplete",
    "step6_external_live_journeys_not_all_passed",
    "step6_final_gate_live_provider_incomplete",
    "step6_final_acceptance_not_ready",
    "runtime_eval_final_completion_not_ready",
    "command_surface_final_completion_not_ready",
    "provider_live_proposal_permission_not_executed",
    "provider_backed_web_mcp_agent_loop_not_executed",
  ]);
  const liveOnlyBlockers = (report.blockers ?? []).every(blocker => {
    const text = String(blocker ?? "");
    return step6LiveOnlyFinalGateBlocker(text, allowedBlockers);
  });
  return liveRowsBlocked && localRowsCredited && liveOnlyBlockers;
}

function finalGateAuditBlockedExternalLiveOnly(finalGateAudit) {
  if (finalGateAudit?.blockedExternalLiveOnly === true) return true;
  const report = finalGateAudit?.report;
  if (!report || typeof report !== "object") return false;
  if (
    report.localDeterministicReady !== true ||
    report.externalLiveReady === true ||
    report.overallReady === true ||
    (Array.isArray(report.failedJourneys) && report.failedJourneys.length > 0)
  ) {
    return false;
  }
  return (finalGateAudit?.blockers ?? []).every(blocker => {
    const text = String(blocker ?? "");
    if (text === "tauri_webdriver_step6_final_gate_rejected") return true;
    const unwrapped = text.startsWith("tauri_webdriver_step6_gate_blocker:")
      ? text.slice("tauri_webdriver_step6_gate_blocker:".length)
      : text;
    return step6LiveOnlyFinalGateBlocker(unwrapped);
  });
}

function step6LiveOnlyFinalGateBlocker(blocker, allowedBlockers = defaultStep6LiveOnlyBlockers()) {
  const text = String(blocker ?? "");
  return (
    allowedBlockers.has(text) ||
    text.startsWith("S6-LIVE-WEB:") ||
    text.startsWith("S6-LIVE-MCP:") ||
    text.startsWith("step6_final_gate_live_credit_missing:S6-LIVE-WEB") ||
    text.startsWith("step6_final_gate_live_credit_missing:S6-LIVE-MCP") ||
    text.includes("provider_backed_web_mcp_agent_loop_not_executed") ||
    text.includes("provider_backed_web_agent_loop_not_executed") ||
    text.includes("provider_backed_mcp_agent_loop_not_executed") ||
    text.includes("live_provider") ||
    text.includes("external_live")
  );
}

function defaultStep6LiveOnlyBlockers() {
  return new Set([
    "step6_external_live_evidence_blocked_or_incomplete",
    "step6_external_live_journeys_not_all_passed",
    "step6_final_gate_live_provider_incomplete",
    "step6_final_acceptance_not_ready",
    "runtime_eval_final_completion_not_ready",
    "command_surface_final_completion_not_ready",
    "provider_live_proposal_permission_not_executed",
    "provider_backed_web_mcp_agent_loop_not_executed",
  ]);
}

function finalGateBlockerFromError(error) {
  return metadataSafeBlocker(`tauri_webdriver_step6_final_gate_error:${error?.message ?? error}`);
}

function mergeFinalGateAuditIntoBrowserReport(report, finalGateAudit) {
  const gateReport = finalGateAudit?.report;
  const gateJourneyById = new Map(
    (gateReport?.journeys ?? [])
      .filter(row => typeof row?.journeyId === "string")
      .map(row => [row.journeyId, row])
  );
  const gateBlockers = normalizeBlockers(finalGateAudit?.blockers ?? []);
  const observedJourneys = report.observedJourneys.map(row => {
    const next = copyObservedJourney(row);
    const gateJourney = gateJourneyById.get(row.journeyId);
    const rowBlockers = normalizeBlockers([
      ...(next.blockers ?? []),
      ...(gateJourney?.blockers ?? []),
    ]);
    next.blockers = rowBlockers;
    if (
      next.kind === "external_live" &&
      next.externalLiveStatus === "credited_external_live" &&
      gateJourney &&
      gateJourney.credited !== true
    ) {
      next.externalLiveStatus = "incomplete_external_live";
    }
    return next;
  });
  const merged = buildObservedReport(observedJourneys);
  const finalGateReady =
    finalGateAudit?.ready === true &&
    gateReport?.overallReady === true &&
    gateBlockers.length === 0;
  const blockers = normalizeBlockers([
    ...(merged.blockers ?? []),
    ...gateBlockers,
    ...(!finalGateReady ? ["step6_final_acceptance_not_ready"] : []),
  ]);
  merged.blockers = blockers;
  merged.acceptanceReady = merged.acceptanceReady && finalGateReady && blockers.length === 0;
  merged.overallReady = merged.acceptanceReady;
  merged.finalGateReady = finalGateReady;
  merged.finalAcceptanceBlockers = normalizeBlockers([
    ...(gateReport?.finalGateSummary?.finalAcceptanceBlockers ?? []),
    ...(gateReport?.finalGateSummary?.liveProviderBlockers ?? []),
    ...(gateReport?.blockers ?? []),
  ]);
  merged.finalGateSummary = gateReport?.finalGateSummary ?? null;
  merged.finalGateReportKind = gateReport?.reportKind ?? null;
  merged.reportDigest = digestLabel(step6ReportDigestInput(merged));
  if (!finalGateReady) {
    console.error(
      `[step6_final_gate:blocked] ${JSON.stringify(summarizeFinalGateAudit(finalGateAudit))}`
    );
  }
  return merged;
}

function summarizeFinalGateAudit(finalGateAudit) {
  const report = finalGateAudit?.report;
  return {
    ready: finalGateAudit?.ready === true,
    overallReady: report?.overallReady === true,
    localDeterministicReady: report?.localDeterministicReady === true,
    externalLiveReady: report?.externalLiveReady === true,
    blockers: normalizeBlockers(finalGateAudit?.blockers ?? []),
    finalAcceptanceReady: report?.finalGateSummary?.finalAcceptanceReady === true,
    liveProviderReadyCount: report?.finalGateSummary?.liveProviderReadyCount ?? null,
    liveProviderWebCredit: report?.finalGateSummary?.liveProviderWebCredit === true,
    liveProviderMcpCredit: report?.finalGateSummary?.liveProviderMcpCredit === true,
    finalAcceptanceBlockers: normalizeBlockers(
      report?.finalGateSummary?.finalAcceptanceBlockers ?? []
    ),
    liveProviderBlockers: normalizeBlockers(report?.finalGateSummary?.liveProviderBlockers ?? []),
    journeyBlockers: (report?.journeys ?? [])
      .filter(row => Array.isArray(row?.blockers) && row.blockers.length > 0)
      .map(row => ({
        journeyId: row.journeyId,
        credited: row.credited === true,
        status: row.status,
        blockers: normalizeBlockers(row.blockers),
      })),
  };
}

function buildObservedReport(observedJourneys) {
  const journeyBlockers = validateObservedJourneysForReport(observedJourneys);
  const blockedLiveJourneys = observedJourneys
    .filter(
      row => row.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence"
    )
    .map(row => row.journeyId);
  const passedJourneys = observedJourneys.filter(step6JourneyPassed).map(row => row.journeyId);
  const failedJourneys = requiredJourneys.filter(
    id => !passedJourneys.includes(id) && !blockedLiveJourneys.includes(id)
  );
  const localReady = localJourneys.every(id => passedJourneys.includes(id));
  const externalLiveReady = externalLiveJourneys.every(id => passedJourneys.includes(id));
  const externalLiveBlockers = observedJourneys
    .filter(
      row => row.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence"
    )
    .flatMap(row =>
      row.blockers.map(blocker => `${row.journeyId}:${metadataSafeBlocker(blocker)}`)
    );
  const blockers = normalizeBlockers([
    ...journeyBlockers,
    ...(!localReady ? ["step6_local_deterministic_journeys_incomplete"] : []),
    ...(!externalLiveReady ? ["step6_external_live_evidence_blocked_or_incomplete"] : []),
    ...externalLiveBlockers,
  ]);
  const runId = `step6-tauri-webdriver-real-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const report = baseReport({
    e2eEnvironmentReady: true,
    evidenceSource: observedSource,
    runId,
    generatedAt,
    observedJourneys,
    passedJourneys,
    blockedLiveJourneys,
    failedJourneys,
    externalLiveBlockers,
    blockers,
  });
  report.reportDigest = digestLabel(step6ReportDigestInput(report));
  return report;
}

function writeBlockedReport(blockers, observedJourneys = []) {
  const runId = `step6-tauri-webdriver-blocked-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const normalizedBlockers = normalizeBlockers(blockers);
  const copiedObservedJourneys = Array.isArray(observedJourneys)
    ? observedJourneys.map(copyObservedJourney)
    : [];
  const externalRows =
    copiedObservedJourneys.length > 0
      ? copiedObservedJourneys.filter(row => row.kind === "external_live")
      : externalLiveJourneys.map(id =>
          blockedLiveJourney(
            journeys.find(row => row.id === id),
            normalizedBlockers
          )
        );
  const report = baseReport({
    e2eEnvironmentReady: false,
    evidenceSource: blockedSource,
    runId,
    generatedAt,
    observedJourneys: copiedObservedJourneys.length > 0 ? copiedObservedJourneys : externalRows,
    passedJourneys: [],
    blockedLiveJourneys: externalLiveJourneys,
    failedJourneys: localJourneys,
    externalLiveBlockers: externalLiveJourneys.flatMap(id =>
      normalizedBlockers.map(blocker => `${id}:${metadataSafeBlocker(blocker)}`)
    ),
    blockers: normalizedBlockers,
  });
  report.reportDigest = digestLabel(step6ReportDigestInput(report));
  writeReport(report);
}

function baseReport(input) {
  const localReady = localJourneys.every(id => input.passedJourneys.includes(id));
  const externalLiveReady = externalLiveJourneys.every(id => input.passedJourneys.includes(id));
  const blockers = normalizeBlockers(input.blockers);
  return {
    reportKind: "main_chat_step6_product_acceptance",
    schemaVersion,
    readinessSemantics,
    e2eEnvironmentReady: input.e2eEnvironmentReady,
    selfContainedRunner: true,
    smokePassed: Boolean(input.e2eEnvironmentReady),
    localDeterministicReady: localReady,
    externalLiveReady,
    acceptanceReady:
      input.e2eEnvironmentReady && localReady && externalLiveReady && blockers.length === 0,
    reportPath,
    evidenceSource: input.evidenceSource,
    runId: input.runId,
    generatedAt: input.generatedAt,
    reportDigest: "",
    localJourneyCount: localJourneys.length,
    externalLiveJourneyCount: externalLiveJourneys.length,
    requiredJourneys,
    passedJourneys: [...input.passedJourneys],
    blockedLiveJourneys: [...input.blockedLiveJourneys],
    failedJourneys: [...input.failedJourneys],
    observedJourneys: input.observedJourneys.map(copyObservedJourney),
    noSilentDurableWrite: input.observedJourneys.every(row => !row.silentDurableWriteDetected),
    noHiddenLegacyFallback: input.observedJourneys.every(row => !row.legacyFallbackUsed),
    noLocalEvidenceCreditedAsExternalLive: input.observedJourneys.every(
      row =>
        !row.localFixtureCreditedAsExternalLive &&
        (row.externalLiveStatus !== "credited_external_live" ||
          row.externalLiveProviderKind === "external_provider")
    ),
    noInventedUnavailableEvidence: input.observedJourneys.every(
      row => row.noInventedUnavailableEvidence === true && !row.unavailableEvidenceInvented
    ),
    uiStatusFromStructuredEvidence: input.observedJourneys.every(
      row => row.uiStatusEvidence.length > 0 && !hasUnsafeLabel(row.uiStatusEvidence)
    ),
    externalLiveBlockers: normalizeBlockers(input.externalLiveBlockers ?? []),
    blockers,
  };
}

function writeReport(report) {
  const outputPath = path.resolve(repoRoot, reportPath);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`);
}

function validateStep6PrepReport(prepReport) {
  const blockers = [];
  if (!prepReport?.prepared) blockers.push("tauri_webdriver_step6_prep_not_prepared");
  if (prepReport?.evidenceSource !== "real_app_state_task_continuity_seed") {
    blockers.push("tauri_webdriver_step6_prep_source_invalid");
  }
  if (prepReport?.directWritesExecuted) {
    blockers.push("tauri_webdriver_step6_prep_direct_write_detected");
  }
  if (prepReport?.durableLifemodelWritesExecuted) {
    blockers.push("tauri_webdriver_step6_prep_durable_lifemodel_write_detected");
  }
  if (prepReport?.fileOrExternalWritesExecuted) {
    blockers.push("tauri_webdriver_step6_prep_file_or_external_write_detected");
  }
  if (Array.isArray(prepReport?.blockers) && prepReport.blockers.length > 0) {
    blockers.push(
      ...prepReport.blockers.map(blocker =>
        metadataSafeBlocker(`tauri_webdriver_step6_prep_blocker:${blocker}`)
      )
    );
  } else if (prepReport?.blockers !== undefined && !Array.isArray(prepReport.blockers)) {
    blockers.push("tauri_webdriver_step6_prep_blockers_invalid");
  }
  for (const id of requiredPrepTaskIds) {
    const taskSessionId = prepReport?.taskSessionIds?.[id] ?? "";
    if (!taskSessionId) {
      blockers.push(`tauri_webdriver_step6_prep_missing_seeded_task:${id}`);
    } else if (!metadataSafeLabel(taskSessionId) || taskSessionId.startsWith("stage1_task_")) {
      blockers.push(`tauri_webdriver_step6_prep_task_id_unsafe:${id}`);
    }
  }
  return uniqueValues(blockers);
}

function validateObservedJourneysForReport(observedJourneys) {
  const blockers = [];
  if (!Array.isArray(observedJourneys)) return ["tauri_webdriver_step6_observed_journeys_missing"];
  const observedIds = observedJourneys.map(row => row?.journeyId ?? "");
  if (observedIds.length !== requiredJourneys.length) {
    blockers.push("tauri_webdriver_step6_observed_journey_count_mismatch");
  }
  if (observedIds.join(",") !== requiredJourneys.join(",")) {
    blockers.push("tauri_webdriver_step6_observed_journey_order_mismatch");
  }
  for (const id of observedIds.filter((id, index) => observedIds.indexOf(id) !== index)) {
    blockers.push(`tauri_webdriver_step6_duplicate_journey:${metadataSafeBlocker(id)}`);
  }
  for (const row of observedJourneys) {
    const expected = journeys.find(item => item.id === row?.journeyId);
    if (!expected) {
      blockers.push(`tauri_webdriver_step6_unknown_journey:${metadataSafeBlocker(row?.journeyId)}`);
      continue;
    }
    if (row.kind !== expected.kind)
      blockers.push(`tauri_webdriver_step6_kind_mismatch:${expected.id}`);
    const blockedLive =
      expected.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence";
    if (!metadataSafeLabel(row.journeyId))
      blockers.push(`tauri_webdriver_step6_id_unsafe:${expected.id}`);
    if (!metadataSafeLabel(row.observedVia)) {
      blockers.push(`tauri_webdriver_step6_observed_via_unsafe:${expected.id}`);
    }
    if (!metadataSafeLabel(row.entryPoint)) {
      blockers.push(`tauri_webdriver_step6_entry_point_unsafe:${expected.id}`);
    }
    if (row.entryPoint !== expectedEntryPointForStep6Journey(expected.id, blockedLive)) {
      blockers.push(`tauri_webdriver_step6_entry_point_mismatch:${expected.id}`);
    }
    if (!metadataSafeLabel(row.routeStrategy)) {
      blockers.push(`tauri_webdriver_step6_route_unsafe:${expected.id}`);
    } else if (routeStrategyMentionsHiddenFallback(row.routeStrategy)) {
      blockers.push(`tauri_webdriver_step6_route_legacy_or_fallback:${expected.id}`);
    }
    if (blockedLive && row.routeStrategy !== "blocked_external_live") {
      blockers.push(`tauri_webdriver_step6_route_strategy_mismatch:${expected.id}`);
    }
    if (hasUnsafeLabel(row.answerEvidence)) {
      blockers.push(`tauri_webdriver_step6_answer_label_unsafe:${expected.id}`);
    }
    if (hasUnsafeLabel(row.runtimeEvidence)) {
      blockers.push(`tauri_webdriver_step6_runtime_label_unsafe:${expected.id}`);
    }
    if (hasUnsafeLabel(row.uiStatusEvidence)) {
      blockers.push(`tauri_webdriver_step6_ui_label_unsafe:${expected.id}`);
    }
    if (hasUnsafeLabel(row.finalDeliverySections)) {
      blockers.push(`tauri_webdriver_step6_final_label_unsafe:${expected.id}`);
    }
    if (hasUnsafeLabel(row.traceEvidence)) {
      blockers.push(`tauri_webdriver_step6_trace_label_unsafe:${expected.id}`);
    }
    if (hasUnsafeBlocker(row.blockers)) {
      blockers.push(`tauri_webdriver_step6_blocker_label_unsafe:${expected.id}`);
    }
    if (row.unavailableEvidenceInvented) {
      blockers.push(`tauri_webdriver_step6_invented_unavailable_evidence:${expected.id}`);
    }
    if (row.legacyFallbackUsed)
      blockers.push(`tauri_webdriver_step6_legacy_fallback:${expected.id}`);
    if (row.silentDurableWriteDetected)
      blockers.push(`tauri_webdriver_step6_silent_write:${expected.id}`);
    if (row.localFixtureCreditedAsExternalLive) {
      blockers.push(`tauri_webdriver_step6_local_fixture_credited_as_live:${expected.id}`);
    }

    if (blockedLive) {
      if (row.observedVia !== "blocked_live_evidence_report") {
        blockers.push(`tauri_webdriver_step6_blocked_live_not_reported:${expected.id}`);
      }
      if (!row.uiStatusEvidence?.includes(blockedLiveUiStatus)) {
        blockers.push(`tauri_webdriver_step6_blocked_live_ui_status_missing:${expected.id}`);
      }
      if (!Array.isArray(row.blockers) || row.blockers.length === 0) {
        blockers.push(`tauri_webdriver_step6_blocked_live_missing_blocker:${expected.id}`);
      }
      continue;
    }

    for (const evidence of expected.expectedAnswerEvidence) {
      if (!row.answerEvidence?.includes(evidence)) {
        blockers.push(`tauri_webdriver_step6_answer_missing:${expected.id}:${evidence}`);
      }
    }
    for (const evidence of expected.expectedRuntimeEvidence) {
      if (!row.runtimeEvidence?.includes(evidence)) {
        blockers.push(`tauri_webdriver_step6_runtime_missing:${expected.id}:${evidence}`);
      }
    }
    if (!expected.expectedUiStatus.some(status => row.uiStatusEvidence?.includes(status))) {
      blockers.push(`tauri_webdriver_step6_ui_status_missing:${expected.id}`);
    }
    if (!Array.isArray(row.finalDeliverySections) || row.finalDeliverySections.length === 0) {
      blockers.push(`tauri_webdriver_step6_final_delivery_missing:${expected.id}`);
    }
    if (
      !expected.expectedFinalDeliverySections.some(section =>
        row.finalDeliverySections?.includes(section)
      )
    ) {
      blockers.push(`tauri_webdriver_step6_final_delivery_section_missing:${expected.id}`);
    }
    if (expected.kind === "deterministic_local") {
      if (row.observedVia !== "real_tauri_chat_or_control_path") {
        blockers.push(`tauri_webdriver_step6_local_not_real_tauri:${expected.id}`);
      }
      if (!metadataSafeRuntimeId(row.taskSessionId)) {
        blockers.push(`tauri_webdriver_step6_task_session_unobserved:${expected.id}`);
      }
      if (!metadataSafeRuntimeId(row.runId)) {
        blockers.push(`tauri_webdriver_step6_run_unobserved:${expected.id}`);
      }
      if (row.externalLiveStatus !== "not_applicable") {
        blockers.push(`tauri_webdriver_step6_local_live_status_invalid:${expected.id}`);
      }
      if (row.externalLiveProviderKind) {
        blockers.push(`tauri_webdriver_step6_local_provider_kind_invalid:${expected.id}`);
      }
    } else if (row.externalLiveStatus === "credited_external_live") {
      if (row.externalLiveProviderKind !== "external_provider") {
        blockers.push(`tauri_webdriver_step6_external_provider_missing:${expected.id}`);
      }
      if (!metadataSafeRuntimeId(row.taskSessionId)) {
        blockers.push(`tauri_webdriver_step6_live_task_session_unobserved:${expected.id}`);
      }
      if (!metadataSafeRuntimeId(row.runId)) {
        blockers.push(`tauri_webdriver_step6_live_run_unobserved:${expected.id}`);
      }
    } else {
      blockers.push(`tauri_webdriver_step6_live_evidence_missing:${expected.id}`);
    }
  }
  return uniqueValues(blockers.map(metadataSafeBlocker));
}

function step6JourneyPassed(row) {
  const expected = journeys.find(item => item.id === row.journeyId);
  if (!expected || row.kind !== expected.kind) return false;
  if (Array.isArray(row.blockers) && row.blockers.length > 0) {
    return false;
  }
  if (
    row.unavailableEvidenceInvented ||
    row.legacyFallbackUsed ||
    row.silentDurableWriteDetected ||
    row.localFixtureCreditedAsExternalLive
  ) {
    return false;
  }
  if (!expected.expectedAnswerEvidence.every(evidence => row.answerEvidence.includes(evidence))) {
    return false;
  }
  if (!expected.expectedRuntimeEvidence.every(evidence => row.runtimeEvidence.includes(evidence))) {
    return false;
  }
  if (!expected.expectedUiStatus.some(status => row.uiStatusEvidence.includes(status))) {
    return false;
  }
  if (
    row.finalDeliverySections.length === 0 ||
    !expected.expectedFinalDeliverySections.some(section =>
      row.finalDeliverySections.includes(section)
    )
  ) {
    return false;
  }
  if (
    !metadataSafeLabel(row.entryPoint) ||
    !metadataSafeLabel(row.routeStrategy) ||
    routeStrategyMentionsHiddenFallback(row.routeStrategy) ||
    row.entryPoint !== expectedEntryPointForStep6Journey(row.journeyId, false)
  ) {
    return false;
  }
  if (expected.kind === "deterministic_local") {
    return (
      row.observedVia === "real_tauri_chat_or_control_path" &&
      row.externalLiveStatus === "not_applicable" &&
      metadataSafeRuntimeId(row.taskSessionId) &&
      metadataSafeRuntimeId(row.runId)
    );
  }
  return (
    row.observedVia === "real_tauri_chat_or_control_path" &&
    row.externalLiveStatus === "credited_external_live" &&
    row.externalLiveProviderKind === "external_provider" &&
    metadataSafeRuntimeId(row.taskSessionId) &&
    metadataSafeRuntimeId(row.runId)
  );
}

function expectedEntryPointForStep6Journey(journeyId, blockedLive) {
  if (blockedLive) return "blocked_live_evidence_report";
  return journeyId === "S6-PERMISSION" || journeyId === "S6-RECOVERY"
    ? "task_continuity_control"
    : "ordinary_main_chat_input";
}

function finalDeliveryMatchesJourney(row, finalDeliverySections) {
  return row.expectedFinalDeliverySections.some(section => finalDeliverySections.includes(section));
}

function routeStrategyMentionsHiddenFallback(value) {
  const lower = String(value ?? "").toLowerCase();
  return lower.includes("legacy") || lower.includes("fallback");
}

function normalizeFinalSection(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function finalDeliverySectionsFromDetail(detail) {
  const delivery = detail?.finalDelivery ?? detail?.final_delivery;
  const metrics =
    delivery?.metadata && typeof delivery.metadata === "object"
      ? { ...delivery, ...delivery.metadata }
      : delivery;
  if (!metrics || typeof metrics !== "object") return [];
  const sections = [
    arrayLength(metrics, "completedActions") > 0 ? "completed_actions" : "",
    arrayLength(metrics, "observationsUsed") > 0 ? "sources_used" : "",
    arrayLength(metrics, "proposalsCreated") > 0 ? "proposals_created" : "",
    arrayLength(metrics, "blockers") > 0 ? "blocked_items" : "",
    arrayLength(metrics, "skippedWork") > 0 ? "skipped_work" : "",
    arrayLength(metrics, "pendingUserActions") > 0 ? "pending_user_actions" : "",
    arrayLength(metrics, "durableChanges") > 0 ? "durable_changes" : "",
    arrayLength(metrics, "nextSteps") > 0 ? "next_steps" : "",
  ].filter(Boolean);
  if (sections.length === 0 && structuredCompletedWorkMetrics(metrics)) {
    sections.push("completed_work");
  }
  return sections;
}

function structuredCompletedWorkMetrics(metrics) {
  if (!metrics || typeof metrics !== "object") return false;
  for (const key of ["summary", "answer", "headline"]) {
    if (typeof metrics[key] === "string" && metrics[key].trim()) return true;
  }
  const status = normalizeFinalSection(metrics.status);
  return status === "completed" || status === "delivered" || status === "succeeded";
}

function normalizedStatusFromContinuity(status, finalDeliverySections) {
  const normalized = normalizeFinalSection(status);
  if (["cancelled", "canceled"].includes(normalized)) return "cancelled";
  if (["completed", "delivered", "succeeded"].includes(normalized)) return "completed";
  if (["blocked", "waiting_permission"].includes(normalized)) return "blocked";
  if (finalDeliverySections.length > 0) return "completed";
  return normalized;
}

function arrayLength(value, key) {
  const item = value?.[key];
  return Array.isArray(item) ? item.length : 0;
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

function visibleControlEventForLabel(label) {
  const normalized =
    String(label ?? "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "_")
      .replace(/^_+|_+$/g, "") || "button";
  return `visible_control.${normalized}`;
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

async function waitForScript(sessionId, script, args, timeoutMs, timeoutError) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await executeScript(sessionId, script, args);
    if (value) return value;
    await delay(500);
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
      await delay(500);
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

function step6ReportDigestInput(report) {
  return [
    "step6-product-acceptance-report-v1",
    `reportKind=${digestPart(report.reportKind)}`,
    `schema=${digestPart(report.schemaVersion)}`,
    `readiness=${digestPart(report.readinessSemantics)}`,
    `e2eEnvironmentReady=${String(report.e2eEnvironmentReady)}`,
    `selfContainedRunner=${String(report.selfContainedRunner)}`,
    `smokePassed=${String(report.smokePassed)}`,
    `reportPath=${digestPart(report.reportPath)}`,
    `source=${digestPart(report.evidenceSource)}`,
    `runId=${digestPart(report.runId)}`,
    `generatedAt=${digestPart(report.generatedAt)}`,
    `localJourneyCount=${String(report.localJourneyCount)}`,
    `externalLiveJourneyCount=${String(report.externalLiveJourneyCount)}`,
    `localDeterministicReady=${String(report.localDeterministicReady)}`,
    `externalLiveReady=${String(report.externalLiveReady)}`,
    `acceptanceReady=${String(report.acceptanceReady)}`,
    `required=${digestArray(report.requiredJourneys)}`,
    `passed=${digestArray(report.passedJourneys)}`,
    `blockedLive=${digestArray(report.blockedLiveJourneys)}`,
    `failed=${digestArray(report.failedJourneys)}`,
    `externalLiveBlockers=${digestArray(report.externalLiveBlockers)}`,
    `blockers=${digestArray(report.blockers)}`,
    `noSilentDurableWrite=${String(report.noSilentDurableWrite)}`,
    `noHiddenLegacyFallback=${String(report.noHiddenLegacyFallback)}`,
    `noLocalEvidenceCreditedAsExternalLive=${String(report.noLocalEvidenceCreditedAsExternalLive)}`,
    `noInventedUnavailableEvidence=${String(report.noInventedUnavailableEvidence)}`,
    `uiStatusFromStructuredEvidence=${String(report.uiStatusFromStructuredEvidence)}`,
    "observed:",
    report.observedJourneys
      .map(row =>
        [
          row.journeyId,
          row.kind,
          row.observedVia,
          row.entryPoint,
          row.routeStrategy,
          row.taskSessionId,
          row.runId,
          digestArray(row.answerEvidence),
          digestArray(row.runtimeEvidence),
          digestArray(row.uiStatusEvidence),
          digestArray(row.finalDeliverySections),
          digestArray(row.traceEvidence),
          String(row.noInventedUnavailableEvidence),
          String(row.unavailableEvidenceInvented),
          String(row.legacyFallbackUsed),
          String(row.silentDurableWriteDetected),
          String(row.localFixtureCreditedAsExternalLive),
          row.externalLiveStatus,
          row.externalLiveProviderKind ?? "",
          digestArray(row.blockers),
        ]
          .map(digestPart)
          .join("|")
      )
      .join("\n"),
  ].join("\n");
}

function digestArray(values) {
  return (values ?? []).map(digestPart).join(",");
}

function digestPart(value) {
  return `${new TextEncoder().encode(String(value ?? "")).byteLength}:${value ?? ""}`;
}

function digestLabel(input) {
  const bytes = new TextEncoder().encode(input);
  return `bytes:${bytes.byteLength} hash:sha256:${createHash("sha256").update(input).digest("hex")}`;
}

function copyObservedJourney(row) {
  return {
    ...row,
    answerEvidence: [...(row.answerEvidence ?? [])],
    runtimeEvidence: [...(row.runtimeEvidence ?? [])],
    uiStatusEvidence: [...(row.uiStatusEvidence ?? [])],
    finalDeliverySections: [...(row.finalDeliverySections ?? [])],
    traceEvidence: [...(row.traceEvidence ?? [])],
    noInventedUnavailableEvidence: row.noInventedUnavailableEvidence === true,
    blockers: [...(row.blockers ?? [])],
  };
}

function metadataSafeRuntimeId(value) {
  return (
    metadataSafeLabel(value) &&
    !value.startsWith("step6_task_") &&
    !value.startsWith("step6_run_") &&
    !value.startsWith("stage1_task_") &&
    !value.startsWith("stage1_run_")
  );
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

function hasUnsafeBlocker(values) {
  return (
    Array.isArray(values) &&
    values.some(value => metadataSafeBlocker(value) !== String(value) || String(value).length > 160)
  );
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
  return uniqueValues((values ?? []).map(metadataSafeBlocker));
}

function uniqueValues(values) {
  return (values ?? []).filter((value, index) => value && values.indexOf(value) === index);
}

function delay(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}
