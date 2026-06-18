import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

import {
  STAGE1_NON_TAURI_BROWSER_BLOCKERS,
  STAGE1_REQUIRED_BROWSER_JOURNEYS,
  buildStage1TauriWebdriverPreflight,
  buildStage1BlockedBrowserEvidenceReport,
  buildStage1PassingBrowserEvidenceReportFromObservedScenarios,
  stage1NonTauriBrowserBlockersForPlatform,
  type Stage1ObservedBrowserScenario,
} from "./stage1BrowserEvidence";
import { STAGE1_DOGFOOD_SCENARIOS } from "./stage1DogfoodScenarios";

function baseGateReport(overrides: Record<string, unknown> = {}) {
  const scenarios = STAGE1_REQUIRED_BROWSER_JOURNEYS.map(id => ({
    scenarioId: id,
    entryPoint: isChatScenarioId(id)
      ? "ordinary_main_chat_input"
      : "seeded_visible_control_surface",
    routeStrategy: routeStrategyForTest(id),
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
    return STAGE1_REQUIRED_BROWSER_JOURNEYS.map(id => {
      const scenario = stage1DogfoodScenarioForTest(id);
      return {
        scenarioId: id,
        observedVia: "real_tauri_chat_or_control_path",
        entryPoint: isChatScenarioId(id)
          ? "ordinary_main_chat_input"
          : "seeded_visible_control_surface",
        taskSessionId: `real-task-${id}`,
        runId: `real-run-${id}`,
        routeStrategy: routeStrategyForTest(id),
        runtimeEvents: [
          "task_session.created",
          ...(isChatScenarioId(id)
            ? ["visible_control.chat_send"]
            : [seededVisibleControlEventForTest(id)]),
          "final_delivery.created",
        ],
        visibleUiStates: [...scenario.expectedUiStates],
        finalDeliverySections: [...scenario.expectedFinalSections],
        visibleBlockers: scenario.expectedBlocker ? [scenario.expectedBlocker] : [],
        runtimeEvidenceObserved: true,
        uiStateObserved: true,
        finalDeliveryObserved: true,
        nonFakeEvidenceObserved: true,
        legacyFallbackUsed: false,
        silentDurableWriteDetected: false,
        fakeExecutionDetected: false,
      };
    });
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

  it("rejects observed scenarios whose entry point does not match the gate row", () => {
    const observed = observedScenarios();
    observed[0] = {
      ...observed[0],
      entryPoint: "seeded_visible_control_surface",
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-entry-mismatch-test",
      })
    ).toThrow(/stage1_browser_observed_scenarios_incomplete/);
  });

  it("rejects observed scenarios that reuse too few browser runtime identities", () => {
    const observed = observedScenarios().map(row => ({
      ...row,
      taskSessionId: "reused-browser-task",
      runId: "reused-browser-run",
    }));

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-reused-runtime-id-test",
      })
    ).toThrow(/observed_task_session_distinct_count_below_20/);
  });

  it("rejects seeded task-control scenarios without observed visible-control evidence", () => {
    const observed = observedScenarios();
    const seededIndex = observed.findIndex(row => row.scenarioId === "D09");
    observed[seededIndex] = {
      ...observed[seededIndex],
      runtimeEvents: ["task_session.created", "final_delivery.created"],
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-missing-visible-control-test",
      })
    ).toThrow(/visible_control_not_observed:D09/);
  });

  it("rejects seeded task-control scenarios without the scenario-specific visible control event", () => {
    const observed = observedScenarios();
    const seededIndex = observed.findIndex(row => row.scenarioId === "D13");
    observed[seededIndex] = {
      ...observed[seededIndex],
      runtimeEvents: ["task_session.created", "visible_control.applied", "final_delivery.created"],
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-wrong-seeded-control-test",
      })
    ).toThrow(/seeded_control_event_not_observed:D13/);
  });

  it("rejects ordinary chat scenarios without observed composer send evidence", () => {
    const observed = observedScenarios();
    const chatIndex = observed.findIndex(row => row.scenarioId === "D01");
    observed[chatIndex] = {
      ...observed[chatIndex],
      runtimeEvents: observed[chatIndex].runtimeEvents.filter(
        event => event !== "visible_control.chat_send"
      ),
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-missing-chat-send-test",
      })
    ).toThrow(/chat_send_control_not_observed:D01/);
  });

  it("rejects seeded task-control scenarios with a generic synthetic route", () => {
    const observed = observedScenarios();
    const seededIndex = observed.findIndex(row => row.scenarioId === "D13");
    observed[seededIndex] = {
      ...observed[seededIndex],
      routeStrategy: "task_control",
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-generic-route-test",
      })
    ).toThrow(/generic_route_not_observed:D13/);
  });

  it("rejects observed scenarios whose route does not match the gate row", () => {
    const observed = observedScenarios();
    const scenarioIndex = observed.findIndex(row => row.scenarioId === "D02");
    observed[scenarioIndex] = {
      ...observed[scenarioIndex],
      routeStrategy: "DirectAnswer",
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-route-mismatch-test",
      })
    ).toThrow(/route_mismatch:D02/);
  });

  it("rejects observed scenarios missing a required visible UI state", () => {
    const observed = observedScenarios();
    const scenarioIndex = observed.findIndex(row => row.scenarioId === "D08");
    observed[scenarioIndex] = {
      ...observed[scenarioIndex],
      visibleUiStates: observed[scenarioIndex].visibleUiStates.filter(
        state => state !== "planning"
      ),
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-missing-ui-state-test",
      })
    ).toThrow(/required_ui_state_not_observed:D08:planning/);
  });

  it("rejects observed scenarios missing a required final delivery section", () => {
    const observed = observedScenarios();
    const scenarioIndex = observed.findIndex(row => row.scenarioId === "D08");
    observed[scenarioIndex] = {
      ...observed[scenarioIndex],
      finalDeliverySections: observed[scenarioIndex].finalDeliverySections.filter(
        section => section !== "next_action"
      ),
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-missing-final-section-test",
      })
    ).toThrow(/required_final_section_not_observed:D08:next_action/);
  });

  it("rejects contract-unsafe observed browser evidence labels", () => {
    const observed = observedScenarios();
    observed[0] = {
      ...observed[0],
      runtimeEvents: [
        ...observed[0].runtimeEvents,
        "assistant text says Actions and observations are complete",
      ],
      visibleUiStates: [...observed[0].visibleUiStates, "completed\nfrom assistant text"],
      finalDeliverySections: [...observed[0].finalDeliverySections, "Final delivery: complete"],
      visibleBlockers: [...observed[0].visibleBlockers, "permission required by assistant text"],
    };

    expect(() =>
      buildStage1PassingBrowserEvidenceReportFromObservedScenarios(observed, baseGateReport(), {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-unsafe-label-test",
      })
    ).toThrow(/runtime_event_unsafe:D01/);
  });

  it("keeps unavailable Tauri browser command surface honestly blocked", () => {
    const report = buildStage1BlockedBrowserEvidenceReport([...STAGE1_NON_TAURI_BROWSER_BLOCKERS], {
      now: new Date("2026-06-18T07:20:00.000Z"),
      runId: "stage1-browser-e2e-blocked-test",
    });

    expect(report.browserE2eEnvironmentReady).toBe(false);
    expect(report.smokePassed).toBe(false);
    expect(report.passedJourneys).toEqual([]);
    expect(report.failedJourneys).toEqual(STAGE1_REQUIRED_BROWSER_JOURNEYS);
    expect(report.observedScenarios).toEqual([]);
    expect(report.blockers).toContain("not_ready_browser_e2e_blocked");
    expect(report.blockers).toEqual([
      "not_ready_browser_e2e_blocked",
      ...STAGE1_NON_TAURI_BROWSER_BLOCKERS,
    ]);
  });

  it("normalizes blocked browser report blockers to metadata-safe labels", () => {
    const report = buildStage1BlockedBrowserEvidenceReport(
      [
        " raw blocker\nwith spaces and symbols!!! ",
        "real_tauri_browser_command_surface_unavailable",
      ],
      {
        now: new Date("2026-06-18T07:20:00.000Z"),
        runId: "stage1-browser-e2e-blocked-sanitize-test",
      }
    );

    expect(report.blockers).toContain("raw_blocker_with_spaces_and_symbols");
    expect(report.blockers.every(blocker => /^[A-Za-z0-9_.:/-]{1,160}$/.test(blocker))).toBe(true);
  });

  it("adds the official macOS Tauri WebDriver limitation only on Darwin", () => {
    expect(stage1NonTauriBrowserBlockersForPlatform("darwin")).toEqual([
      ...STAGE1_NON_TAURI_BROWSER_BLOCKERS,
      "tauri_webdriver_macos_not_supported_by_tauri_driver",
    ]);
    expect(stage1NonTauriBrowserBlockersForPlatform("linux")).toEqual([
      ...STAGE1_NON_TAURI_BROWSER_BLOCKERS,
    ]);
    expect(stage1NonTauriBrowserBlockersForPlatform("win32")).toEqual([
      ...STAGE1_NON_TAURI_BROWSER_BLOCKERS,
    ]);
  });

  it("preflights supported Tauri WebDriver runner prerequisites without minting pass evidence", () => {
    expect(
      buildStage1TauriWebdriverPreflight({
        platform: "linux",
        tauriDriverAvailable: true,
        nativeWebdriverAvailable: true,
        appBinaryAvailable: true,
      })
    ).toEqual({
      ready: true,
      supportedPlatform: true,
      blockers: [],
    });

    expect(
      buildStage1TauriWebdriverPreflight({
        platform: "linux",
        tauriDriverAvailable: false,
        nativeWebdriverAvailable: false,
        appBinaryAvailable: false,
      })
    ).toEqual({
      ready: false,
      supportedPlatform: true,
      blockers: [
        "tauri_driver_binary_missing",
        "native_webdriver_binary_missing",
        "tauri_debug_app_binary_missing",
      ],
    });

    expect(
      buildStage1TauriWebdriverPreflight({
        platform: "darwin",
        tauriDriverAvailable: true,
        nativeWebdriverAvailable: true,
        appBinaryAvailable: true,
      })
    ).toEqual({
      ready: false,
      supportedPlatform: false,
      blockers: ["tauri_webdriver_macos_not_supported_by_tauri_driver"],
    });
  });

  it("exports the reusable D01-D36 browser journey matrix for supported Tauri WebDriver runners", () => {
    expect(STAGE1_DOGFOOD_SCENARIOS.map(scenario => scenario.id)).toEqual(
      STAGE1_REQUIRED_BROWSER_JOURNEYS
    );
    expect(
      STAGE1_DOGFOOD_SCENARIOS.filter(scenario => scenario.scenarioType === "chat_e2e")
    ).toHaveLength(24);
    expect(
      STAGE1_DOGFOOD_SCENARIOS.filter(
        scenario => scenario.scenarioType === "seeded_task_control_e2e"
      )
    ).toHaveLength(12);
    expect(STAGE1_DOGFOOD_SCENARIOS.some(scenario => scenario.selectedSkillId)).toBe(true);
    expect(STAGE1_DOGFOOD_SCENARIOS.some(scenario => scenario.expectedBlocker)).toBe(true);
  });

  it("keeps Playwright dogfood on the shared Stage 1 scenario matrix", () => {
    const spec = fs.readFileSync(
      path.resolve(process.cwd(), "e2e/main-chat-stage1-dogfood.spec.ts"),
      "utf8"
    );

    expect(spec).toContain("STAGE1_DOGFOOD_SCENARIOS");
    expect(spec).toContain("stage1_browser_prep_task_id_unsafe");
    expect(spec).toContain("metadataSafeLabel(taskSessionId)");
    expect(spec).toContain("stage1_selected_skill_input_missing");
    expect(spec).toContain("stage1_selected_skill_not_applied");
    expect(spec).toContain('getAttribute("aria-label")');
    expect(spec).toContain('getAttribute("title")');
    expect(spec).not.toContain("const SCENARIOS:");
    expect(spec).not.toContain("function s(");
  });

  it("exposes a checked-in Tauri WebDriver runner entrypoint for supported platforms", () => {
    const packageJson = JSON.parse(
      fs.readFileSync(path.resolve(process.cwd(), "package.json"), "utf8")
    );
    const script = fs.readFileSync(
      path.resolve(process.cwd(), "scripts/stage1-tauri-webdriver.mjs"),
      "utf8"
    );

    expect(packageJson.scripts["test:e2e:tauri"]).toBe("node scripts/stage1-tauri-webdriver.mjs");
    expect(fs.existsSync(path.resolve(process.cwd(), "scripts/stage1-tauri-webdriver.mjs"))).toBe(
      true
    );
    expect(script).not.toContain("tauri_webdriver_d01_d36_executor_not_implemented");
    expect(script).toContain("'tauri:options'");
    expect(script).toContain("browserName: 'wry'");
    expect(script).toContain('const webdriverUrl = "http://127.0.0.1:4444"');
    expect(script).toContain('"/session"');
    expect(script).toContain("stage1DogfoodScenarios.ts");
    expect(script).toContain("STAGE1_DOGFOOD_SCENARIOS");
    expect(script).toContain("prepare_main_chat_agent_stage1_browser_dogfood_state");
    expect(script).toContain("validateStage1BrowserPrepReport");
    expect(script).toContain("directWritesExecuted");
    expect(script).toContain("durableLifemodelWritesExecuted");
    expect(script).toContain("fileOrExternalWritesExecuted");
    expect(script).toContain("tauri_webdriver_stage1_prep_missing_seeded_task");
    expect(script).toContain("tauri_webdriver_stage1_prep_task_id_unsafe");
    expect(script).toContain("metadataSafeLabel(taskSessionId)");
    expect(script).toContain("run_main_chat_agent_stage1_dogfood_gate");
    expect(script).toContain("executeChatScenarioWithWebDriver");
    expect(script).toContain("executeSeededControlScenarioWithWebDriver");
    expect(script).toContain("chat-input");
    expect(script).toContain("send-button");
    expect(script).toContain("webdriver_selected_skill_not_applied");
    expect(script).toContain("requestedSkillId.trim()");
    expect(script).toContain(
      'Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")'
    );
    expect(script).toContain(
      'Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")'
    );
    expect(script).toContain("visible_control.task_continuity_detail_opened");
    expect(script).toContain("getAttribute('aria-label')");
    expect(script).toContain("getAttribute('title')");
    expect(script).not.toContain("tauri_webdriver_seeded_control_observation_not_completed");
    expect(script).toContain("expectedUiStates");
    expect(script).toContain("expectedFinalSections");
    expect(script).toContain("get_main_chat_agent_state_snapshot");
    expect(script).toContain("get_main_chat_agent_task_detail");
    expect(script).toContain("list_main_chat_agent_events");
    expect(script).toContain("uiStateObserved(");
    expect(script).toContain("finalSectionObserved(");
    expect(script).toContain("visibleBlockersForScenario(");
    expect(script).not.toContain("visibleUiStates: []");
    expect(script).not.toContain("uiStateObserved: false");
    expect(script).not.toContain("finalDeliveryObserved: false");
    expect(script).not.toContain("nonFakeEvidenceObserved: false");
    expect(script).toContain("validateObservedScenariosForPassingReport");
    expect(script).toContain("tauri_webdriver_entry_point_mismatch");
    expect(script).toContain("tauri_webdriver_route_mismatch");
    expect(script).toContain("tauri_webdriver_runtime_event_unsafe");
    expect(script).toContain("tauri_webdriver_visible_ui_state_unsafe");
    expect(script).toContain("tauri_webdriver_final_delivery_section_unsafe");
    expect(script).toContain("tauri_webdriver_visible_blocker_unsafe");
    expect(script).toContain("metadataSafeLabel(");
    expect(script).toContain("stage1_task_");
    expect(script).toContain("stage1_run_");
    expect(script).toContain("tauri_webdriver_observed_task_session_distinct_count_below_20");
    expect(script).toContain("tauri_webdriver_observed_run_distinct_count_below_20");
    expect(script).toContain("seededVisibleControlEventPrefixes");
    expect(script).toContain("seededVisibleControlEventMatchesPrefix");
    expect(script).toContain("writePassingReport");
    expect(script).toContain("assertFinalStage1GateReadyWithBrowserEvidence");
    expect(script).toContain("readinessRecommendation");
    expect(script).toContain("ready_for_engineering_dogfood");
    expect(script).toContain("browserE2ePassedJourneyCount");
    expect(script).toContain("tauri_webdriver_final_gate_rejected");
    expect(script).toContain("finalGateBlockerFromError");
    expect(script).toContain("tauri_command_surface_browser_observed");
    expect(script).toContain("smokePassed: true");
    expect(script).toContain("normalizeBlockers(");
    expect(script).toContain("startFrontendDevServer");
    expect(script).toContain("waitForFrontendDevServer");
    expect(script).toContain('const frontendDevUrl = "http://127.0.0.1:5173"');
    expect(script).toContain('"corepack"');
    expect(script).toContain('"pnpm", "dev", "--", "--host", "127.0.0.1", "--port", "5173"');
    expect(script).toContain("frontendDevServer.kill()");
    expect(script).toContain("fileURLToPath(import.meta.url)");
    expect(script).toContain('const repoRoot = path.resolve(frontendRoot, "..")');
    expect(script).toContain("cwd: frontendRoot");
    expect(script).toContain('path.resolve(repoRoot, "frontend", "test-results"');
    expect(script).not.toContain(
      'path.resolve(process.cwd(), "test-results/main-chat-stage1-dogfood-report.json")'
    );
  });

  it("exposes a Linux CI path for real Tauri WebDriver D01-D36 dogfood", () => {
    const workflowPath = path.resolve(
      process.cwd(),
      "..",
      ".github",
      "workflows",
      "stage1-tauri-dogfood.yml"
    );
    const workflow = fs.readFileSync(workflowPath, "utf8");

    expect(workflow).toContain("runs-on: ubuntu-22.04");
    expect(workflow).toContain("webkit2gtk-driver");
    expect(workflow).toContain("xvfb");
    expect(workflow).toContain("cargo install tauri-driver --locked");
    expect(workflow).toContain("cargo build -p openlife-tauri");
    expect(workflow).toContain("pnpm --dir frontend test:e2e:tauri");
    expect(workflow).toContain(
      "cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture"
    );
    expect(workflow).toContain(
      "cargo test -p openlife-tauri run_main_chat_agent_stage1_dogfood_command_returns_isolated_report -- --nocapture"
    );
    expect(workflow).toContain("frontend/test-results/main-chat-stage1-dogfood-report.json");
    expect(workflow).toContain("actions/upload-artifact");
    expect(workflow).not.toContain("macos-latest");
    expect(workflow).not.toContain("tauri_command_surface_browser_observed: true");
  });

  it("keeps browser observation helpers from using broad assistant text as evidence", () => {
    const files = [
      path.resolve(process.cwd(), "scripts/stage1-tauri-webdriver.mjs"),
      path.resolve(process.cwd(), "e2e/main-chat-stage1-dogfood.spec.ts"),
    ];
    for (const file of files) {
      const source = fs.readFileSync(file, "utf8");
      expect(source).not.toContain('text.includes("Actions")');
      expect(source).not.toContain('text.includes("Observations")');
      expect(source).not.toContain('text.includes("Sources used")');
      expect(source).not.toContain("/Final delivery|completed/i.test(text)");
      expect(source).not.toContain("/permission|pending user/i.test(text)");
      expect(source).not.toContain("/blocked|stale/.test");
      expect(source).not.toContain("/memory|proposal/i.test(text)");
      expect(source).not.toContain("/Retry/i.test(text)");
      expect(source).not.toContain("/replay|refresh/.test(text)");
      expect(source).not.toContain("/Proposals/i.test(text)");
      expect(source).not.toContain("/Blockers|blocked/i.test(text)");
    }
  });
});

function isChatScenarioId(id: string): boolean {
  return new Set([
    "D01",
    "D02",
    "D03",
    "D04",
    "D05",
    "D06",
    "D07",
    "D08",
    "D10",
    "D16",
    "D17",
    "D18",
    "D21",
    "D22",
    "D23",
    "D24",
    "D25",
    "D26",
    "D29",
    "D30",
    "D31",
    "D32",
    "D33",
    "D34",
  ]).has(id);
}

function seededVisibleControlEventForTest(id: string): string {
  if (id === "D09") return "visible_control.skip_step_seeded_plan_step";
  if (id === "D11") return "visible_control.accept_proposal";
  if (id === "D12") return "visible_control.rollback_memory";
  if (id === "D13") return "visible_control.resume_task_from_continuity_detail";
  if (id === "D14") return "visible_control.retry_task_action";
  if (id === "D15") return "visible_control.cancel_task_from_continuity_detail";
  if (id === "D19") return "visible_control.task_continuity_detail_opened";
  if (id === "D20") return "visible_control.task_continuity_detail_opened";
  if (id === "D27") return "visible_control.refresh_task_context";
  if (id === "D28") return "visible_control.task_continuity_detail_opened";
  if (id === "D35") return "visible_control.deny";
  if (id === "D36") return "visible_control.defer";
  throw new Error(`unexpected seeded stage1 scenario: ${id}`);
}

function routeStrategyForTest(id: string): string {
  const scenario = stage1DogfoodScenarioForTest(id);
  if (id === "D01") return "DirectAnswer";
  if (id === "D02") return "read_action:file.read";
  if (id === "D03") return "session.search";
  if (id === "D04") return "DirectAnswer:memory_context";
  if (id === "D05") return "ReAct:web_fixture";
  if (id === "D06") return "selected_skill_context";
  if (id === "D07") return "ReAct:MCP_read";
  if (id === "D08") return "Plan-Execute";
  if (id === "D09") return "plan_control";
  if (id === "D10") return "memory_proposal";
  if (id === "D11") return "proposal_control";
  if (id === "D12") return "memory_rollback";
  if (id === "D13") return "task_resume_control";
  if (id === "D14") return "retry_action_control";
  if (id === "D15") return "cancel_task_control";
  if (id === "D16") return "permission_blocker";
  if (id === "D17") return "ReAct:tool_trace";
  if (id === "D18") return "blocked:unselected_skill";
  if (id === "D19") return "final_delivery_read";
  if (id === "D20") return "event_replay";
  if (id === "D21") return "memory_conflict";
  if (id === "D22") return "multi_read_ReAct";
  if (id === "D23") return "web_blocker";
  if (id === "D24") return "MCP_blocker";
  if (id === "D25") return "context_inspection";
  if (id === "D26") return "knowledge_proposal";
  if (id === "D27") return "stale_blocker";
  if (id === "D28") return "final_delivery_read";
  if (id === "D29") return "DirectAnswer";
  if (id === "D30") return "read_plus_proposal";
  if (id === "D31") return "Plan-Execute_blocker";
  if (id === "D32") return "skill_plus_file_read";
  if (id === "D33") return "session_plus_memory";
  if (id === "D34") return "knowledge_proposal";
  if (id === "D35") return "permission_control";
  if (id === "D36") return "proposal_control";
  throw new Error(`missing route for stage1 dogfood scenario: ${scenario.id}`);
}

function stage1DogfoodScenarioForTest(id: string) {
  const scenario = STAGE1_DOGFOOD_SCENARIOS.find(item => item.id === id);
  if (!scenario) throw new Error(`missing stage1 dogfood scenario: ${id}`);
  return scenario;
}
