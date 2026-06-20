import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import AgentStage, { AGENT_STAGE_STATES, type AgentStageState } from "./AgentStage";

const expectedLabels: Record<AgentStageState, string> = {
  idle: "安静待命",
  listening: "正在听",
  sorting: "整理中",
  memory: "翻看记忆",
  planning: "规划下一步",
  review: "有信等你回",
  privacy: "边界开启",
  error: "需要修复",
};

describe("AgentStage", () => {
  it("renders every W161 state with stable selectors", () => {
    for (const state of AGENT_STAGE_STATES) {
      const { unmount } = render(<AgentStage state={state} />);

      const stage = screen.getByTestId("agent-stage");
      expect(stage).toHaveAttribute("data-state", state);
      expect(screen.getByText(expectedLabels[state])).toBeInTheDocument();

      unmount();
    }
  });

  it("exposes accessible status text for the current state", () => {
    render(<AgentStage state="planning" />);

    const status = screen.getByRole("status", { name: /OpenLife Agent 状态/ });
    expect(status).toHaveAttribute("aria-live", "polite");
    expect(within(status).getByText("规划下一步")).toBeInTheDocument();
    expect(within(status).getByText(/压缩成一小步可执行的路径/)).toBeInTheDocument();
  });

  it("uses the cat yarn image as the visual figure", () => {
    render(<AgentStage state="idle" />);

    expect(screen.getByTestId("agent-stage-figure")).toHaveAttribute(
      "src",
      expect.stringContaining("cat-yarn.png")
    );
  });

  it("does not render raw prompt, memory, LifeModel, or tool payload fields", () => {
    const rawPayloadProps = {
      state: "memory",
      prompt: "raw prompt: call my bank",
      memory: "raw memory: private journal entry",
      lifeModel: "raw LifeModel: identity payload",
      toolPayload: "raw tool payload: filesystem write",
    } as unknown as { state: AgentStageState };

    render(<AgentStage {...rawPayloadProps} />);

    expect(screen.queryByText(/raw prompt/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/private journal/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw LifeModel/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/filesystem write/i)).not.toBeInTheDocument();
  });

  it("keeps the visual figure static without animation utility classes", () => {
    render(<AgentStage state="review" />);

    const figure = screen.getByTestId("agent-stage-figure");
    expect(figure.className).not.toContain("animate-");
    expect(figure.className).not.toContain("transition");
    expect(screen.queryByTestId("agent-stage-motion")).not.toBeInTheDocument();
  });
});
