import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
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

describe("AgentControlPlane", () => {
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
    expect(screen.getByRole("link", { name: "open_review_center" })).toHaveAttribute(
      "href",
      "/review"
    );
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
});
