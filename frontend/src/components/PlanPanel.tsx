import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Check,
  X,
  Play,
  RotateCcw,
  Ban,
  AlertTriangle,
  Info,
  ChevronDown,
  ChevronUp,
  ListOrdered,
  Shield,
  Eye,
  Edit2,
} from "lucide-react";
import type { AgentPlan, PlanOperationResult, PlanStatus } from "@/types";
import {
  confirmAgentPlan,
  rejectAgentPlan,
  executeAgentPlan,
  cancelAgentPlan,
  retryAgentPlan,
  continueAgentPlan,
  listAgentPlansForRun,
  editAgentPlan,
  type EditPlanRequest,
} from "../tauri";

interface Props {
  runId: string;
}

const LEGAL_CONFIRM_STATES: PlanStatus[] = ["draft", "published"];
const LEGAL_EXECUTE_STATES: PlanStatus[] = ["confirmed"];
const LEGAL_CANCEL_STATES: PlanStatus[] = ["published", "confirmed", "executing"];
const LEGAL_RETRY_STATES: PlanStatus[] = ["failed", "failed_review"];

function statusLabel(status: PlanStatus): string {
  const labels: Record<PlanStatus, string> = {
    draft: "草稿",
    published: "已发布",
    confirmed: "已确认",
    executing: "执行中",
    completed: "已完成",
    rejected: "已拒绝",
    cancelled: "已取消",
    failed: "失败",
    failed_review: "审查未通过",
  };
  return labels[status] || status;
}

function riskLabel(level: string): string {
  return level === "high" ? "高风险" : level === "medium" ? "中风险" : "低风险";
}

export default function PlanPanel({ runId }: Props) {
  const [plans, setPlans] = useState<AgentPlan[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actingId, setActingId] = useState<string | null>(null);
  const [result, setResult] = useState<PlanOperationResult | null>(null);
  const [expandedPlan, setExpandedPlan] = useState<string | null>(null);
  const [editingPlanId, setEditingPlanId] = useState<string | null>(null);
  const [editGoal, setEditGoal] = useState("");
  const [editAssumptions, setEditAssumptions] = useState("");
  const [editSuccessCriteria, setEditSuccessCriteria] = useState("");

  useEffect(() => {
    loadPlans();
  }, [runId]);

  async function loadPlans() {
    try {
      setLoading(true);
      const data = await listAgentPlansForRun(runId);
      setPlans(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function doAction(
    planId: string,
    fn: (id: string) => Promise<PlanOperationResult>,
    label: string
  ) {
    setActingId(planId);
    setError(null);
    try {
      const res = await fn(planId);
      setResult(res);
      await loadPlans();
    } catch (e) {
      setError(`${label}失败: ${String(e)}`);
    } finally {
      setActingId(null);
    }
  }

  function startEdit(plan: AgentPlan) {
    setEditingPlanId(plan.id);
    setEditGoal(plan.goal);
    setEditAssumptions(plan.assumptions.join("\n"));
    setEditSuccessCriteria(plan.successCriteria.join("\n"));
  }

  function cancelEdit() {
    setEditingPlanId(null);
    setEditGoal("");
    setEditAssumptions("");
    setEditSuccessCriteria("");
  }

  async function saveEdit(planId: string) {
    setActingId(planId);
    setError(null);
    try {
      const edit: EditPlanRequest = {
        goal: editGoal.trim() || undefined,
        assumptions: editAssumptions
          .split("\n")
          .map(s => s.trim())
          .filter(Boolean),
        successCriteria: editSuccessCriteria
          .split("\n")
          .map(s => s.trim())
          .filter(Boolean),
      };
      const res = await editAgentPlan(planId, edit);
      setResult(res);
      cancelEdit();
      await loadPlans();
    } catch (e) {
      setError(`编辑失败: ${String(e)}`);
    } finally {
      setActingId(null);
    }
  }

  if (loading) {
    return (
      <div className="mb-6">
        <h3 className="text-sm font-semibold text-stone-700 mb-2 flex items-center gap-2">
          <ListOrdered size={14} /> 计划
        </h3>
        <div className="text-xs text-stone-400 py-2">加载中...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="mb-6">
        <h3 className="text-sm font-semibold text-stone-700 mb-2 flex items-center gap-2">
          <ListOrdered size={14} /> 计划
        </h3>
        <div className="text-xs text-red-500">{error}</div>
      </div>
    );
  }

  if (plans.length === 0) {
    return (
      <div className="mb-6">
        <h3 className="text-sm font-semibold text-stone-700 mb-2 flex items-center gap-2">
          <ListOrdered size={14} /> 计划
        </h3>
        <div className="text-xs text-stone-400 py-2">此运行中没有关联计划。</div>
      </div>
    );
  }

  return (
    <div className="mb-6">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-stone-700 flex items-center gap-2">
          <ListOrdered size={14} /> 计划 ({plans.length})
        </h3>
        <button onClick={loadPlans} className="text-xs text-stone-500 hover:text-stone-700">
          刷新
        </button>
      </div>

      {result && (
        <div
          className={`mb-3 rounded-lg px-3 py-2 text-xs ${
            result.success
              ? "bg-emerald-50 text-emerald-800 border border-emerald-200"
              : "bg-red-50 text-red-800 border border-red-200"
          }`}
        >
          {result.message ||
            `${result.operation}: ${result.success ? "成功" : "失败"} (状态: ${result.status})`}
          {result.reviewVerdict && <span className="ml-2">审查: {result.reviewVerdict}</span>}
        </div>
      )}

      <div className="space-y-3">
        {plans.map(plan => {
          const isExpanded = expandedPlan === plan.id;
          const isActing = actingId === plan.id;

          return (
            <div
              key={plan.id}
              className={`rounded-lg border p-4 ${
                plan.riskLevel === "high" || plan.riskLevel === "critical"
                  ? "border-rose-200 bg-rose-50/30"
                  : "border-stone-200 bg-white"
              }`}
            >
              {/* Plan header */}
              <button
                onClick={() => setExpandedPlan(isExpanded ? null : plan.id)}
                className="w-full text-left"
              >
                <div className="flex items-center gap-3">
                  <span
                    className={`shrink-0 w-2 h-2 rounded-full ${
                      plan.status === "completed"
                        ? "bg-emerald-400"
                        : plan.status === "executing"
                          ? "bg-blue-400 animate-pulse"
                          : plan.status === "confirmed"
                            ? "bg-indigo-400"
                            : plan.status === "failed" || plan.status === "failed_review"
                              ? "bg-red-400"
                              : plan.status === "rejected" || plan.status === "cancelled"
                                ? "bg-stone-400"
                                : "bg-amber-400"
                    }`}
                  />
                  <span className="font-medium text-stone-800 text-sm flex-1 truncate">
                    {plan.goal}
                  </span>
                  <span
                    className={`text-[10px] px-2 py-0.5 rounded-full ${
                      plan.riskLevel === "high" || plan.riskLevel === "critical"
                        ? "bg-rose-100 text-rose-700"
                        : plan.riskLevel === "medium"
                          ? "bg-amber-100 text-amber-700"
                          : "bg-emerald-100 text-emerald-700"
                    }`}
                  >
                    {riskLabel(plan.riskLevel)}
                  </span>
                  <span className="text-[10px] text-stone-500 bg-stone-100 px-2 py-0.5 rounded-full">
                    {statusLabel(plan.status)}
                  </span>
                  {plan.requiresConfirmation && (
                    <span className="text-[10px] text-orange-600 bg-orange-50 px-1.5 py-0.5 rounded border border-orange-100">
                      需确认
                    </span>
                  )}
                  {isExpanded ? (
                    <ChevronUp size={16} className="text-stone-400" />
                  ) : (
                    <ChevronDown size={16} className="text-stone-400" />
                  )}
                </div>
              </button>

              {/* Expanded detail */}
              {isExpanded && (
                <div className="mt-4 space-y-3 border-t border-stone-100 pt-3">
                  {/* Plan ID & timestamps */}
                  <div className="flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-stone-400">
                    <span>ID: {plan.id.slice(0, 8)}...</span>
                    <span>创建: {new Date(plan.createdAt).toLocaleString("zh-CN")}</span>
                    {plan.confirmedAt && (
                      <span>确认: {new Date(plan.confirmedAt).toLocaleString("zh-CN")}</span>
                    )}
                    {plan.completedAt && (
                      <span>完成: {new Date(plan.completedAt).toLocaleString("zh-CN")}</span>
                    )}
                  </div>

                  {/* Assumptions */}
                  {plan.assumptions.length > 0 && (
                    <div className="text-xs">
                      <div className="font-medium text-stone-600 mb-1">前提假设</div>
                      <ul className="list-disc pl-4 space-y-0.5 text-stone-500">
                        {plan.assumptions.map((a, i) => (
                          <li key={i}>{a}</li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {/* Missing context */}
                  {plan.missingContext.length > 0 && (
                    <div className="text-xs">
                      <div className="font-medium text-amber-600 mb-1 flex items-center gap-1">
                        <AlertTriangle size={12} /> 缺失上下文
                      </div>
                      <ul className="list-disc pl-4 space-y-0.5 text-amber-700">
                        {plan.missingContext.map((m, i) => (
                          <li key={i}>{m}</li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {/* Steps */}
                  {plan.steps.length > 0 && (
                    <div className="text-xs">
                      <div className="font-medium text-stone-600 mb-2">
                        步骤 ({plan.steps.length})
                      </div>
                      <div className="space-y-1.5">
                        {plan.steps.map(step => (
                          <div
                            key={step.index}
                            className="flex items-start gap-2 rounded bg-stone-50 px-3 py-2"
                          >
                            <span className="shrink-0 w-5 h-5 rounded-full bg-stone-200 text-stone-600 text-[10px] flex items-center justify-center font-medium">
                              {step.index + 1}
                            </span>
                            <div className="flex-1 min-w-0">
                              <div className="text-stone-700">{step.description}</div>
                              <div className="flex flex-wrap gap-x-3 gap-y-0.5 mt-1 text-[10px] text-stone-400">
                                {step.toolIntent && <span>工具: {step.toolIntent}</span>}
                                {step.expectedOutput && (
                                  <span>预期输出: {step.expectedOutput}</span>
                                )}
                                {step.dependsOn.length > 0 && (
                                  <span>依赖步骤: {step.dependsOn.map(d => d + 1).join(", ")}</span>
                                )}
                              </div>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Tool Intents */}
                  {plan.toolIntents.length > 0 && (
                    <div className="text-xs">
                      <div className="font-medium text-stone-600 mb-2">工具意图</div>
                      <div className="space-y-1">
                        {plan.toolIntents.map((ti, i) => (
                          <div
                            key={i}
                            className="flex items-center gap-2 rounded bg-stone-50 px-3 py-1.5"
                          >
                            <WrenchIcon size={12} className="text-stone-400" />
                            <span className="font-mono text-stone-700">{ti.toolName}</span>
                            <span className="text-stone-400">— {ti.purpose}</span>
                            <span
                              className={`text-[10px] px-1.5 py-0.5 rounded ${
                                ti.riskLevel === "high"
                                  ? "bg-rose-100 text-rose-600"
                                  : "bg-stone-100 text-stone-500"
                              }`}
                            >
                              {riskLabel(ti.riskLevel)}
                            </span>
                            {ti.isWrite && (
                              <span className="text-[10px] text-amber-600 bg-amber-50 px-1 py-0.5 rounded">
                                写操作
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Deviations */}
                  {plan.status === "completed" || plan.status === "executing" ? (
                    <div className="text-xs rounded bg-amber-50/50 border border-amber-100 px-3 py-2">
                      <div className="font-medium text-amber-700 mb-1 flex items-center gap-1">
                        <Info size={12} />
                        {plan.status === "executing" ? "执行中" : "执行完成"}
                      </div>
                      <div className="text-amber-600">
                        {plan.status === "executing"
                          ? "计划正在执行中。如有偏差，将记录在此处。"
                          : "计划已执行完成。可查看执行轨迹了解详情。"}
                      </div>
                    </div>
                  ) : null}

                  {/* Permission requirements */}
                  {plan.permissionRequirements.length > 0 && (
                    <div className="text-xs rounded bg-orange-50 border border-orange-100 px-3 py-2">
                      <div className="font-medium text-orange-700 mb-1 flex items-center gap-1">
                        <Shield size={12} /> 权限需求
                      </div>
                      <div className="space-y-1">
                        {plan.permissionRequirements.map((pr, i) => (
                          <div key={i} className="flex items-center gap-2 text-orange-600">
                            <span>{pr.target}</span>
                            <span className="text-[10px]">{pr.reason}</span>
                            <Link
                              to="/review"
                              className="text-orange-700 hover:underline inline-flex items-center gap-1"
                            >
                              <Eye size={10} /> 查看权限
                            </Link>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {plan.rollbackPlan && (
                    <div className="text-xs rounded bg-slate-50 border border-slate-200 px-3 py-2">
                      <div className="font-medium text-slate-600 mb-1">回滚计划</div>
                      <div className="text-slate-500">{plan.rollbackPlan}</div>
                    </div>
                  )}

                  {/* Success criteria */}
                  {plan.successCriteria.length > 0 && (
                    <div className="text-xs">
                      <div className="font-medium text-stone-600 mb-1">成功标准</div>
                      <ul className="list-disc pl-4 space-y-0.5 text-stone-500">
                        {plan.successCriteria.map((sc, i) => (
                          <li key={i}>{sc}</li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {/* Action buttons */}
                  <div className="flex flex-wrap gap-2 pt-2 border-t border-stone-100">
                    {/* Edit button — only for draft/published plans */}
                    {(plan.status === "draft" || plan.status === "published") &&
                      (editingPlanId !== plan.id ? (
                        <button
                          onClick={() => startEdit(plan)}
                          disabled={isActing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-stone-200 text-stone-600 text-xs hover:bg-stone-50 disabled:opacity-50"
                        >
                          <Edit2 size={12} />
                          编辑
                        </button>
                      ) : null)}

                    {LEGAL_CONFIRM_STATES.includes(plan.status) && (
                      <>
                        <button
                          onClick={() => doAction(plan.id, confirmAgentPlan, "确认")}
                          disabled={isActing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600 text-white text-xs hover:bg-indigo-700 disabled:opacity-50"
                        >
                          <Check size={12} />
                          {isActing ? "确认中..." : "确认计划"}
                        </button>
                        <button
                          onClick={() => doAction(plan.id, rejectAgentPlan, "拒绝")}
                          disabled={isActing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-rose-200 text-rose-700 text-xs hover:bg-rose-50 disabled:opacity-50"
                        >
                          <X size={12} />
                          {isActing ? "拒接中..." : "拒绝"}
                        </button>
                      </>
                    )}

                    {LEGAL_EXECUTE_STATES.includes(plan.status) && (
                      <button
                        onClick={() => doAction(plan.id, executeAgentPlan, "执行")}
                        disabled={isActing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-600 text-white text-xs hover:bg-emerald-700 disabled:opacity-50"
                      >
                        <Play size={12} />
                        {isActing ? "执行中..." : "执行计划"}
                      </button>
                    )}

                    {LEGAL_CANCEL_STATES.includes(plan.status) && (
                      <button
                        onClick={() => doAction(plan.id, cancelAgentPlan, "取消")}
                        disabled={isActing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-stone-300 text-stone-600 text-xs hover:bg-stone-50 disabled:opacity-50"
                      >
                        <Ban size={12} />
                        {isActing ? "取消中..." : "取消"}
                      </button>
                    )}

                    {LEGAL_RETRY_STATES.includes(plan.status) && (
                      <button
                        onClick={() => doAction(plan.id, retryAgentPlan, "重试")}
                        disabled={isActing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-100 text-amber-700 text-xs hover:bg-amber-200 disabled:opacity-50"
                      >
                        <RotateCcw size={12} />
                        {isActing ? "重试中..." : "重试"}
                      </button>
                    )}

                    {plan.status === "executing" && plan.toolIntents.some(ti => ti.isWrite) && (
                      <Link
                        to="/review"
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-orange-200 text-orange-700 text-xs hover:bg-orange-50"
                      >
                        <Eye size={12} />
                        查看写操作权限
                      </Link>
                    )}
                  </div>

                  {/* Inline edit form */}
                  {editingPlanId === plan.id && (
                    <div className="mt-3 space-y-3 p-3 rounded-lg border border-stone-200 bg-stone-50">
                      <div className="text-xs font-medium text-stone-700">编辑计划</div>
                      <div>
                        <label className="text-[10px] text-stone-500">目标</label>
                        <textarea
                          value={editGoal}
                          onChange={e => setEditGoal(e.target.value)}
                          className="w-full mt-1 border border-stone-200 rounded px-2 py-1 text-xs"
                          rows={2}
                        />
                      </div>
                      <div>
                        <label className="text-[10px] text-stone-500">前提假设（每行一项）</label>
                        <textarea
                          value={editAssumptions}
                          onChange={e => setEditAssumptions(e.target.value)}
                          className="w-full mt-1 border border-stone-200 rounded px-2 py-1 text-xs"
                          rows={3}
                        />
                      </div>
                      <div>
                        <label className="text-[10px] text-stone-500">成功标准（每行一项）</label>
                        <textarea
                          value={editSuccessCriteria}
                          onChange={e => setEditSuccessCriteria(e.target.value)}
                          className="w-full mt-1 border border-stone-200 rounded px-2 py-1 text-xs"
                          rows={3}
                        />
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => saveEdit(plan.id)}
                          disabled={isActing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-indigo-600 text-white text-xs hover:bg-indigo-700 disabled:opacity-50"
                        >
                          <Check size={12} />
                          {isActing ? "保存中..." : "保存"}
                        </button>
                        <button
                          onClick={cancelEdit}
                          disabled={isActing}
                          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-stone-200 text-stone-600 text-xs hover:bg-stone-50"
                        >
                          <X size={12} />
                          取消
                        </button>
                      </div>
                      <div className="text-[10px] text-stone-400">
                        编辑后保持当前状态。仅 draft/published 可编辑。
                      </div>
                    </div>
                  )}

                  {/* Continue for blocked actions */}
                  {plan.status === "confirmed" && (
                    <button
                      onClick={() => doAction(plan.id, continueAgentPlan, "继续")}
                      disabled={isActing}
                      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-blue-200 text-blue-600 text-xs hover:bg-blue-50 disabled:opacity-50"
                    >
                      <Play size={12} />
                      {isActing ? "继续中..." : "继续已阻断的操作"}
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function WrenchIcon({ size, className }: { size: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
    >
      <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
    </svg>
  );
}
