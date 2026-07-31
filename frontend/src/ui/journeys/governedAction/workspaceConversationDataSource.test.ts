import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StreamMessageChunkPayload, StreamMessageStartPayload } from "@/tauri";

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
  startStreamMessage: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(event, handler);
    return mocks.unlisten;
  }),
}));

vi.mock("@/tauri", () => ({
  cancelMainChatAgentTask: vi.fn(),
  createChatSession: vi.fn(),
  deleteChatSession: vi.fn(),
  getChatHistory: vi.fn(),
  listChatSessions: vi.fn(),
  renameChatSession: vi.fn(),
  startStreamMessage: mocks.startStreamMessage,
}));

import { tauriWorkspaceConversationDataSource } from "./workspaceConversationDataSource";

describe("workspace conversation Tauri stream adapter", () => {
  beforeEach(() => {
    mocks.listeners.clear();
    mocks.unlisten.mockClear();
    mocks.startStreamMessage.mockReset();
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
    const pending = tauriWorkspaceConversationDataSource.streamTurn(
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
        task_session_id: "wrong-task",
        run_id: "wrong-run",
        reasoning_trace: {},
        tool_calls: [],
      } satisfies StreamMessageStartPayload,
    });
    chunk({
      payload: {
        session_id: "another-conversation",
        operation_id: "operation-1",
        task_session_id: "wrong-task",
        run_id: "wrong-run",
        chunk: "错误回复",
      } satisfies StreamMessageChunkPayload,
    });
    expect(onStart).not.toHaveBeenCalled();
    expect(onChunk).not.toHaveBeenCalled();

    const exactStart = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_session_id: "task-1",
      run_id: "run-1",
      reasoning_trace: {},
      tool_calls: [],
    } satisfies StreamMessageStartPayload;
    const exactChunk = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_session_id: "task-1",
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
      task_session_id: "task-1",
      run_id: "run-1",
      reply: "正确回复",
      status: "completed",
      reasoning_trace: {},
      tool_calls: [],
    });
    await pending;
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });

  it("always removes installed listeners when starting the stream fails", async () => {
    mocks.startStreamMessage.mockRejectedValue(new Error("stream_start_failed"));

    await expect(
      tauriWorkspaceConversationDataSource.streamTurn(
        "conversation-1",
        [{ role: "user", content: "继续" }],
        { operationId: "operation-2" },
        { onStart: vi.fn(), onChunk: vi.fn() }
      )
    ).rejects.toThrow("stream_start_failed");
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });
});
