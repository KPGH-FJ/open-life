import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ChatSession,
  ImportedResourceReceipt,
  MainChatSkillSummary,
  MainChatToolCandidateList,
  MainChatTurnStatus,
  MarkdownMemoryProposalReceipt,
  MarkdownMemoryScope,
  MarkdownMemoryViewModel,
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

export type WorkspaceCapabilityState =
  | { phase: "idle" }
  | { phase: "loading" }
  | { phase: "ready" }
  | { phase: "selecting"; skillId: string | null }
  | { phase: "failed"; reason: string };

export type WorkspaceMarkdownMemoryState =
  | { phase: "loading"; model: MarkdownMemoryViewModel | null }
  | { phase: "ready"; model: MarkdownMemoryViewModel; lastProposal?: MarkdownMemoryProposalReceipt }
  | { phase: "selecting_root"; model: MarkdownMemoryViewModel | null; scope: MarkdownMemoryScope }
  | { phase: "submitting"; model: MarkdownMemoryViewModel; operation: "write" | "deactivate" }
  | { phase: "failed"; model: MarkdownMemoryViewModel | null; reason: string };

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
  skills: MainChatSkillSummary[];
  selectedSkillId: string | null;
  toolCandidates: MainChatToolCandidateList | null;
  capabilityState: WorkspaceCapabilityState;
  markdownMemory: WorkspaceMarkdownMemoryState;
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<boolean>;
  selectSession: (sessionId: string) => void;
  startNewConversation: () => void;
  setDraft: (value: string) => void;
  attachResources: () => Promise<boolean>;
  detachResource: (resourceId: string) => Promise<boolean>;
  selectSkill: (skillId: string | null) => Promise<boolean>;
  reloadMarkdownMemory: () => Promise<boolean>;
  selectMarkdownMemoryRoot: (scope: MarkdownMemoryScope) => Promise<boolean>;
  proposeMarkdownMemoryWrite: (request: {
    scope: MarkdownMemoryScope;
    relativePath: string;
    content: string;
    expectedCurrentDigest?: string;
  }) => Promise<boolean>;
  proposeMarkdownMemoryDeactivation: (request: {
    scope: MarkdownMemoryScope;
    relativePath: string;
    expectedCurrentDigest: string;
  }) => Promise<boolean>;
  sendAction: (disabledReason?: string) => ProductAction;
  send: (disabledReason?: string) => Promise<void>;
  cancel: () => Promise<void>;
  renameSelected: (title: string) => Promise<boolean>;
  deleteSelected: () => Promise<boolean>;
};

export function useWorkspaceConversation(
  dataSource: WorkspaceConversationDataSource | undefined,
  announce: Announce,
  onAfterTurn: () => Promise<void>,
  preferredSessionId?: string | null
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
  const [skills, setSkills] = useState<MainChatSkillSummary[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [toolCandidates, setToolCandidates] = useState<MainChatToolCandidateList | null>(null);
  const [capabilityState, setCapabilityState] = useState<WorkspaceCapabilityState>({
    phase: "idle",
  });
  const [markdownMemory, setMarkdownMemory] = useState<WorkspaceMarkdownMemoryState>({
    phase: "loading",
    model: null,
  });
  const requestRef = useRef(0);
  const operationRef = useRef(0);
  const cancelRequestRef = useRef(0);
  const markdownMemoryRequestRef = useRef(0);
  const loadedRef = useRef(false);
  const explicitConversationChoiceRef = useRef(false);

  useEffect(() => {
    requestRef.current += 1;
    operationRef.current += 1;
    cancelRequestRef.current += 1;
    markdownMemoryRequestRef.current += 1;
    loadedRef.current = false;
    explicitConversationChoiceRef.current = false;
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
    setSkills([]);
    setSelectedSkillId(null);
    setToolCandidates(null);
    setCapabilityState({ phase: "idle" });
    setMarkdownMemory({ phase: "loading", model: null });
    return () => {
      requestRef.current += 1;
      operationRef.current += 1;
      cancelRequestRef.current += 1;
      markdownMemoryRequestRef.current += 1;
      loadedRef.current = false;
    };
  }, [dataSource]);

  const reloadMarkdownMemory = useCallback(async (): Promise<boolean> => {
    const requestId = ++markdownMemoryRequestRef.current;
    if (!dataSource?.loadMarkdownMemory) {
      setMarkdownMemory(current => ({
        phase: "failed",
        model: current.model,
        reason: "markdown_memory_data_source_unavailable",
      }));
      return false;
    }
    setMarkdownMemory(current => ({ phase: "loading", model: current.model }));
    try {
      const model = await dataSource.loadMarkdownMemory();
      if (requestId !== markdownMemoryRequestRef.current) return false;
      setMarkdownMemory({ phase: "ready", model });
      return true;
    } catch (error) {
      if (requestId !== markdownMemoryRequestRef.current) return false;
      setMarkdownMemory(current => ({
        phase: "failed",
        model: current.model,
        reason: errorText(error),
      }));
      return false;
    }
  }, [dataSource]);

  useEffect(() => {
    if (!dataSource?.loadMarkdownMemory) return;
    void reloadMarkdownMemory();
  }, [dataSource, reloadMarkdownMemory]);

  const selectMarkdownMemoryRoot = useCallback(
    async (scope: MarkdownMemoryScope): Promise<boolean> => {
      if (!dataSource?.selectMarkdownMemoryRoot || markdownMemory.phase === "selecting_root") {
        announce("当前不能选择 Markdown Memory 文件夹。");
        return false;
      }
      const currentModel = markdownMemory.model;
      const requestId = ++markdownMemoryRequestRef.current;
      setMarkdownMemory({ phase: "selecting_root", model: currentModel, scope });
      try {
        const selection = await dataSource.selectMarkdownMemoryRoot(scope);
        if (requestId !== markdownMemoryRequestRef.current) return false;
        if (selection.cancelled) {
          if (currentModel) setMarkdownMemory({ phase: "ready", model: currentModel });
          else setMarkdownMemory({ phase: "loading", model: null });
          announce("没有选择文件夹；当前 Markdown Memory 作用域未改变。");
          return false;
        }
        if (selection.scope !== scope || !selection.selectedPath) {
          throw new Error("markdown_memory_root_selection_identity_mismatch");
        }
        const reloaded = await reloadMarkdownMemory();
        announce(
          reloaded
            ? `${scope === "workspace" ? "Workspace" : "Project"} Memory 文件夹已重新读取。`
            : "文件夹选择已返回，但 Memory 状态没有得到后端确认。"
        );
        return reloaded;
      } catch (error) {
        if (requestId !== markdownMemoryRequestRef.current) return false;
        setMarkdownMemory({ phase: "failed", model: currentModel, reason: errorText(error) });
        announce("Markdown Memory 文件夹没有完成选择。");
        return false;
      }
    },
    [announce, dataSource, markdownMemory, reloadMarkdownMemory]
  );

  const proposeMarkdownMemoryWrite = useCallback(
    async (request: {
      scope: MarkdownMemoryScope;
      relativePath: string;
      content: string;
      expectedCurrentDigest?: string;
    }): Promise<boolean> => {
      if (!dataSource?.draftMarkdownMemoryFileProposal || !markdownMemory.model) {
        announce("当前不能提交 Markdown Memory 变更。");
        return false;
      }
      const model = markdownMemory.model;
      const requestId = ++markdownMemoryRequestRef.current;
      setMarkdownMemory({ phase: "submitting", model, operation: "write" });
      try {
        const receipt = await dataSource.draftMarkdownMemoryFileProposal(request);
        if (requestId !== markdownMemoryRequestRef.current) return false;
        setMarkdownMemory({ phase: "ready", model, lastProposal: receipt });
        announce("Markdown Memory 变更已进入 Review；当前文件尚未修改。");
        return true;
      } catch (error) {
        if (requestId !== markdownMemoryRequestRef.current) return false;
        setMarkdownMemory({ phase: "failed", model, reason: errorText(error) });
        announce("Markdown Memory 变更未能进入 Review；当前文件未修改。");
        return false;
      }
    },
    [announce, dataSource, markdownMemory.model]
  );

  const proposeMarkdownMemoryDeactivation = useCallback(
    async (request: {
      scope: MarkdownMemoryScope;
      relativePath: string;
      expectedCurrentDigest: string;
    }): Promise<boolean> => {
      if (!dataSource?.deactivateMarkdownMemoryFileProposal || !markdownMemory.model) {
        announce("当前不能停用 Markdown Memory 文件。");
        return false;
      }
      const model = markdownMemory.model;
      const requestId = ++markdownMemoryRequestRef.current;
      setMarkdownMemory({ phase: "submitting", model, operation: "deactivate" });
      try {
        const receipt = await dataSource.deactivateMarkdownMemoryFileProposal(request);
        if (requestId !== markdownMemoryRequestRef.current) return false;
        setMarkdownMemory({ phase: "ready", model, lastProposal: receipt });
        announce("停用请求已进入 Review；批准并物化前仍会继续召回当前文件。");
        return true;
      } catch (error) {
        if (requestId !== markdownMemoryRequestRef.current) return false;
        setMarkdownMemory({ phase: "failed", model, reason: errorText(error) });
        announce("停用请求未能进入 Review；当前文件仍保持启用。");
        return false;
      }
    },
    [announce, dataSource, markdownMemory.model]
  );

  useEffect(() => {
    let active = true;
    if (!dataSource?.listSkills || !dataSource.listToolCandidates || loadStatus !== "ready") {
      setSkills([]);
      setSelectedSkillId(null);
      setToolCandidates(null);
      setCapabilityState({ phase: "idle" });
      return () => {
        active = false;
      };
    }

    setCapabilityState({ phase: "loading" });
    void Promise.all([
      dataSource.listSkills(selectedSessionId ?? undefined),
      dataSource.listToolCandidates(activeTaskSessionId ?? undefined),
    ])
      .then(([nextSkills, nextTools]) => {
        if (!active) return;
        setSkills(nextSkills);
        setSelectedSkillId(nextSkills.find(skill => skill.selected)?.skillId ?? null);
        setToolCandidates(nextTools);
        setCapabilityState({ phase: "ready" });
      })
      .catch(error => {
        if (!active) return;
        setSkills([]);
        setSelectedSkillId(null);
        setToolCandidates(null);
        setCapabilityState({ phase: "failed", reason: errorText(error) });
      });

    return () => {
      active = false;
    };
  }, [activeTaskSessionId, dataSource, loadStatus, selectedSessionId]);

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
      const preferredStillExists =
        preferredSessionId && nextSessions.some(item => item.session_id === preferredSessionId);
      const nextSessionId = currentStillExists
        ? selectedSessionId
        : preferredStillExists
          ? preferredSessionId
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
  }, [dataSource, loadHistory, preferredSessionId, selectedSessionId]);

  useEffect(() => {
    if (
      !preferredSessionId ||
      loadStatus !== "ready" ||
      selectedSessionId === preferredSessionId ||
      explicitConversationChoiceRef.current ||
      draft.trim() ||
      pendingResources.length > 0 ||
      !sessions.some(session => session.session_id === preferredSessionId)
    ) {
      return;
    }
    const requestId = ++requestRef.current;
    setLoadStatus("loading");
    setLoadError(null);
    setTurnState({ phase: "idle" });
    setStreamingReply("");
    setActiveTaskSessionId(null);
    void loadHistory(preferredSessionId, requestId)
      .then(() => {
        if (requestId === requestRef.current) setLoadStatus("ready");
      })
      .catch(error => {
        if (requestId !== requestRef.current) return;
        setLoadStatus("error");
        setLoadError(errorText(error));
        setMessages([]);
      });
  }, [
    draft,
    loadHistory,
    loadStatus,
    pendingResources.length,
    preferredSessionId,
    selectedSessionId,
    sessions,
  ]);

  const ensureLoaded = useCallback(() => {
    if (loadedRef.current || loadStatus === "loading") return;
    void reload();
  }, [loadStatus, reload]);

  const busy =
    ["creating_session", "sending", "streaming", "cancelling", "refreshing"].includes(
      turnState.phase
    ) ||
    ["renaming", "deleting"].includes(sessionMutation.phase) ||
    ["importing", "detaching"].includes(resourceMutation.phase) ||
    capabilityState.phase === "selecting";

  const selectSkill = useCallback(
    async (skillId: string | null): Promise<boolean> => {
      if (
        !dataSource ||
        !selectedSessionId ||
        busy ||
        (skillId && !dataSource.selectSkill) ||
        (!skillId && !dataSource.clearSkill)
      ) {
        announce("当前不能改变技能；请先进入一段已保存且空闲的对话。");
        return false;
      }
      setCapabilityState({ phase: "selecting", skillId });
      try {
        const result = skillId
          ? await dataSource.selectSkill!(selectedSessionId, skillId)
          : await dataSource.clearSkill!(selectedSessionId);
        if (
          result.sessionId !== selectedSessionId ||
          (result.selectedSkillId ?? null) !== skillId
        ) {
          throw new Error("main_chat_skill_selection_identity_mismatch");
        }
        setSelectedSkillId(result.selectedSkillId ?? null);
        setSkills(current =>
          current.map(skill => ({ ...skill, selected: skill.skillId === result.selectedSkillId }))
        );
        setCapabilityState({ phase: "ready" });
        announce(
          result.selectedSkillId
            ? "技能已绑定到当前对话；它只作为有界上下文，不会提升权限。"
            : "当前对话已取消技能绑定；未选择的技能不会注入上下文。"
        );
        return true;
      } catch (error) {
        setCapabilityState({ phase: "failed", reason: errorText(error) });
        announce("技能选择没有得到后端确认；本轮不会使用新的技能。");
        return false;
      }
    },
    [announce, busy, dataSource, selectedSessionId]
  );

  const selectSession = useCallback(
    (sessionId: string) => {
      if (!sessionId || sessionId === selectedSessionId || busy) return;
      if (pendingResources.length > 0) {
        announce("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
        return;
      }
      explicitConversationChoiceRef.current = true;
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
    explicitConversationChoiceRef.current = true;
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

  const updateDraft = useCallback((value: string) => {
    if (value.trim()) explicitConversationChoiceRef.current = true;
    setDraft(value);
  }, []);

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
          { operationId: turnOperationId, selectedSkillId: selectedSkillId ?? undefined },
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
      selectedSkillId,
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
      skills,
      selectedSkillId,
      toolCandidates,
      capabilityState,
      markdownMemory,
      busy,
      ensureLoaded,
      reload,
      selectSession,
      startNewConversation,
      setDraft: updateDraft,
      attachResources,
      detachResource,
      selectSkill,
      reloadMarkdownMemory,
      selectMarkdownMemoryRoot,
      proposeMarkdownMemoryWrite,
      proposeMarkdownMemoryDeactivation,
      sendAction,
      send,
      cancel,
      renameSelected,
      deleteSelected,
    }),
    [
      busy,
      cancel,
      capabilityState,
      markdownMemory,
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
      reloadMarkdownMemory,
      reload,
      resourceMutation,
      selectSkill,
      selectMarkdownMemoryRoot,
      selectSession,
      selectedSessionId,
      selectedSkillId,
      sessionMutation,
      send,
      sendAction,
      sessions,
      skills,
      startNewConversation,
      streamingReply,
      activeTaskSessionId,
      turnState,
      toolCandidates,
      updateDraft,
      proposeMarkdownMemoryWrite,
      proposeMarkdownMemoryDeactivation,
    ]
  );
}
