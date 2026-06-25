import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ReasoningTracePanel from "./ReasoningTracePanel";
import type { ReasoningTrace } from "../tauri";

function renderTrace(trace: ReasoningTrace, show = true) {
  return render(<ReasoningTracePanel trace={trace} show={show} onToggle={vi.fn()} />);
}

describe("ReasoningTracePanel", () => {
  it("renders provider route runtime fact labels without inferring from prose", () => {
    renderTrace({
      generation_result: {
        text: "Route answer text should not be parsed for evidence.",
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "运行时路线",
        uiStatus: "completed",
        modelGenerated: true,
        currentTurnGenerationProvider: "openai",
        currentTurnGenerationModel: "gpt-slice-b-current",
        configuredProvider: "deepseek",
        configuredModel: "deepseek-chat",
        plannedRouteIfModelNeededProvider: "openai",
        plannedRouteIfModelNeededModel: "gpt-slice-b-current",
        routeLabels: [
          "current_turn_generation: actual openai / gpt-slice-b-current (cloud)",
          "last_completed_generation: anthropic / claude-last (cloud) run run-last",
          "configured_default_route: deepseek / deepseek-chat",
          "planned_route_if_model_needed: openai / gpt-slice-b-current (cloud) preflight=ready",
        ],
        providerPreflightStatus: "ready",
        providerPreflightBlockers: [],
      },
    });

    expect(screen.getByText("来源：运行时路线")).toBeInTheDocument();
    expect(screen.getByText("状态：completed")).toBeInTheDocument();
    expect(screen.getByText("模型路线证据")).toBeInTheDocument();
    expect(screen.getByText("current turn generation")).toBeInTheDocument();
    expect(screen.getByText("actual openai / gpt-slice-b-current (cloud)")).toBeInTheDocument();
    expect(screen.getByText("last completed generation")).toBeInTheDocument();
    expect(screen.getByText("anthropic / claude-last (cloud) run run-last")).toBeInTheDocument();
    expect(screen.getByText("configured default route")).toBeInTheDocument();
    expect(screen.getByText("deepseek / deepseek-chat")).toBeInTheDocument();
    expect(screen.getByText("planned route if model needed")).toBeInTheDocument();
    expect(
      screen.getByText("openai / gpt-slice-b-current (cloud) preflight=ready")
    ).toBeInTheDocument();
    expect(screen.getByText("provider preflight")).toBeInTheDocument();
    expect(screen.getByText("ready")).toBeInTheDocument();
  });

  it("renders blocked provider preflight as restricted route evidence", () => {
    renderTrace({
      generation_result: {
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "运行时路线",
        uiStatus: "restricted",
        modelGenerated: false,
        routeLabels: [
          "current_turn_generation: no model generated in this turn",
          "configured_default_route: openai / gpt-blocked",
          "planned_route_if_model_needed: ollama / llama3.1:latest (local) preflight=blocked",
        ],
        providerPreflightStatus: "blocked",
        providerPreflightBlockers: ["network_disabled", "provider_api_key_missing"],
      },
    });

    expect(screen.getByText("状态：restricted")).toBeInTheDocument();
    expect(screen.getByText("no model generated in this turn")).toBeInTheDocument();
    expect(
      screen.getByText("blocked (network_disabled, provider_api_key_missing)")
    ).toBeInTheDocument();
  });

  it("renders tool availability runtime facts as bounded evidence rows", () => {
    renderTrace({
      generation_result: {
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "工具可用性",
        uiStatus: "restricted",
        toolWebConfigEnabled: true,
        toolWebPolicyAllowed: false,
        toolWebReachabilityStatus: "unknown",
        toolWebAvailable: "blocked",
        toolMcpRegisteredCount: 1,
        toolMcpSafeReadCandidateCount: 0,
        toolMcpServerStatus: "unknown",
        toolMcpAvailable: "no_safe_read_candidate",
        toolWriteAvailable: "proposal_permission_or_blocker",
        toolWriteRequiresPermission: true,
        toolAvailabilityLabels: [
          "web: config_enabled=true credential_available=true policy_allowed=false reachability=unknown available=blocked",
          "mcp: registered_count=1 safe_read_candidate_count=0 server_status=unknown available=no_safe_read_candidate",
          "write: available=proposal_permission_or_blocker requires_permission=true silent_write_available=false",
        ],
      },
    });

    expect(screen.getByText("来源：工具可用性")).toBeInTheDocument();
    expect(screen.getByText("状态：restricted")).toBeInTheDocument();
    expect(screen.getByText("工具可用性证据")).toBeInTheDocument();
    expect(screen.getByText("web")).toBeInTheDocument();
    expect(
      screen.getByText(
        "config_enabled=true credential_available=true policy_allowed=false reachability=unknown available=blocked"
      )
    ).toBeInTheDocument();
    expect(screen.getByText("mcp")).toBeInTheDocument();
    expect(
      screen.getByText(
        "registered_count=1 safe_read_candidate_count=0 server_status=unknown available=no_safe_read_candidate"
      )
    ).toBeInTheDocument();
    expect(screen.getByText("web availability")).toBeInTheDocument();
    expect(
      screen.getByText("policy=false · reachability=unknown · available=blocked")
    ).toBeInTheDocument();
    expect(screen.queryByText(/RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER/)).not.toBeInTheDocument();
  });

  it("renders agent self-state rows from structured generation fields", () => {
    renderTrace({
      generation_result: {
        text: "DIRECT_PROSE_SHOULD_NOT_BE_STATUS should not become evidence.",
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "提案待审",
        uiStatus: "waiting_for_user",
        taskStatus: "waiting_permission",
        runStatus: "completed",
        deliveryStatus: "response_delivered_pending_review",
        pendingPermissionCount: 0,
        pendingProposalCount: 1,
        durableChangeStatus: "pending_review",
        durableChangeCompleted: false,
        completedResponse: true,
        finalDeliveryEvidence: true,
        assistantProseUsedForTaskStatus: false,
        selfStateEvidenceLabels: ["agent_run", "proposal_store", "task_session"],
      },
    });

    expect(screen.getByText("来源：提案待审")).toBeInTheDocument();
    expect(screen.getByText("状态：waiting_for_user")).toBeInTheDocument();
    expect(screen.getByText("任务状态证据")).toBeInTheDocument();
    expect(screen.getByText("task status")).toBeInTheDocument();
    expect(
      screen.getByText(
        "task=waiting_permission · run=completed · delivery=response_delivered_pending_review"
      )
    ).toBeInTheDocument();
    expect(screen.getByText("pending state")).toBeInTheDocument();
    expect(
      screen.getByText("permission=0 · proposal=1 · durable=pending_review")
    ).toBeInTheDocument();
    expect(screen.getByText("evidence")).toBeInTheDocument();
    expect(screen.getByText("agent_run, proposal_store, task_session")).toBeInTheDocument();
  });

  it("renders self-state observation and trace-gap evidence without parsing prose", () => {
    renderTrace({
      generation_result: {
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "工具观察",
        uiStatus: "completed",
        taskStatus: "completed",
        runStatus: "completed",
        deliveryStatus: "delivered",
        lastActionSummary: "action=file.read status=completed observation_source=file",
        observationCount: 1,
        selfStateEvidenceLabels: ["action_queue", "execution_transcript"],
      },
    });

    expect(screen.getByText("来源：工具观察")).toBeInTheDocument();
    expect(screen.getByText("last action")).toBeInTheDocument();
    expect(
      screen.getByText("action=file.read status=completed observation_source=file · observations=1")
    ).toBeInTheDocument();

    renderTrace({
      generation_result: {
        sourceType: "runtime_fact",
        uiPrimarySourceChip: "任务状态未知",
        uiStatus: "unknown",
        deliveryStatus: "unknown",
        runtimeFactTraceGap: true,
        traceGapCode: "task_session_missing",
      },
    });

    expect(screen.getByText("来源：任务状态未知")).toBeInTheDocument();
    expect(screen.getByText("trace gap")).toBeInTheDocument();
    expect(screen.getByText("task_session_missing")).toBeInTheDocument();
  });
});
