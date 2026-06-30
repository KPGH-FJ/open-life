import { describe, expect, it } from "vitest";
import { inspectDailyGoalName, splitDailyGoalsByDisplayQuality } from "./dailyGoalDisplayGuard";

describe("dailyGoalDisplayGuard", () => {
  it("allows normal actionable goals", () => {
    expect(inspectDailyGoalName("阅读 30 分钟")).toMatchObject({
      valid: true,
      cardType: "goal",
    });
    expect(inspectDailyGoalName("完成周报初稿")).toMatchObject({
      valid: true,
      cardType: "goal",
    });
  });

  it("blocks state receipts, metric samples, and system feedback", () => {
    expect(inspectDailyGoalName("已记录状态 qapressure = 8 points")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("qapressure = 8 points")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("energy: 6/10")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("mood = anxious")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("pressure 8 分")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("confidence = 0.7")).toMatchObject({
      valid: false,
      cardType: "state_signal",
    });
    expect(inspectDailyGoalName("暂时无法发送普通对话：模型未就绪")).toMatchObject({
      valid: false,
      cardType: "blocker",
    });
  });

  it("classifies suggestions as suggestions instead of confirmed goals", () => {
    expect(inspectDailyGoalName("建议今天先休息 20 分钟")).toMatchObject({
      valid: false,
      cardType: "suggestion",
    });
  });

  it("blocks governance and tool blocker text from becoming goals", () => {
    for (const text of [
      "这次没有执行工具调用：当前请求选择的工具不在本轮治理允许范围内。",
      "That tool call is blocked by governance: model_selected_disallowed_tool",
      "That read action is blocked by governance: web_network_policy_blocked",
      "mcp_missing_read_target",
      "tool_permission_required",
    ]) {
      expect(inspectDailyGoalName(text)).toMatchObject({
        valid: false,
        cardType: "blocker",
      });
    }
  });

  it("splits polluted goals out of the displayable set", () => {
    const result = splitDailyGoalsByDisplayQuality([
      { name: "写完需求整理", done: false },
      { name: "这次没有执行工具调用：model_selected_disallowed_tool", done: false },
    ]);

    expect(result.displayable.map(goal => goal.name)).toEqual(["写完需求整理"]);
    expect(result.suspicious).toHaveLength(1);
  });
});
