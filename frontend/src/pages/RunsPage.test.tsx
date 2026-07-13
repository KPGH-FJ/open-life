import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import RunsPage from "./RunsPage";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("RunsPage contract", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-1",
            taskId: "task-1",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "camel case user input",
            outputPreview: "camel case output preview",
            generatedProposals: ["proposal-1"],
            actions: [
              {
                id: "action-1",
                actionType: "mcp_tool",
                target: "memory.search",
                input: { arguments: { query: "raw query should not render" } },
                status: "succeeded",
                timestamp: new Date().toISOString(),
                reactTrace: {
                  actionId: "action-1",
                  stepIndex: 0,
                  toolCallIndex: 0,
                  actionType: "mcp_tool",
                  toolId: "memory.search",
                  toolName: "memory.search",
                  toolSource: "builtin",
                  actionCategory: "read",
                  riskLevel: "low",
                  status: "succeeded",
                  outputPreview: "40 bytes redacted",
                  outputReceipt: {
                    version: 2,
                    kind: "tool_output",
                    provenance: "observed_tool_adapter_body",
                    byteCount: 40,
                    digest: `sha256:${"a".repeat(64)}`,
                    verified: true,
                  },
                  metadataSafe: true,
                },
              },
            ],
            observations: [],
            startedAt: new Date().toISOString(),
          },
          {
            id: "run-sensitive-1",
            taskId: "task-sensitive-1",
            sessionId: "session-sensitive",
            status: "running",
            kind: "conversation",
            userInput: "Contact qa@example.com with sk-sensitive-token-123456789",
            outputPreview: "Token pk-sensitive-output-token-123456789 should redact",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date(Date.now() - 30 * 60 * 1000).toISOString(),
          },
          {
            id: "run-2",
            taskId: "task-2",
            status: "completed",
            kind: "builder",
            userInput: "deleted run",
            outputPreview: "hidden by default",
            generatedProposals: [],
            actions: [],
            observations: [],
            deletedAt: new Date().toISOString(),
            startedAt: new Date().toISOString(),
          },
          {
            id: "run-preview-1",
            taskId: "task-preview-1",
            sessionId: "session-preview",
            status: "completed",
            kind: "conversation",
            generatedProposals: [],
            actions: [],
            observations: [],
            reasoningStrategy: "multi_strategy_preview",
            reasoningTrace: {
              strategy_result: {
                previewRuntime: "multi_strategy",
                runtimeStrategyTraceKind: "multi_strategy_preview",
                selectedStrategyKind: "planExecute",
                strategyKind: "planExecute",
                payloadKind: "planExecute",
                strategyDescriptorId: "plan_execute",
                selectionReasonCode: "write_like_intent",
                registryReady: true,
                governanceDecisionKind: "warn",
                riskLevel: "medium",
                reasonCode: "write_like_intent",
                warnings: ["preview runtime forces allowWrites=false"],
              },
            },
            outputPreview: "Multi-strategy preview: planExecute / warn",
            startedAt: new Date().toISOString(),
          },
          {
            id: "run-plan-1",
            taskId: "task-plan-1",
            sessionId: "workspace_weekly_planning",
            status: "completed",
            kind: "planning",
            generatedProposals: ["proposal-plan-1"],
            actions: [],
            observations: [],
            reasoningStrategy: "plan_execute_product",
            reasoningTrace: {
              strategy_result: {
                planExecuteProductVertical: true,
                runtimeStrategyTraceKind: "plan_execute_product",
                scenarioId: "weekly_planning",
                planSessionId: "plan-session-1",
                strategyKind: "plan_execute",
                selectedStrategyKind: "plan_execute",
                payloadKind: "plan_execute",
                strategyDescriptorId: "plan_execute",
                selectionReasonCode: "weekly_planning_product",
                registryReady: true,
                status: "finalized",
                stepCount: 3,
                stepStatusCounts: {
                  planned: 1,
                  executed: 1,
                  requiresProposal: 1,
                  blocked: 0,
                },
                generatedProposalIds: ["proposal-plan-1"],
                generatedProposalCount: 1,
                governanceDecisionCounts: {
                  allow: 1,
                  requireProposal: 1,
                  block: 0,
                },
                warningCount: 0,
                metadataSafe: true,
                directLifeModelWrites: false,
                externalWritesExecuted: false,
              },
            },
            outputPreview: "raw-sensitive-weekly-plan-should-not-render",
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "get_tasks_view_model") {
        const now = new Date().toISOString();
        return Promise.resolve({
          data: {
            items: [
              {
                canonicalTaskId: "task-1",
                taskSessionId: null,
                relatedRunIds: ["run-1"],
                conversationId: "session-1",
                title: "camel case user input",
                strategy: "conversation",
                lifecycleStatus: "completed_needs_evidence",
                terminalDeliveryStatus: "missing_final_delivery_evidence",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: [],
                pendingReviewItemRefs: [],
                allowedControls: [],
                nextRecommendedControl: "open_trace",
                latestResultPreview: {
                  status: "missing_final_delivery_evidence",
                  label: "missing final delivery evidence",
                  preview: "camel case user input",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt: now,
              },
              {
                canonicalTaskId: "task-sensitive-1",
                taskSessionId: "task-session-sensitive",
                relatedRunIds: ["run-sensitive-1"],
                conversationId: "session-sensitive",
                title: "Sensitive running task",
                strategy: "direct_answer",
                lifecycleStatus: "failed",
                terminalDeliveryStatus: "failed",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: ["provider_timed_out"],
                pendingReviewItemRefs: [],
                allowedControls: [
                  {
                    id: "task-session-sensitive:open_trace",
                    label: "Open trace",
                    kind: "open_trace",
                    effect: "evidence_only",
                    enabled: true,
                    targetTaskId: "task-session-sensitive",
                    completionProofAfterDispatch: false,
                  },
                ],
                nextRecommendedControl: "open_trace",
                latestResultPreview: {
                  status: "failed",
                  label: "failed",
                  preview: "Provider timed out",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt: now,
              },
              {
                canonicalTaskId: "task-preview-1",
                taskSessionId: null,
                relatedRunIds: ["run-preview-1"],
                conversationId: "session-preview",
                title: "Multi-strategy preview",
                strategy: "multi_strategy_preview",
                lifecycleStatus: "completed_needs_evidence",
                terminalDeliveryStatus: "missing_final_delivery_evidence",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: [],
                pendingReviewItemRefs: [],
                allowedControls: [],
                nextRecommendedControl: "open_run",
                latestResultPreview: {
                  status: "missing_final_delivery_evidence",
                  label: "missing final delivery evidence",
                  preview: "Multi-strategy preview: planExecute / warn",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt: now,
              },
              {
                canonicalTaskId: "task-plan-1",
                taskSessionId: null,
                relatedRunIds: ["run-plan-1"],
                conversationId: "workspace_weekly_planning",
                title: "weekly_planning · 3 步 · 待确认 1",
                strategy: "plan_execute_product",
                lifecycleStatus: "completed_needs_evidence",
                terminalDeliveryStatus: "missing_final_delivery_evidence",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: [],
                pendingReviewItemRefs: [],
                allowedControls: [],
                nextRecommendedControl: "open_run",
                latestResultPreview: {
                  status: "missing_final_delivery_evidence",
                  label: "missing final delivery evidence",
                  preview: "weekly_planning · 3 步 · 待确认 1",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt: now,
              },
              {
                canonicalTaskId: "task-2",
                taskSessionId: null,
                relatedRunIds: ["run-2"],
                title: "deleted run",
                strategy: "builder",
                lifecycleStatus: "completed_needs_evidence",
                terminalDeliveryStatus: "missing_final_delivery_evidence",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: [],
                pendingReviewItemRefs: [],
                allowedControls: [],
                nextRecommendedControl: "open_run",
                latestResultPreview: {
                  status: "missing_final_delivery_evidence",
                  label: "missing final delivery evidence",
                  preview: "deleted run",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt: now,
              },
            ],
            summary: {
              total: 5,
              activeCount: 0,
              waitingPermissionCount: 0,
              blockedCount: 0,
              pendingReviewCount: 0,
              completedCount: 0,
              completedNeedsEvidenceCount: 4,
              failedCount: 1,
              cancelledCount: 0,
              byLifecycleStatus: { completed_needs_evidence: 4, failed: 1 },
            },
            sourceRefs: [],
            contractLimitations: [],
          },
          status: "ready",
          lastUpdatedAt: now,
          source: "backend-readmodel",
          evidenceRefs: [],
          warnings: [],
          actions: { primary: [] },
        });
      }
      if (cmd === "list_main_chat_agent_tasks") {
        return Promise.resolve([
          {
            taskSessionId: "task-session-sensitive",
            conversationId: "session-sensitive",
            runId: "run-sensitive-1",
            title: "Sensitive running task",
            strategy: "direct_answer",
            status: "failed",
            lastUpdatedAt: new Date().toISOString(),
            lastObservationPreview: "Provider timed out",
            pendingBlockerCount: 0,
            pendingProposalCount: 0,
            nextRecommendedControl: "open_trace",
            staleState: "terminal",
            resumeSafetyDigest: "sha256:test",
            lifecycleState: "timed_out",
            lastSafeEvent: "Provider timed out",
            actionCount: 1,
            observationCount: 2,
            allowedControls: ["open_trace"],
            redactionState: "metadata_only",
            routeEvidence: {
              evidence_id: "runtime-route-run-sensitive",
              generated_at: new Date().toISOString(),
              conversation_id: "session-sensitive",
              run_id: "run-sensitive-1",
              task_session_id: "task-session-sensitive",
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
            },
            evidenceView: {
              runId: "run-sensitive-1",
              taskSessionId: "task-session-sensitive",
              title: "Sensitive running task",
              lifecycleState: "timed_out",
              routeEvidence: {
                evidence_id: "runtime-route-run-sensitive",
                generated_at: new Date().toISOString(),
                conversation_id: "session-sensitive",
                run_id: "run-sensitive-1",
                task_session_id: "task-session-sensitive",
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
              },
              eventTimeline: [
                {
                  id: "timeout-event",
                  kind: "error",
                  summary: "Provider timed out",
                  createdAt: new Date().toISOString(),
                  failureKind: "timeout",
                  normalizedLifecycleState: "timed_out",
                  sourceRef: "v6.provider_timeout_replay",
                },
              ],
              actionCount: 1,
              observationCount: 2,
              blockers: [],
              proposals: [],
              planRefs: ["timeout-context"],
              allowedControls: ["open_trace"],
              nextRecommendedControl: "open_trace",
              redactionState: "metadata_only",
            },
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("labels remote unknown runs as uncertainty and never as failed", async () => {
    const now = new Date().toISOString();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-remote-unknown",
            taskId: "task-remote-unknown",
            status: "remote_unknown",
            kind: "tool_execution",
            userInput: "A2A outbound task",
            generatedProposals: [],
            actions: [],
            observations: [],
            error: {
              message: "remote_state_unknown",
              phase: "startup_projection_recovery",
              recoverable: false,
            },
            startedAt: now,
            finishedAt: now,
          },
        ]);
      }
      if (cmd === "get_tasks_view_model") {
        return Promise.resolve({
          data: {
            items: [
              {
                canonicalTaskId: "task-remote-unknown",
                taskSessionId: null,
                relatedRunIds: ["run-remote-unknown"],
                conversationId: null,
                title: "A2A outbound task",
                strategy: "tool_execution",
                lifecycleStatus: "remote_unknown",
                terminalDeliveryStatus: "unknown",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: [],
                pendingReviewItemRefs: [],
                allowedControls: [],
                nextRecommendedControl: "open_trace",
                latestResultPreview: null,
                evidenceRefs: [],
                updatedAt: now,
              },
            ],
            summary: {
              total: 1,
              activeCount: 0,
              waitingPermissionCount: 0,
              blockedCount: 0,
              pendingReviewCount: 0,
              completedCount: 0,
              completedNeedsEvidenceCount: 0,
              failedCount: 0,
              cancelledCount: 0,
              byLifecycleStatus: { remote_unknown: 1 },
            },
            sourceRefs: [],
            contractLimitations: [],
          },
          status: "ready",
          lastUpdatedAt: now,
          source: "backend-readmodel",
          evidenceRefs: [],
          warnings: [],
          actions: { primary: [] },
        });
      }
      return mockInvoke(cmd);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect((await screen.findAllByText("任务远端状态未知")).length).toBeGreaterThan(0);
    expect(screen.getByText("远端状态未知，未自动重试")).toBeInTheDocument();
    expect(screen.queryByText("任务失败")).not.toBeInTheDocument();
    expect(screen.queryByText("run_failed")).not.toBeInTheDocument();
  });

  it("uses metadata-safe AgentRun fields for search, preview, proposals, and trash filtering", async () => {
    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("camel case user input")).toBeInTheDocument();
    expect(screen.getByText("camel case output preview")).toBeInTheDocument();
    expect(screen.queryByText(/qa@example\.com/)).not.toBeInTheDocument();
    expect(screen.queryByText(/sk-sensitive-token/)).not.toBeInTheDocument();
    expect(screen.getByText("任务失败")).toBeInTheDocument();
    expect(screen.getAllByText("下一步：查看记录").length).toBeGreaterThan(0);
    expect(screen.getByText("连续性需复核")).toBeInTheDocument();
    expect(screen.getAllByText("待确认 1").length).toBeGreaterThan(0);
    expect(screen.queryByText("策略预览")).not.toBeInTheDocument();
    expect(screen.queryByText("策略：planExecute")).not.toBeInTheDocument();
    expect(screen.queryByText("治理：warn")).not.toBeInTheDocument();
    expect(screen.getAllByText("计划执行").length).toBeGreaterThan(0);
    expect(screen.getByText("weekly_planning · 3 步 · 待确认 1")).toBeInTheDocument();
    expect(screen.getByText("状态：finalized")).toBeInTheDocument();
    expect(screen.getByText("步骤：3")).toBeInTheDocument();
    expect(screen.getByText("待确认：1")).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-weekly-plan-should-not-render")
    ).not.toBeInTheDocument();
    expect(screen.queryByText("hidden by default")).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("搜索任务、模型、工具、状态..."), {
      target: { value: "plan-session-1" },
    });
    expect(screen.getAllByText("计划执行").length).toBeGreaterThan(0);

    fireEvent.change(screen.getByPlaceholderText("搜索任务、模型、工具、状态..."), {
      target: { value: "weekly_planning_product" },
    });
    expect(screen.getAllByText("计划执行").length).toBeGreaterThan(0);

    fireEvent.change(screen.getByPlaceholderText("搜索任务、模型、工具、状态..."), {
      target: { value: "memory.search" },
    });
    expect(screen.getAllByText("对话任务").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByText("已删除"));
    await waitFor(() => {
      expect(screen.getByText("deleted run")).toBeInTheDocument();
    });
  });

  it("does not present legacy route, output, or tool metadata as observed truth", async () => {
    const updatedAt = new Date().toISOString();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "legacy-run-1",
            taskId: "legacy-task-1",
            status: "failed",
            kind: "conversation",
            legacyPayloadUnverified: true,
            outputPreview: `run_output:bytes=1:sha256:${"a".repeat(64)}`,
            modelRoute: {
              provider: "forged-provider",
              model: "forged-model",
              routeType: "cloud",
              preferLocal: false,
              localModel: "forged-local",
              reason: "forged actual route",
              privacyLevel: "none",
              retryCount: 4,
            },
            generatedProposals: ["legacy-proposal-ref"],
            error: {
              message: "LEGACY_RUN_ERROR_MUST_NOT_RENDER",
              phase: "provider",
              recoverable: true,
            },
            actions: [
              {
                id: "legacy-action",
                actionType: "tool",
                input: {},
                status: "succeeded",
                timestamp: updatedAt,
              },
            ],
            observations: [],
            startedAt: new Date(Date.now() - 30 * 60 * 1000).toISOString(),
          },
        ]);
      }
      if (cmd === "get_tasks_view_model") {
        return Promise.resolve({
          data: {
            items: [
              {
                canonicalTaskId: "legacy-task-1",
                relatedRunIds: ["legacy-run-1"],
                title: "Legacy migrated task",
                strategy: "conversation",
                lifecycleStatus: "completed_needs_evidence",
                terminalDeliveryStatus: "missing_final_delivery_evidence",
                finalDeliveryEvidencePresent: false,
                pendingBlockers: ["LEGACY_BLOCKER_MUST_NOT_RENDER"],
                pendingReviewItemRefs: [
                  { id: "legacy-review", kind: "proposal", label: "legacy proposal" },
                ],
                allowedControls: [
                  {
                    id: "legacy-retry",
                    label: "重试此任务",
                    kind: "retry",
                    effect: "task_retry_request",
                    enabled: true,
                    targetTaskId: "legacy-task-1",
                    targetActionId: "legacy-action",
                    completionProofAfterDispatch: false,
                  },
                  {
                    id: "legacy-cancel",
                    label: "取消此任务",
                    kind: "cancel",
                    effect: "task_cancel_request",
                    enabled: true,
                    targetTaskId: "legacy-task-1",
                    completionProofAfterDispatch: false,
                  },
                ],
                nextRecommendedControl: "retry",
                latestResultPreview: {
                  status: "completed",
                  label: "completed",
                  preview: "LEGACY_RESULT_MUST_NOT_RENDER",
                  evidenceRefs: [],
                },
                evidenceRefs: [],
                updatedAt,
              },
            ],
            summary: {
              total: 1,
              activeCount: 0,
              waitingPermissionCount: 0,
              blockedCount: 0,
              pendingReviewCount: 0,
              completedCount: 0,
              completedNeedsEvidenceCount: 1,
              failedCount: 0,
              cancelledCount: 0,
              byLifecycleStatus: { completed_needs_evidence: 1 },
            },
            sourceRefs: [],
            contractLimitations: [],
          },
          status: "ready",
          lastUpdatedAt: updatedAt,
          source: "backend-readmodel",
          evidenceRefs: [],
          warnings: [],
          actions: { primary: [] },
        });
      }
      return mockInvoke(cmd);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("旧版执行元数据未验证")).toBeInTheDocument();
    expect(screen.getByText(/receipt、route 与 digest 均不可作为已观察事实/)).toBeInTheDocument();
    expect(screen.getAllByText("路线未验证").length).toBeGreaterThan(0);
    expect(screen.getAllByText("工具调用未验证").length).toBeGreaterThan(0);
    expect(screen.queryByText(/forged-provider/)).not.toBeInTheDocument();
    expect(screen.queryByText(/forged actual route/)).not.toBeInTheDocument();
    expect(screen.queryByText(new RegExp(`sha256:${"a".repeat(64)}`))).not.toBeInTheDocument();
    expect(screen.getByText("任务未知")).toBeInTheDocument();
    expect(screen.getByText("下一步：查看记录")).toBeInTheDocument();
    expect(screen.getByText("交付：未知")).toBeInTheDocument();
    expect(screen.getAllByText("状态未记录").length).toBeGreaterThan(0);
    expect(screen.getAllByText("阻断状态未验证").length).toBeGreaterThan(0);
    expect(screen.queryByText("任务缺少完成证据")).not.toBeInTheDocument();
    expect(screen.queryByText(/LEGACY_RESULT_MUST_NOT_RENDER/)).not.toBeInTheDocument();
    expect(screen.queryByText(/LEGACY_RUN_ERROR_MUST_NOT_RENDER/)).not.toBeInTheDocument();
    expect(screen.queryByText(/LEGACY_BLOCKER_MUST_NOT_RENDER/)).not.toBeInTheDocument();
    expect(screen.queryByText("待审核：1")).not.toBeInTheDocument();
    expect(screen.queryByText("待确认 1")).not.toBeInTheDocument();
    expect(screen.queryByText("连续性需复核")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "重试此任务" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "取消此任务" })).not.toBeInTheDocument();
  });

  it("preflights selected run deletion before calling the final delete command", async () => {
    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("camel case user input")).toBeInTheDocument();
    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[1]);
    fireEvent.click(screen.getByRole("button", { name: "删除" }));

    expect(
      await screen.findByRole("dialog", { name: "动作预检：删除运行记录" })
    ).toBeInTheDocument();
    expect(screen.getByText("写入 durable state")).toBeInTheDocument();
    expect(screen.getByText("影响数量")).toBeInTheDocument();
    expect(screen.getByText("id / scope digest")).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "delete_agent_run")).toBe(false);

    const continueButton = screen.getByRole("button", { name: "继续删除" });
    expect(continueButton).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/输入 DELETE RUN 以继续/), {
      target: { value: "WRONG" },
    });
    expect(continueButton).toBeDisabled();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "delete_agent_run")).toBe(false);

    fireEvent.change(screen.getByLabelText(/输入 DELETE RUN 以继续/), {
      target: { value: "DELETE RUN" },
    });
    fireEvent.click(continueButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "delete_agent_run",
        expect.objectContaining({
          runId: "run-1",
          confirmationEvidence: expect.objectContaining({
            actionType: "agent_run_delete",
            confirmationPhrase: "DELETE RUN",
            targetIds: ["run-1"],
          }),
        })
      );
    });
  });
});
