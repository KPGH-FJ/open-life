import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  AlertCircle,
  Check,
  Clock,
  Inbox,
  RefreshCw,
  ShieldCheck,
  X,
  Edit2,
  Hammer,
  SlidersHorizontal,
  Info,
  ChevronDown,
  ChevronUp,
  FileText,
  Bot,
  GitBranch,
} from "lucide-react";
import {
  acceptProposal,
  listProposals,
  batchAcceptLowRiskProposals,
  postponeProposal,
  rejectProposal,
  editProposal,
  getSystemDiagnostics,
  getConfig,
  replayAgentAction,
  type AgentProposal,
  type AppConfig,
  type SystemDiagnostics,
} from "../tauri";
import { isSafeMode, getSafeModeReason } from "../utils/safeMode";

function valuePreview(value: unknown): string {
  if (value === null || value === undefined) return "空";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function riskClass(risk: AgentProposal["riskLevel"]): string {
  if (risk === "high" || risk === "critical") return "border-rose-200 bg-rose-50 text-rose-800";
  if (risk === "medium") return "border-amber-200 bg-amber-50 text-amber-800";
  return "border-emerald-200 bg-emerald-50 text-emerald-800";
}

function sourceLabel(source: AgentProposal["source"]): { label: string; icon: React.ReactNode } {
  const labels: Record<string, { label: string; icon: React.ReactNode }> = {
    builder_review: { label: "Builder 构建", icon: <Hammer size={10} /> },
    calibration_run: { label: "Calibration 校准", icon: <SlidersHorizontal size={10} /> },
    feedback_evolution: { label: "反馈进化", icon: <RefreshCw size={10} /> },
    memory_governance: { label: "记忆治理", icon: <FileText size={10} /> },
    skill_runtime: { label: "Skill 运行", icon: <Bot size={10} /> },
    proactive_agent: { label: "主动建议", icon: <Bot size={10} /> },
    plugin: { label: "Plugin", icon: <GitBranch size={10} /> },
    manual: { label: "手动", icon: <Edit2 size={10} /> },
  };
  return labels[source] || { label: source, icon: <FileText size={10} /> };
}

function beforeAfterSummary(before: unknown, after: unknown): string {
  if (before === null || before === undefined) return `新增：${valuePreview(after).slice(0, 60)}`;
  const beforeStr = valuePreview(before);
  const afterStr = valuePreview(after);
  if (beforeStr.length < 60 && afterStr.length < 60) {
    return `${beforeStr} → ${afterStr}`;
  }
  return `从 ${beforeStr.slice(0, 40)}… 变更为 ${afterStr.slice(0, 40)}…`;
}

function evidenceSummary(proposal: AgentProposal): string {
  const parts: string[] = [];
  if (proposal.confidence >= 0.8) parts.push("高置信度");
  else if (proposal.confidence >= 0.5) parts.push("中等置信度");
  else parts.push("低置信度");
  if (proposal.source) {
    parts.push(`来源：${proposal.source}`);
  }
  if (proposal.reason.length > 0) {
    parts.push(`依据：${proposal.reason.slice(0, 80)}`);
  }
  return parts.join(" · ");
}

const TYPE_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "life_model_update", label: "LifeModel" },
  { value: "goal_update", label: "Goal" },
  { value: "state_update", label: "State" },
  { value: "preference_update", label: "Preference" },
  { value: "capability_update", label: "Capability" },
  { value: "memory_write", label: "Memory Write" },
  { value: "memory_archive", label: "Memory Archive" },
  { value: "tool_permission", label: "Tool" },
  { value: "plugin_permission", label: "Plugin" },
  { value: "schedule_checkin", label: "Schedule Check-in" },
  { value: "scheduled_task", label: "Scheduled Task" },
  { value: "external_write_action", label: "External Write" },
  { value: "model_policy_change", label: "Model Policy" },
  { value: "data_export", label: "Data Export" },
];

const RISK_OPTIONS = [
  { value: "", label: "全部风险" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "critical", label: "Critical" },
];

export default function ProposalReviewPage() {
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [loading, setLoading] = useState(true);
  const [actingId, setActingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filterType, setFilterType] = useState("");
  const [filterRisk, setFilterRisk] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [batchAccepting, setBatchAccepting] = useState(false);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [safePaths, setSafePaths] = useState<string[]>([]);
  const [expandedEvidence, setExpandedEvidence] = useState<Set<string>>(new Set());
  const [continueInfo, setContinueInfo] = useState<{
    runId: string;
    actionId: string;
  } | null>(null);
  const [replaying, setReplaying] = useState(false);

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  function isPathInSafePaths(path: string | undefined): boolean {
    if (!path || safePaths.length === 0) return false;
    // Normalize path separators
    const normalized = path.replace(/\\/g, "/");
    return safePaths.some(safe => {
      const safeNorm = safe.replace(/\\/g, "/");
      // Exact match or path is under safe directory
      return (
        normalized === safeNorm ||
        normalized.startsWith(safeNorm + "/") ||
        normalized.startsWith(safeNorm + "\\")
      );
    });
  }

  const appliedNotice = (proposal: AgentProposal): string => {
    if (proposal.proposalType === "tool_permission") {
      return `已更新工具权限：${proposal.affectedPath}`;
    }
    if (proposal.proposalType === "memory_write" || proposal.proposalType === "memory_archive") {
      return `已应用到记忆治理：${proposal.affectedPath}`;
    }
    if (
      proposal.proposalType === "plugin_permission" ||
      proposal.proposalType === "scheduled_task" ||
      proposal.proposalType === "external_write_action" ||
      proposal.proposalType === "model_policy_change" ||
      proposal.proposalType === "data_export"
    ) {
      return `已处理 Proposal：${proposal.affectedPath}`;
    }
    return `已应用到人生模型：${proposal.affectedPath}`;
  };

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [data, diag, config] = await Promise.all([
        listProposals("pending", filterType || undefined, filterRisk || undefined, 100),
        getSystemDiagnostics().catch(() => null),
        getConfig().catch(() => null),
      ]);
      setProposals(data);
      setDiagnostics(diag);
      setSafePaths((config as AppConfig | null)?.system?.safe_paths ?? []);
      setSelectedIds(new Set());
    } catch (e) {
      setError(`加载 Proposal 失败：${String(e)}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, [filterType, filterRisk]);

  const isUnsupportedType = (type: string): boolean => {
    return ["plugin_permission", "model_policy_change", "schedule_checkin", "unsupported"].includes(
      type
    );
  };

  const runAction = async (proposal: AgentProposal, action: "accept" | "reject" | "postpone") => {
    setActingId(proposal.id);
    setError(null);
    setNotice(null);
    setContinueInfo(null);

    // Prevent accepting unsupported proposal types
    if (action === "accept" && isUnsupportedType(proposal.proposalType)) {
      setError(
        `「${proposal.proposalType}」类型的 Proposal 在当前版本中尚未接入应用器，无法应用。该 Proposal 将保持 pending 状态，等待后续版本支持。`
      );
      setActingId(null);
      return;
    }

    try {
      if (action === "accept") {
        const res = await acceptProposal(proposal.id);
        setNotice(appliedNotice(proposal));
        // If backend signals the action can be replayed, expose it to the user
        const canCont = res.canContinue ?? res.can_continue;
        if (canCont) {
          const runId = res.continueRunId ?? res.continue_run_id;
          const actionId = res.continueActionId ?? res.continue_action_id;
          if (runId && actionId) {
            setContinueInfo({ runId, actionId });
          }
        }
      } else if (action === "reject") {
        await rejectProposal(proposal.id);
        setNotice(`已拒绝：${proposal.affectedPath}`);
      } else {
        await postponeProposal(proposal.id);
        setNotice(`已稍后处理：${proposal.affectedPath}`);
      }
      await load();
    } catch (e) {
      const msg = String(e);
      if (msg.includes("no_such_field") || msg.includes("不包含字段路径")) {
        setError(`应用失败：字段路径 "${proposal.affectedPath}" 不存在于当前 LifeModel。`);
      } else if (msg.includes("无法转换")) {
        setError(`应用失败：值类型与字段 "${proposal.affectedPath}" 不匹配。`);
      } else if (msg.includes("尚未接入应用器") || msg.includes("not supported")) {
        setError(`应用失败：该 Proposal 类型在当前版本中尚未支持。Proposal 将保持 pending 状态。`);
      } else {
        setError(`处理 Proposal 失败：${msg}`);
      }
    } finally {
      setActingId(null);
    }
  };

  const handleReplay = async () => {
    if (!continueInfo) return;
    setReplaying(true);
    setNotice(null);
    setError(null);
    try {
      const result = await replayAgentAction(continueInfo.runId, continueInfo.actionId);
      setNotice(
        `已重放动作，状态：${result.status}${
          result.error ? ` — ${result.error.slice(0, 100)}` : ""
        }`
      );
      setContinueInfo(null);
      await load();
    } catch (e) {
      setError(`重放失败：${String(e)}`);
    } finally {
      setReplaying(false);
    }
  };

  const toggleSelection = (id: string, risk: string) => {
    if (risk === "high" || risk === "critical") return;
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleBatchAccept = async () => {
    if (selectedIds.size === 0) return;
    setBatchAccepting(true);
    setError(null);
    try {
      const count = await batchAcceptLowRiskProposals(Array.from(selectedIds));
      setNotice(`批量接受完成：${count} 个 Proposal 已应用`);
      await load();
    } catch (e) {
      setError(`批量接受失败：${String(e)}`);
    } finally {
      setBatchAccepting(false);
    }
  };

  const startEdit = (proposal: AgentProposal) => {
    setEditingId(proposal.id);
    setEditValue(valuePreview(proposal.after));
    setError(null);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditValue("");
  };

  const saveEdit = async (proposal: AgentProposal) => {
    setActingId(proposal.id);
    try {
      let parsed: unknown;
      try {
        parsed = JSON.parse(editValue);
      } catch {
        parsed = editValue;
      }
      await editProposal(proposal.id, parsed);
      setNotice(`已编辑并应用：${proposal.affectedPath}`);
      setEditingId(null);
      await load();
    } catch (e) {
      const msg = String(e);
      if (msg.includes("无法转换") || msg.includes("JSON")) {
        setError(`编辑失败：值无法应用到字段 "${proposal.affectedPath}"，请检查 JSON 格式。`);
      } else {
        setError(`编辑失败：${msg}`);
      }
    } finally {
      setActingId(null);
    }
  };

  const selectableCount = proposals.filter(
    p => p.riskLevel !== "high" && p.riskLevel !== "critical"
  ).length;
  const allSelected = selectableCount > 0 && selectableCount === selectedIds.size;

  const toggleSelectAll = () => {
    if (allSelected) {
      setSelectedIds(new Set());
    } else {
      const ids = proposals
        .filter(p => p.riskLevel !== "high" && p.riskLevel !== "critical")
        .map(p => p.id);
      setSelectedIds(new Set(ids));
    }
  };

  return (
    <div className="h-full overflow-auto bg-[#f7f1e8] p-6">
      <div className="mx-auto max-w-6xl space-y-6">
        <div className="overflow-hidden rounded-3xl border border-stone-200 bg-stone-950 text-amber-50 shadow-sm">
          <div className="relative p-6">
            <div className="absolute -right-12 -top-16 h-44 w-44 rounded-full bg-amber-400/20 blur-3xl" />
            <div className="relative flex flex-wrap items-start justify-between gap-4">
              <div>
                <div className="inline-flex items-center gap-2 rounded-full border border-amber-200/20 bg-white/8 px-3 py-1 text-xs text-amber-100">
                  <ShieldCheck size={14} />
                  Proposal / Confirmation
                </div>
                <h2 className="mt-4 text-2xl font-bold tracking-tight">Review Center</h2>
                <p className="mt-2 max-w-2xl text-sm leading-6 text-stone-300">
                  这里集中处理 OpenLife
                  对人生模型的更新建议。确认前不会写入，拒绝后不会影响现有模型。
                </p>
              </div>
              <button
                onClick={load}
                className="inline-flex items-center gap-2 rounded-full border border-amber-100/20 bg-white/10 px-4 py-2 text-sm text-amber-50 hover:bg-white/15"
              >
                <RefreshCw size={15} />
                刷新
              </button>
            </div>
          </div>
        </div>

        {notice && (
          <div className="rounded-2xl border border-emerald-100 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
            {notice}
            {continueInfo && (
              <div className="mt-2">
                <button
                  onClick={handleReplay}
                  disabled={replaying}
                  className="inline-flex items-center gap-1 rounded-xl bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-700 disabled:opacity-50"
                >
                  {replaying ? "重放中…" : "继续执行已批准的动作"}
                </button>
              </div>
            )}
          </div>
        )}
        {error && (
          <div className="rounded-2xl border border-rose-100 bg-rose-50 px-4 py-3 text-sm text-rose-800">
            <div className="font-medium">处理失败</div>
            <div className="mt-1">{error}</div>
          </div>
        )}
        {safeMode && (
          <div className="rounded-2xl border border-amber-100 bg-amber-50 px-4 py-3 text-sm text-amber-800">
            <div className="font-medium">系统处于 Safe Mode</div>
            <div className="mt-1">
              {safeModeReason} 当前仅可查看和拒绝/稍后处理 Proposal，无法应用或编辑。
            </div>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2 rounded-full border border-stone-200 bg-white px-3 py-1.5">
            <SlidersHorizontal size={14} className="text-stone-400" />
            <select
              value={filterType}
              onChange={e => setFilterType(e.target.value)}
              className="bg-transparent text-sm text-stone-700 outline-none"
            >
              {TYPE_OPTIONS.map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
          <div className="flex items-center gap-2 rounded-full border border-stone-200 bg-white px-3 py-1.5">
            <ShieldCheck size={14} className="text-stone-400" />
            <select
              value={filterRisk}
              onChange={e => setFilterRisk(e.target.value)}
              className="bg-transparent text-sm text-stone-700 outline-none"
            >
              {RISK_OPTIONS.map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
          {selectedIds.size > 0 && (
            <button
              onClick={handleBatchAccept}
              disabled={safeMode || batchAccepting}
              className="inline-flex items-center gap-1.5 rounded-full bg-emerald-600 px-3 py-1.5 text-xs text-white hover:bg-emerald-700 disabled:opacity-50"
            >
              <Check size={13} />
              {safeMode ? "批量接受（Safe Mode）" : `批量接受低风险 (${selectedIds.size})`}
            </button>
          )}
        </div>

        {loading ? (
          <div className="rounded-3xl border border-stone-200 bg-white p-8 text-sm text-stone-500">
            正在加载待确认 Proposal...
          </div>
        ) : proposals.length === 0 ? (
          <div className="rounded-3xl border border-stone-200 bg-white p-10 text-center">
            <Inbox className="mx-auto text-stone-300" size={42} />
            <div className="mt-4 text-lg font-semibold text-stone-900">当前没有待确认 Proposal</div>
            <p className="mt-2 text-sm text-stone-500">
              完成 Builder Review 或 Calibration 后，更新建议会出现在这里。
            </p>
            <div className="mt-6 flex justify-center gap-3">
              <Link
                to="/builder"
                className="inline-flex items-center gap-1.5 rounded-full bg-stone-900 px-4 py-2 text-sm text-amber-50 hover:bg-stone-800"
              >
                <Hammer size={14} />去 Builder 构建
              </Link>
              <Link
                to="/calibration"
                className="inline-flex items-center gap-1.5 rounded-full border border-stone-200 px-4 py-2 text-sm text-stone-600 hover:bg-stone-50"
              >
                <SlidersHorizontal size={14} />去 Calibration 校准
              </Link>
            </div>
          </div>
        ) : (
          <div className="space-y-4">
            {selectableCount > 0 && (
              <div className="flex items-center gap-2 text-sm text-stone-600">
                <button
                  onClick={toggleSelectAll}
                  className="inline-flex items-center gap-1.5 rounded-full border border-stone-200 px-3 py-1 text-xs hover:bg-stone-50"
                >
                  {allSelected ? "取消全选" : "全选低风险"}
                </button>
                <span className="text-xs text-stone-400">
                  已选择 {selectedIds.size} 个（高风险不可选）
                </span>
              </div>
            )}
            <div className="grid gap-4">
              {proposals.map(proposal => (
                <div
                  key={proposal.id}
                  className="rounded-3xl border border-stone-200 bg-white p-5 shadow-sm"
                >
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div className="flex items-start gap-3">
                      {(proposal.riskLevel === "low" || proposal.riskLevel === "medium") && (
                        <input
                          type="checkbox"
                          checked={selectedIds.has(proposal.id)}
                          onChange={() => toggleSelection(proposal.id, proposal.riskLevel)}
                          className="mt-1 h-4 w-4 rounded border-stone-300 text-stone-900 focus:ring-stone-900"
                        />
                      )}
                      <div>
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="font-semibold text-stone-950">
                            {proposal.affectedPath}
                          </span>
                          <span
                            className={`rounded-full border px-2.5 py-1 text-[11px] font-medium ${riskClass(proposal.riskLevel)}`}
                          >
                            {proposal.riskLevel}
                          </span>
                          <span className="rounded-full bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">
                            {proposal.proposalType}
                          </span>
                          {isUnsupportedType(proposal.proposalType) && (
                            <span className="rounded-full bg-amber-100 px-2.5 py-1 text-[11px] text-amber-700">
                              暂不支持
                            </span>
                          )}
                        </div>
                        <p className="mt-2 text-sm leading-6 text-stone-600">{proposal.reason}</p>
                        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-stone-400">
                          <span>来源：{proposal.source}</span>
                          <span>置信度：{Math.round(proposal.confidence * 100)}%</span>
                          {proposal.sourceDetail && (
                            <span className="text-stone-500" title={proposal.sourceDetail}>
                              详情：{proposal.sourceDetail.slice(0, 30)}
                              {proposal.sourceDetail.length > 30 ? "..." : ""}
                            </span>
                          )}
                          {proposal.source && (
                            <span className="inline-flex items-center gap-1 rounded-full bg-stone-100 px-2 py-0.5 text-[10px]">
                              {proposal.source === "builder_review"
                                ? "Builder"
                                : proposal.source === "calibration_run"
                                  ? "Calibration"
                                  : proposal.source === "skill_runtime"
                                    ? "Skill"
                                    : proposal.source}
                              {proposal.runId && (
                                <Link
                                  to={`/runs/${proposal.runId}`}
                                  className="text-stone-500 hover:text-stone-700 hover:underline"
                                  title={`查看来源 Run: ${proposal.runId}`}
                                >
                                  #{proposal.runId.slice(0, 8)}
                                </Link>
                              )}
                            </span>
                          )}
                        </div>

                        {/* Evidence & Source Context */}
                        <div className="mt-3 border-t border-stone-100 pt-3 space-y-2">
                          {/* Source badge with clear distinction */}
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="inline-flex items-center gap-1.5 rounded-full bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">
                              {sourceLabel(proposal.source).icon}
                              {sourceLabel(proposal.source).label}
                            </span>
                            {proposal.runId && (
                              <Link
                                to={`/runs/${proposal.runId}`}
                                className="inline-flex items-center gap-1 text-[11px] text-blue-600 hover:text-blue-800 hover:underline"
                              >
                                <Info size={10} />
                                Run #{proposal.runId.slice(0, 8)}
                              </Link>
                            )}
                            {(proposal.riskLevel === "high" ||
                              proposal.riskLevel === "critical") && (
                              <span className="inline-flex items-center gap-1 rounded-full bg-rose-50 border border-rose-200 px-2.5 py-1 text-[11px] text-rose-700">
                                <AlertCircle size={10} />
                                高风险 — 需谨慎审查
                              </span>
                            )}
                          </div>

                          {/* Before/After comparison */}
                          {(proposal.before !== undefined || proposal.after !== undefined) && (
                            <div className="rounded-lg bg-stone-50 border border-stone-200 p-3">
                              <button
                                onClick={() => {
                                  const next = new Set(expandedEvidence);
                                  if (next.has(proposal.id)) next.delete(proposal.id);
                                  else next.add(proposal.id);
                                  setExpandedEvidence(next);
                                }}
                                className="w-full flex items-center justify-between text-xs text-stone-600"
                              >
                                <span className="font-medium flex items-center gap-1.5">
                                  <GitBranch size={12} />
                                  变更摘要
                                </span>
                                {expandedEvidence.has(proposal.id) ? (
                                  <ChevronUp size={14} />
                                ) : (
                                  <ChevronDown size={14} />
                                )}
                              </button>
                              {expandedEvidence.has(proposal.id) ? (
                                <div className="mt-2 space-y-2 text-xs">
                                  <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                                    <div className="rounded bg-white border border-stone-100 p-2">
                                      <div className="text-[10px] text-stone-400 mb-1">变更前</div>
                                      <pre className="text-stone-600 whitespace-pre-wrap break-all">
                                        {valuePreview(proposal.before ?? "(空)")}
                                      </pre>
                                    </div>
                                    <div className="rounded bg-white border border-stone-100 p-2">
                                      <div className="text-[10px] text-stone-400 mb-1">变更后</div>
                                      <pre className="text-stone-600 whitespace-pre-wrap break-all">
                                        {valuePreview(proposal.after)}
                                      </pre>
                                    </div>
                                  </div>
                                </div>
                              ) : (
                                <div className="mt-1 text-stone-500 text-[11px]">
                                  {beforeAfterSummary(proposal.before, proposal.after)}
                                </div>
                              )}
                            </div>
                          )}

                          {/* Evidence summary */}
                          <div className="rounded-lg bg-blue-50/50 border border-blue-100 p-2.5">
                            <div className="flex items-center gap-1.5 text-[10px] text-blue-600 font-medium mb-1">
                              <FileText size={10} />
                              Evidence 上下文
                            </div>
                            <div className="text-[11px] text-blue-800 leading-relaxed">
                              {evidenceSummary(proposal)}
                            </div>
                          </div>
                        </div>
                        {proposal.proposalType === "tool_permission" && proposal.after && (
                          <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] text-stone-500">
                            <span className="rounded bg-stone-100 px-1.5 py-0.5">
                              工具：
                              {proposal.after.tool_name ||
                                proposal.after.toolName ||
                                proposal.after.name ||
                                "unknown"}
                            </span>
                            <span className="rounded bg-stone-100 px-1.5 py-0.5">
                              权限：
                              {proposal.after.permission ||
                                proposal.after.level ||
                                "allow_until_revoked"}
                            </span>
                            {proposal.after.source && (
                              <span className="rounded bg-stone-100 px-1.5 py-0.5">
                                来源：{proposal.after.source}
                              </span>
                            )}
                            {proposal.after.risk_level && (
                              <span className="rounded bg-stone-100 px-1.5 py-0.5">
                                风险：{proposal.after.risk_level}
                              </span>
                            )}
                          </div>
                        )}
                        {proposal.proposalType === "external_write_action" && proposal.after && (
                          <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 p-3 space-y-2">
                            <div className="text-xs font-medium text-stone-700">文件写入详情</div>
                            <div className="grid grid-cols-2 gap-2 text-xs text-stone-600">
                              <div>
                                <span className="text-stone-400">路径：</span>
                                <span className="font-mono">{proposal.after.path || "—"}</span>
                              </div>
                              <div>
                                <span className="text-stone-400">操作：</span>
                                <span
                                  className={
                                    proposal.after.operation === "overwrite"
                                      ? "text-amber-600"
                                      : "text-green-600"
                                  }
                                >
                                  {proposal.after.operation === "overwrite" ? "覆盖" : "创建"}
                                </span>
                              </div>
                              <div>
                                <span className="text-stone-400">大小：</span>
                                {proposal.after.size_bytes != null
                                  ? `${proposal.after.size_bytes} bytes`
                                  : "—"}
                              </div>
                              <div>
                                <span className="text-stone-400">编码：</span>
                                {proposal.after.encoding || "utf-8"}
                              </div>
                              {proposal.after.content_hash && (
                                <div className="col-span-2">
                                  <span className="text-stone-400">SHA256：</span>
                                  <span className="font-mono text-[10px]">
                                    {proposal.after.content_hash.slice(0, 16)}...
                                  </span>
                                </div>
                              )}
                              <div className="col-span-2">
                                <span className="text-stone-400">Safe Path：</span>
                                {isPathInSafePaths(proposal.after.path) ? (
                                  <span className="text-green-600 font-medium">
                                    ✅ 在 Safe Paths 内
                                  </span>
                                ) : (
                                  <span className="text-red-600 font-medium">
                                    ❌ 不在 Safe Paths 内（接受将失败）
                                  </span>
                                )}
                              </div>
                            </div>
                            {proposal.after.content_preview && (
                              <div className="mt-2">
                                <div className="text-[10px] text-stone-400 mb-1">内容预览：</div>
                                <pre className="text-xs text-stone-600 bg-white rounded p-2 max-h-24 overflow-auto whitespace-pre-wrap break-all">
                                  {proposal.after.content_preview}
                                </pre>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <button
                        onClick={() => startEdit(proposal)}
                        disabled={safeMode || actingId === proposal.id || editingId === proposal.id}
                        className="inline-flex items-center gap-1.5 rounded-full border border-stone-200 px-3 py-1.5 text-xs text-stone-600 hover:bg-stone-50 disabled:opacity-50"
                      >
                        <Edit2 size={13} />
                        {safeMode ? "编辑（Safe Mode）" : "编辑"}
                      </button>
                      <button
                        onClick={() => runAction(proposal, "postpone")}
                        disabled={actingId === proposal.id}
                        className="inline-flex items-center gap-1.5 rounded-full border border-stone-200 px-3 py-1.5 text-xs text-stone-600 hover:bg-stone-50 disabled:opacity-50"
                      >
                        <Clock size={13} />
                        稍后
                      </button>
                      <button
                        onClick={() => runAction(proposal, "reject")}
                        disabled={actingId === proposal.id}
                        className="inline-flex items-center gap-1.5 rounded-full border border-rose-200 px-3 py-1.5 text-xs text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                      >
                        <X size={13} />
                        拒绝
                      </button>
                      <button
                        onClick={() => runAction(proposal, "accept")}
                        disabled={
                          safeMode ||
                          actingId === proposal.id ||
                          isUnsupportedType(proposal.proposalType) ||
                          (proposal.proposalType === "external_write_action" &&
                            proposal.after &&
                            !isPathInSafePaths(proposal.after.path))
                        }
                        title={
                          isUnsupportedType(proposal.proposalType)
                            ? "该类型 Proposal 在当前版本中尚未支持"
                            : proposal.proposalType === "external_write_action" &&
                                proposal.after &&
                                !isPathInSafePaths(proposal.after.path)
                              ? "目标路径不在 Safe Paths 内，无法应用"
                              : undefined
                        }
                        className="inline-flex items-center gap-1.5 rounded-full bg-stone-900 px-3 py-1.5 text-xs text-amber-50 hover:bg-stone-800 disabled:opacity-50"
                      >
                        <Check size={13} />
                        {isUnsupportedType(proposal.proposalType) ? "暂不支持" : "应用"}
                      </button>
                    </div>
                  </div>

                  {editingId === proposal.id ? (
                    <div className="mt-4 space-y-3">
                      <div className="rounded-2xl border border-amber-200 bg-amber-50/50 p-3">
                        <div className="text-xs font-medium text-amber-700">编辑 After 值</div>
                        <textarea
                          value={editValue}
                          onChange={e => setEditValue(e.target.value)}
                          className="mt-2 w-full rounded-xl border border-amber-200 bg-white p-3 text-xs leading-5 text-stone-800 outline-none focus:border-amber-400"
                          rows={6}
                        />
                        <div className="mt-2 flex gap-2">
                          <button
                            onClick={() => saveEdit(proposal)}
                            disabled={actingId === proposal.id}
                            className="inline-flex items-center gap-1.5 rounded-full bg-stone-900 px-3 py-1.5 text-xs text-amber-50 hover:bg-stone-800 disabled:opacity-50"
                          >
                            <Check size={13} />
                            保存并应用
                          </button>
                          <button
                            onClick={cancelEdit}
                            className="inline-flex items-center gap-1.5 rounded-full border border-stone-200 px-3 py-1.5 text-xs text-stone-600 hover:bg-stone-50"
                          >
                            取消
                          </button>
                        </div>
                      </div>
                    </div>
                  ) : (
                    <div className="mt-4 grid gap-3 md:grid-cols-2">
                      <div className="rounded-2xl border border-stone-100 bg-stone-50 p-3">
                        <div className="text-xs font-medium text-stone-500">Before</div>
                        <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap text-xs leading-5 text-stone-700">
                          {valuePreview(proposal.before)}
                        </pre>
                      </div>
                      <div className="rounded-2xl border border-amber-100 bg-amber-50/50 p-3">
                        <div className="text-xs font-medium text-amber-700">After</div>
                        <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap text-xs leading-5 text-stone-800">
                          {valuePreview(proposal.after)}
                        </pre>
                      </div>
                    </div>
                  )}

                  {(proposal.riskLevel === "high" || proposal.riskLevel === "critical") && (
                    <div className="mt-3 flex items-start gap-2 rounded-2xl border border-rose-100 bg-rose-50 px-3 py-2 text-xs text-rose-700">
                      <AlertCircle size={14} className="mt-0.5 shrink-0" />
                      高风险字段会改变 OpenLife 对你的核心理解，请确认它真的符合你。此 Proposal
                      不支持批量接受。
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
