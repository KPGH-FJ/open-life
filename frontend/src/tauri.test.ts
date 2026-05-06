import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  addDailyGoal,
  acceptProposal,
  builderStart,
  editProposal,
  getStateHistory,
  recordState,
  restoreArchivedChunks,
  saveChatMessage,
  startStreamMessage,
  getAgentSpec,
  listAgentSpecs,
  getDefaultAgentSpec,
  updateAgentSpec,
  setDefaultAgentSpec,
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

// ── P7 stabilization: AgentSpec wrapper command tests ─────────────

describe("AgentSpec command aliases", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("getAgentSpec invokes get_agent_spec with dual arg aliases", async () => {
    vi.mocked(invoke).mockResolvedValue({
      id: "main.default",
      role: "main",
      name: "OpenLife Main Agent",
    });
    await getAgentSpec("main.default");

    expect(invoke).toHaveBeenCalledWith(
      "get_agent_spec",
      expect.objectContaining({
        specId: "main.default",
        spec_id: "main.default",
      })
    );
  });

  it("listAgentSpecs invokes list_agent_specs", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    await listAgentSpecs();
    expect(invoke).toHaveBeenCalledWith("list_agent_specs", undefined);
  });

  it("getDefaultAgentSpec invokes get_default_agent_spec", async () => {
    vi.mocked(invoke).mockResolvedValue({
      id: "main.default",
      role: "main",
    });
    await getDefaultAgentSpec();
    expect(invoke).toHaveBeenCalledWith("get_default_agent_spec", undefined);
  });

  it("updateAgentSpec invokes update_agent_spec", async () => {
    const spec = { id: "main.default", role: "main", name: "Update Test" };
    await updateAgentSpec(spec as any);
    expect(invoke).toHaveBeenCalledWith("update_agent_spec", { spec });
  });

  it("setDefaultAgentSpec invokes set_default_agent_spec with dual arg aliases", async () => {
    await setDefaultAgentSpec("main.alt");

    expect(invoke).toHaveBeenCalledWith(
      "set_default_agent_spec",
      expect.objectContaining({
        specId: "main.alt",
        spec_id: "main.alt",
      })
    );
  });
});
