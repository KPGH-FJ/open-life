import { describe, expect, it } from "vitest";
import { inspectDailyGoalName, splitDailyGoalsByDisplayQuality } from "./dailyGoalDisplayGuard";

describe("dailyGoalDisplayGuard", () => {
  it("allows normal actionable goals", () => {
    expect(inspectDailyGoalName("阅读 30 分钟").valid).toBe(true);
    expect(inspectDailyGoalName("完成周报初稿").valid).toBe(true);
  });

  it("blocks state receipts, metric samples, and system feedback", () => {
    expect(inspectDailyGoalName("已记录状态 qapressure = 8 points").valid).toBe(false);
    expect(inspectDailyGoalName("qapressure = 8 points").valid).toBe(false);
    expect(inspectDailyGoalName("暂时无法发送普通对话：模型未就绪").valid).toBe(false);
  });

  it("blocks governance and tool blocker text from becoming goals", () => {
    for (const text of [
      "这次没有执行工具调用：当前请求选择的工具不在本轮治理允许范围内。",
      "That tool call is blocked by governance: model_selected_disallowed_tool",
      "That read action is blocked by governance: web_network_policy_blocked",
      "mcp_missing_read_target",
      "tool_permission_required",
    ]) {
      expect(inspectDailyGoalName(text).valid).toBe(false);
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
