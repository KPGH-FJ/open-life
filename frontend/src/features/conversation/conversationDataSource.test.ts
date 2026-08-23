import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StreamMessageChunkPayload, StreamMessageStartPayload } from "@/tauri";

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
  startStreamMessage: vi.fn(),
  cancelChatTurn: vi.fn(),
  createProject: vi.fn(),
  assignConversationProject: vi.fn(),
  setConversationMemoryMode: vi.fn(),
  pickAndImportResources: vi.fn(),
  detachResourceFromTurn: vi.fn(),
  listMainChatSkills: vi.fn(),
  selectMainChatSkill: vi.fn(),
  clearMainChatSkill: vi.fn(),
  listMainChatToolCandidates: vi.fn(),
  getConversationViewModel: vi.fn(),
  submitMainChatTaskSteering: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(event, handler);
    return mocks.unlisten;
  }),
}));

vi.mock("@/ipc/conversation", () => ({
  cancelChatTurn: mocks.cancelChatTurn,
  createChatSession: vi.fn(),
  createProject: mocks.createProject,
  assignConversationProject: mocks.assignConversationProject,
  setConversationMemoryMode: mocks.setConversationMemoryMode,
  deleteChatSession: vi.fn(),
  getConversationViewModel: mocks.getConversationViewModel,
  renameChatSession: vi.fn(),
  startStreamMessage: mocks.startStreamMessage,
  pickAndImportResources: mocks.pickAndImportResources,
  detachResourceFromTurn: mocks.detachResourceFromTurn,
  listMainChatSkills: mocks.listMainChatSkills,
  selectMainChatSkill: mocks.selectMainChatSkill,
  clearMainChatSkill: mocks.clearMainChatSkill,
  listMainChatToolCandidates: mocks.listMainChatToolCandidates,
  submitMainChatTaskSteering: mocks.submitMainChatTaskSteering,
}));

import { tauriConversationDataSource } from "./conversationDataSource";

describe("Conversation Tauri stream adapter", () => {
  beforeEach(() => {
    mocks.listeners.clear();
    mocks.unlisten.mockClear();
    mocks.startStreamMessage.mockReset();
    mocks.createProject.mockReset();
    mocks.assignConversationProject.mockReset();
    mocks.setConversationMemoryMode.mockReset();
    mocks.pickAndImportResources.mockReset();
    mocks.detachResourceFromTurn.mockReset();
    mocks.listMainChatSkills.mockReset();
    mocks.selectMainChatSkill.mockReset();
    mocks.clearMainChatSkill.mockReset();
    mocks.listMainChatToolCandidates.mockReset();
    mocks.getConversationViewModel.mockReset();
    mocks.submitMainChatTaskSteering.mockReset();
  });

  it("forwards skill selection and tool discovery through system-owned bridges", async () => {
    mocks.listMainChatSkills.mockResolvedValue([]);
    mocks.selectMainChatSkill.mockResolvedValue({ sessionId: "conversation-1", skillId: "review" });
    mocks.clearMainChatSkill.mockResolvedValue({ sessionId: "conversation-1", skillId: null });
    mocks.listMainChatToolCandidates.mockResolvedValue({ candidates: [], blockedCount: 0 });

    await tauriConversationDataSource.listSkills?.("conversation-1");
    await tauriConversationDataSource.selectSkill?.("conversation-1", "review");
    await tauriConversationDataSource.clearSkill?.("conversation-1");
    await tauriConversationDataSource.listToolCandidates?.("task-1");

    expect(mocks.listMainChatSkills).toHaveBeenCalledWith("conversation-1");
    expect(mocks.selectMainChatSkill).toHaveBeenCalledWith("conversation-1", "review");
    expect(mocks.clearMainChatSkill).toHaveBeenCalledWith("conversation-1");
    expect(mocks.listMainChatToolCandidates).toHaveBeenCalledWith("task-1");
  });

  it("forwards resource import and detach through the exact Tauri bridge", async () => {
    mocks.pickAndImportResources.mockResolvedValue({ cancelled: true, receipt: null });
    mocks.detachResourceFromTurn.mockResolvedValue({ bindingRemoved: true });

    await tauriConversationDataSource.pickResources("import-1", "turn-1");
    await tauriConversationDataSource.detachResource("detach-1", "turn-1", "resource-1");

    expect(mocks.pickAndImportResources).toHaveBeenCalledWith("import-1", "turn-1");
    expect(mocks.detachResourceFromTurn).toHaveBeenCalledWith("detach-1", "turn-1", "resource-1");
  });

  it("forwards the selected Conversation Memory mode to the canonical command", async () => {
    mocks.setConversationMemoryMode.mockResolvedValue({
      conversationId: "conversation-1",
      mode: "use_only",
    });

    await tauriConversationDataSource.setMemoryMode?.("conversation-1", "use_only");

    expect(mocks.setConversationMemoryMode).toHaveBeenCalledWith("conversation-1", "use_only");
  });

  it("forwards only events bound to the exact conversation and operation", async () => {
    let finish!: (value: unknown) => void;
    mocks.startStreamMessage.mockImplementation(
      () =>
        new Promise(resolve => {
          finish = resolve;
        })
    );
    const onStart = vi.fn();
    const onChunk = vi.fn();
    const pending = tauriConversationDataSource.streamTurn(
      "conversation-1",
      [{ role: "user", content: "继续" }],
      { operationId: "operation-1" },
      { onStart, onChunk }
    );
    await vi.waitFor(() => expect(mocks.listeners.size).toBe(2));

    const start = mocks.listeners.get("stream-message-start")!;
    const chunk = mocks.listeners.get("stream-message-chunk")!;
    start({
      payload: {
        session_id: "conversation-1",
        operation_id: "another-operation",
        task_id: "wrong-task",
        run_id: "wrong-run",
      } satisfies StreamMessageStartPayload,
    });
    chunk({
      payload: {
        session_id: "another-conversation",
        operation_id: "operation-1",
        task_id: "wrong-task",
        run_id: "wrong-run",
        chunk: "错误回复",
      } satisfies StreamMessageChunkPayload,
    });
    expect(onStart).not.toHaveBeenCalled();
    expect(onChunk).not.toHaveBeenCalled();

    const exactStart = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_id: "task-1",
      run_id: "run-1",
    } satisfies StreamMessageStartPayload;
    const exactChunk = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_id: "task-1",
      run_id: "run-1",
      chunk: "正确回复",
    } satisfies StreamMessageChunkPayload;
    start({ payload: exactStart });
    chunk({ payload: exactChunk });
    expect(onStart).toHaveBeenCalledWith(exactStart);
    expect(onChunk).toHaveBeenCalledWith(exactChunk);

    finish({
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_id: "task-1",
      run_id: "run-1",
      reply: "正确回复",
      status: "completed",
    });
    await pending;
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });

  it("always removes installed listeners when starting the stream fails", async () => {
    mocks.startStreamMessage.mockRejectedValue(new Error("stream_start_failed"));

    await expect(
      tauriConversationDataSource.streamTurn(
        "conversation-1",
        [{ role: "user", content: "继续" }],
        { operationId: "operation-2" },
        { onStart: vi.fn(), onChunk: vi.fn() }
      )
    ).rejects.toThrow("stream_start_failed");
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });
});
