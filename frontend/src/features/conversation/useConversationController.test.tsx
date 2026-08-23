import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ChatSession, ConversationViewModel, StreamMessageDonePayload } from "@/tauri";
import type { ChatMessage } from "@/types";
import type { ConversationDataSource } from "./conversationDataSource";
import { useConversationController } from "./useConversationController";

const existingMessages: ChatMessage[] = [
  { role: "user", content: "整理今天的访谈" },
  { role: "assistant", content: "先确认本次范围。" },
];

function turnResult(status: StreamMessageDonePayload["status"]): StreamMessageDonePayload {
  return {
    session_id: "conversation-1",
    operation_id: "operation-1",
    task_id: "task-1",
    run_id: "run-1",
    reply: "我会先拆分步骤。",
    status,
    blockers: status === "blocked" ? ["permission_required"] : [],
  } as StreamMessageDonePayload;
}

type ConversationTestSource = ConversationDataSource & {
  listSessions(): Promise<ChatSession[]>;
  loadHistory(sessionId: string): Promise<ChatMessage[]>;
};

function source(overrides: Partial<ConversationTestSource> = {}) {
  const listSessions =
    overrides.listSessions ??
    vi.fn().mockResolvedValue([
      {
        session_id: "conversation-1",
        title: "访谈整理",
        created_at: "2026-07-21T00:00:00Z",
        updated_at: "2026-07-21T00:01:00Z",
      },
    ]);
  const loadHistory = overrides.loadHistory ?? vi.fn().mockResolvedValue(existingMessages);
  const loadConversation =
    overrides.loadConversation ??
    vi.fn(async (conversationId?: string): Promise<ConversationViewModel> => {
      const conversations = await listSessions();
      const selectedConversationId =
        conversationId &&
        conversations.some((item: ChatSession) => item.session_id === conversationId)
          ? conversationId
          : (conversations[0]?.session_id ?? null);
      return {
        status: conversations.length > 0 ? "ready" : "empty",
        conversations,
        projects: [],
        selectedProjectId: null,
        selectedConversationId,
        globalMemoryEnabled: true,
        selectedMemoryMode: "use_and_learn",
        messages: selectedConversationId ? await loadHistory(selectedConversationId) : [],
        latestTurn: null,
        providerStatus: "ready",
        providerProfiles: [],
        selectedProviderProfileId: null,
        providerErrorCode: null,
        workStatus: "available",
      };
    });
  const dataSource: ConversationTestSource = {
    loadConversation,
    listSessions,
    loadHistory,
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
    cancelChatTurn: vi.fn().mockResolvedValue({ status: "cancelled" }),
    ...overrides,
  };
  return dataSource;
}

describe("conversation controller", () => {
  it("keeps the backend-owned Life Model influence receipt after a completed turn", async () => {
    const response = turnResult("completed");
    response.life_model_influence = {
      status: "applied_context_building",
      sourceId: "lifemodel.v2.runtime",
      modelVersion: 3,
      selectedItems: [
        {
          itemRef: "collaboration_preferences:communication-direct",
          statement: "沟通保持简洁直接",
          sourceRefs: ["message:user:send-stream-parity"],
          confirmedAt: "2026-08-09T00:00:00Z",
          reasonCode: "task intent matches collaboration_preferences",
        },
      ],
      appliedSurfaces: ["context_building", "communication_style"],
      currentInstructionPriorityPreserved: true,
      policyPriorityPreserved: true,
      permissionGranted: false,
      durableWriteAuthorized: false,
    };
    const dataSource = source({ streamTurn: vi.fn().mockResolvedValue(response) });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("请写一封项目邮件"));

    await act(async () => result.current.send());

    expect(result.current.turnState).toMatchObject({
      phase: "resolved",
      status: "completed",
      lifeModelInfluence: {
        status: "applied_context_building",
        modelVersion: 3,
        selectedItems: [
          {
            itemRef: "collaboration_preferences:communication-direct",
            statement: "沟通保持简洁直接",
          },
        ],
        permissionGranted: false,
        durableWriteAuthorized: false,
      },
    });
  });

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
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: options.operationId,
          conversation_id: sessionId,
          turn_id: options.operationId,
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: options.operationId,
          conversation_id: sessionId,
          turn_id: options.operationId,
        };
      }
    );
    const dataSource = source({ pickResources, streamTurn });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
      {
        operationId: exactTurnOperationId,
        mode: "work",
        selectedSkillId: undefined,
        taskId: expect.any(String),
        runId: expect.any(String),
      },
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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

  it("directs failed Work to its persisted Results or Needs Attention state", async () => {
    const announce = vi.fn();
    const onAfterTurn = vi.fn().mockResolvedValue(undefined);
    const dataSource = source({
      streamTurn: vi.fn().mockRejectedValue(new Error("read_tool_blocked")),
    });
    const { result } = renderHook(() =>
      useConversationController(dataSource, announce, onAfterTurn)
    );
    await act(async () => result.current.reload());
    act(() => result.current.setMode("work"));
    act(() => result.current.setDraft("查询官网标题"));

    await act(async () => result.current.send());

    expect(announce).toHaveBeenCalledWith(
      "这项工作未完成；请在结果或需处理中核对系统记录的任务状态。"
    );
    expect(onAfterTurn).toHaveBeenCalledTimes(1);
  });

  it("fails closed when an attachment turn returns a foreign terminal identity", async () => {
    const dataSource = source({
      streamTurn: vi.fn().mockResolvedValue({
        ...turnResult("completed"),
        session_id: "conversation-1",
        operation_id: "foreign-operation",
        task_id: "foreign-task",
      }),
    });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: "foreign-task",
          run_id: "foreign-run",
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: options.operationId,
          run_id: options.operationId,
        };
      }
    );
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: options.operationId,
          run_id: options.operationId,
        });
        events.onChunk({
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: "foreign-task",
          run_id: "foreign-run",
          chunk: "不应显示的内容",
        });
        return {
          ...turnResult("completed"),
          session_id: sessionId,
          operation_id: options.operationId,
          task_id: options.operationId,
          run_id: options.operationId,
        };
      }
    );
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
      useConversationController(dataSource, announce, vi.fn().mockResolvedValue(undefined))
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );

    await act(async () => result.current.reload());

    expect(result.current.loadStatus).toBe("ready");
    expect(result.current.selectedSessionId).toBe("conversation-1");
    expect(result.current.messages).toEqual(existingMessages);
    expect(dataSource.loadHistory).toHaveBeenCalledWith("conversation-1");
  });

  it("restores the paused task conversation instead of a newer unrelated session", async () => {
    const loadHistory = vi.fn().mockResolvedValue(existingMessages);
    const dataSource = source({
      listSessions: vi.fn().mockResolvedValue([
        {
          session_id: "conversation-2",
          title: "稍后更新的其他对话",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:02:00Z",
        },
        {
          session_id: "conversation-1",
          title: "等待继续的项目任务",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:01:00Z",
        },
      ]),
      loadHistory,
    });
    const { result } = renderHook(() =>
      useConversationController(
        dataSource,
        vi.fn(),
        vi.fn().mockResolvedValue(undefined),
        "conversation-1"
      )
    );

    await act(async () => result.current.reload());

    expect(result.current.selectedSessionId).toBe("conversation-1");
    expect(loadHistory).toHaveBeenCalledWith("conversation-1");
    expect(loadHistory).not.toHaveBeenCalledWith("conversation-2");
  });

  it("rebinds the paused task conversation when the task read model loads after history", async () => {
    const dataSource = source({
      listSessions: vi.fn().mockResolvedValue([
        {
          session_id: "conversation-2",
          title: "稍后更新的其他对话",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:02:00Z",
        },
        {
          session_id: "conversation-1",
          title: "等待继续的项目任务",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:01:00Z",
        },
      ]),
    });
    const { result, rerender } = renderHook(
      ({ preferredSessionId }: { preferredSessionId: string | null }) =>
        useConversationController(
          dataSource,
          vi.fn(),
          vi.fn().mockResolvedValue(undefined),
          preferredSessionId
        ),
      { initialProps: { preferredSessionId: null as string | null } }
    );

    await act(async () => result.current.reload());
    expect(result.current.selectedSessionId).toBe("conversation-2");

    rerender({ preferredSessionId: "conversation-1" });

    await waitFor(() => expect(result.current.selectedSessionId).toBe("conversation-1"));
  });

  it("does not override an explicit conversation choice with task recovery preference", async () => {
    const dataSource = source({
      listSessions: vi.fn().mockResolvedValue([
        {
          session_id: "conversation-2",
          title: "活动任务对话",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:02:00Z",
        },
        {
          session_id: "conversation-1",
          title: "用户选择的对话",
          created_at: "2026-08-05T00:00:00Z",
          updated_at: "2026-08-05T00:01:00Z",
        },
      ]),
    });
    const { result, rerender } = renderHook(
      ({ preferredSessionId }: { preferredSessionId: string | null }) =>
        useConversationController(
          dataSource,
          vi.fn(),
          vi.fn().mockResolvedValue(undefined),
          preferredSessionId
        ),
      { initialProps: { preferredSessionId: null as string | null } }
    );

    await act(async () => result.current.reload());
    act(() => result.current.selectSession("conversation-1"));
    await waitFor(() => expect(result.current.selectedSessionId).toBe("conversation-1"));

    rerender({ preferredSessionId: "conversation-2" });

    await act(async () => Promise.resolve());
    expect(result.current.selectedSessionId).toBe("conversation-1");
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
    const { result } = renderHook(() => useConversationController(dataSource, vi.fn(), afterTurn));
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
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
    const { result } = renderHook(() => useConversationController(dataSource, vi.fn(), afterTurn));
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
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-1",
          task_id: "task-1",
          run_id: "run-1",
        });
        events.onChunk({
          session_id: "conversation-1",
          operation_id: "operation-1",
          task_id: "task-1",
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("继续"));

    act(() => void result.current.send());

    await waitFor(() => expect(result.current.turnState.phase).toBe("streaming"));
    expect(result.current.streamingReply).toBe("正在整理");
    expect(result.current.activeTaskId).toBe("task-1");

    await act(async () => finishTurn(turnResult("completed")));
    await waitFor(() => expect(result.current.turnState.phase).toBe("resolved"));
  });

  it("binds an in-flight adjustment to the exact session task and run", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        _options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-steer",
          conversation_id: "conversation-1",
          turn_id: "operation-steer",
          task_id: "task-steer",
          run_id: "run-steer",
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const steerTask = vi.fn().mockResolvedValue({
      steering: { steeringId: "steering-1", status: "pending" },
      scopeExpansionBlocked: false,
    });
    const announce = vi.fn();
    const dataSource = source({ streamTurn, steerTask });
    const { result } = renderHook(() =>
      useConversationController(dataSource, announce, vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("生成访谈报告"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskId).toBe("task-steer"));

    act(() => result.current.setDraft("把风险结论放在最前面"));
    await act(async () => result.current.steer());

    expect(steerTask).toHaveBeenCalledWith(
      expect.objectContaining({
        taskId: "task-steer",
        runId: "run-steer",
        sessionId: "conversation-1",
        content: "把风险结论放在最前面",
      })
    );
    expect(result.current.draft).toBe("");
    expect(announce).toHaveBeenLastCalledWith("调整已加入当前任务，将在下一次安全步骤生效。");
    await act(async () => finishTurn(turnResult("completed")));
  });

  it("explains a closed steering window without exposing backend checkpoint terms", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const announce = vi.fn();
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        _options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: "operation-steer-closed",
          conversation_id: "conversation-1",
          turn_id: "operation-steer-closed",
          task_id: "task-steer-closed",
          run_id: "run-steer-closed",
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const steerTask = vi.fn().mockRejectedValue(new Error("canonical_steering_checkpoint_passed"));
    const dataSource = source({ streamTurn, steerTask });
    const { result } = renderHook(() =>
      useConversationController(dataSource, announce, vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("生成访谈报告"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskId).toBe("task-steer-closed"));

    act(() => result.current.setDraft("缩短结论"));
    await act(async () => result.current.steer());

    expect(result.current.draft).toBe("缩短结论");
    expect(announce).toHaveBeenLastCalledWith(
      "这次任务已经进入最终生成阶段，当前调整没有加入；完成后可以继续补充要求。"
    );
    await act(async () => finishTurn(turnResult("completed")));
  });

  it("lets canonical Work continue in the background while another conversation opens", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const announce = vi.fn();
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: options.operationId,
          conversation_id: "conversation-1",
          turn_id: options.operationId,
          task_id: options.taskId,
          run_id: options.runId,
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const loadHistory = vi.fn(async (sessionId: string) =>
      sessionId === "conversation-1"
        ? existingMessages
        : [{ role: "user" as const, content: "另一项工作" }]
    );
    const dataSource = source({
      listSessions: vi.fn().mockResolvedValue([
        {
          session_id: "conversation-1",
          title: "访谈整理",
          created_at: "2026-07-21T00:00:00Z",
          updated_at: "2026-07-21T00:01:00Z",
        },
        {
          session_id: "conversation-2",
          title: "另一项工作",
          created_at: "2026-07-21T00:02:00Z",
          updated_at: "2026-07-21T00:03:00Z",
        },
      ]),
      loadHistory,
      streamTurn,
    });
    const { result } = renderHook(() =>
      useConversationController(dataSource, announce, vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setMode("work"));
    act(() => result.current.setDraft("生成完整报告"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.turnState.phase).toBe("streaming"));

    act(() => result.current.selectSession("conversation-2"));
    await waitFor(() => expect(result.current.selectedSessionId).toBe("conversation-2"));
    expect(result.current.turnState.phase).toBe("idle");
    expect(loadHistory).toHaveBeenCalledWith("conversation-2");
    expect(announce).toHaveBeenCalledWith(
      "任务会在后台继续；已切换到另一段对话，可在需要处理中查看后续状态。"
    );

    await act(async () => finishTurn(turnResult("completed")));
    expect(result.current.selectedSessionId).toBe("conversation-2");
  });

  it("cancels the exact active task and waits for the stream terminal state", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    let emittedTaskId = "";
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        emittedTaskId = options.taskId ?? "";
        events.onStart({
          session_id: "conversation-1",
          operation_id: options.operationId,
          conversation_id: "conversation-1",
          turn_id: options.operationId,
          task_id: emittedTaskId,
          run_id: options.runId,
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const cancelWorkTask = vi.fn().mockResolvedValue({ status: "cancelled" });
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useConversationController(
        dataSource,
        vi.fn(),
        vi.fn().mockResolvedValue(undefined),
        undefined,
        cancelWorkTask
      )
    );
    await act(async () => result.current.reload());
    act(() => result.current.setMode("work"));
    act(() => result.current.setDraft("继续"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskId).toBe(emittedTaskId));

    await act(async () => result.current.cancel());

    expect(cancelWorkTask).toHaveBeenCalledWith(emittedTaskId);
    expect(result.current.turnState.phase).toBe("cancelling");

    await act(async () => finishTurn(turnResult("cancelled")));
    await waitFor(() =>
      expect(result.current.turnState).toMatchObject({ phase: "resolved", status: "cancelled" })
    );
  });

  it("treats the canonical cancellation terminal error as a cancelled turn", async () => {
    let rejectTurn!: (reason: Error) => void;
    let emittedTaskId = "";
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        emittedTaskId = options.taskId ?? "";
        events.onStart({
          session_id: "conversation-1",
          operation_id: options.operationId,
          conversation_id: "conversation-1",
          turn_id: options.operationId,
          task_id: emittedTaskId,
          run_id: options.runId,
        });
        return new Promise<StreamMessageDonePayload>((_resolve, reject) => {
          rejectTurn = reject;
        });
      }
    );
    const cancelWorkTask = vi.fn().mockResolvedValue({ status: "cancelled" });
    const announce = vi.fn();
    const afterTurn = vi.fn().mockResolvedValue(undefined);
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useConversationController(dataSource, announce, afterTurn, undefined, cancelWorkTask)
    );
    await act(async () => result.current.reload());
    act(() => result.current.setMode("work"));
    act(() => result.current.setDraft("执行后取消"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskId).toBe(emittedTaskId));

    await act(async () => result.current.cancel());
    await act(async () => rejectTurn(new Error("canonical_work_cancelled")));

    await waitFor(() =>
      expect(result.current.turnState).toMatchObject({ phase: "resolved", status: "cancelled" })
    );
    expect(announce).toHaveBeenCalledWith("本轮已取消。");
    expect(afterTurn).toHaveBeenCalled();
  });

  it("cancels canonical Chat by exact Conversation and Turn without a Task", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: options.operationId,
          conversation_id: "conversation-1",
          turn_id: options.operationId,
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const cancelChatTurn = vi.fn().mockResolvedValue({ status: "cancelled" });
    const dataSource = source({ streamTurn, cancelChatTurn });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    act(() => result.current.setDraft("停止这一轮"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.turnState.phase).toBe("streaming"));
    const turnId =
      result.current.turnState.phase === "streaming" ? result.current.turnState.turnId : "";

    await act(async () => result.current.cancel());

    expect(cancelChatTurn).toHaveBeenCalledWith("conversation-1", turnId);
    expect(result.current.turnState.phase).toBe("cancelling");
    await act(async () =>
      finishTurn({
        ...turnResult("cancelled"),
        conversation_id: "conversation-1",
        turn_id: turnId,
        operation_id: turnId,
        task_id: undefined,
        run_id: undefined,
      })
    );
  });

  it("does not let a late cancel failure overwrite the stream terminal state", async () => {
    let finishTurn!: (value: StreamMessageDonePayload) => void;
    let rejectCancel!: (reason: Error) => void;
    const streamTurn = vi.fn(
      async (
        _sessionId,
        _messages,
        options,
        events: Parameters<ConversationDataSource["streamTurn"]>[3]
      ) => {
        events.onStart({
          session_id: "conversation-1",
          operation_id: options.operationId,
          conversation_id: "conversation-1",
          turn_id: options.operationId,
          task_id: options.taskId,
          run_id: options.runId,
        });
        return new Promise<StreamMessageDonePayload>(resolve => {
          finishTurn = resolve;
        });
      }
    );
    const cancelWorkTask = vi.fn(
      () =>
        new Promise<void>((_resolve, reject) => {
          rejectCancel = reject;
        })
    );
    const announce = vi.fn();
    const dataSource = source({ streamTurn });
    const { result } = renderHook(() =>
      useConversationController(
        dataSource,
        announce,
        vi.fn().mockResolvedValue(undefined),
        undefined,
        cancelWorkTask
      )
    );
    await act(async () => result.current.reload());
    act(() => result.current.setMode("work"));
    act(() => result.current.setDraft("继续"));
    act(() => void result.current.send());
    await waitFor(() => expect(result.current.activeTaskId).not.toBeNull());

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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    await act(async () =>
      expect(await result.current.renameSelected("  新的   名称  ")).toBe(true)
    );

    expect(dataSource.renameSession).toHaveBeenCalledWith("conversation-1", "新的 名称");
    expect(listSessions).toHaveBeenCalledTimes(2);
    expect(result.current.sessionMutation.phase).toBe("idle");
  });

  it("creates and assigns a Project only after the canonical view confirms it", async () => {
    const project = {
      id: "project-1",
      name: "访谈研究",
      revision: 1,
      createdAt: "2026-08-14T00:00:00Z",
      updatedAt: "2026-08-14T00:00:00Z",
    };
    const canonical = (assigned: boolean) => ({
      status: "ready" as const,
      conversations: [
        {
          session_id: "conversation-1",
          title: "访谈整理",
          created_at: "2026-07-21T00:00:00Z",
          updated_at: "2026-07-21T00:01:00Z",
        },
      ],
      projects: assigned ? [project] : [],
      selectedProjectId: assigned ? project.id : null,
      selectedConversationId: "conversation-1",
      globalMemoryEnabled: true,
      selectedMemoryMode: "use_and_learn" as const,
      messages: existingMessages,
      latestTurn: null,
      providerStatus: "ready" as const,
      providerProfiles: [],
      selectedProviderProfileId: null,
      providerErrorCode: null,
      workStatus: "available" as const,
    });
    const loadConversation = vi
      .fn()
      .mockResolvedValueOnce(canonical(false))
      .mockResolvedValueOnce(canonical(true));
    const createProject = vi.fn().mockResolvedValue(project);
    const assignProject = vi.fn().mockResolvedValue(undefined);
    const dataSource = source({ loadConversation, createProject, assignProject });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    await act(async () => expect(await result.current.createProject("  访谈   研究 ")).toBe(true));

    expect(createProject).toHaveBeenCalledWith(expect.any(String), "访谈 研究");
    expect(assignProject).toHaveBeenCalledWith("conversation-1", "project-1");
    expect(result.current.projects).toEqual([project]);
    expect(result.current.selectedProjectId).toBe("project-1");
  });

  it("does not rename or delete a conversation while a resource is bound to the pending turn", async () => {
    const dataSource = source();
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await act(async () => result.current.attachResources());

    await act(async () => expect(await result.current.renameSelected("新名称")).toBe(false));
    await act(async () => expect(await result.current.deleteSelected()).toBe(false));

    expect(dataSource.renameSession).not.toHaveBeenCalled();
    expect(dataSource.deleteSession).not.toHaveBeenCalled();
  });

  it("binds a backend-confirmed skill to the exact session and next turn", async () => {
    const selectSkill = vi.fn().mockResolvedValue({
      sessionId: "conversation-1",
      selectedSkillId: "research",
      selectedSkillDigest: "sha256:skill",
      selectionReason: "user_selected_local_skill",
      boundedInstructionsPreview: "Research with evidence.",
      evidenceDigest: "sha256:evidence",
      policyNotes: [],
      includedAsBoundedContextOnly: true,
      unselectedSkillsInjected: false,
      controls: ["clear_skill"],
    });
    const streamTurn = vi.fn().mockResolvedValue(turnResult("completed"));
    const dataSource = source({
      listSkills: vi.fn().mockResolvedValue([
        {
          skillId: "research",
          name: "Research",
          source: "bundled:research",
          scope: "session",
          description: "Evidence-backed research",
          riskLevel: "low",
          available: true,
          selected: false,
          instructionDigest: "sha256:skill",
          sourceKind: "bundled",
        },
      ]),
      listToolCandidates: vi.fn().mockResolvedValue({
        candidates: [],
        blockedTools: [],
        evidenceDigest: "sha256:tools",
        controls: [],
      }),
      selectSkill,
      clearSkill: vi.fn(),
      streamTurn,
    });
    const { result } = renderHook(() =>
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());
    await waitFor(() => expect(result.current.capabilityState.phase).toBe("ready"));

    await act(async () => expect(await result.current.selectSkill("research")).toBe(true));
    act(() => result.current.setDraft("请研究这个问题"));
    await act(async () => result.current.send());

    expect(selectSkill).toHaveBeenCalledWith("conversation-1", "research");
    expect(streamTurn).toHaveBeenCalledWith(
      "conversation-1",
      expect.any(Array),
      expect.objectContaining({ selectedSkillId: "research" }),
      expect.any(Object)
    );
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
      useConversationController(dataSource, vi.fn(), vi.fn().mockResolvedValue(undefined))
    );
    await act(async () => result.current.reload());

    expect(dataSource.deleteSession).not.toHaveBeenCalled();
    await act(async () => expect(await result.current.deleteSelected()).toBe(true));

    expect(dataSource.deleteSession).toHaveBeenCalledWith("conversation-1");
    expect(result.current.selectedSessionId).toBeNull();
    expect(result.current.messages).toEqual([]);
  });
});
