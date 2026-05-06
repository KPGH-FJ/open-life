import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  addDailyGoal,
  acceptProposal,
  builderStart,
  editProposal,
  getStateHistory,
  normalizePlanExecutionResult,
  recordState,
  restoreArchivedChunks,
  saveChatMessage,
  startStreamMessage,
} from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("tauri command argument aliases", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("adds camelCase aliases for snake_case command arguments", async () => {
    await getStateHistory("专注度", 7);
    await restoreArchivedChunks([1, 2]);

    expect(invoke).toHaveBeenCalledWith(
      "get_state_history",
      expect.objectContaining({
        dimensionName: "专注度",
        dimension_name: "专注度",
        limit: 7,
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "restore_archived_chunks",
      expect.objectContaining({
        chunkIds: [1, 2],
        chunk_ids: [1, 2],
      })
    );
  });

  it("keeps existing explicit aliases for high-traffic chat and builder commands", async () => {
    await startStreamMessage("session-1", [{ role: "user", content: "你好" }]);
    await saveChatMessage("session-1", { role: "assistant", content: "你好" });
    await builderStart("incremental", "builder-1", "goals");

    expect(invoke).toHaveBeenCalledWith(
      "start_stream_message",
      expect.objectContaining({
        sessionId: "session-1",
        session_id: "session-1",
        args: expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
        }),
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "save_chat_message",
      expect.objectContaining({
        sessionId: "session-1",
        session_id: "session-1",
        message: { role: "assistant", content: "你好" },
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "builder_start",
      expect.objectContaining({
        sessionId: "builder-1",
        session_id: "builder-1",
        targetDimension: "goals",
        target_dimension: "goals",
      })
    );
  });

  it("normalizes optional state and daily-goal arguments before invoke", async () => {
    await recordState("睡眠", 7.5, "小时", "昨晚", 6, 9, 2);
    await addDailyGoal("阅读30分钟");

    expect(invoke).toHaveBeenCalledWith(
      "record_state",
      expect.objectContaining({
        dimensionName: "睡眠",
        dimension_name: "睡眠",
        minThreshold: 6,
        min_threshold: 6,
        maxThreshold: 9,
        max_threshold: 9,
        alertDays: 2,
        alert_days: 2,
      })
    );
    expect(invoke).toHaveBeenCalledWith("add_daily_goal", { name: "阅读30分钟" });
  });

  it("normalizes proposal command arguments", async () => {
    await acceptProposal("proposal-1");
    await editProposal("proposal-1", { name: "新值" });

    expect(invoke).toHaveBeenCalledWith(
      "accept_proposal",
      expect.objectContaining({
        proposalId: "proposal-1",
        proposal_id: "proposal-1",
      })
    );
    expect(invoke).toHaveBeenCalledWith(
      "edit_proposal",
      expect.objectContaining({
        proposalId: "proposal-1",
        proposal_id: "proposal-1",
        newAfter: { name: "新值" },
        new_after: { name: "新值" },
      })
    );
  });
});

// ── Plan execution result normalization ──────────────────────────────

describe("normalizePlanExecutionResult", () => {
  it("maps snake_case backend response to camelCase", () => {
    const result = normalizePlanExecutionResult({
      plan_id: "p-1",
      success: true,
      steps_completed: 3,
      steps_failed: 0,
      deviations: [],
      status: "completed",
    });

    expect(result).toEqual({
      planId: "p-1",
      success: true,
      stepsCompleted: 3,
      stepsFailed: 0,
      deviations: [],
      status: "completed",
    });
  });

  it("preserves already-camelCase response (mock passthrough)", () => {
    const result = normalizePlanExecutionResult({
      planId: "p-2",
      success: false,
      stepsCompleted: 1,
      stepsFailed: 1,
      deviations: ["step 0 deviated"],
    });

    expect(result.planId).toBe("p-2");
    expect(result.stepsCompleted).toBe(1);
    expect(result.stepsFailed).toBe(1);
    expect(result.deviations).toEqual(["step 0 deviated"]);
    expect(result.status).toBeUndefined();
  });

  it("defaults missing optional arrays to empty", () => {
    const result = normalizePlanExecutionResult({
      success: true,
    });

    expect(result.deviations).toEqual([]);
  });

  it("defaults missing numbers to zero", () => {
    const result = normalizePlanExecutionResult({
      success: false,
    });

    expect(result.stepsCompleted).toBe(0);
    expect(result.stepsFailed).toBe(0);
    expect(result.planId).toBe("");
  });

  it("preserves failed_review status", () => {
    const result = normalizePlanExecutionResult({
      plan_id: "p-3",
      success: false,
      status: "failed_review",
    });

    expect(result.planId).toBe("p-3");
    expect(result.success).toBe(false);
    expect(result.status).toBe("failed_review");
  });
});
