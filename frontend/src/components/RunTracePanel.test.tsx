import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import RunTracePanel from "./RunTracePanel";
import type { ProductAgentRun } from "../tauri";

const baseRun: ProductAgentRun = {
  id: "run-1",
  taskId: "task-1",
  status: "completed",
  kind: "planning",
  generatedProposals: [],
  actions: [],
  observations: [],
  legacyPayloadUnverified: false,
  behaviorChecks: [],
  statusUpdates: [],
  stepCount: 0,
  toolCallCount: 0,
  warnings: [],
  startedAt: new Date().toISOString(),
};

function renderPanel(run: ProductAgentRun) {
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
    } as unknown as ProductAgentRun);

    expect(screen.getByText("AI collaboration rules used")).toBeInTheDocument();
    expect(screen.getByText("Confirm before external writes")).toBeInTheDocument();
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

    expect(
      screen.getByText("Verified runtime trace is unavailable; execution details remain unknown.")
    ).toBeInTheDocument();
    expect(screen.queryByText("AI collaboration style")).not.toBeInTheDocument();
  });

  it("suppresses migrated legacy trace claims", () => {
    renderPanel({
      ...baseRun,
      legacyPayloadUnverified: true,
      actions: [
        {
          id: "legacy-action",
          actionType: "tool",
          status: "succeeded",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "legacy-action",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "tool",
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
    } as ProductAgentRun);

    expect(
      screen.getByText(/Legacy collaboration, tool, and strategy metadata is unverified/)
    ).toBeInTheDocument();
    expect(screen.queryByText("legacy.tool")).not.toBeInTheDocument();
    expect(screen.queryByText(new RegExp(`sha256:${"f".repeat(64)}`))).not.toBeInTheDocument();
  });

  it("renders ReAct action lifecycle metadata without raw payloads or PII", () => {
    renderPanel({
      ...baseRun,
      actions: [
        {
          id: "action-1",
          actionType: "mcp_tool",
          target: "file.write_proposal",
          status: "succeeded",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "action-1",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "mcp_tool",
            toolName: "file.write_proposal",
            toolSource: "builtin",
            actionCategory: "proposal",
            riskLevel: "high",
            status: "succeeded",
            proposalId: "proposal-1",
            observationId: "observation-1",
            outputPreview: "128 bytes redacted",
            outputReceipt: {
              version: 2,
              kind: "tool_output",
              provenance: "observed_tool_adapter_body",
              byteCount: 128,
              digest: `sha256:${"d".repeat(64)}`,
              verified: true,
            },
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
            toolName: "file.write_proposal",
            toolSource: "builtin",
            actionCategory: "proposal",
            riskLevel: "high",
            status: "succeeded",
            proposalId: "proposal-1",
            observationId: "observation-1",
            outputPreview: "128 bytes redacted",
            outputReceipt: {
              version: 2,
              kind: "tool_output",
              provenance: "observed_tool_adapter_body",
              byteCount: 128,
              digest: `sha256:${"d".repeat(64)}`,
              verified: true,
            },
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
    expect(screen.getByRole("link", { name: "Proposal: proposal-1" })).toHaveAttribute(
      "href",
      "/mailbox?proposal=proposal-1"
    );
    expect(screen.getByText("128 bytes redacted")).toBeInTheDocument();
    expect(screen.getAllByText(new RegExp(`sha256:${"d".repeat(64)}`)).length).toBeGreaterThan(0);
    expect(screen.queryByText(/raw-file-secret/)).not.toBeInTheDocument();
    expect(screen.queryByText(/secret@example.com/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw output should not render/)).not.toBeInTheDocument();
  });

  it("keeps a missing Product output receipt explicitly unknown", () => {
    renderPanel({
      ...baseRun,
      actions: [
        {
          id: "action-with-unknown-receipt",
          actionType: "mcp_tool",
          status: "failed",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "action-with-unknown-receipt",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "mcp_tool",
            toolName: "unknown_result_tool",
            toolSource: "mcp",
            actionCategory: "read",
            riskLevel: "low",
            status: "failed",
            metadataSafe: true,
          },
        },
      ],
    });

    expect(screen.getByText("unknown_result_tool")).toBeInTheDocument();
    expect(screen.getByText("Output receipt: unknown")).toBeInTheDocument();
    expect(screen.queryByText("Output receipt: verified")).not.toBeInTheDocument();
  });

  it("does not promote a Product trace that is explicitly not metadata-safe", () => {
    renderPanel({
      ...baseRun,
      actions: [
        {
          id: "unsafe-trace-action",
          actionType: "mcp_tool",
          status: "succeeded",
          timestamp: new Date().toISOString(),
          reactTrace: {
            actionId: "unsafe-trace-action",
            stepIndex: 1,
            toolCallIndex: 1,
            actionType: "mcp_tool",
            toolName: "D010_UNSAFE_TRACE_MUST_NOT_RENDER",
            toolSource: "mcp",
            actionCategory: "read",
            riskLevel: "low",
            status: "succeeded",
            metadataSafe: false,
          },
        },
      ],
    });

    expect(screen.queryByText("D010_UNSAFE_TRACE_MUST_NOT_RENDER")).not.toBeInTheDocument();
    expect(
      screen.getByText("Verified runtime trace is unavailable; execution details remain unknown.")
    ).toBeInTheDocument();
  });

  it("does not reinterpret compatibility action output or observation structuredResult as product trace", () => {
    const hostileCompatibilityRun = {
      ...baseRun,
      kind: "skill",
      actions: [
        {
          id: "compat-action",
          actionType: "skill_run",
          status: "completed_with_warnings",
          timestamp: new Date().toISOString(),
          output: {
            skillTrace: {
              traceKind: "skill_runtime",
              skillId: "D010_COMPAT_ACTION_OUTPUT_MUST_NOT_BECOME_FACT",
            },
          },
        },
      ],
      observations: [
        {
          id: "compat-observation",
          actionId: "compat-action",
          content: "D010_PRIVATE_OBSERVATION_BODY_MUST_NOT_RENDER",
          source: "unknown",
          timestamp: new Date().toISOString(),
          structuredResult: {
            skillTrace: {
              traceKind: "skill_runtime",
              skillId: "D010_COMPAT_STRUCTURED_RESULT_MUST_NOT_BECOME_FACT",
            },
          },
        },
      ],
    } as unknown as ProductAgentRun;

    renderPanel(hostileCompatibilityRun);

    expect(screen.queryByText("Skill Runtime trace")).not.toBeInTheDocument();
    expect(screen.queryByText(/D010_COMPAT/)).not.toBeInTheDocument();
    expect(screen.queryByText(/D010_PRIVATE/)).not.toBeInTheDocument();
    expect(
      screen.getByText("Verified runtime trace is unavailable; execution details remain unknown.")
    ).toBeInTheDocument();
  });

  it("does not render retired multi-strategy preview audit metadata", () => {
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
    } as unknown as ProductAgentRun);

    expect(screen.queryByText("Multi-strategy preview trace")).not.toBeInTheDocument();
    expect(screen.queryByText("Strategy: planExecute")).not.toBeInTheDocument();
    expect(screen.queryByText("Descriptor: plan_execute")).not.toBeInTheDocument();
    expect(screen.queryByText("preview runtime forces allowWrites=false")).not.toBeInTheDocument();
    expect(
      screen.getByText("Verified runtime trace is unavailable; execution details remain unknown.")
    ).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-payload-should-not-drive-trace")
    ).not.toBeInTheDocument();
  });

  it("does not reinterpret compatibility reasoningTrace as a Product runtime fact", () => {
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
    } as unknown as ProductAgentRun);

    expect(screen.queryByText("Plan-Execute product trace")).not.toBeInTheDocument();
    expect(screen.queryByText("Descriptor: plan_execute")).not.toBeInTheDocument();
    expect(screen.queryByText("Direct writes: none")).not.toBeInTheDocument();
    expect(screen.queryByText("External writes: none")).not.toBeInTheDocument();
    expect(
      screen.getByText("Verified runtime trace is unavailable; execution details remain unknown.")
    ).toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-weekly-plan-should-not-render")
    ).not.toBeInTheDocument();
  });
});
