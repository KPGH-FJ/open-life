import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ChatSession,
  ImportedResourceReceipt,
  MainChatTurnStatus,
  ProductAction,
  StreamMessageChunkPayload,
  StreamMessageStartPayload,
} from "@/tauri";
import type { ChatMessage } from "@/types";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import type { WorkspaceConversationDataSource } from "./workspaceConversationDataSource";

type Announce = (message: string) => void;

export type WorkspaceConversationLoadStatus = "idle" | "loading" | "ready" | "error";
export type WorkspaceSessionMutationState =
  | { phase: "idle" }
  | { phase: "renaming"; sessionId: string }
  | { phase: "deleting"; sessionId: string }
  | { phase: "failed"; action: "rename" | "delete"; reason: string };

export type WorkspaceResourceMutationState =
  | { phase: "idle" }
  | { phase: "importing"; importOperationId: string; turnOperationId: string }
  | { phase: "detaching"; resourceId: string; turnOperationId: string }
  | { phase: "failed"; action: "import" | "detach"; reason: string };

export type WorkspaceTurnState =
  | { phase: "idle" }
  | { phase: "creating_session" }
  | { phase: "sending"; sessionId: string }
  | { phase: "streaming"; sessionId: string; taskSessionId: string; cancelError?: string }
  | { phase: "cancelling"; sessionId: string; taskSessionId: string }
  | { phase: "refreshing"; sessionId: string; status: MainChatTurnStatus }
  | {
      phase: "resolved";
      sessionId: string;
      status: MainChatTurnStatus;
      blockers: string[];
    }
  | { phase: "failed"; stage: "create" | "send" | "refresh"; reason: string };

function sessionTitle(input: string): string {
  const normalized = input.replace(/\s+/g, " ").trim();
  return normalized.length > 28 ? `${normalized.slice(0, 28)}...` : normalized;
}

function resolvedTurnStatus(status: MainChatTurnStatus | undefined): MainChatTurnStatus {
  return status ?? "failed";
}

function turnAnnouncement(status: MainChatTurnStatus): string {
  switch (status) {
    case "completed":
      return "回复已返回；工作区正在核对后端任务状态。";
    case "completed_with_pending_items":
      return "回复已返回，但存在待决定事项；它们不会被解释成已完成。";
    case "blocked":
      return "本轮被后端阻断；没有把阻断状态解释成完成。";
    case "remote_unknown":
      return "远端结果未知；当前不会重试或显示成功。";
    case "cancelled":
      return "本轮已取消。";
    case "interrupted":
      return "本轮已中断；当前结果不完整。";
    case "failed":
      return "本轮失败；当前不会显示成功。";
  }
}

export type WorkspaceConversationController = {
  sessions: ChatSession[];
  selectedSessionId: string | null;
  messages: ChatMessage[];
  draft: string;
  loadStatus: WorkspaceConversationLoadStatus;
  loadError: string | null;
  turnState: WorkspaceTurnState;
  streamingReply: string;
  activeTaskSessionId: string | null;
  sessionMutation: WorkspaceSessionMutationState;
  pendingResources: ImportedResourceReceipt[];
  pendingResourceTurnOperationId: string | null;
  resourceMutation: WorkspaceResourceMutationState;
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<boolean>;
  selectSession: (sessionId: string) => void;
  startNewConversation: () => void;
  setDraft: (value: string) => void;
  attachResources: () => Promise<boolean>;
  detachResource: (resourceId: string) => Promise<boolean>;
  sendAction: (disabledReason?: string) => ProductAction;
  send: (disabledReason?: string) => Promise<void>;
  cancel: () => Promise<void>;
  renameSelected: (title: string) => Promise<boolean>;
  deleteSelected: () => Promise<boolean>;
};

export function useWorkspaceConversation(
  dataSource: WorkspaceConversationDataSource | undefined,
  announce: Announce,
  onAfterTurn: () => Promise<void>
): WorkspaceConversationController {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [loadStatus, setLoadStatus] = useState<WorkspaceConversationLoadStatus>("idle");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [turnState, setTurnState] = useState<WorkspaceTurnState>({ phase: "idle" });
  const [streamingReply, setStreamingReply] = useState("");
  const [activeTaskSessionId, setActiveTaskSessionId] = useState<string | null>(null);
  const [sessionMutation, setSessionMutation] = useState<WorkspaceSessionMutationState>({
    phase: "idle",
  });
  const [pendingResources, setPendingResources] = useState<ImportedResourceReceipt[]>([]);
  const [pendingResourceTurnOperationId, setPendingResourceTurnOperationId] = useState<
    string | null
  >(null);
  const [resourceMutation, setResourceMutation] = useState<WorkspaceResourceMutationState>({
    phase: "idle",
  });
  const requestRef = useRef(0);
  const operationRef = useRef(0);
  const cancelRequestRef = useRef(0);
  const loadedRef = useRef(false);

  useEffect(() => {
    requestRef.current += 1;
    operationRef.current += 1;
    cancelRequestRef.current += 1;
    loadedRef.current = false;
    setSessions([]);
    setSelectedSessionId(null);
    setMessages([]);
    setDraft("");
    setLoadStatus("idle");
    setLoadError(null);
    setTurnState({ phase: "idle" });
    setStreamingReply("");
    setActiveTaskSessionId(null);
    setSessionMutation({ phase: "idle" });
    setPendingResources([]);
    setPendingResourceTurnOperationId(null);
    setResourceMutation({ phase: "idle" });
    return () => {
      requestRef.current += 1;
      operationRef.current += 1;
      cancelRequestRef.current += 1;
      loadedRef.current = false;
    };
  }, [dataSource]);

  const loadHistory = useCallback(
    async (sessionId: string, requestId: number): Promise<void> => {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const history = await dataSource.loadHistory(sessionId);
      if (requestId !== requestRef.current) return;
      setSelectedSessionId(sessionId);
      setMessages(history);
    },
    [dataSource]
  );

  const reload = useCallback(async (): Promise<boolean> => {
    const requestId = ++requestRef.current;
    setLoadStatus("loading");
    setLoadError(null);
    try {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const nextSessions = await dataSource.listSessions();
      if (requestId !== requestRef.current) return false;
      setSessions(nextSessions);
      const currentStillExists =
        selectedSessionId && nextSessions.some(item => item.session_id === selectedSessionId);
      const nextSessionId = currentStillExists
        ? selectedSessionId
        : (nextSessions[0]?.session_id ?? null);
      if (nextSessionId) {
        await loadHistory(nextSessionId, requestId);
      } else {
        setSelectedSessionId(null);
        setMessages([]);
      }
      if (requestId !== requestRef.current) return false;
      loadedRef.current = true;
      setLoadStatus("ready");
      return true;
    } catch (error) {
      if (requestId !== requestRef.current) return false;
      loadedRef.current = false;
      setLoadStatus("error");
      setLoadError(errorText(error));
      setMessages([]);
      return false;
    }
  }, [dataSource, loadHistory, selectedSessionId]);

  const ensureLoaded = useCallback(() => {
    if (loadedRef.current || loadStatus === "loading") return;
    void reload();
  }, [loadStatus, reload]);

  const busy =
    ["creating_session", "sending", "streaming", "cancelling", "refreshing"].includes(
      turnState.phase
    ) ||
    ["renaming", "deleting"].includes(sessionMutation.phase) ||
    ["importing", "detaching"].includes(resourceMutation.phase);

  const selectSession = useCallback(
    (sessionId: string) => {
      if (!sessionId || sessionId === selectedSessionId || busy) return;
      if (pendingResources.length > 0) {
        announce("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
        return;
      }
      const requestId = ++requestRef.current;
      setLoadStatus("loading");
      setLoadError(null);
      setTurnState({ phase: "idle" });
      setStreamingReply("");
      setActiveTaskSessionId(null);
      void loadHistory(sessionId, requestId)
        .then(() => {
          if (requestId === requestRef.current) setLoadStatus("ready");
        })
        .catch(error => {
          if (requestId !== requestRef.current) return;
          setLoadStatus("error");
          setLoadError(errorText(error));
          setMessages([]);
        });
    },
    [announce, busy, loadHistory, pendingResources.length, selectedSessionId]
  );

  const startNewConversation = useCallback(() => {
    if (busy) return;
    if (pendingResources.length > 0) {
      announce("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
      return;
    }
    requestRef.current += 1;
    setSelectedSessionId(null);
    setMessages([]);
    setDraft("");
    setLoadError(null);
    setLoadStatus("ready");
    setTurnState({ phase: "idle" });
    setStreamingReply("");
    setActiveTaskSessionId(null);
    announce("已打开新对话草稿；发送前不会创建会话或写入记录。");
  }, [announce, busy, pendingResources.length]);

  const attachResources = useCallback(async (): Promise<boolean> => {
    if (!dataSource || loadStatus !== "ready" || busy) {
      announce("当前不能添加文件；请等待会话和正在进行的操作完成。");
      return false;
    }
    if (pendingResources.length >= 5) {
      announce("本轮最多添加 5 个文件。");
      return false;
    }

    const turnOperationId = pendingResourceTurnOperationId ?? crypto.randomUUID();
    const importOperationId = crypto.randomUUID();
    setPendingResourceTurnOperationId(turnOperationId);
    setResourceMutation({ phase: "importing", importOperationId, turnOperationId });
    try {
      const result = await dataSource.pickResources(importOperationId, turnOperationId);
      if (result.cancelled) {
        if (pendingResources.length === 0) setPendingResourceTurnOperationId(null);
        setResourceMutation({ phase: "idle" });
        announce("没有选择文件；当前回合没有新增资源。");
        return false;
      }
      const receipt = result.receipt;
      if (
        !receipt ||
        receipt.operationId !== importOperationId ||
        receipt.messageId !== turnOperationId ||
        receipt.resources.length === 0
      ) {
        throw new Error("resource_import_identity_mismatch");
      }
      const ids = new Set(pendingResources.map(resource => resource.resourceId));
      if (receipt.resources.some(resource => ids.has(resource.resourceId))) {
        throw new Error("resource_import_duplicate_receipt");
      }
      setPendingResources(current => [...current, ...receipt.resources]);
      setResourceMutation({ phase: "idle" });
      announce(`已添加 ${receipt.resources.length} 个文件；它们只绑定到下一次发送。`);
      return true;
    } catch (error) {
      if (pendingResources.length === 0) setPendingResourceTurnOperationId(null);
      setResourceMutation({ phase: "failed", action: "import", reason: errorText(error) });
      announce("文件没有完成导入；当前不会把它显示为已添加。");
      return false;
    }
  }, [announce, busy, dataSource, loadStatus, pendingResourceTurnOperationId, pendingResources]);

  const detachResource = useCallback(
    async (resourceId: string): Promise<boolean> => {
      const resource = pendingResources.find(item => item.resourceId === resourceId);
      if (!dataSource || !resource || !pendingResourceTurnOperationId || busy) {
        announce("当前不能移除这个文件。");
        return false;
      }
      const operationId = crypto.randomUUID();
      const turnOperationId = pendingResourceTurnOperationId;
      setResourceMutation({ phase: "detaching", resourceId, turnOperationId });
      try {
        const receipt = await dataSource.detachResource(operationId, turnOperationId, resourceId);
        if (
          receipt.operationId !== operationId ||
          receipt.messageId !== turnOperationId ||
          receipt.resourceId !== resourceId ||
          !receipt.bindingRemoved
        ) {
          throw new Error("resource_detach_identity_mismatch");
        }
        const remaining = pendingResources.filter(item => item.resourceId !== resourceId);
        setPendingResources(remaining);
        if (remaining.length === 0) setPendingResourceTurnOperationId(null);
        setResourceMutation({ phase: "idle" });
        announce(`已移除 ${resource.filename}；下一次发送不会包含它。`);
        return true;
      } catch (error) {
        setResourceMutation({ phase: "failed", action: "detach", reason: errorText(error) });
        announce("文件移除没有得到后端确认；当前仍按已绑定处理。");
        return false;
      }
    },
    [announce, busy, dataSource, pendingResourceTurnOperationId, pendingResources]
  );

  const sendAction = useCallback(
    (disabledReason?: string): ProductAction => {
      const trimmedDraft = draft.trim();
      const reason =
        disabledReason?.trim() ||
        (!dataSource
          ? "工作区会话数据源不可用。"
          : loadStatus !== "ready"
            ? "会话记录尚未完成读取。"
            : busy
              ? "上一轮仍在发送或核对。"
              : !trimmedDraft
                ? "先输入要发送的内容。"
                : undefined);
      return {
        id: selectedSessionId
          ? `workspace.continue:${selectedSessionId}`
          : "workspace.start:new-conversation",
        label: selectedSessionId ? "发送" : "开始并发送",
        kind: selectedSessionId ? "continue" : "start",
        enabled: !reason,
        ...(reason ? { disabledReason: reason } : {}),
        targetRef: selectedSessionId ?? "new-conversation",
      };
    },
    [busy, dataSource, draft, loadStatus, selectedSessionId]
  );

  const send = useCallback(
    async (disabledReason?: string): Promise<void> => {
      const action = sendAction(disabledReason);
      if (!action.enabled || !dataSource) {
        announce(`当前不能发送：${action.disabledReason ?? "动作不可用"}`);
        return;
      }
      const text = draft.trim();
      const operationId = ++operationRef.current;
      const turnOperationId =
        pendingResources.length > 0 ? pendingResourceTurnOperationId : crypto.randomUUID();
      if (!turnOperationId) {
        announce("文件已经显示为待发送，但缺少精确回合标识；当前不会发送。");
        return;
      }
      cancelRequestRef.current += 1;
      let sessionId = selectedSessionId;
      let sessionCreated = false;
      let streamStarted = false;
      let resourceStreamIdentityMismatch = false;
      try {
        if (!sessionId) {
          setTurnState({ phase: "creating_session" });
          sessionId = crypto.randomUUID();
          await dataSource.createSession(sessionId, sessionTitle(text));
          sessionCreated = true;
          if (operationId !== operationRef.current) return;
          const timestamp = new Date().toISOString();
          setSessions(current => [
            {
              session_id: sessionId!,
              title: sessionTitle(text),
              created_at: timestamp,
              updated_at: timestamp,
            },
            ...current,
          ]);
          setSelectedSessionId(sessionId);
        }

        const userMessage: ChatMessage = { role: "user", content: text };
        const requestMessages = [...messages, userMessage];
        const turnSessionId = sessionId;
        setMessages(requestMessages);
        setDraft("");
        setTurnState({ phase: "sending", sessionId });
        setStreamingReply("");
        setActiveTaskSessionId(null);
        announce("正在发送；命令返回后仍会重新读取会话和工作区状态。");

        const result = await dataSource.streamTurn(
          turnSessionId,
          requestMessages,
          { operationId: turnOperationId },
          {
            onStart: (payload: StreamMessageStartPayload) => {
              if (operationId !== operationRef.current) return;
              if (
                pendingResources.length > 0 &&
                (payload.session_id !== turnSessionId ||
                  payload.operation_id !== turnOperationId ||
                  payload.task_session_id !== turnOperationId)
              ) {
                resourceStreamIdentityMismatch = true;
                return;
              }
              streamStarted = true;
              if (pendingResources.length > 0) {
                setPendingResources([]);
                setPendingResourceTurnOperationId(null);
                setResourceMutation({ phase: "idle" });
              }
              setActiveTaskSessionId(payload.task_session_id);
              setTurnState({
                phase: "streaming",
                sessionId: turnSessionId,
                taskSessionId: payload.task_session_id,
              });
              announce("OpenLife 正在回复；现在可以取消这一项任务。");
            },
            onChunk: (payload: StreamMessageChunkPayload) => {
              if (operationId !== operationRef.current) return;
              if (
                pendingResources.length > 0 &&
                (payload.session_id !== turnSessionId ||
                  payload.operation_id !== turnOperationId ||
                  payload.task_session_id !== turnOperationId)
              ) {
                resourceStreamIdentityMismatch = true;
                return;
              }
              setStreamingReply(current => current + payload.chunk);
            },
          }
        );
        if (operationId !== operationRef.current) return;
        if (
          pendingResources.length > 0 &&
          (resourceStreamIdentityMismatch ||
            result.session_id !== turnSessionId ||
            result.operation_id !== turnOperationId ||
            result.task_session_id !== turnOperationId)
        ) {
          throw new Error("resource_turn_terminal_identity_mismatch");
        }
        if (pendingResources.length > 0) {
          setPendingResources([]);
          setPendingResourceTurnOperationId(null);
          setResourceMutation({ phase: "idle" });
        }
        cancelRequestRef.current += 1;
        const status = resolvedTurnStatus(result.status ?? result.turn_terminal?.status);
        setActiveTaskSessionId(result.task_session_id);
        setTurnState({ phase: "refreshing", sessionId, status });

        let refreshedHistory: ChatMessage[];
        try {
          refreshedHistory = await dataSource.loadHistory(sessionId);
        } catch (error) {
          if (operationId !== operationRef.current) return;
          setTurnState({ phase: "failed", stage: "refresh", reason: errorText(error) });
          announce("命令已返回，但会话记录刷新失败；当前不确认回复已持久化。");
          await onAfterTurn();
          return;
        }
        if (operationId !== operationRef.current) return;
        setMessages(refreshedHistory);
        setStreamingReply("");
        setActiveTaskSessionId(null);
        setTurnState({
          phase: "resolved",
          sessionId,
          status,
          blockers: result.blockers ?? result.turn_terminal?.blockers ?? [],
        });
        await onAfterTurn();
        announce(turnAnnouncement(status));
      } catch (error) {
        if (operationId !== operationRef.current) return;
        cancelRequestRef.current += 1;
        if (!streamStarted) setDraft(text);
        setTurnState({
          phase: "failed",
          stage: sessionCreated || selectedSessionId ? "send" : "create",
          reason: errorText(error),
        });
        announce(
          sessionCreated || selectedSessionId
            ? "消息发送失败；当前不会显示成功结论。"
            : "新会话未能建立；没有发送消息。"
        );
        try {
          if (sessionId) {
            const refreshedHistory = await dataSource.loadHistory(sessionId);
            if (operationId === operationRef.current) setMessages(refreshedHistory);
          }
        } catch {
          // The explicit failed state already communicates that persistence is unverified.
        }
      }
    },
    [
      announce,
      dataSource,
      draft,
      messages,
      onAfterTurn,
      pendingResourceTurnOperationId,
      pendingResources,
      selectedSessionId,
      sendAction,
    ]
  );

  const cancel = useCallback(async (): Promise<void> => {
    if (!dataSource || turnState.phase !== "streaming" || !turnState.taskSessionId.trim()) {
      announce("当前没有可以取消的运行中任务。");
      return;
    }
    const { sessionId, taskSessionId } = turnState;
    const cancelRequestId = ++cancelRequestRef.current;
    setTurnState({ phase: "cancelling", sessionId, taskSessionId });
    announce("正在请求取消；只有后端终态返回后才会显示已取消。");
    try {
      await dataSource.cancelTask(taskSessionId);
    } catch (error) {
      if (cancelRequestId !== cancelRequestRef.current) return;
      setTurnState({
        phase: "streaming",
        sessionId,
        taskSessionId,
        cancelError: errorText(error),
      });
      announce("取消请求失败；当前不会把任务显示为已取消。");
    }
  }, [announce, dataSource, turnState]);

  const renameSelected = useCallback(
    async (title: string): Promise<boolean> => {
      const normalizedTitle = title.replace(/\s+/g, " ").trim();
      if (
        !dataSource ||
        !selectedSessionId ||
        busy ||
        pendingResources.length > 0 ||
        !normalizedTitle
      ) {
        announce("当前不能重命名这段对话。");
        return false;
      }
      setSessionMutation({ phase: "renaming", sessionId: selectedSessionId });
      try {
        await dataSource.renameSession(selectedSessionId, normalizedTitle);
        if (!(await reload())) {
          throw new Error("conversation_refresh_failed_after_rename");
        }
        setSessionMutation({ phase: "idle" });
        announce("对话名称已从后端重新读取并确认。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "rename", reason: errorText(error) });
        announce("对话重命名失败；当前名称未被解释为已保存。");
        return false;
      }
    },
    [announce, busy, dataSource, pendingResources.length, reload, selectedSessionId]
  );

  const deleteSelected = useCallback(async (): Promise<boolean> => {
    if (!dataSource || !selectedSessionId || busy || pendingResources.length > 0) {
      announce("当前不能删除这段对话。");
      return false;
    }
    const targetSessionId = selectedSessionId;
    setSessionMutation({ phase: "deleting", sessionId: targetSessionId });
    try {
      await dataSource.deleteSession(targetSessionId);
      if (!(await reload())) {
        throw new Error("conversation_refresh_failed_after_delete");
      }
      setSessionMutation({ phase: "idle" });
      announce("对话删除已由后端重新读取确认。");
      return true;
    } catch (error) {
      setSessionMutation({ phase: "failed", action: "delete", reason: errorText(error) });
      announce("对话删除失败；当前仍按未删除处理。");
      return false;
    }
  }, [announce, busy, dataSource, pendingResources.length, reload, selectedSessionId]);

  return useMemo(
    () => ({
      sessions,
      selectedSessionId,
      messages,
      draft,
      loadStatus,
      loadError,
      turnState,
      streamingReply,
      activeTaskSessionId,
      sessionMutation,
      pendingResources,
      pendingResourceTurnOperationId,
      resourceMutation,
      busy,
      ensureLoaded,
      reload,
      selectSession,
      startNewConversation,
      setDraft,
      attachResources,
      detachResource,
      sendAction,
      send,
      cancel,
      renameSelected,
      deleteSelected,
    }),
    [
      busy,
      cancel,
      deleteSelected,
      detachResource,
      draft,
      ensureLoaded,
      attachResources,
      loadError,
      loadStatus,
      messages,
      pendingResources,
      pendingResourceTurnOperationId,
      renameSelected,
      reload,
      resourceMutation,
      selectSession,
      selectedSessionId,
      sessionMutation,
      send,
      sendAction,
      sessions,
      startNewConversation,
      streamingReply,
      activeTaskSessionId,
      turnState,
    ]
  );
}
