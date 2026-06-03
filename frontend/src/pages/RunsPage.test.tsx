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
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
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
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("uses camelCase AgentRun fields for search, preview, proposals, and trash filtering", async () => {
    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("camel case output preview")).toBeInTheDocument();
    expect(screen.getAllByText("1 个提案").length).toBeGreaterThan(0);
    expect(screen.getByText("Multi-Strategy Preview")).toBeInTheDocument();
    expect(screen.getByText("Strategy: planExecute")).toBeInTheDocument();
    expect(screen.getByText("Governance: warn")).toBeInTheDocument();
    expect(screen.getByText("1 warning")).toBeInTheDocument();
    expect(screen.getByText("Plan-Execute Weekly Plan")).toBeInTheDocument();
    expect(screen.getByText("weekly_planning · 3 steps · 1 proposal")).toBeInTheDocument();
    expect(screen.getByText("Status: finalized")).toBeInTheDocument();
    expect(screen.getByText("Steps: 3")).toBeInTheDocument();
    expect(screen.getByText("Proposals: 1")).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-weekly-plan-should-not-render")
    ).not.toBeInTheDocument();
    expect(screen.queryByText("hidden by default")).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("搜索输入内容或输出..."), {
      target: { value: "plan-session-1" },
    });
    expect(screen.getByText("Plan-Execute Weekly Plan")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("搜索输入内容或输出..."), {
      target: { value: "weekly_planning_product" },
    });
    expect(screen.getByText("Plan-Execute Weekly Plan")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("搜索输入内容或输出..."), {
      target: { value: "camel case user" },
    });
    expect(screen.getByText(/camel case user input/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("已删除"));
    await waitFor(() => {
      expect(screen.getByText("hidden by default")).toBeInTheDocument();
    });
  });
});
