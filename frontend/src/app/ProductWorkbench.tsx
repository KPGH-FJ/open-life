import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, Bot, Monitor, Network, UserRound } from "lucide-react";
import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  ReviewItem,
  TaskViewModelItem,
  TasksViewModel,
  ViewModelEnvelope,
} from "@/tauri";
import {
  OpenLifeWorkbenchShell,
  type WorkbenchContextSummary,
  type WorkbenchEvidenceReference,
  type WorkbenchInspectorModel,
  type WorkbenchNavigationItem,
} from "@/ui/shell";
import { ResultsView } from "@/features/work/ResultsView";
import { GlobalActivityView } from "@/features/work/GlobalActivityView";
import { buildReadModelErrorEnvelope } from "@/shared/readModelEnvelope";
import { taskLifecyclePresentation } from "@/features/work/taskPresentation";
import { useConversationController } from "@/features/conversation/useConversationController";
import type { ConversationDataSource } from "@/features/conversation/conversationDataSource";
import {
  boundaryPresentation,
  collectBoundaryEvidence,
  toWorkbenchEvidence,
} from "@/shared/evidencePresentation";
import {
  activeScopedTask,
  scopedReviewItems,
  scopedTasks,
  type WorkbenchDataSource,
} from "@/app/workbenchDataSource";
import { reviewInspector, workspaceContext, workspaceInspector } from "@/app/workbenchPresentation";
import { useWorkbenchController } from "@/app/useWorkbenchController";
import { ReviewView } from "@/features/review/ReviewView";
import { WorkspaceView } from "@/features/work/WorkspaceView";
import type { PersonalIntelligenceDataSource } from "@/features/personalIntelligence/personalIntelligenceDataSource";
import {
  personalIntelligenceContext,
  personalIntelligenceInspector,
} from "@/features/personalIntelligence/personalIntelligencePresentation";
import { PersonalIntelligenceView } from "@/features/personalIntelligence/PersonalIntelligenceView";
import { usePersonalIntelligenceController } from "@/features/personalIntelligence/usePersonalIntelligenceController";
import type { SettingsDataSource } from "@/features/settings/settingsDataSource";
import type { SettingsSurfaceId } from "@/features/settings/settingsPresentation";
import { settingsContext, settingsInspector } from "@/features/settings/settingsShellModel";
import { SettingsView } from "@/features/settings/SettingsView";
import { useSettingsController } from "@/features/settings/useSettingsController";

export type PublicProductSurfaceId = "workspace" | "life-model";
export type ProductWorkbenchRouteState = {
  mode: "product" | "settings";
  surface: PublicProductSurfaceId;
};

function routeEntryAnnouncement(surface: PublicProductSurfaceId): string {
  switch (surface) {
    case "workspace":
      return "已进入 Workbench；对话、工作、结果与需处理事项共享同一上下文。";
    case "life-model":
      return "已进入个人智能；LifeModel 与 Agent Memory 分别显示各自系统已经证明的结果。";
  }
}

const productNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "workspace", label: "Workbench", meta: "对话、工作与结果", icon: Monitor },
  { id: "life-model", label: "个人智能", meta: "关于我与记忆", icon: UserRound },
];

const settingsNavigation: readonly WorkbenchNavigationItem[] = [
  {
    id: "model-provider",
    label: "模型与供应商",
    meta: "本地与云端连接",
    searchTerms: ["API 地址", "API 凭据", "连接测试", "本地模型", "Provider"],
    icon: Bot,
  },
  {
    id: "privacy-network",
    label: "隐私与网络",
    meta: "路由与传输边界",
    searchTerms: ["外部传输", "网络策略", "本地限定", "风险"],
    icon: Network,
  },
  {
    id: "diagnostics",
    label: "产品诊断",
    meta: "构建、存储与任务健康",
    searchTerms: ["版本", "构建", "存储", "canonical", "诊断", "任务统计"],
    icon: Activity,
  },
];

const unavailableCopy: Record<PublicProductSurfaceId, { title: string; reason: string }> = {
  workspace: {
    title: "工作区状态源不可用",
    reason: "系统没有提供可组合的当前任务、权限与审核状态；页面不会从历史记录推断当前执行。",
  },
  "life-model": {
    title: "个人智能状态源不可用",
    reason: "系统没有提供可用的 LifeModel 或 Agent Memory 读模型；页面不会从旧记录补造当前结论。",
  },
};

const settingsCopy: Record<string, { title: string; reason: string }> = {
  "model-provider": {
    title: "模型与供应商暂不可用",
    reason: "需要系统同时提供可编辑配置、测试结果与传输边界；页面不会用默认值代替。",
  },
  "privacy-network": {
    title: "隐私与网络暂不可用",
    reason: "需要系统提供当前传输边界；未知状态不会显示为本地或私密。",
  },
  diagnostics: {
    title: "产品诊断暂不可用",
    reason: "需要系统提供当前产品诊断；页面不会从日志或旧状态推断健康状态。",
  },
};

function isSettingsSurface(id: string): id is SettingsSurfaceId {
  return id === "model-provider" || id === "privacy-network" || id === "diagnostics";
}

function loadingBoundaryEnvelope(): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data: null,
    status: "loading",
    lastUpdatedAt: null,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

type WorkbenchTasksSnapshot = {
  envelope: ViewModelEnvelope<TasksViewModel>;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: Array<{ id: string; status: string; message?: string }>;
};

function uniqueEvidence(refs: readonly WorkbenchEvidenceReference[]): WorkbenchEvidenceReference[] {
  const seen = new Set<string>();
  return refs.filter(ref => {
    if (seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}

function diagnosticValue(
  diagnostics: ReadonlyArray<{ id: string; status: string; message?: string }>
): string {
  if (diagnostics.length === 0) return "尚未完成读取";
  return diagnostics
    .map(item => `${item.id}:${item.status}${item.message ? ` (${item.message})` : ""}`)
    .join(" | ");
}

function envelopeEvidence(refs: ReadonlyArray<EvidenceRef>): WorkbenchEvidenceReference[] {
  return refs.map(toWorkbenchEvidence);
}

function taskInspector(
  snapshot: WorkbenchTasksSnapshot,
  selectedTask: TaskViewModelItem | null,
  selectedEvidence: string
): WorkbenchInspectorModel {
  const { envelope, boundaryEnvelope, diagnostics } = snapshot;
  const taskEvidence = selectedTask
    ? [...selectedTask.evidenceRefs, ...(selectedTask.latestResultPreview?.evidenceRefs ?? [])]
    : (envelope.data?.sourceRefs ?? []);
  const evidence = uniqueEvidence([
    ...envelopeEvidence(taskEvidence),
    ...envelopeEvidence(envelope.evidenceRefs ?? []),
    ...collectBoundaryEvidence(boundaryEnvelope).map(toWorkbenchEvidence),
  ]);
  const lifecycle = selectedTask ? taskLifecyclePresentation(selectedTask) : null;
  const needsDecision = (selectedTask?.pendingReviewItemRefs.length ?? 0) > 0;
  const hasEnabledTaskControl =
    selectedTask?.allowedControls.some(
      control => control.enabled && ["resume", "retry", "stop_run"].includes(control.kind)
    ) ?? false;

  return {
    title: selectedTask ? selectedTask.title : "任务列表依据",
    conclusion: selectedTask
      ? `系统任务状态将该任务标记为“${lifecycle?.label ?? "状态未知"}”。选择任务只改变检查器上下文。`
      : envelope.status === "ready" || envelope.status === "empty"
        ? "任务列表直接来自系统任务读模型，没有与旧运行记录在前端拼接。"
        : envelope.status === "stale"
          ? "任务列表已陈旧，当前只用于核对。"
          : "系统任务读模型尚未提供可用列表。",
    risk: selectedTask
      ? needsDecision
        ? "存在等待决定的事项；任务不能因此显示为完成。"
        : selectedTask.lifecycleStatus === "completed" && !lifecycle?.verified
          ? "系统生命周期看似完成，但缺少最终交付证据，页面保持阻断态。"
          : selectedTask.lifecycleStatus === "unknown"
            ? "任务生命周期未知，不能开放恢复、重试或完成结论。"
            : hasEnabledTaskControl
              ? "可用动作来自系统；发送后仍需刷新同一任务，命令返回不代表任务完成。"
              : "系统当前没有开放可执行的任务动作。"
      : envelope.status === "stale" || envelope.status === "error"
        ? "陈旧或缺失的任务状态不能用于恢复、重试、取消或完成判断。"
        : "选择任务后，只显示系统明确允许的动作。",
    nextAction: selectedTask
      ? needsDecision
        ? "前往需处理事项查看决定上下文；查看本身不会改变审核状态。"
        : hasEnabledTaskControl
          ? "使用系统允许的动作，并等待刷新后的同一任务确认结果。"
          : "核对来源，或重新读取任务状态。"
      : envelope.status === "stale" || envelope.status === "error"
        ? "先重新读取任务状态。"
        : "选择一个任务查看它的状态来源与限制。",
    evidence,
    evidenceFeedback:
      selectedEvidence || evidence.length === 0
        ? selectedEvidence
          ? `已选择 ${selectedEvidence}。当前契约只允许识别来源，不打开或修改原始记录。`
          : "当前没有系统提供的可展示证据；页面不会补造完成证明。"
        : undefined,
    technicalDetails: [
      { label: "contract", value: "TasksViewModel" },
      { label: "envelopeStatus", value: envelope.status },
      { label: "lastUpdatedAt", value: envelope.lastUpdatedAt ?? "unknown" },
      {
        label: "boundaryBlockedReason",
        value: boundaryEnvelope.data?.blockedReason ?? "none",
      },
      { label: "taskId", value: selectedTask?.canonicalTaskId ?? "none" },
      {
        label: "providerModel",
        value: selectedTask?.latestRunProvenance
          ? `${selectedTask.latestRunProvenance.providerId}/${selectedTask.latestRunProvenance.modelId}`
          : "none",
      },
      {
        label: "projectScope",
        value: selectedTask?.latestRunProvenance?.projectScopeDigest ?? "none",
      },
      {
        label: "turnErrorCode",
        value: selectedTask?.latestRunProvenance?.turnErrorCode ?? "none",
      },
      { label: "lifecycleStatus", value: selectedTask?.lifecycleStatus ?? "none" },
      {
        label: "terminalDelivery",
        value: selectedTask?.terminalDeliveryStatus ?? "none",
      },
      {
        label: "finalEvidencePresent",
        value: selectedTask ? String(selectedTask.finalDeliveryEvidencePresent) : "none",
      },
      { label: "evidenceIds", value: evidence.map(ref => ref.id).join(", ") || "none" },
      { label: "sourceDiagnostics", value: diagnosticValue(diagnostics) },
    ],
  };
}

function unavailableInspector(title: string): WorkbenchInspectorModel {
  return {
    title,
    conclusion: "当前页面没有可用的系统契约或数据源。",
    risk: "使用示例或旧页面记录填充这里会制造未经系统确认的产品结论。",
    nextAction: "返回 Workbench；在受治理的数据源可用前保持关闭状态。",
    evidence: [],
    evidenceFeedback: "当前没有可确认的证据；页面不会补造来源。",
    technicalDetails: [{ label: "availability", value: "not_migrated" }],
  };
}

export function ProductWorkbench({
  workbenchDataSource,
  personalIntelligenceDataSource,
  settingsDataSource,
  conversationDataSource,
  initialSurface = "workspace",
  initialMode = "product",
  onRouteChange,
}: {
  workbenchDataSource?: WorkbenchDataSource;
  personalIntelligenceDataSource?: PersonalIntelligenceDataSource;
  settingsDataSource?: SettingsDataSource;
  conversationDataSource?: ConversationDataSource;
  initialSurface?: PublicProductSurfaceId;
  initialMode?: ProductWorkbenchRouteState["mode"];
  onRouteChange?: (route: ProductWorkbenchRouteState) => void;
}) {
  const [mode, setMode] = useState<"product" | "settings">(initialMode);
  const [activeSurface, setActiveSurface] = useState<PublicProductSurfaceId>(initialSurface);
  const [settingsReturnSurface, setSettingsReturnSurface] = useState<PublicProductSurfaceId>(
    initialSurface === "life-model" ? "life-model" : "workspace"
  );
  const [activeSettingsId, setActiveSettingsId] = useState<SettingsSurfaceId>("model-provider");
  const [settingsQuery, setSettingsQuery] = useState("");
  const [selectedTask, setSelectedTask] = useState<TaskViewModelItem | null>(null);
  const [explicitReviewItemId, setExplicitReviewItemId] = useState<string | null>(null);
  const [reviewOrigin, setReviewOrigin] = useState<{
    mode: "product" | "settings";
    surface: PublicProductSurfaceId;
    settingsId: SettingsSurfaceId;
  } | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [workspaceInspectorContext, setWorkspaceInspectorContext] = useState<
    "workspace" | "task" | "review"
  >("workspace");
  const [selectedEvidence, setSelectedEvidence] = useState("");
  const [focusedLifeModelItemRef, setFocusedLifeModelItemRef] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState(() => routeEntryAnnouncement(initialSurface));
  const [focusKey, setFocusKey] = useState("initial");
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const announceSettings = useCallback((message: string) => {
    if (modeRef.current === "settings") setAnnouncement(message);
  }, []);
  const refreshWorkbenchDependentsRef = useRef<() => Promise<void>>(async () => undefined);
  const refreshWorkbenchDependents = useCallback(() => refreshWorkbenchDependentsRef.current(), []);
  const workbench = useWorkbenchController(
    workbenchDataSource,
    setAnnouncement,
    refreshWorkbenchDependents
  );
  const selectedConversationIdRef = useRef<string | null>(null);
  const refreshWorkbenchAfterTurn = useCallback(
    async (completedConversationId: string) => {
      if (workbenchDataSource && selectedConversationIdRef.current === completedConversationId) {
        // A completed/blocked turn may have created a new canonical Task. The
        // prior manual Task selection must not pin Results to stale work after
        // the active Conversation has just produced a newer Task.
        setSelectedTask(null);
        await workbench.load(false, completedConversationId);
      }
    },
    [workbench.load, workbenchDataSource]
  );
  const conversation = useConversationController(
    conversationDataSource,
    setAnnouncement,
    refreshWorkbenchAfterTurn,
    null,
    workbench.stopRunningTask
  );
  refreshWorkbenchDependentsRef.current = async () => {
    if (!(await conversation.reload())) {
      throw new Error("conversation_refresh_failed_after_workbench_mutation");
    }
  };
  const personalIntelligence = usePersonalIntelligenceController(
    personalIntelligenceDataSource,
    setAnnouncement
  );
  const settingsController = useSettingsController(settingsDataSource, announceSettings);
  const focusSequenceRef = useRef(0);

  useEffect(() => {
    setMode(initialMode);
    setActiveSurface(initialSurface);
  }, [initialMode, initialSurface]);

  useEffect(() => {
    if (mode === "product" && activeSurface === "workspace" && conversationDataSource) {
      conversation.ensureLoaded();
    }
  }, [activeSurface, conversation.ensureLoaded, mode, conversationDataSource]);

  const requestFocus = useCallback((prefix: string) => {
    focusSequenceRef.current += 1;
    setFocusKey(`${prefix}:${focusSequenceRef.current}`);
  }, []);

  useEffect(() => {
    setSelectedTask(null);
    setExplicitReviewItemId(null);
    setReviewOrigin(null);
    setSelectedEvidence("");
    setInspectorOpen(false);
    setAnnouncement(routeEntryAnnouncement(initialSurface));
    if (workbenchDataSource && initialSurface === "workspace") {
      void workbench.load(false, "");
    }
    if (personalIntelligenceDataSource && initialSurface === "life-model") {
      void personalIntelligence.load(false);
    }
  }, [
    personalIntelligence.load,
    personalIntelligenceDataSource,
    workbench.load,
    workbenchDataSource,
    initialSurface,
  ]);

  useEffect(() => {
    if (initialSurface !== "workspace" || !workbenchDataSource) return;
    selectedConversationIdRef.current = conversation.selectedSessionId;
    const projectedConversationId =
      workbench.snapshot?.workspaceEnvelope.data?.selectedConversationId ?? null;
    if (
      conversation.loadStatus === "ready" &&
      projectedConversationId !== conversation.selectedSessionId
    ) {
      void workbench.load(false, conversation.selectedSessionId ?? "");
    }
  }, [
    conversation.loadStatus,
    conversation.selectedSessionId,
    workbench.load,
    workbench.snapshot?.workspaceEnvelope.data?.selectedConversationId,
    workbenchDataSource,
    initialSurface,
  ]);

  useEffect(() => {
    if (mode !== "settings" || !settingsDataSource) return;
    let cancelled = false;
    setAnnouncement("已进入设置上下文，正在核对清理后的配置与模型传输边界。 ");
    void settingsController.ensureLoaded().then(result => {
      if (cancelled) return;
      if (!result.loadedFromSource) {
        setAnnouncement(
          result.retainedUnsavedDraft
            ? "已返回设置；未保存草稿仍保留，未重新读取或覆盖。"
            : "已返回设置；沿用已读取的系统快照，未执行写入。"
        );
        return;
      }
      const next = result.snapshot;
      const projectionLoaded = next.diagnostics.some(
        diagnostic => diagnostic.id === "life_state_projection" && diagnostic.status === "loaded"
      );
      setAnnouncement(
        next.config && next.boundaryEnvelope.status !== "error" && projectionLoaded && next.safeMode
          ? next.safeMode.active
            ? "设置已从系统读取；安全模式仍在生效，测试与保存保持关闭。"
            : "设置与模型传输边界已从系统读取。"
          : "设置读取不完整；测试、保存和本地确定态保持关闭。"
      );
    });
    return () => {
      cancelled = true;
    };
  }, [mode, settingsController.ensureLoaded, settingsDataSource]);

  function navigateProduct(id: string, lifeModelItemRef?: string): void {
    if (id !== "workspace" && id !== "life-model") return;
    const next: PublicProductSurfaceId = id;
    setFocusedLifeModelItemRef(next === "life-model" ? (lifeModelItemRef ?? null) : null);
    setExplicitReviewItemId(null);
    setReviewOrigin(null);
    setMode("product");
    setActiveSurface(next);
    onRouteChange?.({ mode: "product", surface: next });
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`nav-${next}`);
    setAnnouncement(routeEntryAnnouncement(next));
    if (workbenchDataSource && next === "workspace") {
      void workbench.load(false, conversation.selectedSessionId ?? "");
    } else if (next === "life-model" && personalIntelligenceDataSource) {
      void personalIntelligence.load(false);
    } else if (next === "workspace" || next === "life-model") {
      setAnnouncement(`“${unavailableCopy[next].title}”，当前没有替代数据或重定向。`);
    }
  }

  function openSettings(): void {
    const returnSurface: PublicProductSurfaceId =
      activeSurface === "life-model" ? "life-model" : "workspace";
    setSettingsReturnSurface(returnSurface);
    setMode("settings");
    setInspectorOpen(false);
    setSelectedEvidence("");
    onRouteChange?.({ mode: "settings", surface: returnSurface });
    requestFocus("settings-open");
    if (settingsDataSource) {
      setAnnouncement("已进入设置上下文，正在核对清理后的配置与模型传输边界。 ");
    }
  }

  function backFromSettings(): void {
    setMode("product");
    setActiveSurface(settingsReturnSurface);
    setSettingsQuery("");
    setInspectorOpen(false);
    setSelectedEvidence("");
    onRouteChange?.({ mode: "product", surface: settingsReturnSurface });
    setAnnouncement("已返回之前的产品工作区，正在重新读取已生效设置。 ");
    if (settingsReturnSurface === "workspace") {
      if (conversationDataSource) {
        void conversation.reload();
      }
      if (workbenchDataSource) {
        void workbench.load(false, conversation.selectedSessionId ?? "");
      }
    } else if (personalIntelligenceDataSource) {
      void personalIntelligence.load(false);
    }
  }

  function navigateSettings(id: string): void {
    if (!isSettingsSurface(id)) return;
    setActiveSettingsId(id);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`settings-${id}`);
    if (settingsDataSource) {
      void settingsController.ensureLoaded();
      const label = settingsNavigation.find(item => item.id === id)?.label ?? id;
      setAnnouncement(`已进入“${label}”；产品事实只取自系统配置与边界读模型。`);
    }
  }

  const productBoundaryEnvelope =
    activeSurface === "life-model"
      ? personalIntelligence.snapshot?.boundaryEnvelope
      : workbench.snapshot?.boundaryEnvelope;
  const currentBoundaryEnvelope =
    mode === "settings" && settingsDataSource
      ? settingsController.effectiveBoundaryEnvelope
      : (productBoundaryEnvelope ?? loadingBoundaryEnvelope());
  const boundary = boundaryPresentation(currentBoundaryEnvelope);

  const effectiveTasksSnapshot: WorkbenchTasksSnapshot = useMemo(
    () => ({
      envelope:
        workbench.snapshot?.tasksEnvelope ??
        buildReadModelErrorEnvelope<TasksViewModel>(
          "tasks",
          "tasks_view_model.not_loaded",
          "Workbench task state has not been loaded."
        ),
      boundaryEnvelope:
        workbench.snapshot?.boundaryEnvelope ??
        buildReadModelErrorEnvelope<ProviderPrivacyBoundarySummary>(
          "provider_privacy_boundary",
          "provider_privacy_boundary.not_loaded",
          "Workspace provider boundary has not been loaded."
        ),
      diagnostics:
        workbench.snapshot?.diagnostics.filter(item => item.id === "tasks_view_model") ?? [],
    }),
    [workbench.snapshot]
  );
  const effectiveSelectedTask = useMemo(() => {
    const workspaceEnvelope = workbench.snapshot?.workspaceEnvelope;
    const workspace =
      workspaceEnvelope && ["ready", "stale"].includes(workspaceEnvelope.status)
        ? workspaceEnvelope.data
        : null;
    const projectionMatches =
      !conversationDataSource ||
      (workspace?.selectedConversationId ?? null) === conversation.selectedSessionId ||
      (workspace?.selectedConversationId === "" && conversation.selectedSessionId === null);
    const tasks = projectionMatches ? scopedTasks(workbench.snapshot) : [];
    if (selectedTask) {
      return tasks.find(item => item.canonicalTaskId === selectedTask.canonicalTaskId) ?? null;
    }
    if (conversationDataSource && conversation.selectedSessionId === null) return null;
    return (projectionMatches ? activeScopedTask(workbench.snapshot) : null) ?? tasks[0] ?? null;
  }, [
    conversation.selectedSessionId,
    workbench.snapshot?.workspaceEnvelope.data,
    selectedTask,
    conversationDataSource,
  ]);

  const context: WorkbenchContextSummary = useMemo(() => {
    if (mode === "settings") {
      if (settingsDataSource) {
        return settingsContext(settingsController, activeSettingsId);
      }
      return {
        eyebrow: "设置",
        title: settingsCopy[activeSettingsId].title,
        status: { label: "尚未迁移", status: "unknown" },
      };
    }
    if (activeSurface === "workspace" && workbenchDataSource) {
      return workspaceContext(workbench.snapshot);
    }
    if (activeSurface === "life-model" && personalIntelligenceDataSource) {
      return personalIntelligenceContext(
        personalIntelligence.snapshot,
        personalIntelligence.selectedItem
      );
    }
    return {
      eyebrow: "桌面工作台",
      title: unavailableCopy[activeSurface].title,
      status: { label: "尚未迁移", status: "unknown" },
    };
  }, [
    activeSettingsId,
    activeSurface,
    personalIntelligence.selectedItem,
    personalIntelligence.snapshot,
    personalIntelligenceDataSource,
    effectiveTasksSnapshot.envelope,
    workbench.selectedItem,
    workbench.snapshot,
    workbenchDataSource,
    mode,
    settingsController,
    settingsDataSource,
  ]);

  const inspector = useMemo(() => {
    if (mode === "settings") {
      if (settingsDataSource) {
        return settingsInspector(settingsController, activeSettingsId, selectedEvidence);
      }
      return unavailableInspector(settingsCopy[activeSettingsId].title);
    }
    if (activeSurface === "workspace" && workbenchDataSource) {
      if (workspaceInspectorContext === "review") {
        return reviewInspector(workbench.snapshot, workbench.selectedItem, selectedEvidence);
      }
      if (workspaceInspectorContext === "task" && effectiveSelectedTask) {
        return taskInspector(effectiveTasksSnapshot, effectiveSelectedTask, selectedEvidence);
      }
      return workspaceInspector(workbench.snapshot, selectedEvidence);
    }
    if (activeSurface === "life-model" && personalIntelligenceDataSource) {
      return personalIntelligenceInspector(
        personalIntelligence.snapshot,
        personalIntelligence.selectedItem,
        selectedEvidence,
        null
      );
    }
    return unavailableInspector(unavailableCopy[activeSurface].title);
  }, [
    activeSettingsId,
    activeSurface,
    personalIntelligence.selectedItem,
    personalIntelligence.snapshot,
    personalIntelligenceDataSource,
    mode,
    selectedEvidence,
    effectiveSelectedTask,
    effectiveTasksSnapshot,
    workbench.selectedItem,
    workbench.snapshot,
    workbenchDataSource,
    settingsController,
    settingsDataSource,
    workspaceInspectorContext,
  ]);

  function openInspector(): void {
    setInspectorOpen(true);
    setAnnouncement("已打开详情。 ");
  }

  function openReviewItem(item: ReviewItem): void {
    setReviewOrigin({ mode, surface: activeSurface, settingsId: activeSettingsId });
    setWorkspaceInspectorContext("review");
    setMode("product");
    setActiveSurface("workspace");
    setSelectedTask(null);
    setInspectorOpen(false);
    setSelectedEvidence("");
    const currentReviewItems =
      workbench.snapshot && ["ready", "stale"].includes(workbench.snapshot.reviewEnvelope.status)
        ? (workbench.snapshot.reviewEnvelope.data?.items ?? [])
        : [];
    if (currentReviewItems.some(candidate => candidate.id === item.id)) {
      workbench.selectReviewItem(item.id);
      setExplicitReviewItemId(item.id);
      requestFocus(`review-${item.id}`);
      setAnnouncement(`已打开“${item.decisionContext.title}”；查看没有记录任何决定。`);
      return;
    }

    setExplicitReviewItemId(null);
    setAnnouncement(`正在从系统核对“${item.decisionContext.title}”。`);
    if (!workbenchDataSource) {
      setAnnouncement("当前无法读取决定节点；没有记录任何决定。");
      return;
    }
    void workbench.load(false).then(refreshed => {
      const refreshedItem = ["ready", "stale"].includes(refreshed.reviewEnvelope.status)
        ? refreshed.reviewEnvelope.data?.items.find(candidate => candidate.id === item.id)
        : null;
      if (!refreshedItem) {
        setAnnouncement("刷新后的系统读模型没有返回这个决定节点；没有跳转到其他审核项。");
        return;
      }
      workbench.selectReviewItem(refreshedItem.id);
      setExplicitReviewItemId(refreshedItem.id);
      requestFocus(`review-${refreshedItem.id}`);
      setAnnouncement(`已打开“${refreshedItem.decisionContext.title}”；查看没有记录任何决定。`);
    });
  }

  function openEvidence(evidence: WorkbenchEvidenceReference): void {
    setSelectedEvidence(evidence.id);
    setAnnouncement(
      `已选择依据“${evidence.label}”；来源 ${evidence.source}，敏感级别 ${evidence.sensitivity}。`
    );
  }

  let content;
  if (mode === "settings") {
    if (settingsDataSource) {
      content = (
        <SettingsView
          controller={settingsController}
          surface={activeSettingsId}
          onOpenReview={openReviewItem}
          onOpenInspector={openInspector}
        />
      );
    } else {
      content = null;
    }
  } else if (activeSurface === "workspace" && workbenchDataSource) {
    const workspaceEnvelope = workbench.snapshot?.workspaceEnvelope;
    const workspace =
      workspaceEnvelope && ["ready", "stale"].includes(workspaceEnvelope.status)
        ? workspaceEnvelope.data
        : null;
    const projectionMatches =
      !conversationDataSource ||
      (workspace?.selectedConversationId ?? null) === conversation.selectedSessionId ||
      (workspace?.selectedConversationId === "" && conversation.selectedSessionId === null);
    const tasks = projectionMatches ? scopedTasks(workbench.snapshot) : [];
    const pendingWorkReviews = projectionMatches ? scopedReviewItems(workbench.snapshot) : [];
    const explicitReviewItem =
      explicitReviewItemId && workbench.selectedItem?.id === explicitReviewItemId
        ? workbench.selectedItem
        : null;
    const visibleReviews = explicitReviewItem ? [explicitReviewItem] : pendingWorkReviews;
    const selectedWorkReview = pendingWorkReviews.find(
      item => item.id === workbench.selectedItem?.id
    );
    const turnTaskId =
      conversation.turnState.phase === "resolved" ? conversation.turnState.taskId : undefined;
    const turnTask = turnTaskId
      ? tasks.find(task => task.canonicalTaskId === turnTaskId)
      : undefined;
    const recoveryControl = turnTask?.allowedControls.find(
      control => control.enabled && (control.kind === "retry" || control.kind === "resume")
    );
    const inlineCheckpoint =
      visibleReviews.length > 0 ? (
        <section className="ol-conversation-checkpoints" aria-label="当前 Work 的决定节点">
          <ReviewView
            snapshot={workbench.snapshot}
            visibleItems={visibleReviews}
            embedded
            selectedItem={explicitReviewItem ?? selectedWorkReview ?? pendingWorkReviews[0] ?? null}
            refreshing={workbench.refreshing}
            dispatchState={workbench.reviewState}
            onRefresh={() => void workbench.load(true)}
            onSelectItem={item => {
              workbench.selectReviewItem(item);
              if (explicitReviewItem) setExplicitReviewItemId(item.id);
              setWorkspaceInspectorContext("review");
              setSelectedEvidence("");
              setAnnouncement(`已选择“${item.decisionContext.title}”；没有记录任何决定。`);
            }}
            onRequestAction={action => {
              const owningTask = tasks.find(task =>
                task.pendingReviewItemRefs.some(ref => ref.id === action.targetReviewItemId)
              );
              if (owningTask) setSelectedTask(owningTask);
              workbench.requestReviewAction(action);
            }}
            onConfirmAction={workbench.confirmReviewAction}
            onCancelConfirmation={workbench.cancelReviewConfirmation}
            onEditLifeModelLearning={workbench.editLifeModelLearning}
            onBackWorkspace={() => {
              setExplicitReviewItemId(null);
              setWorkspaceInspectorContext("workspace");
              if (reviewOrigin?.mode === "settings") {
                setMode("settings");
                setActiveSurface(reviewOrigin.surface);
                setActiveSettingsId(reviewOrigin.settingsId);
                setReviewOrigin(null);
                requestFocus("review-back-settings");
                setAnnouncement("已返回打开决定节点的设置上下文。");
              } else if (reviewOrigin?.surface === "life-model") {
                navigateProduct("life-model");
              } else {
                setReviewOrigin(null);
              }
            }}
            backLabel={
              reviewOrigin?.mode === "settings"
                ? "返回设置"
                : reviewOrigin?.surface === "life-model"
                  ? "返回个人智能"
                  : "返回 Workbench"
            }
            onOpenInspector={() => {
              setWorkspaceInspectorContext("review");
              openInspector();
            }}
          />
        </section>
      ) : undefined;
    content = (
      <div
        className={`ol-conversation-workbench-layout${
          tasks.length > 0 ? " ol-conversation-workbench-layout--with-results" : ""
        }`}
        data-testid="conversation-workbench"
      >
        <GlobalActivityView
          items={workbench.snapshot?.tasksEnvelope.data?.items ?? []}
          selectedTaskId={effectiveSelectedTask?.canonicalTaskId ?? null}
          onOpenTask={task => {
            setSelectedTask(task);
            setExplicitReviewItemId(null);
            setWorkspaceInspectorContext("task");
            setSelectedEvidence("");
            if (
              conversationDataSource &&
              task.conversationId &&
              task.conversationId !== conversation.selectedSessionId
            ) {
              conversation.selectSession(task.conversationId);
            }
            setAnnouncement(`已打开全局活动“${task.title}”；正在核对它的对话与运行状态。`);
          }}
        />
        <WorkspaceView
          snapshot={workbench.snapshot}
          refreshing={workbench.refreshing}
          onRefresh={() => void workbench.load(true)}
          onOpenInspector={() => {
            setWorkspaceInspectorContext("workspace");
            openInspector();
          }}
          onOpenLifeModel={itemRef => navigateProduct("life-model", itemRef)}
          conversation={conversationDataSource ? conversation : undefined}
          inlineCheckpoint={inlineCheckpoint}
          recoveryControl={recoveryControl}
          onRequestRecovery={(control, expectedTaskId) => {
            if (turnTask) setSelectedTask(turnTask);
            workbench.requestTaskControl(control, expectedTaskId);
          }}
          onOpenProviderSettings={() => {
            openSettings();
            navigateSettings("model-provider");
          }}
        />
        {tasks.length > 0 && (
          <ResultsView
            envelope={effectiveTasksSnapshot.envelope}
            scopedItems={tasks}
            embedded
            refreshing={workbench.refreshing}
            selectedTaskId={effectiveSelectedTask?.canonicalTaskId ?? null}
            onRefresh={() => void workbench.load(true)}
            onSelectTask={task => {
              setSelectedTask(task);
              setExplicitReviewItemId(null);
              setWorkspaceInspectorContext("task");
              setSelectedEvidence("");
              setAnnouncement(`已选择 Work“${task.title}”。`);
            }}
            onOpenInspector={() => {
              setWorkspaceInspectorContext("task");
              openInspector();
            }}
            onAnnounce={setAnnouncement}
            taskControlState={workbench.taskControlState}
            onRequestTaskControl={(control, expectedTaskId) => {
              // A task can be the default Results selection without having
              // been explicitly clicked. Pin the exact command target before
              // a retry/continue refresh so another older attention item
              // cannot silently replace it in the detail pane.
              const target = tasks.find(task => task.canonicalTaskId === expectedTaskId);
              if (target) setSelectedTask(target);
              workbench.requestTaskControl(control, expectedTaskId);
            }}
            onConfirmTaskControl={workbench.confirmTaskControl}
            onCancelTaskControlConfirmation={workbench.cancelTaskControlConfirmation}
            onRequestArtifactUndo={async artifactId => {
              await workbenchDataSource.requestArtifactUndo(artifactId);
              await workbench.load(true);
              setAnnouncement("撤销请求已进入当前 Work 的决定节点；批准前不会移动文件。");
            }}
            onRequestTaskArtifactUndo={async taskId => {
              const receipt = await workbenchDataSource.requestTaskArtifactUndo(taskId);
              const failureReason = receipt.failures[0]?.reasonCode;
              const outcomeAnnouncement =
                receipt.failures.length === 0
                  ? "全部可撤销修改已进入当前 Work 的逐文件决定节点；批准前不会改动文件。"
                  : failureReason === "artifact_undo_source_changed"
                    ? `部分撤销决定已创建；${receipt.failures.length} 项文件已被修改，OpenLife 未覆盖这些新内容。`
                    : failureReason === "artifact_undo_target_conflict"
                      ? `部分撤销决定已创建；${receipt.failures.length} 项原位置已有文件，OpenLife 未覆盖现有内容。`
                      : `部分撤销决定已创建；${receipt.failures.length} 项缺少可核验的恢复依据，保持原状态。`;
              await workbench.load(true);
              setAnnouncement(outcomeAnnouncement);
            }}
            onReviseArtifact={async (taskId, artifactId, baseVersion, instruction) => {
              await workbenchDataSource.reviseArtifact(
                taskId,
                artifactId,
                baseVersion,
                instruction
              );
              setAnnouncement("聚焦修订已创建新 Run；当前版本会保留到新版本完成验证与必要审核。");
              await workbench.load(true);
            }}
            onOpenArtifact={async (artifactId, version) => {
              await workbenchDataSource.openArtifactResult(artifactId, version);
              setAnnouncement("已核验并打开文件。");
            }}
            onExportArtifact={async (artifactId, version) => {
              const savedPath = await workbenchDataSource.exportArtifactResult(artifactId, version);
              setAnnouncement(savedPath ? `已另存并核验：${savedPath}` : "已取消另存。");
            }}
          />
        )}
      </div>
    );
  } else if (activeSurface === "life-model" && personalIntelligenceDataSource) {
    content = (
      <PersonalIntelligenceView
        snapshot={personalIntelligence.snapshot}
        focusedLifeModelItemRef={focusedLifeModelItemRef}
        selectedItem={personalIntelligence.selectedItem}
        refreshing={personalIntelligence.refreshing}
        memoryAction={personalIntelligence.memoryAction}
        lifeModelAction={personalIntelligence.lifeModelAction}
        learningAction={personalIntelligence.learningAction}
        onRefresh={() => void personalIntelligence.load(true)}
        onSelectItem={item => {
          personalIntelligence.selectItem(item);
          setSelectedEvidence("");
          setAnnouncement(`已选择“${item.decisionContext.title}”；没有记录任何决定。`);
        }}
        onOpenReview={openReviewItem}
        onOpenInspector={openInspector}
        onCorrectMemory={personalIntelligence.correctMemory}
        onArchiveMemory={personalIntelligence.archiveMemory}
        onRestoreMemory={personalIntelligence.restoreMemory}
        onRollbackMemory={personalIntelligence.rollbackMemory}
        onPrivacyEraseMemory={personalIntelligence.privacyEraseMemory}
        onDraftLifeModelChange={personalIntelligence.draftLifeModelChange}
        onDraftLegacyLifeModelMigration={personalIntelligence.draftLegacyLifeModelMigration}
        onDraftLifeModelRollback={personalIntelligence.draftLifeModelRollback}
        onDraftLifeModelExport={personalIntelligence.draftLifeModelExport}
        onConfirmLearningCandidate={personalIntelligence.confirmLifeModelLearningCandidate}
        onStageLearningCandidate={personalIntelligence.stageLifeModelLearningCandidate}
        onDeleteLearningCandidate={personalIntelligence.deleteLifeModelLearningCandidate}
        onRejectLearningCandidate={personalIntelligence.rejectLifeModelLearningCandidate}
        onPauseLearningSuggestionClass={personalIntelligence.pauseLifeModelLearningSuggestionClass}
        onOpenReviewCenter={() => {
          if (personalIntelligence.selectedItem) openReviewItem(personalIntelligence.selectedItem);
          else navigateProduct("workspace");
        }}
      />
    );
  } else {
    content = null;
  }

  return (
    <OpenLifeWorkbenchShell
      mode={mode}
      activeNavigationId={activeSurface}
      navigationItems={productNavigation}
      onNavigate={navigateProduct}
      activeSettingsId={activeSettingsId}
      settingsItems={settingsNavigation}
      settingsQuery={settingsQuery}
      onSettingsQueryChange={setSettingsQuery}
      onSettingsNavigate={navigateSettings}
      onOpenSettings={openSettings}
      onBackFromSettings={backFromSettings}
      boundary={boundary}
      context={context}
      focusKey={focusKey}
      inspectorOpen={inspectorOpen}
      inspector={inspector}
      onOpenInspector={openInspector}
      onCloseInspector={() => {
        setInspectorOpen(false);
        setAnnouncement("详情已关闭，焦点返回打开按钮。 ");
      }}
      onOpenEvidence={openEvidence}
      announcement={announcement}
    >
      {content}
    </OpenLifeWorkbenchShell>
  );
}
