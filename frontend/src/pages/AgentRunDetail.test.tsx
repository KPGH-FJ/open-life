import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { describe, expect, it, vi, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import AgentRunDetail from "./AgentRunDetail";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const now = new Date().toISOString();

const routeEvidence = {
  evidence_id: "runtime-route-timeout-1",
  generated_at: now,
  conversation_id: "chat-timeout-1",
  run_id: "run-timeout-1",
  task_session_id: "task-timeout-1",
  answer_scope: "current_turn",
  planned_route: null,
  actual_route: {
    provider: "deepseek",
    model: "deepseek-chat",
    route_type: "cloud",
    privacy_level: "summary",
    reason: "runtime_route_evidence",
    provider_health_is_estimated: false,
  },
  last_completed_route: null,
  provider_readiness: {
    configured: true,
    credential_present: true,
    validated: true,
    validation_status: "ready",
    preferred: "cloud",
    actually_used: "deepseek",
    stale: false,
    failed: false,
    last_checked_at: null,
  },
  fallback: null,
  external_transmission: "sent",
  source_refs: [],
  truth_confidence: "verified",
};

function baseRun(id: string) {
  return {
    id,
    taskId: `task-${id}`,
    sessionId: "chat-timeout-1",
    status: "failed",
    kind: "conversation",
    userInput: "metadata safe user summary",
    outputPreview: "",
    generatedProposals: [],
    actions: [],
    observations: [],
    modelRoute: {
      provider: "wrong-provider",
      model: "wrong-model",
      routeType: "local",
      preferLocal: true,
      localModel: "wrong-local",
      reason: "legacy model route should not drive Runs detail",
      privacyLevel: "none",
      retryCount: 0,
      providerHealthIsEstimated: true,
    },
    startedAt: now,
    finishedAt: now,
  };
}

function baseTaskSession(status: string) {
  return {
    id: status === "blocked" ? "task-blocked-1" : "task-timeout-1",
    chatSessionId: "chat-timeout-1",
    userGoal: "metadata safe goal",
    selectedStrategy: "direct_answer",
    status,
    currentPlanSummary: null,
    actionQueueIds: [],
    pendingBlockers: status === "blocked" ? ["web_network_policy_blocked"] : [],
    contextSnapshotRefs: ["ctx-1"],
    createdAt: now,
    updatedAt: now,
    finalSummary: status === "blocked" ? "web_network_policy_blocked" : "timeout",
  };
}

function diagnostics() {
  return {
    staleContext: false,
    missingActionEvidence: false,
    permissionScopeMismatch: false,
    terminalNoResume: true,
    providerUnavailable: false,
    toolUnavailable: false,
    requiresUserDecision: false,
    reasonCodes: ["terminal_no_resume"],
    automaticReplayAllowed: false,
  };
}

function renderDetail(runId: string) {
  render(
    <MemoryRouter initialEntries={[`/runs/${runId}`]}>
      <Routes>
        <Route path="/runs/:runId" element={<AgentRunDetail />} />
        <Route path="/runs" element={<div>Runs list</div>} />
      </Routes>
    </MemoryRouter>
  );
}

describe("AgentRunDetail evidence view", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders remote unknown as amber uncertainty instead of a failed error card", async () => {
    const remoteUnknownRun = {
      ...baseRun("run-remote-unknown"),
      status: "remote_unknown",
      kind: "tool_execution",
      error: {
        message: "remote_state_unknown",
        phase: "startup_projection_recovery",
        recoverable: false,
      },
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_agent_run") return Promise.resolve(remoteUnknownRun);
      if (cmd === "list_main_chat_agent_tasks") return Promise.resolve([]);
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    renderDetail("run-remote-unknown");

    expect((await screen.findAllByText("远端状态未知")).length).toBeGreaterThan(0);
    expect(screen.getByText(/请求已离开本地，但尚未观察到可信的远端终态/)).toBeInTheDocument();
    expect(screen.queryByText("错误")).not.toBeInTheDocument();
    expect(screen.queryByText("run_failed")).not.toBeInTheDocument();
  });

  it("labels migrated legacy metadata as unverified and suppresses fake execution claims", async () => {
    const legacyRun = {
      ...baseRun("run-legacy-unverified"),
      legacyPayloadUnverified: true,
      generatedProposals: ["LEGACY_PROPOSAL_MUST_NOT_RENDER"],
      reasoningStrategy: "layered",
      error: {
        message: "LEGACY_ERROR_MUST_NOT_RENDER",
        phase: "provider",
        recoverable: false,
      },
      contextSummary: {
        lifeModelEmpty: false,
        memoryHitCount: 99,
        usedToolsPrompt: true,
        redactionApplied: false,
      },
      actions: [
        {
          id: "legacy-action",
          actionType: "tool",
          input: {},
          status: "succeeded",
          timestamp: now,
          reactTrace: {
            actionId: "legacy-action",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "tool",
            toolId: "legacy.tool",
            toolName: "legacy.tool",
            toolSource: "legacy",
            actionCategory: "read",
            riskLevel: "low",
            status: "succeeded",
            outputReceipt: {
              version: 2,
              kind: "tool_output",
              provenance: "observed_tool_adapter_body",
              byteCount: 1,
              digest: `sha256:${"f".repeat(64)}`,
              verified: true,
            },
            metadataSafe: true,
          },
        },
      ],
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_agent_run") return Promise.resolve(legacyRun);
      if (cmd === "list_main_chat_agent_tasks") {
        return Promise.resolve([
          {
            taskSessionId: "task-legacy-unverified",
            conversationId: "chat-timeout-1",
            runId: "run-legacy-unverified",
            title: "legacy task",
            strategy: "legacy",
            status: "completed",
            lifecycleState: "completed",
            lastUpdatedAt: now,
            lastObservationPreview: "LEGACY_OBSERVATION_MUST_NOT_RENDER",
            pendingBlockerCount: 0,
            pendingProposalCount: 1,
            nextRecommendedControl: "retry",
            staleState: "stale",
            resumeSafetyDigest: "legacy digest",
          },
        ]);
      }
      if (cmd === "get_main_chat_agent_task_detail") {
        return Promise.resolve({
          taskSession: baseTaskSession("completed"),
          actions: [],
          transcript: [],
          proposals: ["LEGACY_PROPOSAL_MUST_NOT_RENDER"],
          blockers: [],
          finalDelivery: null,
          continuityDiagnostics: diagnostics(),
          allowedControls: ["retry", "cancel"],
          nextRecommendedControl: "retry",
          retryTargetActionId: "legacy-retry-target",
          lastSafeResumePoint: null,
          contextDigest: "legacy digest",
          selectedSkillDigest: null,
          toolManifestDigest: "legacy digest",
          evidenceView: null,
        });
      }
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    renderDetail("run-legacy-unverified");

    expect(await screen.findByText(/旧版 payload 迁移的未验证执行记录/)).toBeInTheDocument();
    expect(screen.getByText("计划路线（非调用证据）")).toBeInTheDocument();
    expect(
      screen.getByText(/Provider、model、route、privacy、retry 与 health 均未获得/)
    ).toBeInTheDocument();
    expect(screen.getByText("旧版 trace 未验证")).toBeInTheDocument();
    expect(screen.getByText(/旧版协作规则、工具与行为检查未验证/)).toBeInTheDocument();
    expect(screen.queryByText(/wrong-provider/)).not.toBeInTheDocument();
    expect(screen.queryByText(new RegExp(`sha256:${"f".repeat(64)}`))).not.toBeInTheDocument();
    expect(screen.queryByText("legacy.tool")).not.toBeInTheDocument();
    expect(screen.getAllByText("unknown").length).toBeGreaterThan(0);
    expect(screen.queryByText("failed")).not.toBeInTheDocument();
    expect(screen.queryByText("错误")).not.toBeInTheDocument();
    expect(screen.queryByText("LEGACY_ERROR_MUST_NOT_RENDER")).not.toBeInTheDocument();
    expect(screen.queryByText("上下文摘要")).not.toBeInTheDocument();
    expect(screen.queryByText("记忆命中: 99")).not.toBeInTheDocument();
    expect(screen.queryByText("completed")).not.toBeInTheDocument();
    expect(screen.queryByText(/连续性需复核/)).not.toBeInTheDocument();
    expect(screen.queryByText(/LEGACY_OBSERVATION_MUST_NOT_RENDER/)).not.toBeInTheDocument();
    expect(screen.queryByText(/LEGACY_PROPOSAL_MUST_NOT_RENDER/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /retry/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /cancel/i })).not.toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/失败|已完成|可重试|可取消|无需操作|无阻断/);
  });

  it("renders timeout and final timeline from evidence view and uses RuntimeRouteEvidence", async () => {
    const evidenceView = {
      runId: "run-timeout-1",
      taskSessionId: "task-timeout-1",
      title: "Timeout task",
      lifecycleState: "timed_out",
      routeEvidence,
      eventTimeline: [
        {
          id: "timeout-event",
          kind: "error",
          summary: "Provider timed out after deadline.",
          createdAt: now,
          failureKind: "timeout",
          normalizedLifecycleState: "timed_out",
          sourceRef: "v6.provider_timeout_replay",
        },
        {
          id: "final-event",
          kind: "final_result",
          summary: "Final response was not delivered.",
          createdAt: now,
          normalizedLifecycleState: "timed_out",
          sourceRef: "finalizer",
        },
      ],
      actionCount: 1,
      observationCount: 2,
      blockers: [],
      proposals: [],
      planRefs: ["ctx-1"],
      allowedControls: ["open_trace"],
      nextRecommendedControl: "open_trace",
      redactionState: "metadata_only",
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_agent_run") return Promise.resolve(baseRun("run-timeout-1"));
      if (cmd === "list_main_chat_agent_tasks") {
        return Promise.resolve([
          {
            taskSessionId: "task-timeout-1",
            conversationId: "chat-timeout-1",
            runId: "run-timeout-1",
            title: "Timeout task",
            strategy: "direct_answer",
            status: "failed",
            lastUpdatedAt: now,
            lastObservationPreview: "Provider timed out after deadline.",
            pendingBlockerCount: 0,
            pendingProposalCount: 0,
            nextRecommendedControl: "open_trace",
            staleState: "terminal",
            resumeSafetyDigest: "sha256:test",
            lifecycleState: "timed_out",
            allowedControls: ["open_trace"],
            evidenceView,
          },
        ]);
      }
      if (cmd === "get_main_chat_agent_task_detail") {
        return Promise.resolve({
          taskSession: baseTaskSession("failed"),
          actions: [],
          transcript: [],
          proposals: [],
          blockers: [],
          finalDelivery: null,
          continuityDiagnostics: diagnostics(),
          allowedControls: ["open_trace"],
          nextRecommendedControl: "open_trace",
          lastSafeResumePoint: null,
          contextDigest: "bytes:1 hash:sha256:test",
          selectedSkillDigest: null,
          toolManifestDigest: "bytes:1 hash:sha256:test",
          evidenceView,
        });
      }
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    renderDetail("run-timeout-1");

    expect((await screen.findAllByText("timed_out")).length).toBeGreaterThan(0);
    expect(screen.getByText("Provider timed out after deadline.")).toBeInTheDocument();
    expect(screen.getByText("Final response was not delivered.")).toBeInTheDocument();
    expect(screen.getAllByText(/云端路线 · deepseek/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/wrong-provider/)).not.toBeInTheDocument();
    expect(screen.getByText("当前只允许查看 trace。")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /resume/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /retry/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /cancel/i })).not.toBeInTheDocument();
  });

  it("renders blocker event and only backend allowed controls", async () => {
    const evidenceView = {
      runId: "run-blocked-1",
      taskSessionId: "task-blocked-1",
      title: "Blocked task",
      lifecycleState: "blocked",
      routeEvidence: null,
      eventTimeline: [
        {
          id: "blocker-event",
          kind: "blocker",
          summary: "web_network_policy_blocked",
          createdAt: now,
          failureKind: "policy_blocker",
          normalizedLifecycleState: "blocked",
          sourceRef: "v4.web_mcp_blocker_replay",
        },
      ],
      actionCount: 0,
      observationCount: 1,
      blockers: ["web_network_policy_blocked"],
      proposals: ["proposal-1"],
      planRefs: ["ctx-1"],
      allowedControls: ["open_trace", "refresh_context"],
      nextRecommendedControl: "refresh_context",
      redactionState: "metadata_only",
    };
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_agent_run") return Promise.resolve(baseRun("run-blocked-1"));
      if (cmd === "list_main_chat_agent_tasks") {
        return Promise.resolve([
          {
            taskSessionId: "task-blocked-1",
            conversationId: "chat-timeout-1",
            runId: "run-blocked-1",
            title: "Blocked task",
            strategy: "react_tool_execution",
            status: "blocked",
            lastUpdatedAt: now,
            lastObservationPreview: "web_network_policy_blocked",
            pendingBlockerCount: 1,
            pendingProposalCount: 1,
            nextRecommendedControl: "refresh_context",
            staleState: "fresh",
            resumeSafetyDigest: "sha256:test",
            lifecycleState: "blocked",
            allowedControls: ["open_trace", "refresh_context"],
            evidenceView,
          },
        ]);
      }
      if (cmd === "get_main_chat_agent_task_detail") {
        return Promise.resolve({
          taskSession: baseTaskSession("blocked"),
          actions: [],
          transcript: [],
          proposals: [],
          blockers: ["web_network_policy_blocked"],
          finalDelivery: null,
          continuityDiagnostics: diagnostics(),
          allowedControls: ["open_trace", "refresh_context"],
          nextRecommendedControl: "refresh_context",
          lastSafeResumePoint: null,
          contextDigest: "bytes:1 hash:sha256:test",
          selectedSkillDigest: null,
          toolManifestDigest: "bytes:1 hash:sha256:test",
          evidenceView,
        });
      }
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    renderDetail("run-blocked-1");

    expect(await screen.findAllByText("blocked")).not.toHaveLength(0);
    expect(screen.getAllByText("web_network_policy_blocked").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /refresh context/i })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /retry/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /cancel/i })).not.toBeInTheDocument();
  });

  it("preflights single run deletion before calling delete_agent_run", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_agent_run") return Promise.resolve(baseRun("run-delete-1"));
      if (cmd === "list_main_chat_agent_tasks") return Promise.resolve([]);
      if (cmd === "get_danger_action_preflight") {
        return Promise.resolve({
          actionType: "agent_run_delete",
          riskTier: "high",
          scopeSummary:
            "删除选中的 AgentRun 运行记录；预检只保留数量和 id digest，不展开 transcript。",
          dataCategories: ["agent_run_metadata", "run_trace_metadata"],
          writesDurableState: true,
          privacySensitive: true,
          externalTransmission: "not_sent_externally",
          dryRunAvailable: false,
          backupStatus: "soft_delete_trash_view",
          requiresTypedConfirmation: false,
          confirmationRequired: true,
          confirmationPhrase: "DELETE RUN",
          confirmationScopeDigest: `bytes:10 hash:sha256:${"a".repeat(64)}`,
          preflightId: `danger-preflight:sha256:${"b".repeat(64)}`,
          affectedItemCount: 1,
          affectedItemDigest: `bytes:10 hash:sha256:${"a".repeat(64)}`,
          finalActionEnabled: true,
          safeModeBlocked: false,
          blockingReasons: [],
          sourceRefs: [
            "settings_command:get_danger_action_preflight",
            "final_command:delete_agent_run",
            "governance:slice5c_danger_zone_consolidation",
          ],
        });
      }
      if (cmd === "delete_agent_run") return Promise.resolve(undefined);
      return Promise.reject(new Error(`unexpected command ${cmd}`));
    });

    renderDetail("run-delete-1");

    fireEvent.click(await screen.findByRole("button", { name: "删除运行记录" }));

    expect(
      await screen.findByRole("dialog", { name: "动作预检：删除运行记录" })
    ).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "delete_agent_run")).toBe(false);
    const continueButton = screen.getByRole("button", { name: "继续删除" });
    expect(continueButton).toBeDisabled();

    fireEvent.change(screen.getByLabelText(/输入 DELETE RUN 以继续/), {
      target: { value: "DELETE RUN" },
    });
    fireEvent.click(continueButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "delete_agent_run",
        expect.objectContaining({
          runId: "run-delete-1",
          confirmationEvidence: expect.objectContaining({
            actionType: "agent_run_delete",
            targetIds: ["run-delete-1"],
          }),
        })
      );
    });
  });
});
