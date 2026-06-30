import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  getAgentRun,
  deleteAgentRun,
  getDangerActionPreflight,
  buildDangerActionConfirmationEvidence,
  replayAgentAction,
  listMainChatAgentTasks,
  getMainChatAgentTaskDetail,
  resumeMainChatAgentTask,
  cancelMainChatAgentTask,
  retryMainChatAgentAction,
  refreshMainChatAgentTaskContext,
  type AgentRun,
  type DangerActionPreflightView,
  type MainChatTaskSummary,
  type MainChatTaskDetail,
  type RunEvidenceView,
} from "../tauri";
import RunTracePanel from "../components/RunTracePanel";
import RuntimeDisclosureStrip from "../components/RuntimeDisclosureStrip";
import ConfirmDangerDialog from "../components/ConfirmDangerDialog";
import DangerActionPreflightDetails from "../components/DangerActionPreflightDetails";
import { DangerZone, StatusChip } from "../components/product/ProductPrimitives";
import { buildRuntimeDisclosure } from "../utils/runtimeDisclosure";
import { safePreviewText } from "../utils/safePreview";
import {
  ArrowLeft,
  Activity,
  Clock,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Trash2,
  Download,
  Play,
  Wrench,
  Eye,
  Zap,
  ListOrdered,
  RotateCw,
  Ban,
} from "lucide-react";

const STALE_RUN_THRESHOLD_MS = 10 * 60 * 1000;

function statusIcon(status: string) {
  switch (status) {
    case "running":
      return <Activity size={20} className="text-blue-500 animate-pulse" />;
    case "blocked":
      return <AlertTriangle size={20} className="text-amber-500" />;
    case "timed_out":
      return <Clock size={20} className="text-red-500" />;
    case "completed":
      return <CheckCircle size={20} className="text-emerald-500" />;
    case "failed":
      return <XCircle size={20} className="text-red-500" />;
    case "cancelled":
      return <AlertTriangle size={20} className="text-amber-500" />;
    default:
      return <Activity size={20} className="text-stone-400" />;
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
    skill: "Skill Runtime",
    plugin: "Plugin",
  };
  return labels[kind] || kind;
}

type ActivityTimelineItem = {
  id: string;
  title: string;
  body: string;
  timestamp?: string;
  tone: "neutral" | "info" | "warning" | "danger" | "ready";
};

function transcriptTitle(kind: string): string {
  const labels: Record<string, string> = {
    user_input: "用户目标",
    route_decision: "路线选择",
    plan: "计划",
    action: "执行动作",
    observation: "观察结果",
    permission_request: "需要权限",
    proposal_request: "创建 Review 建议",
    error: "发生错误",
    retry: "重试",
    final_result: "最终结果",
    fallback: "降级处理",
    follow_up: "后续回复",
  };
  return labels[kind] ?? kind.replace(/_/g, " ");
}

function lifecycleLabel(status: string): string {
  const labels: Record<string, string> = {
    running: "running",
    blocked: "blocked",
    timed_out: "timed_out",
    failed: "failed",
    cancelled: "cancelled",
    completed: "completed",
    waiting_permission: "blocked",
  };
  return labels[status] ?? status.replace(/_/g, " ");
}

function failureTitle(failureKind?: string | null, fallbackKind?: string): string {
  const labels: Record<string, string> = {
    timeout: "超时",
    cancelled: "已取消",
    provider_error: "Provider 错误",
    tool_error: "工具错误",
    policy_blocker: "治理阻断",
    unknown_error: "未知错误",
  };
  if (failureKind && labels[failureKind]) return labels[failureKind];
  if (fallbackKind === "blocker") return "治理阻断";
  return transcriptTitle(fallbackKind ?? "observation");
}

function toneForLifecycle(status?: string): ActivityTimelineItem["tone"] {
  if (status === "timed_out" || status === "failed") return "danger";
  if (status === "blocked") return "warning";
  if (status === "completed") return "ready";
  if (status === "cancelled") return "neutral";
  return "info";
}

function timelineTone(kind: string, status?: string): ActivityTimelineItem["tone"] {
  if (kind === "error" || status === "failed") return "danger";
  if (
    kind === "permission_request" ||
    kind === "proposal_request" ||
    status === "needs_confirmation"
  ) {
    return "warning";
  }
  if (kind === "final_result" || status === "completed" || status === "observed") return "ready";
  if (kind === "route_decision" || kind === "plan" || kind === "action") return "info";
  return "neutral";
}

function buildActivityTimeline(
  run: AgentRun,
  taskDetail: MainChatTaskDetail | null,
  evidenceView: RunEvidenceView | null
): ActivityTimelineItem[] {
  if (evidenceView) {
    return evidenceView.eventTimeline.map((entry, index) => ({
      id: entry.id || `evidence-${index}`,
      title: failureTitle(entry.failureKind, entry.kind),
      body: safePreviewText(entry.summary, 220),
      timestamp: entry.createdAt ?? undefined,
      tone: entry.normalizedLifecycleState
        ? toneForLifecycle(entry.normalizedLifecycleState)
        : timelineTone(entry.kind),
    }));
  }

  const transcriptItems =
    taskDetail?.transcript.map(entry => ({
      id: entry.id,
      title: transcriptTitle(entry.kind),
      body: safePreviewText(entry.summary, 220),
      timestamp: entry.createdAt,
      tone: timelineTone(entry.kind),
    })) ?? [];

  if (transcriptItems.length > 0) return transcriptItems;

  const statusItems =
    run.statusUpdates?.map((update, index) => ({
      id: `status-${index}`,
      title: transcriptTitle(update.phase),
      body: safePreviewText(update.message, 220),
      timestamp: update.timestamp,
      tone: timelineTone(update.phase),
    })) ?? [];

  const actionItems = run.actions.map(action => ({
    id: `action-${action.id}`,
    title:
      action.status === "needs_confirmation"
        ? "需要确认"
        : action.actionType === "tool"
          ? "工具动作"
          : "执行动作",
    body: safePreviewText(
      [action.actionType, action.target, action.error, action.reactTrace?.outputPreview]
        .filter(Boolean)
        .join(" · "),
      220
    ),
    timestamp: action.startedAt ?? action.timestamp,
    tone: timelineTone("action", action.status),
  }));

  const observationItems = run.observations.map(observation => ({
    id: `observation-${observation.id}`,
    title: "观察结果",
    body: safePreviewText(observation.reactTrace?.outputPreview ?? observation.content, 220),
    timestamp: observation.timestamp,
    tone: "ready" as const,
  }));

  const proposalItems = run.generatedProposals.map(proposalId => ({
    id: `proposal-${proposalId}`,
    title: "创建 Review 建议",
    body: `已创建待确认建议 ${proposalId}`,
    timestamp: run.finishedAt ?? run.startedAt,
    tone: "warning" as const,
  }));

  const finalItems = run.outputPreview
    ? [
        {
          id: "final-result",
          title: "最终结果",
          body: safePreviewText(run.outputPreview, 220),
          timestamp: run.finishedAt,
          tone: "ready" as const,
        },
      ]
    : [];

  return [
    ...statusItems,
    ...actionItems,
    ...observationItems,
    ...proposalItems,
    ...finalItems,
  ].sort((a, b) => {
    const timeA = a.timestamp ? new Date(a.timestamp).getTime() : 0;
    const timeB = b.timestamp ? new Date(b.timestamp).getTime() : 0;
    if (!Number.isFinite(timeA) || timeA === 0) return 1;
    if (!Number.isFinite(timeB) || timeB === 0) return -1;
    return timeA - timeB;
  });
}

export default function AgentRunDetail() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const [run, setRun] = useState<AgentRun | null>(null);
  const [taskSummary, setTaskSummary] = useState<MainChatTaskSummary | null>(null);
  const [taskDetail, setTaskDetail] = useState<MainChatTaskDetail | null>(null);
  const [taskBusy, setTaskBusy] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [deletePreflight, setDeletePreflight] = useState<DangerActionPreflightView | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  useEffect(() => {
    if (runId) {
      loadRun(runId);
    }
  }, [runId]);

  async function loadRun(id: string) {
    try {
      setLoading(true);
      const [data, summaries] = await Promise.all([
        getAgentRun(id),
        listMainChatAgentTasks({ includeTerminal: true, includeStale: true }, 100, 0).catch(
          () => []
        ),
      ]);
      setRun(data);
      const summary = summaries.find(item => item.runId === id) ?? null;
      setTaskSummary(summary);
      if (summary) {
        const detail = await getMainChatAgentTaskDetail(summary.taskSessionId).catch(() => null);
        setTaskDetail(detail);
      } else {
        setTaskDetail(null);
      }
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleTaskControl(control: "resume" | "cancel" | "retry" | "refresh") {
    if (!runId || !taskSummary) return;
    setTaskBusy(control);
    setError(null);
    try {
      if (control === "resume") {
        await resumeMainChatAgentTask(taskSummary.taskSessionId);
      } else if (control === "cancel") {
        await cancelMainChatAgentTask(taskSummary.taskSessionId);
      } else if (control === "refresh") {
        await refreshMainChatAgentTaskContext(taskSummary.taskSessionId);
      } else {
        const detail = taskDetail ?? (await getMainChatAgentTaskDetail(taskSummary.taskSessionId));
        const failedAction = detail.actions.find(action => action.status === "failed");
        if (!failedAction) {
          throw new Error("没有可重试的失败 action");
        }
        await retryMainChatAgentAction(taskSummary.taskSessionId, failedAction.id);
      }
      await loadRun(runId);
    } catch (e) {
      setError(`任务控制失败: ${String(e)}`);
    } finally {
      setTaskBusy(null);
    }
  }

  async function handleDelete() {
    if (!runId || !run) return;
    setDeleteBusy(true);
    setError(null);
    try {
      const view = await getDangerActionPreflight("agent_run_delete", false, {
        targetIds: [runId],
        affectedCount: 1,
      });
      setDeletePreflight(view);
    } catch (e) {
      setError(`删除预检失败: ${e}`);
    } finally {
      setDeleteBusy(false);
    }
  }

  async function continueDelete() {
    if (!runId || !deletePreflight || !deletePreflight.finalActionEnabled) return;
    const evidence = buildDangerActionConfirmationEvidence(deletePreflight, [runId]);
    setDeleteBusy(true);
    setError(null);
    try {
      await deleteAgentRun(runId, "user_confirmed_preflight", evidence);
      setDeletePreflight(null);
      navigate("/runs");
    } catch (e) {
      setError(`删除失败: ${e}`);
    } finally {
      setDeleteBusy(false);
    }
  }

  function handleDownloadTrace() {
    if (!run) return;
    const trace = {
      ...run,
      actions: run.actions,
      observations: run.observations,
    };
    const blob = new Blob([JSON.stringify(trace, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `run-${run.id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

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

  if (!run) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-stone-500">运行记录不存在</div>
      </div>
    );
  }

  const startedAt = new Date(run.startedAt).getTime();
  const stale =
    (taskSummary?.staleState && !["fresh", "none", "ok"].includes(taskSummary.staleState)) ||
    (run.status === "running" &&
      Number.isFinite(startedAt) &&
      Date.now() - startedAt > STALE_RUN_THRESHOLD_MS);
  const evidenceView = taskDetail?.evidenceView ?? taskSummary?.evidenceView ?? null;
  const lifecycleState = evidenceView?.lifecycleState ?? taskSummary?.lifecycleState ?? run.status;
  const activityTimeline = buildActivityTimeline(run, taskDetail, evidenceView);
  const allowedControls = evidenceView?.allowedControls ?? taskDetail?.allowedControls ?? [];
  const actionControls = allowedControls.filter(control =>
    ["resume", "retry", "cancel", "refresh_context"].includes(control)
  );

  return (
    <div className="h-full overflow-auto p-6">
      {deletePreflight && (
        <ConfirmDangerDialog
          open={Boolean(deletePreflight)}
          title="动作预检：删除运行记录"
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
          onConfirm={() => void continueDelete()}
          onCancel={() => setDeletePreflight(null)}
        />
      )}
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <button
            onClick={() => navigate("/runs")}
            className="flex items-center gap-2 text-stone-600 hover:text-stone-900"
          >
            <ArrowLeft size={20} />
            <span>返回列表</span>
          </button>
          <div className="flex items-center gap-2">
            <button
              onClick={handleDownloadTrace}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-stone-100 text-stone-700 hover:bg-stone-200 text-sm"
            >
              <Download size={14} />
              导出 Trace
            </button>
          </div>
        </div>

        <div className="bg-white rounded-xl border border-stone-200 p-6">
          <div className="flex items-center gap-3 mb-6">
            {statusIcon(lifecycleState)}
            <div>
              <h1 className="text-xl font-bold text-stone-900">{kindLabel(run.kind)}</h1>
              <div className="text-sm text-stone-500 flex items-center gap-2 mt-1">
                <span>ID: {run.id.slice(0, 8)}...</span>
                <span>·</span>
                <span>{lifecycleLabel(lifecycleState)}</span>
                <span>·</span>
                <Clock size={14} />
                <span>{new Date(run.startedAt).toLocaleString()}</span>
              </div>
            </div>
          </div>

          <div className="mb-6">
            <RuntimeDisclosureStrip
              view={buildRuntimeDisclosure(run, {
                taskSummary: taskSummary ?? undefined,
                evidenceView,
                runtimeRouteEvidence:
                  evidenceView?.routeEvidence ?? taskSummary?.routeEvidence ?? null,
                strictRuntimeRouteEvidence: Boolean(evidenceView),
              })}
              runId={run.id}
            />
          </div>

          {/* Stats Summary */}
          {evidenceView ? (
            <div className="mb-6 grid grid-cols-2 gap-3 md:grid-cols-4">
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <ListOrdered size={14} />
                  <span>Lifecycle</span>
                </div>
                <div className="text-sm font-bold text-stone-900">
                  {lifecycleLabel(evidenceView.lifecycleState)}
                </div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Zap size={14} />
                  <span>Actions</span>
                </div>
                <div className="text-xl font-bold text-stone-900">{evidenceView.actionCount}</div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Eye size={14} />
                  <span>Observations</span>
                </div>
                <div className="text-xl font-bold text-stone-900">
                  {evidenceView.observationCount}
                </div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Wrench size={14} />
                  <span>Controls</span>
                </div>
                <div className="text-sm font-bold text-stone-900">
                  {evidenceView.allowedControls.join(", ") || "open_trace"}
                </div>
              </div>
            </div>
          ) : run.stepCount || run.toolCallCount || run.actions.length || run.observations.length ? (
            <div className="mb-6 grid grid-cols-2 md:grid-cols-4 gap-3">
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <ListOrdered size={14} />
                  <span>推理步数</span>
                </div>
                <div className="text-xl font-bold text-stone-900">{run.stepCount ?? 0}</div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Wrench size={14} />
                  <span>工具调用</span>
                </div>
                <div className="text-xl font-bold text-stone-900">{run.toolCallCount ?? 0}</div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Zap size={14} />
                  <span>Actions</span>
                </div>
                <div className="text-xl font-bold text-stone-900">{run.actions.length}</div>
              </div>
              <div className="bg-stone-50 rounded-lg p-3 text-center">
                <div className="flex items-center justify-center gap-1 text-stone-500 text-xs mb-1">
                  <Eye size={14} />
                  <span>Observations</span>
                </div>
                <div className="text-xl font-bold text-stone-900">{run.observations.length}</div>
              </div>
            </div>
          ) : (
            <div className="mb-6 rounded-lg border border-dashed border-stone-200 bg-stone-50 px-4 py-5 text-sm text-stone-500">
              没有可展示的 task/run evidence。
            </div>
          )}

          {/* Duration */}
          {run.finishedAt && (
            <div className="mb-6 text-xs text-stone-500">
              持续时间:{" "}
              {(() => {
                const start = new Date(run.startedAt).getTime();
                const end = new Date(run.finishedAt).getTime();
                const diff = Math.round((end - start) / 1000);
                if (diff < 60) return `${diff} 秒`;
                return `${Math.floor(diff / 60)} 分 ${diff % 60} 秒`;
              })()}
            </div>
          )}

          {run.userInput && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">用户输入摘要</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800">
                {safePreviewText(run.userInput, 160)}
              </div>
            </div>
          )}

          {run.outputPreview && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">输出摘要</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800">
                {safePreviewText(run.outputPreview, 160)}
              </div>
            </div>
          )}

          <div className="mb-6">
            <h3 className="mb-3 text-sm font-semibold text-stone-700">Activity timeline</h3>
            {activityTimeline.length > 0 ? (
              <div className="space-y-2">
                {activityTimeline.map((item, index) => (
                  <div
                    key={item.id}
                    className="grid gap-3 rounded-lg border border-stone-200 bg-white px-3 py-3 text-sm sm:grid-cols-[90px_1fr]"
                  >
                    <div className="text-xs text-stone-500">
                      <div>
                        {item.timestamp ? new Date(item.timestamp).toLocaleTimeString() : "-"}
                      </div>
                      <div className="mt-1">Step {index + 1}</div>
                    </div>
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-semibold text-stone-950">{item.title}</span>
                        <StatusChip label={item.tone} tone={item.tone} />
                      </div>
                      <div className="mt-1 leading-6 text-stone-700">{item.body}</div>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-dashed border-stone-200 bg-stone-50 px-4 py-6 text-sm text-stone-500">
                这个运行记录还没有可展示的 timeline。
              </div>
            )}
          </div>

          <div className="mb-6">
            <h3 className="text-sm font-semibold text-stone-700 mb-2">任务控制</h3>
            {taskSummary ? (
              <div className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-700">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="rounded-md bg-stone-900 px-2 py-1 text-xs font-semibold text-white">
                    {lifecycleLabel(lifecycleState)}
                  </span>
                  <span>Session {taskSummary.taskSessionId.slice(-8)}</span>
                  <span>
                    推荐：
                    {evidenceView?.nextRecommendedControl ?? taskSummary.nextRecommendedControl}
                  </span>
                  {stale && (
                    <span className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-800">
                      连续性需复核
                    </span>
                  )}
                </div>
                {(evidenceView?.eventTimeline.length || taskSummary.lastObservationPreview) && (
                  <div className="mt-2 text-xs text-stone-500">
                    最近事件：
                    {safePreviewText(
                      evidenceView?.eventTimeline[evidenceView.eventTimeline.length - 1]?.summary ??
                        taskSummary.lastObservationPreview,
                      140
                    )}
                  </div>
                )}
                {evidenceView && (
                  <div className="mt-3 grid gap-2 text-xs text-stone-600">
                    <div>脱敏：隐藏 raw transcript 和敏感正文；保留 metadata-safe summary、ids、状态和 evidence digest。</div>
                    {evidenceView.blockers.length > 0 && (
                      <div>Blockers：{evidenceView.blockers.join(", ")}</div>
                    )}
                    {evidenceView.proposals.length > 0 && (
                      <div>Proposals：{evidenceView.proposals.join(", ")}</div>
                    )}
                    {evidenceView.planRefs.length > 0 && (
                      <div>Plan refs：{evidenceView.planRefs.join(", ")}</div>
                    )}
                  </div>
                )}
                {actionControls.length > 0 ? (
                  <div className="mt-3 flex flex-wrap gap-2">
                    {actionControls.map(control => {
                      const command =
                        control === "refresh_context"
                          ? "refresh"
                          : control === "resume"
                            ? "resume"
                            : control === "retry"
                              ? "retry"
                              : "cancel";
                      const Icon =
                        control === "cancel" ? Ban : control === "resume" ? Play : RotateCw;
                      return (
                        <button
                          key={control}
                          type="button"
                          onClick={() => void handleTaskControl(command)}
                          disabled={taskBusy !== null}
                          className="inline-flex items-center gap-1 rounded-md border border-stone-200 bg-white px-3 py-1.5 text-xs font-medium text-stone-700 disabled:opacity-50"
                        >
                          <Icon
                            size={12}
                            aria-hidden="true"
                            className={
                              taskBusy === "refresh" && control === "refresh_context"
                                ? "animate-spin"
                                : ""
                            }
                          />
                          {control.replace(/_/g, " ")}
                        </button>
                      );
                    })}
                  </div>
                ) : (
                  <div className="mt-3 text-xs text-stone-500">
                    当前只允许查看 trace。
                  </div>
                )}
              </div>
            ) : (
              <div className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-500">
                旧 run 或缺少 task session，当前无法直接控制。
              </div>
            )}
          </div>

          {run.error && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-red-700 mb-2">错误</h3>
              <div className="bg-red-50 rounded-lg p-3 text-sm text-red-800">
                <div className="font-medium">{run.error.message}</div>
                <div className="text-xs text-red-600 mt-1">
                  阶段: {run.error.phase} · 可恢复: {run.error.recoverable ? "是" : "否"}
                </div>
              </div>
            </div>
          )}

          {run.warnings && run.warnings.length > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-amber-700 mb-2">警告</h3>
              <div className="bg-amber-50 rounded-lg p-3 text-sm text-amber-800 space-y-1">
                {run.warnings.map((warning, idx) => (
                  <div key={idx} className="flex items-start gap-2">
                    <AlertTriangle size={14} className="text-amber-500 mt-0.5 shrink-0" />
                    <span>{warning}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {run.contextSummary && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">上下文摘要</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800 space-y-1">
                <div>LifeModel 空: {run.contextSummary.lifeModelEmpty ? "是" : "否"}</div>
                <div>记忆命中: {run.contextSummary.memoryHitCount}</div>
                <div>工具提示: {run.contextSummary.usedToolsPrompt ? "是" : "否"}</div>
                <div>脱敏: {run.contextSummary.redactionApplied ? "是" : "否"}</div>
              </div>
            </div>
          )}

          {evidenceView?.routeEvidence ? (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">路线详情</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800 space-y-1">
                <div>Evidence: {evidenceView.routeEvidence.evidence_id}</div>
                <div>
                  Provider:{" "}
                  {evidenceView.routeEvidence.actual_route?.provider ??
                    evidenceView.routeEvidence.planned_route?.provider ??
                    "provider 未验证"}
                </div>
                <div>
                  Model:{" "}
                  {evidenceView.routeEvidence.actual_route?.model ??
                    evidenceView.routeEvidence.planned_route?.model ??
                    "model 未验证"}
                </div>
                <div>
                  Route:{" "}
                  {evidenceView.routeEvidence.actual_route?.route_type ??
                    evidenceView.routeEvidence.planned_route?.route_type ??
                    "unknown"}
                </div>
                <div>External transmission: {evidenceView.routeEvidence.external_transmission}</div>
                <div>Truth confidence: {evidenceView.routeEvidence.truth_confidence}</div>
              </div>
            </div>
          ) : !evidenceView && run.modelRoute ? (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">路线详情</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800 space-y-1">
                <div>Provider: {run.modelRoute.provider}</div>
                <div>Model: {run.modelRoute.model}</div>
                <div>Route: {run.modelRoute.routeType}</div>
                <div>Reason: {run.modelRoute.reason}</div>
                <div>Privacy: {run.modelRoute.privacyLevel}</div>
                <div>Retry: {run.modelRoute.retryCount}</div>
                {run.modelRoute.fallbackReason && (
                  <div>Fallback: {run.modelRoute.fallbackReason}</div>
                )}
                {run.modelRoute.providerHealthIsEstimated !== undefined && (
                  <div>
                    Health:{" "}
                    {run.modelRoute.providerHealthIsEstimated ? "estimated / gray" : "probed"}
                  </div>
                )}
              </div>
            </div>
          ) : null}

          <div className="mb-6">
            <h3 className="text-sm font-semibold text-stone-700 mb-2">协作行为</h3>
            <RunTracePanel run={run} />
          </div>

          {/* Status Timeline */}
          {run.statusUpdates && run.statusUpdates.length > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">
                状态时间线 ({run.statusUpdates.length})
              </h3>
              <div className="space-y-1">
                {run.statusUpdates.map((update, idx) => (
                  <div
                    key={idx}
                    className="flex items-start gap-3 text-sm py-1.5 px-3 rounded-lg hover:bg-stone-50 transition"
                  >
                    <div className="flex-shrink-0 w-16 text-xs text-stone-400">
                      {new Date(update.timestamp).toLocaleTimeString()}
                    </div>
                    <div className="flex-shrink-0">
                      {update.phase === "thinking" && (
                        <Activity size={14} className="text-blue-500" />
                      )}
                      {update.phase === "executing_tool" && (
                        <Wrench size={14} className="text-amber-500" />
                      )}
                      {update.phase === "observing" && <Eye size={14} className="text-green-500" />}
                      {update.phase === "generating" && (
                        <Zap size={14} className="text-purple-500" />
                      )}
                      {update.phase === "completed" && (
                        <CheckCircle size={14} className="text-emerald-500" />
                      )}
                      {update.phase === "error" && <XCircle size={14} className="text-red-500" />}
                      {![
                        "thinking",
                        "executing_tool",
                        "observing",
                        "generating",
                        "completed",
                        "error",
                      ].includes(update.phase) && <Clock size={14} className="text-stone-400" />}
                    </div>
                    <div className="flex-1">
                      <span className="text-xs font-medium text-stone-600">{update.phase}</span>
                      <span className="text-stone-700 ml-2">{update.message}</span>
                      {(update.stepIndex !== undefined || update.toolCallIndex !== undefined) && (
                        <span className="text-xs text-stone-400 ml-2">
                          {update.stepIndex !== undefined && `step ${update.stepIndex}`}
                          {update.toolCallIndex !== undefined && ` · tool ${update.toolCallIndex}`}
                        </span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {run.generatedProposals.length > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">生成的提案</h3>
              <div className="space-y-2">
                {run.generatedProposals.map(proposalId => (
                  <div key={proposalId} className="bg-blue-50 rounded-lg p-3 text-sm text-blue-800">
                    {proposalId}
                  </div>
                ))}
              </div>
            </div>
          )}

          {(run.actions.length > 0 || run.observations.length > 0) && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">
                执行时间线 ({run.actions.length + run.observations.length})
              </h3>
              <div className="space-y-2">
                {(() => {
                  const timeline = [
                    ...run.actions.map(a => ({ type: "action" as const, item: a })),
                    ...run.observations.map(o => ({ type: "observation" as const, item: o })),
                  ];
                  timeline.sort((a, b) => {
                    const timeA = a.item.timestamp ? new Date(a.item.timestamp).getTime() : 0;
                    const timeB = b.item.timestamp ? new Date(b.item.timestamp).getTime() : 0;
                    if (Number.isNaN(timeA) || timeA === 0) return 1;
                    if (Number.isNaN(timeB) || timeB === 0) return -1;
                    return timeA - timeB;
                  });
                  return timeline.map(entry => {
                    if (entry.type === "action") {
                      const action = entry.item;
                      const trace = action.reactTrace;
                      return (
                        <div
                          key={action.id}
                          className="bg-stone-50 rounded-lg p-3 text-sm border-l-4 border-blue-400"
                        >
                          <div className="font-medium text-stone-800 flex items-center gap-2">
                            <span className="text-blue-600 text-xs font-bold">ACTION</span>
                            {action.actionType}
                            {action.target ? ` · ${action.target}` : ""}
                            {action.status === "needs_confirmation" && (
                              <span className="inline-flex items-center gap-1 text-orange-600 text-xs">
                                <AlertTriangle size={12} /> 待确认
                              </span>
                            )}
                          </div>
                          <div className="text-xs text-stone-500 mt-1">
                            Status: {action.status} · Permission:{" "}
                            {action.permissionDecision ?? "n/a"} ·{" "}
                            {new Date(action.startedAt ?? action.timestamp).toLocaleString()}
                          </div>
                          {trace && (
                            <div className="mt-2 text-xs text-stone-600 bg-white rounded p-2">
                              <div className="font-medium mb-1">ReAct Trace:</div>
                              <div>Tool: {trace.toolName}</div>
                              <div>Source: {trace.toolSource}</div>
                              <div>Risk: {trace.riskLevel}</div>
                              <div>Status: {trace.status}</div>
                              <div>Category: {trace.actionCategory}</div>
                              {trace.outputPreview && <div>Output: {trace.outputPreview}</div>}
                              {trace.outputHash && <div>Hash: {trace.outputHash}</div>}
                            </div>
                          )}
                          {action.toolScope && (
                            <div className="mt-2 text-xs text-stone-600 bg-white rounded p-2">
                              <div className="font-medium mb-1">Tool Scope:</div>
                              <div>Tool: {action.toolScope.toolName}</div>
                              <div>Source: {action.toolScope.source}</div>
                              <div>Risk: {action.toolScope.riskLevel}</div>
                              <div>
                                Capabilities: {action.toolScope.capabilities.join(", ") || "none"}
                              </div>
                            </div>
                          )}
                          {/* Linked proposal extraction */}
                          {(() => {
                            let proposalId: string | null = null;
                            if (trace?.proposalId) {
                              proposalId = trace.proposalId;
                            } else if (action.output) {
                              // Try direct proposal_id
                              if (typeof action.output === "object" && action.output !== null) {
                                const direct = (action.output as any).proposal_id;
                                if (direct) proposalId = direct;
                                // Try wrapped in text field
                                const text = (action.output as any).text;
                                if (text && typeof text === "string") {
                                  try {
                                    const parsed = JSON.parse(text);
                                    if (parsed.proposal_id) proposalId = parsed.proposal_id;
                                  } catch {
                                    /* ignore parse error */
                                  }
                                }
                              }
                            }
                            if (!proposalId && run.generatedProposals.length > 0) {
                              // Fallback: link to the first generated proposal if action is recent
                              proposalId = run.generatedProposals[0];
                            }
                            return proposalId ? (
                              <div className="mt-2 text-xs bg-blue-50 rounded p-2">
                                <div className="font-medium text-blue-800 mb-1">
                                  Linked Proposal:
                                </div>
                                <div className="text-blue-700">{proposalId}</div>
                                <button
                                  onClick={() => navigate(`/review?proposal=${proposalId}`)}
                                  className="mt-1 text-blue-600 hover:text-blue-800 underline"
                                >
                                  查看 Proposal
                                </button>
                              </div>
                            ) : null;
                          })()}
                          {action.status === "needs_confirmation" && (
                            <div className="mt-2">
                              <button
                                onClick={async () => {
                                  try {
                                    await replayAgentAction(run.id, action.id);
                                    await loadRun(run.id);
                                  } catch (e) {
                                    alert(`Replay failed: ${e}`);
                                  }
                                }}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded bg-orange-600 text-white text-xs hover:bg-orange-700"
                              >
                                <Play size={12} />
                                重新执行
                              </button>
                            </div>
                          )}
                          {action.error && (
                            <div className="mt-2 rounded bg-red-50 px-2 py-1 text-xs text-red-700">
                              {safePreviewText(action.error, 140)}
                            </div>
                          )}
                        </div>
                      );
                    } else {
                      const obs = entry.item;
                      const trace = obs.reactTrace;
                      return (
                        <div
                          key={obs.id}
                          className="bg-stone-50 rounded-lg p-3 text-sm border-l-4 border-green-400"
                        >
                          <div className="flex items-center gap-2 mb-1">
                            <span className="text-green-600 text-xs font-bold">OBSERVATION</span>
                            <span className="text-xs text-stone-500">
                              {new Date(obs.timestamp).toLocaleString()}
                            </span>
                          </div>
                          <div className="text-stone-800">
                            {trace?.outputPreview ?? safePreviewText(obs.content, 140)}
                          </div>
                          <div className="text-xs text-stone-500 mt-1">
                            Source: {obs.source}
                            {obs.actionId ? ` · Action: ${obs.actionId.slice(0, 8)}` : ""}
                          </div>
                          {trace?.outputHash && (
                            <div className="mt-2 rounded bg-white px-2 py-1 text-xs text-stone-600">
                              {trace.outputHash}
                            </div>
                          )}
                        </div>
                      );
                    }
                  });
                })()}
              </div>
            </div>
          )}

          <DangerZone
            title="危险操作"
            description="删除运行记录只应在确认不再需要审计线索时使用；删除后当前版本不可恢复。"
          >
            <button
              onClick={handleDelete}
              disabled={deleteBusy}
              className="inline-flex items-center gap-1.5 rounded-md bg-white px-3 py-2 text-sm font-medium text-rose-700 ring-1 ring-rose-200 hover:bg-rose-100 disabled:opacity-50"
            >
              <Trash2 size={14} />
              {deleteBusy ? "预检中..." : "删除运行记录"}
            </button>
          </DangerZone>
        </div>
      </div>
    </div>
  );
}
