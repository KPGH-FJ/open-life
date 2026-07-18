import { useState } from "react";
import {
  Check,
  X,
  Edit2,
  AlertTriangle,
  Info,
  ShieldCheck,
  Sparkles,
  Target,
  User,
  Zap,
  Heart,
} from "lucide-react";
import type { BuilderSignal, BuilderSummary, BuilderSignalDecision } from "../tauri";
import SuggestionContextPanel from "./SuggestionContextPanel";

interface Props {
  signals: BuilderSignal[];
  summary: BuilderSummary;
  onCreateProposals: (decisions: BuilderSignalDecision[]) => void;
  onReject: () => void;
}

interface SignalEditState {
  [signalId: string]: unknown;
}

const riskPriority: Record<"low" | "medium" | "high", number> = {
  high: 0,
  medium: 1,
  low: 2,
};

function fieldPathLabel(path: string): string {
  const parts = path.split(".");
  const last = parts[parts.length - 1];
  if (parts.length >= 2 && parts[0] === "identity") {
    if (last === "values") return "价值观";
    if (last === "mission_statement") return "使命宣言";
    if (last === "primary_role") return "主角色";
    if (last === "voice_style") return "沟通风格";
    if (last === "name") return "名称";
  }
  if (parts.length >= 2 && parts[0] === "goals") {
    if (last === "short_term") return "短期目标";
    if (last === "medium_term") return "中期目标";
    if (last === "long_term") return "长期目标";
    if (last === "life_goals") return "人生目标";
    if (last === "daily") return "每日目标";
  }
  if (parts.length >= 2 && parts[0] === "capabilities") {
    if (last === "skills") return "技能";
    if (last === "resources") return "资源";
    if (last === "knowledge_domains") return "知识域";
    if (last === "tools") return "工具";
  }
  if (parts.length >= 2 && parts[0] === "state") {
    if (last === "current_focus") return "当前焦点";
    if (last === "emotional_state") return "情绪状态";
    if (last === "health_status") return "健康状态";
    if (last === "alerts") return "预警";
  }
  return last;
}

const dimensionIcons: Record<string, React.ReactNode> = {
  Identity: <User size={16} />,
  Goals: <Target size={16} />,
  Capabilities: <Zap size={16} />,
  State: <Heart size={16} />,
};

const dimensionLabels: Record<string, string> = {
  Identity: "Identity 我是谁",
  Goals: "Goals 我要去哪里",
  Capabilities: "Capabilities 我有什么",
  State: "State 我现在怎么样",
};

const riskConfig: Record<string, { color: string; label: string; defaultChecked: boolean }> = {
  low: {
    color: "text-green-600 bg-green-50 border-green-200",
    label: "低风险",
    defaultChecked: true,
  },
  medium: {
    color: "text-amber-600 bg-amber-50 border-amber-200",
    label: "中风险",
    defaultChecked: true,
  },
  high: {
    color: "text-rose-600 bg-rose-50 border-rose-200",
    label: "高风险",
    defaultChecked: false,
  },
};

function formatValue(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  if (Array.isArray(value)) {
    return value.map(formatValue).join("、");
  }
  if (typeof value === "object") {
    // Try to extract name or description from object
    const obj = value as Record<string, unknown>;
    if (obj.name) return String(obj.name);
    if (obj.description) return String(obj.description);
    return JSON.stringify(value);
  }
  return String(value);
}

function groupSignalsByDimension(signals: BuilderSignal[]): Record<string, BuilderSignal[]> {
  return signals.reduce(
    (acc, signal) => {
      const dim = signal.dimension;
      if (!acc[dim]) acc[dim] = [];
      acc[dim].push(signal);
      return acc;
    },
    {} as Record<string, BuilderSignal[]>
  );
}

export default function BuilderPatchReview({
  signals,
  summary,
  onCreateProposals,
  onReject,
}: Props) {
  // Track which signals are selected (checked)
  const [selected, setSelected] = useState<Set<string>>(() => {
    const initial = new Set<string>();
    signals.forEach(s => {
      if (riskConfig[s.risk_level]?.defaultChecked) {
        initial.add(s.id);
      }
    });
    return initial;
  });

  // Track edited values per signal id
  const [editedValues, setEditedValues] = useState<SignalEditState>({});
  const [editing, setEditing] = useState<string | null>(null);
  const [editValue, setEditValue] = useState<string>("");
  const [editErrors, setEditErrors] = useState<Record<string, string>>({});

  const grouped = groupSignalsByDimension(signals);
  const dimensions = Object.keys(grouped);

  const toggleSignal = (id: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const startEdit = (signal: BuilderSignal) => {
    setEditing(signal.id);
    const value = editedValues[signal.id] ?? signal.proposed_value;
    setEditValue(typeof value === "string" ? value : JSON.stringify(value, null, 2));
    setEditErrors(prev => ({ ...prev, [signal.id]: "" }));
  };

  const saveEdit = (signal: BuilderSignal) => {
    let nextValue: unknown = editValue;
    const original = signal.proposed_value;
    if (Array.isArray(original) || (original && typeof original === "object")) {
      try {
        nextValue = JSON.parse(editValue);
      } catch {
        setEditErrors(prev => ({ ...prev, [signal.id]: "JSON 格式无效，请修正后再保存。" }));
        return;
      }
    }
    setEditedValues(prev => ({ ...prev, [signal.id]: nextValue }));
    setEditing(null);
    setEditValue("");
    setEditErrors(prev => ({ ...prev, [signal.id]: "" }));
    // Ensure edited item is selected
    setSelected(prev => {
      const next = new Set(prev);
      next.add(signal.id);
      return next;
    });
  };

  const buildDecisions = (): BuilderSignalDecision[] => {
    return signals.map(signal => {
      const isSelected = selected.has(signal.id);
      const hasEdit = signal.id in editedValues;

      if (hasEdit && isSelected) {
        return {
          id: signal.id,
          status: "edited" as const,
          proposed_value: editedValues[signal.id],
        };
      }
      if (isSelected) {
        return {
          id: signal.id,
          status: "accepted" as const,
        };
      }
      return {
        id: signal.id,
        status: "rejected" as const,
      };
    });
  };

  const handleCreateProposals = () => {
    onCreateProposals(buildDecisions());
  };

  const acceptedCount = signals.filter(s => selected.has(s.id) && !(s.id in editedValues)).length;
  const editedCount = signals.filter(s => selected.has(s.id) && s.id in editedValues).length;
  const rejectedCount = signals.length - acceptedCount - editedCount;
  // Merged count will be computed after backend returns actual merge results
  const mergedCount = 0;

  return (
    <div className="space-y-6">
      {/* Header - 产品化标题 */}
      <div className="bg-gradient-to-r from-indigo-50 to-purple-50 rounded-xl p-5 border border-indigo-100">
        <div className="flex items-center gap-2 mb-2">
          <Sparkles className="text-indigo-600" size={22} />
          <h3 className="font-semibold text-indigo-900 text-lg">OpenLife 准备这样理解你</h3>
        </div>
        <p className="text-sm text-indigo-700">
          基于我们的对话，我整理了对你的理解。
          <strong>建议你发送到 Mailbox 逐条审阅后再写入</strong>
          ，这样你可以清楚地看到每一项变更的前后对比。
        </p>
        <div className="mt-3 text-xs text-indigo-600 bg-indigo-100/50 px-3 py-1.5 rounded-full inline-flex items-center gap-1">
          <Info size={12} />共 {signals.length} 条理解建议 · 高风险内容默认未勾选 · 请审阅后确认
        </div>
      </div>

      {/* Summary Cards - 展示应用结果统计 */}
      <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
        <div className="bg-white border rounded-lg p-3">
          <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
            <Check size={14} className="text-green-500" />
            <span>接受</span>
          </div>
          <div className="text-lg font-semibold text-green-600">{acceptedCount}</div>
          <div className="text-xs text-gray-500">发送确认</div>
        </div>
        <div className="bg-white border rounded-lg p-3">
          <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
            <Edit2 size={14} className="text-amber-500" />
            <span>编辑</span>
          </div>
          <div className="text-lg font-semibold text-amber-600">{editedCount}</div>
          <div className="text-xs text-gray-500">修改后发送</div>
        </div>
        <div className="bg-white border rounded-lg p-3">
          <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
            <ShieldCheck size={14} className="text-indigo-500" />
            <span>合并</span>
          </div>
          <div className="text-lg font-semibold text-indigo-600">{mergedCount}</div>
          <div className="text-xs text-gray-500">由 Mailbox 审阅</div>
        </div>
        <div className="bg-white border rounded-lg p-3">
          <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
            <X size={14} className="text-rose-500" />
            <span>跳过</span>
          </div>
          <div className="text-lg font-semibold text-rose-600">{rejectedCount}</div>
          <div className="text-xs text-gray-500">暂不写入</div>
        </div>
        <div className="bg-gradient-to-br from-indigo-50 to-purple-50 border border-indigo-100 rounded-lg p-3">
          <div className="flex items-center gap-1.5 text-xs text-indigo-600 mb-1">
            <Target size={14} />
            <span>总计</span>
          </div>
          <div className="text-lg font-semibold text-indigo-900">{signals.length}</div>
          <div className="text-xs text-indigo-500">条建议</div>
        </div>
      </div>

      {/* Signal List by Dimension */}
      <div className="space-y-4">
        {dimensions.map(dim => {
          const dimSignals = [...(grouped[dim] || [])].sort((a, b) => {
            const riskDiff = riskPriority[a.risk_level] - riskPriority[b.risk_level];
            if (riskDiff !== 0) return riskDiff;
            return b.confidence - a.confidence;
          });
          return (
            <div key={dim} className="border rounded-xl overflow-hidden bg-white">
              <div className="bg-gradient-to-r from-gray-50 to-gray-100 px-4 py-3 border-b flex items-center gap-2">
                {dimensionIcons[dim]}
                <span className="font-medium text-gray-800">{dimensionLabels[dim]}</span>
                <span className="text-xs text-gray-500 ml-auto">{dimSignals.length} 项建议</span>
              </div>
              <div className="divide-y divide-gray-100">
                {dimSignals.map(signal => {
                  const isSelected = selected.has(signal.id);
                  const risk = riskConfig[signal.risk_level];
                  const isEditing = editing === signal.id;
                  const isHighRisk = signal.risk_level === "high";

                  return (
                    <div
                      key={signal.id}
                      className={`p-4 flex items-start gap-3 transition-all ${
                        isSelected ? "bg-indigo-50/30" : "bg-white hover:bg-gray-50/50"
                      } ${isHighRisk && !isSelected ? "border-l-4 border-l-amber-400" : ""}`}
                    >
                      <input
                        type="checkbox"
                        checked={isSelected}
                        onChange={() => toggleSignal(signal.id)}
                        className="mt-1 w-4 h-4 text-indigo-600 rounded border-gray-300 focus:ring-indigo-500"
                      />
                      <div className="flex-1 min-w-0">
                        {/* Top meta row - 展示来源和置信度 */}
                        <div className="flex items-center gap-2 flex-wrap mb-2">
                          <span className="font-medium text-gray-900 text-sm">
                            {fieldPathLabel(signal.affected_path)}
                          </span>
                          <span
                            className={`rounded-full border px-2 py-0.5 text-[10px] font-medium ${risk.color}`}
                          >
                            {risk.label}
                          </span>
                          <span className="rounded-full border border-indigo-100 bg-indigo-50 px-2 py-0.5 text-[10px] font-medium text-indigo-700">
                            置信度 {Math.round(signal.confidence * 100)}%
                          </span>
                          <span className="rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5 text-[10px] font-medium text-slate-600">
                            来源 {signal.source_question_id}
                          </span>
                          <span
                            className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
                              isHighRisk
                                ? "bg-amber-100 text-amber-800"
                                : "bg-emerald-100 text-emerald-800"
                            }`}
                          >
                            {isHighRisk ? "需要你显式确认" : "默认已勾选"}
                          </span>
                        </div>

                        {isEditing ? (
                          <div className="mt-2 space-y-2">
                            {Array.isArray(signal.proposed_value) ||
                            (signal.proposed_value && typeof signal.proposed_value === "object") ? (
                              <>
                                <div className="text-[11px] text-indigo-700 bg-indigo-50 rounded-lg px-3 py-2">
                                  这是结构化字段，请保持合法
                                  JSON。保存后会按原结构写回，而不是降级成普通字符串。
                                </div>
                                <textarea
                                  value={editValue}
                                  onChange={e => setEditValue(e.target.value)}
                                  className="w-full min-h-28 px-3 py-2 text-xs font-mono border rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500"
                                  autoFocus
                                />
                              </>
                            ) : (
                              <input
                                type="text"
                                value={editValue}
                                onChange={e => setEditValue(e.target.value)}
                                className="w-full px-3 py-1.5 text-sm border rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500"
                                autoFocus
                              />
                            )}
                            {editErrors[signal.id] && (
                              <div className="text-xs text-rose-600">{editErrors[signal.id]}</div>
                            )}
                            <div className="flex gap-2">
                              <button
                                onClick={() => saveEdit(signal)}
                                className="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded-lg hover:bg-indigo-700"
                              >
                                保存
                              </button>
                              <button
                                onClick={() => setEditing(null)}
                                className="px-3 py-1.5 text-sm text-gray-600 hover:bg-gray-100 rounded-lg"
                              >
                                取消
                              </button>
                            </div>
                          </div>
                        ) : (
                          <div className="space-y-1.5">
                            <div className="flex items-start gap-2">
                              <span className="text-[10px] text-indigo-600 font-medium bg-indigo-50 px-1.5 py-0.5 rounded shrink-0 mt-0.5">
                                建议值
                              </span>
                              <div
                                className="text-sm text-gray-800 font-medium"
                                data-testid={`proposed-value-${signal.id}`}
                              >
                                {formatValue(editedValues[signal.id] ?? signal.proposed_value)}
                              </div>
                            </div>
                          </div>
                        )}

                        <div className="mt-3">
                          <SuggestionContextPanel
                            reason={signal.reason}
                            affectedPath={signal.affected_path}
                            sourceLabel={signal.source_question_id}
                            confidence={signal.confidence}
                            riskLabel={risk.label}
                            note={
                              isHighRisk
                                ? "这是高风险字段，默认不会自动勾选，只有在你明确认可后才会写入人生模型。"
                                : "这条建议会在保存时和你当前的人生模型一起判断，尽量避免误覆盖已有内容。"
                            }
                          />
                        </div>
                      </div>

                      {!isEditing && (
                        <button
                          onClick={() => startEdit(signal)}
                          className="p-1.5 text-gray-400 hover:text-indigo-600 hover:bg-indigo-50 rounded"
                          title="编辑"
                        >
                          <Edit2 size={14} />
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>

      {/* Backend-owned review context */}
      {summary.assumptions.length > 0 && (
        <div className="bg-blue-50 border border-blue-100 rounded-lg p-4">
          <div className="flex items-center gap-2 text-blue-800 font-medium mb-2">
            <Info size={16} />
            <span>本轮审阅依据</span>
          </div>
          <ul className="text-sm text-blue-700 space-y-1">
            {summary.assumptions.map((a, i) => (
              <li key={i}>• {a}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Recommended Next Steps */}
      {summary.recommended_next_steps.length > 0 && (
        <div className="bg-amber-50 border border-amber-100 rounded-lg p-4">
          <div className="flex items-center gap-2 text-amber-800 font-medium mb-2">
            <ShieldCheck size={16} />
            <span>建议的下一步</span>
          </div>
          <ul className="text-sm text-amber-700 space-y-1">
            {summary.recommended_next_steps.map((step, i) => (
              <li key={i}>• {step}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Actions */}
      <div className="flex flex-col gap-3 pt-4 border-t">
        <div className="flex items-center justify-between">
          <div className="text-sm text-gray-600">
            已接受 <span className="font-semibold text-indigo-600">{acceptedCount}</span> 项
            {editedCount > 0 && (
              <span className="ml-2">
                已编辑 <span className="font-semibold text-amber-600">{editedCount}</span> 项
              </span>
            )}
            {rejectedCount > 0 && (
              <span className="ml-2">
                已拒绝 <span className="font-semibold text-rose-600">{rejectedCount}</span> 项
              </span>
            )}
          </div>
          <button
            onClick={onReject}
            className="px-4 py-2 text-gray-600 hover:bg-gray-100 rounded-lg flex items-center gap-2"
          >
            <X size={18} />
            暂不保存
          </button>
        </div>
        <div className="flex items-center justify-end gap-3">
          <button
            onClick={handleCreateProposals}
            disabled={acceptedCount === 0 && editedCount === 0}
            className="px-5 py-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
          >
            <ShieldCheck size={18} />
            发送到 Mailbox
          </button>
        </div>
        <div className="text-xs text-gray-500 text-right">
          「发送到 Mailbox」会创建确认项，你可以在 Mailbox 逐条审阅后再确认写入。
        </div>
      </div>

      {/* High Risk Warning */}
      {signals.some(s => s.risk_level === "high" && !selected.has(s.id)) && (
        <div className="flex items-start gap-2 text-xs text-amber-600 bg-amber-50 p-3 rounded-lg">
          <AlertTriangle size={14} className="shrink-0 mt-0.5" />
          <span>
            你有未勾选的高风险字段（如长期目标、核心价值观等）。建议勾选后发送到 Mailbox 审阅。
          </span>
        </div>
      )}
    </div>
  );
}
