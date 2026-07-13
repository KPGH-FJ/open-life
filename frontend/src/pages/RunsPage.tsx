import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  listAgentRuns,
  deleteAgentRun,
  getDangerActionPreflight,
  buildDangerActionConfirmationEvidence,
  getTasksViewModel,
  resumeMainChatAgentTask,
  cancelMainChatAgentTask,
  retryMainChatAgentAction,
  refreshMainChatAgentTaskContext,
  type AgentRun,
  type DangerActionPreflightView,
  type TaskControl,
  type TaskViewModelItem,
  type TasksViewModel,
} from "../tauri";
import { safePreviewText } from "../utils/safePreview";
import {
  getPlanExecuteProductTrace,
  planExecuteProductSearchText,
  planExecuteProductSubtitle,
} from "../utils/planExecuteProduct";
import { buildRuntimeDisclosure } from "../utils/runtimeDisclosure";
import RuntimeDisclosureStrip from "../components/RuntimeDisclosureStrip";
import ConfirmDangerDialog from "../components/ConfirmDangerDialog";
import DangerActionPreflightDetails from "../components/DangerActionPreflightDetails";
import { runDetailRoute } from "../productShellContract";
import {
  Activity,
  Clock,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Trash2,
  RotateCcw,
  Search,
  Filter,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
} from "lucide-react";

const CONTENT_ABSENT_RECEIPT = /^[a-z0-9_]+:bytes=\d+:sha256:[0-9a-f]{64}$/;

function executionSummary(value: string | undefined): string {
  if (!value || CONTENT_ABSENT_RECEIPT.test(value)) return "正文未保存在执行记录中";
  return safePreviewText(value, 96);
}

function statusIcon(status: string) {
  switch (status) {
    case "running":
      return <Activity size={16} className="text-blue-500 animate-pulse" />;
    case "waiting_permission":
    case "blocked":
    case "completed_with_pending_review":
    case "completed_needs_evidence":
    case "remote_unknown":
      return <AlertTriangle size={16} className="text-amber-500" />;
    case "timed_out":
      return <Clock size={16} className="text-red-500" />;
    case "completed":
      return <CheckCircle size={16} className="text-emerald-500" />;
    case "failed":
      return <XCircle size={16} className="text-red-500" />;
    case "cancelled":
      return <AlertTriangle size={16} className="text-amber-500" />;
    default:
      return <Activity size={16} className="text-stone-400" />;
  }
}

function kindLabel(kind: string): string {
  const labels: Record<string, string> = {
    conversation: "对话任务",
    builder: "Life Model 构建",
    calibration: "Calibration",
    evolution: "Evolution",
    tool_execution: "Tool",
    proactive: "Proactive",
    planning: "Planning",
    review: "Mailbox",
    writing: "Writing",
    memory_governance: "Memory",
  };
  return labels[kind] || kind;
}

function runKindLabel(run: AgentRun): string {
  if (run.legacyPayloadUnverified) return "旧版运行记录";
  if (getPlanExecuteProductTrace(run)) return "计划执行";
  return kindLabel(run.kind);
}

function runSubtitle(run: AgentRun): string {
  if (run.legacyPayloadUnverified) return "旧版执行元数据未验证";
  const productTrace = getPlanExecuteProductTrace(run);
  if (productTrace) {
    return planExecuteProductSubtitle(productTrace);
  }
  return run.userInput ? safePreviewText(run.userInput, 96) : "No user input";
}

function warningLabel(count: number): string {
  return `${count} warning${count === 1 ? "" : "s"}`;
}

function taskStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    running: "运行中",
    waiting_permission: "等待确认",
    blocked: "已阻断",
    timed_out: "已超时",
    completed: "已完成",
    completed_with_pending_review: "待审核，未完成",
    completed_needs_evidence: "缺少完成证据",
    failed: "失败",
    remote_unknown: "远端状态未知",
    cancelled: "已取消",
    unknown: "未知",
  };
  return labels[status] ?? status.replace(/_/g, " ");
}

function terminalDeliveryLabel(status: string): string {
  const labels: Record<string, string> = {
    not_terminal: "未到终态",
    delivered: "已交付",
    missing_final_delivery_evidence: "缺少交付证据",
    completed_with_pending_review: "待审核，未完成",
    blocked: "已阻断",
    failed: "失败",
    cancelled: "已取消",
    unknown: "未知",
  };
  return labels[status] ?? status.replace(/_/g, " ");
}

function nextControlLabel(control: string): string {
  const labels: Record<string, string> = {
    cancel: "取消",
    resume: "继续",
    retry: "重试",
    refresh_context: "刷新上下文",
    open_trace: "查看记录",
    review_permission: "处理权限",
  };
  return labels[control] ?? control.replace(/_/g, " ");
}

function reactTraceSearchText(run: AgentRun): string {
  const actionText = run.actions
    .map(action => {
      const trace = action.reactTrace;
      if (!trace) return `${action.actionType} ${action.target ?? ""} ${action.status}`;
      return `${trace.toolName} ${trace.toolSource} ${trace.status} ${trace.riskLevel} ${trace.actionCategory} ${trace.permissionDecision ?? ""} ${trace.proposalId ?? ""}`;
    })
    .join(" ");
  const observationText = run.observations
    .map(observation => {
      const trace = observation.reactTrace;
      if (!trace) return observation.source;
      return `${trace.toolName} ${trace.toolSource} ${trace.observationStatus ?? ""} ${trace.outputReceipt?.digest ?? ""}`;
    })
    .join(" ");
  return `${actionText} ${observationText}`;
}

const PAGE_SIZE = 20;
const STALE_RUN_THRESHOLD_MS = 10 * 60 * 1000;

function isPossiblyStaleRun(run?: AgentRun): boolean {
  if (!run) return false;
  if (run.legacyPayloadUnverified) return false;
  if (run.status !== "running") return false;
  const startedAt = new Date(run.startedAt).getTime();
  return Number.isFinite(startedAt) && Date.now() - startedAt > STALE_RUN_THRESHOLD_MS;
}

function taskItemSearchText(item: TaskViewModelItem, run?: AgentRun): string {
  if (run?.legacyPayloadUnverified) {
    return [item.title, item.strategy, "unknown", run.id].join(" ");
  }
  return [
    item.title,
    item.strategy,
    item.lifecycleStatus,
    item.terminalDeliveryStatus,
    item.latestResultPreview?.preview ?? "",
    item.pendingBlockers.join(" "),
    item.pendingReviewItemRefs.map(ref => ref.label).join(" "),
    run ? `${run.kind} ${run.userInput ?? ""} ${run.outputPreview ?? ""}` : "",
    run ? reactTraceSearchText(run) : "",
  ].join(" ");
}

function enabledActionControls(item: TaskViewModelItem): TaskControl[] {
  return item.allowedControls.filter(
    control =>
      control.enabled &&
      [
        "task_resume_request",
        "task_retry_request",
        "task_cancel_request",
        "task_refresh_request",
      ].includes(control.effect)
  );
}

function commandForTaskControl(control: TaskControl): "resume" | "cancel" | "retry" | "refresh" {
  if (control.kind === "resume") return "resume";
  if (control.kind === "retry") return "retry";
  if (control.kind === "cancel") return "cancel";
  return "refresh";
}

export default function RunsPage() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [tasksViewModel, setTasksViewModel] = useState<TasksViewModel | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [taskActionBusy, setTaskActionBusy] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [kindFilter, setKindFilter] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedRuns, setSelectedRuns] = useState<Set<string>>(new Set());
  const [showTrash, setShowTrash] = useState(false);
  const [page, setPage] = useState(0);
  const [deletePreflight, setDeletePreflight] = useState<DangerActionPreflightView | null>(null);
  const [deleteTargetIds, setDeleteTargetIds] = useState<string[]>([]);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    loadRuns();
  }, [showTrash]);

  async function loadRuns() {
    try {
      setLoading(true);
      const [data, tasksEnvelope] = await Promise.all([listAgentRuns(100, 0), getTasksViewModel()]);
      setRuns(data);
      setTasksViewModel(tasksEnvelope.data);
      setError(null);
    } catch (e) {
      setError(String(e));
      setTasksViewModel(null);
    } finally {
      setLoading(false);
    }
  }

  async function handleTaskControl(
    item: TaskViewModelItem,
    taskControl: TaskControl,
    command: "resume" | "cancel" | "retry" | "refresh"
  ) {
    const taskSessionId = item.taskSessionId;
    if (!taskSessionId) {
      setError("任务控制不可用：缺少后端 task session。");
      return;
    }
    setTaskActionBusy(`${taskSessionId}:${taskControl.id}`);
    setError(null);
    try {
      if (command === "resume") {
        await resumeMainChatAgentTask(taskSessionId);
      } else if (command === "cancel") {
        await cancelMainChatAgentTask(taskSessionId);
      } else if (command === "refresh") {
        await refreshMainChatAgentTaskContext(taskSessionId);
      } else {
        if (!taskControl.targetActionId) {
          throw new Error("后端 read model 未提供可重试 action");
        }
        await retryMainChatAgentAction(taskSessionId, taskControl.targetActionId);
      }
      await loadRuns();
    } catch (e) {
      setError(`任务控制失败: ${String(e)}`);
    } finally {
      setTaskActionBusy(null);
    }
  }

  const taskItems = tasksViewModel?.items ?? [];
  const runById = new Map(runs.map(run => [run.id, run]));

  const filteredItems = taskItems.filter(item => {
    const run = item.relatedRunIds.map(runId => runById.get(runId)).find(Boolean);
    const lifecycle = run?.legacyPayloadUnverified ? "unknown" : item.lifecycleStatus;
    // Trash filter
    if (showTrash) {
      return !!run?.deletedAt;
    } else {
      if (run?.deletedAt) return false;
    }

    // Status filter
    if (statusFilter !== "all" && lifecycle !== statusFilter) return false;

    // Kind filter
    if (kindFilter !== "all" && (run?.kind ?? item.strategy) !== kindFilter) return false;

    // Search
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      const productTrace =
        run && !run.legacyPayloadUnverified ? getPlanExecuteProductTrace(run) : null;
      const productText = productTrace ? planExecuteProductSearchText(productTrace) : "";
      const text = `${taskItemSearchText(item, run)} ${productText}`.toLowerCase();
      if (!text.includes(query)) return false;
    }

    return true;
  });

  const paginatedItems = filteredItems.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const totalPages = Math.ceil(filteredItems.length / PAGE_SIZE);
  const paginatedSelectableRunIds = paginatedItems
    .map(item => item.relatedRunIds.map(runId => runById.get(runId)).find(Boolean)?.id)
    .filter((id): id is string => Boolean(id));

  function toggleSelect(runId: string) {
    const newSet = new Set(selectedRuns);
    if (newSet.has(runId)) {
      newSet.delete(runId);
    } else {
      newSet.add(runId);
    }
    setSelectedRuns(newSet);
  }

  function selectAll() {
    if (
      paginatedSelectableRunIds.length > 0 &&
      paginatedSelectableRunIds.every(id => selectedRuns.has(id))
    ) {
      setSelectedRuns(new Set());
    } else {
      setSelectedRuns(new Set(paginatedSelectableRunIds));
    }
  }

  async function handleBatchDelete() {
    const targetIds = Array.from(selectedRuns);
    if (targetIds.length === 0) return;
    setDeleteBusy(true);
    setError(null);
    try {
      const view = await getDangerActionPreflight(
        targetIds.length === 1 ? "agent_run_delete" : "agent_run_bulk_delete",
        false,
        { targetIds, affectedCount: targetIds.length }
      );
      setDeleteTargetIds(targetIds);
      setDeletePreflight(view);
    } catch (e) {
      setError(`删除预检失败: ${String(e)}`);
    } finally {
      setDeleteBusy(false);
    }
  }

  async function continueBatchDelete() {
    if (!deletePreflight || !deletePreflight.finalActionEnabled) return;
    const evidence = buildDangerActionConfirmationEvidence(deletePreflight, deleteTargetIds);
    setDeleteBusy(true);
    setError(null);
    try {
      for (const runId of deleteTargetIds) {
        await deleteAgentRun(runId, "user_confirmed_preflight", evidence);
      }
      setSelectedRuns(new Set());
      setDeletePreflight(null);
      setDeleteTargetIds([]);
      await loadRuns();
    } catch (e) {
      setError(String(e));
    } finally {
      setDeleteBusy(false);
    }
  }

  const statusOptions = [
    { value: "all", label: "全部状态" },
    { value: "running", label: "运行中" },
    { value: "blocked", label: "已阻断" },
    { value: "timed_out", label: "已超时" },
    { value: "completed", label: "已完成" },
    { value: "completed_with_pending_review", label: "待审核，未完成" },
    { value: "completed_needs_evidence", label: "缺少完成证据" },
    { value: "failed", label: "失败" },
    { value: "cancelled", label: "已取消" },
    { value: "unknown", label: "未知" },
  ];

  const kindOptions = [
    { value: "all", label: "全部类型" },
    { value: "conversation", label: "Chat" },
    { value: "builder", label: "Builder" },
    { value: "calibration", label: "Calibration" },
    { value: "planning", label: "Planning" },
  ];

  return (
    <div className="h-full overflow-auto p-6">
      {deletePreflight && (
        <ConfirmDangerDialog
          open={Boolean(deletePreflight)}
          title={
            deletePreflight.actionType === "agent_run_bulk_delete"
              ? "动作预检：批量删除运行记录"
              : "动作预检：删除运行记录"
          }
          description={<DangerActionPreflightDetails view={deletePreflight} />}
          confirmLabel={deletePreflight.finalActionEnabled ? "继续删除" : "Safe Mode 已阻断"}
          cancelLabel="返回"
          severity="danger"
          confirmationText={
            deletePreflight.confirmationRequired && deletePreflight.confirmationPhrase
              ? deletePreflight.confirmationPhrase
              : undefined
          }
          confirmDisabled={!deletePreflight.finalActionEnabled}
          busy={deleteBusy}
          onConfirm={() => void continueBatchDelete()}
          onCancel={() => {
            setDeletePreflight(null);
            setDeleteTargetIds([]);
          }}
        />
      )}
      <div className="max-w-5xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold text-stone-900">
              {showTrash ? "已删除记录" : "Runs"}
            </h1>
            <div className="text-sm text-stone-500">
              后端任务视图 · 共 {filteredItems.length} 条{showTrash && " (当前版本不可恢复)"}
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => {
                setShowTrash(!showTrash);
                setPage(0);
                setSelectedRuns(new Set());
              }}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition ${
                showTrash
                  ? "bg-stone-900 text-amber-50"
                  : "bg-white border border-stone-200 text-stone-700 hover:bg-stone-50"
              }`}
            >
              {showTrash ? <RotateCcw size={16} /> : <Trash2 size={16} />}
              {showTrash ? "返回列表" : "已删除"}
            </button>
            <button
              onClick={loadRuns}
              className="flex items-center gap-2 px-4 py-2 bg-white border border-stone-200 text-stone-700 rounded-lg text-sm font-medium hover:bg-stone-50 transition"
            >
              <RefreshCw size={16} />
              刷新
            </button>
          </div>
        </div>

        {/* Filters */}
        <div className="bg-white rounded-xl border border-stone-200 p-4 mb-4 space-y-3">
          <div className="flex flex-wrap gap-3">
            {/* Search */}
            <div className="flex-1 min-w-[200px]">
              <div className="relative">
                <Search
                  size={16}
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-stone-400"
                />
                <input
                  type="text"
                  placeholder="搜索任务、模型、工具、状态..."
                  value={searchQuery}
                  onChange={e => {
                    setSearchQuery(e.target.value);
                    setPage(0);
                  }}
                  className="w-full pl-9 pr-4 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
                />
              </div>
            </div>

            {/* Status filter */}
            <div className="flex items-center gap-2">
              <Filter size={16} className="text-stone-400" />
              <select
                value={statusFilter}
                onChange={e => {
                  setStatusFilter(e.target.value);
                  setPage(0);
                }}
                className="px-3 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
              >
                {statusOptions.map(opt => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>

            {/* Kind filter */}
            <select
              value={kindFilter}
              onChange={e => {
                setKindFilter(e.target.value);
                setPage(0);
              }}
              className="px-3 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
            >
              {kindOptions.map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          {/* Batch actions */}
          {selectedRuns.size > 0 && (
            <div className="flex items-center gap-3 pt-3 border-t border-stone-100">
              <span className="text-sm text-stone-600">已选择 {selectedRuns.size} 条</span>
              {showTrash ? (
                <span className="text-xs text-stone-400">已删除记录不可恢复</span>
              ) : (
                <button
                  onClick={handleBatchDelete}
                  disabled={deleteBusy}
                  className="flex items-center gap-1 px-3 py-1.5 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700 transition disabled:opacity-50"
                >
                  <Trash2 size={14} />
                  {deleteBusy ? "预检中..." : "删除"}
                </button>
              )}
              <button
                onClick={() => setSelectedRuns(new Set())}
                className="px-3 py-1.5 text-stone-600 text-sm hover:bg-stone-100 rounded-lg transition"
              >
                取消选择
              </button>
            </div>
          )}
        </div>

        {/* Runs List */}
        {loading ? (
          <div className="rounded-xl border border-stone-200 bg-white px-4 py-12 text-center text-stone-500">
            正在加载 Runs...
          </div>
        ) : error ? (
          <div className="rounded-xl border border-rose-200 bg-rose-50 px-4 py-6 text-rose-900">
            <div className="text-sm font-semibold">Runs 暂不可用</div>
            <div className="mt-1 text-xs leading-5 text-rose-800">{error}</div>
            <button
              onClick={loadRuns}
              className="mt-4 inline-flex items-center gap-2 rounded-lg border border-rose-200 bg-white px-3 py-1.5 text-sm font-medium text-rose-800 hover:bg-rose-50"
            >
              <RefreshCw size={14} />
              重新加载
            </button>
          </div>
        ) : paginatedItems.length === 0 ? (
          <div className="text-center py-12 text-stone-400">
            <Activity size={48} className="mx-auto mb-4 opacity-30" />
            <p>{showTrash ? "暂无已删除记录" : "暂无运行记录"}</p>
            <p className="text-sm mt-1">
              {showTrash
                ? "已删除的 Run 在当前版本中不可恢复"
                : "开始对话或构建 LifeModel 后将在此显示"}
            </p>
          </div>
        ) : (
          <>
            <div className="space-y-3">
              {/* Select all header */}
              <div className="flex items-center gap-3 px-4 py-2 bg-stone-50 rounded-lg">
                <input
                  type="checkbox"
                  checked={
                    paginatedSelectableRunIds.length > 0 &&
                    paginatedSelectableRunIds.every(runId => selectedRuns.has(runId))
                  }
                  onChange={selectAll}
                  className="rounded border-stone-300"
                />
                <span className="text-xs text-stone-500">全选本页</span>
              </div>

              {paginatedItems.map(item => {
                const run = item.relatedRunIds.map(runId => runById.get(runId)).find(Boolean);
                const legacyUnknown = run?.legacyPayloadUnverified === true;
                const productTrace = run && !legacyUnknown ? getPlanExecuteProductTrace(run) : null;
                const lifecycle = legacyUnknown ? "unknown" : item.lifecycleStatus;
                const subtitle = legacyUnknown
                  ? "旧版执行元数据未验证"
                  : productTrace
                    ? runSubtitle(run!)
                    : item.latestResultPreview?.preview
                      ? safePreviewText(item.latestResultPreview.preview, 96)
                      : terminalDeliveryLabel(item.terminalDeliveryStatus);
                const stale = isPossiblyStaleRun(run);
                const actionControls = legacyUnknown ? [] : enabledActionControls(item);
                const checkboxRunId = run?.id;
                return (
                  <div
                    key={item.canonicalTaskId}
                    className={`bg-white rounded-xl border p-4 cursor-pointer hover:shadow-md transition-shadow ${
                      checkboxRunId && selectedRuns.has(checkboxRunId)
                        ? "border-stone-900 ring-1 ring-stone-900"
                        : "border-stone-200"
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <input
                        type="checkbox"
                        checked={Boolean(checkboxRunId && selectedRuns.has(checkboxRunId))}
                        disabled={!checkboxRunId}
                        onChange={e => {
                          e.stopPropagation();
                          if (checkboxRunId) toggleSelect(checkboxRunId);
                        }}
                        className="mt-1 rounded border-stone-300"
                      />
                      <div
                        className="flex-1"
                        onClick={() => {
                          const targetRunId = item.relatedRunIds[0];
                          if (targetRunId) navigate(runDetailRoute(targetRunId));
                        }}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-3">
                            {statusIcon(lifecycle)}
                            <div>
                              <div className="font-medium text-stone-900">
                                {run ? runKindLabel(run) : item.strategy.replace(/_/g, " ")}
                              </div>
                              <div className="text-xs text-stone-500 mt-0.5">{subtitle}</div>
                            </div>
                          </div>
                          <div className="text-right">
                            {item.updatedAt && (
                              <div className="text-xs text-stone-400 flex items-center gap-1">
                                <Clock size={12} />
                                {new Date(item.updatedAt).toLocaleString()}
                              </div>
                            )}
                            {!legacyUnknown && !productTrace && run?.outputPreview && (
                              <div className="text-xs text-stone-500 mt-1 max-w-xs truncate">
                                {executionSummary(run.outputPreview)}
                              </div>
                            )}
                          </div>
                        </div>
                        {run && (
                          <div className="mt-3">
                            <RuntimeDisclosureStrip
                              view={buildRuntimeDisclosure(run, {
                                strictRuntimeRouteEvidence: legacyUnknown,
                              })}
                              runId={run.id}
                              compact
                            />
                          </div>
                        )}
                        <div className="mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                          <span className="font-semibold text-stone-800">
                            任务{taskStatusLabel(lifecycle)}
                          </span>
                          <span>
                            下一步：
                            {nextControlLabel(
                              legacyUnknown ? "open_trace" : item.nextRecommendedControl
                            )}
                          </span>
                          {stale && (
                            <span className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 font-medium text-amber-800">
                              连续性需复核
                            </span>
                          )}
                          <span>
                            交付：
                            {terminalDeliveryLabel(
                              legacyUnknown ? "unknown" : item.terminalDeliveryStatus
                            )}
                          </span>
                          {!legacyUnknown && item.pendingBlockers.length > 0 && (
                            <span>阻断：{item.pendingBlockers.slice(0, 3).join(", ")}</span>
                          )}
                          {!legacyUnknown && item.pendingReviewItemRefs.length > 0 && (
                            <span>待审核：{item.pendingReviewItemRefs.length}</span>
                          )}
                          {actionControls.length > 0 && (
                            <div className="ml-auto flex items-center gap-1">
                              {actionControls.map(control => (
                                <button
                                  key={control.id}
                                  type="button"
                                  onClick={event => {
                                    event.stopPropagation();
                                    void handleTaskControl(
                                      item,
                                      control,
                                      commandForTaskControl(control)
                                    );
                                  }}
                                  disabled={taskActionBusy !== null}
                                  title={control.disabledReason ?? control.effect}
                                  className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700 disabled:opacity-50"
                                >
                                  {control.label || nextControlLabel(control.kind)}
                                </button>
                              ))}
                            </div>
                          )}
                        </div>
                        {productTrace && (
                          <div className="mt-2 flex flex-wrap gap-2 text-xs">
                            <span className="rounded bg-stone-100 px-2 py-1 text-stone-700">
                              计划执行
                            </span>
                            {productTrace.status && (
                              <span className="rounded bg-teal-50 px-2 py-1 text-teal-700">
                                状态：{productTrace.status}
                              </span>
                            )}
                            {productTrace.stepCount !== undefined && (
                              <span className="rounded bg-blue-50 px-2 py-1 text-blue-700">
                                步骤：{productTrace.stepCount}
                              </span>
                            )}
                            {productTrace.generatedProposalCount !== undefined && (
                              <span className="rounded bg-amber-50 px-2 py-1 text-amber-700">
                                待确认：{productTrace.generatedProposalCount}
                              </span>
                            )}
                            {productTrace.metadataSafe && (
                              <span className="rounded bg-emerald-50 px-2 py-1 text-emerald-700">
                                metadata-safe
                              </span>
                            )}
                            {productTrace.warningCount !== undefined &&
                              productTrace.warningCount > 0 && (
                                <span className="rounded bg-red-50 px-2 py-1 text-red-700">
                                  {warningLabel(productTrace.warningCount)}
                                </span>
                              )}
                          </div>
                        )}
                        {run?.error && !legacyUnknown && (
                          <div
                            className={`mt-2 rounded px-2 py-1 text-xs ${
                              run.status === "remote_unknown"
                                ? "bg-amber-50 text-amber-700"
                                : "bg-red-50 text-red-500"
                            }`}
                          >
                            {run.status === "remote_unknown"
                              ? "远端状态未知，未自动重试"
                              : executionSummary(run.error.message)}
                          </div>
                        )}
                        {run?.legacyPayloadUnverified && (
                          <div className="mt-2 rounded border border-amber-200 bg-amber-50 px-2 py-1 text-xs text-amber-800">
                            旧版执行记录：receipt、route 与 digest 均不可作为已观察事实。
                          </div>
                        )}
                        {!legacyUnknown &&
                          !productTrace &&
                          run &&
                          run.generatedProposals.length > 0 && (
                            <div className="mt-2 text-xs text-blue-600 bg-blue-50 rounded px-2 py-1">
                              待确认 {run.generatedProposals.length}
                            </div>
                          )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
              <div className="flex items-center justify-between mt-6">
                <div className="text-sm text-stone-500">
                  第 {page + 1} / {totalPages} 页
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => setPage(Math.max(0, page - 1))}
                    disabled={page === 0}
                    className="flex items-center gap-1 px-3 py-2 bg-white border border-stone-200 rounded-lg text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-stone-50 transition"
                  >
                    <ChevronLeft size={16} />
                    上一页
                  </button>
                  <button
                    onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
                    disabled={page >= totalPages - 1}
                    className="flex items-center gap-1 px-3 py-2 bg-white border border-stone-200 rounded-lg text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-stone-50 transition"
                  >
                    下一页
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
