import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import RunTracePanel from "./RunTracePanel";
import type { AgentRun } from "../tauri";

const baseRun: AgentRun = {
  id: "run-1",
  taskId: "task-1",
  status: "completed",
  kind: "planning",
  generatedProposals: [],
  actions: [],
  observations: [],
  startedAt: new Date().toISOString(),
};

function renderPanel(run: AgentRun) {
  return render(
    <MemoryRouter>
      <RunTracePanel run={run} />
    </MemoryRouter>
  );
}

describe("RunTracePanel", () => {
  it("renders selected collaboration rules, styles, and behavior checks without raw HS terms", () => {
    renderPanel({
      ...baseRun,
      hsSelectionAudit: {
        selectedPolicyIds: ["policy.external_writes.proposal_first"],
        selectedHeuristicIds: ["heuristic.low_energy_planning"],
        estimatedTokens: 42,
        tokenBudget: 256,
      },
      behaviorChecks: [
        {
          id: "regression.external_write_proposal_first",
          label: "External writes stay reviewable",
          passed: true,
          summary: "Direct writes become proposals.",
        },
      ],
      outputPreview: "raw-sensitive-payload-should-not-drive-trace",
    });

    expect(screen.getByText("AI collaboration rules used")).toBeInTheDocument();
    expect(screen.getByText("Review before external writes")).toBeInTheDocument();
    expect(screen.getByText("AI collaboration style")).toBeInTheDocument();
    expect(screen.getByText("Low-energy planning style")).toBeInTheDocument();
    expect(screen.getByText("behavior check")).toBeInTheDocument();
    expect(screen.getByText("External writes stay reviewable")).toBeInTheDocument();
    expect(screen.queryByText(/heuristic/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-payload-should-not-drive-trace")
    ).not.toBeInTheDocument();
  });

  it("renders an empty state when no HS assets affect a run", () => {
    renderPanel(baseRun);

    expect(screen.getByText("No collaboration rules affected this run.")).toBeInTheDocument();
    expect(screen.queryByText("AI collaboration style")).not.toBeInTheDocument();
  });

  it("renders ReAct action lifecycle metadata without raw payloads or PII", () => {
    renderPanel({
      ...baseRun,
      actions: [
        {
          id: "action-1",
          actionType: "mcp_tool",
          target: "file.write_proposal",
          input: { arguments: { content: "raw-file-secret@example.com" } },
          status: "succeeded",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "action-1",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "mcp_tool",
            toolId: "file.write_proposal",
            toolName: "file.write_proposal",
            toolSource: "builtin",
            actionCategory: "proposal",
            riskLevel: "high",
            status: "succeeded",
            proposalId: "proposal-1",
            observationId: "observation-1",
            observationStatus: "succeeded",
            outputPreview: "128 bytes redacted",
            outputHash: "sha256:def456",
            outputByteCount: 128,
            metadataSafe: true,
          },
        },
      ],
      observations: [
        {
          id: "observation-1",
          actionId: "action-1",
          content: "raw observation with secret@example.com",
          source: "builtin",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "action-1",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "mcp_tool",
            toolId: "file.write_proposal",
            toolName: "file.write_proposal",
            toolSource: "builtin",
            actionCategory: "proposal",
            riskLevel: "high",
            status: "succeeded",
            proposalId: "proposal-1",
            observationId: "observation-1",
            observationStatus: "succeeded",
            outputPreview: "128 bytes redacted",
            outputHash: "sha256:def456",
            outputByteCount: 128,
            metadataSafe: true,
          },
        },
      ],
      outputPreview: "raw output should not render",
    });

    expect(screen.getByText("ReAct action lifecycle")).toBeInTheDocument();
    expect(screen.getByText("file.write_proposal")).toBeInTheDocument();
    expect(screen.getByText("Source: builtin")).toBeInTheDocument();
    expect(screen.getByText("Risk: high")).toBeInTheDocument();
    expect(screen.getByText("Status: succeeded")).toBeInTheDocument();
    expect(screen.getByText("Proposal: proposal-1")).toBeInTheDocument();
    expect(screen.getByText("128 bytes redacted")).toBeInTheDocument();
    expect(screen.getByText("sha256:def456")).toBeInTheDocument();
    expect(screen.queryByText(/raw-file-secret/)).not.toBeInTheDocument();
    expect(screen.queryByText(/secret@example.com/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw output should not render/)).not.toBeInTheDocument();
  });

  it("renders metadata-safe multi-strategy preview audit", () => {
    renderPanel({
      ...baseRun,
      reasoningStrategy: "multi_strategy_preview",
      reasoningTrace: {
        strategy_result: {
          previewRuntime: "multi_strategy",
          runtimeStrategyTraceKind: "multi_strategy_preview",
          selectedStrategyKind: "planExecute",
          strategyKind: "planExecute",
          payloadKind: "planExecute",
          strategyDescriptorId: "plan_execute",
          strategyCapabilityIds: ["planning.plan_execute"],
          governanceDecisionKind: "warn",
          selectionReasonCode: "write_like_intent",
          riskLevel: "medium",
          reasonCode: "write_like_intent",
          registryReady: true,
          defaultChatUnchanged: true,
          sideEffectBudget: { externalWrites: 0 },
          hasHsPacket: true,
          planStepCount: 1,
          planStepStatuses: ["requires_proposal"],
          warnings: ["preview runtime forces allowWrites=false"],
          blocked: false,
          metadataSafe: true,
        },
      },
      outputPreview: "raw-sensitive-payload-should-not-drive-trace",
    });

    expect(screen.getByText("Multi-strategy preview trace")).toBeInTheDocument();
    expect(screen.getByText("Strategy: planExecute")).toBeInTheDocument();
    expect(screen.getByText("Descriptor: plan_execute")).toBeInTheDocument();
    expect(screen.getByText("Registry: ready")).toBeInTheDocument();
    expect(screen.getByText("Governance: warn")).toBeInTheDocument();
    expect(screen.getByText("write_like_intent")).toBeInTheDocument();
    expect(screen.getByText("requires_proposal")).toBeInTheDocument();
    expect(screen.getByText("preview runtime forces allowWrites=false")).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-payload-should-not-drive-trace")
    ).not.toBeInTheDocument();
  });

  it("renders metadata-safe Plan-Execute product trace metadata", () => {
    renderPanel({
      ...baseRun,
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
          strategyCapabilityIds: ["planning.plan_execute"],
          selectionReasonCode: "weekly_planning_product",
          governanceDecisionKind: "require_proposal",
          registryReady: true,
          defaultChatUnchanged: true,
          sideEffectBudget: { externalWrites: 0 },
          status: "finalized",
          sourceAgentRunId: "run-plan-1",
          sourceChatSessionId: "workspace_weekly_planning",
          stepCount: 3,
          stepStatusCounts: {
            planned: 1,
            executed: 1,
            requiresProposal: 1,
            blocked: 0,
          },
          generatedProposalIds: ["proposal-1"],
          generatedProposalCount: 1,
          governanceDecisionCounts: {
            allow: 1,
            requireProposal: 1,
            block: 0,
          },
          warningCount: 0,
          metadataSafe: true,
          rawPromptStored: false,
          rawWeeklyPlanProseStored: false,
          rawLifeModelStored: false,
          rawMemoryStored: false,
          rawToolPayloadStored: false,
          rawProposalPayloadStored: false,
          directLifeModelWrites: false,
          externalWritesExecuted: false,
        },
      },
      outputPreview: "raw-sensitive-weekly-plan-should-not-render",
    });

    expect(screen.getByText("Plan-Execute product trace")).toBeInTheDocument();
    expect(screen.getByText("Descriptor: plan_execute")).toBeInTheDocument();
    expect(screen.getByText("Registry: ready")).toBeInTheDocument();
    expect(screen.getByText("Scenario: weekly_planning")).toBeInTheDocument();
    expect(screen.getByText("Session: plan-session-1")).toBeInTheDocument();
    expect(screen.getByText("Steps: 3")).toBeInTheDocument();
    expect(screen.getByText("Proposals: 1")).toBeInTheDocument();
    expect(screen.getByText("requires proposal: 1")).toBeInTheDocument();
    expect(screen.getByText("Direct writes: none")).toBeInTheDocument();
    expect(screen.getByText("External writes: none")).toBeInTheDocument();
    expect(screen.getByText("proposal-1")).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-weekly-plan-should-not-render")
    ).not.toBeInTheDocument();
  });
});
