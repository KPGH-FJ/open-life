import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import WorkspaceOverview from "./WorkspaceOverview";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("WorkspaceOverview contract", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-1",
            taskId: "task-1",
            status: "completed",
            kind: "conversation",
            userInput: "hello",
            outputPreview: "world",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-1",
            status: "pending",
            proposalType: "memory_write",
            source: "memory_governance",
            affectedPath: "memory.candidates",
            after: { content: "prefers concise replies" },
            reason: "candidate",
            confidence: 0.7,
            riskLevel: "medium",
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("loads runs through the formal listAgentRuns wrapper and reads camelCase dates", async () => {
    render(
      <MemoryRouter>
        <WorkspaceOverview />
      </MemoryRouter>
    );

    expect(await screen.findByText("今日 Agent Run")).toBeInTheDocument();
    expect(screen.getAllByText("1").length).toBeGreaterThan(0);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("list_agent_runs", {
      limit: 100,
      offset: 0,
    });
  });

  it("does not render plugin declarative-only skills as executable workspace skills", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") return Promise.resolve([] as any);
      if (cmd === "list_proposals") return Promise.resolve([] as any);
      if (cmd === "list_skills") {
        return Promise.resolve([
          {
            id: "weekly_review",
            name: "Weekly Review",
            description: "Built-in weekly review",
            requiredContext: [],
            allowedTools: [],
            executionBudget: {
              maxSteps: 5,
              maxToolCalls: 0,
              timeoutSeconds: 60,
              allowCloud: true,
              allowWrites: false,
            },
            outputSchema: {},
            proposalPolicy: "review_required",
            sourceKind: "built_in",
            executionStatus: "executable_built_in",
          },
          {
            id: "plugin:demo:weekly_review",
            name: "Plugin Weekly Review",
            description: "Declarative plugin skill",
            requiredContext: [],
            allowedTools: [],
            executionBudget: {
              maxSteps: 5,
              maxToolCalls: 0,
              timeoutSeconds: 60,
              allowCloud: true,
              allowWrites: false,
            },
            outputSchema: {},
            proposalPolicy: "review_required",
            sourceKind: "plugin",
            executionStatus: "disabled_declarative_only",
          },
        ] as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <WorkspaceOverview />
      </MemoryRouter>
    );

    expect(await screen.findByText("Weekly Review")).toBeInTheDocument();
    expect(screen.queryByText("Plugin Weekly Review")).not.toBeInTheDocument();
  });

  it("creates, edits, finalizes, and executes a weekly planning session", async () => {
    const user = userEvent.setup();
    const now = new Date().toISOString();
    const draftSession = {
      sessionId: "plan-session-1",
      sourceAgentRunId: "run-plan-1",
      sourceChatSessionId: "workspace_weekly_planning",
      scenario: "weekly_planning",
      status: "draft",
      createdAt: now,
      updatedAt: now,
      finalizedAt: null,
      metadataSafeObjective: "scenario=weekly_planning",
      stepCount: 2,
      completedStepCount: 0,
      proposalRequiredStepCount: 1,
      linkedProposalIds: [],
      warnings: [],
      steps: [
        {
          stepId: "step-1",
          order: 1,
          title: "Review current priorities",
          intent: "read_only_reasoning",
          toolName: null,
          actionKind: "reason",
          riskLevel: "low",
          declaredWrite: false,
          status: "planned",
          linkedProposalId: null,
          observationSummary: null,
          policyReasonCode: null,
          metadataSafeSummary: {},
        },
        {
          stepId: "step-2",
          order: 2,
          title: "Prepare weekly check-in proposal",
          intent: "write_like_schedule_task",
          toolName: "review_center.propose_scheduled_task",
          actionKind: "schedule",
          riskLevel: "medium",
          declaredWrite: true,
          status: "planned",
          linkedProposalId: null,
          observationSummary: null,
          policyReasonCode: null,
          metadataSafeSummary: {},
        },
      ],
      metadataSafeSummary: {},
    };
    const finalizedSession = { ...draftSession, status: "finalized", finalizedAt: now };
    const afterReadSession = {
      ...finalizedSession,
      status: "in_progress",
      completedStepCount: 1,
      steps: finalizedSession.steps.map(step =>
        step.stepId === "step-1"
          ? {
              ...step,
              status: "executed",
              observationSummary: "read-only internal reasoning completed; raw prompt omitted",
            }
          : step
      ),
    };
    const afterProposalSession = {
      ...afterReadSession,
      status: "completed",
      linkedProposalIds: ["proposal-plan-1"],
      steps: afterReadSession.steps.map(step =>
        step.stepId === "step-2"
          ? { ...step, status: "requires_proposal", linkedProposalId: "proposal-plan-1" }
          : step
      ),
    };

    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_plan_execute_sessions") return Promise.resolve([] as any);
      if (cmd === "create_plan_execute_session") return Promise.resolve(draftSession as any);
      if (cmd === "update_plan_execute_session_draft") {
        return Promise.resolve({
          ...draftSession,
          steps: draftSession.steps.map(step =>
            step.stepId === "step-1"
              ? { ...step, title: args?.input?.steps?.[0]?.title ?? step.title }
              : step
          ),
        } as any);
      }
      if (cmd === "finalize_plan_execute_session") return Promise.resolve(finalizedSession as any);
      if (cmd === "execute_plan_execute_step") {
        const stepId = args?.input?.stepId;
        return Promise.resolve({
          session: stepId === "step-1" ? afterReadSession : afterProposalSession,
          executedStep: {
            sessionId: "plan-session-1",
            stepId,
            stepStatus: stepId === "step-1" ? "executed" : "requires_proposal",
            linkedProposalId: stepId === "step-1" ? null : "proposal-plan-1",
            observationSummary:
              stepId === "step-1"
                ? "read-only internal reasoning completed; raw prompt omitted"
                : null,
            metadataSafeSummary: {},
          },
          metadataSafeSummary: {},
        } as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <WorkspaceOverview />
      </MemoryRouter>
    );

    await user.click(await screen.findByRole("button", { name: /开始本周规划/ }));
    expect(invoke).toHaveBeenCalledWith("create_plan_execute_session", {
      input: {
        scenarioId: "weekly_planning",
        sourceChatSessionId: "workspace_weekly_planning",
        maxSteps: 5,
      },
    });

    const titleInput = await screen.findByDisplayValue("Review current priorities");
    await user.clear(titleInput);
    await user.type(titleInput, "Review priorities before planning");
    await user.click(screen.getByRole("button", { name: /保存草稿/ }));
    expect(invoke).toHaveBeenCalledWith("update_plan_execute_session_draft", {
      input: {
        sessionId: "plan-session-1",
        steps: expect.arrayContaining([
          expect.objectContaining({
            stepId: "step-1",
            title: "Review priorities before planning",
          }),
        ]),
      },
    });

    await user.click(screen.getByRole("button", { name: /确认计划/ }));
    expect(invoke).toHaveBeenCalledWith("finalize_plan_execute_session", {
      input: { sessionId: "plan-session-1" },
    });

    await user.click(screen.getByRole("button", { name: /执行 step-1/ }));
    expect(await screen.findByText(/read-only internal reasoning completed/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /执行 step-2/ }));
    expect(await screen.findByText("proposal-plan-1")).toBeInTheDocument();
  });
});
