import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  addDailyGoal,
  acceptProposal,
  builderStart,
  applyCalibration,
  confirmManagedKnowledgeWrite,
  createManagedKnowledgeWriteDraft,
  draftEditMemoryProposal,
  editProposal,
  getStateHistory,
  recordState,
  checkControlledChatPilotEligibility,
  checkControlledChatCutoverReadiness,
  checkControlledChatCutoverCandidatePromotionReadiness,
  checkControlledChatMigrationImplementationGate,
  checkControlledPilotPromotionReadiness,
  checkRuntimeMigrationGate,
  getMainChatRuntimeStatus,
  getRuntimeStrategyRegistryStatus,
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
  runMainChatAgentBetaV1ReadinessGate,
  runMainChatAgentStage1DogfoodGate,
  runMainChatAgentStage2ReadinessGate,
  validateMainChatAgentStage2ManualDogfoodArtifact,
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
  runMainChatStage3ExecutionUxReport,
  runMainChatStage4MemoryKnowledgeReport,
  evaluateMainChatStage5ReleaseDebugPreflight,
  exportMainChatAgentDebugBundle,
  createMainChatInternalIssueReport,
  listMainChatDebugBundles,
  getMainChatDebugBundle,
  deleteMainChatDebugBundle,
  listMainChatInternalIssueReports,
  getMainChatInternalIssueReport,
  deleteMainChatInternalIssueReport,
  runMainChatStage5ReleaseDebugReport,
  listStage4KnowledgeAssetInventory,
  rollbackManagedKnowledgeWrite,
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
    await draftEditMemoryProposal("proposal-memory-1", { content: "draft" });

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
    expect(invoke).toHaveBeenCalledWith(
      "draft_edit_memory_proposal",
      expect.objectContaining({
        proposalId: "proposal-memory-1",
        proposal_id: "proposal-memory-1",
        newAfter: { content: "draft" },
        new_after: { content: "draft" },
      })
    );
  });

  it("normalizes Stage 4 managed knowledge command arguments", async () => {
    await listStage4KnowledgeAssetInventory("review");
    await createManagedKnowledgeWriteDraft("USER.md", "profile", "proposal-1", ["memory:1"]);
    await confirmManagedKnowledgeWrite("proposal-managed-1");
    await rollbackManagedKnowledgeWrite("knowledge_version:1");

    expect(invoke).toHaveBeenCalledWith(
      "list_stage4_knowledge_asset_inventory",
      expect.objectContaining({
        selectedSkillId: "review",
        selected_skill_id: "review",
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "create_managed_knowledge_write_draft",
      expect.objectContaining({
        targetPath: "USER.md",
        target_path: "USER.md",
        afterContent: "profile",
        after_content: "profile",
        sourceProposalId: "proposal-1",
        source_proposal_id: "proposal-1",
        linkedMemoryIds: ["memory:1"],
        linked_memory_ids: ["memory:1"],
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "confirm_managed_knowledge_write",
      expect.objectContaining({
        proposalId: "proposal-managed-1",
        proposal_id: "proposal-managed-1",
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "rollback_managed_knowledge_write",
      expect.objectContaining({
        versionId: "knowledge_version:1",
        version_id: "knowledge_version:1",
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

  it("invokes Main Chat Stage 3 execution UX report without readiness semantics", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_stage3_execution_ux",
      schemaVersion: "stage3-execution-ux-v1",
      dataPath:
        "Main Chat send/stream -> AgentIngress / strategy route -> AgentTaskSession / ActionQueue / ExecutionTranscript / Main Chat event stream -> MainChatAgentStateSnapshot -> AgentControlPlane",
      totalScenarioCount: 13,
      passedScenarioCount: 13,
      failedScenarioCount: 0,
      blockedScenarioCount: 0,
      executionFirstRequiredIds: [
        "UX3-02",
        "UX3-03",
        "UX3-04",
        "UX3-06",
        "UX3-09",
        "UX3-11",
        "UX3-12",
      ],
      executionFirstPassedIds: [
        "UX3-02",
        "UX3-03",
        "UX3-04",
        "UX3-06",
        "UX3-09",
        "UX3-11",
        "UX3-12",
      ],
      executionFirstClaimValid: true,
      readyForLimitedInternalTrial: false,
      readinessRecommendation: "not_ready_for_limited_internal_trial",
      stage2ReadinessPreserved:
        "stage2_readiness_remains_fail_closed_without_manual_dogfood_and_current_commit_live_evidence",
      nonGoals: ["manual_dogfood_rows_not_run_or_fabricated"],
      coverage: Array.from({ length: 13 }, (_, index) => ({
        scenarioId: `UX3-${String(index + 1).padStart(2, "0")}`,
        scenario: "covered Stage 3 scenario",
        status: "passed",
        evidence: ["runtime-backed evidence"],
        blockers: [],
      })),
      blockers: [],
    });

    const result = await runMainChatStage3ExecutionUxReport();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_stage3_execution_ux_report", undefined);
    expect(result.totalScenarioCount).toBe(13);
    expect(result.coverage.map(row => row.scenarioId)).toContain("UX3-13");
    expect(result.readyForLimitedInternalTrial).toBe(false);
    expect(result.readinessRecommendation).toBe("not_ready_for_limited_internal_trial");
    expect(result.nonGoals).toContain("manual_dogfood_rows_not_run_or_fabricated");
  });

  it("invokes Main Chat Stage 4 memory knowledge report without readiness claim", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_stage4_memory_knowledge",
      schemaVersion: "stage4.v1",
      scenarioCount: 18,
      passedScenarioCount: 18,
      blockedScenarioCount: 0,
      notAReadinessGate: true,
      readinessClaim: false,
      stage2ReadinessPreserved: true,
      rows: [],
      evidenceIds: [],
      blockers: [],
      activeMemoryIds: [],
      excludedMemoryIds: [],
      loadedKnowledgeAssetIds: [],
      skippedKnowledgeAssetIds: [],
      managedKnowledgeWriteAssetIds: [],
      managedKnowledgeWriteVersionIds: [],
      managedKnowledgeWriteAuditIds: [],
      managedKnowledgeRollbackSnapshotIds: [],
      directWriteCount: 0,
      confirmedKnowledgeWriteCount: 0,
      rollbackEventCount: 0,
    });

    const result = await runMainChatStage4MemoryKnowledgeReport();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_stage4_memory_knowledge_report", undefined);
    expect(result.scenarioCount).toBe(18);
    expect(result.notAReadinessGate).toBe(true);
    expect(result.readinessClaim).toBe(false);
    expect(result.stage2ReadinessPreserved).toBe(true);
  });

  it("invokes Main Chat Stage 5 release/debug operations with metadata-safe arguments", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        reportKind: "main_chat_stage5_release_debug_preflight",
        schemaVersion: "stage5-preflight-v1",
        failure: { class: "environment_preflight_failure" },
        provider: { keyPresent: false },
        externalProviderInvokedByDefault: false,
        modelInvoked: false,
        directWritesExecuted: false,
        metadataSafe: true,
      })
      .mockResolvedValueOnce({
        bundleId: "stage5-bundle-test",
        schemaVersion: "stage5-debug-bundle-v1",
        task: { taskSessionId: "task-stage5", runId: "run-stage5" },
        failure: { class: "tool_selection_failure" },
        artifact: {
          artifactId: "stage5-bundle-test",
          storageAlias: "stage5/debug_bundles/stage5-bundle-test.json",
          byteSize: 2048,
        },
      })
      .mockResolvedValueOnce({
        reportId: "stage5-issue-test",
        schemaVersion: "stage5-issue-report-v1",
        notesPreview: null,
        artifact: { artifactId: "stage5-issue-test", byteSize: 512 },
      })
      .mockResolvedValueOnce([{ artifactId: "stage5-bundle-test" }])
      .mockResolvedValueOnce({ bundleId: "stage5-bundle-test" })
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce([{ artifactId: "stage5-issue-test" }])
      .mockResolvedValueOnce({ reportId: "stage5-issue-test" })
      .mockResolvedValueOnce(true)
      .mockResolvedValueOnce({
        reportKind: "main_chat_stage5_release_debug",
        schemaVersion: "stage5-release-debug-v1",
        scenarioCount: 24,
        passedScenarioCount: 12,
        blockedScenarioCount: 12,
        notAReadinessGate: true,
        readinessClaim: false,
        managedKnowledgeEval: {
          isolatedEvalAppState: true,
          tempWorkspace: true,
          realWorkspaceWriteExecuted: false,
          userWriteCompleted: true,
          memoryRollbackCompleted: true,
          managedKnowledgeWriteVersionIds: ["knowledge_version:test"],
          managedKnowledgeAuditIds: ["knowledge_audit:test"],
          rollbackSnapshotIds: ["snapshot:test"],
          evidenceIds: ["stage5_isolated_managed_knowledge_eval"],
          blockers: [],
        },
        stage2ReadinessPreserved: true,
      });

    const callStart = vi.mocked(invoke).mock.calls.length;
    const preflight = await evaluateMainChatStage5ReleaseDebugPreflight();
    const bundle = await exportMainChatAgentDebugBundle("task-stage5", {
      scenarioId: "DBG5-04",
      reviewerId: "tester-alpha",
      uiEvidence: {
        frontendRoute: "/companion",
        surface: "AgentControlPlane",
        visibleControlLabels: ["Export debug bundle"],
        taskSessionId: "task-stage5",
        timestamp: "2026-06-20T00:00:00Z",
      },
    });
    const issue = await createMainChatInternalIssueReport({
      scenarioId: "DBG5-19",
      reviewerId: "tester-alpha",
      status: "fail",
      taskSessionId: "task-stage5",
      runId: "run-stage5",
      bundleId: "stage5-bundle-test",
      failureClass: "tool_selection_failure",
      notes: "Authorization: Bearer sk-stage5-secret",
    });
    const bundles = await listMainChatDebugBundles();
    await getMainChatDebugBundle("stage5-bundle-test");
    await deleteMainChatDebugBundle("stage5-bundle-test");
    const issues = await listMainChatInternalIssueReports();
    await getMainChatInternalIssueReport("stage5-issue-test");
    await deleteMainChatInternalIssueReport("stage5-issue-test");
    const report = await runMainChatStage5ReleaseDebugReport();
    const calls = vi.mocked(invoke).mock.calls.slice(callStart);

    expect(calls[0]).toEqual(["evaluate_main_chat_stage5_release_debug_preflight", undefined]);
    expect(calls[1]).toEqual([
      "export_main_chat_agent_debug_bundle",
      expect.objectContaining({
        taskSessionId: "task-stage5",
        task_session_id: "task-stage5",
        scenarioId: "DBG5-04",
        scenario_id: "DBG5-04",
      }),
    ]);
    expect(calls[2]).toEqual([
      "create_main_chat_internal_issue_report",
      expect.objectContaining({
        input: expect.objectContaining({
          notes: "Authorization: Bearer sk-stage5-secret",
        }),
      }),
    ]);
    expect(calls[3]).toEqual(["list_main_chat_debug_bundles", undefined]);
    expect(calls[4]).toEqual([
      "get_main_chat_debug_bundle",
      expect.objectContaining({
        bundleId: "stage5-bundle-test",
        bundle_id: "stage5-bundle-test",
      }),
    ]);
    expect(calls[5]).toEqual([
      "delete_main_chat_debug_bundle",
      expect.objectContaining({
        bundleId: "stage5-bundle-test",
        bundle_id: "stage5-bundle-test",
      }),
    ]);
    expect(calls[6]).toEqual(["list_main_chat_internal_issue_reports", undefined]);
    expect(calls[7]).toEqual([
      "get_main_chat_internal_issue_report",
      expect.objectContaining({
        reportId: "stage5-issue-test",
        report_id: "stage5-issue-test",
      }),
    ]);
    expect(calls[8]).toEqual([
      "delete_main_chat_internal_issue_report",
      expect.objectContaining({
        reportId: "stage5-issue-test",
        report_id: "stage5-issue-test",
      }),
    ]);
    expect(calls[9]).toEqual(["run_main_chat_stage5_release_debug_report", undefined]);
    expect(preflight.externalProviderInvokedByDefault).toBe(false);
    expect(bundle.artifact.storageAlias).toMatch(/^stage5\/debug_bundles\//);
    expect(issue.notesPreview).toBeNull();
    expect(bundles).toHaveLength(1);
    expect(issues).toHaveLength(1);
    expect(report.notAReadinessGate).toBe(true);
    expect(report.readinessClaim).toBe(false);
    expect(report.managedKnowledgeEval.isolatedEvalAppState).toBe(true);
    expect(report.managedKnowledgeEval.realWorkspaceWriteExecuted).toBe(false);

    const redacted = redactInvokeArgs("create_main_chat_internal_issue_report", {
      input: {
        notes: "Authorization: Bearer sk-stage5-secret",
      },
    });
    expect(JSON.stringify(redacted)).not.toContain("sk-stage5-secret");
    expect(JSON.stringify(redacted)).not.toContain("Authorization");
  });

  it("invokes external live productization gate as opt-in non-default evidence", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_external_live_productization_gate",
      scenarioCount: 6,
      defaultGateScenarioCount: 0,
      readinessSemantics: "opt_in_external_live_product_evidence_only_default_readiness_unchanged",
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
      defaultDeterministicScenarioCount: 43,
      defaultLiveProdExcludedCount: 6,
      externalLiveScenarioCount: 6,
      defaultScenarioPassedCount: 33,
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

  it("invokes Main Chat Agent readiness gate with deterministic and live sections", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_agent_beta_v1_readiness_gate",
      readinessSemantics: "beta_v1_execution_first_default_deterministic_live_opt_in_separate",
      defaultReadinessScope: "beta_v1_default_deterministic_local_only",
      optInLiveReadinessScope: "beta_v1_external_live_opt_in_only",
      foundationInventoryExists: true,
      foundationInventoryItems: [
        {
          component: "Knowledge assets and context inventory",
          status: "partial",
          evidence: ["B27 inspection and B28 proposal-first edit evidence"],
          developmentDecision: "reuse minimum beta slice; broader manager deferred",
        },
      ],
      workstreams: [
        {
          workstreamId: "phase_5",
          label: "Capability Hardening",
          status: "ready",
          ready: true,
          evidence: ["structured readiness report and release notes"],
          blockers: [],
        },
      ],
      productMaturityPhaseCounts: [
        {
          phaseId: "phase_a",
          capabilityGroup: "memory_lifecycle",
          scenarioCount: 9,
          passed: 7,
          expectedBlocker: 2,
          failed: 0,
          blocked: 0,
          ready: true,
          optInOnly: false,
        },
      ],
      defaultReadinessStatus: "ready",
      defaultReady: true,
      optInLiveReady: false,
      externalLiveAttempted: false,
      defaultRealTaskScenarioCount: 28,
      defaultRealTaskPassedCount: 28,
      optInLiveRealTaskScenarioCount: 2,
      defaultExperienceRequiredStateCount: 11,
      defaultExperienceVerifiedStateCount: 11,
      productMaturityDefaultScenarioCount: 43,
      commandSurfaceTotalCases: 38,
      commandSurfaceFailedCases: 0,
      legacyFallbackCount: 0,
      silentDurableWriteCount: 0,
      noSilentDurableWrites: true,
      defaultBlockers: [],
      optInLiveBlockers: ["explicit_live_eval_required"],
      readinessDimensions: [
        {
          dimension: "Routing",
          status: "ready",
          optInOnly: false,
          evidence: ["governed task sessions and strategy routing"],
          blockers: [],
        },
        {
          dimension: "Live provider",
          status: "blocked_opt_in_not_attempted",
          optInOnly: true,
          evidence: ["external live evidence is opt-in and not run by default"],
          blockers: ["explicit_live_eval_required"],
        },
      ],
    });

    const result = await runMainChatAgentBetaV1ReadinessGate();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_agent_beta_v1_readiness_gate", undefined);
    expect(result.defaultReady).toBe(true);
    expect(result.defaultReadinessScope).toBe("beta_v1_default_deterministic_local_only");
    expect(result.optInLiveReady).toBe(false);
    expect(result.foundationInventoryItems[0]?.status).toBe("partial");
    expect(result.workstreams[0]?.workstreamId).toBe("phase_5");
    expect(result.productMaturityPhaseCounts[0]?.scenarioCount).toBe(9);
    expect(result.defaultRealTaskPassedCount).toBe(28);
    expect(result.productMaturityDefaultScenarioCount).toBe(43);
    expect(result.readinessDimensions.some(dimension => dimension.dimension === "Routing")).toBe(
      true
    );
  });

  it("invokes Main Chat Agent Stage 2 readiness gate with manual and live blockers", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_agent_stage2_readiness_gate",
      schemaVersion: "stage2-readiness-v1",
      runId: "stage2-readiness-test",
      commit: "abc123",
      recommendation: "not_ready_for_limited_internal_trial",
      implementationStatus: "implementation_complete_for_stage2_mechanism",
      blockers: [
        "stage2_manual_dogfood_evidence_missing",
        "stage2_live_provider_p0_evidence_missing",
      ],
      deterministicStage1Ready: true,
      betaFoundationReady: true,
      manualDogfood: {
        attempted: false,
        ready: false,
        reviewerCount: 0,
        requiredScenarioCount: 24,
        attemptedScenarioCount: 0,
        passedScenarioCount: 0,
        missingScenarioIds: ["S2-D01"],
        failedScenarioIds: ["S2-D01"],
        traceIdsPresent: false,
        artifactDigest: null,
        blockers: ["stage2_manual_dogfood_evidence_missing"],
      },
      liveProvider: {
        attempted: false,
        ready: false,
        provider: null,
        model: null,
        requiredScenarioCount: 10,
        passedScenarioCount: 0,
        failedScenarioIds: ["L2-L01"],
        modelInvokedCount: 0,
        mainChatInvokedCount: 0,
        localOrMockCreditRejected: 0,
        artifactDigest: null,
        blockers: ["stage2_live_provider_p0_evidence_missing"],
        scenarioPlans: [
          {
            scenarioId: "L2-L01",
            scenario: "direct_answer",
            scenarioSetup: "live_provider_enabled",
            requiredRuntimeEvidence: [
              "provider_model_identity",
              "model_invoked",
              "response_preview",
              "no_agent_loop_metadata",
            ],
            failClosedBlocker: "live_provider_generation_not_completed",
            executionSource: "existing_v1_live_harness",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L02",
            scenario: "file_read_request",
            scenarioSetup: "seeded_workspace_file_or_missing_file_fixture",
            requiredRuntimeEvidence: ["file_action_or_blocker", "no_fake_observation"],
            failClosedBlocker: "live_provider_read_action_missing",
            executionSource: "stage2_live_file_read_runner",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L03",
            scenario: "web_policy_blocker",
            scenarioSetup: "web_network_policy_disabled",
            requiredRuntimeEvidence: ["web_policy_blocker", "no_provider_backed_web_credit"],
            failClosedBlocker: "live_provider_web_policy_bypass",
            executionSource: "stage2_live_web_policy_runner",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L04",
            scenario: "provider_backed_web_read",
            scenarioSetup: "governed_web_read_enabled",
            requiredRuntimeEvidence: [
              "selected_web_candidate",
              "action_status",
              "observation",
              "final_synthesis",
            ],
            failClosedBlocker: "provider_backed_web_agent_loop_not_executed",
            executionSource: "existing_v1_live_harness",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L05",
            scenario: "registered_mcp_read",
            scenarioSetup: "two_bounded_read_only_mcp_candidates",
            requiredRuntimeEvidence: [
              "candidate_ids",
              "target_allowlist",
              "selected_rank",
              "observation",
            ],
            failClosedBlocker: "provider_backed_mcp_agent_loop_not_executed",
            executionSource: "existing_v1_live_harness",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L06",
            scenario: "mcp_tool_permission_proposal",
            scenarioSetup: "permission_required_read_target",
            requiredRuntimeEvidence: [
              "tool_permission_proposal",
              "proposal_target",
              "selected_candidate",
              "no_read_success_overlap",
            ],
            failClosedBlocker: "provider_live_proposal_permission_not_executed",
            executionSource: "existing_v1_live_harness",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L07",
            scenario: "multi_step_react",
            scenarioSetup: "two_safe_read_sources_available",
            requiredRuntimeEvidence: ["two_actions", "two_observations", "final_synthesis"],
            failClosedBlocker: "live_provider_multistep_observation_missing",
            executionSource: "stage2_live_multistep_react_runner",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L08",
            scenario: "memory_proposal",
            scenarioSetup: "memory_proposal_enabled_no_auto_materialization",
            requiredRuntimeEvidence: [
              "proposal_id",
              "source_evidence",
              "no_memory_materialization",
            ],
            failClosedBlocker: "live_provider_memory_proposal_missing",
            executionSource: "stage2_live_memory_proposal_runner",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L09",
            scenario: "permission_denial",
            scenarioSetup: "pending_safe_read_permission_denial",
            requiredRuntimeEvidence: ["denied_permission_state", "no_resumed_action"],
            failClosedBlocker: "live_provider_permission_denial_bypassed",
            executionSource: "stage2_live_permission_denial_runner",
            runnerStatus: "implemented",
          },
          {
            scenarioId: "L2-L10",
            scenario: "failure_recovery",
            scenarioSetup: "induced_bad_tool_or_safe_tool_failure",
            requiredRuntimeEvidence: [
              "blocker_reason",
              "retry_or_cancel_state",
              "no_fake_final_done",
            ],
            failClosedBlocker: "live_provider_failure_hidden",
            executionSource: "stage2_live_failure_recovery_runner",
            runnerStatus: "implemented",
          },
        ],
        scenarioReports: [
          {
            scenarioId: "L2-L01",
            status: "blocked",
            credited: false,
            providerEndpointKind: null,
            blockers: ["stage2_live_provider_p0_evidence_missing"],
            mainChatInvoked: false,
            modelInvoked: false,
            runIdPresent: false,
            taskSessionIdPresent: false,
            responsePreviewPresent: false,
          },
          {
            scenarioId: "L2-L05",
            status: "failed",
            credited: false,
            providerEndpointKind: "external_provider",
            blockers: ["live_provider_model_ranked_selection_trace_missing"],
            mainChatInvoked: true,
            modelInvoked: true,
            runIdPresent: true,
            taskSessionIdPresent: true,
            responsePreviewPresent: true,
          },
        ],
      },
      controlPlane: {
        ready: true,
        requiredCount: 10,
        attemptedCount: 10,
        passedCount: 10,
        failedIds: [],
        coverage: [{ id: "direct_answer", passed: true, evidence: ["trace"], blockers: [] }],
        blockers: [],
      },
      memoryProposal: {
        ready: true,
        requiredCount: 8,
        attemptedCount: 8,
        passedCount: 8,
        failedIds: [],
        coverage: [{ id: "M2-01", passed: true, evidence: ["proposal"], blockers: [] }],
        blockers: [],
      },
      failureRecovery: {
        ready: true,
        requiredCount: 10,
        attemptedCount: 10,
        passedCount: 10,
        failedIds: [],
        coverage: [
          {
            id: "R2-01",
            passed: true,
            evidence: [
              "missing_workspace_file_blocker",
              "blocked_missing_source_state",
              "user_next_action_or_terminal_explanation",
              "no_fake_file_read_completion",
            ],
            blockers: [],
          },
        ],
        blockers: [],
      },
      finalDelivery: {
        ready: true,
        p0ScenarioCount: 24,
        finalDeliveryEvidenceCount: 24,
        finalDoneOverclaimCount: 0,
        blockers: [],
      },
      safety: {
        silentDurableWriteCount: 0,
        hiddenLegacyFallbackCount: 0,
        fakeBrowserEvidenceCount: 0,
        fakeLiveEvidenceCount: 0,
        localProviderCreditedAsLiveCount: 0,
        unscopedPermissionReplayCount: 0,
        finalDoneOverclaimCount: 0,
      },
      artifacts: [
        {
          kind: "stage1_browser_dogfood",
          path: "frontend/test-results/main-chat-stage1-dogfood-report.json",
          digest:
            "bytes:25422 hash:sha256:b53415fe64b623298be32b93fe55d3c45b7941c65d94e1ce6f3c716db8ade678",
          status: "loaded",
        },
        {
          kind: "manual_dogfood",
          path: "frontend/test-results/main-chat-stage2-manual-dogfood-report.json",
          digest: null,
          status: "missing",
        },
        {
          kind: "live_provider",
          path: "frontend/test-results/main-chat-stage2-live-provider-report.json",
          digest: null,
          status: "not_loaded",
        },
      ],
    });

    const result = await runMainChatAgentStage2ReadinessGate();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_agent_stage2_readiness_gate", undefined);
    expect(result.recommendation).toBe("not_ready_for_limited_internal_trial");
    expect(result.implementationStatus).toBe("implementation_complete_for_stage2_mechanism");
    expect(result.blockers).not.toContain("stage2_live_provider_p0_runner_incomplete");
    expect(result.manualDogfood.requiredScenarioCount).toBe(24);
    expect(result.liveProvider.requiredScenarioCount).toBe(10);
    expect(result.liveProvider.scenarioPlans).toHaveLength(10);
    const l205 = result.liveProvider.scenarioReports.find(row => row.scenarioId === "L2-L05");
    expect(l205?.credited).toBe(false);
    expect(l205?.blockers).toContain("live_provider_model_ranked_selection_trace_missing");
    const r201 = result.failureRecovery.coverage.find(row => row.id === "R2-01");
    expect(r201?.evidence).toContain("missing_workspace_file_blocker");
    expect(r201?.evidence.some(evidence => evidence.includes("success"))).toBe(false);
    const requiredEvidenceByScenario = Object.fromEntries(
      result.liveProvider.scenarioPlans.map(plan => [plan.scenarioId, plan.requiredRuntimeEvidence])
    );
    expect(requiredEvidenceByScenario["L2-L01"]).toEqual([
      "provider_model_identity",
      "model_invoked",
      "response_preview",
      "no_agent_loop_metadata",
    ]);
    expect(requiredEvidenceByScenario["L2-L02"]).toEqual([
      "file_action_or_blocker",
      "no_fake_observation",
    ]);
    expect(requiredEvidenceByScenario["L2-L03"]).toEqual([
      "web_policy_blocker",
      "no_provider_backed_web_credit",
    ]);
    expect(requiredEvidenceByScenario["L2-L04"]).toEqual([
      "selected_web_candidate",
      "action_status",
      "observation",
      "final_synthesis",
    ]);
    expect(requiredEvidenceByScenario["L2-L05"]).toEqual([
      "candidate_ids",
      "target_allowlist",
      "selected_rank",
      "observation",
    ]);
    expect(requiredEvidenceByScenario["L2-L06"]).toEqual([
      "tool_permission_proposal",
      "proposal_target",
      "selected_candidate",
      "no_read_success_overlap",
    ]);
    expect(requiredEvidenceByScenario["L2-L07"]).toEqual([
      "two_actions",
      "two_observations",
      "final_synthesis",
    ]);
    expect(requiredEvidenceByScenario["L2-L08"]).toEqual([
      "proposal_id",
      "source_evidence",
      "no_memory_materialization",
    ]);
    expect(requiredEvidenceByScenario["L2-L09"]).toEqual([
      "denied_permission_state",
      "no_resumed_action",
    ]);
    expect(requiredEvidenceByScenario["L2-L10"]).toEqual([
      "blocker_reason",
      "retry_or_cancel_state",
      "no_fake_final_done",
    ]);
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L03")
    ).toMatchObject({
      scenarioSetup: "web_network_policy_disabled",
      executionSource: "stage2_live_web_policy_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_web_policy_bypass",
    });
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L02")
    ).toMatchObject({
      scenarioSetup: "seeded_workspace_file_or_missing_file_fixture",
      executionSource: "stage2_live_file_read_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_read_action_missing",
    });
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L10")
    ).toMatchObject({
      scenarioSetup: "induced_bad_tool_or_safe_tool_failure",
      executionSource: "stage2_live_failure_recovery_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_failure_hidden",
    });
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L09")
    ).toMatchObject({
      scenarioSetup: "pending_safe_read_permission_denial",
      executionSource: "stage2_live_permission_denial_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_permission_denial_bypassed",
    });
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L08")
    ).toMatchObject({
      scenarioSetup: "memory_proposal_enabled_no_auto_materialization",
      executionSource: "stage2_live_memory_proposal_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_memory_proposal_missing",
    });
    expect(
      result.liveProvider.scenarioPlans.find(plan => plan.scenarioId === "L2-L07")
    ).toMatchObject({
      scenarioSetup: "two_safe_read_sources_available",
      executionSource: "stage2_live_multistep_react_runner",
      runnerStatus: "implemented",
      failClosedBlocker: "live_provider_multistep_observation_missing",
    });
    expect(result.controlPlane.ready).toBe(true);
    expect(result.safety.silentDurableWriteCount).toBe(0);
    expect(result.artifacts.map(artifact => artifact.kind)).toEqual([
      "stage1_browser_dogfood",
      "manual_dogfood",
      "live_provider",
    ]);
    expect(
      result.artifacts.find(artifact => artifact.kind === "stage1_browser_dogfood")
    ).toMatchObject({
      path: "frontend/test-results/main-chat-stage1-dogfood-report.json",
      status: "loaded",
    });
  });

  it("invokes Main Chat Agent Stage 2 manual dogfood artifact validator", async () => {
    vi.mocked(invoke).mockResolvedValue({
      attempted: false,
      ready: false,
      reviewerCount: 0,
      requiredScenarioCount: 24,
      attemptedScenarioCount: 0,
      passedScenarioCount: 0,
      missingScenarioIds: ["S2-D01"],
      failedScenarioIds: ["S2-D01"],
      traceIdsPresent: false,
      artifactDigest: null,
      blockers: ["stage2_manual_dogfood_evidence_missing"],
    });

    const result = await validateMainChatAgentStage2ManualDogfoodArtifact();

    expect(invoke).toHaveBeenCalledWith(
      "validate_main_chat_agent_stage2_manual_dogfood_artifact",
      undefined
    );
    expect(result.requiredScenarioCount).toBe(24);
    expect(result.missingScenarioIds).toContain("S2-D01");
    expect(result.blockers).toContain("stage2_manual_dogfood_evidence_missing");
  });

  it("invokes Main Chat Agent Stage 1 dogfood gate with browser and live sections", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "main_chat_agent_stage1_dogfood_gate",
      readinessSemantics:
        "stage1_real_e2e_dogfood_default_deterministic_browser_required_live_opt_in_separate",
      defaultReadinessScope: "stage1_default_deterministic_seeded_dogfood",
      optInLiveReadinessScope: "stage1_external_live_opt_in_only",
      defaultReady: false,
      optInLiveReady: false,
      readinessRecommendation: "not_ready",
      scenarioCount: 40,
      defaultScenarioCount: 36,
      defaultPassedCount: 0,
      defaultFailedCount: 36,
      taskSessionCreatedCount: 36,
      ordinaryChatScenarioCount: 24,
      seededTaskControlScenarioCount: 12,
      uiVerifiedScenarioCount: 0,
      finalDeliveryVerifiedScenarioCount: 36,
      legacyFallbackCount: 0,
      silentDurableWriteCount: 0,
      fakeExecutionDetectedCount: 0,
      externalLiveAttempted: false,
      externalLiveScenarioCount: 4,
      externalLivePassedCount: 0,
      externalLiveBlockedCount: 4,
      externalLiveBlockers: ["explicit_live_eval_required"],
      defaultReadinessUnaffectedByLive: true,
      browserE2eEnvironmentReady: false,
      browserE2eReportPath: "frontend/test-results/main-chat-stage1-dogfood-report.json",
      browserE2eRequiredJourneyCount: 36,
      browserE2ePassedJourneyCount: 0,
      browserE2eFailedJourneyCount: 36,
      manualDogfoodStatus: "not_attempted_engineering_dogfood_only",
      betaV1DefaultReady: true,
      productMaturityDefaultScenarioCount: 43,
      seedManifest: {
        seedWorkspaceRootKind: "temp_isolated",
        knowledgeAssetCount: 9,
        skillCount: 3,
        sessionSeedCount: 1,
        memorySeedCount: 5,
        proposalSeedCount: 2,
        taskSeedCount: 5,
        planSeedCount: 1,
        mcpManifestSeedCount: 2,
        webFixtureSeedCount: 1,
        seedDigest:
          "bytes:12 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        fileDigests: {
          "project_brief.md":
            "bytes:12 hash:sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        },
        runtimeObjectDigests: {},
        secretsDetected: false,
      },
      scenarios: [
        {
          scenarioId: "D01",
          scenarioType: "chat_e2e",
          entryPoint: "ordinary_main_chat_input",
          scenarioPromptId: "stage1:P0:D01",
          boundedPromptPreview: "What is the difference between a task and a proposal in OpenLife?",
          userPromptDigest:
            "bytes:12 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
          taskSessionId: "stage1_task_d01",
          runId: "stage1_run_d01",
          routeStrategy: "DirectAnswer",
          expectedOutcome: "success",
          actualOutcome: "success",
          runtimeEvents: ["route.selected", "final_delivery.created"],
          actions: [],
          observations: [],
          proposals: [],
          blockers: [],
          uiStates: [],
          finalDeliverySections: ["completed_work", "next_action"],
          controlEvidence: "not_applicable",
          runtimeEvidencePassed: true,
          uiEvidencePassed: false,
          finalDeliveryEvidencePassed: true,
          nonFakeEvidencePassed: true,
          legacyFallbackUsed: false,
          silentDurableWriteDetected: false,
          fakeExecutionDetected: false,
          seedManifestDigest:
            "bytes:12 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          liveProviderEvidence: "default_deterministic",
          passed: false,
          failureReason: "stage1_browser_ui_evidence_missing",
        },
      ],
      blockers: ["not_ready_browser_e2e_blocked"],
      acceptedResidualRisks: ["manual_dogfood_not_attempted_ready_for_engineering_dogfood_only"],
    });

    const result = await runMainChatAgentStage1DogfoodGate();

    expect(invoke).toHaveBeenCalledWith("run_main_chat_agent_stage1_dogfood_gate", undefined);
    expect(result.defaultReady).toBe(false);
    expect(result.readinessRecommendation).toBe("not_ready");
    expect(result.defaultScenarioCount).toBe(36);
    expect(result.ordinaryChatScenarioCount).toBe(24);
    expect(result.seededTaskControlScenarioCount).toBe(12);
    expect(result.browserE2eEnvironmentReady).toBe(false);
    expect(result.seedManifest.seedWorkspaceRootKind).toBe("temp_isolated");
    expect(result.scenarios[0]?.scenarioId).toBe("D01");
    expect(result.optInLiveReady).toBe(false);
  });

  it("invokes runtime strategy registry status as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reportKind: "multi_strategy_runtime_maturity",
      maturityReady: true,
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
    expect(result.migrationPermission).toBe(false);
  });

  it("invokes runtime migration gate as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
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
    expect(result.previewPathHealthy).toBe(true);
    expect(result.blockingReasons).toEqual([]);
  });

  it("invokes controlled Chat pilot eligibility as explicit read-only diagnostic", async () => {
    vi.mocked(invoke).mockResolvedValue({
      eligible: true,
      requiredCleanRuns: 3,
      cleanRunCount: 3,
      checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
      blockingReasons: [],
      lastGateReport: {
        previewPathHealthy: true,
        metadataSafeTraceReady: true,
        fallbackAvailable: true,
        noExternalWrites: true,
        proposalFirstPreserved: true,
        blockingReasons: [],
      },
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

  it("invokes Main Chat runtime status as kernel truth", async () => {
    vi.mocked(invoke).mockResolvedValue({
      statusVersion: 2,
      authoritativeRuntime: "main_chat_kernel",
      defaultSendPath: "main_chat_kernel",
      startStreamPath: "main_chat_kernel",
      sourceOfTruth: "main_chat_turn_pipeline",
      kernelEvidence: {
        kernelBackedDefault: false,
        finalGateEvidencePresent: false,
        finalGateReady: false,
        latestKernelRouteObserved: true,
        legacyFallbackFreeSinceStartup: true,
      },
      latestRouteEvidence: {
        status: "observed",
        directAnswerObserved: true,
        governedBlockerObserved: false,
        agentLoopObserved: false,
        kernelBackedDefaultObserved: true,
        legacyFallbackUsed: false,
        lastKernelEventCount: 3,
      },
      legacyFallback: {
        mode: "explicit_only",
        allowedByDefault: false,
        usedCountSinceStartup: 0,
        lastUsedAt: null,
        lastReasonCode: null,
      },
      finalGateReadiness: {
        authority: "main_chat_final_acceptance_gate",
        status: "not_run",
        blockers: [],
        lastReportRunId: null,
      },
    });

    const result = await getMainChatRuntimeStatus();

    expect(invoke).toHaveBeenCalledWith("get_main_chat_runtime_status", undefined);
    expect(result.authoritativeRuntime).toBe("main_chat_kernel");
    expect(result.legacyFallback.allowedByDefault).toBe(false);
    expect(result.finalGateReadiness.authority).toBe("main_chat_final_acceptance_gate");
  });
});
