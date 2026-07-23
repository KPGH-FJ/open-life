import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChatSession, MainChatTurnStatus, ProductAction } from "@/tauri";
import type { ChatMessage } from "@/types";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import type { WorkspaceConversationDataSource } from "./workspaceConversationDataSource";

type Announce = (message: string) => void;

export type WorkspaceConversationLoadStatus = "idle" | "loading" | "ready" | "error";

export type WorkspaceTurnState =
  | { phase: "idle" }
  | { phase: "creating_session" }
  | { phase: "sending"; sessionId: string }
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
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<void>;
  selectSession: (sessionId: string) => void;
  startNewConversation: () => void;
  setDraft: (value: string) => void;
  sendAction: (disabledReason?: string) => ProductAction;
  send: (disabledReason?: string) => Promise<void>;
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
  const requestRef = useRef(0);
  const operationRef = useRef(0);
  const loadedRef = useRef(false);

  useEffect(() => {
    requestRef.current += 1;
    operationRef.current += 1;
    loadedRef.current = false;
    setSessions([]);
    setSelectedSessionId(null);
    setMessages([]);
    setDraft("");
    setLoadStatus("idle");
    setLoadError(null);
    setTurnState({ phase: "idle" });
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

  const reload = useCallback(async (): Promise<void> => {
    const requestId = ++requestRef.current;
    setLoadStatus("loading");
    setLoadError(null);
    try {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const nextSessions = await dataSource.listSessions();
      if (requestId !== requestRef.current) return;
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
      if (requestId !== requestRef.current) return;
      loadedRef.current = true;
      setLoadStatus("ready");
    } catch (error) {
      if (requestId !== requestRef.current) return;
      loadedRef.current = false;
      setLoadStatus("error");
      setLoadError(errorText(error));
      setMessages([]);
    }
  }, [dataSource, loadHistory, selectedSessionId]);

  const ensureLoaded = useCallback(() => {
    if (loadedRef.current || loadStatus === "loading") return;
    void reload();
  }, [loadStatus, reload]);

  const selectSession = useCallback(
    (sessionId: string) => {
      if (!sessionId || sessionId === selectedSessionId || turnState.phase === "sending") return;
      const requestId = ++requestRef.current;
      setLoadStatus("loading");
      setLoadError(null);
      setTurnState({ phase: "idle" });
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
    [loadHistory, selectedSessionId, turnState.phase]
  );

  const startNewConversation = useCallback(() => {
    if (["creating_session", "sending", "refreshing"].includes(turnState.phase)) return;
    requestRef.current += 1;
    setSelectedSessionId(null);
    setMessages([]);
    setDraft("");
    setLoadError(null);
    setLoadStatus("ready");
    setTurnState({ phase: "idle" });
    announce("已打开新对话草稿；发送前不会创建会话或写入记录。");
  }, [announce, turnState.phase]);

  const busy = ["creating_session", "sending", "refreshing"].includes(turnState.phase);

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
      let sessionId = selectedSessionId;
      let sessionCreated = false;
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
        setMessages(requestMessages);
        setDraft("");
        setTurnState({ phase: "sending", sessionId });
        announce("正在发送；命令返回后仍会重新读取会话和工作区状态。");

        const result = await dataSource.sendTurn(sessionId, requestMessages, {
          operationId: crypto.randomUUID(),
        });
        if (operationId !== operationRef.current) return;
        const status = resolvedTurnStatus(result.status ?? result.turn_terminal?.status);
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
    [announce, dataSource, draft, messages, onAfterTurn, selectedSessionId, sendAction]
  );

  return useMemo(
    () => ({
      sessions,
      selectedSessionId,
      messages,
      draft,
      loadStatus,
      loadError,
      turnState,
      busy,
      ensureLoaded,
      reload,
      selectSession,
      startNewConversation,
      setDraft,
      sendAction,
      send,
    }),
    [
      busy,
      draft,
      ensureLoaded,
      loadError,
      loadStatus,
      messages,
      reload,
      selectSession,
      selectedSessionId,
      send,
      sendAction,
      sessions,
      startNewConversation,
      turnState,
    ]
  );
}
