import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  Sparkles,
  ChevronDown,
  ChevronUp,
  Activity,
  MessageCircle,
  MousePointer,
  Brain,
  GitCommit,
  Info,
} from "lucide-react";
import {
  generateCalibrationReport,
  generateMicroEvolutionChanges,
  applyCalibration,
  markCalibrationShown,
  getLifeModel,
  type EvolutionChange,
  type EvolutionSignalSummary,
} from "../tauri";
import type { LifeModel } from "../types";
import LoadingSpinner from "../components/LoadingSpinner";
import EmptyState from "../components/EmptyState";
import ErrorBanner from "../components/ErrorBanner";

interface CalibrationData {
  report: {
    period_days: number;
    feedback_up: number;
    feedback_down: number;
    top_liked_patterns: string[];
    top_disliked_patterns: string[];
    value_changes: string[];
    suggested_actions: string[];
    summary_text: string;
  } | null;
  changes: EvolutionChange[];
  applied: boolean;
  message: string;
  before?: { identity: number; goals: number; capabilities: number; state: number; overall: number };
  after?: { identity: number; goals: number; capabilities: number; state: number; overall: number };
  requires_confirmation?: boolean;
  signal_summary?: EvolutionSignalSummary;
}

// Progress bar component for visual comparison
function ProgressBar({
  before,
  after,
  label,
  max = 100,
}: {
  before: number;
  after: number;
  label: string;
  max?: number;
}) {
  const beforePct = Math.min((before / max) * 100, 100);
  const afterPct = Math.min((after / max) * 100, 100);
  const delta = after - before;
  const increased = delta > 0;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-xs">
        <span className="text-gray-600 font-medium">{label}</span>
        <span className={`font-semibold ${increased ? "text-green-600" : delta < 0 ? "text-orange-600" : "text-gray-500"}`}>
          {before}% → {after}%
          {delta !== 0 && (
            <span className="ml-1">({increased ? "+" : ""}{delta}%)</span>
          )}
        </span>
      </div>
      <div className="relative h-3 bg-gray-100 rounded-full overflow-hidden">
        {/* Before bar */}
        <div
          className="absolute top-0 left-0 h-full bg-gray-300 rounded-full transition-all"
          style={{ width: `${beforePct}%` }}
        />
        {/* After bar */}
        <div
          className={`absolute top-0 left-0 h-full rounded-full transition-all ${
            increased ? "bg-green-400" : delta < 0 ? "bg-orange-400" : "bg-blue-400"
          }`}
          style={{ width: `${afterPct}%` }}
        />
      </div>
    </div>
  );
}

function sourceIcon(source: string) {
  switch (source) {
    case "feedback":
      return <MessageCircle size={14} className="text-emerald-500" />;
    case "behavior":
      return <MousePointer size={14} className="text-blue-500" />;
    case "inference":
      return <Brain size={14} className="text-purple-500" />;
    default:
      return <Activity size={14} className="text-gray-400" />;
  }
}

function sourceLabel(source: string) {
  switch (source) {
    case "feedback":
      return "反馈信号";
    case "behavior":
      return "行为记录";
    case "inference":
      return "对话推断";
    default:
      return source;
  }
}

function sourceColorClass(source: string) {
  switch (source) {
    case "feedback":
      return "bg-emerald-50 text-emerald-700 border-emerald-100";
    case "behavior":
      return "bg-blue-50 text-blue-700 border-blue-100";
    case "inference":
      return "bg-purple-50 text-purple-700 border-purple-100";
    default:
      return "bg-gray-50 text-gray-700 border-gray-100";
  }
}

function dimensionLabel(d: string) {
  if (d === "identity.values") return "价值观权重";
  if (d === "goals") return "目标优先级";
  if (d === "capabilities.skills") return "技能熟练度";
  return d;
}

function buildSourceExplanation(sources: { source: string; score: number; weight: number }[]) {
  if (!sources || sources.length === 0) return "";
  const totalWeight = sources.reduce((s, item) => s + item.weight, 0);
  const items = sources
    .map((s) => ({
      label: s.source === "feedback" ? "反馈" : s.source === "behavior" ? "行为记录" : "对话推断",
      pct: totalWeight > 0 ? Math.round((s.weight / totalWeight) * 100) : 0,
    }))
    .sort((a, b) => b.pct - a.pct);
  if (items.length === 1) return `这个建议完全基于「${items[0].label}」信号。`;
  const main = items[0];
  const rest = items.slice(1);
  return `这个建议主要依据「${main.label}」（占 ${main.pct}%），同时参考了${rest.map((r) => `「${r.label}」（${r.pct}%）`).join("、")}。`;
}

function isHighImpactChange(change: EvolutionChange) {
  return change.dimension.includes("identity") || change.dimension.includes("mission") || change.dimension.includes("long_term");
}

export default function CalibrationPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(true);
  const [model, setModel] = useState<LifeModel | null>(null);
  const [data, setData] = useState<CalibrationData>({
    report: null,
    changes: [],
    applied: false,
    message: "",
  });
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [applyLoading, setApplyLoading] = useState(false);
  const [error, setError] = useState<string>("");
  const [pageError, setPageError] = useState<string>("");
  const [expandedItems, setExpandedItems] = useState<Set<number>>(new Set());

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [m, report, evolution] = await Promise.all([
          getLifeModel(),
          generateCalibrationReport(7),
          generateMicroEvolutionChanges(),
        ]);
        if (cancelled) return;
        setModel(m);
        setData({
          report,
          changes: evolution.changes,
          applied: evolution.applied,
          message: evolution.message,
          before: evolution.before,
          after: evolution.after,
          requires_confirmation: evolution.requires_confirmation,
          signal_summary: evolution.signal_summary,
        });
        setSelected(new Set(evolution.changes.map((change, i) => isHighImpactChange(change) ? -1 : i).filter((i) => i >= 0)));
      } catch (e: any) {
        setError(String(e?.message ?? e));
      } finally {
        setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const toggleItem = (idx: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const toggleExpand = (idx: number) => {
    setExpandedItems((prev) => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const handleApply = async () => {
    if (!model) return;
    const toApply = Array.from(selected)
      .sort((a, b) => a - b)
      .map((i) => data.changes[i]);
    if (toApply.length === 0) {
      setPageError("请先选择至少一项变更");
      return;
    }
    setApplyLoading(true);
    setPageError("");
    try {
      const result = await applyCalibration(toApply);
      await markCalibrationShown("weekly");
      setPageError(result.message);
      setTimeout(() => navigate("/dashboard"), 1200);
    } catch (e: any) {
      setPageError(String(e?.message ?? e));
    } finally {
      setApplyLoading(false);
    }
  };

  const handleRejectAll = async () => {
    await markCalibrationShown("weekly");
    navigate("/dashboard");
  };

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <LoadingSpinner text="正在生成校准报告与微进化建议…" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center p-6">
        <div className="max-w-lg w-full rounded-xl border border-amber-100 bg-amber-50 p-6 text-center">
          <EmptyState title="校准报告生成失败" description={error} className="py-0" />
          <p className="mt-3 text-sm text-amber-800">
            这通常与人生模型读取、反馈数据不足或模型服务配置有关。你可以稍后重试，或去设置页查看“试用就绪检查”。
          </p>
          <div className="mt-5 flex justify-center gap-3">
            <button
              onClick={() => window.location.reload()}
              className="rounded-md bg-amber-600 px-4 py-2 text-sm font-medium text-white hover:bg-amber-700"
            >
              稍后重试
            </button>
            <Link
              to="/settings"
              className="rounded-md bg-white px-4 py-2 text-sm font-medium text-amber-700 border border-amber-100 hover:bg-amber-50"
            >
              去设置页检查模型
            </Link>
          </div>
        </div>
      </div>
    );
  }

  const report = data.report;

  return (
    <div className="h-full overflow-auto bg-gray-50 p-6">
      <div className="max-w-5xl mx-auto space-y-6">
        <ErrorBanner message={pageError} onClose={() => setPageError("")} severity={pageError.includes("成功") ? "info" : "error"} />
        {/* Header */}
        <div className="bg-white rounded-xl shadow p-6 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Sparkles size={20} className="text-indigo-500" />
              <h2 className="text-xl font-bold text-indigo-700">周期校准</h2>
            </div>
            <div className="text-sm text-gray-500">
              {report ? `统计周期：近 ${report.period_days} 天` : "无报告数据"}
            </div>
          </div>
          {report && (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
              <div className="bg-green-50 border border-green-100 rounded-lg p-4">
                <div className="text-sm text-green-700">正面反馈</div>
                <div className="text-2xl font-semibold text-green-800">{report.feedback_up}</div>
              </div>
              <div className="bg-red-50 border border-red-100 rounded-lg p-4">
                <div className="text-sm text-red-700">负面反馈</div>
                <div className="text-2xl font-semibold text-red-800">{report.feedback_down}</div>
              </div>
              <div className="bg-indigo-50 border border-indigo-100 rounded-lg p-4 col-span-2">
                <div className="text-sm text-indigo-700">摘要</div>
                <div className="text-sm text-gray-700 mt-1">{report.summary_text}</div>
              </div>
            </div>
          )}
        </div>

        {/* Changes Section */}
        <div className="bg-white rounded-xl shadow p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold text-gray-800">建议变更</h3>
            <div className="flex items-center gap-1 text-xs text-gray-500">
              <GitCommit size={14} />
              应用前自动创建快照
            </div>
          </div>
          <div className="rounded-lg border border-indigo-100 bg-indigo-50 px-4 py-3 text-sm text-indigo-800">
            身份、使命、长期目标等高影响字段默认不勾选，需要你手动确认。每条建议都显示了信号来源和置信度。
          </div>

          {/* Signal Summary */}
          {data.signal_summary && (
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
              <div className="rounded-lg border border-indigo-100 bg-indigo-50/60 p-4">
                <div className="flex items-center gap-2 text-sm font-medium text-indigo-700">
                  <Activity size={16} />
                  融合信号概览
                </div>
                <div className="mt-2 space-y-1 text-xs text-gray-700">
                  <div>反馈关键词：{data.signal_summary.feedback_terms}</div>
                  <div>行为事件：{data.signal_summary.behavior_events}</div>
                  <div>对话推断：{data.signal_summary.inference_items}</div>
                </div>
                <div className="mt-2 text-[10px] text-indigo-600/80 leading-relaxed">
                  这三类信号分别来自：你对 AI 回复的点赞/点踩（反馈）、你在对话中提到的行为和状态变化（行为）、以及 AI 从对话内容中推断出的隐含偏好（推断）。
                </div>
              </div>
              <div className="rounded-lg border border-emerald-100 bg-emerald-50/60 p-4">
                <div className="flex items-center gap-2 text-sm font-medium text-emerald-700">
                  <MessageCircle size={16} className="text-emerald-500" />
                  最强反馈信号
                </div>
                <div className="mt-2 space-y-1 text-xs text-gray-700">
                  {(data.signal_summary.top_feedback.length > 0 ? data.signal_summary.top_feedback : [{ name: "暂无", score: 0, source: "feedback" }]).map((item) => (
                    <div key={`${item.source}-${item.name}`}>{item.name} · {item.score.toFixed(2)}</div>
                  ))}
                </div>
              </div>
              <div className="rounded-lg border border-amber-100 bg-amber-50/60 p-4">
                <div className="flex items-center gap-2 text-sm font-medium text-amber-700">
                  <Brain size={16} className="text-amber-500" />
                  最强对话/行为信号
                </div>
                <div className="mt-2 space-y-1 text-xs text-gray-700">
                  {[...data.signal_summary.top_behavior, ...data.signal_summary.top_inference].slice(0, 3).map((item) => (
                    <div key={`${item.source}-${item.name}`}>{item.name} · {item.score.toFixed(2)}</div>
                  ))}
                  {data.signal_summary.top_behavior.length === 0 && data.signal_summary.top_inference.length === 0 && <div>暂无</div>}
                </div>
              </div>
            </div>
          )}

          {/* Before/After Progress Bars */}
          {data.before && data.after && (
            <div className="rounded-lg border border-gray-100 bg-gray-50/50 p-4 space-y-3">
              <div className="text-sm font-medium text-gray-700">四维完整度变化</div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <ProgressBar
                  label="Identity（身份认同）"
                  before={data.before.identity}
                  after={data.after.identity}
                />
                <ProgressBar
                  label="Goals（目标体系）"
                  before={data.before.goals}
                  after={data.after.goals}
                />
                <ProgressBar
                  label="Capabilities（能力资源）"
                  before={data.before.capabilities}
                  after={data.after.capabilities}
                />
                <ProgressBar
                  label="State（当前状态）"
                  before={data.before.state}
                  after={data.after.state}
                />
              </div>
            </div>
          )}

          {/* Change Items */}
          {data.changes.length === 0 ? (
            <div className="text-gray-500 text-sm">{data.message || "近7天暂无足够信号生成微调建议"}</div>
          ) : (
            <div className="space-y-3">
              {data.changes.map((c, idx) => {
                const isSelected = selected.has(idx);
                const expanded = expandedItems.has(idx);
                const increased = c.new_value > c.old_value;
                const highImpact = isHighImpactChange(c);
                return (
                  <div
                    key={idx}
                    className={`border rounded-lg transition ${
                      isSelected
                        ? "border-indigo-300 bg-indigo-50"
                        : "border-gray-200 bg-white"
                    }`}
                  >
                    {/* Header row */}
                    <div
                      onClick={() => toggleItem(idx)}
                      className="cursor-pointer p-4 flex items-start gap-3"
                    >
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggleItem(idx)}
                        onClick={(e) => e.stopPropagation()}
                        className="mt-1"
                      />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-xs px-2 py-0.5 rounded bg-gray-100 text-gray-700">
                            {dimensionLabel(c.dimension)}
                          </span>
                          <span className="font-medium text-gray-800">{c.target_name}</span>
                          {typeof c.confidence === "number" && (
                            <span
                              className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                                c.confidence >= 0.8
                                  ? "bg-emerald-50 text-emerald-700"
                                  : c.confidence >= 0.5
                                  ? "bg-blue-50 text-blue-700"
                                  : "bg-amber-50 text-amber-700"
                              }`}
                              title="置信度由信号方向一致性与平均强度加权计算"
                            >
                              置信度 {Math.round(c.confidence * 100)}%
                            </span>
                          )}
                          <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${highImpact ? "bg-rose-50 text-rose-700" : "bg-slate-50 text-slate-600"}`}>
                            {highImpact ? "高影响·需手动确认" : "可选建议"}
                          </span>
                        </div>
                        <div className="mt-1 flex items-center gap-3 text-sm">
                          <span className="text-gray-500">{c.old_value.toFixed(2)}</span>
                          <span className="text-gray-300">→</span>
                          <span className={`font-semibold ${increased ? "text-green-600" : "text-orange-600"}`}>
                            {c.new_value.toFixed(2)}
                          </span>
                          <span className="text-xs text-gray-400">
                            {increased ? "↑" : "↓"} {(Math.abs(c.new_value - c.old_value)).toFixed(2)}
                          </span>
                        </div>
                        <div className="mt-1 text-xs text-gray-500">{c.reason}</div>
                      </div>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          toggleExpand(idx);
                        }}
                        className="text-gray-400 hover:text-gray-600 mt-1"
                        title="查看详细信号来源"
                      >
                        {expanded ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                      </button>
                    </div>

                    {/* Expanded details */}
                    {expanded && (
                      <div className="px-4 pb-4 pt-0 border-t border-gray-100">
                        <div className="mt-3 space-y-3">
                          <div className="text-xs font-medium text-gray-500">信号来源明细</div>
                          {c.sources && c.sources.length > 0 ? (
                            <div className="space-y-2">
                              {c.sources.map((s) => (
                                <div
                                  key={s.source}
                                  className={`flex items-center justify-between rounded-lg border px-3 py-2 text-xs ${sourceColorClass(s.source)}`}
                                >
                                  <div className="flex items-center gap-2">
                                    {sourceIcon(s.source)}
                                    <span className="font-medium">{sourceLabel(s.source)}</span>
                                  </div>
                                  <div className="flex items-center gap-3">
                                    <span>权重 {Math.round(s.weight * 100)}%</span>
                                    <span>强度 {s.score > 0 ? "+" : ""}{s.score.toFixed(2)}</span>
                                  </div>
                                </div>
                              ))}
                              <div className="text-[11px] text-gray-400 bg-gray-50 rounded-lg px-3 py-2">
                                <Info size={12} className="inline mr-1" />
                                {buildSourceExplanation(c.sources)}
                              </div>
                            </div>
                          ) : (
                            <div className="text-xs text-gray-400">无详细信号来源记录</div>
                          )}
                          <div className="text-[11px] text-gray-400">
                            影响字段：{c.dimension} / {c.target_name}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Action bar */}
        <div className="flex items-center justify-between bg-white rounded-xl shadow p-4">
          <button
            onClick={handleRejectAll}
            className="px-4 py-2 rounded-md text-sm font-medium text-gray-600 hover:bg-gray-100"
          >
            全部拒绝
          </button>
          <div className="flex items-center gap-3">
            <span className="text-sm text-gray-500">
              已选择 {selected.size} / {data.changes.length} 项
            </span>
            <button
              onClick={handleApply}
              disabled={applyLoading || selected.size === 0 || data.requires_confirmation === false}
              className="px-5 py-2 rounded-md text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50"
            >
              {applyLoading ? "应用中…" : "确认应用"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
