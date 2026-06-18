import { describe, expect, it } from "vitest";

import {
  STAGE1_REQUIRED_BROWSER_JOURNEYS,
  buildStage1BlockedBrowserEvidenceReport,
  buildStage1PassingBrowserEvidenceReportFromObservedScenarios,
  type Stage1ObservedBrowserScenario,
} from "./stage1BrowserEvidence";

function baseGateReport(overrides: Record<string, unknown> = {}) {
  const scenarios = STAGE1_REQUIRED_BROWSER_JOURNEYS.map(id => ({
    scenarioId: id,
    liveProviderEvidence: "default_deterministic",
    taskSessionId: `real-task-${id}`,
    runId: `real-run-${id}`,
    runtimeEvidencePassed: true,
    finalDeliveryEvidencePassed: true,
    nonFakeEvidencePassed: true,
    legacyFallbackUsed: false,
    silentDurableWriteDetected: false,
    fakeExecutionDetected: false,
  }));

  return {
    reportKind: "main_chat_agent_stage1_dogfood_gate",
    defaultScenarioCount: 36,
    taskSessionCreatedCount: 36,
    ordinaryChatScenarioCount: 24,
    seededTaskControlScenarioCount: 12,
    finalDeliveryVerifiedScenarioCount: 36,
    legacyFallbackCount: 0,
    silentDurableWriteCount: 0,
    fakeExecutionDetectedCount: 0,
    scenarios,
    ...overrides,
  };
}

describe("stage1 browser evidence report builder", () => {
  function observedScenarios(): Stage1ObservedBrowserScenario[] {
    return STAGE1_REQUIRED_BROWSER_JOURNEYS.map(id => ({
      scenarioId: id,
      observedVia: "real_tauri_chat_or_control_path",
      entryPoint: Number(id.slice(1)) <= 8 ? "ordinary_main_chat_input" : "observed_user_path",
      taskSessionId: `real-task-${id}`,
      runId: `real-run-${id}`,
      routeStrategy: "observed_route",
      runtimeEvents: ["task_session.created", "final_delivery.created"],
      visibleUiStates: ["Agent Control Plane", "completed"],
      finalDeliverySections: ["completed_work"],
      visibleBlockers: [],
      runtimeEvidenceObserved: true,
      uiStateObserved: true,
      finalDeliveryObserved: true,
      nonFakeEvidenceObserved: true,
      legacyFallbackUsed: false,
      silentDurableWriteDetected: false,
      fakeExecutionDetected: false,
    }));
  }

  it("builds passing browser evidence only from observed per-scenario Chat/Tauri state", () => {
    const report = buildStage1PassingBrowserEvidenceReportFromObservedScenarios(
      observedScenarios(),
      baseGateReport(),
      {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-real-test",
      }
    );

    expect(report.smokePassed).toBe(true);
    expect(report.evidenceSource).toBe("tauri_command_surface_browser_observed");
    expect(report.requiredJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
    expect(report.passedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
    expect(report.failedJourneys).toEqual([]);
    expect(report.observedScenarios).toHaveLength(36);
    expect(report.blockers).toEqual([]);
    expect(report.reportDigest).toMatch(/^bytes:[1-9][0-9]* hash:sha256:[a-f0-9]{64}$/);
  });

  it("rejects aggregate gate evidence as browser pass evidence", () => {
    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios([], baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-aggregate-only-test",
      })
    ).toThrow(/stage1_browser_observed_scenarios_incomplete/);
  });

  it("rejects incomplete gate evidence instead of minting a passing report", () => {
    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(
        observedScenarios(),
        baseGateReport({ taskSessionCreatedCount: 35 }),
        {
          now: new Date("2026-06-18T07:20:00.000Z"),
          runId: "stage1-browser-e2e-incomplete-test",
        }
      )
    ).toThrow(/stage1_browser_gate_runtime_evidence_incomplete/);
  });

  it("keeps unavailable Tauri browser command surface honestly blocked", () => {
    const report = buildStage1BlockedBrowserEvidenceReport(
      ["real_tauri_browser_command_surface_unavailable"],
      {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-blocked-test",
      }
    );

    expect(report.browserE2eEnvironmentReady).toBe(false);
    expect(report.smokePassed).toBe(false);
    expect(report.passedJourneys).toEqual([]);
    expect(report.failedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
    expect(report.observedScenarios).toEqual([]);
    expect(report.blockers).toContain("not_ready_browser_e2e_blocked");
  });
});
