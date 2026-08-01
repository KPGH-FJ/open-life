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
    pickResources: vi.fn(async (importOperationId: string, turnOperationId: string) => ({
      cancelled: false,
      receipt: {
        operationId: importOperationId,
        messageId: turnOperationId,
        resources: [
          {
            resourceId: "4a006c47-67ee-4421-9f84-736f37926090",
            bindingId: "binding-1",
            filename: "notes.md",
            digest: "resource-digest",
            byteCount: 1024,
            chunkCount: 1,
            reusedExisting: false,
          },
        ],
        committedAt: "2026-08-01T00:00:00Z",
      },
    })),
    detachResource: vi.fn(
      async (operationId: string, turnOperationId: string, resourceId: string) => ({
        operationId,
        messageId: turnOperationId,
        resourceId,
        bindingRemoved: true,
        resourceDeleted: true,
        eventId: "detach-event-1",
        committedAt: "2026-08-01T00:00:00Z",
      })
    ),
    streamTurn: vi.fn().mockResolvedValue(turnResult("completed")),
    cancelTask: vi.fn().mockResolvedValue({} as MainChatAgentTaskState),
    ...overrides,
  };
  return dataSource;
}

describe("workspace conversation journey", () => {
  it("binds selected resources and the streamed turn to one exact operation", async () => {
    const pickResources = vi.fn(async (importOperationId: string, turnOperationId: string) => ({
      cancelled: false,
      receipt: {
        operationId: importOperationId,
        messageId: turnOperationId,
        resources: [
          {
            resourceId: "c4edfb29-972c-46d2-aea7-abf0ea75a40b",
            bindingId: "binding-exact-turn",
            filename: "research.md",
            digest: "digest-exact-turn",
            byteCount: 4096,
            chunkCount: 2,
            reusedExisting: false,
          },
        ],
        committedAt: "2026-08-01T00:00:00Z",
      },
    }));
    const streamTurn = vi.fn(
      async (
        sessionId,
        _messages,
        options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: options.operationId,
          run_id: options.operationId,
          reasoning_trace: {},
          tool_calls: [],
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: options.operationId,
          run_id: options.operationId,
        };
      }
    );
    const dataSource = source({ pickResources, streamTurn });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    await act(async () => expect(await result.current.attachResources()).toBe(true));
    const exactTurnOperationId = result.current.pendingResourceTurnOperationId;
    expect(exactTurnOperationId).toEqual(expect.any(String));
    expect(result.current.pendingResources).toHaveLength(1);
    expect(pickResources).toHaveBeenCalledWith(expect.any(String), exactTurnOperationId);

    act(() => result.current.setDraft("请总结附件并给出来源"));
    await act(async () => result.current.send());

    expect(streamTurn).toHaveBeenCalledWith(
      "conversation-1",
      expect.any(Array),
      { operationId: exactTurnOperationId },
      expect.any(Object)
    );
    expect(result.current.pendingResources).toEqual([]);
    expect(result.current.pendingResourceTurnOperationId).toBeNull();
  });

  it("rejects an import receipt bound to a different turn", async () => {
    const dataSource = source({
      pickResources: vi.fn(async (importOperationId: string) => ({
        cancelled: false,
        receipt: {
          operationId: importOperationId,
          messageId: "foreign-turn",
          resources: [
            {
              resourceId: "227d17ec-6eb8-4a92-bc8c-77093578f77d",
              bindingId: "foreign-binding",
              filename: "wrong.md",
              digest: "foreign-digest",
              byteCount: 128,
              chunkCount: 1,
              reusedExisting: false,
            },
          ],
          committedAt: "2026-08-01T00:00:00Z",
        },
      })),
    });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    await act(async () => expect(await result.current.attachResources()).toBe(false));

    expect(result.current.pendingResources).toEqual([]);
    expect(result.current.resourceMutation).toMatchObject({
      phase: "failed",
      action: "import",
      reason: "resource_import_identity_mismatch",
    });
  });

  it("keeps a resource bound when detach is not confirmed by the backend", async () => {
    const detachResource = vi.fn(async () => ({
      operationId: "wrong-operation",
      messageId: "wrong-turn",
      resourceId: "wrong-resource",
      bindingRemoved: false,
      resourceDeleted: false,
      eventId: "wrong-event",
      committedAt: "2026-08-01T00:00:00Z",
    }));
    const dataSource = source({ detachResource });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());
    const resourceId = result.current.pendingResources[0].resourceId;

    await act(async () => expect(await result.current.detachResource(resourceId)).toBe(false));

    expect(result.current.pendingResources).toHaveLength(1);
    expect(result.current.resourceMutation).toMatchObject({
      phase: "failed",
      action: "detach",
      reason: "resource_detach_identity_mismatch",
    });
  });

  it("retains the selected resource and draft when streaming fails before task start", async () => {
    const dataSource = source({
      streamTurn: vi.fn().mockRejectedValue(new Error("stream_start_failed")),
    });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());
    act(() => result.current.setDraft("请读取这个文件"));

    await act(async () => result.current.send());

    expect(result.current.turnState).toMatchObject({
      phase: "failed",
      stage: "send",
      reason: "stream_start_failed",
    });
    expect(result.current.draft).toBe("请读取这个文件");
    expect(result.current.pendingResources).toHaveLength(1);
  });

  it("fails closed when an attachment turn returns a foreign terminal identity", async () => {
    const dataSource = source({
      streamTurn: vi.fn().mockResolvedValue({
        ...turnResult("completed"),
        session_id: "conversation-1",
        operation_id: "foreign-operation",
        task_session_id: "foreign-task",
      }),
    });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());
    act(() => result.current.setDraft("请读取这个文件"));

    await act(async () => result.current.send());

    expect(result.current.turnState).toMatchObject({
      phase: "failed",
      stage: "send",
      reason: "resource_turn_terminal_identity_mismatch",
    });
    expect(result.current.pendingResources).toHaveLength(1);
  });

  it("fails closed when an attachment stream starts with a foreign task identity", async () => {
    const streamTurn = vi.fn(
      async (
        sessionId,
        _messages,
        options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: "foreign-task",
          run_id: "foreign-run",
          reasoning_trace: {},
          tool_calls: [],
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: options.operationId,
          run_id: options.operationId,
        };
      }
    );
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());
    act(() => result.current.setDraft("请读取这个文件"));

    await act(async () => result.current.send());

    expect(result.current.turnState).toMatchObject({
      phase: "failed",
      stage: "send",
      reason: "resource_turn_terminal_identity_mismatch",
    });
    expect(result.current.pendingResources).toHaveLength(1);
  });

  it("ignores a foreign attachment chunk and fails the turn closed", async () => {
    const streamTurn = vi.fn(
      async (
        sessionId,
        _messages,
        options,
        events: Parameters<WorkspaceConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: options.operationId,
          run_id: options.operationId,
          reasoning_trace: {},
          tool_calls: [],
        });
        events.onChunk({
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: "foreign-task",
          run_id: "foreign-run",
          chunk: "不应显示的内容",
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_session_id: options.operationId,
          run_id: options.operationId,
        };
      }
    );
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());
    act(() => result.current.setDraft("请读取这个文件"));

    await act(async () => result.current.send());

    expect(result.current.streamingReply).toBe("");
    expect(result.current.turnState).toMatchObject({
      phase: "failed",
      stage: "send",
      reason: "resource_turn_terminal_identity_mismatch",
    });
  });

  it("does not switch conversations while a resource is bound to the pending turn", async () => {
    const announce = vi.fn();
    const dataSource = source();
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, announce, vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());

    act(() => result.current.startNewConversation());

    expect(result.current.selectedSessionId).toBe("conversation-1");
    expect(announce).toHaveBeenCalledWith("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
  });

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

  it("does not rename or delete a conversation while a resource is bound to the pending turn", async () => {
    const dataSource = source();
    const { result } = renderHook(() =>
      useWorkspaceConversation(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());

    await act(async () => expect(await result.current.renameSelected("新名称")).toBe(false));
    await act(async () => expect(await result.current.deleteSelected()).toBe(false));

    expect(dataSource.renameSession).not.toHaveBeenCalled();
    expect(dataSource.deleteSession).not.toHaveBeenCalled();
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
