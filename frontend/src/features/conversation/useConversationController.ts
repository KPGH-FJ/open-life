import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ProviderProfileViewModel,
  ChatSession,
  ConversationViewModel,
  ConversationMessageViewModel,
  ConversationMemoryMode,
  ImportedResourceReceipt,
  MainChatSkillSummary,
  MainChatSkillDetail,
  MainChatLifeModelProductReceipt,
  MainChatToolCandidateList,
  MainChatTurnStatus,
  ProductAction,
  ProjectLifecycleViewModel,
  ReasoningEffort,
  WorkExecutionMode,
  StreamMessageChunkPayload,
  StreamMessageStartPayload,
} from "@/tauri";
import type { ChatMessage } from "@/types";
import { productErrorCode as errorText } from "@/shared/productError";
import type { ConversationDataSource } from "./conversationDataSource";

type ConversationTranscriptMessage = ChatMessage &
  Partial<Pick<ConversationMessageViewModel, "turnId" | "attachmentsStatus" | "attachments">>;

type Announce = (message: string) => void;

export type ConversationLoadStatus = "idle" | "loading" | "ready" | "error";
export type ConversationMode = "chat" | "work";
export type ConversationProviderState = {
  status: "unknown" | "ready" | "unavailable";
  profiles: ProviderProfileViewModel[];
  selectedProfileId: string | null;
  selectedReasoningEffort: ReasoningEffort | null;
  errorCode: string | null;
};
export type ConversationWorkStatus = "unknown" | "available" | "unavailable";

function providerStateFromConversation(
  canonical: ConversationViewModel
): ConversationProviderState {
  const persistedEffort =
    canonical.latestTurn?.providerProfileId === canonical.selectedProviderProfileId
      ? (canonical.latestTurn.reasoningEffort ?? null)
      : null;
  return {
    status: canonical.providerStatus,
    profiles: canonical.providerProfiles,
    selectedProfileId: canonical.selectedProviderProfileId,
    selectedReasoningEffort: persistedEffort,
    errorCode: canonical.providerErrorCode,
  };
}
export type WorkspaceSessionMutationState =
  | { phase: "idle" }
  | { phase: "renaming"; sessionId: string }
  | { phase: "archiving"; sessionId: string }
  | { phase: "restoring"; sessionId: string }
  | { phase: "deleting"; sessionId: string }
  | { phase: "creating_project" }
  | { phase: "assigning_project"; projectId: string | null }
  | {
      phase: "mutating_project";
      action: "update" | "archive" | "restore" | "delete";
      projectId: string;
    }
  | { phase: "assigning_memory"; mode: ConversationMemoryMode }
  | {
      phase: "failed";
      action: "rename" | "archive" | "restore" | "delete" | "project" | "memory";
      reason: string;
    };

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

export type WorkspaceTurnState =
  | { phase: "idle" }
  | { phase: "creating_session" }
  | { phase: "sending"; sessionId: string }
  | {
      phase: "streaming";
      sessionId: string;
      turnId: string;
      taskId?: string;
      runId?: string;
      cancelError?: string;
    }
  | { phase: "cancelling"; sessionId: string; turnId: string; taskId?: string }
  | { phase: "refreshing"; sessionId: string; status: MainChatTurnStatus }
  | {
      phase: "resolved";
      sessionId: string;
      status: MainChatTurnStatus;
      blockers: string[];
      taskId?: string;
      runId?: string;
      lifeModelInfluence?: MainChatLifeModelProductReceipt;
    }
  | { phase: "failed"; stage: "create" | "send" | "refresh"; reason: string };

function sessionTitle(input: string): string {
  const normalized = input.replace(/\s+/g, " ").trim();
  return normalized.length > 28 ? `${normalized.slice(0, 28)}...` : normalized;
}

function resolvedTurnStatus(status: MainChatTurnStatus | undefined): MainChatTurnStatus {
  return status ?? "failed";
}

function streamIdentityMatches(
  payload: {
    session_id: string;
    operation_id: string;
    conversation_id?: string;
    turn_id?: string;
    task_id?: string;
  },
  mode: ConversationMode,
  sessionId: string,
  operationId: string
): boolean {
  if (payload.session_id !== sessionId || payload.operation_id !== operationId) return false;
  return mode === "chat"
    ? payload.conversation_id === sessionId && payload.turn_id === operationId
    : payload.conversation_id === sessionId && payload.turn_id === operationId;
}

function turnAnnouncement(status: MainChatTurnStatus): string {
  switch (status) {
    case "completed":
      return "回复已返回；工作区正在核对系统任务状态。";
    case "completed_with_pending_items":
      return "回复已返回，但存在待决定事项；它们不会被解释成已完成。";
    case "blocked":
      return "本轮被系统阻断；没有把阻断状态解释成完成。";
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

function steeringFailureAnnouncement(error: unknown): string {
  const code = errorText(error);
  if (code.includes("canonical_steering_checkpoint_passed")) {
    return "这次任务已经进入最终生成阶段，当前调整没有加入；完成后可以继续补充要求。";
  }
  if (code.includes("canonical_steering_pending_conflict")) {
    return "上一条调整仍在等待应用，请等它进入当前任务后再继续补充。";
  }
  if (
    code.includes("canonical_steering_target_terminal") ||
    code.includes("conversation_steering_target_not_running")
  ) {
    return "当前任务已经结束，不能再作为运行中调整；可以在对话中发起后续工作。";
  }
  return "当前调整未能加入任务，请稍后重试。";
}

export type ConversationController = {
  sessions: ChatSession[];
  archivedSessions: ChatSession[];
  projects: ProjectLifecycleViewModel[];
  selectedProjectId: string | null;
  selectedSessionId: string | null;
  globalMemoryEnabled: boolean;
  memoryMode: ConversationMemoryMode;
  messages: ConversationTranscriptMessage[];
  draft: string;
  loadStatus: ConversationLoadStatus;
  loadError: string | null;
  turnState: WorkspaceTurnState;
  streamingReply: string;
  activeTaskId: string | null;
  mode: ConversationMode;
  executionMode: WorkExecutionMode;
  provider: ConversationProviderState;
  workStatus: ConversationWorkStatus;
  sessionMutation: WorkspaceSessionMutationState;
  pendingResources: ImportedResourceReceipt[];
  pendingResourceTurnOperationId: string | null;
  resourceMutation: WorkspaceResourceMutationState;
  skills: MainChatSkillSummary[];
  selectedSkillId: string | null;
  selectedSkillDetail: MainChatSkillDetail | null;
  toolCandidates: MainChatToolCandidateList | null;
  capabilityState: WorkspaceCapabilityState;
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<boolean>;
  selectSession: (sessionId: string) => void;
  startNewConversation: () => void;
  createProject: (name: string) => Promise<boolean>;
  bindProjectDirectory: (projectId: string, expectedRevision: number) => Promise<boolean>;
  addProjectReadRoot: (projectId: string, expectedRevision: number) => Promise<boolean>;
  removeProjectReadRoot: (
    projectId: string,
    rootId: string,
    expectedRevision: number
  ) => Promise<boolean>;
  updateProjectName: (
    projectId: string,
    name: string,
    expectedRevision: number
  ) => Promise<boolean>;
  archiveProject: (projectId: string, expectedRevision: number) => Promise<boolean>;
  restoreProject: (projectId: string, expectedRevision: number) => Promise<boolean>;
  deleteProject: (projectId: string, expectedRevision: number) => Promise<boolean>;
  assignProject: (projectId: string | null) => Promise<boolean>;
  setMemoryMode: (mode: ConversationMemoryMode) => Promise<boolean>;
  selectProviderProfile: (profileId: string) => Promise<boolean>;
  selectReasoningEffort: (effort: ReasoningEffort | null) => boolean;
  setDraft: (value: string) => void;
  setMode: (mode: ConversationMode) => void;
  setExecutionMode: (mode: WorkExecutionMode) => boolean;
  attachResources: () => Promise<boolean>;
  detachResource: (resourceId: string) => Promise<boolean>;
  selectSkill: (skillId: string | null) => Promise<boolean>;
  sendAction: (disabledReason?: string) => ProductAction;
  send: (disabledReason?: string) => Promise<void>;
  steer: () => Promise<void>;
  cancel: () => Promise<void>;
  renameSelected: (title: string) => Promise<boolean>;
  archiveSelected: () => Promise<boolean>;
  restoreArchived: (sessionId: string) => Promise<boolean>;
  deleteArchived: (sessionId: string) => Promise<boolean>;
  deleteSelected: () => Promise<boolean>;
};

export function useConversationController(
  dataSource: ConversationDataSource | undefined,
  announce: Announce,
  onAfterTurn: (conversationId: string) => Promise<void>,
  preferredSessionId?: string | null,
  stopRunningWork?: (taskId: string, runId: string) => Promise<void>,
  onAfterProjectScopeChange?: (conversationId: string | null) => Promise<void>
): ConversationController {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [archivedSessions, setArchivedSessions] = useState<ChatSession[]>([]);
  const [projects, setProjects] = useState<ProjectLifecycleViewModel[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [globalMemoryEnabled, setGlobalMemoryEnabled] = useState(true);
  const [memoryMode, setMemoryModeState] = useState<ConversationMemoryMode>("use_and_learn");
  const [messages, setMessages] = useState<ConversationTranscriptMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [loadStatus, setLoadStatus] = useState<ConversationLoadStatus>("idle");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [turnState, setTurnState] = useState<WorkspaceTurnState>({ phase: "idle" });
  const [streamingReply, setStreamingReply] = useState("");
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [mode, setModeState] = useState<ConversationMode>("chat");
  const [executionMode, setExecutionModeState] = useState<WorkExecutionMode>("scoped_agent");
  const [provider, setProvider] = useState<ConversationProviderState>({
    status: "unknown",
    profiles: [],
    selectedProfileId: null,
    selectedReasoningEffort: null,
    errorCode: null,
  });
  const [workStatus, setWorkStatus] = useState<ConversationWorkStatus>("unknown");
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
  const [selectedSkillDetail, setSelectedSkillDetail] = useState<MainChatSkillDetail | null>(null);
  const [toolCandidates, setToolCandidates] = useState<MainChatToolCandidateList | null>(null);
  const [capabilityState, setCapabilityState] = useState<WorkspaceCapabilityState>({
    phase: "idle",
  });
  const requestRef = useRef(0);
  const operationRef = useRef(0);
  const cancelRequestRef = useRef(0);
  const loadedRef = useRef(false);
  const explicitConversationChoiceRef = useRef(false);

  useEffect(() => {
    requestRef.current += 1;
    operationRef.current += 1;
    cancelRequestRef.current += 1;
    loadedRef.current = false;
    explicitConversationChoiceRef.current = false;
    setSessions([]);
    setArchivedSessions([]);
    setProjects([]);
    setSelectedProjectId(null);
    setSelectedSessionId(null);
    setGlobalMemoryEnabled(true);
    setMemoryModeState("use_and_learn");
    setMessages([]);
    setDraft("");
    setLoadStatus("idle");
    setLoadError(null);
    setTurnState({ phase: "idle" });
    setStreamingReply("");
    setActiveTaskId(null);
    setModeState("chat");
    setExecutionModeState("scoped_agent");
    setProvider({
      status: "unknown",
      profiles: [],
      selectedProfileId: null,
      selectedReasoningEffort: null,
      errorCode: null,
    });
    setWorkStatus("unknown");
    setSessionMutation({ phase: "idle" });
    setPendingResources([]);
    setPendingResourceTurnOperationId(null);
    setResourceMutation({ phase: "idle" });
    setSkills([]);
    setSelectedSkillId(null);
    setSelectedSkillDetail(null);
    setToolCandidates(null);
    setCapabilityState({ phase: "idle" });
    return () => {
      requestRef.current += 1;
      operationRef.current += 1;
      cancelRequestRef.current += 1;
      loadedRef.current = false;
    };
  }, [dataSource]);

  useEffect(() => {
    let active = true;
    if (!dataSource?.listSkills || loadStatus !== "ready") {
      setSkills([]);
      setSelectedSkillId(null);
      setToolCandidates(null);
      setCapabilityState({ phase: "idle" });
      return () => {
        active = false;
      };
    }

    setCapabilityState({ phase: "loading" });
    const shouldLoadWorkTools =
      mode === "work" && workStatus === "available" && Boolean(dataSource.listToolCandidates);
    void Promise.all([
      dataSource.listSkills(selectedSessionId ?? undefined),
      shouldLoadWorkTools
        ? dataSource.listToolCandidates!(activeTaskId ?? undefined)
        : Promise.resolve(null),
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
  }, [activeTaskId, dataSource, loadStatus, mode, selectedSessionId, workStatus]);

  useEffect(() => {
    let active = true;
    if (!selectedSkillId || !dataSource?.getSkillDetail) {
      setSelectedSkillDetail(null);
      return () => {
        active = false;
      };
    }
    void dataSource
      .getSkillDetail(selectedSkillId)
      .then(detail => {
        if (active && detail.skillId === selectedSkillId) setSelectedSkillDetail(detail);
      })
      .catch(() => {
        if (active) setSelectedSkillDetail(null);
      });
    return () => {
      active = false;
    };
  }, [dataSource, selectedSkillId]);

  const loadHistory = useCallback(
    async (sessionId: string, requestId: number): Promise<void> => {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const canonical = await dataSource.loadConversation(sessionId);
      if (canonical.selectedConversationId !== sessionId) {
        throw new Error("conversation_view_model_selection_mismatch");
      }
      if (requestId !== requestRef.current) return;
      setProjects(canonical.projects);
      setSelectedProjectId(canonical.selectedProjectId);
      setGlobalMemoryEnabled(canonical.globalMemoryEnabled);
      setMemoryModeState(canonical.selectedMemoryMode);
      setProvider(providerStateFromConversation(canonical));
      setWorkStatus(canonical.workStatus);
      setSelectedSessionId(sessionId);
      setMessages(canonical.messages);
      if (
        canonical.latestTurn?.status === "failed" ||
        canonical.latestTurn?.status === "cancelled" ||
        canonical.latestTurn?.status === "interrupted"
      ) {
        setTurnState({
          phase: "resolved",
          sessionId,
          status: canonical.latestTurn.status,
          blockers: canonical.latestTurn.errorCode ? [canonical.latestTurn.errorCode] : [],
          taskId: canonical.latestTurn.taskId ?? undefined,
          runId: canonical.latestTurn.runId ?? undefined,
        });
      } else {
        setTurnState({ phase: "idle" });
      }
    },
    [dataSource]
  );

  const reload = useCallback(async (): Promise<boolean> => {
    const requestId = ++requestRef.current;
    setLoadStatus("loading");
    setLoadError(null);
    try {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const canonical = await dataSource.loadConversation(
        selectedSessionId ?? preferredSessionId ?? undefined
      );
      const nextSessions = canonical.conversations;
      setArchivedSessions(canonical.archivedConversations ?? []);
      setProjects(canonical.projects);
      setSelectedProjectId(canonical.selectedProjectId);
      setGlobalMemoryEnabled(canonical.globalMemoryEnabled);
      setMemoryModeState(canonical.selectedMemoryMode);
      setProvider(providerStateFromConversation(canonical));
      setWorkStatus(canonical.workStatus);
      if (requestId !== requestRef.current) return false;
      setSessions(nextSessions);
      const currentStillExists =
        selectedSessionId && nextSessions.some(item => item.session_id === selectedSessionId);
      const preferredStillExists =
        preferredSessionId && nextSessions.some(item => item.session_id === preferredSessionId);
      const nextSessionId = canonical.selectedConversationId
        ? canonical.selectedConversationId
        : currentStillExists
          ? selectedSessionId
          : preferredStillExists
            ? preferredSessionId
            : (nextSessions[0]?.session_id ?? null);
      if (nextSessionId) {
        if (canonical.selectedConversationId === nextSessionId) {
          setSelectedSessionId(nextSessionId);
          setMessages(canonical.messages);
          setSelectedProjectId(canonical.selectedProjectId);
          setMemoryModeState(canonical.selectedMemoryMode);
          const turn = canonical.latestTurn;
          setTurnState(
            !turn || turn.status === "completed"
              ? { phase: "idle" }
              : turn.status === "running"
                ? { phase: "idle" }
                : {
                    phase: "resolved",
                    sessionId: nextSessionId,
                    status: turn.status,
                    blockers: turn.errorCode ? [turn.errorCode] : [],
                    taskId: turn.taskId ?? undefined,
                    runId: turn.runId ?? undefined,
                  }
          );
        } else {
          await loadHistory(nextSessionId, requestId);
        }
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
    setActiveTaskId(null);
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
    [
      "renaming",
      "archiving",
      "restoring",
      "deleting",
      "creating_project",
      "assigning_project",
      "mutating_project",
    ].includes(sessionMutation.phase) ||
    ["importing", "detaching"].includes(resourceMutation.phase) ||
    capabilityState.phase === "selecting";

  const selectSkill = useCallback(
    async (skillId: string | null): Promise<boolean> => {
      if (!selectedSessionId) {
        const selected = skillId
          ? skills.find(skill => skill.skillId === skillId && skill.available)
          : null;
        if (skillId && !selected) {
          announce("所选技能当前不可用；新对话不会绑定它。");
          return false;
        }
        setSelectedSkillId(skillId);
        setSkills(current =>
          current.map(skill => ({ ...skill, selected: skill.skillId === skillId }))
        );
        announce(
          skillId
            ? "技能将在创建对话时原子绑定；它不会扩大模型、网络、工具或写入权限。"
            : "新对话将不使用技能。"
        );
        return true;
      }
      if (
        !dataSource ||
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
        announce("技能选择没有得到系统确认；本轮不会使用新的技能。");
        return false;
      }
    },
    [announce, busy, dataSource, selectedSessionId, skills]
  );

  const selectSession = useCallback(
    (sessionId: string) => {
      const canDetachBackgroundWork = mode === "work" && turnState.phase === "streaming";
      if (!sessionId || sessionId === selectedSessionId || (busy && !canDetachBackgroundWork)) {
        return;
      }
      if (pendingResources.length > 0) {
        announce("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
        return;
      }
      explicitConversationChoiceRef.current = true;
      if (canDetachBackgroundWork) {
        operationRef.current += 1;
        cancelRequestRef.current += 1;
        announce("任务会在后台继续；已切换到另一段对话，可在需要处理中查看后续状态。");
      }
      const requestId = ++requestRef.current;
      setLoadStatus("loading");
      setLoadError(null);
      setTurnState({ phase: "idle" });
      setStreamingReply("");
      setActiveTaskId(null);
      const persistSelection = dataSource?.selectConversation
        ? dataSource.selectConversation(sessionId)
        : Promise.resolve();
      void persistSelection
        .then(() => loadHistory(sessionId, requestId))
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
    [
      announce,
      busy,
      dataSource,
      loadHistory,
      mode,
      pendingResources.length,
      selectedSessionId,
      turnState.phase,
    ]
  );

  const startNewConversation = useCallback(() => {
    const canDetachBackgroundWork = mode === "work" && turnState.phase === "streaming";
    if (busy && !canDetachBackgroundWork) return;
    if (pendingResources.length > 0) {
      announce("当前有文件绑定到下一次发送；请先发送或逐个移除文件。");
      return;
    }
    explicitConversationChoiceRef.current = true;
    if (canDetachBackgroundWork) {
      operationRef.current += 1;
      cancelRequestRef.current += 1;
      announce("任务会在后台继续；已打开新对话，可在需要处理中查看后续状态。");
    }
    requestRef.current += 1;
    setSelectedSessionId(null);
    setMessages([]);
    setDraft("");
    setLoadError(null);
    setLoadStatus("ready");
    setTurnState({ phase: "idle" });
    setStreamingReply("");
    setActiveTaskId(null);
    if (!canDetachBackgroundWork) {
      announce("已打开新对话草稿；发送前不会创建会话或写入记录。");
    }
  }, [announce, busy, mode, pendingResources.length, turnState.phase]);

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
      setModeState("work");
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
        announce("文件移除没有得到系统确认；当前仍按已绑定处理。");
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
                : mode === "work" && workStatus !== "available"
                  ? "Work 模式当前不可用。"
                  : provider.status === "unavailable"
                    ? "当前选择的模型不可用；请选择可用模型或前往设置完成配置。"
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
    [busy, dataSource, draft, loadStatus, mode, provider.status, selectedSessionId, workStatus]
  );

  const setMode = useCallback(
    (nextMode: ConversationMode) => {
      if (busy) {
        announce("当前轮次结束前不能切换 Chat / Work。");
        return;
      }
      if (nextMode === "chat" && pendingResources.length > 0) {
        announce("已添加的文件属于 Work 范围；移除文件后才能切回 Chat。");
        return;
      }
      setModeState(nextMode);
      announce(nextMode === "chat" ? "已切换为 Chat。" : "已切换为 Work。");
    },
    [announce, busy, pendingResources.length]
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
      const workTaskId = mode === "work" ? crypto.randomUUID() : undefined;
      const workRunId = mode === "work" ? crypto.randomUUID() : undefined;
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
          await dataSource.createSession(sessionId, sessionTitle(text), {
            projectId: selectedProjectId,
            memoryMode,
            selectedSkillId,
          });
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
        setActiveTaskId(null);
        announce("正在发送；命令返回后仍会重新读取会话和工作区状态。");

        const result = await dataSource.streamTurn(
          turnSessionId,
          requestMessages,
          {
            operationId: turnOperationId,
            selectedSkillId: selectedSkillId ?? undefined,
            providerProfileId: provider.selectedProfileId ?? undefined,
            reasoningEffort: provider.selectedReasoningEffort ?? undefined,
            executionMode: mode === "work" ? executionMode : undefined,
            mode,
            taskId: workTaskId,
            runId: workRunId,
          },
          {
            onStart: (payload: StreamMessageStartPayload) => {
              if (operationId !== operationRef.current) return;
              if (
                pendingResources.length > 0 &&
                !streamIdentityMatches(payload, mode, turnSessionId, turnOperationId)
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
              const activeTaskId = payload.task_id ?? null;
              setActiveTaskId(activeTaskId);
              setTurnState({
                phase: "streaming",
                sessionId: turnSessionId,
                turnId: payload.turn_id ?? turnOperationId,
                taskId: activeTaskId ?? undefined,
                runId: payload.run_id,
              });
              announce("OpenLife 正在回复；现在可以取消这一轮对话。");
            },
            onChunk: (payload: StreamMessageChunkPayload) => {
              if (operationId !== operationRef.current) return;
              if (
                pendingResources.length > 0 &&
                !streamIdentityMatches(payload, mode, turnSessionId, turnOperationId)
              ) {
                resourceStreamIdentityMismatch = true;
                return;
              }
              setStreamingReply(current => current + payload.chunk);
            },
          }
        );
        if (operationId !== operationRef.current) {
          await onAfterTurn(turnSessionId).catch(() => undefined);
          return;
        }
        if (
          pendingResources.length > 0 &&
          (resourceStreamIdentityMismatch ||
            !streamIdentityMatches(result, mode, turnSessionId, turnOperationId))
        ) {
          throw new Error("resource_turn_terminal_identity_mismatch");
        }
        if (pendingResources.length > 0) {
          setPendingResources([]);
          setPendingResourceTurnOperationId(null);
          setResourceMutation({ phase: "idle" });
        }
        cancelRequestRef.current += 1;
        const status = resolvedTurnStatus(result.status);
        setActiveTaskId(result.task_id ?? null);
        setTurnState({ phase: "refreshing", sessionId, status });

        let refreshedConversation: ConversationViewModel;
        try {
          refreshedConversation = await dataSource.loadConversation(sessionId);
        } catch (error) {
          if (operationId !== operationRef.current) return;
          setTurnState({ phase: "failed", stage: "refresh", reason: errorText(error) });
          announce("命令已返回，但会话记录刷新失败；当前不确认回复已持久化。");
          await onAfterTurn(sessionId);
          return;
        }
        if (operationId !== operationRef.current) return;
        setMessages(refreshedConversation.messages);
        setProvider(providerStateFromConversation(refreshedConversation));
        setWorkStatus(refreshedConversation.workStatus);
        setStreamingReply("");
        setActiveTaskId(null);
        setTurnState({
          phase: "resolved",
          sessionId,
          status,
          blockers: result.blockers ?? [],
          taskId: refreshedConversation.latestTurn?.taskId ?? result.task_id ?? undefined,
          runId: refreshedConversation.latestTurn?.runId ?? result.run_id ?? undefined,
          lifeModelInfluence: result.life_model_influence,
        });
        await onAfterTurn(sessionId);
        announce(turnAnnouncement(status));
      } catch (error) {
        if (operationId !== operationRef.current) {
          if (sessionId) await onAfterTurn(sessionId).catch(() => undefined);
          return;
        }
        cancelRequestRef.current += 1;
        const failureCode = errorText(error);
        if (
          streamStarted &&
          sessionId &&
          (failureCode === "canonical_work_cancelled" ||
            failureCode === "canonical_chat_turn_cancelled")
        ) {
          try {
            const refreshedConversation = await dataSource.loadConversation(sessionId);
            if (operationId !== operationRef.current) return;
            setMessages(refreshedConversation.messages);
            setProvider(providerStateFromConversation(refreshedConversation));
            setWorkStatus(refreshedConversation.workStatus);
          } catch {
            if (operationId !== operationRef.current) return;
          }
          setStreamingReply("");
          setActiveTaskId(null);
          setTurnState({
            phase: "resolved",
            sessionId,
            status: "cancelled",
            blockers: [],
          });
          await onAfterTurn(sessionId);
          announce(turnAnnouncement("cancelled"));
          return;
        }
        if (!streamStarted) setDraft(text);
        setTurnState({
          phase: "failed",
          stage: sessionCreated || selectedSessionId ? "send" : "create",
          reason: failureCode,
        });
        announce(
          sessionCreated || selectedSessionId
            ? mode === "work"
              ? "这项工作未完成；请在结果或需处理中核对系统记录的任务状态。"
              : "消息发送失败；当前不会显示成功结论。"
            : "新会话未能建立；没有发送消息。"
        );
        try {
          if (sessionId) {
            const refreshedConversation = await dataSource.loadConversation(sessionId);
            if (operationId === operationRef.current) {
              setMessages(refreshedConversation.messages);
              setProvider(providerStateFromConversation(refreshedConversation));
              setWorkStatus(refreshedConversation.workStatus);
              const terminalTurn = refreshedConversation.latestTurn;
              if (
                terminalTurn &&
                (terminalTurn.status === "failed" ||
                  terminalTurn.status === "cancelled" ||
                  terminalTurn.status === "interrupted")
              ) {
                setTurnState({
                  phase: "resolved",
                  sessionId,
                  status: terminalTurn.status,
                  blockers: terminalTurn.errorCode ? [terminalTurn.errorCode] : [],
                  taskId: terminalTurn.taskId ?? undefined,
                  runId: terminalTurn.runId ?? undefined,
                });
              }
            }
          }
        } catch {
          // The explicit failed state already communicates that persistence is unverified.
        }
        // A failed stream may still have durably created or terminalized a
        // canonical Work Task (for example, a blocked tool attempt). Refresh
        // the backend-owned Workspace projection on both success and failure
        // so Results / Needs Attention never disappear behind the composer
        // error state.
        if (operationId === operationRef.current && sessionId) await onAfterTurn(sessionId);
      }
    },
    [
      announce,
      dataSource,
      draft,
      memoryMode,
      messages,
      mode,
      executionMode,
      provider,
      workStatus,
      onAfterTurn,
      pendingResourceTurnOperationId,
      pendingResources,
      selectedSessionId,
      selectedSkillId,
      selectedSkillDetail,
      selectedProjectId,
      sendAction,
    ]
  );

  const cancel = useCallback(async (): Promise<void> => {
    if (!dataSource || turnState.phase !== "streaming" || !turnState.turnId.trim()) {
      announce("当前没有可以停止的运行。");
      return;
    }
    const { sessionId, turnId, taskId, runId } = turnState;
    const cancelRequestId = ++cancelRequestRef.current;
    setTurnState({ phase: "cancelling", sessionId, turnId, taskId });
    announce("正在停止当前运行；只有系统终态返回后才会确认已停止。");
    try {
      if (mode === "work" && taskId && runId && stopRunningWork) {
        await stopRunningWork(taskId, runId);
      } else if (dataSource.cancelChatTurn) {
        await dataSource.cancelChatTurn(sessionId, turnId);
      } else {
        throw new Error("canonical_turn_cancel_unavailable");
      }
    } catch (error) {
      if (cancelRequestId !== cancelRequestRef.current) return;
      setTurnState({
        phase: "streaming",
        sessionId,
        turnId,
        taskId,
        runId: turnState.runId,
        cancelError: errorText(error),
      });
      announce("停止请求失败；当前运行仍按进行中处理。");
    }
  }, [announce, dataSource, mode, stopRunningWork, turnState]);

  const steer = useCallback(async (): Promise<void> => {
    const text = draft.trim();
    if (
      !dataSource?.steerTask ||
      turnState.phase !== "streaming" ||
      !selectedSessionId ||
      !activeTaskId ||
      !turnState.runId ||
      !text
    ) {
      announce("当前任务尚未到可接收调整的运行状态。");
      return;
    }
    try {
      await dataSource.steerTask({
        steeringId: crypto.randomUUID(),
        taskId: activeTaskId,
        runId: turnState.runId,
        sessionId: selectedSessionId,
        content: text,
      });
      setDraft("");
      announce("调整已加入当前任务，正在等待 canonical Work 的安全检查点处理。");
    } catch (error) {
      announce(steeringFailureAnnouncement(error));
    }
  }, [activeTaskId, announce, dataSource, draft, selectedSessionId, turnState]);

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
        announce("对话名称已从系统重新读取并确认。");
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
      announce("对话删除已由系统重新读取确认。");
      return true;
    } catch (error) {
      setSessionMutation({ phase: "failed", action: "delete", reason: errorText(error) });
      announce("对话删除失败；当前仍按未删除处理。");
      return false;
    }
  }, [announce, busy, dataSource, pendingResources.length, reload, selectedSessionId]);

  const archiveSelected = useCallback(async (): Promise<boolean> => {
    if (!dataSource?.archiveSession || !selectedSessionId || busy || pendingResources.length > 0) {
      announce("当前不能归档这段对话。");
      return false;
    }
    const targetSessionId = selectedSessionId;
    setSessionMutation({ phase: "archiving", sessionId: targetSessionId });
    try {
      await dataSource.archiveSession(targetSessionId);
      if (!(await reload())) throw new Error("conversation_refresh_failed_after_archive");
      setSessionMutation({ phase: "idle" });
      announce("对话已归档；消息、任务、文件和证据仍由原所有者保留。");
      return true;
    } catch (error) {
      setSessionMutation({ phase: "failed", action: "archive", reason: errorText(error) });
      announce("对话归档失败；当前仍按未归档处理。");
      return false;
    }
  }, [announce, busy, dataSource, pendingResources.length, reload, selectedSessionId]);

  const restoreArchived = useCallback(
    async (sessionId: string): Promise<boolean> => {
      if (!dataSource?.restoreSession || busy) {
        announce("当前不能恢复这段对话。");
        return false;
      }
      setSessionMutation({ phase: "restoring", sessionId });
      try {
        await dataSource.restoreSession(sessionId);
        if (!(await reload())) throw new Error("conversation_refresh_failed_after_restore");
        setSessionMutation({ phase: "idle" });
        announce("对话已恢复并由系统重新读取确认。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "restore", reason: errorText(error) });
        announce("对话恢复失败；当前仍按已归档处理。");
        return false;
      }
    },
    [announce, busy, dataSource, reload]
  );

  const deleteArchived = useCallback(
    async (sessionId: string): Promise<boolean> => {
      if (!dataSource || busy) {
        announce("当前不能永久删除这段对话记录。");
        return false;
      }
      setSessionMutation({ phase: "deleting", sessionId });
      try {
        await dataSource.deleteSession(sessionId);
        if (!(await reload())) throw new Error("conversation_refresh_failed_after_delete");
        setSessionMutation({ phase: "idle" });
        announce("空对话记录的永久删除已由系统重新读取确认。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "delete", reason: errorText(error) });
        announce("对话删除失败；当前仍按未删除处理。");
        return false;
      }
    },
    [announce, busy, dataSource, reload]
  );

  const createProject = useCallback(
    async (name: string): Promise<boolean> => {
      const normalized = name.replace(/\s+/g, " ").trim();
      const draftProviderProfileId = provider.selectedProfileId;
      const draftReasoningEffort = provider.selectedReasoningEffort;
      const draftMemoryMode = memoryMode;
      if (!dataSource?.createProject || busy) {
        announce("当前不能创建 Project。");
        return false;
      }
      setSessionMutation({ phase: "creating_project" });
      try {
        const result = await dataSource.createProject(crypto.randomUUID(), normalized || undefined);
        if (result.cancelled) {
          setSessionMutation({ phase: "idle" });
          announce("已取消选择 Project 文件夹。");
          return false;
        }
        const project = result.project;
        if (!project?.workspaceRoot) throw new Error("project_workspace_root_missing");
        if (selectedSessionId && dataSource.assignProject) {
          await dataSource.assignProject(selectedSessionId, project.id);
        }
        if (!(await reload())) throw new Error("project_refresh_failed_after_create");
        if (!selectedSessionId) {
          explicitConversationChoiceRef.current = true;
          requestRef.current += 1;
          setSelectedSessionId(null);
          setMessages([]);
          setTurnState({ phase: "idle" });
          setStreamingReply("");
          setActiveTaskId(null);
          setSelectedProjectId(project.id);
          setMemoryModeState(draftMemoryMode);
          setProvider(current => {
            const selected = current.profiles.find(
              profile => profile.profileId === draftProviderProfileId
            );
            if (!selected || selected.availability !== "ready") return current;
            return {
              ...current,
              status: "ready",
              selectedProfileId: selected.profileId,
              selectedReasoningEffort:
                draftReasoningEffort === null ||
                selected.supportedReasoningEfforts.includes(draftReasoningEffort)
                  ? draftReasoningEffort
                  : null,
              errorCode: null,
            };
          });
        }
        setModeState("work");
        setSessionMutation({ phase: "idle" });
        announce(
          selectedSessionId
            ? "Project 文件夹已创建并绑定到当前对话；运行方式已切换为 Work。"
            : "Project 文件夹已选择；新的 Work 对话会在首次发送时创建。"
        );
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("Project 创建或绑定未得到系统确认。");
        return false;
      }
    },
    [
      announce,
      busy,
      dataSource,
      memoryMode,
      provider.selectedProfileId,
      provider.selectedReasoningEffort,
      reload,
      selectedSessionId,
    ]
  );

  const bindProjectDirectory = useCallback(
    async (projectId: string, expectedRevision: number): Promise<boolean> => {
      const preserveNewConversationDraft = selectedSessionId === null;
      const draftProviderProfileId = provider.selectedProfileId;
      const draftReasoningEffort = provider.selectedReasoningEffort;
      const draftMemoryMode = memoryMode;
      if (!dataSource?.bindProjectDirectory || busy) {
        announce("当前不能改变 Project 文件夹。");
        return false;
      }
      setSessionMutation({ phase: "creating_project" });
      try {
        const result = await dataSource.bindProjectDirectory(projectId, expectedRevision);
        if (result.cancelled) {
          setSessionMutation({ phase: "idle" });
          announce("已取消选择 Project 文件夹。");
          return false;
        }
        if (!result.project?.workspaceRoot) throw new Error("project_workspace_root_missing");
        if (!(await reload())) throw new Error("project_refresh_failed_after_directory_binding");
        await onAfterProjectScopeChange?.(selectedSessionId);
        if (preserveNewConversationDraft) {
          explicitConversationChoiceRef.current = true;
          requestRef.current += 1;
          setSelectedSessionId(null);
          setMessages([]);
          setTurnState({ phase: "idle" });
          setStreamingReply("");
          setActiveTaskId(null);
          setSelectedProjectId(projectId);
          setMemoryModeState(draftMemoryMode);
          setProvider(current => {
            const selected = current.profiles.find(
              profile => profile.profileId === draftProviderProfileId
            );
            if (!selected || selected.availability !== "ready") return current;
            return {
              ...current,
              status: "ready",
              selectedProfileId: selected.profileId,
              selectedReasoningEffort:
                draftReasoningEffort === null ||
                selected.supportedReasoningEfforts.includes(draftReasoningEffort)
                  ? draftReasoningEffort
                  : null,
              errorCode: null,
            };
          });
        }
        setModeState("work");
        setSessionMutation({ phase: "idle" });
        announce("Project 文件夹范围已更新并由系统重新读取确认。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("Project 文件夹更新未得到系统确认。");
        return false;
      }
    },
    [
      announce,
      busy,
      dataSource,
      memoryMode,
      provider.selectedProfileId,
      provider.selectedReasoningEffort,
      reload,
      onAfterProjectScopeChange,
      selectedSessionId,
    ]
  );

  const addProjectReadRoot = useCallback(
    async (projectId: string, expectedRevision: number): Promise<boolean> => {
      if (!dataSource?.addProjectReadRoot || busy) {
        announce("当前不能添加 Project 读取文件夹。");
        return false;
      }
      setSessionMutation({ phase: "creating_project" });
      try {
        const result = await dataSource.addProjectReadRoot(projectId, expectedRevision);
        if (result.cancelled) {
          setSessionMutation({ phase: "idle" });
          announce("已取消添加读取文件夹。");
          return false;
        }
        if (!result.project) throw new Error("project_read_root_missing");
        if (!(await reload())) throw new Error("project_refresh_failed_after_read_root_add");
        await onAfterProjectScopeChange?.(selectedSessionId);
        setSessionMutation({ phase: "idle" });
        announce("读取文件夹已加入 Project 范围；它不会获得文件写入权限。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("读取文件夹没有加入 Project 范围。");
        return false;
      }
    },
    [announce, busy, dataSource, onAfterProjectScopeChange, reload, selectedSessionId]
  );

  const removeProjectReadRoot = useCallback(
    async (projectId: string, rootId: string, expectedRevision: number): Promise<boolean> => {
      if (!dataSource?.removeProjectReadRoot || busy) {
        announce("当前不能移除 Project 读取文件夹。");
        return false;
      }
      setSessionMutation({ phase: "mutating_project", action: "update", projectId });
      try {
        await dataSource.removeProjectReadRoot(projectId, rootId, expectedRevision);
        if (!(await reload())) throw new Error("project_refresh_failed_after_read_root_remove");
        await onAfterProjectScopeChange?.(selectedSessionId);
        setSessionMutation({ phase: "idle" });
        announce("读取范围已移除；本地文件夹和文件没有被删除。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("读取范围移除没有得到系统确认。");
        return false;
      }
    },
    [announce, busy, dataSource, onAfterProjectScopeChange, reload, selectedSessionId]
  );

  const mutateProject = useCallback(
    async (
      action: "update" | "archive" | "restore" | "delete",
      projectId: string,
      expectedRevision: number,
      name?: string
    ): Promise<boolean> => {
      const operation = {
        update: dataSource?.updateProjectName,
        archive: dataSource?.archiveProject,
        restore: dataSource?.restoreProject,
        delete: dataSource?.deleteProject,
      }[action];
      if (!operation || busy) {
        announce("当前不能改变 Project。请等待现有操作结束后重试。");
        return false;
      }
      setSessionMutation({ phase: "mutating_project", action, projectId });
      try {
        if (action === "update") {
          const normalized = name?.replace(/\s+/g, " ").trim() ?? "";
          if (!normalized) throw new Error("project_name_empty");
          await dataSource!.updateProjectName!(projectId, normalized, expectedRevision);
        } else if (action === "archive") {
          await dataSource!.archiveProject!(projectId, expectedRevision);
        } else if (action === "restore") {
          await dataSource!.restoreProject!(projectId, expectedRevision);
        } else {
          await dataSource!.deleteProject!(projectId, expectedRevision);
        }
        if (!(await reload())) throw new Error(`project_refresh_failed_after_${action}`);
        setSessionMutation({ phase: "idle" });
        announce(
          {
            update: "Project 名称已更新并由系统重新读取确认。",
            archive: "Project 已归档；本地文件夹和内容没有被删除。",
            restore: "Project 已恢复，可以重新绑定到对话。",
            delete: "Project 元数据已永久删除；本地文件夹和内容没有被删除。",
          }[action]
        );
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce(
          action === "delete"
            ? "Project 删除没有完成；系统仍按未删除处理。"
            : "Project 变更没有得到系统确认。"
        );
        return false;
      }
    },
    [announce, busy, dataSource, reload]
  );

  const updateProjectName = useCallback(
    (projectId: string, name: string, expectedRevision: number) =>
      mutateProject("update", projectId, expectedRevision, name),
    [mutateProject]
  );

  const archiveProject = useCallback(
    (projectId: string, expectedRevision: number) =>
      mutateProject("archive", projectId, expectedRevision),
    [mutateProject]
  );

  const restoreProject = useCallback(
    (projectId: string, expectedRevision: number) =>
      mutateProject("restore", projectId, expectedRevision),
    [mutateProject]
  );

  const deleteProject = useCallback(
    (projectId: string, expectedRevision: number) =>
      mutateProject("delete", projectId, expectedRevision),
    [mutateProject]
  );

  const assignProject = useCallback(
    async (projectId: string | null): Promise<boolean> => {
      const canAssignExisting = Boolean(selectedSessionId && dataSource?.assignProject);
      const canSelectForNew = Boolean(
        !selectedSessionId && dataSource?.selectProjectForNewConversation
      );
      if ((!canAssignExisting && !canSelectForNew) || busy) {
        announce("当前不能改变这段对话的 Project。");
        return false;
      }
      setSessionMutation({ phase: "assigning_project", projectId });
      try {
        if (selectedSessionId) {
          await dataSource!.assignProject!(selectedSessionId, projectId);
        } else {
          await dataSource!.selectProjectForNewConversation!(projectId);
        }
        if (!(await reload())) throw new Error("project_refresh_failed_after_assign");
        setSessionMutation({ phase: "idle" });
        announce(
          selectedSessionId
            ? projectId
              ? "当前对话已绑定到 Project。"
              : "当前对话已移出 Project。"
            : projectId
              ? "Project 已选为下一段新对话的工作范围。"
              : "下一段新对话将不属于 Project。"
        );
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("Project 绑定未得到系统确认。");
        return false;
      }
    },
    [announce, busy, dataSource, reload, selectedSessionId]
  );

  const setMemoryMode = useCallback(
    async (nextMode: ConversationMemoryMode): Promise<boolean> => {
      if (busy || !globalMemoryEnabled) {
        announce("当前不能改变这段对话的记忆设置。");
        return false;
      }
      if (!selectedSessionId) {
        setMemoryModeState(nextMode);
        announce("下一段新对话将使用这个记忆设置。");
        return true;
      }
      if (!dataSource?.setMemoryMode) {
        announce("当前不能改变这段对话的记忆设置。");
        return false;
      }
      setSessionMutation({ phase: "assigning_memory", mode: nextMode });
      try {
        await dataSource.setMemoryMode(selectedSessionId, nextMode);
        if (!(await reload())) throw new Error("memory_mode_refresh_failed_after_update");
        setSessionMutation({ phase: "idle" });
        announce("当前对话的记忆设置已更新。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "memory", reason: errorText(error) });
        announce("记忆设置未得到系统确认。");
        return false;
      }
    },
    [announce, busy, dataSource, globalMemoryEnabled, reload, selectedSessionId]
  );

  const selectProviderProfile = useCallback(
    async (profileId: string): Promise<boolean> => {
      if (busy) return false;
      const selected = provider.profiles.find(profile => profile.profileId === profileId);
      if (!selected || selected.availability !== "ready") {
        announce("这个模型当前不可用，未改变本轮模型。");
        return false;
      }
      if (!dataSource?.selectProviderProfile) {
        announce("当前不能保存这次模型选择。");
        return false;
      }
      try {
        await dataSource.selectProviderProfile(selectedSessionId, selected.profileId);
      } catch (error) {
        announce(`模型选择未保存：${errorText(error)}`);
        return false;
      }
      setProvider(current => ({
        ...current,
        status: "ready",
        selectedProfileId: selected.profileId,
        selectedReasoningEffort: null,
        errorCode: null,
      }));
      announce(`本轮将使用 ${selected.providerId} · ${selected.modelId}。`);
      return true;
    },
    [announce, busy, dataSource, provider.profiles, selectedSessionId]
  );

  const selectReasoningEffort = useCallback(
    (effort: ReasoningEffort | null): boolean => {
      if (busy) return false;
      const selected = provider.profiles.find(
        profile => profile.profileId === provider.selectedProfileId
      );
      if (!selected || (effort !== null && !selected.supportedReasoningEfforts.includes(effort))) {
        announce("当前模型不支持这个推理强度，未改变本轮设置。");
        return false;
      }
      setProvider(current => ({ ...current, selectedReasoningEffort: effort }));
      announce(effort === null ? "本轮将使用模型默认推理。" : `本轮推理强度已设为 ${effort}。`);
      return true;
    },
    [announce, busy, provider.profiles, provider.selectedProfileId]
  );

  const setExecutionMode = useCallback(
    (nextMode: WorkExecutionMode): boolean => {
      if (busy) return false;
      setExecutionModeState(nextMode);
      announce(
        nextMode === "observe_only"
          ? "本轮 Work 已设为只读研究；不会创建文件或写入个人长期状态。"
          : "本轮 Work 已设为标准执行；范围扩展和敏感动作仍会请求确认。"
      );
      return true;
    },
    [announce, busy]
  );

  return useMemo(
    () => ({
      sessions,
      archivedSessions,
      projects,
      selectedProjectId,
      selectedSessionId,
      globalMemoryEnabled,
      memoryMode,
      messages,
      draft,
      loadStatus,
      loadError,
      turnState,
      streamingReply,
      activeTaskId,
      mode,
      executionMode,
      provider,
      workStatus,
      sessionMutation,
      pendingResources,
      pendingResourceTurnOperationId,
      resourceMutation,
      skills,
      selectedSkillId,
      selectedSkillDetail,
      toolCandidates,
      capabilityState,
      busy,
      ensureLoaded,
      reload,
      selectSession,
      startNewConversation,
      createProject,
      bindProjectDirectory,
      addProjectReadRoot,
      removeProjectReadRoot,
      updateProjectName,
      archiveProject,
      restoreProject,
      deleteProject,
      assignProject,
      setMemoryMode,
      selectProviderProfile,
      selectReasoningEffort,
      setDraft: updateDraft,
      setMode,
      setExecutionMode,
      attachResources,
      detachResource,
      selectSkill,
      sendAction,
      send,
      steer,
      cancel,
      renameSelected,
      archiveSelected,
      restoreArchived,
      deleteArchived,
      deleteSelected,
    }),
    [
      busy,
      cancel,
      capabilityState,
      bindProjectDirectory,
      addProjectReadRoot,
      archiveProject,
      archiveSelected,
      archivedSessions,
      createProject,
      deleteProject,
      deleteArchived,
      deleteSelected,
      detachResource,
      draft,
      ensureLoaded,
      attachResources,
      loadError,
      loadStatus,
      messages,
      mode,
      executionMode,
      pendingResources,
      pendingResourceTurnOperationId,
      projects,
      renameSelected,
      removeProjectReadRoot,
      restoreProject,
      restoreArchived,
      reload,
      resourceMutation,
      selectProviderProfile,
      selectReasoningEffort,
      selectSkill,
      selectSession,
      selectedProjectId,
      globalMemoryEnabled,
      memoryMode,
      setMode,
      setExecutionMode,
      selectedSessionId,
      selectedSkillId,
      sessionMutation,
      send,
      steer,
      sendAction,
      sessions,
      skills,
      startNewConversation,
      assignProject,
      setMemoryMode,
      streamingReply,
      activeTaskId,
      turnState,
      toolCandidates,
      updateDraft,
      updateProjectName,
    ]
  );
}
