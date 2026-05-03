import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { getAgentRun, deleteAgentRun, replayAgentAction, type AgentRun } from "../tauri";
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
} from "lucide-react";

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

  useEffect(() => {
    if (runId) {
      loadRun(runId);
    }
  }, [runId]);

  async function loadRun(id: string) {
    try {
      setLoading(true);
      const data = await getAgentRun(id);
      setRun(data);
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
        <div className="text-stone-500">运行记录不存在</div>
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
              <h3 className="text-sm font-semibold text-stone-700 mb-2">用户输入</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800">
                {run.userInput}
              </div>
            </div>
          )}

          {run.outputPreview && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">输出预览</h3>
              <div className="bg-stone-50 rounded-lg p-3 text-sm text-stone-800">
                {run.outputPreview}
              </div>
            </div>
          )}

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

          {run.modelRoute && (
            <div className="mb-6">
              <h3 className="text-sm font-semibold text-stone-700 mb-2">模型路由</h3>
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
          )}

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
                            if (action.output) {
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
                    } else {
                      const obs = entry.item;
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
                      );
                    }
                  });
                })()}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
