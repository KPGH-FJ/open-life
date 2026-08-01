import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MainChatAgentTaskState, StreamMessageDonePayload } from "@/tauri";
import type { ChatMessage } from "@/types";
import type { WorkspaceConversationDataSource } from "./workspaceConversationDataSource";
import { useWorkspaceConversation } from "./useWorkspaceConversation";

const existingMessages: ChatMessage[] = [
  { role: "user", content: "整理今天的访谈" },
  { role: "assistant", content: "先确认本次范围。" },
];

function turnResult(status: StreamMessageDonePayload["status"]): StreamMessageDonePayload {
  return {
    session_id: "conversation-1",
    operation_id: "operation-1",
    task_session_id: "task-1",
    run_id: "run-1",
    reply: "我会先拆分步骤。",
    status,
    blockers: status === "blocked" ? ["permission_required"] : [],
    reasoning_trace: { steps: [] },
    tool_calls: [],
  } as StreamMessageDonePayload;
}

function source(overrides: Partial<WorkspaceConversationDataSource> = {}) {
  const dataSource: WorkspaceConversationDataSource = {
    listSessions: vi.fn().mockResolvedValue([
      {
        session_id: "conversation-1",
        title: "访谈整理",
        created_at: "2026-07-21T00:00:00Z",
        updated_at: "2026-07-21T00:01:00Z",
      },
    ]),
    loadHistory: vi.fn().mockResolvedValue(existingMessages),
    createSession: vi.fn().mockResolvedValue(undefined),
    renameSession: vi.fn().mockResolvedValue(undefined),
    deleteSession: vi.fn().mockResolvedValue(undefined),
    streamTurn: vi.fn().mockResolvedValue(turnResult("completed")),
    cancelTask: vi.fn().mockResolvedValue({} as MainChatAgentTaskState),
    ...overrides,
  };
  return dataSource;
}

describe("workspace conversation journey", () => {
  it("loads the exact selected session and its persisted history", async () => {
    const dataSource = source();
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );

    await act(async () => result.current.reload());

    expect(result.current.loadStatus).toBe("ready");
    expect(result.current.selectedSessionId).toBe("conversation-1");
    expect(result.current.messages).toEqual(existingMessages);
    expect(dataSource.loadHistory).toHaveBeenCalledWith("conversation-1");
  });

  it("keeps pending work distinct after send and refreshes both history and work state", async () => {
    const refreshedMessages: ChatMessage[] = [
      ...existingMessages,
      { role: "user", content: "继续" },
      { role: "assistant", content: "需要你决定访问范围。" },
    ];
    const loadHistory = vi
      .fn()
      .mockResolvedValueOnce(existingMessages)
      .mockResolvedValueOnce(refreshedMessages);
    const dataSource = source({
      loadHistory,
      streamTurn: vi.fn().mockResolvedValue(turnResult("completed_with_pending_items")),
    });
    const afterTurn = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useWorkspaceConversation(dataSource, vi.fn(), afterTurn));
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));

    await act(async () => result.current.send());

    expect(dataSource.streamTurn).toHaveBeenCalledWith(
      "conversation-1",
      [...existingMessages, { role: "user", content: "继续" }],
      expect.objectContaining({ operationId: expect.any(String) }),
      expect.objectContaining({
        onStart: expect.any(Function),
        onChunk: expect.any(Function),
      })
    );
    expect(result.current.turnState).toMatchObject({
      phase: "resolved",
      status: "completed_with_pending_items",
    });
    expect(result.current.messages).toEqual(refreshedMessages);
    expect(afterTurn).toHaveBeenCalledTimes(1);
  });

  it("does not create a new session until the explicit send action", async () => {
    const dataSource = source({
      listSessions: vi.fn().mockResolvedValue([]),
      loadHistory: vi.fn().mockResolvedValueOnce([
        { role: "user", content: "规划本周" },
        { role: "assistant", content: "先从三个重点开始。" },
      ]),
    });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    expect(dataSource.createSession).not.toHaveBeenCalled();
    expect(result.current.sendAction().enabled).toBe(false);

    act(() => result.current.setDraft("规划本周"));
    await act(async () => result.current.send());

    expect(dataSource.createSession).toHaveBeenCalledWith(expect.any(String), "规划本周");
    expect(dataSource.streamTurn).toHaveBeenCalledTimes(1);
  });

  it("fails closed when post-dispatch history refresh cannot confirm persistence", async () => {
    const dataSource = source({
      loadHistory: vi
        .fn()
        .mockResolvedValueOnce(existingMessages)
        .mockRejectedValueOnce(new Error("history_refresh_failed")),
    });
    const afterTurn = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useWorkspaceConversation(dataSource, vi.fn(), afterTurn));
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));
    await act(async () => result.current.send());

    await waitFor(() => expect(result.current.turnState.phase).toBe("failed"));
    expect(result.current.turnState).toMatchObject({
      phase: "failed",
      stage: "refresh",
      reason: "history_refresh_failed",
    });
    expect(afterTurn).toHaveBeenCalledTimes(1);
  });

  it("shows only chunks from the exact active stream before persisted history is confirmed", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        _options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-1",
          task_session_id: "task-1",
          run_id: "run-1",
          reasoning_trace: {},
          tool_calls: [],
        });
        events.onChunk({
          session_id: "conversation-1",
          operation_id: "operation-1",
          task_session_id: "task-1",
          run_id: "run-1",
          chunk: "正在整理",
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));

    act(() => void result.current.send());

    await waitFor(() => expect(result.current.turnState.phase).toBe("streaming"));
    expect(result.current.streamingReply).toBe("正在整理");
    expect(result.current.activeTaskSessionId).toBe("task-1");

    await act(async () => finishTurn(turnResult("completed")));
    await waitFor(() => expect(result.current.turnState.phase).toBe("resolved"));
  });

  it("cancels the exact active task and waits for the stream terminal state", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        _options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-2",
          task_session_id: "task-2",
          run_id: "run-2",
          reasoning_trace: {},
          tool_calls: [],
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const cancelTask = vi.fn().mockResolvedValue({} as MainChatAgentTaskState);
    const dataSource = source({ streamTurn, cancelTask });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskSessionId).toBe("task-2"));

    await act(async () => result.current.cancel());

    expect(cancelTask).toHaveBeenCalledWith("task-2");
    expect(result.current.turnState.phase).toBe("cancelling");

    await act(async () => finishTurn(turnResult("cancelled")));
    await waitFor(() =>
      expect(result.current.turnState).toMatchObject({ phase: "resolved", status: "cancelled" })
    );
  });

  it("does not let a late cancel failure overwrite the stream terminal state", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    let rejectCancel!: (reason: Error) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        _options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-race",
          task_session_id: "task-race",
          run_id: "run-race",
          reasoning_trace: {},
          tool_calls: [],
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const cancelTask = vi.fn(
      () =>
        new Promise<MainChatAgentTaskState>((_resolve, reject) => {
          rejectCancel = reject;
        })
    );
    const announce = vi.fn();
    const dataSource = source({ streamTurn, cancelTask });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, announce, vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskSessionId).toBe("task-race"));

    let cancelPromise!: Promise<void>;
    act(() => {
      cancelPromise = result.current.cancel();
    });
    await waitFor(() => expect(result.current.turnState.phase).toBe("cancelling"));

    await act(async () => finishTurn(turnResult("completed")));
    await waitFor(() =>
      expect(result.current.turnState).toMatchObject({ phase: "resolved", status: "completed" })
    );

    await act(async () => rejectCancel(new Error("late_cancel_failure")));
    await act(async () => cancelPromise);

    expect(result.current.turnState).toMatchObject({ phase: "resolved", status: "completed" });
    expect(announce).not.toHaveBeenCalledWith("取消请求失败；当前不会把任务显示为已取消。");
  });

  it("renames the exact selected conversation and confirms it through a reload", async () => {
    const renamedSession = {
      session_id: "conversation-1",
      title: "新的名称",
      created_at: "2026-07-21T00:00:00Z",
      updated_at: "2026-07-21T00:02:00Z",
    };
    const listSessions = vi
      .fn()
      .mockResolvedValueOnce([renamedSession])
      .mockResolvedValueOnce([renamedSession]);
    const dataSource = source({ listSessions });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    await act(async () =>
      expect(await result.current.renameSelected("  新的   名称  ")).toBe(true)
    );

    expect(dataSource.renameSession).toHaveBeenCalledWith("conversation-1", "新的 名称");
    expect(listSessions).toHaveBeenCalledTimes(2);
    expect(result.current.sessionMutation.phase).toBe("idle");
  });

  it("deletes only after the explicit controller action and re-reads the remaining sessions", async () => {
    const listSessions = vi
      .fn()
      .mockResolvedValueOnce([
        {
          session_id: "conversation-1",
          title: "访谈整理",
          created_at: "2026-07-21T00:00:00Z",
          updated_at: "2026-07-21T00:01:00Z",
        },
      ])
      .mockResolvedValueOnce([]);
    const dataSource = source({ listSessions });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    expect(dataSource.deleteSession).not.toHaveBeenCalled();
    await act(async () => expect(await result.current.deleteSelected()).toBe(true));

    expect(dataSource.deleteSession).toHaveBeenCalledWith("conversation-1");
    expect(result.current.selectedSessionId).toBeNull();
    expect(result.current.messages).toEqual([]);
  });
});
