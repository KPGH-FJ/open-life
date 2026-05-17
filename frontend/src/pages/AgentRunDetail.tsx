import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import {
  getAgentRun,
  deleteAgentRun,
  replayAgentAction,
  listAgentRunEvents,
  type AgentRun,
} from "../tauri";
import type { AgentRunEvent } from "../types";
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
  History,
} from "lucide-react";
import { getTypedActionViewModel } from "../utils/typedContract";
import RunTracePanel from "../components/RunTracePanel";
import ToolObservationPanel from "../components/ToolObservationPanel";
import PlanPanel from "../components/PlanPanel";
import RunExplanationPanel from "../components/RunExplanationPanel";

function statusIcon(status: string) {
  switch (status) {
    case "running":
      return <Activity size={20} className="text-blue-500 animate-pulse" />;
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
  };
  return labels[kind] || kind;
}

export default function AgentRunDetail() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const [run, setRun] = useState<AgentRun | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<AgentRunEvent[]>([]);
  const [showTrace, setShowTrace] = useState(false);

  useEffect(() => {
    if (runId) {
      loadRun(runId);
    }
  }, [runId]);

  async function loadRun(id: string) {
    try {
      setLoading(true);
      const [data, evts] = await Promise.all([
        getAgentRun(id),
        listAgentRunEvents(id).catch(() => [] as AgentRunEvent[]),
      ]);
      setRun(data);
      setEvents(evts);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleDelete() {
    if (!runId || !run) return;
    if (!confirm("确定要删除这条运行记录吗？")) return;
    try {
      await deleteAgentRun(runId);
      navigate("/runs");
    } catch (e) {
      setError(`删除失败: ${e}`);
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
        <div className="text-center">
          <div className="text-stone-500 mb-3">运行记录不存在</div>
          <button
            onClick={() => navigate("/runs")}
            className="text-sm px-3 py-1.5 rounded-md bg-stone-100 text-stone-700 hover:bg-stone-200 border"
          >
            返回 Runs 列表
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
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
            <button
              onClick={handleDelete}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-50 text-red-600 hover:bg-red-100 text-sm"
            >
              <Trash2 size={14} />
              删除
            </button>
          </div>
        </div>

        <div className="bg-white rounded-xl border border-stone-200 p-6">
          <div className="flex items-center gap-3 mb-6">
            {statusIcon(run.status)}
            <div>
              <h1 className="text-xl font-bold text-stone-900">{kindLabel(run.kind)}</h1>
              <div className="text-sm text-stone-500 flex items-center gap-2 mt-1">
                <span>ID: {run.id.slice(0, 8)}...</span>
                <span>·</span>
                <Clock size={14} />
                <span>{new Date(run.startedAt).toLocaleString()}</span>
              </div>
            </div>
          </div>

          {/* Stats Summary */}
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

          {/* Duration */}
          {run.finishedAt && (
            <div className="mb-6 text-xs text-stone-500">
              持续时间:{" "}
              {Math.round(
                (new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime()) / 1000
              ) < 60
                ? `${Math.round((new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime()) / 1000)} 秒`
                : `${Math.floor((new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime()) / 1000 / 60)} 分 ${Math.round((new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime()) / 1000) % 60} 秒`}
            </div>
          )}

          {/* Run-level explanation (typed contract driven) */}
          {events.length > 0 && (
            <div className="mb-6">
              <RunExplanationPanel events={events} run={run} />
            </div>
          )}

          {/* AgentRunEvent Timeline */}
          {events.length > 0 && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2 flex items-center gap-2">
                <History size={14} />
                事件时间线 ({events.length})
              </h3>
              <RunTracePanel
                events={events}
                runId={run.id}
                show={showTrace}
                onToggle={() => setShowTrace(!showTrace)}
              />
            </div>
          )}

          {/* Tool Observation Panel */}
          <ToolObservationPanel run={run} />

          {/* Plan Panel */}
          <PlanPanel runId={run.id} />

          {(run.actions.length > 0 || run.observations.length > 0) && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">
                详细执行时间线 ({run.actions.length + run.observations.length})
              </h3>
              <div className="space-y-2">
                {run.actions.map(action => {
                  const vm = getTypedActionViewModel(action);
                  const borderClass = vm.isBlocked
                    ? "border-l-amber-400"
                    : vm.isFailed
                      ? "border-l-red-400"
                      : vm.isSuccess
                        ? "border-l-green-400"
                        : "border-l-blue-400";
                  return (
                    <div
                      key={action.id}
                      className={`bg-stone-50 rounded-lg p-3 text-sm border-l-4 ${borderClass}`}
                    >
                      <div className="font-medium text-stone-800 flex items-center gap-2">
                        <span className="text-blue-600 text-xs font-bold">ACTION</span>
                        {action.actionType}
                        {action.target ? ` · ${action.target}` : ""}
                        {vm.needsConfirmation && (
                          <span className="inline-flex items-center gap-1 text-orange-600 text-xs">
                            <AlertTriangle size={12} /> 待确认
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-stone-500 mt-1">
                        Status: {action.status} · Permission: {action.permissionDecision ?? "n/a"} ·{" "}
                        {new Date(action.startedAt ?? action.timestamp).toLocaleString()}
                      </div>
                      {vm.typedReasonAvailable && (
                        <div className="mt-2 space-y-1">
                          {vm.blockReasonLabel && (
                            <div className="text-xs bg-red-50 rounded px-2 py-1 text-red-700">
                              阻断原因: {vm.blockReasonLabel}
                            </div>
                          )}
                          {vm.proposalReasonLabel && (
                            <div className="text-xs bg-blue-50 rounded px-2 py-1 text-blue-700">
                              需确认: {vm.proposalReasonLabel}
                            </div>
                          )}
                          {vm.failureKindLabel && (
                            <div className="text-xs bg-red-50 rounded px-2 py-1 text-red-700">
                              失败类型: {vm.failureKindLabel}
                            </div>
                          )}
                          {vm.agentSpecId && (
                            <div className="text-xs text-stone-500">
                              AgentSpec: {vm.agentSpecId}
                            </div>
                          )}
                          {vm.proposalId && (
                            <div className="text-xs text-blue-600">Proposal: {vm.proposalId}</div>
                          )}
                        </div>
                      )}
                      {/* Legacy: show error as detail only (not for state inference) */}
                      {action.error && !vm.typedReasonAvailable && (
                        <div className="mt-2 rounded bg-red-50 px-2 py-1 text-xs text-red-700">
                          {action.error}
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
                      {(() => {
                        let proposalId: string | null = null;
                        if (action.output) {
                          if (typeof action.output === "object" && action.output !== null) {
                            const direct = (action.output as any).proposal_id;
                            if (direct) proposalId = direct;
                            const text = (action.output as any).text;
                            if (text && typeof text === "string") {
                              try {
                                const parsed = JSON.parse(text);
                                if (parsed.proposal_id) proposalId = parsed.proposal_id;
                              } catch {
                                /* ignore */
                              }
                            }
                          }
                        }
                        if (!proposalId && run.generatedProposals.length > 0) {
                          proposalId = run.generatedProposals[0];
                        }
                        if (!proposalId) return null;
                        return (
                          <div className="mt-2 text-xs bg-blue-50 rounded p-2">
                            <div className="font-medium text-blue-800 mb-1">Linked Proposal:</div>
                            <div className="text-blue-700">{proposalId}</div>
                            <button
                              onClick={() => navigate(`/review?proposal=${proposalId}`)}
                              className="mt-1 text-blue-600 hover:text-blue-800 underline"
                            >
                              查看 Proposal
                            </button>
                          </div>
                        );
                      })()}
                      {action.status === "needs_confirmation" && (
                        <div className="mt-2 space-y-2">
                          <button
                            onClick={async () => {
                              try {
                                const result = await replayAgentAction(run.id, action.id);
                                await loadRun(run.id);
                                if (result.status === "blocked" || result.status === "failed") {
                                  const reason = result.error ?? result.status;
                                  alert(
                                    `重放结果: ${result.status}${reason ? ` — ${reason.slice(0, 100)}` : ""}`
                                  );
                                }
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
                          {action.error}
                        </div>
                      )}
                      {action.output && (
                        <pre className="mt-2 max-h-32 overflow-auto rounded bg-white px-2 py-1 text-xs text-stone-600">
                          {JSON.stringify(action.output, null, 2)}
                        </pre>
                      )}
                    </div>
                  );
                })}
                {run.observations.map(obs => (
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
                    <div className="text-stone-800">{obs.content}</div>
                    <div className="text-xs text-stone-500 mt-1">
                      Source: {obs.source}
                      {obs.actionId ? ` · Action: ${obs.actionId.slice(0, 8)}` : ""}
                    </div>
                    {obs.structuredResult && (
                      <pre className="mt-2 max-h-32 overflow-auto rounded bg-white px-2 py-1 text-xs text-stone-600">
                        {JSON.stringify(obs.structuredResult, null, 2)}
                      </pre>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
