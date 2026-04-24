import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import {
  LayoutDashboard, Target, Brain, Zap, Hammer, ArrowRight,
  RefreshCw, ClipboardList, TrendingUp,
  Plus, Check, X, Edit2, Trash2, Activity, Tag, Compass, Sparkles
} from "lucide-react";
import type { LifeModel, DailyGoal, StateHistoryEntry, StateAlert, CustomStateDimension, LifeModelVersion } from "../types";
import {
  getLifeModel, searchMemory, getFeedbackSummary, countMemoryChunks,
  runMicroEvolution, generateCalibrationReport, goalCapabilityGapReport,
  identityGoalAlignmentReport, getDailyGoals, addDailyGoal, updateDailyGoal,
  deleteDailyGoal, toggleDailyGoal, recordState, getStateHistory, getStateAlerts,
  shouldShowCalibration, markCalibrationShown, listSnapshots, getSystemDiagnostics,
  type CapabilityGap, type AlignmentIssue, type SystemDiagnostics
} from "../tauri";
import { getModelEmptyState } from "../utils/modelEmpty";
import EmptyState from "../components/EmptyState";
import ErrorBanner from "../components/ErrorBanner";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";

type TrendDirection = "up" | "down" | "stable";

type StateTrendSummary = {
  latest: number;
  previous: number | null;
  delta: number;
  min: number;
  max: number;
  average: number;
  direction: TrendDirection;
  thresholdStatus: "high" | "low" | "normal" | "none";
  explanation: string;
  alertMessage?: string;
};

function RadarChart({ skills }: { skills: { name: string; value: number }[] }) {
  if (skills.length === 0) return <div className="text-sm text-gray-500">暂无能力数据</div>;
  const size = 200;
  const cx = size / 2;
  const cy = size / 2;
  const radius = 80;
  const total = skills.length;
  const points = skills.map((_, i) => {
    const angle = (Math.PI * 2 * i) / total - Math.PI / 2;
    const x = cx + radius * Math.cos(angle);
    const y = cy + radius * Math.sin(angle);
    return { x, y };
  });
  const valuePoints = skills.map((s, i) => {
    const angle = (Math.PI * 2 * i) / total - Math.PI / 2;
    const r = radius * Math.min(1, Math.max(0, s.value / 5));
    const x = cx + r * Math.cos(angle);
    const y = cy + r * Math.sin(angle);
    return { x, y };
  });
  const poly = valuePoints.map((p) => `${p.x},${p.y}`).join(" ");
  return (
    <div className="flex flex-col items-center">
      <svg width={size} height={size}>
        <circle cx={cx} cy={cy} r={radius} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        <circle cx={cx} cy={cy} r={radius * 0.6} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        <circle cx={cx} cy={cy} r={radius * 0.3} fill="none" stroke="#e5e7eb" strokeWidth={1} />
        {points.map((p, i) => (
          <line key={i} x1={cx} y1={cy} x2={p.x} y2={p.y} stroke="#e5e7eb" strokeWidth={1} />
        ))}
        <polygon points={poly} fill="rgba(99,102,241,0.2)" stroke="#6366f1" strokeWidth={2} />
        {valuePoints.map((p, i) => (
          <circle key={i} cx={p.x} cy={p.y} r={3} fill="#6366f1" />
        ))}
      </svg>
      <div className="flex flex-wrap justify-center gap-3 mt-3">
        {skills.map((s) => (
          <div key={s.name} className="flex items-center gap-1 text-xs text-gray-600">
            <span className="w-2 h-2 rounded-full bg-indigo-500" />
            {s.name}
          </div>
        ))}
      </div>
    </div>
  );
}

function MiniLineChart({ data, width = 280, height = 120 }: { data: StateHistoryEntry[]; width?: number; height?: number }) {
  if (data.length < 2) return <div className="text-sm text-gray-500">数据不足，无法绘制趋势</div>;
  const values = data.map((d) => d.value);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const pad = 10;
  const chartW = width - pad * 2;
  const chartH = height - pad * 2;
  const points = data.map((d, i) => {
    const x = pad + (i / (data.length - 1)) * chartW;
    const y = pad + chartH - ((d.value - min) / (Math.max(max - min, 0.0001))) * chartH;
    return { x, y };
  });
  const linePath = points.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
  const areaPath = `${linePath} L ${points[points.length - 1].x} ${height - pad} L ${points[0].x} ${height - pad} Z`;
  return (
    <svg width={width} height={height} className="overflow-visible">
      <path d={areaPath} fill="rgba(79, 70, 229, 0.12)" />
      <path d={linePath} fill="none" stroke="#4f46e5" strokeWidth={2.5} strokeLinecap="round" strokeLinejoin="round" />
      {points.map((p, i) => (
        <circle key={i} cx={p.x} cy={p.y} r={3} fill="#4f46e5" />
      ))}
      <text x={pad} y={height - 2} fontSize="10" fill="#9ca3af">{new Date(data[0].recorded_at).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" })}</text>
      <text x={width - pad} y={height - 2} textAnchor="end" fontSize="10" fill="#9ca3af">{new Date(data[data.length - 1].recorded_at).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" })}</text>
    </svg>
  );
}

function getTrendDirection(delta: number, range: number): TrendDirection {
  const threshold = Math.max(range * 0.08, 0.5);
  if (delta > threshold) return "up";
  if (delta < -threshold) return "down";
  return "stable";
}

function summarizeStateDimension(
  dimension: CustomStateDimension | null,
  history: StateHistoryEntry[],
  alerts: StateAlert[],
): StateTrendSummary | null {
  if (!dimension || history.length === 0) return null;
  const ordered = [...history].sort((a, b) => new Date(a.recorded_at).getTime() - new Date(b.recorded_at).getTime());
  const values = ordered.map((item) => item.value);
  const latest = values[values.length - 1];
  const previous = values.length > 1 ? values[values.length - 2] : null;
  const delta = previous === null ? 0 : latest - previous;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const average = values.reduce((sum, value) => sum + value, 0) / values.length;
  const direction = getTrendDirection(delta, max - min);
  const alertMessage = alerts.find((alert) => alert.dimension_name === dimension.name)?.message;

  let thresholdStatus: StateTrendSummary["thresholdStatus"] = "none";
  if (dimension.min_threshold !== undefined && latest < dimension.min_threshold) thresholdStatus = "low";
  if (dimension.max_threshold !== undefined && latest > dimension.max_threshold) thresholdStatus = "high";
  if (
    (dimension.min_threshold !== undefined || dimension.max_threshold !== undefined) &&
    thresholdStatus === "none"
  ) {
    thresholdStatus = "normal";
  }

  const directionText =
    direction === "up" ? "整体在回升" : direction === "down" ? "最近有下降趋势" : "最近比较平稳";
  const thresholdText =
    thresholdStatus === "high" ? "当前已经高于你设定的安全区间" :
    thresholdStatus === "low" ? "当前已经低于你设定的安全区间" :
    thresholdStatus === "normal" ? "当前仍在你设定的阈值区间内" :
    "目前还没有阈值约束";
  const explanation = `${dimension.name}${directionText}，最近 ${ordered.length} 条记录平均值为 ${average.toFixed(1)}${dimension.unit}，${thresholdText}。`;

  return {
    latest,
    previous,
    delta,
    min,
    max,
    average,
    direction,
    thresholdStatus,
    explanation,
    alertMessage,
  };
}

function trendBadge(direction: TrendDirection) {
  switch (direction) {
    case "up":
      return "bg-emerald-50 text-emerald-700 border-emerald-100";
    case "down":
      return "bg-rose-50 text-rose-700 border-rose-100";
    default:
      return "bg-slate-50 text-slate-600 border-slate-200";
  }
}

export default function DashboardPage() {
  const location = useLocation();
  const [model, setModel] = useState<LifeModel | null>(null);
  const [memories, setMemories] = useState<Array<{ chunk: any; score: number }>>([]);
  const [memoryQuery, setMemoryQuery] = useState("");
  const [feedback, setFeedback] = useState<{ total_messages: number; total_feedback_up: number; total_feedback_down: number; session_count: number } | null>(null);
  const [memoryCount, setMemoryCount] = useState<number>(0);
  const [calibration, setCalibration] = useState<{
    period_days: number; feedback_up: number; feedback_down: number;
    top_liked_patterns: string[]; top_disliked_patterns: string[];
    value_changes: string[]; suggested_actions: string[]; summary_text: string;
  } | null>(null);
  const [calibrationLoading, setCalibrationLoading] = useState(false);
  const [evolutionMsg, setEvolutionMsg] = useState<string | null>(null);
  const [gaps, setGaps] = useState<CapabilityGap[] | null>(null);
  const [alignments, setAlignments] = useState<AlignmentIssue[] | null>(null);

  // Daily goals
  const [dailyGoals, setDailyGoals] = useState<DailyGoal[]>([]);
  const [addingGoal, setAddingGoal] = useState(false);
  const [newGoalName, setNewGoalName] = useState("");
  const [editingGoalIndex, setEditingGoalIndex] = useState<number | null>(null);
  const [editGoalName, setEditGoalName] = useState("");

  // State
  const [dimensions, setDimensions] = useState<CustomStateDimension[]>([]);
  const [stateAlerts, setStateAlerts] = useState<StateAlert[]>([]);
  const [selectedDimension, setSelectedDimension] = useState<string | null>(null);
  const [dimensionHistory, setDimensionHistory] = useState<StateHistoryEntry[]>([]);
  const [showStateModal, setShowStateModal] = useState(false);
  const [stateInputName, setStateInputName] = useState("");
  const [stateInputValue, setStateInputValue] = useState("");
  const [stateInputUnit, setStateInputUnit] = useState("");
  const [stateInputNote, setStateInputNote] = useState("");
  const [stateInputMin, setStateInputMin] = useState("");
  const [stateInputMax, setStateInputMax] = useState("");
  const [stateInputAlertDays, setStateInputAlertDays] = useState("3");

  const [calibrationPrompt, setCalibrationPrompt] = useState<{ weekly: boolean; monthly: boolean } | null>(null);
  const [latestVersion, setLatestVersion] = useState<LifeModelVersion | null>(null);
  const [loadWarnings, setLoadWarnings] = useState<string[]>([]);
  const [calibrationError, setCalibrationError] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const navigate = useNavigate();

  const warningText = (label: string, error: unknown) => `${label}加载失败：${error instanceof Error ? error.message : String(error)}`;

  useEffect(() => {
    refreshAll();
    (async () => {
      try {
        const res = await shouldShowCalibration();
        if (res.weekly || res.monthly) {
          setCalibrationPrompt({ weekly: res.weekly, monthly: res.monthly });
        }
      } catch (e) {
        setLoadWarnings((prev) => [...prev, warningText("校准提醒", e)]);
      }
    })();
  }, []);

  useEffect(() => {
    const refreshDashboardContext = () => {
      refreshAll();
      shouldShowCalibration()
        .then((res) => {
          if (res.weekly || res.monthly) {
            setCalibrationPrompt({ weekly: res.weekly, monthly: res.monthly });
          } else {
            setCalibrationPrompt(null);
          }
        })
        .catch((e) => {
          setLoadWarnings((prev) => [...prev, warningText("校准提醒", e)]);
        });
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        refreshDashboardContext();
      }
    };
    window.addEventListener("focus", refreshDashboardContext);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("focus", refreshDashboardContext);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (!(location.state as { refreshFromBuilder?: boolean } | null)?.refreshFromBuilder) return;
    refreshAll();
    shouldShowCalibration()
      .then((res) => {
        if (res.weekly || res.monthly) {
          setCalibrationPrompt({ weekly: res.weekly, monthly: res.monthly });
        } else {
          setCalibrationPrompt(null);
        }
      })
      .catch((e) => {
        setLoadWarnings((prev) => [...prev, warningText("校准提醒", e)]);
      });
  }, [location.state]);

  useEffect(() => {
    if (dimensions.length === 0 || selectedDimension) return;
    openDimension(dimensions[0].name).catch((e) => {
      setLoadWarnings((prev) => [...prev, warningText("状态历史", e)]);
    });
  }, [dimensions, selectedDimension]);

  const dismissCalibrationPrompt = async () => {
    if (calibrationPrompt?.weekly) await markCalibrationShown("weekly");
    if (calibrationPrompt?.monthly) await markCalibrationShown("monthly");
    setCalibrationPrompt(null);
  };

  const refreshAll = async () => {
    const warnings: string[] = [];
    try {
      const m = await getLifeModel();
      setModel(m);
      setDimensions(m.state.custom_dimensions || []);
    } catch (e) { warnings.push(warningText("人生模型", e)); }
    try { setFeedback(await getFeedbackSummary()); } catch (e) { warnings.push(warningText("反馈统计", e)); }
    try { setMemoryCount(await countMemoryChunks()); } catch (e) { warnings.push(warningText("记忆统计", e)); }
    try { setGaps(await goalCapabilityGapReport()); } catch (e) { warnings.push(warningText("能力缺口", e)); }
    try { setAlignments(await identityGoalAlignmentReport()); } catch (e) { warnings.push(warningText("价值观一致性", e)); }
    try { setDailyGoals(await getDailyGoals()); } catch (e) { warnings.push(warningText("每日目标", e)); }
    try { setStateAlerts(await getStateAlerts()); } catch (e) { warnings.push(warningText("状态预警", e)); }
    try { setDiagnostics(await getSystemDiagnostics()); } catch (e) { warnings.push(warningText("系统诊断", e)); }
    try {
      const snaps = await listSnapshots();
      if (snaps.length > 0) {
        setLatestVersion(snaps[0]);
      }
    } catch (e) { warnings.push(warningText("版本快照", e)); }
    setLoadWarnings(warnings);
  };

  const handleMemorySearch = async () => {
    if (!memoryQuery.trim()) return;
    const res = await searchMemory(memoryQuery.trim(), 5);
    setMemories(res);
  };

  const handleRunMicroEvolution = async () => {
    setEvolutionMsg(null);
    try {
      const result = await runMicroEvolution();
      const suffix = result.snapshot_version ? ` 已创建快照 ${result.snapshot_version}` : "";
      setEvolutionMsg(`${result.message}${suffix}`);
      const m = await getLifeModel();
      setModel(m);
      const snaps = await listSnapshots();
      if (snaps.length > 0) setLatestVersion(snaps[0]);
    } catch (e: any) {
      setEvolutionMsg("微进化执行失败: " + (e?.message || String(e)));
    }
  };

  const handleGenerateCalibration = async () => {
    setCalibrationLoading(true);
    setCalibrationError(null);
    try {
      const report = await generateCalibrationReport(7);
      setCalibration(report);
    } catch (e: any) {
      setCalibrationError("生成校准报告失败: " + (e?.message || String(e)));
    } finally {
      setCalibrationLoading(false);
    }
  };

  // Daily goals handlers
  const onAddGoal = async () => {
    if (!newGoalName.trim()) return;
    await addDailyGoal(newGoalName.trim());
    setNewGoalName("");
    setAddingGoal(false);
    setDailyGoals(await getDailyGoals());
  };

  const onToggleGoal = async (idx: number) => {
    await toggleDailyGoal(idx);
    setDailyGoals(await getDailyGoals());
  };

  const onDeleteGoal = async (idx: number) => {
    await deleteDailyGoal(idx);
    setDailyGoals(await getDailyGoals());
  };

  const startEditGoal = (idx: number, name: string) => {
    setEditingGoalIndex(idx);
    setEditGoalName(name);
  };

  const onSaveEditGoal = async (idx: number) => {
    if (!editGoalName.trim()) return;
    await updateDailyGoal(idx, editGoalName.trim(), undefined);
    setEditingGoalIndex(null);
    setDailyGoals(await getDailyGoals());
  };

  // State handlers
  const openDimension = async (name: string) => {
    setSelectedDimension(name);
    const hist = await getStateHistory(name, 30);
    setDimensionHistory(hist);
  };

  const onRecordState = async () => {
    const name = stateInputName.trim();
    const val = parseFloat(stateInputValue);
    const unit = stateInputUnit.trim() || "单位";
    if (!name || Number.isNaN(val)) return;
    const minVal = stateInputMin.trim() ? parseFloat(stateInputMin.trim()) : undefined;
    const maxVal = stateInputMax.trim() ? parseFloat(stateInputMax.trim()) : undefined;
    const alertDays = stateInputAlertDays.trim() ? parseInt(stateInputAlertDays.trim(), 10) : undefined;
    await recordState(name, val, unit, stateInputNote.trim() || undefined, minVal, maxVal, alertDays);
    setShowStateModal(false);
    setStateInputName("");
    setStateInputValue("");
    setStateInputUnit("");
    setStateInputNote("");
    setStateInputMin("");
    setStateInputMax("");
    setStateInputAlertDays("3");
    // refresh
    const m = await getLifeModel();
    setDimensions(m.state.custom_dimensions || []);
    setStateAlerts(await getStateAlerts());
    if (selectedDimension === name) {
      setDimensionHistory(await getStateHistory(name, 30));
    }
  };

  const goals = model?.goals ?? null;
  const capabilities = model?.capabilities ?? null;

  const allGoals = [
    ...(goals?.short_term ?? []),
    ...(goals?.medium_term ?? []),
    ...(goals?.long_term ?? []),
    ...(goals?.life_goals ?? []),
  ];

  const completedGoals = allGoals.filter((g) => g.status === "completed").length;
  const goalProgress = allGoals.length ? (completedGoals / allGoals.length) * 100 : 0;

  const skillData = (capabilities?.skills ?? []).slice(0, 6).map((s) => ({
    name: s.name,
    value: s.proficiency ?? 3,
  }));

  const completedDaily = dailyGoals.filter((g) => g.done).length;
  const selectedDimensionModel = dimensions.find((dim) => dim.name === selectedDimension) ?? null;
  const trendSummary = summarizeStateDimension(selectedDimensionModel, dimensionHistory, stateAlerts);
  // Build recommendations based on model completion
  const builderCompletion = diagnostics?.builder_completion;
  const completionThreshold = 60; // Show recommendation if dimension is below 60%
  const dimensionLabels: Record<string, string> = {
    identity: "Identity",
    goals: "Goals",
    capabilities: "Capabilities",
    state: "State",
  };
  const lowestDimension = builderCompletion?.lowest_dimension;
  const lowestDimensionValue = lowestDimension ? (builderCompletion[lowestDimension as keyof typeof builderCompletion] as number) : 100;
  
  const builderRecommendations = (() => {
    if (!builderCompletion || getModelEmptyState(model, diagnostics)) return [];
    
    const recs: Array<{ title: string; detail: string; to: string }> = [];
    
    // Recommend completing the lowest dimension
    if (lowestDimension && lowestDimensionValue < completionThreshold) {
      const dimLabel = dimensionLabels[lowestDimension] || lowestDimension;
      recs.push({
        title: `完善 ${dimLabel} 维度`,
        detail: `${dimLabel} 完成度仅 ${Math.round(lowestDimensionValue)}%，通过渐进构建补充更多细节，可获得更精准的个性化建议。`,
        to: "/builder",
      });
    }
    
    // If overall completion is low, recommend quick build or incremental
    if (builderCompletion.overall < 40) {
      recs.push({
        title: "继续构建人生模型",
        detail: `整体完成度 ${Math.round(builderCompletion.overall)}%，建议通过渐进构建完善核心维度。`,
        to: "/builder",
      });
    }
    
    return recs;
  })();

  const nextActions = [
    diagnostics && diagnostics.model_empty && (diagnostics.pending_builder_review_sessions ?? 0) > 0
      ? {
          title: "先审阅待确认的构建建议",
          detail: `Builder 里还有 ${diagnostics.pending_builder_review_sessions} 个待确认 Review。先把这些建议应用掉，后续对话才会真正个性化。`,
          to: "/builder",
        }
      : diagnostics && diagnostics.model_empty && diagnostics.unfinished_builder_sessions > 0
      ? {
          title: "继续未完成的构建会话",
          detail: `Builder 里还有 ${diagnostics.unfinished_builder_sessions} 个待继续或待确认的会话。先把 Review 应用掉，后续对话才会真正个性化。`,
          to: "/builder",
        }
      : null,
    getModelEmptyState(model, diagnostics)
      ? { title: "先构建人生模型", detail: "模型为空时，Chat 和 Dashboard 都只能提供通用建议。", to: "/builder" }
      : null,
    ...builderRecommendations.map(r => r as { title: string; detail: string; to: string }),
    diagnostics && !diagnostics.chat_ready
      ? { title: "修复聊天配置", detail: diagnostics.readiness_issues[0] ?? "本地或云端模型尚未就绪。", to: "/settings" }
      : null,
    calibrationPrompt
      ? { title: "查看周期校准", detail: "有新的模型校准提醒，建议确认后再应用变更。", to: "/calibration" }
      : null,
    latestVersion
      ? { title: "最近快照可回滚", detail: `最近快照是 ${latestVersion.version}，可在版本控制中查看差异。`, to: "/versions" }
      : null,
  ].filter(Boolean) as Array<{ title: string; detail: string; to: string }>;

  const trialRoute = [
    !diagnostics?.chat_ready
      ? { title: "先完成模型与 API 配置", detail: diagnostics?.readiness_issues?.[0] ?? "先让对话后端进入可用状态。", to: "/settings" }
      : null,
    diagnostics && diagnostics.model_empty && (diagnostics.pending_builder_review_sessions ?? 0) > 0
      ? {
          title: "继续 Builder 中待确认的 Review",
          detail: `当前人生模型还没真正落库，但你已经有 ${diagnostics.pending_builder_review_sessions} 个待确认 Review。先回 Builder 审阅并应用它们，比重新开始更合适。`,
          to: "/builder",
        }
      : diagnostics && diagnostics.model_empty && diagnostics.unfinished_builder_sessions > 0
      ? {
          title: "继续 Builder 中待确认的 Review",
          detail: `当前人生模型还没真正落库，但你已经有 ${diagnostics.unfinished_builder_sessions} 个未完成构建会话。先回 Builder 应用它们，比重新开始更合适。`,
          to: "/builder",
        }
      : null,
    getModelEmptyState(model, diagnostics)
      ? { title: "完成第一次人生模型构建", detail: "先用快速构建建立最小可用模型，再开始个性化对话。", to: "/builder" }
      : builderCompletion && builderCompletion.overall < 60
      ? {
          title: `补强最弱维度：${dimensionLabels[lowestDimension || "state"] || "State"}`,
          detail: `当前整体完成度 ${Math.round(builderCompletion.overall)}%，继续构建会明显提升后续对话质量。`,
          to: "/builder",
        }
      : { title: "开始一次个性化对话", detail: "让 OpenLife 基于当前模型做今日规划、复盘或决策陪跑。", to: "/chat" },
    calibrationPrompt
      ? { title: "查看周期校准建议", detail: "有新的模型校准提醒，建议确认哪些变化值得吸收。", to: "/calibration" }
      : null,
    latestVersion
      ? { title: "检查最近模型变化", detail: `最近快照 ${latestVersion.version} 已可查看差异和回滚。`, to: "/versions" }
      : null,
  ].filter(Boolean) as Array<{ title: string; detail: string; to: string }>;

  const overallCompletion = diagnostics?.builder_completion?.overall ?? 0;
  const actionSignals = [
    diagnostics && !diagnostics.chat_ready
      ? {
          label: "运行环境",
          tone: "amber",
          title: "模型后端还没完全就绪",
          detail: diagnostics.readiness_issues[0] ?? "先修复配置，再做对话和深度试用会更顺畅。",
        }
      : null,
    getModelEmptyState(model, diagnostics)
      ? {
          label: "人生模型",
          tone: "indigo",
          title: "你的人生模型还没有建立起来",
          detail: "OpenLife 现在只能给通用建议。先完成一次快速构建，后续对话才会真正围绕你展开。",
        }
      : null,
    builderCompletion && builderCompletion.overall > 0
      ? {
          label: "模型完整度",
          tone: "emerald",
          title: `当前整体完成度 ${Math.round(builderCompletion.overall)}%`,
          detail:
            lowestDimension && lowestDimensionValue < completionThreshold
              ? `${dimensionLabels[lowestDimension] ?? lowestDimension} 仍是最薄弱维度，继续补这块会比盲目聊天更有效。`
              : "模型基础已经够用，可以把重心转到对话、复盘和持续校准。",
        }
      : null,
    dailyGoals.length > 0
      ? {
          label: "今日推进",
          tone: "rose",
          title: `今天还有 ${dailyGoals.length - completedDaily} 个目标待完成`,
          detail:
            completedDaily < dailyGoals.length
              ? "先完成一个低阻力的小闭环，通常比重新规划更能带来推进感。"
              : "今日目标已经清空，可以把时间留给反思、校准或下一阶段规划。",
        }
      : null,
    stateAlerts.length > 0
      ? {
          label: "状态信号",
          tone: "amber",
          title: `检测到 ${stateAlerts.length} 条状态预警`,
          detail: "系统判断你现在更适合先稳住节奏、做状态复盘，而不是继续叠加强刺激任务。",
        }
      : null,
  ].filter(Boolean) as Array<{ label: string; tone: "amber" | "indigo" | "emerald" | "rose"; title: string; detail: string }>;
  const actionSignalToneClass: Record<string, string> = {
    amber: "border-amber-100 bg-amber-50 text-amber-900",
    indigo: "border-indigo-100 bg-indigo-50 text-indigo-900",
    emerald: "border-emerald-100 bg-emerald-50 text-emerald-900",
    rose: "border-rose-100 bg-rose-50 text-rose-900",
  };
  const compassTone = diagnostics && !diagnostics.chat_ready
    ? "需要先修复运行环境"
    : getModelEmptyState(model, diagnostics)
    ? "先让 OpenLife 认识你"
    : stateAlerts.length > 0
    ? "今天适合稳住节奏"
    : dailyGoals.length > 0 && completedDaily < dailyGoals.length
    ? "今天适合推进一个小闭环"
    : "今天可以做一次深度对话";
  const compassDetail = diagnostics && !diagnostics.chat_ready
    ? "模型后端还没有就绪，先去设置页完成试用检查，会比继续点功能更省时间。"
    : getModelEmptyState(model, diagnostics)
    ? "人生模型还是空的。先完成一次快速构建，OpenLife 的建议才会真正围绕你展开。"
    : stateAlerts.length > 0
    ? `检测到 ${stateAlerts.length} 条状态预警，建议先降低任务强度，做一次状态复盘。`
    : dailyGoals.length > 0 && completedDaily < dailyGoals.length
    ? `今日还有 ${dailyGoals.length - completedDaily} 个目标未完成，适合先完成一个低阻力目标。`
    : "当前基础状态不错，可以进入 Chat 做今日规划、目标拆解或一次决策陪跑。";

  return (
    <div className="h-full overflow-auto bg-[#f4efe7] p-4 sm:p-6">
      <div className="max-w-6xl mx-auto space-y-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold text-stone-950 flex items-center gap-2">
              <LayoutDashboard className="text-stone-700" size={24} />
              今日驾驶舱
              <span className="sr-only">仪表盘</span>
            </h2>
            <p className="mt-1 text-sm text-stone-500">先看状态，再决定下一步。OpenLife 会把功能折叠成行动建议。</p>
          </div>
          {latestVersion && (
            <div className="flex items-center gap-2 text-sm text-gray-600">
              <Tag size={14} className="text-indigo-500" />
              <span>版本 {latestVersion.version}</span>
              <span className="text-gray-300">·</span>
              <span className="text-gray-400">{latestVersion.timestamp.slice(0, 10)}</span>
            </div>
          )}
        </div>

        <section className="relative overflow-hidden rounded-3xl border border-stone-200 bg-[#fbf7ef] p-6 shadow-sm">
          <div className="absolute -right-10 -top-16 h-52 w-52 rounded-full bg-amber-200/40 blur-2xl" />
          <div className="absolute -bottom-20 left-1/3 h-48 w-48 rounded-full bg-emerald-200/30 blur-2xl" />
          <div className="relative grid gap-6 lg:grid-cols-[1.3fr_0.7fr]">
            <div>
              <div className="inline-flex items-center gap-2 rounded-full bg-stone-900 px-3 py-1 text-xs font-medium text-amber-50">
                <Compass size={14} />
                今日判断
              </div>
              <h3 className="mt-4 text-3xl font-semibold tracking-tight text-stone-950">{compassTone}</h3>
              <p className="mt-3 max-w-2xl text-sm leading-7 text-stone-600">{compassDetail}</p>
              <div className="mt-5 flex flex-wrap gap-3">
                {(nextActions.length > 0 ? nextActions.slice(0, 3) : [
                  { title: "开始一次今日规划", detail: "进入对话，让 OpenLife 帮你把今天切成可执行步骤。", to: "/chat" },
                ]).map((action, index) => (
                  <Link
                    key={action.title}
                    to={action.to}
                    className={`inline-flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium transition ${
                      index === 0
                        ? "bg-stone-900 text-amber-50 hover:bg-stone-800"
                        : "bg-white/80 text-stone-700 border border-stone-200 hover:bg-white"
                    }`}
                  >
                    {action.title}
                    <ArrowRight size={14} />
                  </Link>
                ))}
              </div>
            </div>
            <div className="grid gap-3 sm:grid-cols-3 lg:grid-cols-1">
              <div className="rounded-2xl border border-white/70 bg-white/70 p-4">
                <div className="text-xs text-stone-500">人生模型完整度</div>
                <div className="mt-2 text-2xl font-semibold text-stone-900">{Math.round(overallCompletion)}%</div>
                <div className="mt-2 h-2 rounded-full bg-stone-200">
                  <div className="h-2 rounded-full bg-emerald-600" style={{ width: `${Math.max(0, Math.min(100, overallCompletion))}%` }} />
                </div>
              </div>
              <div className="rounded-2xl border border-white/70 bg-white/70 p-4">
                <div className="text-xs text-stone-500">今日目标进度</div>
                <div className="mt-2 text-2xl font-semibold text-stone-900">{completedDaily}/{dailyGoals.length}</div>
                <div className="mt-1 text-xs text-stone-500">完成</div>
              </div>
              <div className="rounded-2xl border border-white/70 bg-white/70 p-4">
                <div className="text-xs text-stone-500">系统状态</div>
                <div className="mt-2 flex items-center gap-2 text-sm font-medium text-stone-800">
                  <Sparkles size={15} className={diagnostics?.chat_ready ? "text-emerald-600" : "text-amber-600"} />
                  {diagnostics?.chat_ready ? "可对话" : "需配置"}
                </div>
                <div className="mt-1 text-xs text-stone-500">{diagnostics?.cloud_provider ?? "模型后端检测中"}</div>
              </div>
            </div>
          </div>
        </section>

        {nextActions.length > 0 && (
          <div className="rounded-2xl border border-indigo-100 bg-gradient-to-r from-indigo-50 via-white to-amber-50 p-5">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="text-sm font-semibold text-gray-900">下一步行动</div>
                <div className="mt-1 text-xs text-gray-500">按这个顺序推进，就能完成“建模 → 对话 → 查看 → 校准 → 回滚”的核心路径。</div>
              </div>
              <button onClick={refreshAll} className="rounded-md border bg-white px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-50">
                刷新状态
              </button>
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-2">
              {nextActions.slice(0, 4).map((action) => (
                <Link
                  key={action.title}
                  to={action.to}
                  className="group rounded-xl border border-white/80 bg-white/80 p-4 shadow-sm transition hover:-translate-y-0.5 hover:shadow-md"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-sm font-medium text-gray-900">{action.title}</div>
                    <ArrowRight size={15} className="text-gray-400 group-hover:text-indigo-600" />
                  </div>
                  <div className="mt-1 text-xs leading-relaxed text-gray-600">{action.detail}</div>
                </Link>
              ))}
            </div>
          </div>
        )}

        {trialRoute.length > 0 && (
          <div className="rounded-2xl border border-stone-200 bg-white/90 p-5">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="text-sm font-semibold text-gray-900">推荐试用路线</div>
                <div className="mt-1 text-xs text-gray-500">如果你今天只想顺着一条最省力的路径往前走，按这个顺序即可。</div>
              </div>
              <Link to={trialRoute[0].to} className="rounded-full bg-stone-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-stone-800">
                从第一步开始
              </Link>
            </div>
            <div className="mt-4 space-y-3">
              {trialRoute.map((item, index) => (
                <Link
                  key={`${index}-${item.title}`}
                  to={item.to}
                  className="flex items-start gap-3 rounded-xl border border-stone-100 bg-stone-50/70 px-4 py-3 transition hover:bg-white"
                >
                  <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-stone-900 text-xs font-semibold text-amber-50">
                    {index + 1}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-stone-900">{item.title}</div>
                    <div className="mt-1 text-xs leading-5 text-stone-600">{item.detail}</div>
                  </div>
                  <ArrowRight size={14} className="mt-1 shrink-0 text-stone-400" />
                </Link>
              ))}
            </div>
          </div>
        )}

        {actionSignals.length > 0 && (
          <div className="rounded-2xl border border-stone-200 bg-white/90 p-5">
            <div className="text-sm font-semibold text-stone-900">为什么今天先做这个</div>
            <div className="mt-1 text-xs text-stone-500">
              这些不是随机建议，而是系统根据你当前的人生模型、目标进度、状态信号和运行环境整理出的判断依据。
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-2">
              {actionSignals.slice(0, 4).map((signal) => (
                <div
                  key={`${signal.label}-${signal.title}`}
                  className={`rounded-xl border px-4 py-3 ${actionSignalToneClass[signal.tone]}`}
                >
                  <div className="text-[11px] font-medium opacity-80">{signal.label}</div>
                  <div className="mt-1 text-sm font-medium">{signal.title}</div>
                  <div className="mt-1 text-xs leading-5 opacity-90">{signal.detail}</div>
                </div>
              ))}
            </div>
          </div>
        )}

        {diagnostics && isSafeMode(diagnostics) && (
            <div className="rounded-2xl border border-amber-200 bg-amber-50 p-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-amber-900">Safe Mode：建议先修复数据环境再深度试用</div>
                  <div className="mt-1 text-xs leading-5 text-amber-800">
                    {getSafeModeReason(diagnostics)}
                  </div>
                  <div className="mt-1 text-xs text-amber-700">
                    可以继续查看仪表盘，但若要进行 Builder、长期记忆或高频聊天试用，建议先去恢复控制台。
                  </div>
                </div>
                <Link
                  to="/settings"
                  className="inline-flex shrink-0 items-center gap-2 rounded-full bg-amber-900 px-4 py-2 text-sm font-medium text-amber-50 hover:bg-amber-950"
                >
                  去恢复控制台
                  <ArrowRight size={14} />
                </Link>
              </div>
            </div>
          )}

        {loadWarnings.length > 0 && (
          <div className="rounded-xl border border-amber-100 bg-amber-50 p-4 text-sm text-amber-900">
            <div className="flex items-center justify-between mb-1">
              <div className="font-semibold">部分数据暂时不可用</div>
              <button
                onClick={() => setLoadWarnings([])}
                className="text-xs text-amber-700 hover:text-amber-900 underline"
              >
                关闭
              </button>
            </div>
            <ul className="list-disc pl-5 space-y-1">
              {loadWarnings.slice(0, 5).map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
            <div className="mt-2 text-xs text-amber-700">你仍可继续使用已加载的功能；如聊天或模型不可用，请到设置页查看“试用就绪检查”。</div>
          </div>
        )}
        {calibrationError && (
          <ErrorBanner
            message={calibrationError}
            severity="error"
            onClose={() => setCalibrationError(null)}
            className="rounded-xl"
          />
        )}

        {/* Prompts */}
        {calibrationPrompt && (
          <div className="bg-gradient-to-r from-amber-50 to-orange-50 border border-amber-200 rounded-xl p-4 flex flex-col sm:flex-row sm:items-center gap-4">
            <div className="flex-1">
              <div className="font-semibold text-amber-900 flex items-center gap-2">
                <ClipboardList size={18} className="text-amber-600" />
                {calibrationPrompt.monthly ? "每月校准提醒" : "每周校准提醒"}
              </div>
              <p className="text-sm text-amber-800 mt-1">
                到了回顾与微调人生模型的时候。查看周期报告并确认建议变更，让模型持续贴合你的真实状态。
              </p>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              <button onClick={dismissCalibrationPrompt} className="px-3 py-2 rounded-lg text-sm text-amber-900 hover:bg-amber-100">忽略</button>
              <button onClick={() => navigate("/calibration")} className="px-4 py-2 rounded-lg text-sm bg-amber-600 text-white hover:bg-amber-700">去校准</button>
            </div>
          </div>
        )}

        {getModelEmptyState(model, diagnostics) && (
          <div className="bg-white border rounded-xl p-5 flex flex-col sm:flex-row sm:items-center gap-4">
            <div className="flex-1">
              <div className="font-semibold text-indigo-900 flex items-center gap-2">
                <Hammer size={18} className="text-indigo-600" />
                开启你的人生模型
              </div>
              <p className="text-sm text-gray-600 mt-1">
                你的仪表盘还是空白的。花 10 分钟通过「构建」模式建立人生模型，OpenLife 就能为你提供真正个性化的对话与洞察。
              </p>
            </div>
            <Link to="/builder" className="inline-flex items-center justify-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 shrink-0">
              去构建 <ArrowRight size={16} />
            </Link>
          </div>
        )}

        {/* Top: Daily Goals + Quick Stats */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div className="lg:col-span-2 bg-white border rounded-xl p-5">
            <div className="flex items-center justify-between mb-4">
              <div className="font-semibold text-gray-800 flex items-center gap-2"><Target size={18} className="text-indigo-600" /> 今日目标</div>
              <div className="text-sm text-gray-500">{completedDaily} / {dailyGoals.length} 完成</div>
            </div>
            <div className="space-y-2">
              {dailyGoals.map((g, i) => (
                <div key={i} className="flex items-center gap-3 bg-gray-50 rounded-lg px-3 py-2">
                  <button onClick={() => onToggleGoal(i)} className={`w-5 h-5 rounded border flex items-center justify-center ${g.done ? "bg-indigo-600 border-indigo-600 text-white" : "border-gray-300"}`}>
                    {g.done && <Check size={14} />}
                  </button>
                  {editingGoalIndex === i ? (
                    <>
                      <input value={editGoalName} onChange={(e) => setEditGoalName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && onSaveEditGoal(i)} className="flex-1 border rounded px-2 py-1 text-sm" autoFocus />
                      <button onClick={() => onSaveEditGoal(i)} className="text-green-600"><Check size={16} /></button>
                      <button onClick={() => setEditingGoalIndex(null)} className="text-gray-500"><X size={16} /></button>
                    </>
                  ) : (
                    <>
                      <span className={`flex-1 text-sm ${g.done ? "line-through text-gray-400" : "text-gray-800"}`}>{g.name}</span>
                      <button onClick={() => startEditGoal(i, g.name)} className="text-gray-400 hover:text-indigo-600"><Edit2 size={14} /></button>
                      <button onClick={() => onDeleteGoal(i)} className="text-gray-400 hover:text-rose-600"><Trash2 size={14} /></button>
                    </>
                  )}
                </div>
              ))}
              {dailyGoals.length === 0 && !addingGoal && (
                <EmptyState title="暂无今日目标" description="添加一个小目标，开启一天的好状态。" className="py-4" />
              )}
              {addingGoal ? (
                <div className="flex items-center gap-2">
                  <input value={newGoalName} onChange={(e) => setNewGoalName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && onAddGoal()} placeholder="输入目标..." className="flex-1 border rounded-lg px-3 py-2 text-sm" autoFocus />
                  <button onClick={onAddGoal} className="text-green-600"><Check size={18} /></button>
                  <button onClick={() => setAddingGoal(false)} className="text-gray-500"><X size={18} /></button>
                </div>
              ) : (
                <button onClick={() => setAddingGoal(true)} className="inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-700"><Plus size={16} /> 添加目标</button>
              )}
            </div>
          </div>

          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><Activity size={18} className="text-amber-600" /> 状态预警</div>
            <div className="space-y-3">
              {stateAlerts.length > 0 ? (
                stateAlerts.slice(0, 3).map((alert, i) => (
                  <div key={i} className="bg-amber-50 border border-amber-100 rounded-lg px-3 py-3">
                    <div className="text-sm font-medium text-amber-800">{alert.dimension_name}</div>
                    <div className="text-sm text-amber-700 mt-1">{alert.message}</div>
                  </div>
                ))
              ) : (
                <EmptyState title="状态良好" description="当前没有状态预警，继续保持！" className="py-4" />
              )}
              {stateAlerts.length > 3 && <div className="text-xs text-gray-500">还有 {stateAlerts.length - 3} 项预警</div>}
            </div>
          </div>
        </div>

        {/* Middle: Radar + Gaps + State Trends */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><Zap size={18} className="text-indigo-600" /> 能力雷达</div>
            <RadarChart skills={skillData} />
          </div>

          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><TrendingUp size={18} className="text-indigo-600" /> 目标-能力缺口</div>
            {gaps && gaps.length > 0 ? (
              <div className="space-y-2">
                {gaps.slice(0, 5).map((gap, i) => (
                  <div key={i} className="bg-indigo-50 rounded-lg px-3 py-2 text-sm text-gray-800">
                    {gap.goal_name} · {gap.skill_name} · {gap.current_level}/{gap.target_level}
                  </div>
                ))}
                {gaps.length > 5 && <div className="text-xs text-gray-500">还有 {gaps.length - 5} 项缺口</div>}
              </div>
            ) : (
              <div className="text-sm text-gray-500">暂无显著能力缺口，继续保持！</div>
            )}
          </div>

          <div className="bg-white border rounded-xl p-5 flex flex-col">
            <div className="flex items-center justify-between mb-4">
              <div className="font-semibold text-gray-800 flex items-center gap-2"><Activity size={18} className="text-indigo-600" /> 状态趋势</div>
              <button onClick={() => setShowStateModal(true)} className="inline-flex items-center gap-1 text-xs bg-indigo-600 text-white px-2 py-1 rounded hover:bg-indigo-700"><Plus size={12} /> 记录</button>
            </div>
            <div className="flex-1 overflow-auto space-y-2">
              {dimensions.length === 0 && <EmptyState title="暂无状态维度" description="记录一个自定义状态维度，追踪长期趋势。" className="py-4" />}
              {dimensions.slice(0, 5).map((dim) => (
                <div key={dim.name} onClick={() => openDimension(dim.name)} className={`cursor-pointer border rounded-lg p-3 hover:shadow-sm transition ${selectedDimension === dim.name ? "border-indigo-500 ring-1 ring-indigo-500" : ""}`}>
                  <div className="flex items-center justify-between">
                    <div className="font-medium text-gray-800">{dim.name}</div>
                    <div className="text-xs text-gray-500">{dim.unit}</div>
                  </div>
                  <div className="text-lg font-bold text-indigo-700">{dim.current_value.toFixed(1)}</div>
                  <div className="text-xs text-gray-400">{(dim.min_threshold !== undefined || dim.max_threshold !== undefined) ? `阈值 ${dim.min_threshold ?? "-"} ~ ${dim.max_threshold ?? "-"}` : "未设置阈值"}</div>
                </div>
              ))}
            </div>
            {selectedDimension && (
              <div className="mt-3 border-t pt-3">
                <div className="flex items-center justify-between mb-1">
                  <div className="text-xs font-medium text-gray-700">{selectedDimension} 趋势</div>
                  <div className="text-[10px] text-gray-400">最近 {dimensionHistory.length} 条</div>
                </div>
                <div className="bg-gray-50 rounded-lg p-2">
                  <MiniLineChart data={dimensionHistory} height={80} />
                </div>
                {trendSummary && (
                  <div className="mt-3 space-y-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${trendBadge(trendSummary.direction)}`}>
                        {trendSummary.direction === "up" ? "趋势回升" : trendSummary.direction === "down" ? "趋势下降" : "趋势平稳"}
                      </div>
                      <div className="text-xs text-gray-500">
                        {trendSummary.previous === null ? "首条记录" : `较上一条 ${trendSummary.delta >= 0 ? "+" : ""}${trendSummary.delta.toFixed(1)} ${selectedDimensionModel?.unit ?? ""}`}
                      </div>
                    </div>
                    <div className="grid grid-cols-3 gap-2">
                      <div className="rounded-lg bg-indigo-50 px-3 py-2">
                        <div className="text-[10px] text-indigo-500">当前</div>
                        <div className="text-sm font-semibold text-indigo-900">{trendSummary.latest.toFixed(1)} {selectedDimensionModel?.unit}</div>
                      </div>
                      <div className="rounded-lg bg-slate-50 px-3 py-2">
                        <div className="text-[10px] text-slate-500">平均</div>
                        <div className="text-sm font-semibold text-slate-900">{trendSummary.average.toFixed(1)} {selectedDimensionModel?.unit}</div>
                      </div>
                      <div className="rounded-lg bg-amber-50 px-3 py-2">
                        <div className="text-[10px] text-amber-500">波动</div>
                        <div className="text-sm font-semibold text-amber-900">{trendSummary.min.toFixed(1)} - {trendSummary.max.toFixed(1)}</div>
                      </div>
                    </div>
                    <div className="rounded-lg border border-slate-200 bg-white px-3 py-3">
                      <div className="text-xs font-medium text-slate-700 mb-1">趋势解释</div>
                      <div className="text-sm text-slate-600">{trendSummary.explanation}</div>
                      {trendSummary.alertMessage && (
                        <div className="mt-2 rounded-lg bg-amber-50 px-3 py-2 text-xs text-amber-800">
                          预警原因：{trendSummary.alertMessage}
                        </div>
                      )}
                    </div>
                    {dimensionHistory.some((item) => item.note) && (
                      <div className="rounded-lg bg-slate-50 px-3 py-3">
                        <div className="text-xs font-medium text-slate-700 mb-2">最近备注</div>
                        <div className="space-y-1">
                          {dimensionHistory.filter((item) => item.note).slice(-3).reverse().map((item) => (
                            <div key={item.id} className="text-xs text-slate-600">
                              {new Date(item.recorded_at).toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" })} · {item.note}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Stats row */}
        <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-3">
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">目标进度</div>
            <div className="text-xl font-bold text-indigo-700">{Math.round(goalProgress)}%</div>
            <div className="text-[10px] text-gray-400">{completedGoals} / {allGoals.length}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">技能</div>
            <div className="text-xl font-bold text-green-700">{capabilities?.skills?.length ?? 0}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">价值观</div>
            <div className="text-xl font-bold text-purple-700">{model?.identity?.values?.length ?? 0}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">记忆</div>
            <div className="text-xl font-bold text-orange-700">{memoryCount}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">消息</div>
            <div className="text-xl font-bold text-gray-900">{feedback?.total_messages ?? 0}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">正向反馈</div>
            <div className="text-xl font-bold text-green-700">{feedback?.total_feedback_up ?? 0}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">负向反馈</div>
            <div className="text-xl font-bold text-rose-700">{feedback?.total_feedback_down ?? 0}</div>
          </div>
          <div className="bg-white border rounded-xl p-4">
            <div className="text-xs text-gray-500 mb-1">校准</div>
            <button onClick={handleGenerateCalibration} disabled={calibrationLoading} className="text-xs bg-gray-900 text-white px-2 py-1 rounded hover:bg-gray-800 disabled:opacity-50">{calibrationLoading ? "生成中" : "报告"}</button>
          </div>
        </div>

        {/* Bottom: Memory search + Calibration */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div className="lg:col-span-2 bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><Brain size={18} className="text-indigo-600" /> 语义记忆检索</div>
            <div className="flex gap-2 mb-3">
              <input value={memoryQuery} onChange={(e) => setMemoryQuery(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleMemorySearch()} placeholder="搜索记忆..." className="flex-1 border rounded-lg px-3 py-2 text-sm" />
              <button onClick={handleMemorySearch} className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700">搜索</button>
            </div>
            <div className="space-y-2 max-h-48 overflow-auto">
              {memories.map((m, i) => (
                <div key={i} className="bg-gray-50 rounded-lg p-3 text-sm">
                  <div className="text-gray-800">{m.chunk.content}</div>
                  <div className="text-xs text-gray-500 mt-1">source: {m.chunk.source} · 相关度: {m.score.toFixed(3)}</div>
                </div>
              ))}
              {memories.length === 0 && (
                <EmptyState title="暂无搜索结果" description="输入关键词搜索语义记忆。" className="py-4" />
              )}
            </div>
          </div>

          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><RefreshCw size={18} className="text-indigo-600" /> 微进化</div>
            <div className="flex flex-col gap-3">
              <button onClick={handleRunMicroEvolution} className="inline-flex items-center justify-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700"><RefreshCw size={16} /> 运行微进化</button>
              {evolutionMsg && <div className="bg-green-50 text-green-800 rounded-lg px-3 py-2 text-xs">{evolutionMsg}</div>}
              {calibration && (
                <div className="space-y-2 text-sm">
                  <div className="text-xs text-gray-500">{calibration.summary_text.slice(0, 60)}...</div>
                  <div className="flex gap-2">
                    <div className="flex-1 bg-gray-50 rounded p-2 text-center"><div className="text-xs text-gray-400">正向</div><div className="font-semibold">{calibration.feedback_up}</div></div>
                    <div className="flex-1 bg-gray-50 rounded p-2 text-center"><div className="text-xs text-gray-400">负向</div><div className="font-semibold">{calibration.feedback_down}</div></div>
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><TrendingUp size={18} className="text-indigo-600" /> 目标-能力缺口</div>
            <div className="space-y-3">
              {gaps && gaps.length > 0 ? gaps.slice(0, 3).map((gap, idx) => (
                <div key={`${gap.goal_name}-${gap.skill_name}-${idx}`} className="rounded-lg border border-amber-100 bg-amber-50/70 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="font-medium text-gray-800">{gap.goal_name}</div>
                    <span className={`text-[10px] px-2 py-0.5 rounded-full ${gap.severity === "high" ? "bg-rose-100 text-rose-700" : gap.severity === "medium" ? "bg-amber-100 text-amber-700" : "bg-slate-100 text-slate-600"}`}>
                      {gap.severity}
                    </span>
                  </div>
                  <div className="mt-1 text-sm text-gray-700">
                    关键能力：{gap.skill_name} · 当前 {gap.current_level}/10 · 建议目标 {gap.target_level}/10
                  </div>
                  <div className="mt-2 text-xs text-gray-600">{gap.suggestion}</div>
                </div>
              )) : (
                <div className="text-sm text-gray-500">暂无显著能力缺口，继续保持！</div>
              )}
            </div>
          </div>

          <div className="bg-white border rounded-xl p-5">
            <div className="font-semibold text-gray-800 mb-4 flex items-center gap-2"><Tag size={18} className="text-indigo-600" /> 价值观-目标一致性</div>
            <div className="space-y-3">
              {alignments && alignments.length > 0 ? alignments.slice(0, 3).map((issue, idx) => (
                <div key={`${issue.goal_name}-${idx}`} className="rounded-lg border border-rose-100 bg-rose-50/70 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <div className="font-medium text-gray-800">{issue.goal_name}</div>
                    <span className={`text-[10px] px-2 py-0.5 rounded-full ${issue.severity === "high" ? "bg-rose-100 text-rose-700" : "bg-amber-100 text-amber-700"}`}>
                      {issue.severity}
                    </span>
                  </div>
                  <div className="mt-1 text-sm text-gray-700">{issue.reason}</div>
                  {issue.related_values.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-2">
                      {issue.related_values.map((value) => (
                        <span key={value} className="rounded-full bg-white px-2 py-0.5 text-[10px] text-rose-700 border border-rose-100">
                          {value}
                        </span>
                      ))}
                    </div>
                  )}
                  <div className="mt-2 text-xs text-gray-600">{issue.suggestion}</div>
                </div>
              )) : (
                <div className="text-sm text-gray-500">当前高优先级目标与核心价值观基本一致。</div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* State Modal */}
      {showStateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <div className="bg-white rounded-xl w-full max-w-md p-5 shadow-lg max-h-[90vh] overflow-auto">
            <div className="font-semibold text-gray-800 mb-4">记录状态</div>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-gray-600 mb-1">维度名称</label>
                <input
                  value={stateInputName}
                  onChange={(e) => setStateInputName(e.target.value)}
                  placeholder="例如：体重、睡眠时长、专注度"
                  className="w-full border rounded-lg px-3 py-2 text-sm"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm text-gray-600 mb-1">数值</label>
                  <input
                    type="number"
                    step="0.1"
                    value={stateInputValue}
                    onChange={(e) => setStateInputValue(e.target.value)}
                    placeholder="0.0"
                    className="w-full border rounded-lg px-3 py-2 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm text-gray-600 mb-1">单位</label>
                  <input
                    value={stateInputUnit}
                    onChange={(e) => setStateInputUnit(e.target.value)}
                    placeholder="kg / h / 分"
                    className="w-full border rounded-lg px-3 py-2 text-sm"
                  />
                </div>
              </div>
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="block text-sm text-gray-600 mb-1">最小阈值</label>
                  <input
                    type="number"
                    step="0.1"
                    value={stateInputMin}
                    onChange={(e) => setStateInputMin(e.target.value)}
                    placeholder="可选"
                    className="w-full border rounded-lg px-3 py-2 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm text-gray-600 mb-1">最大阈值</label>
                  <input
                    type="number"
                    step="0.1"
                    value={stateInputMax}
                    onChange={(e) => setStateInputMax(e.target.value)}
                    placeholder="可选"
                    className="w-full border rounded-lg px-3 py-2 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-sm text-gray-600 mb-1">预警天数</label>
                  <input
                    type="number"
                    min={1}
                    value={stateInputAlertDays}
                    onChange={(e) => setStateInputAlertDays(e.target.value)}
                    className="w-full border rounded-lg px-3 py-2 text-sm"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm text-gray-600 mb-1">备注（可选）</label>
                <input
                  value={stateInputNote}
                  onChange={(e) => setStateInputNote(e.target.value)}
                  placeholder="今天感觉如何？"
                  className="w-full border rounded-lg px-3 py-2 text-sm"
                />
              </div>
            </div>
            <div className="flex justify-end gap-2 mt-5">
              <button onClick={() => setShowStateModal(false)} className="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded-lg">取消</button>
              <button onClick={onRecordState} className="px-4 py-2 text-sm bg-indigo-600 text-white rounded-lg hover:bg-indigo-700">保存</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
