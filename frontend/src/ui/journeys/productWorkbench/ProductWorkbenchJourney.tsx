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
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import {
  OpenLifeWorkbenchShell,
  type WorkbenchContextSummary,
  type WorkbenchEvidenceReference,
  type WorkbenchInspectorModel,
  type WorkbenchNavigationItem,
} from "@/ui/shell";
import { WorkbenchResultsView } from "./WorkbenchResultsView";
import {
  buildReadModelErrorEnvelope,
  type ProductBoundaryDataSource,
} from "./productBoundaryDataSource";
import {
  boundaryPresentation,
  collectBoundaryEvidence,
  taskLifecyclePresentation,
  toWorkbenchEvidence,
} from "./workbenchPresentation";
import {
  reviewInspector,
  workspaceContext,
  workspaceInspector,
  ReviewGovernedView,
  useWorkspaceConversation,
  useGovernedActionJourney,
  WorkspaceGovernedView,
  type GovernedActionDataSource,
  type WorkspaceConversationDataSource,
} from "@/ui/journeys/governedAction";
import {
  durableTruthContext,
  durableTruthInspector,
  DurableTruthView,
  useDurableTruthJourney,
  type DurableTruthDataSource,
} from "@/ui/journeys/durableTruth";
import {
  settingsPrivacyContext,
  settingsPrivacyInspector,
  SettingsPrivacyView,
  useSettingsPrivacyJourney,
  type SettingsPrivacyDataSource,
  type SettingsPrivacySurfaceId,
} from "@/ui/journeys/settingsPrivacy";

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
      return "已进入个人智能；LifeModel 与 Agent Memory 分别显示各自后端已经证明的结果。";
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
    reason: "后端没有提供可组合的当前任务、权限与审核状态；页面不会从历史记录推断当前执行。",
  },
  "life-model": {
    title: "个人智能状态源不可用",
    reason: "后端没有提供可用的 LifeModel 或 Agent Memory 读模型；页面不会从旧记录补造当前结论。",
  },
};

const settingsCopy: Record<string, { title: string; reason: string }> = {
  "model-provider": {
    title: "模型与供应商暂不可用",
    reason: "需要后端同时提供可编辑配置、测试结果与传输边界；页面不会用默认值代替。",
  },
  "privacy-network": {
    title: "隐私与网络暂不可用",
    reason: "需要后端提供当前传输边界；未知状态不会显示为本地或私密。",
  },
  diagnostics: {
    title: "产品诊断暂不可用",
    reason:
      "需要后端提供 canonical 产品诊断；页面不会从旧 AgentRun、日志或兼容 store 推断健康状态。",
  },
};

function isSettingsPrivacySurface(id: string): id is SettingsPrivacySurfaceId {
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
      control =>
        control.enabled && ["resume", "retry", "cancel", "refresh_context"].includes(control.kind)
    ) ?? false;

  return {
    title: selectedTask ? selectedTask.title : "任务列表依据",
    conclusion: selectedTask
      ? `后端任务状态将该任务标记为“${lifecycle?.label ?? "状态未知"}”。选择任务只改变检查器上下文。`
      : envelope.status === "ready" || envelope.status === "empty"
        ? "任务列表直接来自后端任务读模型，没有与旧运行记录在前端拼接。"
        : envelope.status === "stale"
          ? "任务列表已陈旧，当前只用于核对。"
          : "后端任务读模型尚未提供可用列表。",
    risk: selectedTask
      ? needsDecision
        ? "存在等待决定的事项；任务不能因此显示为完成。"
        : selectedTask.lifecycleStatus === "completed" && !lifecycle?.verified
          ? "后端生命周期看似完成，但缺少最终交付证据，页面保持阻断态。"
          : selectedTask.lifecycleStatus === "unknown"
            ? "任务生命周期未知，不能开放恢复、重试或完成结论。"
            : hasEnabledTaskControl
              ? "可用动作来自后端；发送后仍需刷新同一任务，命令返回不代表任务完成。"
              : "后端当前没有开放可执行的任务动作。"
      : envelope.status === "stale" || envelope.status === "error"
        ? "陈旧或缺失的任务状态不能用于恢复、重试、取消或完成判断。"
        : "选择任务后，只显示后端明确允许的动作。",
    nextAction: selectedTask
      ? needsDecision
        ? "前往需处理事项查看决定上下文；查看本身不会改变审核状态。"
        : hasEnabledTaskControl
          ? "使用后端允许的动作，并等待刷新后的同一任务确认结果。"
          : "核对来源，或重新读取任务状态。"
      : envelope.status === "stale" || envelope.status === "error"
        ? "先重新读取任务状态。"
        : "选择一个任务查看它的状态来源与限制。",
    evidence,
    evidenceFeedback:
      selectedEvidence || evidence.length === 0
        ? selectedEvidence
          ? `已选择 ${selectedEvidence}。当前契约只允许识别来源，不打开或修改原始记录。`
          : "当前没有后端提供的可展示证据；页面不会补造完成证明。"
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
    conclusion: "当前页面没有可用的后端契约或数据源。",
    risk: "使用示例或旧页面记录填充这里会制造未经后端确认的产品结论。",
    nextAction: "返回 Workbench；在受治理的数据源可用前保持关闭状态。",
    evidence: [],
    evidenceFeedback: "当前没有可确认的证据；页面不会补造来源。",
    technicalDetails: [{ label: "availability", value: "not_migrated" }],
  };
}

export function ProductWorkbenchJourney({
  dataSource,
  governedActionDataSource,
  durableTruthDataSource,
  settingsPrivacyDataSource,
  workspaceConversationDataSource,
  initialSurface = "workspace",
  initialMode = "product",
  onRouteChange,
}: {
  dataSource: ProductBoundaryDataSource;
  governedActionDataSource?: GovernedActionDataSource;
  durableTruthDataSource?: DurableTruthDataSource;
  settingsPrivacyDataSource?: SettingsPrivacyDataSource;
  workspaceConversationDataSource?: WorkspaceConversationDataSource;
  initialSurface?: PublicProductSurfaceId;
  initialMode?: ProductWorkbenchRouteState["mode"];
  onRouteChange?: (route: ProductWorkbenchRouteState) => void;
}) {
  const [mode, setMode] = useState<"product" | "settings">(initialMode);
  const [activeSurface, setActiveSurface] = useState<PublicProductSurfaceId>(initialSurface);
  const [settingsReturnSurface, setSettingsReturnSurface] = useState<PublicProductSurfaceId>(
    initialSurface === "life-model" ? "life-model" : "workspace"
  );
  const [activeSettingsId, setActiveSettingsId] =
    useState<SettingsPrivacySurfaceId>("model-provider");
  const [settingsQuery, setSettingsQuery] = useState("");
  const [boundaryEnvelope, setBoundaryEnvelope] =
    useState<ViewModelEnvelope<ProviderPrivacyBoundarySummary>>(loadingBoundaryEnvelope);
  const [selectedTask, setSelectedTask] = useState<TaskViewModelItem | null>(null);
  const [explicitReviewItemId, setExplicitReviewItemId] = useState<string | null>(null);
  const [reviewOrigin, setReviewOrigin] = useState<{
    mode: "product" | "settings";
    surface: PublicProductSurfaceId;
    settingsId: SettingsPrivacySurfaceId;
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
  const governed = useGovernedActionJourney(governedActionDataSource, setAnnouncement);
  const selectedConversationIdRef = useRef<string | null>(null);
  const refreshGovernedAfterTurn = useCallback(async () => {
    if (governedActionDataSource) {
      await governed.load(false, selectedConversationIdRef.current ?? "");
    }
  }, [governed.load, governedActionDataSource]);
  const conversation = useWorkspaceConversation(
    workspaceConversationDataSource,
    setAnnouncement,
    refreshGovernedAfterTurn,
    null
  );
  const durable = useDurableTruthJourney(durableTruthDataSource, setAnnouncement);
  const settingsPrivacy = useSettingsPrivacyJourney(settingsPrivacyDataSource, announceSettings);
  const focusSequenceRef = useRef(0);
  const boundaryRequestRef = useRef(0);

  useEffect(() => {
    setMode(initialMode);
    setActiveSurface(initialSurface);
  }, [initialMode, initialSurface]);

  const requestFocus = useCallback((prefix: string) => {
    focusSequenceRef.current += 1;
    setFocusKey(`${prefix}:${focusSequenceRef.current}`);
  }, []);

  const loadBoundary = useCallback(
    async (announceResult: boolean) => {
      const requestId = ++boundaryRequestRef.current;
      try {
        const envelope = await dataSource.loadBoundary();
        if (requestId !== boundaryRequestRef.current) return;
        setBoundaryEnvelope(envelope);
        if (announceResult) {
          setAnnouncement(
            envelope.status === "error"
              ? "传输边界读取失败；外部动作保持关闭。"
              : "传输边界已从后端重新读取。"
          );
        }
      } catch (error) {
        if (requestId !== boundaryRequestRef.current) return;
        setBoundaryEnvelope(
          buildReadModelErrorEnvelope(
            "provider_privacy_boundary",
            "provider_privacy_boundary.load_failed",
            `Provider privacy boundary failed: ${errorText(error)}`
          )
        );
        if (announceResult) setAnnouncement("传输边界读取失败；外部动作保持关闭。");
      }
    },
    [dataSource]
  );

  useEffect(() => {
    boundaryRequestRef.current += 1;
    setBoundaryEnvelope(loadingBoundaryEnvelope());
    setSelectedTask(null);
    setExplicitReviewItemId(null);
    setReviewOrigin(null);
    setSelectedEvidence("");
    setInspectorOpen(false);
    setAnnouncement(routeEntryAnnouncement(initialSurface));
    void loadBoundary(false);
    if (governedActionDataSource && initialSurface === "workspace") {
      void governed.load(false, "");
    }
    if (durableTruthDataSource && initialSurface === "life-model") {
      void durable.load(false);
    }
    return () => {
      boundaryRequestRef.current += 1;
    };
  }, [
    dataSource,
    durable.load,
    durableTruthDataSource,
    governed.load,
    governedActionDataSource,
    initialSurface,
    loadBoundary,
  ]);

  useEffect(() => {
    if (initialSurface === "workspace" && workspaceConversationDataSource) {
      conversation.ensureLoaded();
    }
  }, [conversation.ensureLoaded, initialSurface, workspaceConversationDataSource]);

  useEffect(() => {
    if (initialSurface !== "workspace" || !governedActionDataSource) return;
    selectedConversationIdRef.current = conversation.selectedSessionId;
    if (conversation.loadStatus === "ready") {
      void governed.load(false, conversation.selectedSessionId ?? "");
    }
  }, [
    conversation.loadStatus,
    conversation.selectedSessionId,
    governed.load,
    governedActionDataSource,
    initialSurface,
  ]);

  useEffect(() => {
    if (mode !== "settings" || !settingsPrivacyDataSource) return;
    let cancelled = false;
    setAnnouncement("已进入设置上下文，正在核对清理后的配置与模型传输边界。 ");
    void settingsPrivacy.ensureLoaded().then(result => {
      if (cancelled) return;
      if (!result.loadedFromSource) {
        setAnnouncement(
          result.retainedUnsavedDraft
            ? "已返回设置；未保存草稿仍保留，未重新读取或覆盖。"
            : "已返回设置；沿用已读取的后端快照，未执行写入。"
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
            ? "设置已从后端读取；安全模式仍在生效，测试与保存保持关闭。"
            : "设置与模型传输边界已从后端读取。"
          : "设置读取不完整；测试、保存和本地确定态保持关闭。"
      );
    });
    return () => {
      cancelled = true;
    };
  }, [mode, settingsPrivacy.ensureLoaded, settingsPrivacyDataSource]);

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
    if (governedActionDataSource && next === "workspace") {
      void governed.load(false);
      if (workspaceConversationDataSource) {
        conversation.ensureLoaded();
      }
    } else if (next === "life-model" && durableTruthDataSource) {
      void durable.load(false);
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
    if (settingsPrivacyDataSource) {
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
    setAnnouncement("已返回之前的产品工作区。 ");
  }

  function navigateSettings(id: string): void {
    if (!isSettingsPrivacySurface(id)) return;
    setActiveSettingsId(id);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`settings-${id}`);
    if (settingsPrivacyDataSource) {
      void settingsPrivacy.ensureLoaded();
      const label = settingsNavigation.find(item => item.id === id)?.label ?? id;
      setAnnouncement(`已进入“${label}”；产品事实只取自后端配置与边界读模型。`);
    }
  }

  const currentBoundaryEnvelope =
    mode === "settings" && settingsPrivacyDataSource
      ? settingsPrivacy.effectiveBoundaryEnvelope
      : boundaryEnvelope;
  const boundary = boundaryPresentation(currentBoundaryEnvelope);

  const effectiveTasksSnapshot: WorkbenchTasksSnapshot = useMemo(
    () => ({
      envelope:
        governed.snapshot?.tasksEnvelope ??
        buildReadModelErrorEnvelope<TasksViewModel>(
          "tasks",
          "tasks_view_model.not_loaded",
          "Workbench task state has not been loaded."
        ),
      boundaryEnvelope,
      diagnostics:
        governed.snapshot?.diagnostics.filter(item => item.id === "tasks_view_model") ?? [],
    }),
    [boundaryEnvelope, governed.snapshot]
  );
  const effectiveSelectedTask = useMemo(() => {
    const workspaceEnvelope = governed.snapshot?.workspaceEnvelope;
    const workspace =
      workspaceEnvelope && ["ready", "stale"].includes(workspaceEnvelope.status)
        ? workspaceEnvelope.data
        : null;
    const projectionMatches =
      !workspaceConversationDataSource ||
      (workspace?.selectedConversationId ?? null) === conversation.selectedSessionId ||
      (workspace?.selectedConversationId === "" && conversation.selectedSessionId === null);
    const scopedTasks = projectionMatches ? (workspace?.tasks ?? []) : [];
    if (selectedTask) {
      return (
        scopedTasks.find(item => item.canonicalTaskId === selectedTask.canonicalTaskId) ?? null
      );
    }
    return (projectionMatches ? workspace?.activeTask : undefined) ?? scopedTasks[0] ?? null;
  }, [
    conversation.selectedSessionId,
    governed.snapshot?.workspaceEnvelope.data,
    selectedTask,
    workspaceConversationDataSource,
  ]);

  const context: WorkbenchContextSummary = useMemo(() => {
    if (mode === "settings") {
      if (settingsPrivacyDataSource) {
        return settingsPrivacyContext(settingsPrivacy, activeSettingsId);
      }
      return {
        eyebrow: "设置",
        title: settingsCopy[activeSettingsId].title,
        status: { label: "尚未迁移", status: "unknown" },
      };
    }
    if (activeSurface === "workspace" && governedActionDataSource) {
      return workspaceContext(governed.snapshot);
    }
    if (activeSurface === "life-model" && durableTruthDataSource) {
      return durableTruthContext(durable.snapshot, durable.selectedItem);
    }
    return {
      eyebrow: "桌面工作台",
      title: unavailableCopy[activeSurface].title,
      status: { label: "尚未迁移", status: "unknown" },
    };
  }, [
    activeSettingsId,
    activeSurface,
    durable.selectedItem,
    durable.snapshot,
    durableTruthDataSource,
    effectiveTasksSnapshot.envelope,
    governed.selectedItem,
    governed.snapshot,
    governedActionDataSource,
    mode,
    settingsPrivacy,
    settingsPrivacyDataSource,
  ]);

  const inspector = useMemo(() => {
    if (mode === "settings") {
      if (settingsPrivacyDataSource) {
        return settingsPrivacyInspector(settingsPrivacy, activeSettingsId, selectedEvidence);
      }
      return unavailableInspector(settingsCopy[activeSettingsId].title);
    }
    if (activeSurface === "workspace" && governedActionDataSource) {
      if (workspaceInspectorContext === "review") {
        return reviewInspector(governed.snapshot, governed.selectedItem, selectedEvidence);
      }
      if (workspaceInspectorContext === "task" && effectiveSelectedTask) {
        return taskInspector(effectiveTasksSnapshot, effectiveSelectedTask, selectedEvidence);
      }
      return workspaceInspector(governed.snapshot, selectedEvidence);
    }
    if (activeSurface === "life-model" && durableTruthDataSource) {
      return durableTruthInspector(durable.snapshot, durable.selectedItem, selectedEvidence, null);
    }
    return unavailableInspector(unavailableCopy[activeSurface].title);
  }, [
    activeSettingsId,
    activeSurface,
    durable.selectedItem,
    durable.snapshot,
    durableTruthDataSource,
    mode,
    selectedEvidence,
    effectiveSelectedTask,
    effectiveTasksSnapshot,
    governed.selectedItem,
    governed.snapshot,
    governedActionDataSource,
    settingsPrivacy,
    settingsPrivacyDataSource,
    workspaceInspectorContext,
  ]);

  function openInspector(): void {
    setInspectorOpen(true);
    setAnnouncement("已打开证据与限制检查器。 ");
  }

  function openReviewItem(item: ReviewItem): void {
    setReviewOrigin({ mode, surface: activeSurface, settingsId: activeSettingsId });
    governed.selectReviewItem(item);
    setExplicitReviewItemId(item.id);
    setWorkspaceInspectorContext("review");
    setMode("product");
    setActiveSurface("workspace");
    setSelectedTask(null);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`review-${item.id}`);
    setAnnouncement(`已打开“${item.decisionContext.title}”；查看没有记录任何决定。`);
    if (!governed.snapshot && governedActionDataSource) void governed.load(false);
  }

  function openEvidence(evidence: WorkbenchEvidenceReference): void {
    setSelectedEvidence(evidence.id);
    setAnnouncement(
      `已选择依据“${evidence.label}”；来源 ${evidence.source}，敏感级别 ${evidence.sensitivity}。`
    );
  }

  let content;
  if (mode === "settings") {
    if (settingsPrivacyDataSource) {
      content = (
        <SettingsPrivacyView
          controller={settingsPrivacy}
          surface={activeSettingsId}
          onOpenReview={openReviewItem}
          onOpenInspector={openInspector}
        />
      );
    } else {
      content = null;
    }
  } else if (activeSurface === "workspace" && governedActionDataSource) {
    const workspaceEnvelope = governed.snapshot?.workspaceEnvelope;
    const workspace =
      workspaceEnvelope && ["ready", "stale"].includes(workspaceEnvelope.status)
        ? workspaceEnvelope.data
        : null;
    const projectionMatches =
      !workspaceConversationDataSource ||
      (workspace?.selectedConversationId ?? null) === conversation.selectedSessionId ||
      (workspace?.selectedConversationId === "" && conversation.selectedSessionId === null);
    const scopedTasks = projectionMatches ? (workspace?.tasks ?? []) : [];
    const pendingWorkReviews = projectionMatches ? (workspace?.pendingReviewItems ?? []) : [];
    const explicitReviewItem =
      explicitReviewItemId && governed.selectedItem?.id === explicitReviewItemId
        ? governed.selectedItem
        : null;
    const visibleReviews = explicitReviewItem ? [explicitReviewItem] : pendingWorkReviews;
    const selectedWorkReview = pendingWorkReviews.find(
      item => item.id === governed.selectedItem?.id
    );
    content = (
      <div className="ol-conversation-workbench-layout" data-testid="conversation-workbench">
        <WorkspaceGovernedView
          snapshot={governed.snapshot}
          refreshing={governed.refreshing}
          onRefresh={() => void governed.load(true)}
          onOpenInspector={() => {
            setWorkspaceInspectorContext("workspace");
            openInspector();
          }}
          onOpenLifeModel={itemRef => navigateProduct("life-model", itemRef)}
          conversation={workspaceConversationDataSource ? conversation : undefined}
        />
        {scopedTasks.length > 0 && (
          <WorkbenchResultsView
            envelope={effectiveTasksSnapshot.envelope}
            scopedItems={scopedTasks}
            embedded
            refreshing={governed.refreshing}
            selectedTaskId={effectiveSelectedTask?.canonicalTaskId ?? null}
            onRefresh={() => void governed.load(true)}
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
            taskControlState={governed.taskControlState}
            onRequestTaskControl={governed.requestTaskControl}
            onConfirmTaskControl={governed.confirmTaskControl}
            onCancelTaskControlConfirmation={governed.cancelTaskControlConfirmation}
            onRequestArtifactUndo={async artifactId => {
              await governedActionDataSource.requestArtifactUndo(artifactId);
              setAnnouncement("撤销请求已进入当前 Work 的决定节点；批准前不会移动文件。");
              await governed.load(true);
            }}
          />
        )}
        {visibleReviews.length > 0 && (
          <section className="ol-conversation-checkpoints" aria-label="当前 Work 的决定节点">
            <ReviewGovernedView
              snapshot={governed.snapshot}
              visibleItems={visibleReviews}
              embedded
              selectedItem={
                explicitReviewItem ?? selectedWorkReview ?? pendingWorkReviews[0] ?? null
              }
              refreshing={governed.refreshing}
              dispatchState={governed.reviewState}
              onRefresh={() => void governed.load(true)}
              onSelectItem={item => {
                governed.selectReviewItem(item);
                if (explicitReviewItem) setExplicitReviewItemId(item.id);
                setWorkspaceInspectorContext("review");
                setSelectedEvidence("");
                setAnnouncement(`已选择“${item.decisionContext.title}”；没有记录任何决定。`);
              }}
              onRequestAction={governed.requestReviewAction}
              onConfirmAction={governed.confirmReviewAction}
              onCancelConfirmation={governed.cancelReviewConfirmation}
              onEditLifeModelLearning={governed.editLifeModelLearning}
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
        )}
      </div>
    );
  } else if (activeSurface === "life-model" && durableTruthDataSource) {
    content = (
      <DurableTruthView
        snapshot={durable.snapshot}
        focusedLifeModelItemRef={focusedLifeModelItemRef}
        selectedItem={durable.selectedItem}
        refreshing={durable.refreshing}
        memoryAction={durable.memoryAction}
        migrationAction={durable.migrationAction}
        lifeModelAction={durable.lifeModelAction}
        learningAction={durable.learningAction}
        onRefresh={() => void durable.load(true)}
        onSelectItem={item => {
          durable.selectItem(item);
          setSelectedEvidence("");
          setAnnouncement(`已选择“${item.decisionContext.title}”；没有记录任何决定。`);
        }}
        onOpenReview={openReviewItem}
        onOpenInspector={openInspector}
        onCorrectMemory={durable.correctMemory}
        onArchiveMemory={durable.archiveMemory}
        onStopRecall={durable.stopRecall}
        onRestoreMemory={durable.restoreMemory}
        onRollbackMemory={durable.rollbackMemory}
        onPrivacyEraseMemory={durable.privacyEraseMemory}
        onDraftLegacyMigration={durable.draftLegacyMigration}
        onDraftLifeModelChange={durable.draftLifeModelChange}
        onDraftLifeModelRollback={durable.draftLifeModelRollback}
        onDraftLifeModelExport={durable.draftLifeModelExport}
        onConfirmLearningCandidate={durable.confirmLifeModelLearningCandidate}
        onStageLearningCandidate={durable.stageLifeModelLearningCandidate}
        onDeleteLearningCandidate={durable.deleteLifeModelLearningCandidate}
        onRejectLearningCandidate={durable.rejectLifeModelLearningCandidate}
        onPauseLearningSuggestionClass={durable.pauseLifeModelLearningSuggestionClass}
        onOpenReviewCenter={() => {
          if (durable.selectedItem) openReviewItem(durable.selectedItem);
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
        setAnnouncement("证据检查器已关闭，焦点返回打开按钮。 ");
      }}
      onOpenEvidence={openEvidence}
      announcement={announcement}
    >
      {content}
    </OpenLifeWorkbenchShell>
  );
}
