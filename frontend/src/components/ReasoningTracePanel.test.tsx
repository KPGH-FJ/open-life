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
});
