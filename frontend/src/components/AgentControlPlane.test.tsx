import { fireEvent, render, screen } from "@testing-library/react";
import React from "react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import AgentControlPlane from "./AgentControlPlane";
import type { MainChatAgentStateSnapshot } from "../tauri";

function agentState(
  overrides: Partial<MainChatAgentStateSnapshot> = {}
): MainChatAgentStateSnapshot {
  const base: MainChatAgentStateSnapshot = {
    task: {
      taskId: "task-agent-control-plane-1",
      runId: "run-agent-control-plane-1",
      conversationId: "session-1",
      userMessageId: "message-1",
      title: "Audit the final delivery",
      strategy: "react_tool_execution",
      status: "completed",
      createdAt: "2026-06-16T00:00:00.000Z",
      updatedAt: "2026-06-16T00:00:02.000Z",
      traceAvailable: true,
      controls: ["open_trace"],
      actionIds: ["action-1"],
      observationIds: ["observation-1"],
      blockerIds: [],
      proposalIds: ["proposal-1"],
      finalDeliveryId: "delivery-1",
    },
    route: { strategy: "react_tool_execution", reason: "fixture", confidence: 0.9 },
    context: [],
    actions: [
      {
        actionId: "action-1",
        actionType: "file.read",
        target: "plans/main_chat_final_delivery_contract_v1.md",
        label: "Read final delivery contract",
        status: "succeeded",
        riskLevel: "safe_read",
        policyDecisionId: "policy-action-1",
        observationIds: ["observation-1"],
        retryable: false,
      },
    ],
    observations: [
      {
        observationId: "observation-1",
        actionId: "action-1",
        sourceKind: "file",
        sourceLabel: "main_chat_final_delivery_contract_v1.md",
        preview: "Final delivery separates executed, proposed, blocked, and pending items.",
        citationAvailable: true,
        readExecution: {
          kind: "file_system_read",
          sourceKind: "file",
          sourceLabel: "main_chat_final_delivery_contract_v1.md",
          target: "plans/main_chat_final_delivery_contract_v1.md",
          realReadOnlyExecution: true,
          fixtureBacked: false,
          networkReadAttempted: false,
          directWritesExecuted: false,
        },
        createdAt: "2026-06-16T00:00:01.000Z",
      },
    ],
    blockers: [],
    proposals: [
      {
        proposalId: "proposal-1",
        proposalType: "memory",
        status: "pending_review",
        title: "Remember execution-first preference",
        summary: "Create a memory proposal after review.",
        evidenceIds: ["observation-1"],
        actionIds: [],
        controls: ["accept_proposal", "reject_proposal", "edit_proposal", "defer"],
      },
    ],
    finalDelivery: {
      deliveryId: "delivery-1",
      taskId: "task-agent-control-plane-1",
      runId: "run-agent-control-plane-1",
      status: "completed_with_pending_items",
      headline: "Audit ready",
      answer: "I read the final delivery contract and created one proposal.",
      completedActions: [
        {
          actionId: "action-1",
          actionType: "file.read",
          target: "plans/main_chat_final_delivery_contract_v1.md",
          status: "succeeded",
        },
      ],
      observationsUsed: [
        {
          observationId: "observation-1",
          sourceKind: "file",
          sourceLabel: "main_chat_final_delivery_contract_v1.md",
          preview: "Final delivery separates executed, proposed, blocked, and pending items.",
        },
      ],
      proposalsCreated: [
        {
          proposalId: "proposal-1",
          proposalType: "memory",
          status: "pending_review",
          summary: "Create a memory proposal after review.",
        },
      ],
      blockers: [{ blockerId: "blocker-1", reasonCode: "proposal_pending" }],
      pendingUserActions: [
        {
          pendingId: "proposal-1",
          kind: "proposal_review",
          controls: ["accept_proposal", "reject_proposal", "edit_proposal", "defer"],
        },
      ],
      durableChanges: [
        {
          changeType: "proposal_only",
          target: "memory.project.execution_first",
          provenanceId: "proposal-1",
          rollbackAvailable: false,
        },
      ],
      nextSteps: ["Review the proposal before it affects memory."],
      traceAvailable: true,
    },
    diagnostics: [],
    sequence: 12,
    emittedAt: "2026-06-16T00:00:02.000Z",
    events: [],
  };
  return { ...base, ...overrides };
}

function renderPanel(state: MainChatAgentStateSnapshot) {
  return render(
    <MemoryRouter>
      <AgentControlPlane state={state} />
    </MemoryRouter>
  );
}

function LocationProbe() {
  const location = useLocation();
  return (
    <div
      data-testid="location-probe"
      data-path={`${location.pathname}${location.search}`}
      data-state={JSON.stringify(location.state ?? null)}
    />
  );
}

function renderPanelWithRoutes(state: MainChatAgentStateSnapshot) {
  return render(
    <MemoryRouter initialEntries={["/"]}>
      <Routes>
        <Route path="/" element={<AgentControlPlane state={state} />} />
        <Route path="/mailbox" element={<LocationProbe />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("AgentControlPlane", () => {
  it("links proposal confirmation to the canonical Mailbox deep link and preserves resume state", () => {
    renderPanelWithRoutes(agentState());

    const link = screen.getByRole("link", { name: "Open Mailbox" });
    expect(link).toHaveAttribute("href", "/mailbox?proposal=proposal-1");

    fireEvent.click(link);

    const probe = screen.getByTestId("location-probe");
    expect(probe).toHaveAttribute("data-path", "/mailbox?proposal=proposal-1");
    expect(JSON.parse(probe.getAttribute("data-state") ?? "null")).toEqual({
      mainChatTaskSessionId: "task-agent-control-plane-1",
      returnTo: "/companion",
    });
  });

  it("renders reviewer trace identifiers from runtime state", () => {
    renderPanel(
      agentState({
        task: {
          ...agentState().task,
          status: "blocked",
          blockerIds: ["blocker-permission-1"],
        },
        blockers: [
          {
            blockerId: "blocker-permission-1",
            reasonCode: "tool_permission_required",
            title: "Permission required",
            detail: "Safe read needs scoped approval for action-1.",
            affectedActionId: "action-1",
            recoverable: true,
            controls: ["deny", "cancel", "open_trace"],
          },
        ],
      })
    );

    const trace = screen.getByTestId("agent-reviewer-trace");

    expect(trace).toHaveAttribute("data-task-session-id", "task-agent-control-plane-1");
    expect(trace).toHaveAttribute("data-run-id", "run-agent-control-plane-1");
    expect(trace).toHaveTextContent("task-agent-control-plane-1");
    expect(trace).toHaveTextContent("run-agent-control-plane-1");
    expect(trace).toHaveTextContent("tool_permission_required");
  });

  it("copies reviewer trace evidence as a bounded one-line JSON object", () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    renderPanel(
      agentState({
        task: {
          ...agentState().task,
          status: "blocked",
          blockerIds: ["blocker-permission-1"],
        },
        blockers: [
          {
            blockerId: "blocker-permission-1",
            reasonCode: "tool_permission_required",
            title: "Permission required",
            detail: "Safe read needs scoped approval for action-1.",
            affectedActionId: "action-1",
            recoverable: true,
            controls: ["deny", "cancel", "open_trace"],
          },
        ],
      })
    );

    fireEvent.click(screen.getByRole("button", { name: "Copy reviewer trace" }));

    expect(writeText).toHaveBeenCalledTimes(1);
    const copied = writeText.mock.calls[0][0] as string;
    expect(copied).not.toMatch(/\s{2,}|\n|\t/);
    expect(copied.length).toBeLessThanOrEqual(900);
    const parsed = JSON.parse(copied) as Record<string, unknown>;
    expect(Object.keys(parsed)).toEqual([
      "schemaVersion",
      "taskId",
      "runId",
      "status",
      "route",
      "blockers",
      "provider",
      "model",
      "finalDeliveryStatus",
      "timestamp",
    ]);
    expect(parsed).toMatchObject({
      schemaVersion: "main-chat-stage3-reviewer-trace-v1",
      taskId: "task-agent-control-plane-1",
      runId: "run-agent-control-plane-1",
      status: "blocked",
      route: "react_tool_execution",
      blockers: ["tool_permission_required"],
      provider: null,
      model: null,
      finalDeliveryStatus: "completed_with_pending_items",
    });
    expect(parsed.timestamp).toEqual(expect.stringMatching(/^\d{4}-\d{2}-\d{2}T/));
  });

  it("renders Stage 5 internal debug operations without readiness semantics", () => {
    const onRefreshStage5Preflight = vi.fn();
    const onExportDebugBundle = vi.fn();
    const onCreateIssueReport = vi.fn();

    render(
      <MemoryRouter>
        <AgentControlPlane
          state={agentState()}
          onRefreshStage5Preflight={onRefreshStage5Preflight}
          onExportDebugBundle={onExportDebugBundle}
          onCreateIssueReport={onCreateIssueReport}
          stage5Debug={{
            preflight: {
              failure: { class: "environment_preflight_failure" },
              metadataSafe: true,
              externalProviderInvokedByDefault: false,
              provider: { keyPresent: false },
            } as any,
            latestBundle: {
              bundleId: "stage5-bundle-1",
              artifact: {
                artifactId: "stage5-bundle-1",
                byteSize: 2048,
              },
            } as any,
            latestIssue: null,
            artifacts: [],
            busy: false,
            error: null,
          }}
        />
      </MemoryRouter>
    );

    const strip = screen.getByTestId("stage5-debug-operations");
    expect(strip).toHaveAttribute("data-preflight-status", "environment_preflight_failure");
    expect(strip).toHaveAttribute("data-metadata-safe", "true");
    expect(strip).toHaveAttribute("data-external-provider-invoked", "false");
    expect(strip).toHaveTextContent("Internal debug ops");
    expect(strip).toHaveTextContent("provider key missing");
    expect(strip).toHaveTextContent("bundle");

    fireEvent.click(screen.getByRole("button", { name: "Refresh Stage 5 preflight" }));
    fireEvent.click(screen.getByRole("button", { name: "Export debug bundle" }));
    fireEvent.click(screen.getByRole("button", { name: "Create issue report" }));

    expect(onRefreshStage5Preflight).toHaveBeenCalledTimes(1);
    expect(onExportDebugBundle).toHaveBeenCalledTimes(1);
    expect(onCreateIssueReport).toHaveBeenCalledTimes(1);
  });

  it("renders canonical final delivery sections separately", () => {
    renderPanel(agentState());

    expect(screen.getByText("Completed actions")).toBeInTheDocument();
    expect(screen.getAllByText(/file.read/).length).toBeGreaterThan(0);
    expect(screen.getByText("Sources used")).toBeInTheDocument();
    expect(screen.getAllByText("main_chat_final_delivery_contract_v1.md").length).toBeGreaterThan(
      0
    );
    expect(screen.getByText("Proposals created")).toBeInTheDocument();
    expect(screen.getByText("Pending user actions")).toBeInTheDocument();
    expect(screen.getByText("Durable changes")).toBeInTheDocument();
    expect(screen.getByText("Blocked items")).toBeInTheDocument();
    expect(screen.getByTestId("agent-control-plane")).toHaveAttribute(
      "data-final-delivery-section-titles",
      expect.stringContaining("Next steps")
    );
  });

  it("renders skipped final delivery work as its own terminal section", () => {
    renderPanel(
      agentState({
        finalDelivery: {
          ...agentState().finalDelivery!,
          status: "completed_with_pending_items",
          skippedWork: [
            {
              stepId: "plan-step-skipped-1",
              title: "Publish external note",
              reason: "external write is out of scope",
            },
          ],
        } as any,
      })
    );

    expect(screen.getByText("Skipped work")).toBeInTheDocument();
    expect(screen.getByText("Publish external note")).toBeInTheDocument();
    expect(screen.getByTestId("agent-control-plane")).toHaveAttribute(
      "data-final-delivery-section-titles",
      expect.stringContaining("Skipped work")
    );
  });

  it("renders backend plan artifact body with copy and supported controls only", () => {
    const artifactBody =
      "# Weekly Planning plan\n\nPlan ID: plan:artifact-1\nPlan session: plan-session-artifact-1\n\nSteps\n1. Verify museum facts - source/tool evidence: observation-hours-1";
    const writeText = vi.fn().mockResolvedValue(undefined);
    const onConfirmPlan = vi.fn();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });

    render(
      <MemoryRouter>
        <AgentControlPlane
          state={agentState({
            plan: {
              planId: "plan:artifact-1",
              planSessionId: "plan-session-artifact-1",
              taskSessionId: "task-agent-control-plane-1",
              runId: "run-plan-product-1",
              status: "draft",
              summary: "Draft PlanExecute plan with 1 step.",
              editable: true,
              source: "plan_execute",
              evidenceId: "plan-session-artifact-1",
              revision: 7,
              revisionId: "rev-7",
              controls: ["confirm_plan", "edit_plan", "cancel_task", "open_trace"],
              steps: [
                {
                  stepId: "step-artifact-1",
                  planId: "plan:artifact-1",
                  index: 1,
                  title: "Verify museum facts",
                  description: "Read source evidence before using realtime facts.",
                  kind: "read",
                  status: "planned",
                  revision: 7,
                  basePlanRevision: 7,
                  linkedActionIds: [],
                  linkedObservationIds: ["observation-hours-1"],
                  linkedProposalIds: [],
                  blockerIds: [],
                  evidenceIds: ["observation-hours-1"],
                  controls: ["skip_step"],
                },
              ],
              artifactView: {
                planId: "plan:artifact-1",
                planSessionId: "plan-session-artifact-1",
                taskSessionId: "task-agent-control-plane-1",
                runId: "run-plan-product-1",
                status: "draft",
                title: "Weekly Planning plan",
                summary: "Draft PlanExecute plan with 1 step.",
                body: artifactBody,
                steps: [
                  {
                    stepId: "step-artifact-1",
                    index: 1,
                    title: "Verify museum facts",
                    description: "Read source evidence before using realtime facts.",
                    status: "planned",
                    kind: "read",
                    evidenceIds: ["observation-hours-1"],
                    sourceToolEvidence: [
                      {
                        evidenceId: "observation-hours-1",
                        sourceKind: "web",
                        sourceLabel: "Sichuan Museum official opening hours",
                        toolName: "web_read",
                        preview: "Opening hours require same-day verification.",
                      },
                    ],
                    controls: ["skip_step"],
                  },
                ],
                assumptions: [
                  {
                    label: "Source-backed opening hours note",
                    detail: "Use only the attached source/tool evidence.",
                    evidenceIds: ["observation-hours-1"],
                    sourceToolEvidence: [
                      {
                        evidenceId: "observation-hours-1",
                        sourceKind: "web",
                        sourceLabel: "Sichuan Museum official opening hours",
                        toolName: "web_read",
                        preview: "Opening hours require same-day verification.",
                      },
                    ],
                  },
                ],
                unknowns: [
                  {
                    label: "weather",
                    detail: "No source/tool evidence is attached.",
                    evidenceIds: [],
                    sourceToolEvidence: [],
                  },
                ],
                controls: ["confirm_plan", "cancel_task", "open_trace"],
                routeEvidence: {
                  strategy: "plan_execute",
                  reason: "kernel_supported_plan_execute",
                  confidence: 0.92,
                  evidenceIds: [
                    "task-agent-control-plane-1",
                    "run-plan-product-1",
                    "plan-session-artifact-1",
                  ],
                },
                runEvidence: {
                  taskSessionId: "task-agent-control-plane-1",
                  runId: "run-plan-product-1",
                  planSessionId: "plan-session-artifact-1",
                  actionIds: ["action-plan-1"],
                  observationIds: ["observation-hours-1"],
                  proposalIds: [],
                  blockerIds: [],
                  finalDeliveryId: "delivery-1",
                  metadataSafe: true,
                },
              },
            },
          })}
          onConfirmPlan={onConfirmPlan}
        />
      </MemoryRouter>
    );

    const card = screen.getByTestId("agent-plan-artifact");
    expect(card).toHaveAttribute("data-plan-id", "plan:artifact-1");
    expect(card).toHaveAttribute("data-plan-session-id", "plan-session-artifact-1");
    expect(card).toHaveTextContent("Verify museum facts");
    expect(card).toHaveTextContent("Sichuan Museum official opening hours");
    expect(card).toHaveTextContent("weather");

    fireEvent.click(screen.getByRole("button", { name: "Copy plan artifact" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm plan" }));

    expect(writeText).toHaveBeenCalledWith(artifactBody);
    expect(onConfirmPlan).toHaveBeenCalledWith({
      planSessionId: "plan-session-artifact-1",
      baseRevision: 7,
    });
    expect(screen.queryByRole("button", { name: "Continue" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit plan" })).not.toBeInTheDocument();
  });

  it("renders a linked execution timeline with the current action emphasized", () => {
    renderPanel(
      agentState({
        task: {
          ...agentState().task,
          status: "executing",
          finalDeliveryId: undefined,
        },
        actions: [
          {
            ...agentState().actions[0],
            actionId: "action-running-1",
            label: "Read workspace file",
            status: "running",
            observationIds: ["observation-linked-1"],
          },
          {
            ...agentState().actions[0],
            actionId: "action-blocked-1",
            label: "Fetch governed web source",
            status: "blocked",
            observationIds: [],
          },
        ],
        observations: [
          {
            ...agentState().observations[0],
            observationId: "observation-linked-1",
            actionId: "action-running-1",
            sourceKind: "file",
            sourceLabel: "AGENTS.md",
          },
        ],
        blockers: [
          {
            blockerId: "blocker-web-policy-1",
            reasonCode: "web_network_policy_blocked",
            title: "Web blocked",
            detail: "Network access is disabled for this task.",
            affectedActionId: "action-blocked-1",
            recoverable: false,
            controls: ["cancel", "open_trace"],
          },
        ],
        finalDelivery: undefined,
      })
    );

    expect(screen.getByTestId("agent-execution-timeline")).toBeInTheDocument();
    expect(screen.getByTestId("agent-timeline-action-action-running-1")).toHaveAttribute(
      "data-current-action",
      "true"
    );
    expect(screen.getByTestId("agent-timeline-action-action-running-1")).toHaveTextContent(
      "observation observation-linked-1"
    );
    expect(screen.getByTestId("agent-timeline-action-action-blocked-1")).toHaveTextContent(
      "blocker blocker-web-policy-1"
    );
  });

  it("does not render unwired permission or proposal controls as fake controls", () => {
    renderPanel(
      agentState({
        task: {
          ...agentState().task,
          status: "waiting_for_user",
          controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
          blockerIds: ["blocker-permission-1"],
        },
        blockers: [
          {
            blockerId: "blocker-permission-1",
            reasonCode: "permission_required",
            title: "Permission required",
            detail: "Safe read needs scoped approval for action-1.",
            affectedActionId: "action-1",
            recoverable: true,
            controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
          },
        ],
      })
    );

    expect(screen.getByText("Permission required")).toBeInTheDocument();
    expect(screen.queryByText("approve_once")).not.toBeInTheDocument();
    expect(screen.queryByText("deny")).not.toBeInTheDocument();
    expect(screen.queryByText("defer")).not.toBeInTheDocument();
    expect(screen.queryByText("accept_proposal")).not.toBeInTheDocument();
    expect(screen.queryByText("reject_proposal")).not.toBeInTheDocument();
    expect(screen.queryByText("edit_proposal")).not.toBeInTheDocument();
    expect(screen.queryByText("rollback")).not.toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "Open Mailbox" })[0]).toHaveAttribute(
      "href",
      "/mailbox"
    );
  });

  it("wires proposal controls only when real handlers are supplied", () => {
    const onAcceptProposal = vi.fn();
    const onRejectProposal = vi.fn();
    const onEditProposal = vi.fn();
    const onDefer = vi.fn();
    render(
      <MemoryRouter>
        <AgentControlPlane
          state={agentState()}
          onAcceptProposal={onAcceptProposal}
          onRejectProposal={onRejectProposal}
          onEditProposal={onEditProposal}
          onDefer={onDefer}
        />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole("button", { name: "Accept proposal" }));
    fireEvent.click(screen.getByRole("button", { name: "Reject proposal" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit proposal" }));
    fireEvent.click(screen.getByRole("button", { name: "Defer" }));

    expect(onAcceptProposal).toHaveBeenCalledWith("proposal-1");
    expect(onRejectProposal).toHaveBeenCalledWith("proposal-1");
    expect(onEditProposal).toHaveBeenCalledWith("proposal-1");
    expect(onDefer).toHaveBeenCalledWith({ proposalId: "proposal-1" });
  });

  it("approves permission only for a ToolPermission proposal linked to the affected action", () => {
    const onApproveOnce = vi.fn();
    const onDeny = vi.fn();
    const onDefer = vi.fn();
    render(
      <MemoryRouter>
        <AgentControlPlane
          state={agentState({
            task: {
              ...agentState().task,
              status: "waiting_for_user",
              controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
              blockerIds: ["blocker-permission-1"],
              proposalIds: ["proposal-tool-permission-1"],
            },
            blockers: [
              {
                blockerId: "blocker-permission-1",
                reasonCode: "tool_permission_required",
                title: "Permission required",
                detail: "Read registered MCP notes.",
                affectedActionId: "action-1",
                recoverable: true,
                controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
              },
            ],
            proposals: [
              {
                proposalId: "proposal-tool-permission-1",
                proposalType: "tool_permission",
                status: "pending_review",
                title: "tool_permission proposal",
                summary: "Allow this exact read once.",
                evidenceIds: ["blocker-permission-1"],
                actionIds: ["action-1"],
                controls: ["accept_proposal", "reject_proposal", "defer", "open_review_center"],
              },
            ],
          })}
          onApproveOnce={onApproveOnce}
          onDeny={onDeny}
          onDefer={onDefer}
        />
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole("button", { name: "Approve once" }));
    fireEvent.click(screen.getByRole("button", { name: "Deny" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Defer" })[0]);

    expect(onApproveOnce).toHaveBeenCalledWith({
      proposalId: "proposal-tool-permission-1",
      actionId: "action-1",
      blockerId: "blocker-permission-1",
    });
    expect(onDeny).toHaveBeenCalledWith({
      proposalId: "proposal-tool-permission-1",
      actionId: "action-1",
      blockerId: "blocker-permission-1",
    });
    expect(onDefer).toHaveBeenCalledWith({
      proposalId: "proposal-tool-permission-1",
      actionId: "action-1",
      blockerId: "blocker-permission-1",
    });
  });

  it("does not approve permission when the proposal action target changed", () => {
    render(
      <MemoryRouter>
        <AgentControlPlane
          state={agentState({
            task: {
              ...agentState().task,
              status: "waiting_for_user",
              controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
              blockerIds: ["blocker-permission-1"],
              proposalIds: ["proposal-tool-permission-1"],
            },
            blockers: [
              {
                blockerId: "blocker-permission-1",
                reasonCode: "tool_permission_required",
                title: "Permission required",
                detail: "Read registered MCP notes.",
                affectedActionId: "action-1",
                recoverable: true,
                controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
              },
            ],
            proposals: [
              {
                proposalId: "proposal-tool-permission-1",
                proposalType: "tool_permission",
                status: "pending_review",
                title: "tool_permission proposal",
                summary: "Allow a different read.",
                evidenceIds: ["blocker-permission-1"],
                actionIds: ["changed-action"],
                controls: ["accept_proposal", "reject_proposal", "defer", "open_review_center"],
              },
            ],
          })}
          onApproveOnce={vi.fn()}
          onDeny={vi.fn()}
          onDefer={vi.fn()}
        />
      </MemoryRouter>
    );

    expect(screen.queryByRole("button", { name: "Approve once" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Deny" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "Open Mailbox" })[0]).toHaveAttribute(
      "href",
      "/mailbox"
    );
  });

  it("keeps rollback hidden unless a real rollback command is implemented", () => {
    renderPanel(
      agentState({
        proposals: [
          {
            proposalId: "proposal-accepted-1",
            proposalType: "memory",
            status: "accepted",
            title: "memory proposal",
            summary: "Accepted memory proposal.",
            evidenceIds: ["proposal-accepted-1"],
            actionIds: [],
            controls: ["rollback", "open_review_center"],
          },
        ],
      })
    );

    expect(screen.queryByRole("button", { name: /rollback/i })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open Mailbox" })).toHaveAttribute(
      "href",
      "/mailbox?proposal=proposal-accepted-1"
    );
  });

  it("renders rollback for accepted memory only when a real rollback handler is supplied", () => {
    const onRollbackMemory = vi.fn();
    render(
      <MemoryRouter>
        {React.createElement(AgentControlPlane as any, {
          state: agentState({
            proposals: [
              {
                proposalId: "proposal-accepted-1",
                proposalType: "memory",
                status: "accepted",
                title: "memory proposal",
                summary: "Accepted memory proposal.",
                evidenceIds: ["proposal-accepted-1", "memory-accepted-1"],
                actionIds: [],
                controls: ["rollback", "open_review_center"],
                memoryLifecycle: {
                  memoryId: "memory-accepted-1",
                  proposalId: "proposal-accepted-1",
                  content: "Accepted memory proposal.",
                  scope: "global",
                  category: "preference",
                  riskLevel: "low",
                  status: "materialized",
                  materializationStatus: "materialized",
                  createdBy: "agent",
                  evidenceIds: ["proposal-accepted-1"],
                  confidence: 0.82,
                  conflictIds: [],
                  materializedViewVersion: 2,
                },
              },
            ],
          }),
          onRollbackMemory,
        })}
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole("button", { name: "Rollback memory" }));

    expect(onRollbackMemory).toHaveBeenCalledWith("memory-accepted-1");
  });

  it("renders assembly diagnostics when governed runtime evidence is missing", () => {
    renderPanel(
      agentState({
        task: {
          ...agentState().task,
          status: "failed",
          title: "Agent state assembly diagnostics",
          controls: [],
          actionIds: [],
          observationIds: [],
          blockerIds: [],
          proposalIds: [],
          finalDeliveryId: undefined,
        },
        route: { strategy: "unknown", reason: "agent_state_assembly_failed" },
        actions: [],
        observations: [],
        blockers: [],
        proposals: [],
        finalDelivery: undefined,
        diagnostics: [
          {
            gapId: "agent-state-session-not-found",
            gapCode: "agent_state_session_not_found",
            detail: "Task session could not be loaded.",
            evidenceId: "task-missing",
          },
          {
            gapId: "agent-state-action-queue-store-unavailable",
            gapCode: "agent_state_action_queue_store_unavailable",
            detail: "Action queue store was unavailable.",
          },
        ],
      })
    );

    expect(screen.getByText("Agent state assembly diagnostics")).toBeInTheDocument();
    expect(screen.getByText("agent_state_session_not_found")).toBeInTheDocument();
    expect(screen.getByText("agent_state_action_queue_store_unavailable")).toBeInTheDocument();
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();
    expect(screen.queryByText("Observations")).not.toBeInTheDocument();
  });

  it("renders real read-only observation evidence without adding action controls", () => {
    renderPanel(agentState());

    expect(screen.getByText("file_system_read")).toBeInTheDocument();
    expect(screen.getByText("real read")).toBeInTheDocument();
    expect(screen.getByText("no writes")).toBeInTheDocument();
    expect(screen.queryByText("fixture")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /approve|reject|rollback/i })
    ).not.toBeInTheDocument();
  });
});
