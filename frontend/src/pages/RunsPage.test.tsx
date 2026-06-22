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
                  outputHash: "sha256:run1",
                  outputByteCount: 40,
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
      if (cmd === "list_main_chat_agent_tasks") {
        return Promise.resolve([
          {
            taskSessionId: "task-session-sensitive",
            conversationId: "session-sensitive",
            runId: "run-sensitive-1",
            title: "Sensitive running task",
            strategy: "direct_answer",
            status: "running",
            lastUpdatedAt: new Date().toISOString(),
            lastObservationPreview: "Waiting for model",
            pendingBlockerCount: 0,
            pendingProposalCount: 0,
            nextRecommendedControl: "cancel",
            staleState: "stale",
            resumeSafetyDigest: "sha256:test",
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
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
    expect(screen.getAllByText(/\[email\]/).length).toBeGreaterThan(0);
    expect(screen.getByText("任务运行中")).toBeInTheDocument();
    expect(screen.getByText("下一步：取消")).toBeInTheDocument();
    expect(screen.getByText("可能已卡住")).toBeInTheDocument();
    expect(screen.getAllByText("待确认 1").length).toBeGreaterThan(0);
    expect(screen.getAllByText("策略预览").length).toBeGreaterThan(0);
    expect(screen.getByText("策略：planExecute")).toBeInTheDocument();
    expect(screen.getByText("治理：warn")).toBeInTheDocument();
    expect(screen.getByText("1 warning")).toBeInTheDocument();
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
});
