import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  listAgentRuns,
  deleteAgentRun,
  listMainChatAgentTasks,
  resumeMainChatAgentTask,
  cancelMainChatAgentTask,
  retryMainChatAgentAction,
  refreshMainChatAgentTaskContext,
  getMainChatAgentTaskDetail,
  type AgentRun,
  type MainChatTaskSummary,
} from "../tauri";
import { safePreviewText } from "../utils/safePreview";
import {
  getPlanExecuteProductTrace,
  planExecuteProductSearchText,
  planExecuteProductSubtitle,
} from "../utils/planExecuteProduct";
import { getMultiStrategyPreviewAudit, previewWarningLabel } from "../utils/previewAudit";
import { buildRunDisplaySummary } from "../utils/runDisplaySummary";
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

function statusIcon(status: string) {
  switch (status) {
    case "running":
      return <Activity size={16} className="text-blue-500 animate-pulse" />;
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
    conversation: "Chat",
    builder: "Life Model Building",
    calibration: "Calibration",
    evolution: "Evolution",
    tool_execution: "Tool",
    proactive: "Proactive",
    planning: "Planning",
    review: "Review",
    writing: "Writing",
    memory_governance: "Memory",
  };
  return labels[kind] || kind;
}

function runKindLabel(run: AgentRun): string {
  if (getPlanExecuteProductTrace(run)) return "Plan-Execute Weekly Plan";
  return getMultiStrategyPreviewAudit(run) ? "Multi-Strategy Preview" : kindLabel(run.kind);
}

function runSubtitle(run: AgentRun): string {
  const productTrace = getPlanExecuteProductTrace(run);
  if (productTrace) {
    return planExecuteProductSubtitle(productTrace);
  }
  const audit = getMultiStrategyPreviewAudit(run);
  if (audit) {
    return [audit.strategyKind, audit.payloadKind, audit.reasonCode].filter(Boolean).join(" · ");
  }
  return run.userInput ? safePreviewText(run.userInput, 96) : "No user input";
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
      return `${trace.toolName} ${trace.toolSource} ${trace.observationStatus ?? ""} ${trace.outputHash ?? ""}`;
    })
    .join(" ");
  return `${actionText} ${observationText}`;
}

const PAGE_SIZE = 20;
const STALE_RUN_THRESHOLD_MS = 10 * 60 * 1000;

function isPossiblyStaleRun(run: AgentRun, summary?: MainChatTaskSummary): boolean {
  if (summary?.staleState && !["fresh", "none", "ok"].includes(summary.staleState)) {
    return true;
  }
  if (run.status !== "running") return false;
  const startedAt = new Date(run.startedAt).getTime();
  return Number.isFinite(startedAt) && Date.now() - startedAt > STALE_RUN_THRESHOLD_MS;
}

export default function RunsPage() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [taskSummaries, setTaskSummaries] = useState<MainChatTaskSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [taskActionBusy, setTaskActionBusy] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [kindFilter, setKindFilter] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedRuns, setSelectedRuns] = useState<Set<string>>(new Set());
  const [showTrash, setShowTrash] = useState(false);
  const [page, setPage] = useState(0);
  const navigate = useNavigate();

  useEffect(() => {
    loadRuns();
  }, [showTrash]);

  async function loadRuns() {
    try {
      setLoading(true);
      const [data, tasks] = await Promise.all([
        listAgentRuns(100, 0),
        listMainChatAgentTasks({ includeTerminal: true, includeStale: true }, 100, 0).catch(
          () => []
        ),
      ]);
      setRuns(data);
      setTaskSummaries(tasks);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleTaskControl(
    summary: MainChatTaskSummary,
    control: "resume" | "cancel" | "retry" | "refresh"
  ) {
    setTaskActionBusy(`${summary.taskSessionId}:${control}`);
    setError(null);
    try {
      if (control === "resume") {
        await resumeMainChatAgentTask(summary.taskSessionId);
      } else if (control === "cancel") {
        await cancelMainChatAgentTask(summary.taskSessionId);
      } else if (control === "refresh") {
        await refreshMainChatAgentTaskContext(summary.taskSessionId);
      } else {
        const detail = await getMainChatAgentTaskDetail(summary.taskSessionId);
        const failedAction = detail.actions.find(action => action.status === "failed");
        if (!failedAction) {
          throw new Error("没有可重试的失败 action");
        }
        await retryMainChatAgentAction(summary.taskSessionId, failedAction.id);
      }
      await loadRuns();
    } catch (e) {
      setError(`任务控制失败: ${String(e)}`);
    } finally {
      setTaskActionBusy(null);
    }
  }

  const taskSummaryByRunId = new Map(taskSummaries.map(summary => [summary.runId, summary]));

  const filteredRuns = runs.filter(run => {
    // Trash filter
    if (showTrash) {
      return !!run.deletedAt;
    } else {
      if (run.deletedAt) return false;
    }

    // Status filter
    if (statusFilter !== "all" && run.status !== statusFilter) return false;

    // Kind filter
    if (kindFilter !== "all" && run.kind !== kindFilter) return false;

    // Search
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      const displaySummary = buildRunDisplaySummary(run, taskSummaryByRunId.get(run.id));
      const audit = getMultiStrategyPreviewAudit(run);
      const productTrace = getPlanExecuteProductTrace(run);
      const auditText = audit
        ? `${audit.runtimeStrategyTraceKind ?? ""} ${audit.selectedStrategyKind ?? ""} ${audit.strategyKind ?? ""} ${audit.payloadKind ?? ""} ${audit.strategyDescriptorId ?? ""} ${audit.governanceDecisionKind ?? ""} ${audit.selectionReasonCode ?? ""} ${audit.reasonCode ?? ""} ${(audit.strategyCapabilityIds ?? []).join(" ")}`
        : "";
      const productText = productTrace ? planExecuteProductSearchText(productTrace) : "";
      const outputText = productTrace ? "" : "";
      const traceText = reactTraceSearchText(run);
      const text =
        `${outputText} ${run.kind} ${displaySummary.searchableText} ${auditText} ${productText} ${traceText}`.toLowerCase();
      if (!text.includes(query)) return false;
    }

    return true;
  });

  const paginatedRuns = filteredRuns.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const totalPages = Math.ceil(filteredRuns.length / PAGE_SIZE);

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
    if (selectedRuns.size === paginatedRuns.length) {
      setSelectedRuns(new Set());
    } else {
      setSelectedRuns(new Set(paginatedRuns.map(r => r.id)));
    }
  }

  async function handleBatchDelete() {
    if (!confirm(`确定要删除选中的 ${selectedRuns.size} 条记录吗？`)) return;
    try {
      for (const runId of selectedRuns) {
        await deleteAgentRun(runId);
      }
      setSelectedRuns(new Set());
      await loadRuns();
    } catch (e) {
      setError(String(e));
    }
  }

  const statusOptions = [
    { value: "all", label: "全部状态" },
    { value: "running", label: "运行中" },
    { value: "completed", label: "已完成" },
    { value: "failed", label: "失败" },
    { value: "cancelled", label: "已取消" },
  ];

  const kindOptions = [
    { value: "all", label: "全部类型" },
    { value: "conversation", label: "Chat" },
    { value: "builder", label: "Builder" },
    { value: "calibration", label: "Calibration" },
    { value: "planning", label: "Planning" },
  ];

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-stone-500">加载中...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-red-500">{error}</div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-5xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold text-stone-900">{showTrash ? "已删除" : "Runs"}</h1>
            <div className="text-sm text-stone-500">
              共 {filteredRuns.length} 条记录
              {showTrash && " (当前版本不可恢复)"}
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
                  placeholder="搜索工具、来源或状态..."
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
                  className="flex items-center gap-1 px-3 py-1.5 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700 transition"
                >
                  <Trash2 size={14} />
                  删除
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
        {paginatedRuns.length === 0 ? (
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
                    paginatedRuns.length > 0 && paginatedRuns.every(r => selectedRuns.has(r.id))
                  }
                  onChange={selectAll}
                  className="rounded border-stone-300"
                />
                <span className="text-xs text-stone-500">全选本页</span>
              </div>

              {paginatedRuns.map(run => {
                const previewAudit = getMultiStrategyPreviewAudit(run);
                const productTrace = getPlanExecuteProductTrace(run);
                const warningCount = previewAudit?.warnings?.length ?? 0;
                const taskSummary = taskSummaryByRunId.get(run.id);
                const displaySummary = buildRunDisplaySummary(run, taskSummary);
                const subtitle =
                  productTrace || previewAudit ? runSubtitle(run) : displaySummary.subtitle;
                const stale = isPossiblyStaleRun(run, taskSummary);
                return (
                  <div
                    key={run.id}
                    className={`bg-white rounded-xl border p-4 cursor-pointer hover:shadow-md transition-shadow ${
                      selectedRuns.has(run.id)
                        ? "border-stone-900 ring-1 ring-stone-900"
                        : "border-stone-200"
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <input
                        type="checkbox"
                        checked={selectedRuns.has(run.id)}
                        onChange={e => {
                          e.stopPropagation();
                          toggleSelect(run.id);
                        }}
                        className="mt-1 rounded border-stone-300"
                      />
                      <div className="flex-1" onClick={() => navigate(`/runs/${run.id}`)}>
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-3">
                            {statusIcon(run.status)}
                            <div>
                              <div className="font-medium text-stone-900">{runKindLabel(run)}</div>
                              <div className="text-xs text-stone-500 mt-0.5">{subtitle}</div>
	                            </div>
	                          </div>
	                          <div className="text-right">
                            <div className="text-xs text-stone-400 flex items-center gap-1">
                              <Clock size={12} />
                              {new Date(run.startedAt).toLocaleString()}
                            </div>
                            {!productTrace && run.outputPreview && (
                              <div className="text-xs text-stone-500 mt-1 max-w-xs truncate">
                                {safePreviewText(run.outputPreview, 96)}
                              </div>
                            )}
	                          </div>
	                        </div>
                        <div className="mt-3 grid gap-2 text-xs text-stone-600 sm:grid-cols-2 lg:grid-cols-4">
                          <span className="rounded-lg border border-stone-200 bg-stone-50 px-2.5 py-2">
                            {displaySummary.outcome}
                          </span>
                          <span className="rounded-lg border border-stone-200 bg-stone-50 px-2.5 py-2">
                            {displaySummary.route}
                          </span>
                          <span className="rounded-lg border border-stone-200 bg-stone-50 px-2.5 py-2">
                            {displaySummary.tools}
                          </span>
                          <span className="rounded-lg border border-stone-200 bg-stone-50 px-2.5 py-2">
                            {displaySummary.proposals}
                          </span>
                        </div>
                        {taskSummary && (
                          <div className="mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                            <span className="font-semibold text-stone-800">
                              Task {taskSummary.status.replace(/_/g, " ")}
                            </span>
                            <span>{taskSummary.nextRecommendedControl}</span>
                            {stale && (
                              <span className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 font-medium text-amber-800">
                                可能已卡住
                              </span>
                            )}
                            <div className="ml-auto flex items-center gap-1">
                              <button
                                type="button"
                                onClick={event => {
                                  event.stopPropagation();
                                  void handleTaskControl(taskSummary, "resume");
                                }}
                                disabled={taskActionBusy !== null}
                                className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700 disabled:opacity-50"
                              >
                                Resume
                              </button>
                              <button
                                type="button"
                                onClick={event => {
                                  event.stopPropagation();
                                  void handleTaskControl(taskSummary, "retry");
                                }}
                                disabled={taskActionBusy !== null}
                                className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700 disabled:opacity-50"
                              >
                                Retry
                              </button>
                              <button
                                type="button"
                                onClick={event => {
                                  event.stopPropagation();
                                  void handleTaskControl(taskSummary, "cancel");
                                }}
                                disabled={taskActionBusy !== null}
                                className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700 disabled:opacity-50"
                              >
                                Cancel
                              </button>
                            </div>
                          </div>
                        )}
                        {!taskSummary && run.status === "running" && (
                          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-500">
                            旧 run 或缺少 task session，当前无法直接控制。
                          </div>
                        )}
	                        {previewAudit && (
                          <div className="mt-2 flex flex-wrap gap-2 text-xs">
                            <span className="rounded bg-stone-100 px-2 py-1 text-stone-700">
                              Preview
                            </span>
                            {previewAudit.strategyKind && (
                              <span className="rounded bg-blue-50 px-2 py-1 text-blue-700">
                                Strategy: {previewAudit.strategyKind}
                              </span>
                            )}
                            {previewAudit.payloadKind && (
                              <span className="rounded bg-teal-50 px-2 py-1 text-teal-700">
                                Payload: {previewAudit.payloadKind}
                              </span>
                            )}
                            {previewAudit.governanceDecisionKind && (
                              <span className="rounded bg-amber-50 px-2 py-1 text-amber-700">
                                Governance: {previewAudit.governanceDecisionKind}
                              </span>
                            )}
                            {warningCount > 0 && (
                              <span className="rounded bg-red-50 px-2 py-1 text-red-700">
                                {previewWarningLabel(warningCount)}
                              </span>
                            )}
                          </div>
                        )}
                        {productTrace && (
                          <div className="mt-2 flex flex-wrap gap-2 text-xs">
                            <span className="rounded bg-stone-100 px-2 py-1 text-stone-700">
                              Plan-Execute
                            </span>
                            {productTrace.status && (
                              <span className="rounded bg-teal-50 px-2 py-1 text-teal-700">
                                Status: {productTrace.status}
                              </span>
                            )}
                            {productTrace.stepCount !== undefined && (
                              <span className="rounded bg-blue-50 px-2 py-1 text-blue-700">
                                Steps: {productTrace.stepCount}
                              </span>
                            )}
                            {productTrace.generatedProposalCount !== undefined && (
                              <span className="rounded bg-amber-50 px-2 py-1 text-amber-700">
                                Proposals: {productTrace.generatedProposalCount}
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
                                  {previewWarningLabel(productTrace.warningCount)}
                                </span>
                              )}
                          </div>
                        )}
                        {run.error && (
                          <div className="mt-2 text-xs text-red-500 bg-red-50 rounded px-2 py-1">
                            {run.error.message}
                          </div>
                        )}
                        {!productTrace && run.generatedProposals.length > 0 && (
                          <div className="mt-2 text-xs text-blue-600 bg-blue-50 rounded px-2 py-1">
                            {run.generatedProposals.length} 个提案
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
