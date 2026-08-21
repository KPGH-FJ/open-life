import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ProviderProfileViewModel,
  ChatSession,
  ConversationMemoryMode,
  ConversationViewModel,
  ImportedResourceReceipt,
  MainChatSkillSummary,
  MainChatLifeModelProductReceipt,
  MainChatToolCandidateList,
  MainChatTurnStatus,
  ProductAction,
  ProjectRecord,
  StreamMessageChunkPayload,
  StreamMessageStartPayload,
} from "@/tauri";
import type { ChatMessage } from "@/types";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import type { WorkspaceConversationDataSource } from "./workspaceConversationDataSource";

type Announce = (message: string) => void;

export type WorkspaceConversationLoadStatus = "idle" | "loading" | "ready" | "error";
export type WorkspaceConversationMode = "chat" | "work";
export type WorkspaceConversationProviderState = {
  status: "unknown" | "ready" | "unavailable";
  profiles: ProviderProfileViewModel[];
  selectedProfileId: string | null;
  errorCode: string | null;
};
export type WorkspaceConversationWorkStatus = "unknown" | "available" | "unavailable";
export type WorkspaceSessionMutationState =
  | { phase: "idle" }
  | { phase: "renaming"; sessionId: string }
  | { phase: "deleting"; sessionId: string }
  | { phase: "creating_project" }
  | { phase: "assigning_project"; projectId: string | null }
  | { phase: "assigning_memory"; mode: ConversationMemoryMode }
  | { phase: "failed"; action: "rename" | "delete" | "project" | "memory"; reason: string };

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
      lifeModelInfluence?: MainChatLifeModelProductReceipt;
      sourceBoundBasis?: {
        factCount: number;
        sourceTypes: string[];
        checkStatus: string;
      };
    }
  | { phase: "failed"; stage: "create" | "send" | "refresh"; reason: string };

function sessionTitle(input: string): string {
  const normalized = input.replace(/\s+/g, " ").trim();
  return normalized.length > 28 ? `${normalized.slice(0, 28)}...` : normalized;
}

function resolvedTurnStatus(status: MainChatTurnStatus | undefined): MainChatTurnStatus {
  return status ?? "failed";
}

function sourceBoundBasisFromTrace(
  generation: Record<string, unknown> | undefined
): { factCount: number; sourceTypes: string[]; checkStatus: string } | undefined {
  if (generation?.sourceBound !== true) return undefined;
  const sourceTypes = Array.isArray(generation.sourceBoundSourceTypes)
    ? generation.sourceBoundSourceTypes.filter(
        (value): value is string => typeof value === "string" && value.length > 0
      )
    : [];
  return {
    factCount:
      typeof generation.sourceBoundFactCount === "number" ? generation.sourceBoundFactCount : 0,
    sourceTypes,
    checkStatus:
      typeof generation.sourceBoundCheckStatus === "string"
        ? generation.sourceBoundCheckStatus
        : "unknown",
  };
}

function streamIdentityMatches(
  payload: {
    session_id: string;
    operation_id: string;
    conversation_id?: string;
    turn_id?: string;
    task_id?: string;
  },
  mode: WorkspaceConversationMode,
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

export type WorkspaceConversationController = {
  sessions: ChatSession[];
  projects: ProjectRecord[];
  selectedProjectId: string | null;
  selectedSessionId: string | null;
  globalMemoryEnabled: boolean;
  memoryMode: ConversationMemoryMode;
  messages: ChatMessage[];
  draft: string;
  loadStatus: WorkspaceConversationLoadStatus;
  loadError: string | null;
  turnState: WorkspaceTurnState;
  streamingReply: string;
  activeTaskId: string | null;
  mode: WorkspaceConversationMode;
  provider: WorkspaceConversationProviderState;
  workStatus: WorkspaceConversationWorkStatus;
  sessionMutation: WorkspaceSessionMutationState;
  pendingResources: ImportedResourceReceipt[];
  pendingResourceTurnOperationId: string | null;
  resourceMutation: WorkspaceResourceMutationState;
  skills: MainChatSkillSummary[];
  selectedSkillId: string | null;
  toolCandidates: MainChatToolCandidateList | null;
  capabilityState: WorkspaceCapabilityState;
  busy: boolean;
  ensureLoaded: () => void;
  reload: () => Promise<boolean>;
  selectSession: (sessionId: string) => void;
  startNewConversation: () => void;
  createProject: (name: string) => Promise<boolean>;
  assignProject: (projectId: string | null) => Promise<boolean>;
  setMemoryMode: (mode: ConversationMemoryMode) => Promise<boolean>;
  setDraft: (value: string) => void;
  setMode: (mode: WorkspaceConversationMode) => void;
  attachResources: () => Promise<boolean>;
  detachResource: (resourceId: string) => Promise<boolean>;
  selectSkill: (skillId: string | null) => Promise<boolean>;
  sendAction: (disabledReason?: string) => ProductAction;
  send: (disabledReason?: string) => Promise<void>;
  steer: () => Promise<void>;
  cancel: () => Promise<void>;
  renameSelected: (title: string) => Promise<boolean>;
  deleteSelected: () => Promise<boolean>;
};

export function useWorkspaceConversation(
  dataSource: WorkspaceConversationDataSource | undefined,
  announce: Announce,
  onAfterTurn: () => Promise<void>,
  preferredSessionId?: string | null,
  canonicalSeed?: ConversationViewModel | null
): WorkspaceConversationController {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [globalMemoryEnabled, setGlobalMemoryEnabled] = useState(true);
  const [memoryMode, setMemoryModeState] = useState<ConversationMemoryMode>("use_and_learn");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [loadStatus, setLoadStatus] = useState<WorkspaceConversationLoadStatus>("idle");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [turnState, setTurnState] = useState<WorkspaceTurnState>({ phase: "idle" });
  const [streamingReply, setStreamingReply] = useState("");
  const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
  const [mode, setModeState] = useState<WorkspaceConversationMode>("chat");
  const [provider, setProvider] = useState<WorkspaceConversationProviderState>({
    status: "unknown",
    profiles: [],
    selectedProfileId: null,
    errorCode: null,
  });
  const [workStatus, setWorkStatus] = useState<WorkspaceConversationWorkStatus>("unknown");
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
    setProvider({
      status: "unknown",
      profiles: [],
      selectedProfileId: null,
      errorCode: null,
    });
    setWorkStatus("unknown");
    setSessionMutation({ phase: "idle" });
    setPendingResources([]);
    setPendingResourceTurnOperationId(null);
    setResourceMutation({ phase: "idle" });
    setSkills([]);
    setSelectedSkillId(null);
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
    if (!canonicalSeed) return;
    requestRef.current += 1;
    setSessions(canonicalSeed.conversations);
    setProjects(canonicalSeed.projects);
    setProvider({
      status: canonicalSeed.providerStatus,
      profiles: canonicalSeed.providerProfiles,
      selectedProfileId: canonicalSeed.selectedProviderProfileId,
      errorCode: canonicalSeed.providerErrorCode,
    });
    setWorkStatus(canonicalSeed.workStatus);
    setGlobalMemoryEnabled(canonicalSeed.globalMemoryEnabled);
    // A refreshed Workbench aggregate may still project the previously
    // selected Conversation while the user has explicitly opened a new draft
    // or selected another Conversation. Refresh shared catalog/provider facts,
    // but never let that older projection steal the user's active context.
    if (loadedRef.current && explicitConversationChoiceRef.current) {
      setLoadError(null);
      setLoadStatus("ready");
      return;
    }
    setSelectedProjectId(canonicalSeed.selectedProjectId);
    setMemoryModeState(canonicalSeed.selectedMemoryMode);
    setSelectedSessionId(canonicalSeed.selectedConversationId);
    setMessages(canonicalSeed.messages);
    const selectedSession = canonicalSeed.selectedConversationId;
    const latestTurn = canonicalSeed.latestTurn;
    setTurnState(
      !selectedSession || !latestTurn || latestTurn.status === "completed"
        ? { phase: "idle" }
        : latestTurn.status === "running"
          ? { phase: "idle" }
          : {
              phase: "resolved",
              sessionId: selectedSession,
              status: latestTurn.status,
              blockers: latestTurn.errorCode ? [latestTurn.errorCode] : [],
            }
    );
    loadedRef.current = true;
    setLoadError(null);
    setLoadStatus("ready");
  }, [canonicalSeed]);

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

  const loadHistory = useCallback(
    async (sessionId: string, requestId: number): Promise<void> => {
      if (!dataSource) throw new Error("workspace_conversation_data_source_unavailable");
      const canonical = await dataSource.loadConversation(sessionId);
      if (canonical.selectedConversationId !== sessionId) {
        throw new Error("conversation_view_model_selection_mismatch");
      }
      setProjects(canonical.projects);
      setSelectedProjectId(canonical.selectedProjectId);
      setGlobalMemoryEnabled(canonical.globalMemoryEnabled);
      setMemoryModeState(canonical.selectedMemoryMode);
      const history = canonical.messages;
      if (requestId !== requestRef.current) return;
      setSelectedSessionId(sessionId);
      setMessages(history);
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
      setProjects(canonical.projects);
      setSelectedProjectId(canonical.selectedProjectId);
      setGlobalMemoryEnabled(canonical.globalMemoryEnabled);
      setMemoryModeState(canonical.selectedMemoryMode);
      setProvider({
        status: canonical.providerStatus,
        profiles: canonical.providerProfiles,
        selectedProfileId: canonical.selectedProviderProfileId,
        errorCode: canonical.providerErrorCode,
      });
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
    [announce, busy, loadHistory, mode, pendingResources.length, selectedSessionId, turnState.phase]
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
                : mode === "work" && workStatus !== "available"
                  ? "Work 的 canonical runtime 当前不可用。"
                  : mode === "chat" && provider.status === "unavailable"
                    ? "当前选择的模型不可用；请先在设置中完成模型配置。"
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
    (nextMode: WorkspaceConversationMode) => {
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
          await dataSource.createSession(sessionId, sessionTitle(text));
          if (selectedProjectId && dataSource.assignProject) {
            await dataSource.assignProject(sessionId, selectedProjectId);
          }
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
        if (operationId !== operationRef.current) return;
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

        let refreshedHistory: ChatMessage[];
        try {
          refreshedHistory = (await dataSource.loadConversation(sessionId)).messages;
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
        setActiveTaskId(null);
        setTurnState({
          phase: "resolved",
          sessionId,
          status,
          blockers: result.blockers ?? [],
          lifeModelInfluence: result.life_model_influence,
          sourceBoundBasis: sourceBoundBasisFromTrace(result.reasoning_trace?.generation_result),
        });
        await onAfterTurn();
        announce(turnAnnouncement(status));
      } catch (error) {
        if (operationId !== operationRef.current) return;
        cancelRequestRef.current += 1;
        const failureCode = errorText(error);
        if (
          streamStarted &&
          sessionId &&
          (failureCode === "canonical_work_cancelled" ||
            failureCode === "canonical_chat_turn_cancelled")
        ) {
          try {
            const refreshedHistory = (await dataSource.loadConversation(sessionId)).messages;
            if (operationId !== operationRef.current) return;
            setMessages(refreshedHistory);
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
          await onAfterTurn();
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
              ? "这项工作未完成；请在结果或需处理中核对后端记录的任务状态。"
              : "消息发送失败；当前不会显示成功结论。"
            : "新会话未能建立；没有发送消息。"
        );
        try {
          if (sessionId) {
            const refreshedHistory = (await dataSource.loadConversation(sessionId)).messages;
            if (operationId === operationRef.current) setMessages(refreshedHistory);
          }
        } catch {
          // The explicit failed state already communicates that persistence is unverified.
        }
        // A failed stream may still have durably created or terminalized a
        // canonical Work Task (for example, a blocked tool attempt). Refresh
        // the backend-owned Workspace projection on both success and failure
        // so Results / Needs Attention never disappear behind the composer
        // error state.
        if (operationId === operationRef.current) await onAfterTurn();
      }
    },
    [
      announce,
      dataSource,
      draft,
      messages,
      mode,
      provider,
      workStatus,
      onAfterTurn,
      pendingResourceTurnOperationId,
      pendingResources,
      selectedSessionId,
      selectedSkillId,
      selectedProjectId,
      sendAction,
    ]
  );

  const cancel = useCallback(async (): Promise<void> => {
    if (!dataSource || turnState.phase !== "streaming" || !turnState.turnId.trim()) {
      announce("当前没有可以取消的运行中对话。");
      return;
    }
    const { sessionId, turnId, taskId } = turnState;
    const cancelRequestId = ++cancelRequestRef.current;
    setTurnState({ phase: "cancelling", sessionId, turnId, taskId });
    announce("正在请求取消；只有后端终态返回后才会显示已取消。");
    try {
      if (mode === "work" && taskId && dataSource.cancelWorkTask) {
        await dataSource.cancelWorkTask(taskId);
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
      announce("取消请求失败；当前不会把任务显示为已取消。");
    }
  }, [announce, dataSource, mode, turnState]);

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
      const result = await dataSource.steerTask({
        steeringId: crypto.randomUUID(),
        taskId: activeTaskId,
        runId: turnState.runId,
        sessionId: selectedSessionId,
        content: text,
      });
      setDraft("");
      announce(
        result.scopeExpansionBlocked
          ? "这条调整会扩大权限范围，已记录为阻断项；当前任务不会获得新权限。"
          : "调整已加入当前任务，将在下一次安全步骤生效。"
      );
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

  const createProject = useCallback(
    async (name: string): Promise<boolean> => {
      const normalized = name.replace(/\s+/g, " ").trim();
      if (!dataSource?.createProject || !normalized || busy) {
        announce("当前不能创建 Project。");
        return false;
      }
      setSessionMutation({ phase: "creating_project" });
      try {
        const project = await dataSource.createProject(crypto.randomUUID(), normalized);
        if (selectedSessionId && dataSource.assignProject) {
          await dataSource.assignProject(selectedSessionId, project.id);
        }
        if (!(await reload())) throw new Error("project_refresh_failed_after_create");
        setSessionMutation({ phase: "idle" });
        announce(selectedSessionId ? "Project 已创建并绑定到当前对话。" : "Project 已创建。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("Project 创建或绑定未得到后端确认。");
        return false;
      }
    },
    [announce, busy, dataSource, reload, selectedSessionId]
  );

  const assignProject = useCallback(
    async (projectId: string | null): Promise<boolean> => {
      if (!dataSource?.assignProject || !selectedSessionId || busy) {
        announce("当前不能改变这段对话的 Project。");
        return false;
      }
      setSessionMutation({ phase: "assigning_project", projectId });
      try {
        await dataSource.assignProject(selectedSessionId, projectId);
        if (!(await reload())) throw new Error("project_refresh_failed_after_assign");
        setSessionMutation({ phase: "idle" });
        announce(projectId ? "当前对话已绑定到 Project。" : "当前对话已移出 Project。");
        return true;
      } catch (error) {
        setSessionMutation({ phase: "failed", action: "project", reason: errorText(error) });
        announce("Project 绑定未得到后端确认。");
        return false;
      }
    },
    [announce, busy, dataSource, reload, selectedSessionId]
  );

  const setMemoryMode = useCallback(
    async (nextMode: ConversationMemoryMode): Promise<boolean> => {
      if (!dataSource?.setMemoryMode || !selectedSessionId || busy) {
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
        announce("记忆设置未得到后端确认。");
        return false;
      }
    },
    [announce, busy, dataSource, reload, selectedSessionId]
  );

  return useMemo(
    () => ({
      sessions,
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
      provider,
      workStatus,
      sessionMutation,
      pendingResources,
      pendingResourceTurnOperationId,
      resourceMutation,
      skills,
      selectedSkillId,
      toolCandidates,
      capabilityState,
      busy,
      ensureLoaded,
      reload,
      selectSession,
      startNewConversation,
      createProject,
      assignProject,
      setMemoryMode,
      setDraft: updateDraft,
      setMode,
      attachResources,
      detachResource,
      selectSkill,
      sendAction,
      send,
      steer,
      cancel,
      renameSelected,
      deleteSelected,
    }),
    [
      busy,
      cancel,
      capabilityState,
      createProject,
      deleteSelected,
      detachResource,
      draft,
      ensureLoaded,
      attachResources,
      loadError,
      loadStatus,
      messages,
      mode,
      pendingResources,
      pendingResourceTurnOperationId,
      projects,
      renameSelected,
      reload,
      resourceMutation,
      selectSkill,
      selectSession,
      selectedProjectId,
      globalMemoryEnabled,
      memoryMode,
      setMode,
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
    ]
  );
}
