import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Bot,
  Brain,
  CalendarDays,
  HardDrive,
  KeyRound,
  LifeBuoy,
  ListTodo,
  Monitor,
  Network,
  Palette,
  ShieldCheck,
  UserRound,
} from "lucide-react";
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
import { buildTodayViewModelEnvelope } from "@/viewmodels/today/todayViewModelAdapter";
import { TasksReadOnlyView } from "./TasksReadOnlyView";
import { TodayReadOnlyView } from "./TodayReadOnlyView";
import { UnavailableReadOnlyView } from "./UnavailableReadOnlyView";
import {
  buildReadModelErrorEnvelope,
  type ReadOnlySpineDataSource,
  type ReadSourceDiagnostic,
  type TasksReadOnlySnapshot,
  type TodayReadOnlySnapshot,
} from "./readOnlySpineDataSource";
import {
  boundaryPresentation,
  collectBoundaryEvidence,
  taskLifecyclePresentation,
  tasksContext,
  todayContext,
  toWorkbenchEvidence,
} from "./readOnlySpinePresentation";
import {
  governedBoundaryEnvelope,
  reviewContext,
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
  useLifeModelBuilder,
  useDurableTruthJourney,
  type DurableTruthDataSource,
  type LifeModelBuilderDataSource,
} from "@/ui/journeys/durableTruth";
import {
  settingsPrivacyContext,
  settingsPrivacyInspector,
  SettingsPrivacyView,
  useSettingsPrivacyJourney,
  type SettingsPrivacyDataSource,
  type SettingsPrivacySurfaceId,
} from "@/ui/journeys/settingsPrivacy";

export type ReadOnlyProductSurfaceId = "today" | "workspace" | "tasks" | "review" | "life-model";
export type ReadOnlySpineRouteState = {
  mode: "product" | "settings";
  surface: ReadOnlyProductSurfaceId;
};

function routeEntryAnnouncement(surface: ReadOnlyProductSurfaceId): string {
  switch (surface) {
    case "today":
      return "已进入今日；当前关注只取自后端读模型。";
    case "workspace":
      return "已进入工作区；当前执行与阻塞只取自后端读模型。";
    case "tasks":
      return "已进入任务；任务状态与交付证明只取自后端读模型。";
    case "review":
      return "已进入审核中心；决定状态与后续应用结果分别核对。";
    case "life-model":
      return "已进入个人智能；LifeModel 与 Agent Memory 分别显示各自后端已经证明的结果。";
  }
}

const productNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "today", label: "今日", meta: "当前关注", icon: CalendarDays },
  { id: "workspace", label: "工作区", meta: "当前执行", icon: Monitor },
  { id: "tasks", label: "任务", meta: "队列与连续性", icon: ListTodo },
  { id: "review", label: "审核中心", meta: "建议与权限决定", icon: ShieldCheck },
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
    id: "tools-permissions",
    label: "工具与权限",
    meta: "能力与授权记录",
    searchTerms: ["工具", "权限", "授权", "MCP"],
    icon: KeyRound,
  },
  {
    id: "data-recovery",
    label: "数据与恢复",
    meta: "导入、导出与保留",
    searchTerms: ["本地数据", "备份", "导入", "导出", "删除"],
    icon: HardDrive,
  },
  {
    id: "life-memory",
    label: "LifeModel 与记忆",
    meta: "长期状态治理",
    searchTerms: ["Memory", "长期状态", "应用", "回滚"],
    icon: Brain,
  },
  {
    id: "appearance",
    label: "外观",
    meta: "界面显示",
    searchTerms: ["主题", "字体", "密度"],
    icon: Palette,
  },
  {
    id: "advanced-support",
    label: "高级与支持",
    meta: "诊断与版本信息",
    searchTerms: ["调试", "日志", "版本", "支持"],
    icon: LifeBuoy,
  },
];

const unavailableCopy: Record<
  Exclude<ReadOnlyProductSurfaceId, "today" | "tasks">,
  { title: string; reason: string }
> = {
  workspace: {
    title: "工作区状态源不可用",
    reason: "后端没有提供可组合的当前任务、权限与审核状态；页面不会从历史记录推断当前执行。",
  },
  review: {
    title: "审核状态源不可用",
    reason: "后端没有提供可确认的待决定项；页面不会用旧建议列表代替，也不会把查看解释成批准。",
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
  "tools-permissions": {
    title: "工具与权限暂不可用",
    reason: "当前没有可确认的工具权限状态；页面不会从工具清单和历史记录拼装授权结论。",
  },
  "data-recovery": {
    title: "数据与恢复暂不可用",
    reason: "导入、导出、保留和删除都可能改变持久状态，需要独立契约与危险动作确认。",
  },
  "life-memory": {
    title: "LifeModel 与记忆设置暂不可用",
    reason: "长期状态仍由 LifeModel 产品区提供；设置不会建立第二套长期状态来源。",
  },
  appearance: {
    title: "外观设置暂不可用",
    reason: "当前还没有可保存的外观偏好；页面不会提供没有结果的样式控件。",
  },
  "advanced-support": {
    title: "高级与支持暂不可用",
    reason: "当前没有可确认的诊断与支持状态；高级信息不会替代产品状态。",
  },
};

function isSettingsPrivacySurface(id: string): id is SettingsPrivacySurfaceId {
  return id === "model-provider" || id === "privacy-network";
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

function loadingTodaySnapshot(): TodayReadOnlySnapshot {
  return {
    envelope: buildTodayViewModelEnvelope({ projection: null, status: "loading" }),
    boundaryEnvelope: loadingBoundaryEnvelope(),
    diagnostics: [],
  };
}

function loadingTasksSnapshot(): TasksReadOnlySnapshot {
  return {
    envelope: {
      data: null,
      status: "loading",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    },
    boundaryEnvelope: loadingBoundaryEnvelope(),
    diagnostics: [],
  };
}

function rejectedTodaySnapshot(error: unknown): TodayReadOnlySnapshot {
  const message = errorText(error);
  return {
    envelope: buildTodayViewModelEnvelope({
      projection: null,
      status: "error",
      errorMessage: `Today read model failed: ${message}`,
    }),
    boundaryEnvelope: buildReadModelErrorEnvelope(
      "provider_privacy_boundary",
      "provider_privacy_boundary.not_observed",
      "Provider/privacy boundary was not observed because the Today request failed."
    ),
    diagnostics: [{ id: "life_state_projection", status: "failed", message }],
  };
}

function rejectedTasksSnapshot(error: unknown): TasksReadOnlySnapshot {
  const message = errorText(error);
  return {
    envelope: buildReadModelErrorEnvelope<TasksViewModel>(
      "tasks",
      "tasks_view_model.load_failed",
      `TasksViewModel failed: ${message}`
    ),
    boundaryEnvelope: buildReadModelErrorEnvelope(
      "provider_privacy_boundary",
      "provider_privacy_boundary.not_observed",
      "Provider/privacy boundary was not observed because the Tasks request failed."
    ),
    diagnostics: [{ id: "tasks_view_model", status: "failed", message }],
  };
}

function uniqueEvidence(refs: readonly WorkbenchEvidenceReference[]): WorkbenchEvidenceReference[] {
  const seen = new Set<string>();
  return refs.filter(ref => {
    if (seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}

function diagnosticValue(diagnostics: readonly ReadSourceDiagnostic[]): string {
  if (diagnostics.length === 0) return "尚未完成读取";
  return diagnostics
    .map(item => `${item.id}:${item.status}${item.message ? ` (${item.message})` : ""}`)
    .join(" | ");
}

function envelopeEvidence(refs: ReadonlyArray<EvidenceRef>): WorkbenchEvidenceReference[] {
  return refs.map(toWorkbenchEvidence);
}

function todayInspector(
  snapshot: TodayReadOnlySnapshot,
  selectedEvidence: string
): WorkbenchInspectorModel {
  const { envelope, boundaryEnvelope, diagnostics } = snapshot;
  const evidence = uniqueEvidence([
    ...envelopeEvidence(envelope.evidenceRefs ?? []),
    ...envelopeEvidence(envelope.data?.sourceRefs ?? []),
    ...collectBoundaryEvidence(boundaryEnvelope).map(toWorkbenchEvidence),
  ]);
  const boundary = boundaryPresentation(boundaryEnvelope);
  const hasPendingReview = (envelope.data?.pendingReviewCount ?? 0) > 0;

  return {
    title: "今日状态依据",
    conclusion:
      envelope.status === "ready" || envelope.status === "empty"
        ? "今日重点由 LifeStateProjection 与每日目标兼容投影组合而成；本页没有生成额外产品事实。"
        : envelope.status === "stale"
          ? "今日数据已陈旧，当前只保留查看能力。"
          : envelope.status === "error"
            ? "Today 读模型未成功建立，当前没有可用的产品结论。"
            : "Today 读模型仍在读取。",
    risk:
      envelope.status === "stale" || envelope.status === "error"
        ? "旧数据或缺失数据不能授权任务、外部动作或长期写入。"
        : hasPendingReview
          ? "存在等待决定的建议；未决定、已批准与已应用必须保持分离。"
          : `${boundary.label}。${boundary.detail}`,
    nextAction:
      envelope.status === "stale" || envelope.status === "error"
        ? "先重新读取；刷新成功前不要依据旧状态执行。"
        : hasPendingReview
          ? "前往审核中心查看决定上下文；打开审核项本身不会记录批准或拒绝。"
          : "继续当前重点，必要时查看具体来源。",
    evidence,
    evidenceFeedback:
      selectedEvidence || evidence.length === 0
        ? selectedEvidence
          ? `已选择 ${selectedEvidence}。当前契约只允许识别来源，不打开或修改原始记录。`
          : "当前没有后端提供的可展示证据；页面保持未知，不补造来源。"
        : undefined,
    technicalDetails: [
      { label: "contract", value: "openlife.today-adapter.v1" },
      { label: "envelopeStatus", value: envelope.status },
      { label: "lastUpdatedAt", value: envelope.lastUpdatedAt ?? "unknown" },
      { label: "boundaryStatus", value: boundaryEnvelope.status },
      {
        label: "boundaryBlockedReason",
        value: boundaryEnvelope.data?.blockedReason ?? "none",
      },
      {
        label: "safeModeReason",
        value: envelope.data?.safeMode.reason ?? "none",
      },
      { label: "evidenceIds", value: evidence.map(ref => ref.id).join(", ") || "none" },
      { label: "sourceDiagnostics", value: diagnosticValue(diagnostics) },
    ],
  };
}

function taskInspector(
  snapshot: TasksReadOnlySnapshot,
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
        ? "前往审核中心查看决定上下文；查看本身不会改变审核状态。"
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
    nextAction: "返回今日或任务；在受治理的数据源可用前保持关闭状态。",
    evidence: [],
    evidenceFeedback: "当前没有可确认的证据；页面不会补造来源。",
    technicalDetails: [{ label: "availability", value: "not_migrated" }],
  };
}

export function ReadOnlySpineJourney({
  dataSource,
  governedActionDataSource,
  durableTruthDataSource,
  settingsPrivacyDataSource,
  workspaceConversationDataSource,
  lifeModelBuilderDataSource,
  initialSurface = "today",
  initialMode = "product",
  onRouteChange,
}: {
  dataSource: ReadOnlySpineDataSource;
  governedActionDataSource?: GovernedActionDataSource;
  durableTruthDataSource?: DurableTruthDataSource;
  settingsPrivacyDataSource?: SettingsPrivacyDataSource;
  workspaceConversationDataSource?: WorkspaceConversationDataSource;
  lifeModelBuilderDataSource?: LifeModelBuilderDataSource;
  initialSurface?: ReadOnlyProductSurfaceId;
  initialMode?: ReadOnlySpineRouteState["mode"];
  onRouteChange?: (route: ReadOnlySpineRouteState) => void;
}) {
  const [mode, setMode] = useState<"product" | "settings">(initialMode);
  const [activeSurface, setActiveSurface] = useState<ReadOnlyProductSurfaceId>(initialSurface);
  const [settingsReturnSurface, setSettingsReturnSurface] =
    useState<ReadOnlyProductSurfaceId>(initialSurface);
  const [activeSettingsId, setActiveSettingsId] = useState<SettingsPrivacySurfaceId | string>(
    "model-provider"
  );
  const [reviewReturnSurface, setReviewReturnSurface] = useState<
    "workspace" | "life-model" | "settings"
  >("workspace");
  const [settingsQuery, setSettingsQuery] = useState("");
  const [todaySnapshot, setTodaySnapshot] = useState<TodayReadOnlySnapshot>(loadingTodaySnapshot);
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksReadOnlySnapshot>(loadingTasksSnapshot);
  const [todayRefreshing, setTodayRefreshing] = useState(false);
  const [tasksRefreshing, setTasksRefreshing] = useState(false);
  const [tasksLoaded, setTasksLoaded] = useState(false);
  const [selectedTask, setSelectedTask] = useState<TaskViewModelItem | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [selectedEvidence, setSelectedEvidence] = useState("");
  const [announcement, setAnnouncement] = useState(() => routeEntryAnnouncement(initialSurface));
  const [focusKey, setFocusKey] = useState("initial");
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const announceSettings = useCallback((message: string) => {
    if (modeRef.current === "settings") setAnnouncement(message);
  }, []);
  const governed = useGovernedActionJourney(governedActionDataSource, setAnnouncement);
  const refreshGovernedAfterTurn = useCallback(async () => {
    if (governedActionDataSource) await governed.load(false);
  }, [governed.load, governedActionDataSource]);
  const preferredWorkspaceConversationId =
    governed.snapshot?.workspaceEnvelope.data?.activeTask?.conversationId ?? null;
  const conversation = useWorkspaceConversation(
    workspaceConversationDataSource,
    setAnnouncement,
    refreshGovernedAfterTurn,
    preferredWorkspaceConversationId
  );
  const durable = useDurableTruthJourney(durableTruthDataSource, setAnnouncement);
  const lifeModelBuilder = useLifeModelBuilder(lifeModelBuilderDataSource, setAnnouncement);
  const settingsPrivacy = useSettingsPrivacyJourney(settingsPrivacyDataSource, announceSettings);
  const focusSequenceRef = useRef(0);
  const todayRequestRef = useRef(0);
  const tasksRequestRef = useRef(0);

  useEffect(() => {
    setMode(initialMode);
    setActiveSurface(initialSurface);
  }, [initialMode, initialSurface]);

  const requestFocus = useCallback((prefix: string) => {
    focusSequenceRef.current += 1;
    setFocusKey(`${prefix}:${focusSequenceRef.current}`);
  }, []);

  const loadToday = useCallback(
    async (announceResult: boolean) => {
      const requestId = ++todayRequestRef.current;
      setTodayRefreshing(true);
      try {
        const snapshot = await dataSource.loadToday();
        if (requestId !== todayRequestRef.current) return;
        setTodaySnapshot(snapshot);
        if (announceResult) {
          setAnnouncement(
            snapshot.envelope.status === "error"
              ? "今日状态读取失败，页面保持关闭式状态。"
              : `今日状态已更新，当前为${snapshot.envelope.status}。`
          );
        }
      } catch (error) {
        if (requestId !== todayRequestRef.current) return;
        setTodaySnapshot(rejectedTodaySnapshot(error));
        if (announceResult) setAnnouncement("今日状态读取失败，页面保持关闭式状态。");
      } finally {
        if (requestId === todayRequestRef.current) setTodayRefreshing(false);
      }
    },
    [dataSource]
  );

  const loadTasks = useCallback(
    async (announceResult: boolean) => {
      const requestId = ++tasksRequestRef.current;
      setTasksRefreshing(true);
      try {
        const snapshot = await dataSource.loadTasks();
        if (requestId !== tasksRequestRef.current) return;
        setTasksSnapshot(snapshot);
        setTasksLoaded(true);
        setSelectedTask(current => {
          if (!current) return null;
          return (
            snapshot.envelope.data?.items.find(
              item => item.canonicalTaskId === current.canonicalTaskId
            ) ?? null
          );
        });
        if (announceResult) {
          setAnnouncement(
            snapshot.envelope.status === "error"
              ? "任务状态读取失败，页面保持关闭式状态。"
              : `任务状态已更新，当前为${snapshot.envelope.status}。`
          );
        }
      } catch (error) {
        if (requestId !== tasksRequestRef.current) return;
        setTasksSnapshot(rejectedTasksSnapshot(error));
        setTasksLoaded(true);
        if (announceResult) setAnnouncement("任务状态读取失败，页面保持关闭式状态。");
      } finally {
        if (requestId === tasksRequestRef.current) setTasksRefreshing(false);
      }
    },
    [dataSource]
  );

  useEffect(() => {
    todayRequestRef.current += 1;
    tasksRequestRef.current += 1;
    setTodaySnapshot(loadingTodaySnapshot());
    setTasksSnapshot(loadingTasksSnapshot());
    setTasksLoaded(false);
    setSelectedTask(null);
    setSelectedEvidence("");
    setInspectorOpen(false);
    setAnnouncement(routeEntryAnnouncement(initialSurface));
    void loadToday(false);
    if (initialSurface === "tasks") {
      if (governedActionDataSource) void governed.load(false);
      else void loadTasks(false);
    }
    if (
      governedActionDataSource &&
      (initialSurface === "workspace" || initialSurface === "review")
    ) {
      void governed.load(false);
    }
    if (durableTruthDataSource && initialSurface === "life-model") {
      void durable.load(false);
    }
    return () => {
      todayRequestRef.current += 1;
      tasksRequestRef.current += 1;
    };
  }, [
    dataSource,
    durable.load,
    durableTruthDataSource,
    governed.load,
    governedActionDataSource,
    initialSurface,
    loadTasks,
    loadToday,
  ]);

  useEffect(() => {
    if (initialSurface === "workspace" && workspaceConversationDataSource) {
      conversation.ensureLoaded();
    }
  }, [conversation.ensureLoaded, initialSurface, workspaceConversationDataSource]);

  useEffect(() => {
    if (initialSurface === "life-model" && lifeModelBuilderDataSource) {
      lifeModelBuilder.ensureLoaded();
    }
  }, [initialSurface, lifeModelBuilder.ensureLoaded, lifeModelBuilderDataSource]);

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

  function navigateProduct(id: string, reviewOrigin?: "workspace" | "life-model"): void {
    const next = id as ReadOnlyProductSurfaceId;
    if (next === "review") {
      setReviewReturnSurface(
        reviewOrigin ?? (activeSurface === "life-model" ? "life-model" : "workspace")
      );
    }
    setMode("product");
    setActiveSurface(next);
    onRouteChange?.({ mode: "product", surface: next });
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`nav-${next}`);
    setAnnouncement(routeEntryAnnouncement(next));
    if (next === "tasks" && !governed.snapshot) {
      if (governedActionDataSource) void governed.load(false);
      else if (!tasksLoaded) void loadTasks(false);
    }
    if (next === "today" || next === "tasks") {
      return;
    }
    if (governedActionDataSource && (next === "workspace" || next === "review")) {
      void governed.load(false);
      if (next === "workspace" && workspaceConversationDataSource) {
        conversation.ensureLoaded();
      }
    } else if (next === "life-model" && durableTruthDataSource) {
      void durable.load(false);
      if (lifeModelBuilderDataSource) lifeModelBuilder.ensureLoaded();
    } else {
      setAnnouncement(`“${unavailableCopy[next].title}”，当前没有替代数据或重定向。`);
    }
  }

  function openSettings(): void {
    setSettingsReturnSurface(activeSurface);
    setMode("settings");
    setInspectorOpen(false);
    setSelectedEvidence("");
    onRouteChange?.({ mode: "settings", surface: activeSurface });
    requestFocus("settings-open");
    if (settingsPrivacyDataSource && isSettingsPrivacySurface(activeSettingsId)) {
      setAnnouncement("已进入设置上下文，正在核对清理后的配置与模型传输边界。 ");
    } else {
      setAnnouncement(`已进入“${settingsCopy[activeSettingsId].title}”；当前入口尚未迁移。`);
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
    setActiveSettingsId(id);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`settings-${id}`);
    if (settingsPrivacyDataSource && isSettingsPrivacySurface(id)) {
      void settingsPrivacy.ensureLoaded();
      setAnnouncement(`已进入“${settingsCopy[id].title}”；产品事实只取自后端配置与边界读模型。`);
    } else {
      setAnnouncement(`已进入“${settingsCopy[id].title}”；当前不会读取或保存替代配置。`);
    }
  }

  const currentBoundaryEnvelope =
    mode === "settings" && settingsPrivacyDataSource && isSettingsPrivacySurface(activeSettingsId)
      ? settingsPrivacy.effectiveBoundaryEnvelope
      : governedActionDataSource && (activeSurface === "workspace" || activeSurface === "review")
        ? governedBoundaryEnvelope(governed.snapshot)
        : activeSurface === "tasks"
          ? governed.snapshot
            ? governedBoundaryEnvelope(governed.snapshot)
            : tasksSnapshot.boundaryEnvelope
          : todaySnapshot.boundaryEnvelope;
  const boundary = boundaryPresentation(currentBoundaryEnvelope);

  const effectiveTasksSnapshot: TasksReadOnlySnapshot = useMemo(
    () =>
      governedActionDataSource && governed.snapshot
        ? {
            envelope: governed.snapshot.tasksEnvelope,
            boundaryEnvelope: governedBoundaryEnvelope(governed.snapshot),
            diagnostics: governed.snapshot.diagnostics
              .filter(item => item.id === "tasks_view_model")
              .map(item => ({ ...item, id: "tasks_view_model" as const })),
          }
        : tasksSnapshot,
    [governed.snapshot, governedActionDataSource, tasksSnapshot]
  );
  const effectiveSelectedTask = useMemo(
    () =>
      selectedTask
        ? (effectiveTasksSnapshot.envelope.data?.items.find(
            item => item.canonicalTaskId === selectedTask.canonicalTaskId
          ) ?? null)
        : null,
    [effectiveTasksSnapshot.envelope.data?.items, selectedTask]
  );

  const context: WorkbenchContextSummary = useMemo(() => {
    if (mode === "settings") {
      if (settingsPrivacyDataSource && isSettingsPrivacySurface(activeSettingsId)) {
        return settingsPrivacyContext(settingsPrivacy, activeSettingsId);
      }
      return {
        eyebrow: "设置",
        title: settingsCopy[activeSettingsId].title,
        status: { label: "尚未迁移", status: "unknown" },
      };
    }
    if (activeSurface === "today") return todayContext(todaySnapshot.envelope);
    if (activeSurface === "tasks") return tasksContext(effectiveTasksSnapshot.envelope);
    if (activeSurface === "workspace" && governedActionDataSource) {
      return workspaceContext(governed.snapshot);
    }
    if (activeSurface === "review" && governedActionDataSource) {
      return reviewContext(governed.snapshot, governed.selectedItem);
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
    todaySnapshot.envelope,
  ]);

  const inspector = useMemo(() => {
    if (mode === "settings") {
      if (settingsPrivacyDataSource && isSettingsPrivacySurface(activeSettingsId)) {
        return settingsPrivacyInspector(settingsPrivacy, activeSettingsId, selectedEvidence);
      }
      return unavailableInspector(settingsCopy[activeSettingsId].title);
    }
    if (activeSurface === "today") return todayInspector(todaySnapshot, selectedEvidence);
    if (activeSurface === "tasks") {
      return taskInspector(effectiveTasksSnapshot, effectiveSelectedTask, selectedEvidence);
    }
    if (activeSurface === "workspace" && governedActionDataSource) {
      return workspaceInspector(governed.snapshot, selectedEvidence);
    }
    if (activeSurface === "review" && governedActionDataSource) {
      return reviewInspector(governed.snapshot, governed.selectedItem, selectedEvidence);
    }
    if (activeSurface === "life-model" && durableTruthDataSource) {
      return durableTruthInspector(
        durable.snapshot,
        durable.selectedItem,
        selectedEvidence,
        lifeModelBuilderDataSource ? lifeModelBuilder.error : null
      );
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
    lifeModelBuilder.error,
    lifeModelBuilderDataSource,
    settingsPrivacy,
    settingsPrivacyDataSource,
    todaySnapshot,
  ]);

  function openInspector(): void {
    setInspectorOpen(true);
    setAnnouncement("已打开证据与限制检查器。 ");
  }

  function selectTask(task: TaskViewModelItem): void {
    setSelectedTask(task);
    setSelectedEvidence("");
    setInspectorOpen(true);
    setAnnouncement(`已选择任务“${task.title}”，并打开状态依据。`);
  }

  function openReviewItem(item: ReviewItem): void {
    setReviewReturnSurface(
      mode === "settings" ? "settings" : activeSurface === "life-model" ? "life-model" : "workspace"
    );
    governed.selectReviewItem(item);
    setMode("product");
    setActiveSurface("review");
    onRouteChange?.({ mode: "product", surface: "review" });
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
    if (settingsPrivacyDataSource && isSettingsPrivacySurface(activeSettingsId)) {
      content = (
        <SettingsPrivacyView
          controller={settingsPrivacy}
          surface={activeSettingsId}
          onOpenReview={openReviewItem}
          onOpenInspector={openInspector}
        />
      );
    } else {
      const copy = settingsCopy[activeSettingsId];
      content = (
        <UnavailableReadOnlyView
          title={copy.title}
          reason={copy.reason}
          onToday={() => navigateProduct("today")}
          onTasks={() => navigateProduct("tasks")}
        />
      );
    }
  } else if (activeSurface === "today") {
    content = (
      <TodayReadOnlyView
        envelope={todaySnapshot.envelope}
        refreshing={todayRefreshing}
        onRefresh={() => void loadToday(true)}
        onNavigate={navigateProduct}
        onOpenInspector={openInspector}
      />
    );
  } else if (activeSurface === "tasks") {
    content = (
      <TasksReadOnlyView
        envelope={effectiveTasksSnapshot.envelope}
        refreshing={tasksRefreshing || governed.refreshing}
        selectedTaskId={effectiveSelectedTask?.canonicalTaskId ?? null}
        onRefresh={() =>
          void (governedActionDataSource && governed.snapshot
            ? governed.load(true)
            : loadTasks(true))
        }
        onSelectTask={selectTask}
        onOpenInspector={openInspector}
        onAnnounce={setAnnouncement}
        taskControlState={governed.taskControlState}
        onRequestTaskControl={governed.requestTaskControl}
        onConfirmTaskControl={governed.confirmTaskControl}
        onCancelTaskControlConfirmation={governed.cancelTaskControlConfirmation}
      />
    );
  } else if (activeSurface === "workspace" && governedActionDataSource) {
    content = (
      <WorkspaceGovernedView
        snapshot={governed.snapshot}
        refreshing={governed.refreshing}
        resumeState={governed.resumeState}
        onRefresh={() => void governed.load(true)}
        onOpenReview={openReviewItem}
        onResume={governed.requestResume}
        onConfirmResume={governed.confirmResume}
        onCancelResume={governed.cancelResumeConfirmation}
        onOpenInspector={openInspector}
        conversation={workspaceConversationDataSource ? conversation : undefined}
      />
    );
  } else if (activeSurface === "review" && governedActionDataSource) {
    content = (
      <ReviewGovernedView
        snapshot={governed.snapshot}
        selectedItem={governed.selectedItem}
        refreshing={governed.refreshing}
        dispatchState={governed.reviewState}
        onRefresh={() => void governed.load(true)}
        onSelectItem={item => {
          governed.selectReviewItem(item);
          setSelectedEvidence("");
          setAnnouncement(`已选择“${item.decisionContext.title}”；没有记录任何决定。`);
        }}
        onRequestAction={governed.requestReviewAction}
        onConfirmAction={governed.confirmReviewAction}
        onCancelConfirmation={governed.cancelReviewConfirmation}
        backLabel={
          reviewReturnSurface === "life-model"
            ? "返回个人智能"
            : reviewReturnSurface === "settings"
              ? "返回模型与供应商"
              : undefined
        }
        onBackWorkspace={() => {
          if (reviewReturnSurface === "settings") {
            setMode("settings");
            setActiveSurface(settingsReturnSurface);
            onRouteChange?.({ mode: "settings", surface: settingsReturnSurface });
            setActiveSettingsId("model-provider");
            setInspectorOpen(false);
            setSelectedEvidence("");
            requestFocus("settings-review-return");
            setAnnouncement("已返回模型与供应商；审核决定不会自动重新测试或保存设置。 ");
          } else {
            navigateProduct(reviewReturnSurface);
          }
        }}
        onOpenInspector={openInspector}
      />
    );
  } else if (activeSurface === "life-model" && durableTruthDataSource) {
    content = (
      <DurableTruthView
        snapshot={durable.snapshot}
        selectedItem={durable.selectedItem}
        refreshing={durable.refreshing}
        memoryAction={durable.memoryAction}
        migrationAction={durable.migrationAction}
        lifeModelAction={durable.lifeModelAction}
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
        builder={lifeModelBuilderDataSource ? lifeModelBuilder : undefined}
        onOpenReviewCenter={
          lifeModelBuilderDataSource ? () => navigateProduct("review", "life-model") : undefined
        }
      />
    );
  } else {
    const copy = unavailableCopy[activeSurface];
    content = (
      <UnavailableReadOnlyView
        title={copy.title}
        reason={copy.reason}
        onToday={() => navigateProduct("today")}
        onTasks={() => navigateProduct("tasks")}
      />
    );
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
