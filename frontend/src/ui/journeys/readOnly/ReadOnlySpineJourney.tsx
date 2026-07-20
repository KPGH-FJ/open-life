import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  CalendarDays,
  Cpu,
  Database,
  ListTodo,
  Monitor,
  Shield,
  ShieldCheck,
  UserRound,
  Wrench,
} from "lucide-react";
import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
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

export type ReadOnlyProductSurfaceId = "today" | "workspace" | "tasks" | "review" | "life-model";

const productNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "today", label: "今日", meta: "当前关注", icon: CalendarDays },
  { id: "workspace", label: "工作区", meta: "当前执行", icon: Monitor },
  { id: "tasks", label: "任务", meta: "队列与连续性", icon: ListTodo },
  { id: "review", label: "审核中心", meta: "建议与权限决定", icon: ShieldCheck },
  { id: "life-model", label: "LifeModel", meta: "长期状态", icon: UserRound },
];

const settingsNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "provider-privacy", label: "模型与隐私", meta: "路由与传输边界", icon: Shield },
  { id: "local-data", label: "本地数据", meta: "存储与保留", icon: Database },
  { id: "runtime", label: "运行环境", meta: "模型与工具", icon: Cpu },
  { id: "advanced", label: "高级", meta: "开发与诊断", icon: Wrench },
];

const unavailableCopy: Record<
  Exclude<ReadOnlyProductSurfaceId, "today" | "tasks">,
  { title: string; reason: string }
> = {
  workspace: {
    title: "工作区尚未接入本次只读主干",
    reason:
      "工作区需要贯通权限请求、审核决定、状态刷新与任务恢复。这个完整旅程将在后续阶段单独接入。",
  },
  review: {
    title: "审核中心尚未接入本次只读主干",
    reason: "当前不会用样例数据代替真实待决定项，也不会把“查看”解释成批准、拒绝或应用。",
  },
  "life-model": {
    title: "LifeModel 尚未接入本次只读主干",
    reason: "长期状态需要独立验证建议、决定、应用结果与失败回滚；批准仍不等于已经应用。",
  },
};

const settingsCopy: Record<string, { title: string; reason: string }> = {
  "provider-privacy": {
    title: "模型与隐私设置尚未迁移",
    reason: "本页只显示后端读取到的边界摘要，不在本次只读旅程中编辑或保存配置。",
  },
  "local-data": {
    title: "本地数据设置尚未迁移",
    reason: "导入、导出、保留和删除都可能改变持久状态，需要后续独立契约与确认流程。",
  },
  runtime: {
    title: "运行环境设置尚未迁移",
    reason: "模型与工具配置仍由现有生产页面负责；当前页面不会写入替代配置。",
  },
  advanced: {
    title: "高级诊断尚未迁移",
    reason: "高级信息是辅助检查工具，不作为同级产品入口，也不在产品工作面伪造调试状态。",
  },
};

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

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
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
          ? "审核中心接入前，只能确认存在待决定事项，不能在这里代替审批。"
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
            : "本次 slice 不执行任何任务控制。"
      : envelope.status === "stale" || envelope.status === "error"
        ? "陈旧或缺失的任务状态不能用于恢复、重试、取消或完成判断。"
        : "本次 slice 只读，不把 allowedControls 渲染为可执行按钮。",
    nextAction: selectedTask
      ? needsDecision
        ? "后续由审核旅程展示决定上下文；当前只核对来源。"
        : "核对来源；需要操作时等待对应业务旅程接入。"
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
    conclusion: "该产品旅程尚未在 Phase 4D 当前 slice 接入。",
    risk: "使用 fixture 或旧页面数据填充这里会制造第二套产品 truth。",
    nextAction: "返回今日或任务；等待对应后端契约与旅程在后续 slice 接入。",
    evidence: [],
    evidenceFeedback: "没有为 unavailable 页面伪造证据。",
    technicalDetails: [{ label: "availability", value: "not_migrated" }],
  };
}

export function ReadOnlySpineJourney({
  dataSource,
  initialSurface = "today",
}: {
  dataSource: ReadOnlySpineDataSource;
  initialSurface?: ReadOnlyProductSurfaceId;
}) {
  const [mode, setMode] = useState<"product" | "settings">("product");
  const [activeSurface, setActiveSurface] = useState<ReadOnlyProductSurfaceId>(initialSurface);
  const [activeSettingsId, setActiveSettingsId] = useState("provider-privacy");
  const [settingsQuery, setSettingsQuery] = useState("");
  const [todaySnapshot, setTodaySnapshot] = useState<TodayReadOnlySnapshot>(loadingTodaySnapshot);
  const [tasksSnapshot, setTasksSnapshot] = useState<TasksReadOnlySnapshot>(loadingTasksSnapshot);
  const [todayRefreshing, setTodayRefreshing] = useState(false);
  const [tasksRefreshing, setTasksRefreshing] = useState(false);
  const [tasksLoaded, setTasksLoaded] = useState(false);
  const [selectedTask, setSelectedTask] = useState<TaskViewModelItem | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [selectedEvidence, setSelectedEvidence] = useState("");
  const [announcement, setAnnouncement] = useState("正在读取今日状态。");
  const [focusKey, setFocusKey] = useState("initial");
  const focusSequenceRef = useRef(0);
  const todayRequestRef = useRef(0);
  const tasksRequestRef = useRef(0);

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
    setAnnouncement("正在读取今日状态。");
    void loadToday(false);
    if (initialSurface === "tasks") void loadTasks(false);
    return () => {
      todayRequestRef.current += 1;
      tasksRequestRef.current += 1;
    };
  }, [dataSource, initialSurface, loadTasks, loadToday]);

  function navigateProduct(id: string): void {
    const next = id as ReadOnlyProductSurfaceId;
    setMode("product");
    setActiveSurface(next);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`nav-${next}`);
    if (next === "tasks" && !tasksLoaded) void loadTasks(false);
    if (next === "today") {
      setAnnouncement("已进入今日，只显示后端提供的当前关注。 ");
    } else if (next === "tasks") {
      setAnnouncement("已进入任务，只读查看后端任务状态。 ");
    } else {
      setAnnouncement(`“${unavailableCopy[next].title}”，当前没有替代数据或重定向。`);
    }
  }

  function openSettings(): void {
    setMode("settings");
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus("settings-open");
    setAnnouncement("已进入设置上下文；当前分类尚未迁移，不会写入配置。 ");
  }

  function backFromSettings(): void {
    setMode("product");
    setSettingsQuery("");
    setInspectorOpen(false);
    setSelectedEvidence("");
    setAnnouncement("已返回之前的产品工作区。 ");
  }

  function navigateSettings(id: string): void {
    setActiveSettingsId(id);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestFocus(`settings-${id}`);
    setAnnouncement(`已进入“${settingsCopy[id].title}”；当前不会读取或保存替代配置。`);
  }

  const currentBoundaryEnvelope =
    activeSurface === "tasks" ? tasksSnapshot.boundaryEnvelope : todaySnapshot.boundaryEnvelope;
  const boundary = boundaryPresentation(currentBoundaryEnvelope);

  const context: WorkbenchContextSummary = useMemo(() => {
    if (mode === "settings") {
      return {
        eyebrow: "设置",
        title: settingsCopy[activeSettingsId].title,
        status: { label: "尚未迁移", status: "unknown" },
      };
    }
    if (activeSurface === "today") return todayContext(todaySnapshot.envelope);
    if (activeSurface === "tasks") return tasksContext(tasksSnapshot.envelope);
    return {
      eyebrow: "桌面工作台",
      title: unavailableCopy[activeSurface].title,
      status: { label: "尚未迁移", status: "unknown" },
    };
  }, [activeSettingsId, activeSurface, mode, tasksSnapshot.envelope, todaySnapshot.envelope]);

  const inspector = useMemo(() => {
    if (mode === "settings") {
      return unavailableInspector(settingsCopy[activeSettingsId].title);
    }
    if (activeSurface === "today") return todayInspector(todaySnapshot, selectedEvidence);
    if (activeSurface === "tasks") {
      return taskInspector(tasksSnapshot, selectedTask, selectedEvidence);
    }
    return unavailableInspector(unavailableCopy[activeSurface].title);
  }, [
    activeSettingsId,
    activeSurface,
    mode,
    selectedEvidence,
    selectedTask,
    tasksSnapshot,
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

  function openEvidence(evidence: WorkbenchEvidenceReference): void {
    setSelectedEvidence(evidence.id);
    setAnnouncement(
      `已选择依据“${evidence.label}”；来源 ${evidence.source}，敏感级别 ${evidence.sensitivity}。`
    );
  }

  let content;
  if (mode === "settings") {
    const copy = settingsCopy[activeSettingsId];
    content = (
      <UnavailableReadOnlyView
        title={copy.title}
        reason={copy.reason}
        onToday={() => navigateProduct("today")}
        onTasks={() => navigateProduct("tasks")}
      />
    );
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
        envelope={tasksSnapshot.envelope}
        refreshing={tasksRefreshing}
        selectedTaskId={selectedTask?.canonicalTaskId ?? null}
        onRefresh={() => void loadTasks(true)}
        onSelectTask={selectTask}
        onOpenInspector={openInspector}
        onAnnounce={setAnnouncement}
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
