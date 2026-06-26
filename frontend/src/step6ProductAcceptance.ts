export type Step6JourneyKind = "deterministic_local" | "external_live";

export type Step6ObservedVia = "real_tauri_chat_or_control_path" | "blocked_live_evidence_report";

export type Step6ExternalLiveStatus =
  | "not_applicable"
  | "credited_external_live"
  | "incomplete_external_live"
  | "blocked_live_evidence";

export interface Step6ProductAcceptanceJourney {
  id: string;
  kind: Step6JourneyKind;
  title: string;
  prompt: string;
  expectedAnswerEvidence: string[];
  expectedRuntimeEvidence: string[];
  expectedUiStatus: string[];
  expectedFinalDeliverySections: string[];
}

export interface Step6ObservedProductJourney {
  journeyId: string;
  kind: Step6JourneyKind;
  observedVia: Step6ObservedVia;
  entryPoint: string;
  routeStrategy: string;
  taskSessionId: string;
  runId: string;
  answerEvidence: string[];
  runtimeEvidence: string[];
  uiStatusEvidence: string[];
  finalDeliverySections: string[];
  traceEvidence: string[];
  unavailableEvidenceInvented: boolean;
  legacyFallbackUsed: boolean;
  silentDurableWriteDetected: boolean;
  localFixtureCreditedAsExternalLive: boolean;
  externalLiveStatus: Step6ExternalLiveStatus;
  externalLiveProviderKind?: string | null;
  blockers: string[];
}

export interface Step6ProductAcceptanceReport {
  reportKind: "main_chat_step6_product_acceptance";
  schemaVersion: typeof STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION;
  readinessSemantics: typeof STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS;
  e2eEnvironmentReady: boolean;
  selfContainedRunner: boolean;
  smokePassed: boolean;
  localDeterministicReady: boolean;
  externalLiveReady: boolean;
  acceptanceReady: boolean;
  overallReady?: boolean;
  finalGateReady?: boolean;
  finalAcceptanceBlockers?: string[];
  finalGateSummary?: unknown;
  finalGateReportKind?: string | null;
  reportPath: string;
  evidenceSource: string;
  runId: string;
  generatedAt: string;
  reportDigest: string;
  localJourneyCount: number;
  externalLiveJourneyCount: number;
  requiredJourneys: string[];
  passedJourneys: string[];
  blockedLiveJourneys: string[];
  failedJourneys: string[];
  observedJourneys: Step6ObservedProductJourney[];
  noSilentDurableWrite: boolean;
  noHiddenLegacyFallback: boolean;
  noLocalEvidenceCreditedAsExternalLive: boolean;
  noInventedUnavailableEvidence: boolean;
  uiStatusFromStructuredEvidence: boolean;
  externalLiveBlockers: string[];
  blockers: string[];
}

interface BuildOptions {
  now?: Date;
  runId?: string;
}

export const STEP6_PRODUCT_ACCEPTANCE_REPORT_PATH =
  "frontend/test-results/main-chat-step6-product-acceptance-report.json";

export const STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION = "step6-product-acceptance-v1";

export const STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS =
  "step6_local_deterministic_required_external_live_opt_in_separate";

export const STEP6_PRODUCT_ACCEPTANCE_OBSERVED_SOURCE =
  "tauri_command_surface_step6_browser_observed";

export const STEP6_PRODUCT_ACCEPTANCE_BLOCKED_SOURCE = "tauri_command_surface_unavailable";
export const STEP6_BLOCKED_LIVE_UI_STATUS = "blocked_live_evidence";

export const STEP6_PRODUCT_ACCEPTANCE_JOURNEYS: Step6ProductAcceptanceJourney[] = [
  journey(
    "S6-CLOCK",
    "deterministic_local",
    "Current date/time/weekday",
    "今天星期几？现在的日期和时间是什么？",
    ["answer.clock_value"],
    ["source.runtime_fact", "runtime.clock"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-ROUTE",
    "deterministic_local",
    "Current model route",
    "你现在用什么模型和路线？",
    ["answer.route_summary"],
    ["source.runtime_fact", "runtime.provider_route"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-TOOLS",
    "deterministic_local",
    "Web/MCP/tool availability",
    "你现在能联网、使用 MCP 或执行工具吗？只说当前可用性证据。",
    ["answer.tool_availability"],
    ["source.runtime_fact", "runtime.tool_availability"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-FILE",
    "deterministic_local",
    "Workspace file read",
    "Read file `dogfood/project_brief.md` and summarize the governed observation.",
    ["answer.file_summary"],
    ["tool.file_read", "observation.workspace_file"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-DIRECT-SELF",
    "deterministic_local",
    "Direct answer completion self-state",
    "Answer directly: what is one practical reason to keep task evidence structured?",
    ["answer.direct_complete"],
    ["source.model_or_direct_answer", "self_state.completed_response"],
    ["completed"],
    ["completed_work", "completed_actions"]
  ),
  journey(
    "S6-PROPOSAL",
    "deterministic_local",
    "Proposal pending durable-change state",
    "Remember that I prefer Step 6 evidence before durable writes.",
    ["answer.proposal_pending"],
    ["proposal.created", "durable_write.not_completed"],
    ["proposal_pending"],
    ["proposals_created", "pending_user_actions"]
  ),
  journey(
    "S6-BLOCKED",
    "deterministic_local",
    "Blocked task next action",
    "Use web while network policy blocks it, then tell me the safe next action.",
    ["answer.blocked_next_action"],
    ["blocker.created", "safe_next_control"],
    ["restricted", "blocked"],
    ["blocked_items", "next_steps", "pending_user_actions"]
  ),
  journey(
    "S6-PERMISSION",
    "deterministic_local",
    "Permission review/accept/resume",
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
    ["completed_actions", "sources_used", "completed_work"]
  ),
  journey(
    "S6-LIVE-WEB",
    "external_live",
    "Provider-backed web read",
    'For this live eval, call the allowed web.search candidate exactly once before answering. Return only a JSON action envelope with actions[0].name="web.search", actions[0].action_type="mcp_tool", and actions[0].arguments={}; do not answer directly.',
    ["answer.external_web_summary"],
    ["live_provider.external", "tool.web_read"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-LIVE-MCP",
    "external_live",
    "Provider-backed MCP read",
    'For candidate ranking, if the candidate ids include builtin_echo, put builtin_echo first and include every allowed candidate id exactly once. For action execution, do not explain or answer directly. Return exactly this JSON envelope and nothing else: {"final":"pending governed read","actions":[{"name":"builtin_echo","action_type":"mcp_tool","arguments":{}}],"thought_summary":"Need a governed read.","warnings":[]}. This is a governed mcp read-only utility request.',
    ["answer.external_mcp_summary"],
    ["live_provider.external", "tool.mcp_read", "provider_ranked_selection"],
    ["completed"],
    ["sources_used", "completed_work", "completed_actions"]
  ),
  journey(
    "S6-RECOVERY",
    "deterministic_local",
    "Blocked-task recovery/stop",
    "Recover or explicitly stop a blocked task using the visible task controls.",
    ["answer.recovery_or_stop"],
    ["control.retry_or_cancel", "final_delivery.recorded"],
    ["completed", "blocked", "cancelled"],
    ["blocked_items", "next_steps", "skipped_work", "completed_actions", "completed_work"]
  ),
];

export const STEP6_REQUIRED_PRODUCT_JOURNEYS = STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.map(
  journey => journey.id
);

export function buildStep6BlockedProductAcceptanceReport(
  blockers: string[],
  options: BuildOptions = {}
): Step6ProductAcceptanceReport {
  const runId = options.runId ?? `step6-product-e2e-blocked-${Date.now()}`;
  const generatedAt = (options.now ?? new Date()).toISOString();
  const normalizedBlockers = normalizeBlockers([
    "step6_product_acceptance_e2e_blocked",
    ...blockers,
  ]);
  const externalLiveBlockers = blockedExternalLiveBlockers(normalizedBlockers);
  const observedJourneys = externalLiveJourneyIds().map(id =>
    blockedExternalLiveObservedJourney(id, normalizedBlockers)
  );
  const report = step6Report({
    e2eEnvironmentReady: false,
    evidenceSource: STEP6_PRODUCT_ACCEPTANCE_BLOCKED_SOURCE,
    runId,
    generatedAt,
    observedJourneys,
    passedJourneys: [],
    blockedLiveJourneys: externalLiveJourneyIds(),
    failedJourneys: deterministicJourneyIds(),
    blockers: normalizedBlockers,
    externalLiveBlockers,
  });
  return { ...report, reportDigest: step6ReportDigest(report) };
}

export function buildStep6ProductAcceptanceReportFromObservedJourneys(
  observedJourneys: Step6ObservedProductJourney[],
  options: BuildOptions = {}
): Step6ProductAcceptanceReport {
  const runId = options.runId ?? `step6-product-e2e-${Date.now()}`;
  const generatedAt = (options.now ?? new Date()).toISOString();
  const journeyBlockers = step6ObservedJourneyBlockers(observedJourneys);
  const blockedLiveJourneys = observedJourneys
    .filter(
      row => row.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence"
    )
    .map(row => row.journeyId);
  const passedJourneys = observedJourneys
    .filter(row => step6JourneyPassed(row))
    .map(row => row.journeyId);
  const failedJourneys = STEP6_REQUIRED_PRODUCT_JOURNEYS.filter(
    id => !passedJourneys.includes(id) && !blockedLiveJourneys.includes(id)
  );
  const localDeterministicReady = deterministicJourneyIds().every(id =>
    passedJourneys.includes(id)
  );
  const externalLiveReady = externalLiveJourneyIds().every(id => passedJourneys.includes(id));
  const externalLiveBlockers = observedJourneys
    .filter(
      row => row.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence"
    )
    .flatMap(row =>
      row.blockers.map(blocker => `${row.journeyId}:${metadataSafeBlocker(blocker)}`)
    );
  const blockers = normalizeBlockers([
    ...journeyBlockers,
    ...(!localDeterministicReady ? ["step6_local_deterministic_journeys_incomplete"] : []),
    ...(!externalLiveReady ? ["step6_external_live_evidence_blocked_or_incomplete"] : []),
    ...externalLiveBlockers,
  ]);
  const report = step6Report({
    e2eEnvironmentReady: true,
    evidenceSource: STEP6_PRODUCT_ACCEPTANCE_OBSERVED_SOURCE,
    runId,
    generatedAt,
    observedJourneys,
    passedJourneys,
    blockedLiveJourneys,
    failedJourneys,
    blockers,
    externalLiveBlockers: normalizeBlockers(externalLiveBlockers),
  });
  return { ...report, reportDigest: step6ReportDigest(report) };
}

export function step6ObservedJourneyBlockers(
  observedJourneys: Step6ObservedProductJourney[]
): string[] {
  const blockers: string[] = [];
  const observedIds = observedJourneys.map(row => row.journeyId);
  const expectedById = new Map(STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.map(row => [row.id, row]));

  if (observedJourneys.length !== STEP6_REQUIRED_PRODUCT_JOURNEYS.length) {
    blockers.push("step6_observed_journey_count_mismatch");
  }
  if (!arraysEqual(observedIds, STEP6_REQUIRED_PRODUCT_JOURNEYS)) {
    blockers.push("step6_observed_journey_order_mismatch");
  }
  for (const id of observedIds.filter((id, index) => observedIds.indexOf(id) !== index)) {
    blockers.push(`step6_duplicate_journey:${metadataSafeBlocker(id)}`);
  }
  for (const id of STEP6_REQUIRED_PRODUCT_JOURNEYS) {
    if (!observedIds.includes(id)) blockers.push(`step6_missing_journey:${id}`);
  }

  const runtimeIdentityRows = observedJourneys.filter(row => {
    const expected = expectedById.get(row.journeyId);
    return !(
      expected?.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence"
    );
  });
  const taskSessionIds = runtimeIdentityRows.map(row => row.taskSessionId).filter(Boolean);
  if (uniqueValues(taskSessionIds).length !== taskSessionIds.length) {
    blockers.push("step6_observed_task_session_ids_not_distinct");
  }
  const runIds = runtimeIdentityRows.map(row => row.runId).filter(Boolean);
  if (uniqueValues(runIds).length !== runIds.length) {
    blockers.push("step6_observed_run_ids_not_distinct");
  }

  for (const row of observedJourneys) {
    const expected = expectedById.get(row.journeyId);
    if (!expected) {
      blockers.push(`step6_unknown_journey:${metadataSafeBlocker(row.journeyId)}`);
      continue;
    }
    if (row.kind !== expected.kind) blockers.push(`step6_kind_mismatch:${row.journeyId}`);
    const blockedExternalLive =
      expected.kind === "external_live" && row.externalLiveStatus === "blocked_live_evidence";
    if (!metadataSafeLabel(row.journeyId))
      blockers.push(`step6_journey_id_unsafe:${row.journeyId}`);
    if (!metadataSafeLabel(row.observedVia))
      blockers.push(`step6_observed_via_unsafe:${row.journeyId}`);
    if (!metadataSafeLabel(row.entryPoint))
      blockers.push(`step6_entry_point_unsafe:${row.journeyId}`);
    const expectedEntryPoint = expectedEntryPointForJourney(row.journeyId, blockedExternalLive);
    if (row.entryPoint !== expectedEntryPoint) {
      blockers.push(`step6_entry_point_mismatch:${row.journeyId}`);
    }
    if (!metadataSafeLabel(row.routeStrategy)) {
      blockers.push(`step6_route_unsafe:${row.journeyId}`);
    } else if (routeStrategyMentionsHiddenFallback(row.routeStrategy)) {
      blockers.push(`step6_route_legacy_or_fallback:${row.journeyId}`);
    }
    if (blockedExternalLive && row.routeStrategy !== "blocked_external_live") {
      blockers.push(`step6_route_strategy_mismatch:${row.journeyId}`);
    }
    if (hasUnsafeLabel(row.answerEvidence))
      blockers.push(`step6_answer_evidence_unsafe:${row.journeyId}`);
    if (hasUnsafeLabel(row.runtimeEvidence))
      blockers.push(`step6_runtime_evidence_unsafe:${row.journeyId}`);
    if (hasUnsafeLabel(row.uiStatusEvidence))
      blockers.push(`step6_ui_status_unsafe:${row.journeyId}`);
    if (hasUnsafeLabel(row.finalDeliverySections))
      blockers.push(`step6_final_delivery_unsafe:${row.journeyId}`);
    if (hasUnsafeLabel(row.traceEvidence))
      blockers.push(`step6_trace_evidence_unsafe:${row.journeyId}`);
    if (hasUnsafeBlocker(row.blockers)) blockers.push(`step6_blocker_unsafe:${row.journeyId}`);
    if (row.legacyFallbackUsed) blockers.push(`step6_legacy_fallback:${row.journeyId}`);
    if (row.silentDurableWriteDetected) blockers.push(`step6_silent_write:${row.journeyId}`);
    if (row.localFixtureCreditedAsExternalLive) {
      blockers.push(`step6_local_fixture_credited_as_live:${row.journeyId}`);
    }
    if (blockedExternalLive) {
      if (row.observedVia !== "blocked_live_evidence_report") {
        blockers.push(`step6_blocked_live_not_reported:${row.journeyId}`);
      }
      if (!row.uiStatusEvidence.includes(STEP6_BLOCKED_LIVE_UI_STATUS)) {
        blockers.push(`step6_blocked_live_ui_status_missing:${row.journeyId}`);
      }
      if (row.blockers.length === 0) {
        blockers.push(`step6_blocked_live_missing_blocker:${row.journeyId}`);
      }
      if (row.unavailableEvidenceInvented) {
        blockers.push(`step6_invented_unavailable_evidence:${row.journeyId}`);
      }
      continue;
    }
    for (const evidence of expected.expectedAnswerEvidence) {
      if (!row.answerEvidence.includes(evidence)) {
        blockers.push(`step6_answer_evidence_missing:${row.journeyId}:${evidence}`);
      }
    }
    for (const evidence of expected.expectedRuntimeEvidence) {
      if (!row.runtimeEvidence.includes(evidence)) {
        blockers.push(`step6_runtime_evidence_missing:${row.journeyId}:${evidence}`);
      }
    }
    if (!expected.expectedUiStatus.some(status => row.uiStatusEvidence.includes(status))) {
      blockers.push(`step6_ui_status_missing:${row.journeyId}`);
    }
    if (row.finalDeliverySections.length === 0) {
      blockers.push(`step6_final_delivery_missing:${row.journeyId}`);
    }
    if (
      !expected.expectedFinalDeliverySections.some(section =>
        row.finalDeliverySections.includes(section)
      )
    ) {
      blockers.push(`step6_final_delivery_section_missing:${row.journeyId}`);
    }
    if (row.unavailableEvidenceInvented) {
      blockers.push(`step6_invented_unavailable_evidence:${row.journeyId}`);
    }
    if (expected.kind === "deterministic_local") {
      if (row.observedVia !== "real_tauri_chat_or_control_path") {
        blockers.push(`step6_local_not_real_tauri_observed:${row.journeyId}`);
      }
      if (!metadataSafeRuntimeId(row.taskSessionId)) {
        blockers.push(`step6_task_session_unobserved:${row.journeyId}`);
      }
      if (!metadataSafeRuntimeId(row.runId)) blockers.push(`step6_run_unobserved:${row.journeyId}`);
      if (row.externalLiveStatus !== "not_applicable") {
        blockers.push(`step6_local_journey_has_live_status:${row.journeyId}`);
      }
      if (row.externalLiveProviderKind) {
        blockers.push(`step6_local_journey_has_provider_kind:${row.journeyId}`);
      }
    } else {
      if (row.externalLiveStatus === "credited_external_live") {
        if (row.observedVia !== "real_tauri_chat_or_control_path") {
          blockers.push(`step6_live_not_real_tauri_observed:${row.journeyId}`);
        }
        if (!metadataSafeRuntimeId(row.taskSessionId)) {
          blockers.push(`step6_live_task_session_unobserved:${row.journeyId}`);
        }
        if (!metadataSafeRuntimeId(row.runId)) {
          blockers.push(`step6_live_run_unobserved:${row.journeyId}`);
        }
        if (row.externalLiveProviderKind !== "external_provider") {
          blockers.push(`step6_external_provider_missing:${row.journeyId}`);
        }
      } else {
        blockers.push(`step6_live_evidence_missing:${row.journeyId}`);
      }
    }
  }

  return normalizeBlockers(blockers);
}

export function step6JourneyPassed(row: Step6ObservedProductJourney): boolean {
  const expected = STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.find(journey => journey.id === row.journeyId);
  if (!expected || row.kind !== expected.kind) return false;
  if (
    row.unavailableEvidenceInvented ||
    row.legacyFallbackUsed ||
    row.silentDurableWriteDetected ||
    row.localFixtureCreditedAsExternalLive
  ) {
    return false;
  }
  const expectedEvidencePresent =
    expected.expectedAnswerEvidence.every(evidence => row.answerEvidence.includes(evidence)) &&
    expected.expectedRuntimeEvidence.every(evidence => row.runtimeEvidence.includes(evidence)) &&
    expected.expectedUiStatus.some(status => row.uiStatusEvidence.includes(status)) &&
    expected.expectedFinalDeliverySections.some(section =>
      row.finalDeliverySections.includes(section)
    );
  if (!expectedEvidencePresent || row.finalDeliverySections.length === 0) return false;
  if (
    !metadataSafeLabel(row.entryPoint) ||
    !metadataSafeLabel(row.routeStrategy) ||
    routeStrategyMentionsHiddenFallback(row.routeStrategy) ||
    row.entryPoint !== expectedEntryPointForJourney(row.journeyId, false)
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

function expectedEntryPointForJourney(journeyId: string, blockedExternalLive: boolean): string {
  if (blockedExternalLive) return "blocked_live_evidence_report";
  return journeyId === "S6-PERMISSION" || journeyId === "S6-RECOVERY"
    ? "task_continuity_control"
    : "ordinary_main_chat_input";
}

function routeStrategyMentionsHiddenFallback(value: string): boolean {
  const lower = value.toLowerCase();
  return lower.includes("legacy") || lower.includes("fallback");
}

function step6Report(input: {
  e2eEnvironmentReady: boolean;
  evidenceSource: string;
  runId: string;
  generatedAt: string;
  observedJourneys: Step6ObservedProductJourney[];
  passedJourneys: string[];
  blockedLiveJourneys: string[];
  failedJourneys: string[];
  blockers: string[];
  externalLiveBlockers?: string[];
}): Step6ProductAcceptanceReport {
  const noSilentDurableWrite = input.observedJourneys.every(row => !row.silentDurableWriteDetected);
  const noHiddenLegacyFallback = input.observedJourneys.every(row => !row.legacyFallbackUsed);
  const noLocalEvidenceCreditedAsExternalLive = input.observedJourneys.every(
    row =>
      !row.localFixtureCreditedAsExternalLive &&
      (row.externalLiveStatus !== "credited_external_live" ||
        row.externalLiveProviderKind === "external_provider")
  );
  const noInventedUnavailableEvidence = input.observedJourneys.every(
    row => !row.unavailableEvidenceInvented
  );
  const uiStatusFromStructuredEvidence = input.observedJourneys.every(
    row => row.uiStatusEvidence.length > 0 && !hasUnsafeLabel(row.uiStatusEvidence)
  );
  const localDeterministicReady = deterministicJourneyIds().every(id =>
    input.passedJourneys.includes(id)
  );
  const externalLiveReady = externalLiveJourneyIds().every(id => input.passedJourneys.includes(id));
  const blockers = normalizeBlockers(input.blockers);

  return {
    reportKind: "main_chat_step6_product_acceptance",
    schemaVersion: STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION,
    readinessSemantics: STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS,
    e2eEnvironmentReady: input.e2eEnvironmentReady,
    selfContainedRunner: true,
    smokePassed: input.e2eEnvironmentReady,
    localDeterministicReady,
    externalLiveReady,
    acceptanceReady:
      input.e2eEnvironmentReady &&
      localDeterministicReady &&
      externalLiveReady &&
      blockers.length === 0,
    reportPath: STEP6_PRODUCT_ACCEPTANCE_REPORT_PATH,
    evidenceSource: input.evidenceSource,
    runId: input.runId,
    generatedAt: input.generatedAt,
    reportDigest: "",
    localJourneyCount: deterministicJourneyIds().length,
    externalLiveJourneyCount: externalLiveJourneyIds().length,
    requiredJourneys: [...STEP6_REQUIRED_PRODUCT_JOURNEYS],
    passedJourneys: [...input.passedJourneys],
    blockedLiveJourneys: [...input.blockedLiveJourneys],
    failedJourneys: [...input.failedJourneys],
    observedJourneys: input.observedJourneys.map(row => ({
      ...row,
      answerEvidence: [...row.answerEvidence],
      runtimeEvidence: [...row.runtimeEvidence],
      uiStatusEvidence: [...row.uiStatusEvidence],
      finalDeliverySections: [...row.finalDeliverySections],
      traceEvidence: [...row.traceEvidence],
      blockers: [...row.blockers],
    })),
    noSilentDurableWrite,
    noHiddenLegacyFallback,
    noLocalEvidenceCreditedAsExternalLive,
    noInventedUnavailableEvidence,
    uiStatusFromStructuredEvidence,
    externalLiveBlockers: normalizeBlockers(input.externalLiveBlockers ?? []),
    blockers,
  };
}

export function step6ReportDigest(report: Step6ProductAcceptanceReport): string {
  return digestLabel(step6ReportDigestInput({ ...report, reportDigest: "" }));
}

function step6ReportDigestInput(report: Step6ProductAcceptanceReport): string {
  const rows = report.observedJourneys
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
    .join("\n");

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
    rows,
  ].join("\n");
}

function blockedExternalLiveObservedJourney(
  journeyId: string,
  blockers: string[]
): Step6ObservedProductJourney {
  return {
    journeyId,
    kind: "external_live",
    observedVia: "blocked_live_evidence_report",
    entryPoint: "blocked_live_evidence_report",
    routeStrategy: "blocked_external_live",
    taskSessionId: "",
    runId: "",
    answerEvidence: [],
    runtimeEvidence: [],
    uiStatusEvidence: [STEP6_BLOCKED_LIVE_UI_STATUS],
    finalDeliverySections: [],
    traceEvidence: [],
    unavailableEvidenceInvented: false,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    localFixtureCreditedAsExternalLive: false,
    externalLiveStatus: "blocked_live_evidence",
    externalLiveProviderKind: null,
    blockers: [...blockers],
  };
}

function blockedExternalLiveBlockers(blockers: string[]): string[] {
  const liveBlockers = blockers.length > 0 ? blockers : ["external_live_evidence_unavailable"];
  return externalLiveJourneyIds().flatMap(id =>
    liveBlockers.map(blocker => `${id}:${metadataSafeBlocker(blocker)}`)
  );
}

function journey(
  id: string,
  kind: Step6JourneyKind,
  title: string,
  prompt: string,
  expectedAnswerEvidence: string[],
  expectedRuntimeEvidence: string[],
  expectedUiStatus: string[],
  expectedFinalDeliverySections: string[]
): Step6ProductAcceptanceJourney {
  return {
    id,
    kind,
    title,
    prompt,
    expectedAnswerEvidence,
    expectedRuntimeEvidence,
    expectedUiStatus,
    expectedFinalDeliverySections,
  };
}

function deterministicJourneyIds(): string[] {
  return STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.filter(row => row.kind === "deterministic_local").map(
    row => row.id
  );
}

function externalLiveJourneyIds(): string[] {
  return STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.filter(row => row.kind === "external_live").map(
    row => row.id
  );
}

function metadataSafeRuntimeId(value: string): boolean {
  return (
    metadataSafeLabel(value) &&
    !value.startsWith("step6_task_") &&
    !value.startsWith("step6_run_") &&
    !value.startsWith("stage1_task_") &&
    !value.startsWith("stage1_run_")
  );
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

function normalizeBlockers(values: string[]): string[] {
  return uniqueValues(values.map(metadataSafeBlocker));
}

function hasUnsafeLabel(values: string[]): boolean {
  return values.some(value => !metadataSafeLabel(value));
}

function hasUnsafeBlocker(values: string[]): boolean {
  return values.some(value => metadataSafeBlocker(value) !== value || value.length > 160);
}

function arraysEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function digestArray(values: string[]): string {
  return values.map(digestPart).join(",");
}

function digestPart(value: string): string {
  const bytes = new TextEncoder().encode(value);
  return `${bytes.byteLength}:${value}`;
}

function digestLabel(input: string): string {
  const bytes = new TextEncoder().encode(input);
  return `bytes:${bytes.byteLength} hash:sha256:${sha256(input)}`;
}

function sha256(input: string): string {
  const bytes = new TextEncoder().encode(input);
  const hashWords = bytesToWords(bytes);
  const bitLength = bytes.byteLength * 8;
  hashWords[bitLength >> 5] |= 0x80 << (24 - (bitLength % 32));
  hashWords[(((bitLength + 64) >> 9) << 4) + 15] = bitLength;

  const constants = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  let h0 = 0x6a09e667;
  let h1 = 0xbb67ae85;
  let h2 = 0x3c6ef372;
  let h3 = 0xa54ff53a;
  let h4 = 0x510e527f;
  let h5 = 0x9b05688c;
  let h6 = 0x1f83d9ab;
  let h7 = 0x5be0cd19;
  const words = new Array<number>(64);

  for (let i = 0; i < hashWords.length; i += 16) {
    for (let t = 0; t < 16; t += 1) words[t] = hashWords[i + t] | 0;
    for (let t = 16; t < 64; t += 1) {
      const s0 =
        rotateRight(words[t - 15], 7) ^ rotateRight(words[t - 15], 18) ^ (words[t - 15] >>> 3);
      const s1 =
        rotateRight(words[t - 2], 17) ^ rotateRight(words[t - 2], 19) ^ (words[t - 2] >>> 10);
      words[t] = add(add(add(words[t - 16], s0), words[t - 7]), s1);
    }

    let a = h0;
    let b = h1;
    let c = h2;
    let d = h3;
    let e = h4;
    let f = h5;
    let g = h6;
    let h = h7;
    for (let t = 0; t < 64; t += 1) {
      const s1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
      const ch = (e & f) ^ (~e & g);
      const temp1 = add(add(add(add(h, s1), ch), constants[t]), words[t]);
      const s0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = add(s0, maj);
      h = g;
      g = f;
      f = e;
      e = add(d, temp1);
      d = c;
      c = b;
      b = a;
      a = add(temp1, temp2);
    }
    h0 = add(h0, a);
    h1 = add(h1, b);
    h2 = add(h2, c);
    h3 = add(h3, d);
    h4 = add(h4, e);
    h5 = add(h5, f);
    h6 = add(h6, g);
    h7 = add(h7, h);
  }

  return [h0, h1, h2, h3, h4, h5, h6, h7]
    .map(value => (value >>> 0).toString(16).padStart(8, "0"))
    .join("");
}

function bytesToWords(bytes: Uint8Array): number[] {
  const words: number[] = [];
  for (let i = 0; i < bytes.length; i += 1) {
    words[i >> 2] |= bytes[i] << (24 - (i % 4) * 8);
  }
  return words;
}

function rotateRight(value: number, shift: number): number {
  return (value >>> shift) | (value << (32 - shift));
}

function add(left: number, right: number): number {
  return (left + right) | 0;
}

function uniqueValues(values: string[]): string[] {
  return values.filter((value, index) => value && values.indexOf(value) === index);
}
