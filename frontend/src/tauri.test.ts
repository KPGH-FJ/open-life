import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  addDailyGoal,
  acceptProposal,
  builderStart,
  applyCalibration,
  editProposal,
  getStateHistory,
  recordState,
  checkControlledChatPilotEligibility,
  checkControlledChatCutoverReadiness,
  checkControlledChatCutoverCandidatePromotionReadiness,
  checkControlledChatMigrationImplementationGate,
  checkControlledPilotPromotionReadiness,
  checkRuntimeMigrationGate,
  getDefaultChatRuntimeBoundaryStatus,
  draftControlledChatMigrationPlan,
  getControlledChatCutoverCandidateReviewSummary,
  getControlledChatMigrationReviewDecisionSummary,
  getControlledChatMigrationShadowReviewSummary,
  recordControlledChatCutoverCandidateReviewDecision,
  recordControlledChatMigrationReviewDecision,
  recordControlledChatMigrationShadowReviewDecision,
  runControlledChatCutoverCandidate,
  runControlledChatMigrationShadowRun,
  runMultiStrategyAgentPreview,
  restoreArchivedChunks,
  saveChatMessage,
  startStreamMessage,
} from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauri command argument aliases", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("adds camelCase aliases for snake_case command arguments", async () => {
    await getStateHistory("专注度", 7);
    await restoreArchivedChunks([1, 2]);

    expect(invoke).toHaveBeenCalledWith(
      "get_state_history",
      expect.objectContaining({
        dimensionName: "专注度",
        dimension_name: "专注度",
        limit: 7,
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "restore_archived_chunks",
      expect.objectContaining({
        chunkIds: [1, 2],
        chunk_ids: [1, 2],
      })
    );
  });

  it("keeps existing explicit aliases for high-traffic chat and builder commands", async () => {
    await startStreamMessage("session-1", [{ role: "user", content: "你好" }]);
    await saveChatMessage("session-1", { role: "assistant", content: "你好" });
    await builderStart("incremental", "builder-1", "goals");

    expect(invoke).toHaveBeenCalledWith(
      "start_stream_message",
      expect.objectContaining({
        sessionId: "session-1",
        session_id: "session-1",
        args: expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
        }),
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "save_chat_message",
      expect.objectContaining({
        sessionId: "session-1",
        session_id: "session-1",
        message: { role: "assistant", content: "你好" },
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "builder_start",
      expect.objectContaining({
        sessionId: "builder-1",
        session_id: "builder-1",
        targetDimension: "goals",
        target_dimension: "goals",
      })
    );
  });

  it("normalizes optional state and daily-goal arguments before invoke", async () => {
    await recordState("睡眠", 7.5, "小时", "昨晚", 6, 9, 2);
    await addDailyGoal("阅读30分钟");

    expect(invoke).toHaveBeenCalledWith(
      "record_state",
      expect.objectContaining({
        dimensionName: "睡眠",
        dimension_name: "睡眠",
        minThreshold: 6,
        min_threshold: 6,
        maxThreshold: 9,
        max_threshold: 9,
        alertDays: 2,
        alert_days: 2,
      })
    );
    expect(invoke).toHaveBeenCalledWith("add_daily_goal", { name: "阅读30分钟" });
  });

  it("normalizes proposal command arguments", async () => {
    await acceptProposal("proposal-1");
    await editProposal("proposal-1", { name: "新值" });

    expect(invoke).toHaveBeenCalledWith(
      "accept_proposal",
      expect.objectContaining({
        proposalId: "proposal-1",
        proposal_id: "proposal-1",
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "edit_proposal",
      expect.objectContaining({
        proposalId: "proposal-1",
        proposal_id: "proposal-1",
        newAfter: { name: "新值" },
        new_after: { name: "新值" },
      })
    );
  });

  it("defaults calibration apply calls to proposal mode", async () => {
    await applyCalibration([]);

    expect(invoke).toHaveBeenCalledWith(
      "apply_calibration",
      expect.objectContaining({
        changes: [],
        mode: "proposal",
      })
    );
  });

  it("invokes multi-strategy preview command behind explicit wrapper", async () => {
    vi.mocked(invoke).mockResolvedValue({
      runId: "run-preview-1",
      strategyKind: "react",
      payloadKind: "react",
      proposalIds: [],
      warnings: [],
      metadataSafeSummary: {},
    });

    const result = await runMultiStrategyAgentPreview({
      sessionId: "session-preview",
      userText: "What should I focus on today?",
      toolsPrompt: "Available tools: memory.search",
      allowPlanning: true,
      localModelAvailable: true,
    });

    expect(invoke).toHaveBeenCalledWith("run_multi_strategy_agent_preview", {
      input: expect.objectContaining({
        sessionId: "session-preview",
        userText: "What should I focus on today?",
        toolsPrompt: "Available tools: memory.search",
        allowPlanning: true,
        localModelAvailable: true,
      }),
    });
    expect(result.runId).toBe("run-preview-1");
  });

  it("invokes runtime migration gate as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      defaultChatUnchanged: true,
      previewPathHealthy: true,
      metadataSafeTraceReady: true,
      fallbackAvailable: true,
      noExternalWrites: true,
      proposalFirstPreserved: true,
      blockingReasons: [],
    });

    const result = await checkRuntimeMigrationGate({
      previewRunId: "run-preview-1",
      sessionId: "session-preview",
    });

    expect(invoke).toHaveBeenCalledWith("check_runtime_migration_gate", {
      input: {
        previewRunId: "run-preview-1",
        sessionId: "session-preview",
      },
    });
    expect(result.defaultChatUnchanged).toBe(true);
  });

  it("invokes controlled Chat pilot eligibility as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      eligible: true,
      requiredCleanRuns: 3,
      cleanRunCount: 3,
      checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
      blockingReasons: [],
      lastGateReport: {
        defaultChatUnchanged: true,
        previewPathHealthy: true,
        metadataSafeTraceReady: true,
        fallbackAvailable: true,
        noExternalWrites: true,
        proposalFirstPreserved: true,
        blockingReasons: [],
      },
      defaultChatUnchanged: true,
    });

    const result = await checkControlledChatPilotEligibility();

    expect(invoke).toHaveBeenCalledWith("check_controlled_chat_pilot_eligibility", {
      input: {},
    });
    expect(result.eligible).toBe(true);
    expect(result.cleanRunCount).toBe(3);
  });

  it("invokes controlled pilot promotion readiness as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ready: true,
      requiredPromotions: 3,
      promotedCount: 3,
      recentPromotedPilotRunIds: [
        "run-controlled-pilot-3",
        "run-controlled-pilot-2",
        "run-controlled-pilot-1",
      ],
      latestPromotionTimestamp: "2026-05-30T03:04:05Z",
      sourceTargetMismatchBlockCount: 0,
      metadataSafeEvidenceReady: true,
      defaultChatUnchanged: true,
      blockingReasons: [],
    });

    const result = await checkControlledPilotPromotionReadiness({
      requiredPromotions: 3,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("check_controlled_pilot_promotion_readiness", {
      input: {
        requiredPromotions: 3,
        sessionId: "session-1",
      },
    });
    expect(result.ready).toBe(true);
    expect(result.promotedCount).toBe(3);
  });

  it("invokes controlled chat migration plan draft as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      draftReady: true,
      readinessReport: {
        ready: true,
        requiredPromotions: 3,
        promotedCount: 3,
        recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
        latestPromotionTimestamp: "2026-05-30T03:04:05Z",
        sourceTargetMismatchBlockCount: 0,
        metadataSafeEvidenceReady: true,
        defaultChatUnchanged: true,
        blockingReasons: [],
      },
      migrationScope: ["default Chat remains unchanged"],
      requiredPreconditions: ["separate human approval"],
      rollbackPlan: ["disable the controlled pilot entry"],
      fallbackPlan: ["use the existing default Chat send path"],
      testPlan: ["verify send_message and start_stream_message"],
      manualReviewRequired: true,
      notAutomaticMigration: true,
      blockingReasons: [],
    });

    const result = await draftControlledChatMigrationPlan({
      requiredPromotions: 3,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("draft_controlled_chat_migration_plan", {
      input: {
        requiredPromotions: 3,
        sessionId: "session-1",
      },
    });
    expect(result.draftReady).toBe(true);
    expect(result.manualReviewRequired).toBe(true);
    expect(result.notAutomaticMigration).toBe(true);
  });

  it("records controlled chat migration review decision through explicit wrapper", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_review_1",
      decisionKind: "approve",
      draftReady: true,
      draftHash: "sha256:test-draft",
      createdAt: "2026-05-31T01:02:03Z",
      blockingReasons: [],
    });

    const result = await recordControlledChatMigrationReviewDecision({
      decisionKind: "approve",
      requiredPromotions: 3,
      sessionId: "session-1",
      optionalReviewerNote: "Reviewer note stays backend-sanitized.",
    });

    expect(invoke).toHaveBeenCalledWith("record_controlled_chat_migration_review_decision", {
      input: {
        decisionKind: "approve",
        requiredPromotions: 3,
        sessionId: "session-1",
        optionalReviewerNote: "Reviewer note stays backend-sanitized.",
      },
    });
    expect(result.recorded).toBe(true);
    expect(result.evidenceId).toBe("ev_review_1");
  });

  it("reads controlled chat migration review decision summary through explicit wrapper", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_review_1",
        decisionKind: "request_rework",
        draftReady: true,
        draftHash: "sha256:test-draft",
        createdAt: "2026-05-31T01:02:03Z",
      },
      approvedCount: 1,
      reworkRejectCount: 2,
      latestTimestamp: "2026-05-31T01:02:03Z",
      blockingReasons: [],
    });

    const result = await getControlledChatMigrationReviewDecisionSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_controlled_chat_migration_review_decision_summary",
      undefined
    );
    expect(result.latestDecision?.decisionKind).toBe("request_rework");
    expect(result.approvedCount).toBe(1);
    expect(result.reworkRejectCount).toBe(2);
  });

  it("invokes controlled chat migration implementation gate as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      implementationEligible: true,
      latestDecision: {
        evidenceId: "ev_review_2",
        decisionKind: "approve",
        draftReady: true,
        draftHash: "sha256:test-draft",
        createdAt: "2026-05-31T02:03:04Z",
      },
      readinessReport: {
        ready: true,
        requiredPromotions: 3,
        promotedCount: 3,
        recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
        latestPromotionTimestamp: "2026-05-30T03:04:05Z",
        sourceTargetMismatchBlockCount: 0,
        metadataSafeEvidenceReady: true,
        defaultChatUnchanged: true,
        blockingReasons: [],
      },
      draftHashMatched: true,
      approvedAfterLatestDraft: true,
      blockingReasons: [],
    });

    const result = await checkControlledChatMigrationImplementationGate({
      requiredPromotions: 3,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("check_controlled_chat_migration_implementation_gate", {
      input: {
        requiredPromotions: 3,
        sessionId: "session-1",
      },
    });
    expect(result.implementationEligible).toBe(true);
    expect(result.latestDecision?.decisionKind).toBe("approve");
    expect(result.draftHashMatched).toBe(true);
  });

  it("invokes controlled chat migration shadow run as explicit non-default command", async () => {
    vi.mocked(invoke).mockResolvedValue({
      shadowRunReady: true,
      shadowRunId: "run-shadow-1",
      implementationGateReport: {
        implementationEligible: true,
        latestDecision: {
          evidenceId: "ev_review_2",
          decisionKind: "approve",
          draftReady: true,
          draftHash: "sha256:test-draft",
          createdAt: "2026-05-31T02:03:04Z",
        },
        readinessReport: {
          ready: true,
          requiredPromotions: 3,
          promotedCount: 3,
          recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
          latestPromotionTimestamp: "2026-05-30T03:04:05Z",
          sourceTargetMismatchBlockCount: 0,
          metadataSafeEvidenceReady: true,
          defaultChatUnchanged: true,
          blockingReasons: [],
        },
        draftHashMatched: true,
        approvedAfterLatestDraft: true,
        blockingReasons: [],
      },
      strategyKind: "planExecute",
      payloadKind: "planExecute",
      metadataSafeSummary: {
        descriptorKind: "planning_readiness_probe",
        allowWrites: false,
        metadataSafe: true,
      },
      warnings: ["shadow runtime forced allowWrites=false"],
      blockingReasons: [],
    });

    const result = await runControlledChatMigrationShadowRun({
      sessionId: "session-1",
      userInputChecksum: "sha256:raw-user-input-checksum",
      boundedTestPromptDescriptor: "planning_readiness_probe",
      requiredPromotions: 3,
    });

    expect(invoke).toHaveBeenCalledWith("run_controlled_chat_migration_shadow_run", {
      input: {
        sessionId: "session-1",
        userInputChecksum: "sha256:raw-user-input-checksum",
        boundedTestPromptDescriptor: "planning_readiness_probe",
        requiredPromotions: 3,
      },
    });
    expect(result.shadowRunReady).toBe(true);
    expect(result.metadataSafeSummary.allowWrites).toBe(false);
  });

  it("records controlled chat migration shadow review decision through explicit wrapper", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_shadow_review_1",
      shadowRunId: "run-shadow-1",
      decisionKind: "approve",
      readinessSummaryDigest: "sha256:shadow-readiness",
      createdAt: "2026-05-31T04:05:06Z",
      blockingReasons: [],
    });

    const result = await recordControlledChatMigrationShadowReviewDecision({
      shadowRunId: "run-shadow-1",
      decisionKind: "approve",
      optionalReviewerNote: "Reviewer note stays checksum-only.",
    });

    expect(invoke).toHaveBeenCalledWith("record_controlled_chat_migration_shadow_review_decision", {
      input: {
        shadowRunId: "run-shadow-1",
        decisionKind: "approve",
        optionalReviewerNote: "Reviewer note stays checksum-only.",
      },
    });
    expect(result.recorded).toBe(true);
    expect(result.evidenceId).toBe("ev_shadow_review_1");
  });

  it("reads controlled chat migration shadow review summary through explicit wrapper", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_shadow_review_2",
        shadowRunId: "run-shadow-2",
        decisionKind: "request_rework",
        reviewerNoteChecksum: "sha256:reviewer-note",
        reviewerNoteLength: 19,
        reviewerNoteCategory: "brief",
        readinessSummaryDigest: "sha256:shadow-readiness-2",
        createdAt: "2026-05-31T05:06:07Z",
      },
      approvedCount: 1,
      reworkRejectCount: 2,
      latestTimestamp: "2026-05-31T05:06:07Z",
      blockingReasons: [],
    });

    const result = await getControlledChatMigrationShadowReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_controlled_chat_migration_shadow_review_summary",
      undefined
    );
    expect(result.latestDecision?.decisionKind).toBe("request_rework");
    expect(result.latestDecision?.shadowRunId).toBe("run-shadow-2");
    expect(result.reworkRejectCount).toBe(2);
  });

  it("invokes controlled chat cutover readiness as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      cutoverPlanningEligible: true,
      implementationGateReport: {
        implementationEligible: true,
        latestDecision: {
          evidenceId: "ev_review_2",
          decisionKind: "approve",
          draftReady: true,
          draftHash: "sha256:test-draft",
          createdAt: "2026-05-31T02:03:04Z",
        },
        readinessReport: {
          ready: true,
          requiredPromotions: 3,
          promotedCount: 3,
          recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
          latestPromotionTimestamp: "2026-05-30T03:04:05Z",
          sourceTargetMismatchBlockCount: 0,
          metadataSafeEvidenceReady: true,
          defaultChatUnchanged: true,
          blockingReasons: [],
        },
        draftHashMatched: true,
        approvedAfterLatestDraft: true,
        blockingReasons: [],
      },
      latestShadowReviewDecision: {
        evidenceId: "ev_shadow_review_2",
        shadowRunId: "run-shadow-2",
        decisionKind: "approve",
        reviewerNoteChecksum: "sha256:reviewer-note",
        reviewerNoteLength: 19,
        reviewerNoteCategory: "brief",
        readinessSummaryDigest: "sha256:shadow-readiness-2",
        createdAt: "2026-05-31T05:06:07Z",
      },
      verifiedShadowRunId: "run-shadow-2",
      readinessSummaryDigest: "sha256:shadow-readiness-2",
      defaultChatUnchanged: true,
      requiredEvidenceReady: true,
      blockingReasons: [],
      metadataSafeSummary: {
        metadataSafe: true,
        planningOnly: true,
      },
    });

    const result = await checkControlledChatCutoverReadiness({
      requiredPromotions: 3,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("check_controlled_chat_cutover_readiness", {
      input: {
        requiredPromotions: 3,
        sessionId: "session-1",
      },
    });
    expect(result.cutoverPlanningEligible).toBe(true);
    expect(result.verifiedShadowRunId).toBe("run-shadow-2");
    expect(result.metadataSafeSummary.metadataSafe).toBe(true);
  });

  it("invokes controlled chat cutover candidate as an explicit non-default adapter", async () => {
    vi.mocked(invoke).mockResolvedValue({
      candidateReady: true,
      candidateRunId: "run-candidate-1",
      outputPreview: "Cutover candidate: react / react",
      userOutput: "Candidate-only answer",
      contractShape: "send_message_compatible",
      metadataSafeSummary: {
        metadataSafe: true,
        candidateAdapter: "controlled_chat_cutover_candidate",
      },
      warnings: ["candidate runtime forced allowWrites=false"],
      blockingReasons: [],
    });

    const result = await runControlledChatCutoverCandidate({
      sessionId: "session-candidate-1",
      boundedTestPromptDescriptor: "default_contract_probe",
      requiredPromotions: 3,
    });

    expect(invoke).toHaveBeenCalledWith("run_controlled_chat_cutover_candidate", {
      input: {
        sessionId: "session-candidate-1",
        boundedTestPromptDescriptor: "default_contract_probe",
        requiredPromotions: 3,
      },
    });
    expect(result.candidateReady).toBe(true);
    expect(result.contractShape).toBe("send_message_compatible");
    expect(result.candidateRunId).toBe("run-candidate-1");
  });

  it("invokes controlled chat cutover candidate review decision explicitly", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_candidate_review_1",
      candidateRunId: "run-candidate-1",
      decisionKind: "approve",
      contractShape: "send_message_compatible",
      candidateSummaryDigest: "sha256:candidate-summary",
      createdAt: "2026-05-31T06:07:08Z",
      blockingReasons: [],
    });

    const result = await recordControlledChatCutoverCandidateReviewDecision({
      candidateRunId: "run-candidate-1",
      decisionKind: "approve",
      optionalReviewerNote: "Approved manually.",
    });

    expect(invoke).toHaveBeenCalledWith(
      "record_controlled_chat_cutover_candidate_review_decision",
      {
        input: {
          candidateRunId: "run-candidate-1",
          decisionKind: "approve",
          optionalReviewerNote: "Approved manually.",
        },
      }
    );
    expect(result.recorded).toBe(true);
    expect(result.candidateSummaryDigest).toBe("sha256:candidate-summary");
  });

  it("invokes controlled chat cutover candidate review summary as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_candidate_review_2",
        candidateRunId: "run-candidate-2",
        decisionKind: "request_rework",
        contractShape: "send_message_compatible",
        candidateSummaryDigest: "sha256:candidate-summary-2",
        reviewerNoteChecksum: "sha256:reviewer-note",
        reviewerNoteLength: 18,
        reviewerNoteCategory: "brief",
        createdAt: "2026-05-31T07:08:09Z",
      },
      approvedCount: 1,
      reworkRejectCount: 2,
      latestTimestamp: "2026-05-31T07:08:09Z",
      blockingReasons: [],
    });

    const result = await getControlledChatCutoverCandidateReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_controlled_chat_cutover_candidate_review_summary",
      undefined
    );
    expect(result.latestDecision?.decisionKind).toBe("request_rework");
    expect(result.latestDecision?.candidateRunId).toBe("run-candidate-2");
    expect(result.reworkRejectCount).toBe(2);
  });

  it("invokes controlled chat cutover candidate promotion readiness as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ready: true,
      cutoverReadinessEligible: true,
      requiredApprovedCandidates: 1,
      approvedCandidateCount: 1,
      latestDecision: {
        evidenceId: "ev_candidate_review_3",
        candidateRunId: "run-candidate-3",
        decisionKind: "approve",
        contractShape: "send_message_compatible",
        candidateSummaryDigest: "sha256:candidate-summary-3",
        reviewerNoteChecksum: null,
        reviewerNoteLength: 0,
        reviewerNoteCategory: "none",
        createdAt: "2026-05-31T08:09:10Z",
      },
      approvedCandidates: [
        {
          evidenceId: "ev_candidate_review_3",
          candidateRunId: "run-candidate-3",
          contractShape: "send_message_compatible",
          candidateSummaryDigest: "sha256:candidate-summary-3",
          runReadinessDigest: "sha256:run-readiness",
          decisionCreatedAt: "2026-05-31T08:09:10Z",
          ready: true,
          blockingReasons: [],
        },
      ],
      defaultChatUnchanged: true,
      blockingReasons: [],
      metadataSafeSummary: {
        metadataSafe: true,
        readOnly: true,
      },
      checkedAt: "2026-05-31T08:10:00Z",
    });

    const result = await checkControlledChatCutoverCandidatePromotionReadiness({
      requiredApprovedCandidates: 2,
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_controlled_chat_cutover_candidate_promotion_readiness",
      {
        input: {
          requiredApprovedCandidates: 2,
        },
      }
    );
    expect(result.ready).toBe(true);
    expect(result.approvedCandidateCount).toBe(1);
    expect(result.approvedCandidates[0].candidateRunId).toBe("run-candidate-3");
  });

  it("invokes default chat runtime boundary status as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      currentMode: "legacy_stream",
      controlledCandidateAvailable: false,
      defaultChatUnchanged: true,
      candidatePromotionReadinessRequired: true,
      automaticMigrationEnabled: false,
      blockingReasons: [],
      metadataSafeSummary: {
        runtimeBoundary: "default_chat",
        metadataSafe: true,
        readOnly: true,
        currentMode: "legacy_stream",
        automaticMigrationEnabled: false,
      },
    });

    const result = await getDefaultChatRuntimeBoundaryStatus();

    expect(invoke).toHaveBeenCalledWith("get_default_chat_runtime_boundary_status", undefined);
    expect(result.currentMode).toBe("legacy_stream");
    expect(result.controlledCandidateAvailable).toBe(false);
    expect(result.automaticMigrationEnabled).toBe(false);
  });
});
