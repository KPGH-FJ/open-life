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
  checkDefaultChatAdapterActivationImplementationGate,
  checkDefaultChatAdapterContractHarness,
  checkDefaultChatAdapterImplementationReadiness,
  checkDefaultChatAdapterNarrowImplementationDiscussionGate,
  checkDefaultChatAdapterNarrowImplementationPlanApprovalReadiness,
  checkDefaultChatAdapterControlledPreviewApprovalReadiness,
  checkDefaultChatAdapterCutoverPlanApprovalReadiness,
  draftDefaultChatAdapterNarrowImplementationPlan,
  draftDefaultChatAdapterCutoverImplementationPlan,
  getDefaultChatAdapterNarrowImplementationPlanReviewSummary,
  getDefaultChatAdapterCutoverPlanReviewSummary,
  recordDefaultChatAdapterNarrowImplementationPlanReviewDecision,
  recordDefaultChatAdapterCutoverPlanReviewDecision,
  checkRuntimeMigrationGate,
  getDefaultChatAdapterRoutingStatus,
  getDefaultChatAdapterOrdinaryEntryPreflightStatus,
  getDefaultChatRuntimeBoundaryStatus,
  getRuntimeStrategyRegistryStatus,
  runDefaultChatAdapterControlledPreview,
  getDefaultChatAdapterControlledPreviewReviewSummary,
  recordDefaultChatAdapterControlledPreviewReviewDecision,
  runDefaultChatAdapterDryRun,
  getDefaultChatAdapterDryRunReviewSummary,
  recordDefaultChatAdapterDryRunReviewDecision,
  draftDefaultChatAdapterActivationPlan,
  getDefaultChatAdapterActivationReviewSummary,
  recordDefaultChatAdapterActivationReviewDecision,
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
  runMainChatAgentExecutionV1EvalGate,
  clearMainChatSkill,
  getMainChatSkillDetail,
  listMainChatSkills,
  listMainChatToolCandidates,
  runMainChatExternalLiveProductizationGate,
  runMainChatAgentProductMaturityV2FinalReadinessGate,
  runMainChatAgentProductMaturityV2EventGate,
  runMainChatAgentProductMaturityV2PlanGate,
  runMainChatAgentProductMaturityV2SkillsGate,
  runMainChatAgentProductizationV1Gate,
  selectMainChatSkill,
  listMainChatAgentEvents,
  getMainChatAgentStateSnapshot,
  restoreArchivedChunks,
  restoreSnapshot,
  saveChatMessage,
  startStreamMessage,
  importAllData,
  redactInvokeArgs,
  saveConfig,
  sendMessageV2,
  executeToolCall,
} from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauri command argument aliases", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  function redactedLogForLastInvoke(): string {
    const calls = vi.mocked(invoke).mock.calls;
    const lastCall = calls[calls.length - 1];
    expect(lastCall).toBeTruthy();
    const [cmd, args] = lastCall as [string, Record<string, any> | undefined];
    return JSON.stringify(redactInvokeArgs(cmd, args));
  }

  it("redacts send_message content from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reply: "ok",
      reasoning_trace: {},
      tool_calls: [],
      run_id: "run-1",
    });

    await sendMessageV2("session-secret", [
      { role: "user", content: "我的邮箱 test@example.com 和身份证 11010519491231002X" },
    ]);

    const redacted = redactedLogForLastInvoke();
    expect(redacted).toContain("session-secret");
    expect(redacted).not.toContain("test@example.com");
    expect(redacted).not.toContain("11010519491231002X");
    expect(redacted).toContain('"redacted":true');
  });

  it("redacts save_config secrets from dev invoke logs", async () => {
    await saveConfig({
      llm: {
        provider: "openai",
        openai_base: "https://api.openai.com/v1",
        openai_key: "sk-openai-secret",
        embedding_model: "text-embedding-3-small",
        chat_model: "gpt-4o-mini",
      },
      prefer_local_model: false,
      local_model: "llama3",
    });

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("sk-openai-secret");
    expect(redacted).toContain("openai_key");
    expect(redacted).toContain('"redacted":true');
  });

  it("redacts import_all_data payloads from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      legacy: false,
      governed_operation: true,
      metadata_safe: true,
      durable_lifemodel_write: true,
      imported_message_count: 1,
      imported_vector_count: 1,
    });

    await importAllData({
      version: "1.0",
      exported_at: "2026-06-03T00:00:00Z",
      life_model: { identity: { name: "张三" } } as any,
      messages: [
        {
          session_id: "session-import",
          role: "user",
          content: "导入的私密聊天原文",
          created_at: "2026-06-03T00:00:00Z",
        },
      ],
      vectors: [
        {
          session_id: "session-import",
          content: "导入的向量原文",
          embedding: [1, 2, 3],
          source: "chat",
          created_at: "2026-06-03T00:00:00Z",
          tier: 1,
          access_count: 0,
          last_accessed_at: "2026-06-03T00:00:00Z",
        },
      ],
    });

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("导入的私密聊天原文");
    expect(redacted).not.toContain("导入的向量原文");
    expect(redacted).not.toContain("张三");
    expect(redacted).toContain("payload");
    expect(redacted).toContain('"redacted":true');
  });

  it("redacts tool arguments and file or email content from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      name: "email.propose_draft",
      arguments: {},
      success: true,
    });

    await executeToolCall("email.propose_draft", {
      to: "person@example.com",
      body: "邮件正文原文",
      file_content: "文件内容原文",
      token: "tool-token-secret",
    });

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("person@example.com");
    expect(redacted).not.toContain("邮件正文原文");
    expect(redacted).not.toContain("文件内容原文");
    expect(redacted).not.toContain("tool-token-secret");
    expect(redacted).toContain("arguments");
    expect(redacted).toContain('"redacted":true');
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

  it("passes selected skill id aliases through chat command wrappers", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reply: "ok",
      reasoning_trace: {},
      tool_calls: [],
    });

    await sendMessageV2("session-skill", [{ role: "user", content: "Summarize this" }], {
      selectedSkillId: "summarize",
    });
    await startStreamMessage("session-skill", [{ role: "user", content: "Summarize this" }], {
      selectedSkillId: "summarize",
    });

    expect(invoke).toHaveBeenCalledWith(
      "send_message",
      expect.objectContaining({
        selectedSkillId: "summarize",
        selected_skill_id: "summarize",
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "start_stream_message",
      expect.objectContaining({
        selectedSkillId: "summarize",
        selected_skill_id: "summarize",
        args: expect.objectContaining({
          selectedSkillId: "summarize",
          selected_skill_id: "summarize",
        }),
      })
    );
  });

  it("adds aliases for durable Main Chat event replay commands", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ task: { taskId: "mainchat-task-1" } });

    await listMainChatAgentEvents("mainchat-task-1", 7, 50);
    await getMainChatAgentStateSnapshot("mainchat-task-1");

    expect(invoke).toHaveBeenCalledWith(
      "list_main_chat_agent_events",
      expect.objectContaining({
        taskSessionId: "mainchat-task-1",
        task_session_id: "mainchat-task-1",
        afterSequence: 7,
        after_sequence: 7,
        limit: 50,
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "get_main_chat_agent_state_snapshot",
      expect.objectContaining({
        taskSessionId: "mainchat-task-1",
        task_session_id: "mainchat-task-1",
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

  it("sends governed restore and import request envelopes", async () => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      legacy: false,
      governed_operation: true,
      warning: "metadata-safe",
      metadata_safe: true,
      durable_lifemodel_write: true,
      restored_snapshot_version: "0.1.0",
      pre_restore_snapshot_created: true,
    });
    await restoreSnapshot("0.1.0");

    expect(invoke).toHaveBeenCalledWith("restore_snapshot", {
      version: "0.1.0",
      governedRequest: {
        purpose: "manual_restore",
        explicitUserIntent: true,
        createPreChangeSnapshot: true,
      },
      governed_request: {
        purpose: "manual_restore",
        explicitUserIntent: true,
        createPreChangeSnapshot: true,
      },
    });

    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      legacy: false,
      governed_operation: true,
      warning: "metadata-safe",
      metadata_safe: true,
      durable_lifemodel_write: true,
      imported_message_count: 0,
      imported_vector_count: 0,
    });
    await importAllData({
      version: "1.0",
      exported_at: "2026-06-03T00:00:00Z",
      life_model: {} as any,
      messages: [],
      vectors: [],
    });

    expect(invoke).toHaveBeenCalledWith("import_all_data", {
      payload: expect.objectContaining({
        version: "1.0",
        messages: [],
        vectors: [],
      }),
      importRequest: {
        purpose: "manual_restore",
        explicitUserIntent: true,
        createPreChangeSnapshot: true,
        importTargets: ["life_model", "messages", "vectors"],
      },
      import_request: {
        purpose: "manual_restore",
        explicitUserIntent: true,
        createPreChangeSnapshot: true,
        importTargets: ["life_model", "messages", "vectors"],
      },
    });
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

  it("invokes Main Chat execution v1 eval gate as explicit non-default diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_agent_execution_v1_eval_gate",
      runtimeEval: {
        totalCases: 100,
        runtimeExecutedCaseCount: 100,
        deterministicStubCaseCount: 0,
        passedCases: 100,
        failedCases: 0,
        silentWriteCount: 0,
        finalCompletionReady: false,
        finalCompletionBlockers: ["live_provider_generation_not_executed"],
        failures: [],
      },
      acceptance: {
        ready: false,
        status: "blocked",
        blockers: ["command_surface_cases_below_24"],
        requiredEvidence: [],
        runtimeGateReady: true,
        commandSurfaceGateReady: false,
        liveProviderGateReady: false,
        directWritesExecuted: false,
      },
      liveProviderPreflight: {
        ready: false,
        status: "blocked",
        provider: "openai",
        blockers: ["explicit_live_eval_required", "provider_api_key_missing"],
        requiredEvidence: [
          "live_provider_generation",
          "provider_backed_web_mcp_agent_loop",
          "provider_backed_web_agent_loop",
          "provider_backed_mcp_agent_loop",
          "provider_live_proposal_permission",
        ],
        liveProviderInvocationAllowed: false,
        modelInvoked: false,
        directWritesExecuted: false,
      },
      commandSurfaceGateExecuted: false,
      liveProviderAttempted: false,
      migrationPermission: false,
      metadataSafe: true,
      noExternalProviderInvocation: true,
      noAppStoreWrites: true,
      metadataSafeSummary: {
        liveProviderPreflightBlockers: ["explicit_live_eval_required", "provider_api_key_missing"],
        liveProviderPreflightModelInvoked: false,
      },
    });

    const result = await runMainChatAgentExecutionV1EvalGate();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_agent_execution_v1_eval_gate", undefined);
    expect(result.reportKind).toBe("main_chat_agent_execution_v1_eval_gate");
    expect(result.migrationPermission).toBe(false);
    expect(result.noExternalProviderInvocation).toBe(true);
    expect(result.liveProviderPreflight.modelInvoked).toBe(false);
    expect(result.liveProviderPreflight.requiredEvidence).toContain(
      "provider_backed_web_agent_loop"
    );
    expect(result.liveProviderPreflight.requiredEvidence).toContain(
      "provider_backed_mcp_agent_loop"
    );
    expect(result.metadataSafeSummary.liveProviderPreflightModelInvoked).toBe(false);
  });

  it("invokes Main Chat agent productization v1 gate as full deterministic runtime diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      totalScenarioCount: 93,
      defaultDeterministicScenarioCount: 92,
      readinessSemantics: "full_deterministic_productization_v1_runtime_ready",
      runtimeExecutionScope:
        "default_deterministic_scenarios_runtime_backed_external_live_excluded",
      executedScenarioCount: 92,
      passedScenarioCount: 81,
      expectedBlockerScenarioCount: 11,
      failedScenarioCount: 0,
      externalLiveExcludedCount: 1,
      runtimePayloadSnapshotEventGatePassed: true,
      runtimeRequiredGroupCount: 92,
      runtimeRequiredGroupPassedCount: 92,
      representativeRuntimeGroupCount: 0,
      representativeRuntimeGroupPassedCount: 0,
      fullDeterministicRuntimeScenarioCount: 92,
      fullDeterministicRuntimeScenarioExecutedCount: 92,
      runtimeRequiredGroupEvidence: [
        {
          scenarioId: "OA-02",
          group: "direct_answer:OA-02",
          passed: true,
          runtimeObjectCount: 2,
          observationCount: 0,
          createdActionIds: [],
          createdObservationIds: [],
          createdProposalIds: [],
          createdMemoryIds: [],
          rollbackEventIds: [],
          materializedViewVersions: [],
          inactiveMemoryIds: [],
          finalDeliveryId: "delivery-direct",
          diagnostics: [],
        },
      ],
      eventSemantics:
        "durable_replayable_delta_events_available_snapshot_backfill_excluded_from_live_credit",
      finalReadinessReady: true,
      fullProductizationV1Complete: true,
      futureWork: [],
      routeCounts: {
        direct_answer: { passed: 10, failed: 0, expectedBlocker: 0, unsupported: 0 },
      },
      unsupportedScenarios: [],
      failedScenarios: [],
      blockers: [],
    });

    const result = await runMainChatAgentProductizationV1Gate();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_agent_productization_v1_gate", undefined);
    expect(result.finalReadinessReady).toBe(true);
    expect(result.fullProductizationV1Complete).toBe(true);
    expect(result.futureWork).toEqual([]);
    expect(result.runtimeRequiredGroupCount).toBe(92);
    expect(result.runtimeRequiredGroupPassedCount).toBe(92);
    expect(result.runtimeExecutionScope).toBe(
      "default_deterministic_scenarios_runtime_backed_external_live_excluded"
    );
  });

  it("invokes external live productization gate as opt-in non-default evidence", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_external_live_productization_gate",
      scenarioCount: 6,
      defaultGateScenarioCount: 0,
      readinessSemantics:
        "opt_in_external_live_product_evidence_only_default_readiness_unchanged",
      runMode: "external_live_opt_in",
      liveProviderAttempted: false,
      passedScenarioCount: 0,
      blockedScenarioCount: 6,
      failedScenarioCount: 0,
      ready: false,
      externalProviderInvoked: false,
      directWritesExecuted: false,
      legacyFallbackUsed: false,
      deterministicReadinessUnchanged: true,
      blockers: ["explicit_live_eval_required"],
      proofs: [
        {
          scenarioId: "LIVE-PROD-01",
          passed: false,
          status: "blocked",
          provider: "",
          providerModel: null,
          providerEndpointKind: "",
          taskSessionId: null,
          runId: null,
          actionIds: [],
          observationIds: [],
          proposalIds: [],
          blockerIds: [],
          finalDeliveryId: null,
          eventTypes: [],
          eventSequenceStart: null,
          eventSequenceEnd: null,
          uiStateAssertions: [],
          runtimeEvidence: [],
          controls: [],
          negativeAssertions: [],
          blockers: ["explicit_live_eval_required"],
        },
      ],
    });

    const result = await runMainChatExternalLiveProductizationGate();

    expect(invoke).toHaveBeenCalledWith(
      "run_main_chat_external_live_productization_gate",
      undefined
    );
    expect(result.defaultGateScenarioCount).toBe(0);
    expect(result.ready).toBe(false);
    expect(result.liveProviderAttempted).toBe(false);
    expect(result.deterministicReadinessUnchanged).toBe(true);
    expect(result.blockers).toContain("explicit_live_eval_required");
  });

  it("invokes Product Maturity v2 event gate as an explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      scenarioCount: 8,
      defaultGateScenarioCount: 8,
      passedScenarioCount: 8,
      expectedBlockerCount: 0,
      ready: true,
      blockers: [],
      proofs: [
        {
          scenarioId: "EV-01",
          capabilityGroup: "event_delta_stream",
          passed: true,
          runtimeObjectCount: 2,
          emittedEventIds: ["mainchat_event:mock:1:route.selected:direct_answer:d1"],
          replayedEventIds: ["mainchat_event:mock:1:route.selected:direct_answer:d1"],
          emittedSequences: [1],
          replayedSequences: [1],
          uiState: ["subscribed", "receiving_event"],
          diagnostics: [],
        },
      ],
    });

    const result = await runMainChatAgentProductMaturityV2EventGate();

    expect(invoke).toHaveBeenCalledWith(
      "run_main_chat_agent_product_maturity_v2_event_gate",
      undefined
    );
    expect(result.ready).toBe(true);
    expect(result.scenarioCount).toBe(8);
    expect(result.proofs[0]?.emittedEventIds).toEqual(result.proofs[0]?.replayedEventIds);
  });

  it("invokes Product Maturity v2 plan gate as an explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      scenarioCount: 10,
      defaultGateScenarioCount: 10,
      passedScenarioCount: 10,
      expectedBlockerCount: 3,
      ready: true,
      blockers: [],
      scenarios: [
        {
          id: "PI-01",
          capabilityGroup: "plan_interaction",
          prompt: "Plan this work before executing.",
          preconditions: ["none"],
          expectedRoute: "plan_execute",
          requiredRuntimeEvidence: ["plan.created", "step.created"],
          requiredUiState: ["plan_draft_visible"],
          requiredControls: ["confirm_plan"],
          negativeAssertions: ["no_frontend_only_plan"],
          expectedOutcome: "pass",
          defaultGate: true,
        },
      ],
      proofs: [
        {
          scenarioId: "PI-01",
          passed: true,
          expectedBlocker: false,
          planId: "plan:phase-c",
          revision: 1,
          stepIds: ["step-1"],
          eventTypes: ["plan.created", "step.created"],
          linkedActionIds: [],
          linkedObservationIds: [],
          linkedProposalIds: [],
          blockerIds: [],
          controls: ["confirm_plan"],
          diagnostics: [],
        },
      ],
    });

    const result = await runMainChatAgentProductMaturityV2PlanGate();

    expect(invoke).toHaveBeenCalledWith(
      "run_main_chat_agent_product_maturity_v2_plan_gate",
      undefined
    );
    expect(result.ready).toBe(true);
    expect(result.scenarioCount).toBe(10);
    expect(result.expectedBlockerCount).toBe(3);
    expect(result.proofs[0]?.eventTypes).toContain("plan.created");
  });

  it("invokes Product Maturity v2 skills/tools gate and command-backed selectors", async () => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        scenarioCount: 8,
        defaultGateScenarioCount: 8,
        passedScenarioCount: 8,
        expectedBlockerCount: 2,
        ready: true,
        blockers: [],
        scenarios: [
          {
            id: "SK2-01",
            capabilityGroup: "skills_tools_surface",
            prompt: "Select a bounded local skill.",
            preconditions: ["local_skill_available"],
            expectedRoute: "direct_answer",
            requiredRuntimeEvidence: ["selected_skill.bounded_context"],
            requiredUiState: ["selected_skill_visible"],
            requiredControls: ["clear_skill"],
            negativeAssertions: ["skill_does_not_override_policy"],
            expectedOutcome: "pass",
            defaultGate: true,
          },
        ],
        proofs: [
          {
            scenarioId: "SK2-01",
            passed: true,
            expectedBlocker: false,
            runtimeObjectCount: 3,
            selectedSkillIds: ["phase_e_review"],
            candidateIds: ["project_status.read"],
            blockerIds: [],
            actionIds: [],
            observationIds: [],
            controls: ["clear_skill"],
            runtimeEvidence: ["selected_skill.bounded_context"],
            uiState: ["selected_skill_visible"],
            negativeAssertions: ["skill_does_not_override_policy"],
            diagnostics: [],
          },
        ],
      })
      .mockResolvedValueOnce([
        {
          skillId: "phase_e_review",
          name: "Phase E Review",
          source: "workspace:skills/phase_e_review/SKILL.md",
          scope: "session",
          description: "Review Main Chat Skill/Tool evidence.",
          riskLevel: "low",
          available: true,
          selected: false,
          instructionDigest:
            "bytes:80 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          sourceKind: "workspace",
          lastUsedAt: null,
        },
      ])
      .mockResolvedValueOnce({
        skillId: "phase_e_review",
        manifest: {
          name: "Phase E Review",
          source: "workspace:skills/phase_e_review/SKILL.md",
          sourceKind: "workspace",
          available: true,
        },
        boundedInstructionsPreview: "Use Phase E skill evidence as bounded context only.",
        allowedTools: ["project_status.read"],
        disallowedTools: ["email.send"],
        policyNotes: ["Selected SKILL.md is bounded context, not authority."],
        requiredPermissions: [],
        evidenceDigest:
          "bytes:120 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        redactionSummary: "bounded_preview_no_secrets",
        lastModifiedAt: "2026-06-17T00:00:00.000Z",
      })
      .mockResolvedValueOnce({
        sessionId: "session-42",
        selectedSkillId: "phase_e_review",
        selectedSkillDigest:
          "bytes:80 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        selectionReason: "user_selected_local_skill",
        boundedInstructionsPreview: "Use Phase E skill evidence as bounded context only.",
        evidenceDigest:
          "bytes:120 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        policyNotes: ["Selected SKILL.md is bounded context, not authority."],
        includedAsBoundedContextOnly: true,
        unselectedSkillsInjected: false,
        controls: ["clear_skill"],
      })
      .mockResolvedValueOnce({
        sessionId: "session-42",
        selectedSkillId: null,
        selectedSkillDigest: null,
        selectionReason: "user_cleared_local_skill",
        boundedInstructionsPreview: "",
        evidenceDigest:
          "bytes:34 hash:sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        policyNotes: ["Next task context has no selected skill."],
        includedAsBoundedContextOnly: false,
        unselectedSkillsInjected: false,
        controls: ["select_skill"],
      })
      .mockResolvedValueOnce({
        taskSessionId: "task-42",
        candidates: [
          {
            candidateId: "project_status.read",
            toolName: "project_status.read",
            source: "registered_mcp:project",
            capabilityLabels: ["read"],
            riskLevel: "low",
            selectionReason: "query_match:project",
            policyDecision: "allow",
            requiresPermission: false,
            candidateDigest:
              "bytes:88 hash:sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            linkedActionId: null,
          },
        ],
        blockedTools: [
          {
            toolName: "email.send",
            reasonCode: "write_like_tool_blocked",
            policyDecision: "permission_required",
            requiresPermission: true,
            blockerId: "blocker-email-send",
          },
        ],
        failureRecovery: null,
        evidenceDigest:
          "bytes:142 hash:sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        controls: [],
      });

    const report = await runMainChatAgentProductMaturityV2SkillsGate();
    const skills = await listMainChatSkills("session-42");
    const detail = await getMainChatSkillDetail("phase_e_review");
    const selected = await selectMainChatSkill("session-42", "phase_e_review");
    const cleared = await clearMainChatSkill("session-42");
    const tools = await listMainChatToolCandidates("task-42");

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "run_main_chat_agent_product_maturity_v2_skills_gate",
      undefined
    );
    expect(invoke).toHaveBeenNthCalledWith(2, "list_main_chat_skills", {
      sessionId: "session-42",
      session_id: "session-42",
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "get_main_chat_skill_detail", {
      skillId: "phase_e_review",
      skill_id: "phase_e_review",
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "select_main_chat_skill", {
      sessionId: "session-42",
      session_id: "session-42",
      skillId: "phase_e_review",
      skill_id: "phase_e_review",
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "clear_main_chat_skill", {
      sessionId: "session-42",
      session_id: "session-42",
    });
    expect(invoke).toHaveBeenNthCalledWith(6, "list_main_chat_tool_candidates", {
      taskSessionId: "task-42",
      task_session_id: "task-42",
    });
    expect(report.expectedBlockerCount).toBe(2);
    expect(skills[0]?.skillId).toBe("phase_e_review");
    expect(detail.allowedTools).toContain("project_status.read");
    expect(selected.includedAsBoundedContextOnly).toBe(true);
    expect(cleared.selectedSkillId).toBeNull();
    expect(tools.blockedTools[0]?.reasonCode).toBe("write_like_tool_blocked");
  });

  it("invokes Product Maturity v2 final readiness gate with deterministic and opt-in live readiness separated", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_agent_product_maturity_v2_final_readiness_gate",
      readinessSemantics:
        "phase_g_final_readiness_default_deterministic_live_product_opt_in_separate",
      defaultReadinessScope: "MR_EV_PI_LT2_SK2_deterministic_only",
      optInLiveReadinessScope: "LIVE_PROD_external_live_opt_in_only",
      finalReady: false,
      deterministicReady: true,
      optInLiveReady: false,
      finalReadinessStatus: "blocked_live_productization_not_ready",
      deterministicReadinessStatus: "ready",
      optInLiveReadinessStatus: "blocked",
      defaultDeterministicScenarioCount: 42,
      defaultLiveProdExcludedCount: 6,
      externalLiveScenarioCount: 6,
      defaultScenarioPassedCount: 32,
      defaultScenarioExpectedBlockerCount: 10,
      defaultScenarioFailedCount: 0,
      defaultScenarioBlockedCount: 0,
      externalLivePassedCount: 0,
      externalLiveBlockedCount: 6,
      externalLiveFailedCount: 0,
      phaseCounts: [
        {
          phaseId: "phase_a",
          phaseLabel: "Phase A Memory lifecycle",
          capabilityGroup: "memory_lifecycle",
          scenarioCount: 8,
          passed: 6,
          expectedBlocker: 2,
          failed: 0,
          blocked: 0,
          status: "ready",
          ready: true,
          defaultGate: true,
          optInOnly: false,
          blockers: [],
          supportedScenarios: ["MR-01", "MR-02", "MR-03", "MR-06", "MR-07", "MR-08"],
          blockedScenarios: ["MR-04", "MR-05"],
          unsupportedScenarios: [],
          futureScenarios: [],
        },
        {
          phaseId: "phase_f",
          phaseLabel: "Phase F External live product evidence",
          capabilityGroup: "external_live_productization",
          scenarioCount: 6,
          passed: 0,
          expectedBlocker: 0,
          failed: 0,
          blocked: 6,
          status: "blocked",
          ready: false,
          defaultGate: false,
          optInOnly: true,
          blockers: ["explicit_live_eval_required"],
          supportedScenarios: [],
          blockedScenarios: ["LIVE-PROD-01"],
          unsupportedScenarios: [],
          futureScenarios: [],
        },
      ],
      supportedScenarios: [
        {
          scenarioId: "MR-03",
          phaseId: "phase_a",
          capabilityGroup: "memory_lifecycle",
          status: "supported",
          reason: "passed",
        },
      ],
      blockedScenarios: [
        {
          scenarioId: "LIVE-PROD-01",
          phaseId: "phase_f",
          capabilityGroup: "external_live_productization",
          status: "blocked",
          reason: "explicit_live_eval_required",
        },
      ],
      unsupportedScenarios: [],
      futureScenarios: [],
      blockers: ["explicit_live_eval_required"],
      deterministicBlockers: [],
      optInLiveBlockers: ["explicit_live_eval_required"],
      directWritesExecuted: false,
      noSilentDurableWrites: true,
      defaultLiveProdExcluded: true,
    });

    const result = await runMainChatAgentProductMaturityV2FinalReadinessGate();

    expect(invoke).toHaveBeenCalledWith(
      "run_main_chat_agent_product_maturity_v2_final_readiness_gate",
      undefined
    );
    expect(result.deterministicReady).toBe(true);
    expect(result.optInLiveReady).toBe(false);
    expect(result.finalReadinessStatus).toBe("blocked_live_productization_not_ready");
    expect(result.defaultLiveProdExcludedCount).toBe(6);
    expect(result.unsupportedScenarios).toEqual([]);
    expect(result.phaseCounts[0]?.passed).toBe(6);
    expect(result.phaseCounts[0]?.expectedBlocker).toBe(2);
    expect(result.phaseCounts[1]?.blocked).toBe(6);
  });

  it("invokes runtime strategy registry status as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "multi_strategy_runtime_maturity",
      maturityReady: true,
      defaultChatUnchanged: true,
      migrationPermission: false,
      noRuntimeModelToolExecution: true,
      noBusinessWrites: true,
      registryReadiness: {
        ready: true,
        executableStrategyCount: 2,
        blockingReasons: [],
      },
      executableStrategies: [],
      futureStrategyDescriptors: [],
      statusCommandSideEffectBudget: {
        runtimeCalls: 0,
        modelCalls: 0,
        toolCalls: 0,
      },
      blockingReasons: [],
      metadataSafe: true,
      metadataSafeSummary: {},
    });

    const result = await getRuntimeStrategyRegistryStatus();

    expect(invoke).toHaveBeenCalledWith("get_runtime_strategy_registry_status", undefined);
    expect(result.maturityReady).toBe(true);
    expect(result.defaultChatUnchanged).toBe(true);
    expect(result.migrationPermission).toBe(false);
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

  it("invokes default chat adapter activation plan draft as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      draftReady: true,
      candidatePromotionReadinessReport: {
        ready: true,
        cutoverReadinessEligible: true,
        requiredApprovedCandidates: 1,
        approvedCandidateCount: 1,
        latestDecision: null,
        approvedCandidates: [],
        defaultChatUnchanged: true,
        blockingReasons: [],
        metadataSafeSummary: {
          metadataSafe: true,
          readOnly: true,
        },
        checkedAt: "2026-05-31T08:10:00Z",
      },
      runtimeBoundaryStatus: {
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
        },
      },
      activationScope: ["human review only"],
      requiredPreconditions: ["W33 ready"],
      adapterContractChecks: ["send_message-compatible"],
      fallbackPlan: ["use legacy stream"],
      rollbackPlan: ["revert separate implementation"],
      observabilityPlan: ["metadata-safe counters"],
      testPlan: ["verify send_message and start_stream_message"],
      manualReviewRequired: true,
      notAutomaticMigration: true,
      requiresSeparateImplementation: true,
      blockingReasons: [],
      metadataSafeSummary: {
        activationPlan: "default_chat_adapter_activation",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await draftDefaultChatAdapterActivationPlan({
      requiredApprovedCandidates: 2,
    });

    expect(invoke).toHaveBeenCalledWith("draft_default_chat_adapter_activation_plan", {
      input: {
        requiredApprovedCandidates: 2,
      },
    });
    expect(result.draftReady).toBe(true);
    expect(result.manualReviewRequired).toBe(true);
    expect(result.requiresSeparateImplementation).toBe(true);
    expect(result.activationScope[0]).toContain("human review");
  });

  it("invokes default chat adapter activation review decision explicitly", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_activation_review_1",
      decisionKind: "approve",
      draftReady: true,
      activationPlanDigest: "sha256:activation-plan",
      createdAt: "2026-05-31T10:11:12Z",
      blockingReasons: [],
    });

    const result = await recordDefaultChatAdapterActivationReviewDecision({
      decisionKind: "approve",
      requiredApprovedCandidates: 1,
      optionalReviewerNote: "Reviewed manually.",
    });

    expect(invoke).toHaveBeenCalledWith("record_default_chat_adapter_activation_review_decision", {
      input: {
        decisionKind: "approve",
        requiredApprovedCandidates: 1,
        optionalReviewerNote: "Reviewed manually.",
      },
    });
    expect(result.recorded).toBe(true);
    expect(result.activationPlanDigest).toBe("sha256:activation-plan");
  });

  it("invokes default chat adapter activation review summary as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_activation_review_2",
        decisionKind: "request_rework",
        draftReady: true,
        activationPlanDigest: "sha256:activation-plan-2",
        candidatePromotionReady: true,
        currentMode: "legacy_stream",
        automaticMigrationEnabled: false,
        reviewerNoteChecksum: "sha256:reviewer-note",
        reviewerNoteLength: 18,
        reviewerNoteCategory: "brief",
        createdAt: "2026-05-31T11:12:13Z",
      },
      approvedCount: 1,
      rejectOrReworkCount: 1,
      latestTimestamp: "2026-05-31T11:12:13Z",
      blockingReasons: [],
      metadataSafeSummary: {
        activationReview: "default_chat_adapter_activation",
        readOnly: true,
      },
    });

    const result = await getDefaultChatAdapterActivationReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_default_chat_adapter_activation_review_summary",
      undefined
    );
    expect(result.latestDecision?.decisionKind).toBe("request_rework");
    expect(result.rejectOrReworkCount).toBe(1);
  });

  it("invokes default chat adapter activation implementation gate as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      implementationGateEligible: true,
      draftReady: true,
      latestDecision: {
        evidenceId: "ev_activation_review_3",
        decisionKind: "approve",
        draftReady: true,
        activationPlanDigest: "sha256:activation-plan-3",
        candidatePromotionReady: true,
        currentMode: "legacy_stream",
        automaticMigrationEnabled: false,
        reviewerNoteChecksum: null,
        reviewerNoteLength: 0,
        reviewerNoteCategory: "none",
        createdAt: "2026-05-31T12:13:14Z",
      },
      currentActivationPlanDigest: "sha256:activation-plan-3",
      activationPlanDigestMatched: true,
      defaultChatUnchanged: true,
      automaticMigrationEnabled: false,
      currentMode: "legacy_stream",
      blockingReasons: [],
      metadataSafeSummary: {
        activationImplementationGate: "default_chat_adapter_activation",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await checkDefaultChatAdapterActivationImplementationGate({
      requiredApprovedCandidates: 1,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_activation_implementation_gate",
      {
        input: {
          requiredApprovedCandidates: 1,
          sessionId: "session-1",
        },
      }
    );
    expect(result.implementationGateEligible).toBe(true);
    expect(result.activationPlanDigestMatched).toBe(true);
    expect(result.latestDecision?.decisionKind).toBe("approve");
  });

  it("invokes default chat adapter routing status as read-only disabled scaffold", async () => {
    vi.mocked(invoke).mockResolvedValue({
      currentMode: "legacy_stream",
      adapterScaffoldPresent: true,
      controlledAdapterEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      activationImplementationGateEligible: true,
      requiresSeparateCutoverImplementation: true,
      blockingReasons: [],
      metadataSafeSummary: {
        defaultChatAdapterRouting: "disabled_scaffold",
        metadataSafe: true,
        readOnly: true,
        routingMode: "legacy_stream",
      },
    });

    const result = await getDefaultChatAdapterRoutingStatus({
      requiredApprovedCandidates: 1,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("get_default_chat_adapter_routing_status", {
      input: {
        requiredApprovedCandidates: 1,
        sessionId: "session-1",
      },
    });
    expect(result.currentMode).toBe("legacy_stream");
    expect(result.adapterScaffoldPresent).toBe(true);
    expect(result.controlledAdapterEnabled).toBe(false);
    expect(result.defaultSendPath).toBe("legacy_stream");
    expect(result.startStreamPath).toBe("legacy_stream");
  });

  it("invokes default chat adapter ordinary entry preflight status as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      statusReady: true,
      defaultChatUnchanged: true,
      currentMode: "legacy_stream",
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      sendMessagePreflight: {
        callsite: "send_message",
        preflightReady: true,
        contractReady: true,
        legacyEntryAllowed: true,
        ordinaryEntryPath: "legacy_stream",
        requiredEntryPath: "legacy_stream",
        contractShape: "send_message_compatible",
        sideEffectLockEngaged: true,
        defaultChatMigrationAllowed: false,
        controlledAdapterExecutorAttached: false,
        runtimeCallEnabled: false,
        modelCallEnabled: false,
        toolCallEnabled: false,
        allowWrites: false,
        maxToolCalls: 0,
        chatMessageSaved: false,
        agentRunRecorded: false,
        evidenceRecorded: false,
        blockingReasons: [],
      },
      streamMessagePreflight: {
        callsite: "start_stream_message",
        preflightReady: true,
        contractReady: true,
        legacyEntryAllowed: true,
        ordinaryEntryPath: "legacy_stream",
        requiredEntryPath: "legacy_stream",
        contractShape: "stream_message_compatible",
        sideEffectLockEngaged: true,
        defaultChatMigrationAllowed: false,
        controlledAdapterExecutorAttached: false,
        runtimeCallEnabled: false,
        modelCallEnabled: false,
        toolCallEnabled: false,
        allowWrites: false,
        maxToolCalls: 0,
        chatMessageSaved: false,
        agentRunRecorded: false,
        evidenceRecorded: false,
        blockingReasons: [],
      },
      blockingReasons: [],
      metadataSafeSummary: {
        ordinaryEntryPreflight: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
        statusReady: true,
      },
    });

    const result = await getDefaultChatAdapterOrdinaryEntryPreflightStatus();

    expect(invoke).toHaveBeenCalledWith(
      "get_default_chat_adapter_ordinary_entry_preflight_status",
      undefined
    );
    expect(result.statusReady).toBe(true);
    expect(result.sendMessagePreflight.callsite).toBe("send_message");
    expect(result.streamMessagePreflight.callsite).toBe("start_stream_message");
    expect(result.metadataSafeSummary.readOnly).toBe(true);
  });

  it("checks default chat adapter narrow implementation discussion gate as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      eligible: true,
      defaultChatUnchanged: true,
      cutoverPlanApprovalReady: true,
      ordinaryEntryPreflightStatusReady: true,
      sendPreflightReady: true,
      streamPreflightReady: true,
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      blockingReasons: [],
      metadataSafeSummary: {
        narrowImplementationDiscussionGate: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
        eligible: true,
        notAutomaticMigration: true,
      },
    });

    const result = await checkDefaultChatAdapterNarrowImplementationDiscussionGate({
      sourceSessionId: "session-1",
      message: "settings probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
      requiredPromotions: 3,
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_narrow_implementation_discussion_gate",
      {
        input: {
          sourceSessionId: "session-1",
          message: "settings probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
          requiredPromotions: 3,
        },
      }
    );
    expect(result.eligible).toBe(true);
    expect(result.cutoverPlanApprovalReady).toBe(true);
    expect(result.ordinaryEntryPreflightStatusReady).toBe(true);
    expect(result.metadataSafeSummary.notAutomaticMigration).toBe(true);
  });

  it("drafts default chat adapter narrow implementation plan", async () => {
    vi.mocked(invoke).mockResolvedValue({
      draftReady: true,
      discussionGate: {
        eligible: true,
        defaultChatUnchanged: true,
        cutoverPlanApprovalReady: true,
        ordinaryEntryPreflightStatusReady: true,
        sendPreflightReady: true,
        streamPreflightReady: true,
        controlledAdapterEnabled: false,
        automaticMigrationEnabled: false,
        defaultSendPath: "legacy_stream",
        startStreamPath: "legacy_stream",
        blockingReasons: [],
        metadataSafeSummary: {
          narrowImplementationDiscussionGate: "default_chat_adapter",
          metadataSafe: true,
          readOnly: true,
        },
      },
      manualReviewRequired: true,
      notAutomaticMigration: true,
      requiresSeparateImplementation: true,
      requiresSeparateCutoverReview: true,
      sourceSessionId: "session-1",
      inputMessageLength: 22,
      inputMessageHash: "sha256:message123",
      stablePlanDigest: "sha256:narrow-plan123",
      planSections: [
        {
          sectionKey: "implementationScope",
          title: "Implementation Scope",
          items: ["Keep default Chat unchanged."],
        },
      ],
      blockingReasons: [],
      metadataSafeSummary: {
        narrowImplementationPlan: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
        notAutomaticMigration: true,
      },
    });

    const result = await draftDefaultChatAdapterNarrowImplementationPlan({
      sourceSessionId: "session-1",
      message: "narrow plan probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
      requiredPromotions: 3,
    });

    expect(invoke).toHaveBeenCalledWith("draft_default_chat_adapter_narrow_implementation_plan", {
      input: {
        sourceSessionId: "session-1",
        message: "narrow plan probe",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
        requiredPromotions: 3,
      },
    });
    expect(result.draftReady).toBe(true);
    expect(result.discussionGate.eligible).toBe(true);
    expect(result.planSections[0]?.sectionKey).toBe("implementationScope");
  });

  it("records default chat adapter narrow implementation plan review decisions", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "evidence-narrow-plan-review",
      decisionKind: "approve",
      sourceSessionId: "session-1",
      draftReady: true,
      narrowPlanDigest: "sha256:narrow-plan123",
      planSectionCount: 8,
      createdAt: "2026-06-02T00:00:00Z",
      blockingReasons: [],
    });

    const result = await recordDefaultChatAdapterNarrowImplementationPlanReviewDecision({
      decisionKind: "approve",
      sourceSessionId: "session-1",
      message: "narrow plan review probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
      requiredPromotions: 3,
      optionalReviewerNote: "approved after review",
    });

    expect(invoke).toHaveBeenCalledWith(
      "record_default_chat_adapter_narrow_implementation_plan_review_decision",
      {
        input: {
          decisionKind: "approve",
          sourceSessionId: "session-1",
          message: "narrow plan review probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
          requiredPromotions: 3,
          optionalReviewerNote: "approved after review",
        },
      }
    );
    expect(result.recorded).toBe(true);
    expect(result.narrowPlanDigest).toBe("sha256:narrow-plan123");
  });

  it("loads default chat adapter narrow implementation plan review summary", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "evidence-narrow-plan-review",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        draftReady: true,
        narrowPlanDigest: "sha256:narrow-plan123",
        planSectionCount: 8,
        w57Eligible: true,
        reviewerNoteChecksum: "sha256:note",
        reviewerNoteLength: 21,
        reviewerNoteCategory: "brief",
        createdAt: "2026-06-02T00:00:00Z",
      },
      approvedCount: 1,
      rejectedCount: 0,
      requestReworkCount: 0,
      latestApprovedPlanDigest: "sha256:narrow-plan123",
      latestTimestamp: "2026-06-02T00:00:00Z",
      blockingReasons: [],
      metadataSafeSummary: {
        narrowImplementationPlanReview: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await getDefaultChatAdapterNarrowImplementationPlanReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_default_chat_adapter_narrow_implementation_plan_review_summary",
      undefined
    );
    expect(result.approvedCount).toBe(1);
    expect(result.latestDecision?.decisionKind).toBe("approve");
  });

  it("checks default chat adapter narrow implementation plan approval readiness", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ready: true,
      draftReady: true,
      discussionGateEligible: true,
      narrowPlanReviewApproved: true,
      narrowPlanDigestMatched: true,
      currentPlanDigest: "sha256:narrow-plan123",
      latestApprovedPlanDigest: "sha256:narrow-plan123",
      latestDecision: {
        evidenceId: "evidence-narrow-plan-review",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        draftReady: true,
        narrowPlanDigest: "sha256:narrow-plan123",
        planSectionCount: 8,
        w57Eligible: true,
        reviewerNoteChecksum: "sha256:note",
        reviewerNoteLength: 21,
        reviewerNoteCategory: "brief",
        createdAt: "2026-06-02T00:00:00Z",
      },
      defaultChatUnchanged: true,
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      blockingReasons: [],
      metadataSafeSummary: {
        narrowImplementationPlanApprovalReadiness: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
        notAutomaticMigration: true,
      },
    });

    const result = await checkDefaultChatAdapterNarrowImplementationPlanApprovalReadiness({
      sourceSessionId: "session-1",
      message: "narrow plan approval readiness probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
      requiredPromotions: 3,
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_narrow_implementation_plan_approval_readiness",
      {
        input: {
          sourceSessionId: "session-1",
          message: "narrow plan approval readiness probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
          requiredPromotions: 3,
        },
      }
    );
    expect(result.ready).toBe(true);
    expect(result.narrowPlanDigestMatched).toBe(true);
    expect(result.metadataSafeSummary.notAutomaticMigration).toBe(true);
  });

  it("invokes default chat adapter contract harness as read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({
      contractHarnessReady: true,
      contractShape: "disabled_adapter_legacy_stream_contract",
      adapterDisabled: true,
      activationImplementationGateEligible: true,
      routingStatus: {
        currentMode: "legacy_stream",
        adapterScaffoldPresent: true,
        controlledAdapterEnabled: false,
        defaultSendPath: "legacy_stream",
        startStreamPath: "legacy_stream",
        activationImplementationGateEligible: true,
        requiresSeparateCutoverImplementation: true,
        blockingReasons: [],
        metadataSafeSummary: {},
      },
      sendMessageContract: {
        name: "send_message",
        ready: true,
        expectedPath: "legacy_stream",
        actualPath: "legacy_stream",
        blockingReasons: [],
      },
      streamMessageContract: {
        name: "start_stream_message",
        ready: true,
        expectedPath: "legacy_stream",
        actualPath: "legacy_stream",
        blockingReasons: [],
      },
      blockingReasons: [],
      metadataSafeSummary: {
        contractHarness: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await checkDefaultChatAdapterContractHarness({
      requiredApprovedCandidates: 1,
      sessionId: "session-1",
    });

    expect(invoke).toHaveBeenCalledWith("check_default_chat_adapter_contract_harness", {
      input: {
        requiredApprovedCandidates: 1,
        sessionId: "session-1",
      },
    });
    expect(result.contractHarnessReady).toBe(true);
    expect(result.adapterDisabled).toBe(true);
    expect(result.sendMessageContract.actualPath).toBe("legacy_stream");
    expect(result.streamMessageContract.actualPath).toBe("legacy_stream");
  });

  it("invokes default chat adapter dry run as write-disabled", async () => {
    vi.mocked(invoke).mockResolvedValue({
      dryRunReady: true,
      blocked: false,
      contractShape: "default_chat_adapter_dry_run_contract",
      sourceSessionId: "session-1",
      adapterPath: "controlled_adapter_dry_run",
      allowWrites: false,
      maxToolCalls: 0,
      defaultChatPathUnchanged: true,
      chatMessageSaved: false,
      agentRunRecorded: false,
      contractHarnessReady: true,
      inputMessageLength: 13,
      inputMessageHash: "abc123",
      blockingReasons: [],
      metadataSafeSummary: {
        adapterDryRun: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await runDefaultChatAdapterDryRun({
      sessionId: "session-1",
      message: "dry run probe",
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith("run_default_chat_adapter_dry_run", {
      input: {
        sessionId: "session-1",
        message: "dry run probe",
        requiredApprovedCandidates: 1,
      },
    });
    expect(result.dryRunReady).toBe(true);
    expect(result.allowWrites).toBe(false);
    expect(result.maxToolCalls).toBe(0);
    expect(result.chatMessageSaved).toBe(false);
  });

  it("records default chat adapter dry-run review decision", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_dry_run_review_1",
      decisionKind: "approve",
      sourceSessionId: "session-1",
      contractShape: "default_chat_adapter_dry_run_contract",
      dryRunReady: true,
      dryRunSummaryDigest: "sha256:abc123",
      createdAt: "2026-05-31T00:00:00Z",
      blockingReasons: [],
    });

    const result = await recordDefaultChatAdapterDryRunReviewDecision({
      decisionKind: "approve",
      sourceSessionId: "session-1",
      message: "dry run probe",
      dryRunSummaryDigest: "sha256:abc123",
      requiredApprovedCandidates: 1,
      optionalReviewerNote: "reviewed",
    });

    expect(invoke).toHaveBeenCalledWith("record_default_chat_adapter_dry_run_review_decision", {
      input: {
        decisionKind: "approve",
        sourceSessionId: "session-1",
        message: "dry run probe",
        dryRunSummaryDigest: "sha256:abc123",
        requiredApprovedCandidates: 1,
        optionalReviewerNote: "reviewed",
      },
    });
    expect(result.recorded).toBe(true);
    expect(result.dryRunReady).toBe(true);
    expect(result.evidenceId).toBe("ev_dry_run_review_1");
  });

  it("reads default chat adapter dry-run review summary", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_dry_run_review_1",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        contractShape: "default_chat_adapter_dry_run_contract",
        dryRunReady: true,
        dryRunSummaryDigest: "sha256:abc123",
        reviewerNoteChecksum: "sha256:def456",
        reviewerNoteLength: 8,
        reviewerNoteCategory: "short",
        createdAt: "2026-05-31T00:00:00Z",
      },
      approvedCount: 1,
      rejectOrReworkCount: 0,
      latestTimestamp: "2026-05-31T00:00:00Z",
      blockingReasons: [],
      metadataSafeSummary: {
        dryRunReview: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await getDefaultChatAdapterDryRunReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_default_chat_adapter_dry_run_review_summary",
      undefined
    );
    expect(result.latestDecision?.decisionKind).toBe("approve");
    expect(result.approvedCount).toBe(1);
  });

  it("checks default chat adapter implementation readiness", async () => {
    vi.mocked(invoke).mockResolvedValue({
      implementationReady: true,
      latestDryRunReviewDecision: {
        evidenceId: "ev_dry_run_review_1",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        contractShape: "default_chat_adapter_dry_run_contract",
        dryRunReady: true,
        dryRunSummaryDigest: "sha256:abc123",
        reviewerNoteChecksum: "sha256:def456",
        reviewerNoteLength: 8,
        reviewerNoteCategory: "short",
        createdAt: "2026-05-31T00:00:00Z",
      },
      activationImplementationGateEligible: true,
      contractHarnessReady: true,
      dryRunReady: true,
      dryRunReviewApproved: true,
      dryRunDigestMatched: true,
      defaultChatUnchanged: true,
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      blockingReasons: [],
      metadataSafeSummary: {
        implementationReadiness: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
        implementationReady: true,
      },
    });

    const result = await checkDefaultChatAdapterImplementationReadiness({
      sourceSessionId: "session-1",
      message: "implementation probe",
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith("check_default_chat_adapter_implementation_readiness", {
      input: {
        sourceSessionId: "session-1",
        message: "implementation probe",
        requiredApprovedCandidates: 1,
      },
    });
    expect(result.implementationReady).toBe(true);
    expect(result.dryRunReviewApproved).toBe(true);
  });

  it("runs default chat adapter controlled preview", async () => {
    vi.mocked(invoke).mockResolvedValue({
      previewReady: true,
      blocked: false,
      contractShape: "send_message_compatible",
      sourceSessionId: "session-1",
      adapterPath: "controlled_adapter_preview",
      reply: "Controlled adapter preview reply",
      reasoningTrace: {
        strategyResult: {
          adapterPreview: "default_chat_adapter_controlled_preview",
          metadataSafe: true,
        },
      },
      toolCalls: [],
      runId: "run-adapter-preview-1",
      allowWrites: false,
      maxToolCalls: 0,
      defaultChatPathUnchanged: true,
      chatMessageSaved: false,
      agentRunRecorded: true,
      implementationReady: true,
      warnings: [],
      blockingReasons: [],
      metadataSafeSummary: {
        adapterPreview: "default_chat_adapter_controlled_preview",
        metadataSafe: true,
        allowWrites: false,
        maxToolCalls: 0,
      },
    });

    const result = await runDefaultChatAdapterControlledPreview({
      sourceSessionId: "session-1",
      message: "implementation preview probe",
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith("run_default_chat_adapter_controlled_preview", {
      input: {
        sourceSessionId: "session-1",
        message: "implementation preview probe",
        requiredApprovedCandidates: 1,
      },
    });
    expect(result.previewReady).toBe(true);
    expect(result.reply).toBe("Controlled adapter preview reply");
    expect(result.metadataSafeSummary.adapterPreview).toBe(
      "default_chat_adapter_controlled_preview"
    );
  });

  it("records default chat adapter controlled preview review decisions", async () => {
    vi.mocked(invoke).mockResolvedValue({
      recorded: true,
      evidenceId: "ev_adapter_preview_review_1",
      previewRunId: "run-adapter-preview-1",
      decisionKind: "approve",
      contractShape: "send_message_compatible",
      previewSummaryDigest: "sha256:preview123",
      createdAt: "2026-05-31T00:00:00Z",
      blockingReasons: [],
    });

    const result = await recordDefaultChatAdapterControlledPreviewReviewDecision({
      previewRunId: "run-adapter-preview-1",
      decisionKind: "approve",
      optionalReviewerNote: "Looks safe.",
    });

    expect(invoke).toHaveBeenCalledWith(
      "record_default_chat_adapter_controlled_preview_review_decision",
      {
        input: {
          previewRunId: "run-adapter-preview-1",
          decisionKind: "approve",
          optionalReviewerNote: "Looks safe.",
        },
      }
    );
    expect(result.recorded).toBe(true);
    expect(result.previewRunId).toBe("run-adapter-preview-1");
  });

  it("loads default chat adapter controlled preview review summary", async () => {
    vi.mocked(invoke).mockResolvedValue({
      latestDecision: {
        evidenceId: "ev_adapter_preview_review_1",
        previewRunId: "run-adapter-preview-1",
        decisionKind: "approve",
        contractShape: "send_message_compatible",
        previewSummaryDigest: "sha256:preview123",
        reviewerNoteChecksum: "sha256:note123",
        reviewerNoteLength: 11,
        reviewerNoteCategory: "brief",
        createdAt: "2026-05-31T00:00:00Z",
      },
      approvedCount: 1,
      rejectOrReworkCount: 0,
      latestTimestamp: "2026-05-31T00:00:00Z",
      blockingReasons: [],
      metadataSafeSummary: {
        controlledPreviewReview: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await getDefaultChatAdapterControlledPreviewReviewSummary();

    expect(invoke).toHaveBeenCalledWith(
      "get_default_chat_adapter_controlled_preview_review_summary",
      undefined
    );
    expect(result.approvedCount).toBe(1);
    expect(result.latestDecision?.previewRunId).toBe("run-adapter-preview-1");
  });

  it("checks default chat adapter controlled preview approval readiness", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ready: true,
      requiredApprovedPreviews: 1,
      approvedPreviewCount: 1,
      latestDecision: {
        evidenceId: "ev_adapter_preview_review_1",
        previewRunId: "run-adapter-preview-1",
        decisionKind: "approve",
        contractShape: "send_message_compatible",
        previewSummaryDigest: "sha256:preview123",
        reviewerNoteChecksum: "sha256:note123",
        reviewerNoteLength: 11,
        reviewerNoteCategory: "brief",
        createdAt: "2026-05-31T00:00:00Z",
      },
      verifiedPreviewRunIds: ["run-adapter-preview-1"],
      implementationReadinessReady: true,
      previewReviewApproved: true,
      previewDigestMatched: true,
      defaultChatUnchanged: true,
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      blockingReasons: [],
      metadataSafeSummary: {
        controlledPreviewApprovalReadiness: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await checkDefaultChatAdapterControlledPreviewApprovalReadiness({
      sourceSessionId: "session-1",
      message: "approval readiness probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_controlled_preview_approval_readiness",
      {
        input: {
          sourceSessionId: "session-1",
          message: "approval readiness probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
        },
      }
    );
    expect(result.ready).toBe(true);
    expect(result.previewReviewApproved).toBe(true);
    expect(result.verifiedPreviewRunIds).toEqual(["run-adapter-preview-1"]);
  });

  it("drafts default chat adapter cutover implementation plan", async () => {
    vi.mocked(invoke).mockResolvedValue({
      draftReady: true,
      controlledPreviewApprovalReadiness: {
        ready: true,
        requiredApprovedPreviews: 1,
        approvedPreviewCount: 1,
        latestDecision: null,
        verifiedPreviewRunIds: ["run-adapter-preview-1"],
        implementationReadinessReady: true,
        previewReviewApproved: true,
        previewDigestMatched: true,
        defaultChatUnchanged: true,
        controlledAdapterEnabled: false,
        automaticMigrationEnabled: false,
        defaultSendPath: "legacy_stream",
        startStreamPath: "legacy_stream",
        blockingReasons: [],
        metadataSafeSummary: {
          controlledPreviewApprovalReadiness: "default_chat_adapter",
          metadataSafe: true,
        },
      },
      manualReviewRequired: true,
      notAutomaticMigration: true,
      requiresSeparateImplementation: true,
      requiresSeparateCutoverReview: true,
      sourceSessionId: "session-1",
      inputMessageLength: 22,
      inputMessageHash: "sha256:message123",
      stablePlanDigest: "sha256:plan123",
      planSections: [
        {
          sectionKey: "implementationScope",
          title: "Implementation Scope",
          items: ["Keep default Chat unchanged."],
        },
      ],
      blockingReasons: [],
      metadataSafeSummary: {
        cutoverImplementationPlan: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const result = await draftDefaultChatAdapterCutoverImplementationPlan({
      sourceSessionId: "session-1",
      message: "cutover plan probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith("draft_default_chat_adapter_cutover_implementation_plan", {
      input: {
        sourceSessionId: "session-1",
        message: "cutover plan probe",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      },
    });
    expect(result.draftReady).toBe(true);
    expect(result.stablePlanDigest).toBe("sha256:plan123");
    expect(result.planSections[0].sectionKey).toBe("implementationScope");
  });

  it("records and reads default chat adapter cutover plan review decisions", async () => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        recorded: true,
        evidenceId: "ev_cutover_plan_review_1",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        draftReady: true,
        cutoverPlanDigest: "sha256:plan123",
        planSectionCount: 9,
        createdAt: "2026-06-01T00:00:00Z",
        blockingReasons: [],
      })
      .mockResolvedValueOnce({
        latestDecision: {
          evidenceId: "ev_cutover_plan_review_1",
          decisionKind: "approve",
          sourceSessionId: "session-1",
          draftReady: true,
          cutoverPlanDigest: "sha256:plan123",
          planSectionCount: 9,
          reviewerNoteChecksum: "sha256:note123",
          reviewerNoteLength: 12,
          reviewerNoteCategory: "brief",
          createdAt: "2026-06-01T00:00:00Z",
        },
        approvedCount: 1,
        rejectedCount: 0,
        requestReworkCount: 0,
        latestApprovedPlanDigest: "sha256:plan123",
        latestTimestamp: "2026-06-01T00:00:00Z",
        blockingReasons: [],
        metadataSafeSummary: {
          cutoverPlanReview: "default_chat_adapter",
          metadataSafe: true,
          readOnly: true,
        },
      });

    const result = await recordDefaultChatAdapterCutoverPlanReviewDecision({
      decisionKind: "approve",
      sourceSessionId: "session-1",
      message: "cutover plan probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
      optionalReviewerNote: "review note",
    });
    const summary = await getDefaultChatAdapterCutoverPlanReviewSummary();

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "record_default_chat_adapter_cutover_plan_review_decision",
      {
        input: {
          decisionKind: "approve",
          sourceSessionId: "session-1",
          message: "cutover plan probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
          optionalReviewerNote: "review note",
        },
      }
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "get_default_chat_adapter_cutover_plan_review_summary",
      undefined
    );
    expect(result.recorded).toBe(true);
    expect(result.cutoverPlanDigest).toBe("sha256:plan123");
    expect(summary.latestApprovedPlanDigest).toBe("sha256:plan123");
  });

  it("checks default chat adapter cutover plan approval readiness", async () => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValueOnce({
      ready: true,
      draftReady: true,
      w45Ready: true,
      cutoverPlanReviewApproved: true,
      cutoverPlanDigestMatched: true,
      currentPlanDigest: "sha256:plan123",
      latestApprovedPlanDigest: "sha256:plan123",
      latestDecision: {
        evidenceId: "ev_cutover_plan_review_1",
        decisionKind: "approve",
        sourceSessionId: "session-1",
        draftReady: true,
        cutoverPlanDigest: "sha256:plan123",
        planSectionCount: 9,
        w45Ready: true,
        reviewerNoteChecksum: "sha256:note123",
        reviewerNoteLength: 12,
        reviewerNoteCategory: "brief",
        createdAt: "2026-06-01T00:00:00Z",
      },
      defaultChatUnchanged: true,
      controlledAdapterEnabled: false,
      automaticMigrationEnabled: false,
      defaultSendPath: "legacy_stream",
      startStreamPath: "legacy_stream",
      blockingReasons: [],
      metadataSafeSummary: {
        cutoverPlanApprovalReadiness: "default_chat_adapter",
        metadataSafe: true,
        readOnly: true,
      },
    });

    const report = await checkDefaultChatAdapterCutoverPlanApprovalReadiness({
      sourceSessionId: "session-1",
      message: "cutover approval probe",
      requiredApprovedPreviews: 1,
      requiredApprovedCandidates: 1,
    });

    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_cutover_plan_approval_readiness",
      {
        input: {
          sourceSessionId: "session-1",
          message: "cutover approval probe",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
        },
      }
    );
    expect(report.ready).toBe(true);
    expect(report.cutoverPlanDigestMatched).toBe(true);
    expect(report.currentPlanDigest).toBe("sha256:plan123");
  });
});
