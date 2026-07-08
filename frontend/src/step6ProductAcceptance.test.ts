import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

import {
  STEP6_PRODUCT_ACCEPTANCE_JOURNEYS,
  STEP6_BLOCKED_LIVE_UI_STATUS,
  STEP6_PRODUCT_ACCEPTANCE_BLOCKED_SOURCE,
  STEP6_PRODUCT_ACCEPTANCE_OBSERVED_SOURCE,
  STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS,
  STEP6_PRODUCT_ACCEPTANCE_REPORT_PATH,
  STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION,
  STEP6_REQUIRED_PRODUCT_JOURNEYS,
  buildStep6BlockedProductAcceptanceReport,
  buildStep6ProductAcceptanceReportFromObservedJourneys,
  step6ObservedJourneyBlockers,
  step6JourneyPassed,
  step6ReportDigest,
  type Step6ObservedProductJourney,
} from "./test/archive/step6ProductAcceptance";

function observedJourneys(input: {
  liveStatus: Step6ObservedProductJourney["externalLiveStatus"];
  liveObservedVia: Step6ObservedProductJourney["observedVia"];
  liveProviderKind: string | null;
  liveBlockers: string[];
}): Step6ObservedProductJourney[] {
  return STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.map((journey, index) => {
    const live = journey.kind === "external_live";
    const seededControl = journey.id === "S6-PERMISSION" || journey.id === "S6-RECOVERY";
    return {
      journeyId: journey.id,
      kind: journey.kind,
      observedVia: live ? input.liveObservedVia : "real_tauri_chat_or_control_path",
      entryPoint:
        live && input.liveStatus === "blocked_live_evidence"
          ? "blocked_live_evidence_report"
          : seededControl
            ? "task_continuity_control"
            : "ordinary_main_chat_input",
      routeStrategy:
        live && input.liveStatus === "blocked_live_evidence"
          ? "blocked_external_live"
          : seededControl
            ? "task_continuity_control"
            : live
              ? "external_live_provider"
              : "main_chat_kernel",
      taskSessionId:
        live && input.liveStatus === "blocked_live_evidence" ? "" : `real-task-${journey.id}`,
      runId: live && input.liveStatus === "blocked_live_evidence" ? "" : `real-run-${journey.id}`,
      answerEvidence: [...journey.expectedAnswerEvidence],
      runtimeEvidence: [...journey.expectedRuntimeEvidence],
      uiStatusEvidence:
        live && input.liveStatus === "blocked_live_evidence"
          ? [STEP6_BLOCKED_LIVE_UI_STATUS]
          : [journey.expectedUiStatus[0]],
      finalDeliverySections:
        live && input.liveStatus === "blocked_live_evidence"
          ? []
          : [journey.expectedFinalDeliverySections[0]],
      traceEvidence: [`trace.step6.${index + 1}`],
      noInventedUnavailableEvidence: true,
      unavailableEvidenceInvented: false,
      legacyFallbackUsed: false,
      silentDurableWriteDetected: false,
      localFixtureCreditedAsExternalLive: false,
      externalLiveStatus: live ? input.liveStatus : "not_applicable",
      externalLiveProviderKind: live ? input.liveProviderKind : null,
      blockers: live && input.liveStatus === "blocked_live_evidence" ? [...input.liveBlockers] : [],
    };
  });
}

describe("Step6 product acceptance evidence", () => {
  it("defines the 11 required user-level journeys and report path", () => {
    expect(STEP6_REQUIRED_PRODUCT_JOURNEYS).toEqual([
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
    ]);
    expect(STEP6_PRODUCT_ACCEPTANCE_JOURNEYS).toHaveLength(11);
    expect(STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION).toBe("step6-product-acceptance-v1");
    expect(STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS).toBe(
      "step6_local_deterministic_required_external_live_opt_in_separate"
    );
    expect(STEP6_PRODUCT_ACCEPTANCE_OBSERVED_SOURCE).toBe(
      "tauri_command_surface_step6_browser_observed"
    );
    expect(STEP6_PRODUCT_ACCEPTANCE_BLOCKED_SOURCE).toBe("tauri_command_surface_unavailable");
    expect(STEP6_PRODUCT_ACCEPTANCE_REPORT_PATH).toBe(
      "frontend/test-results/main-chat-step6-product-acceptance-report.json"
    );
  });

  it("builds a ready report only when local and external live journeys have credit", () => {
    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(
      observedJourneys({
        liveStatus: "credited_external_live",
        liveObservedVia: "real_tauri_chat_or_control_path",
        liveProviderKind: "external_provider",
        liveBlockers: [],
      }),
      {
        now: new Date("2026-06-26T10:30:00.000Z"),
        runId: "step6-browser-e2e-real-test",
      }
    );

    expect(report.requiredJourneys).toEqual(STEP6_REQUIRED_PRODUCT_JOURNEYS);
    expect(report.schemaVersion).toBe(STEP6_PRODUCT_ACCEPTANCE_SCHEMA_VERSION);
    expect(report.readinessSemantics).toBe(STEP6_PRODUCT_ACCEPTANCE_READINESS_SEMANTICS);
    expect(report.smokePassed).toBe(true);
    expect(report.evidenceSource).toBe(STEP6_PRODUCT_ACCEPTANCE_OBSERVED_SOURCE);
    expect(report.localDeterministicReady).toBe(true);
    expect(report.externalLiveReady).toBe(true);
    expect(report.acceptanceReady).toBe(true);
    expect(report.blockers).toEqual([]);
    expect(report.reportDigest).toMatch(/^bytes:[1-9][0-9]* hash:sha256:[a-f0-9]{64}$/);
  });

  it("covers report-level readiness and safety claims in the report digest", () => {
    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(
      observedJourneys({
        liveStatus: "credited_external_live",
        liveObservedVia: "real_tauri_chat_or_control_path",
        liveProviderKind: "external_provider",
        liveBlockers: [],
      }),
      {
        now: new Date("2026-06-26T10:30:00.000Z"),
        runId: "step6-browser-e2e-digest-summary-test",
      }
    );

    for (const field of [
      "e2eEnvironmentReady",
      "selfContainedRunner",
      "smokePassed",
      "localDeterministicReady",
      "externalLiveReady",
      "acceptanceReady",
      "noSilentDurableWrite",
      "noHiddenLegacyFallback",
      "noLocalEvidenceCreditedAsExternalLive",
      "noInventedUnavailableEvidence",
      "uiStatusFromStructuredEvidence",
    ] as const) {
      expect(step6ReportDigest({ ...report, [field]: !report[field] })).not.toBe(
        report.reportDigest
      );
    }
    expect(step6ReportDigest({ ...report, readinessSemantics: "tampered" as any })).not.toBe(
      report.reportDigest
    );
    expect(
      step6ReportDigest({ ...report, reportPath: "frontend/test-results/tampered.json" })
    ).not.toBe(report.reportDigest);
    expect(step6ReportDigest({ ...report, localJourneyCount: 0 })).not.toBe(report.reportDigest);
    expect(
      step6ReportDigest({ ...report, externalLiveBlockers: ["S6-LIVE-WEB:tampered"] })
    ).not.toBe(report.reportDigest);
    expect(
      step6ReportDigest({
        ...report,
        observedJourneys: report.observedJourneys.map((row, index) =>
          index === 0 ? { ...row, noInventedUnavailableEvidence: false } : row
        ),
      })
    ).not.toBe(report.reportDigest);
  });

  it("keeps unavailable external live journeys explicitly blocked instead of minting credit", () => {
    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(
      observedJourneys({
        liveStatus: "blocked_live_evidence",
        liveObservedVia: "blocked_live_evidence_report",
        liveProviderKind: null,
        liveBlockers: ["explicit_live_eval_required"],
      }),
      {
        now: new Date("2026-06-26T10:30:00.000Z"),
        runId: "step6-browser-e2e-live-blocked-test",
      }
    );

    expect(report.localDeterministicReady).toBe(true);
    expect(report.externalLiveReady).toBe(false);
    expect(report.acceptanceReady).toBe(false);
    expect(report.blockedLiveJourneys).toEqual(["S6-LIVE-WEB", "S6-LIVE-MCP"]);
    for (const row of report.observedJourneys.filter(row => row.kind === "external_live")) {
      expect(row.uiStatusEvidence).toEqual([STEP6_BLOCKED_LIVE_UI_STATUS]);
    }
    expect(report.failedJourneys).toEqual([]);
    expect(report.externalLiveBlockers).toEqual([
      "S6-LIVE-WEB:explicit_live_eval_required",
      "S6-LIVE-MCP:explicit_live_eval_required",
    ]);
    expect(report.blockers).toContain("step6_external_live_evidence_blocked_or_incomplete");
  });

  it("rejects local fixture or loopback evidence claimed as external live proof", () => {
    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(
      observedJourneys({
        liveStatus: "credited_external_live",
        liveObservedVia: "real_tauri_chat_or_control_path",
        liveProviderKind: "local_test_http",
        liveBlockers: [],
      }),
      {
        now: new Date("2026-06-26T10:30:00.000Z"),
        runId: "step6-browser-e2e-fake-live-test",
      }
    );

    expect(report.externalLiveReady).toBe(false);
    expect(report.noLocalEvidenceCreditedAsExternalLive).toBe(false);
    expect(report.blockers).toContain("step6_external_provider_missing:S6-LIVE-WEB");
    expect(report.blockers).toContain("step6_external_provider_missing:S6-LIVE-MCP");
  });

  it("rejects screenshot-only or prose-derived evidence with missing structured fields", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    observed[0] = {
      ...observed[0],
      runtimeEvidence: ["assistant prose says completed"],
      uiStatusEvidence: ["completed from screenshot"],
      finalDeliverySections: [],
    };

    expect(step6ObservedJourneyBlockers(observed)).toEqual(
      expect.arrayContaining([
        "step6_runtime_evidence_unsafe:S6-CLOCK",
        "step6_ui_status_unsafe:S6-CLOCK",
        "step6_runtime_evidence_missing:S6-CLOCK:source.runtime_fact",
        "step6_ui_status_missing:S6-CLOCK",
        "step6_final_delivery_missing:S6-CLOCK",
      ])
    );
  });

  it("rejects non-blocked journeys that reuse the same runtime identity", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    observed[1] = {
      ...observed[1],
      taskSessionId: observed[0].taskSessionId,
      runId: observed[0].runId,
    };

    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(observed, {
      now: new Date("2026-06-26T10:30:00.000Z"),
      runId: "step6-browser-e2e-duplicate-runtime-test",
    });

    expect(report.acceptanceReady).toBe(false);
    expect(report.blockers).toContain("step6_observed_task_session_ids_not_distinct");
    expect(report.blockers).toContain("step6_observed_run_ids_not_distinct");
  });

  it("rejects wrong entry point or hidden fallback route before Rust ingestion", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    const clock = observed.find(row => row.journeyId === "S6-CLOCK")!;
    clock.entryPoint = "legacy_strategy_adapter";
    clock.routeStrategy = "legacy_fallback";
    const recovery = observed.find(row => row.journeyId === "S6-RECOVERY")!;
    recovery.entryPoint = "ordinary_main_chat_input";

    expect(step6ObservedJourneyBlockers(observed)).toEqual(
      expect.arrayContaining([
        "step6_entry_point_mismatch:S6-CLOCK",
        "step6_route_legacy_or_fallback:S6-CLOCK",
        "step6_entry_point_mismatch:S6-RECOVERY",
      ])
    );
    expect(step6JourneyPassed(clock)).toBe(false);
    expect(step6JourneyPassed(recovery)).toBe(false);

    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(observed, {
      now: new Date("2026-06-26T10:30:00.000Z"),
      runId: "step6-browser-e2e-entry-route-test",
    });
    expect(report.passedJourneys).not.toContain("S6-CLOCK");
    expect(report.passedJourneys).not.toContain("S6-RECOVERY");
  });

  it("rejects generic final delivery that does not match the journey matrix", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    const proposal = observed.find(row => row.journeyId === "S6-PROPOSAL")!;
    proposal.finalDeliverySections = ["completed_work"];

    expect(step6ObservedJourneyBlockers(observed)).toEqual(
      expect.arrayContaining(["step6_final_delivery_section_missing:S6-PROPOSAL"])
    );
    expect(step6JourneyPassed(proposal)).toBe(false);
  });

  it("rejects blocked external live rows without structured blocked UI status", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    for (const row of observed.filter(row => row.kind === "external_live")) {
      row.uiStatusEvidence = [];
    }

    expect(step6ObservedJourneyBlockers(observed)).toEqual(
      expect.arrayContaining([
        "step6_blocked_live_ui_status_missing:S6-LIVE-WEB",
        "step6_blocked_live_ui_status_missing:S6-LIVE-MCP",
      ])
    );
  });

  it("rejects blocked external live rows that still report fallback or silent writes", () => {
    const observed = observedJourneys({
      liveStatus: "blocked_live_evidence",
      liveObservedVia: "blocked_live_evidence_report",
      liveProviderKind: null,
      liveBlockers: ["network_disabled"],
    });
    const liveWeb = observed.find(row => row.journeyId === "S6-LIVE-WEB")!;
    const liveMcp = observed.find(row => row.journeyId === "S6-LIVE-MCP")!;
    liveWeb.legacyFallbackUsed = true;
    liveMcp.silentDurableWriteDetected = true;

    const report = buildStep6ProductAcceptanceReportFromObservedJourneys(observed, {
      now: new Date("2026-06-26T10:30:00.000Z"),
      runId: "step6-browser-e2e-blocked-live-safety-test",
    });

    expect(report.noHiddenLegacyFallback).toBe(false);
    expect(report.noSilentDurableWrite).toBe(false);
    expect(report.blockers).toEqual(
      expect.arrayContaining([
        "step6_legacy_fallback:S6-LIVE-WEB",
        "step6_silent_write:S6-LIVE-MCP",
      ])
    );
  });

  it("keeps a non-Tauri browser run as a machine-readable blocked report", () => {
    const report = buildStep6BlockedProductAcceptanceReport(
      ["real_tauri_browser_command_surface_unavailable"],
      {
        now: new Date("2026-06-26T10:30:00.000Z"),
        runId: "step6-browser-e2e-blocked-test",
      }
    );

    expect(report.e2eEnvironmentReady).toBe(false);
    expect(report.evidenceSource).toBe(STEP6_PRODUCT_ACCEPTANCE_BLOCKED_SOURCE);
    expect(report.acceptanceReady).toBe(false);
    expect(report.passedJourneys).toEqual([]);
    expect(report.observedJourneys.map(row => row.journeyId)).toEqual([
      "S6-LIVE-WEB",
      "S6-LIVE-MCP",
    ]);
    for (const row of report.observedJourneys) {
      expect(row.uiStatusEvidence).toEqual([STEP6_BLOCKED_LIVE_UI_STATUS]);
    }
    expect(report.failedJourneys).toEqual(
      STEP6_REQUIRED_PRODUCT_JOURNEYS.filter(id => !id.startsWith("S6-LIVE-"))
    );
    expect(report.externalLiveBlockers).toEqual([
      "S6-LIVE-WEB:step6_product_acceptance_e2e_blocked",
      "S6-LIVE-WEB:real_tauri_browser_command_surface_unavailable",
      "S6-LIVE-MCP:step6_product_acceptance_e2e_blocked",
      "S6-LIVE-MCP:real_tauri_browser_command_surface_unavailable",
    ]);
    expect(report.blockers).toContain("step6_product_acceptance_e2e_blocked");
    expect(report.blockers).toContain("real_tauri_browser_command_surface_unavailable");
  });

  it("exposes a Step 6 Tauri WebDriver entry point with the same journey matrix", () => {
    const packageJson = JSON.parse(
      fs.readFileSync(path.resolve(process.cwd(), "package.json"), "utf8")
    );
    const scriptPath = path.resolve(process.cwd(), "scripts/step6-tauri-webdriver.mjs");
    const script = fs.readFileSync(scriptPath, "utf8");
    const playwrightSpec = fs.readFileSync(
      path.resolve(process.cwd(), "e2e/main-chat-step6-product-acceptance.spec.ts"),
      "utf8"
    );
    const result = spawnSync(process.execPath, [scriptPath, "--validate-journeys-only"], {
      cwd: process.cwd(),
      encoding: "utf8",
    });
    const observedRuleResult = spawnSync(
      process.execPath,
      [scriptPath, "--validate-observed-rules-only"],
      {
        cwd: process.cwd(),
        encoding: "utf8",
      }
    );

    expect(packageJson.scripts["test:e2e:tauri:step6"]).toBe(
      "node scripts/step6-tauri-webdriver.mjs"
    );
    expect(packageJson.scripts["test:e2e:tauri:step6:local"]).toBe(
      "node scripts/step6-tauri-webdriver.mjs --allow-blocked-live"
    );
    expect(result.status).toBe(0);
    expect(`${result.stdout}${result.stderr}`).toContain(
      "validated_step6_product_acceptance_journeys=11"
    );
    expect(observedRuleResult.status).toBe(0);
    expect(`${observedRuleResult.stdout}${observedRuleResult.stderr}`).toContain(
      "validated_step6_observed_rule_fixtures=ok"
    );
    const contractLine = result.stdout
      .split(/\r?\n/)
      .find(line => line.startsWith("validated_step6_product_acceptance_contract="));
    expect(contractLine).toBeTruthy();
    const contract = JSON.parse(
      contractLine!.replace("validated_step6_product_acceptance_contract=", "")
    );
    expect(contract).toEqual(
      STEP6_PRODUCT_ACCEPTANCE_JOURNEYS.map(journey => ({
        id: journey.id,
        kind: journey.kind,
        executionMode:
          journey.id === "S6-PERMISSION" || journey.id === "S6-RECOVERY"
            ? "seeded_control"
            : "chat",
        prompt: journey.prompt,
        expectedAnswerEvidence: journey.expectedAnswerEvidence,
        expectedRuntimeEvidence: journey.expectedRuntimeEvidence,
        expectedUiStatus: journey.expectedUiStatus,
        expectedFinalDeliverySections: journey.expectedFinalDeliverySections,
        prepTaskId:
          journey.id === "S6-PERMISSION"
            ? "S6_PERMISSION_ACCEPT"
            : journey.id === "S6-RECOVERY"
              ? "D15"
              : null,
        controlLabels:
          journey.id === "S6-PERMISSION"
            ? ["Accept proposal"]
            : journey.id === "S6-RECOVERY"
              ? ["Cancel task from continuity detail", "Cancel task"]
              : [],
      }))
    );
    for (const id of STEP6_REQUIRED_PRODUCT_JOURNEYS) {
      expect(script).toContain(id);
    }
    expect(script).toContain("runStep6TauriProductAcceptance");
    expect(script).toContain("executeStep6LocalJourneyWithWebDriver");
    expect(script).toContain("executeStep6LiveJourneyWithWebDriver");
    expect(script).toContain("executeStep6SeededControlJourneyWithWebDriver");
    expect(script).toContain("executeStep6PermissionAcceptanceJourneyWithWebDriver");
    expect(script).toContain("#/__stage1-dogfood-chat");
    expect(script).toContain("openDiagnosticsIfPossible");
    expect(script).toContain("Show Main Chat diagnostics");
    expect(script).toContain("schemaVersion");
    expect(script).toContain("step6-product-acceptance-v1");
    expect(script).toContain("readinessSemantics");
    expect(script).toContain("smokePassed");
    expect(script).toContain("S6_PERMISSION_ACCEPT");
    expect(script).toContain("permission.accepted");
    expect(script).toContain("automatic_resume_replay");
    expect(script).toContain("prepare_main_chat_agent_stage1_browser_dogfood_state");
    expect(script).toContain("prepare_main_chat_step6_live_provider_eval_state");
    expect(script).toContain("step6LiveProviderStateReady");
    expect(script).toContain("step6_live_provider_state_not_ready");
    expect(script).toContain("external_provider_endpoint_required");
    expect(script).toContain("run_main_chat_agent_step6_product_acceptance_gate");
    expect(script).toContain("tauri_webdriver_step6_final_gate_rejected");
    expect(script).toContain("expectedEntryPointForStep6Journey");
    expect(script).toContain("tauri_webdriver_step6_route_legacy_or_fallback");
    expect(script).toContain("tauri_webdriver_step6_local_provider_kind_invalid");
    expect(script).toContain("expectedFinalDeliverySections");
    expect(script).toContain("tauri_webdriver_step6_final_delivery_section_missing");
    expect(script).toContain("validateStep6StaticJourneyMatrix");
    expect(script).toContain("validated_step6_product_acceptance_contract");
    expect(script).toContain("--allow-blocked-live");
    expect(script).toContain("blockedExternalLiveOnly");
    expect(script).toContain("localDeterministicReady");
    expect(script).not.toContain("step6_tauri_webdriver_executor_not_implemented");
    expect(script).toContain("tauri_webdriver_macos_not_supported_by_tauri_driver");
    expect(playwrightSpec).toContain("prepare_main_chat_step6_live_provider_eval_state");
    expect(playwrightSpec).toContain("step6LiveProviderStateReady");
    expect(playwrightSpec).toContain("isExternalProviderLabel");
    expect(playwrightSpec).toContain("localFixtureCreditedAsExternalLive");
  });

  it("exposes a Linux CI path for real Tauri Step 6 local acceptance without fake live credit", () => {
    const workflowPath = path.resolve(
      process.cwd(),
      "..",
      ".github",
      "workflows",
      "step6-tauri-product-acceptance.yml"
    );
    const workflow = fs.readFileSync(workflowPath, "utf8");

    expect(workflow).toContain("runs-on: ubuntu-22.04");
    expect(workflow).toContain("workflow_dispatch:");
    expect(workflow).toContain("run_external_live:");
    expect(workflow).toContain("webkit2gtk-driver");
    expect(workflow).toContain("xvfb");
    expect(workflow).toContain("corepack prepare pnpm@9.1.0 --activate");
    expect(workflow).toContain("cargo install tauri-driver --locked");
    expect(workflow).toContain("cargo build -p openlife-tauri");
    expect(workflow).toContain("pnpm --dir frontend test:e2e:tauri:step6:local");
    expect(workflow).toContain("tauri_command_surface_step6_browser_observed");
    expect(workflow).toContain("localDeterministicReady !== true");
    expect(workflow).toContain("acceptanceReady !== false");
    expect(workflow).toContain("externalLiveReady !== false");
    expect(workflow).toContain("S6-LIVE-WEB");
    expect(workflow).toContain("S6-LIVE-MCP");
    expect(workflow).toContain("OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL");
    expect(workflow).toContain("OPENLIFE_LIVE_EVAL_PROVIDER");
    expect(workflow).toContain("OPENLIFE_LIVE_EVAL_BASE");
    expect(workflow).toContain("OPENLIFE_LIVE_EVAL_MODEL");
    expect(workflow).toContain("OPENLIFE_LIVE_EVAL_API_KEY");
    expect(workflow).toContain("pnpm --dir frontend test:e2e:tauri:step6");
    expect(workflow).toContain("acceptanceReady !== true");
    expect(workflow).toContain("externalLiveReady !== true");
    expect(workflow).not.toContain("OPENAI_API_KEY");
    expect(workflow).toContain(
      "cargo test -p openlife-tauri --locked main_chat_final_acceptance -- --nocapture"
    );
    expect(workflow).toContain(
      "frontend/test-results/main-chat-step6-product-acceptance-report.json"
    );
    expect(workflow).toContain("actions/upload-artifact");
    expect(workflow).not.toContain("macos-latest");
  });

  it("keeps retired Step 6 prep commands out of the current Tauri command surface", () => {
    const tauriApi = fs.readFileSync(path.resolve(process.cwd(), "src/tauri.ts"), "utf8");
    const rustCommand = fs.readFileSync(
      path.resolve(process.cwd(), "..", "src-tauri", "src", "commands", "agent_runtime", "mod.rs"),
      "utf8"
    );
    const retiredRustGatePath = path.resolve(
      process.cwd(),
      "..",
      "src-tauri",
      "src",
      "main_chat_step6_product_acceptance.rs"
    );

    expect(tauriApi).not.toContain("MainChatStep6LiveProviderEvalStatePrepReport");
    expect(tauriApi).not.toContain("prepareMainChatStep6LiveProviderEvalState");
    expect(tauriApi).not.toContain("prepare_main_chat_step6_live_provider_eval_state");
    expect(rustCommand).not.toContain("prepare_main_chat_step6_live_provider_eval_state");
    expect(rustCommand).not.toContain("run_main_chat_agent_step6_product_acceptance_gate");
    expect(fs.existsSync(retiredRustGatePath)).toBe(false);
  });
});
