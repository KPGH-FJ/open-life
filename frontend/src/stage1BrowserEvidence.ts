import type { MainChatAgentStage1DogfoodReport } from "./tauri";

export const STAGE1_REQUIRED_BROWSER_JOURNEYS = Array.from(
  { length: 36 },
  (_, index) => `D${String(index + 1).padStart(2, "0")}`
);

const REPORT_PATH = "frontend/test-results/main-chat-stage1-dogfood-report.json";
const PASSING_SOURCE = "tauri_command_surface_browser_observed";
const BLOCKED_SOURCE = "tauri_command_surface_unavailable";
const OBSERVED_VIA = "real_tauri_chat_or_control_path";

export interface Stage1ObservedBrowserScenario {
  scenarioId: string;
  observedVia: typeof OBSERVED_VIA;
  entryPoint: string;
  taskSessionId: string;
  runId: string;
  routeStrategy: string;
  runtimeEvents: string[];
  visibleUiStates: string[];
  finalDeliverySections: string[];
  visibleBlockers: string[];
  runtimeEvidenceObserved: boolean;
  uiStateObserved: boolean;
  finalDeliveryObserved: boolean;
  nonFakeEvidenceObserved: boolean;
  legacyFallbackUsed: boolean;
  silentDurableWriteDetected: boolean;
  fakeExecutionDetected: boolean;
}

export interface Stage1BrowserEvidenceReport {
  browserE2eEnvironmentReady: boolean;
  selfContainedRunner: boolean;
  smokePassed: boolean;
  reportPath: string;
  evidenceSource: string;
  runId: string;
  generatedAt: string;
  reportDigest: string;
  requiredJourneys: string[];
  passedJourneys: string[];
  failedJourneys: string[];
  observedScenarios: Stage1ObservedBrowserScenario[];
  blockers: string[];
}

interface BuildOptions {
  now?: Date;
  runId?: string;
}

export function buildStage1BlockedBrowserEvidenceReport(
  blockers: string[],
  options: BuildOptions = {}
): Stage1BrowserEvidenceReport {
  const runId = options.runId ?? `stage1-browser-e2e-blocked-${Date.now()}`;
  const generatedAt = (options.now ?? new Date()).toISOString();
  const normalizedBlockers = uniqueValues(["not_ready_browser_e2e_blocked", ...blockers]);
  return {
    browserE2eEnvironmentReady: false,
    selfContainedRunner: true,
    smokePassed: false,
    reportPath: REPORT_PATH,
    evidenceSource: BLOCKED_SOURCE,
    runId,
    generatedAt,
    reportDigest: digestLabel(
      JSON.stringify({
        runId,
        generatedAt,
        requiredJourneys: STAGE1_REQUIRED_BROWSER_JOURNEYS,
        failedJourneys: STAGE1_REQUIRED_BROWSER_JOURNEYS,
        observedScenarios: [],
        blockers: normalizedBlockers,
      })
    ),
    requiredJourneys: [...STAGE1_REQUIRED_BROWSER_JOURNEYS],
    passedJourneys: [],
    failedJourneys: [...STAGE1_REQUIRED_BROWSER_JOURNEYS],
    observedScenarios: [],
    blockers: normalizedBlockers,
  };
}

export function buildStage1PassingBrowserEvidenceReportFromObservedScenarios(
  observedScenarios: Stage1ObservedBrowserScenario[],
  gateReport: unknown,
  options: BuildOptions = {}
): Stage1BrowserEvidenceReport {
  const blockers = stage1GateRuntimeEvidenceBlockers(gateReport);
  if (blockers.length > 0) {
    throw new Error(["stage1_browser_gate_runtime_evidence_incomplete", ...blockers].join(":"));
  }
  const observedBlockers = stage1ObservedScenarioBlockers(observedScenarios, gateReport);
  if (observedBlockers.length > 0) {
    throw new Error(
      ["stage1_browser_observed_scenarios_incomplete", ...observedBlockers].join(":")
    );
  }
  const report = gateReport as MainChatAgentStage1DogfoodReport;
  const runId = options.runId ?? `stage1-browser-e2e-real-${Date.now()}`;
  const generatedAt = (options.now ?? new Date()).toISOString();
  const evidenceSummary = {
    runId,
    generatedAt,
    defaultScenarioCount: report.defaultScenarioCount,
    taskSessionCreatedCount: report.taskSessionCreatedCount,
    ordinaryChatScenarioCount: report.ordinaryChatScenarioCount,
    seededTaskControlScenarioCount: report.seededTaskControlScenarioCount,
    finalDeliveryVerifiedScenarioCount: report.finalDeliveryVerifiedScenarioCount,
    scenarioIds: STAGE1_REQUIRED_BROWSER_JOURNEYS,
    observedScenarios,
  };

  return {
    browserE2eEnvironmentReady: true,
    selfContainedRunner: true,
    smokePassed: true,
    reportPath: REPORT_PATH,
    evidenceSource: PASSING_SOURCE,
    runId,
    generatedAt,
    reportDigest: digestLabel(JSON.stringify(evidenceSummary)),
    requiredJourneys: [...STAGE1_REQUIRED_BROWSER_JOURNEYS],
    passedJourneys: [...STAGE1_REQUIRED_BROWSER_JOURNEYS],
    failedJourneys: [],
    observedScenarios: observedScenarios.map(row => ({
      ...row,
      runtimeEvents: [...row.runtimeEvents],
      visibleUiStates: [...row.visibleUiStates],
      finalDeliverySections: [...row.finalDeliverySections],
      visibleBlockers: [...row.visibleBlockers],
    })),
    blockers: [],
  };
}

function stage1GateRuntimeEvidenceBlockers(gateReport: unknown): string[] {
  const report = gateReport as Partial<MainChatAgentStage1DogfoodReport> | null;
  const blockers: string[] = [];
  if (!report || report.reportKind !== "main_chat_agent_stage1_dogfood_gate") {
    blockers.push("stage1_gate_report_kind_missing");
    return blockers;
  }
  if (report.defaultScenarioCount !== 36) blockers.push("default_scenario_count_not_36");
  if (report.taskSessionCreatedCount !== 36) blockers.push("task_session_count_not_36");
  if ((report.ordinaryChatScenarioCount ?? 0) < 20) {
    blockers.push("ordinary_chat_count_below_20");
  }
  if ((report.seededTaskControlScenarioCount ?? 0) < 8) {
    blockers.push("seeded_control_count_below_8");
  }
  if (report.finalDeliveryVerifiedScenarioCount !== 36) {
    blockers.push("final_delivery_count_not_36");
  }
  if ((report.legacyFallbackCount ?? 0) !== 0) blockers.push("legacy_fallback_detected");
  if ((report.silentDurableWriteCount ?? 0) !== 0) blockers.push("silent_write_detected");
  if ((report.fakeExecutionDetectedCount ?? 0) !== 0) blockers.push("fake_execution_detected");

  const defaultRows =
    report.scenarios?.filter(row => row.liveProviderEvidence === "default_deterministic") ?? [];
  if (defaultRows.length !== 36) blockers.push("default_row_count_not_36");

  const rowIds = new Set(defaultRows.map(row => row.scenarioId));
  for (const id of STAGE1_REQUIRED_BROWSER_JOURNEYS) {
    if (!rowIds.has(id)) blockers.push(`missing_scenario:${id}`);
  }

  for (const row of defaultRows) {
    if (!row.taskSessionId || !row.runId) {
      blockers.push(`missing_runtime_identity:${row.scenarioId}`);
    }
    if (!row.runtimeEvidencePassed) {
      blockers.push(`runtime_evidence_missing:${row.scenarioId}`);
    }
    if (!row.finalDeliveryEvidencePassed) {
      blockers.push(`final_delivery_missing:${row.scenarioId}`);
    }
    if (!row.nonFakeEvidencePassed) {
      blockers.push(`non_fake_evidence_missing:${row.scenarioId}`);
    }
    if (row.legacyFallbackUsed) {
      blockers.push(`legacy_fallback:${row.scenarioId}`);
    }
    if (row.silentDurableWriteDetected) {
      blockers.push(`silent_write:${row.scenarioId}`);
    }
    if (row.fakeExecutionDetected) {
      blockers.push(`fake_execution:${row.scenarioId}`);
    }
  }

  return uniqueValues(blockers);
}

function stage1ObservedScenarioBlockers(
  observedScenarios: Stage1ObservedBrowserScenario[],
  gateReport: unknown
): string[] {
  const blockers: string[] = [];
  const report = gateReport as Partial<MainChatAgentStage1DogfoodReport> | null;
  const defaultRows =
    report?.scenarios?.filter(row => row.liveProviderEvidence === "default_deterministic") ?? [];
  const gateRowsById = new Map(defaultRows.map(row => [row.scenarioId, row]));

  if (observedScenarios.length !== STAGE1_REQUIRED_BROWSER_JOURNEYS.length) {
    blockers.push("observed_scenario_count_not_36");
  }

  const observedIds = observedScenarios.map(row => row.scenarioId);
  if (!arraysEqual(observedIds, STAGE1_REQUIRED_BROWSER_JOURNEYS)) {
    blockers.push("observed_scenario_ids_not_exact_d01_d36");
  }

  const duplicateIds = observedIds.filter((id, index) => observedIds.indexOf(id) !== index);
  for (const id of uniqueValues(duplicateIds)) blockers.push(`duplicate_observed_scenario:${id}`);

  for (const id of STAGE1_REQUIRED_BROWSER_JOURNEYS) {
    if (!observedIds.includes(id)) blockers.push(`missing_observed_scenario:${id}`);
  }

  for (const row of observedScenarios) {
    const gateRow = gateRowsById.get(row.scenarioId);
    if (!gateRow) blockers.push(`missing_gate_cross_check:${row.scenarioId}`);
    if (row.observedVia !== OBSERVED_VIA) {
      blockers.push(`scenario_not_real_tauri_observed:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.entryPoint)) blockers.push(`entry_point_unsafe:${row.scenarioId}`);
    if (!metadataSafeLabel(row.taskSessionId) || row.taskSessionId.startsWith("stage1_task_")) {
      blockers.push(`task_session_not_observed:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.runId) || row.runId.startsWith("stage1_run_")) {
      blockers.push(`run_not_observed:${row.scenarioId}`);
    }
    if (!metadataSafeLabel(row.routeStrategy)) blockers.push(`route_unsafe:${row.scenarioId}`);
    if (row.runtimeEvents.length === 0) blockers.push(`runtime_events_missing:${row.scenarioId}`);
    if (row.visibleUiStates.length === 0) blockers.push(`ui_state_missing:${row.scenarioId}`);
    if (row.finalDeliverySections.length === 0) {
      blockers.push(`final_delivery_missing:${row.scenarioId}`);
    }
    if (!row.runtimeEvidenceObserved) blockers.push(`runtime_not_observed:${row.scenarioId}`);
    if (!row.uiStateObserved) blockers.push(`ui_not_observed:${row.scenarioId}`);
    if (!row.finalDeliveryObserved) blockers.push(`final_delivery_not_observed:${row.scenarioId}`);
    if (!row.nonFakeEvidenceObserved) blockers.push(`non_fake_not_observed:${row.scenarioId}`);
    if (row.legacyFallbackUsed) blockers.push(`legacy_fallback:${row.scenarioId}`);
    if (row.silentDurableWriteDetected) blockers.push(`silent_write:${row.scenarioId}`);
    if (row.fakeExecutionDetected) blockers.push(`fake_execution:${row.scenarioId}`);
    if (gateRow?.expectedOutcome === "expected_blocker" && row.visibleBlockers.length === 0) {
      blockers.push(`expected_blocker_not_visible:${row.scenarioId}`);
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

function arraysEqual(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
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
