import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Hammer,
  Footprints,
  Brain,
  ArrowRight,
  CheckCircle2,
  RefreshCw,
  Target,
  Zap,
  Heart,
  Sparkles,
  Trash2,
  AlertCircle,
  ShieldCheck,
} from "lucide-react";
import LoadingSpinner from "../components/LoadingSpinner";
import EmptyState from "../components/EmptyState";
import ErrorBanner from "../components/ErrorBanner";
import type { LifeModel, BuilderProgress } from "../types";
import {
  builderStart,
  builderStep,
  builderListUnfinished,
  builderDeleteSession,
  builderCreateProposals,
  getModel4DCompletion,
  getSystemDiagnostics,
  type UnfinishedBuilderSession,
  type Model4DCompletion,
  type BuilderAnalysis,
  type BuilderSignal,
  type SystemDiagnostics,
  type BuilderSignalDecision,
} from "../tauri";
import BuilderPatchReview from "../components/BuilderPatchReview";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";

function CompletionBar({
  label,
  value,
  colorClass,
}: {
  label: string;
  value: number;
  colorClass: string;
}) {
  const pct = Math.max(0, Math.min(100, value));
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-xs">
        <span className="text-gray-600">{label}</span>
        <span className="text-gray-500">{pct}%</span>
      </div>
      <div className="w-full bg-gray-200 rounded-full h-2">
        <div
          className={`${colorClass} h-2 rounded-full transition-all`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}

function RadarChart({ values, size = 120 }: { values: number[]; size?: number }) {
  const labels = ["Identity", "Goals", "Capabilities", "State"];
  const center = size / 2;
  const radius = size * 0.38;
  const angleFor = (i: number) => (Math.PI * 2 * i) / 4 - Math.PI / 2;
  const points = labels.map((_, i) => {
    const a = angleFor(i);
    return `${center + radius * Math.cos(a)},${center + radius * Math.sin(a)}`;
  });
  const valuePoints = values.map((v, i) => {
    const a = angleFor(i);
    const r = radius * Math.max(0, Math.min(1, v / 100));
    return `${center + r * Math.cos(a)},${center + r * Math.sin(a)}`;
  });
  return (
    <svg width={size} height={size} className="shrink-0">
      <polygon points={points.join(" ")} fill="none" stroke="#e5e7eb" strokeWidth={1} />
      {[0.25, 0.5, 0.75].map(scale => {
        const ring = values.map((_, i) => {
          const a = angleFor(i);
          const r = radius * scale;
          return `${center + r * Math.cos(a)},${center + r * Math.sin(a)}`;
        });
        return (
          <polygon
            key={scale}
            points={ring.join(" ")}
            fill="none"
            stroke="#f3f4f6"
            strokeWidth={1}
          />
        );
      })}
      {labels.map((_, i) => {
        const a = angleFor(i);
        return (
          <line
            key={i}
            x1={center}
            y1={center}
            x2={center + radius * Math.cos(a)}
            y2={center + radius * Math.sin(a)}
            stroke="#e5e7eb"
            strokeWidth={1}
          />
        );
      })}
      <polygon
        points={valuePoints.join(" ")}
        fill="rgba(99,102,241,0.25)"
        stroke="#6366f1"
        strokeWidth={2}
      />
    </svg>
  );
}

function quickStepDim(stepIndex: number): { dim: string; color: string } {
  if (stepIndex <= 2) return { dim: "Identity", color: "indigo" };
  if (stepIndex === 3) return { dim: "Goals", color: "green" };
  if (stepIndex === 4) return { dim: "Capabilities", color: "yellow" };
  if (stepIndex === 5) return { dim: "State", color: "purple" };
  return { dim: "Review", color: "gray" };
}

function ModeStepper({ mode, progress }: { mode: string; progress: BuilderProgress }) {
  if (mode === "quick" && progress.total_steps > 0) {
    const labels = ["身份", "价值观", "目标", "能力", "状态", "回顾"];
    return (
      <div className="flex items-center justify-between text-[10px] text-gray-500">
        {labels.map((label, i) => {
          const active = i + 1 === progress.step_index;
          const done = i + 1 < progress.step_index;
          const dimInfo = quickStepDim(i + 1);
          const dimColorClass =
            dimInfo.color === "indigo"
              ? "bg-indigo-100 text-indigo-700"
              : dimInfo.color === "green"
                ? "bg-green-100 text-green-700"
                : dimInfo.color === "yellow"
                  ? "bg-amber-100 text-amber-700"
                  : dimInfo.color === "purple"
                    ? "bg-purple-100 text-purple-700"
                    : "bg-gray-100 text-gray-600";
          return (
            <div key={label} className="flex flex-col items-center gap-1 flex-1">
              <div
                className={`w-6 h-6 rounded-full flex items-center justify-center border ${
                  done
                    ? "bg-indigo-600 border-indigo-600 text-white"
                    : active
                      ? "bg-indigo-50 border-indigo-600 text-indigo-700 font-semibold"
                      : "bg-white border-gray-300 text-gray-400"
                }`}
              >
                {done ? <CheckCircle2 size={12} /> : i + 1}
              </div>
              <span className={active ? "text-indigo-700 font-medium" : ""}>{label}</span>
              <span className={`px-1.5 py-0.5 rounded text-[9px] font-medium ${dimColorClass}`}>
                {dimInfo.dim}
              </span>
            </div>
          );
        })}
      </div>
    );
  }
  if (mode === "socratic" && progress.total_steps > 0) {
    return (
      <div className="space-y-2">
        <div className="flex items-center gap-1 overflow-x-auto pb-1">
          {Array.from({ length: progress.total_steps }).map((_, i) => {
            const idx = i + 1;
            const active = idx === progress.step_index;
            const done = idx < progress.step_index;
            return (
              <div
                key={idx}
                className={`w-7 h-7 rounded-full flex items-center justify-center text-xs border shrink-0 ${
                  done
                    ? "bg-indigo-600 border-indigo-600 text-white"
                    : active
                      ? "bg-indigo-50 border-indigo-600 text-indigo-700 font-semibold"
                      : "bg-white border-gray-300 text-gray-400"
                }`}
              >
                {idx}
              </div>
            );
          })}
        </div>
        <div className="text-[10px] text-indigo-700 bg-indigo-50 inline-block px-2 py-0.5 rounded">
          {progress.current_step_label}
        </div>
      </div>
    );
  }
  return null;
}

function buildStageSuggestions(
  completion: Model4DCompletion | null,
  analysis: BuilderAnalysis | null
) {
  const suggestions: string[] = [];
  if (!completion) return suggestions;
  const dims = [
    { key: "identity", label: "身份认同", value: completion.identity },
    { key: "goals", label: "目标体系", value: completion.goals },
    { key: "capabilities", label: "能力资源", value: completion.capabilities },
    { key: "state", label: "当前状态", value: completion.state },
  ].sort((a, b) => a.value - b.value);
  if (dims[0].value < 60) {
    suggestions.push(`优先补强「${dims[0].label}」，这是当前最薄弱的一维。`);
  }
  if (analysis?.gaps?.length) {
    suggestions.push(`本轮最值得继续补的是：${analysis.gaps.slice(0, 2).join("、")}。`);
  }
  if (completion.overall >= 75) {
    suggestions.push("模型已经进入较完整状态，接下来更适合做精修和校准。");
  }
  return suggestions.slice(0, 3);
}

function sortResumeSessions(sessions: UnfinishedBuilderSession[]): UnfinishedBuilderSession[] {
  return [...sessions].sort((a, b) => {
    const aPendingReview = a.finished && (a.pending_signals?.length ?? 0) > 0;
    const bPendingReview = b.finished && (b.pending_signals?.length ?? 0) > 0;
    if (aPendingReview !== bPendingReview) {
      return aPendingReview ? -1 : 1;
    }
    if (a.finished !== b.finished) {
      return a.finished ? -1 : 1;
    }
    return b.step_index - a.step_index;
  });
}

const dimensionStyleMap: Record<
  "identity" | "goals" | "capabilities" | "state",
  {
    hover: string;
    text: string;
    bar: string;
  }
> = {
  identity: {
    hover: "hover:border-indigo-500 hover:bg-indigo-50",
    text: "text-indigo-700",
    bar: "bg-indigo-500",
  },
  goals: {
    hover: "hover:border-green-500 hover:bg-green-50",
    text: "text-green-700",
    bar: "bg-green-500",
  },
  capabilities: {
    hover: "hover:border-amber-500 hover:bg-amber-50",
    text: "text-amber-700",
    bar: "bg-amber-500",
  },
  state: {
    hover: "hover:border-purple-500 hover:bg-purple-50",
    text: "text-purple-700",
    bar: "bg-purple-500",
  },
};

export default function BuilderPage() {
  const [mode, setMode] = useState<"quick" | "incremental" | "socratic" | null>(null);
  const [sessionId, setSessionId] = useState<string>(crypto.randomUUID());
  const [prompt, setPrompt] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [finished, setFinished] = useState(false);
  const [resultModel, setResultModel] = useState<LifeModel | null>(null);
  const [progress, setProgress] = useState<BuilderProgress | null>(null);
  const [unfinished, setUnfinished] = useState<UnfinishedBuilderSession[]>([]);
  const [completion, setCompletion] = useState<Model4DCompletion | null>(null);
  const [analysis, setAnalysis] = useState<BuilderAnalysis | null>(null);
  const [builderError, setBuilderError] = useState<string | null>(null);
  const [builderNotice, setBuilderNotice] = useState<React.ReactNode>(null);
  const [lastStart, setLastStart] = useState<{
    mode: "quick" | "incremental" | "socratic";
    sessionId: string;
    targetDimension?: "identity" | "goals" | "capabilities" | "state";
  } | null>(null);
  const [pendingSignals, setPendingSignals] = useState<BuilderSignal[]>([]);
  const [reviewMode, setReviewMode] = useState(false);
  const [appliedFields, setAppliedFields] = useState<string[]>([]);
  const [mergedFields, setMergedFields] = useState<string[]>([]);
  const [skippedFields, setSkippedFields] = useState<
    Array<{ path: string; reason: string; expected?: string }>
  >([]);
  const [editedCount, setEditedCount] = useState(0);
  const [rejectedCount, setRejectedCount] = useState(0);
  const [incrementalDimension, setIncrementalDimension] = useState<
    "identity" | "goals" | "capabilities" | "state" | null
  >(null);
  const [beforeBuildCompletion, setBeforeBuildCompletion] = useState<Model4DCompletion | null>(
    null
  );
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const navigate = useNavigate();

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  useEffect(() => {
    loadUnfinished();
    loadCompletion();
    getSystemDiagnostics()
      .then(setDiagnostics)
      .catch(() => null);
  }, []);

  const loadCompletion = async () => {
    try {
      const c = await getModel4DCompletion();
      setCompletion(c);
    } catch (e) {
      console.error("加载四维完成度失败", e);
      setBuilderError("加载四维完成度失败：" + String(e));
    }
  };

  const loadUnfinished = async () => {
    try {
      const list = await builderListUnfinished();
      setUnfinished(sortResumeSessions(list));
    } catch (e) {
      console.error("加载未完成会话失败", e);
      setBuilderError(buildRuntimeActionError("加载未完成会话", e, "data"));
    }
  };

  const start = async (
    selected: "quick" | "incremental" | "socratic",
    sid: string,
    targetDimension?: "identity" | "goals" | "capabilities" | "state"
  ) => {
    if (safeMode) {
      setBuilderError(buildSafeModeBlockedMessage("新的构建写入", diagnostics));
      return;
    }
    setMode(selected);
    setSessionId(sid);
    setLoading(true);
    setBuilderError(null);
    setBuilderNotice(null);
    setLastStart({ mode: selected, sessionId: sid, targetDimension });
    // Snapshot completion before building starts
    try {
      const c = await getModel4DCompletion();
      setBeforeBuildCompletion(c);
    } catch {
      setBeforeBuildCompletion(null);
    }
    try {
      const res = await builderStart(selected, sid, targetDimension);
      setPrompt(res.prompt);
      setProgress(res.progress);
      if (res.analysis) setAnalysis(res.analysis);
      if (res.finished && res.pending_signals && res.pending_signals.length > 0) {
        setPendingSignals(res.pending_signals);
        setReviewMode(true);
        setFinished(true);
      } else {
        setPendingSignals([]);
        setReviewMode(false);
        setFinished(false);
      }
    } catch (e) {
      setBuilderError(buildRuntimeActionError("启动构建会话", e, "review"));
    } finally {
      setLoading(false);
    }
  };

  const resume = async (session: UnfinishedBuilderSession) => {
    if (safeMode) {
      setBuilderError(buildSafeModeBlockedMessage("恢复构建会话", diagnostics));
      return;
    }
    const modeMap: Record<string, "quick" | "incremental" | "socratic"> = {
      Quick: "quick",
      Incremental: "incremental",
      Socratic: "socratic",
    };
    const m = modeMap[session.mode] ?? "quick";
    const targetDim = session.target_dimension?.toLowerCase() as
      | "identity"
      | "goals"
      | "capabilities"
      | "state"
      | undefined;
    await start(m, session.session_id, targetDim);
  };

  const removeSession = async (sid: string) => {
    try {
      await builderDeleteSession(sid);
      setUnfinished(prev => sortResumeSessions(prev.filter(s => s.session_id !== sid)));
    } catch (e) {
      setBuilderError(buildRuntimeActionError("删除未完成会话", e, "data"));
    }
  };

  const sendReply = async (replyOverride?: string) => {
    const reply = (replyOverride ?? input).trim();
    if (!reply || loading) return;
    if (safeMode) {
      setBuilderError(buildSafeModeBlockedMessage("构建回答提交", diagnostics));
      return;
    }
    setLoading(true);
    setBuilderError(null);
    setBuilderNotice(null);
    try {
      const res = await builderStep(sessionId, reply);
      setPrompt(res.prompt);
      setFinished(res.finished);
      setProgress(res.progress);
      if (res.analysis) setAnalysis(res.analysis);
      if (res.model) {
        setResultModel(res.model);
      }
      // For Quick and Incremental modes: when finished, enter review mode instead of auto-saving
      if (
        res.finished &&
        (res.mode === "Quick" || res.mode === "Incremental") &&
        res.pending_signals &&
        res.pending_signals.length > 0
      ) {
        setPendingSignals(res.pending_signals);
        setReviewMode(true);
        // Keep session alive for review
      } else if (res.finished && res.mode === "Socratic") {
        // Socratic mode keeps existing behavior
      }
      setInput("");
      await loadCompletion();
    } catch (e) {
      setBuilderError(buildRuntimeActionError("提交回答", e, "review"));
    } finally {
      setLoading(false);
    }
  };

  const reset = () => {
    setMode(null);
    setPrompt(null);
    setInput("");
    setFinished(false);
    setResultModel(null);
    setProgress(null);
    setAnalysis(null);
    setBuilderError(null);
    setBuilderNotice(null);
    setPendingSignals([]);
    setReviewMode(false);
    setBeforeBuildCompletion(null);
    setAppliedFields([]);
    setMergedFields([]);
    setSkippedFields([]);
    setEditedCount(0);
    setRejectedCount(0);
    setSessionId(crypto.randomUUID());
    loadUnfinished();
  };

  const handleCreateProposals = async (decisions: BuilderSignalDecision[]) => {
    if (safeMode) {
      setBuilderError(buildSafeModeBlockedMessage("创建模型更新 Proposal", diagnostics));
      return;
    }
    setLoading(true);
    setBuilderNotice(null);
    try {
      const res = await builderCreateProposals(sessionId, decisions);
      if (res.success) {
        setReviewMode(false);
        setPendingSignals([]);
        setFinished(false);
        setPrompt(null);
        setProgress(null);
        setAnalysis(null);
        setResultModel(null);
        setMode(null);
        setSessionId(crypto.randomUUID());
        const runInfo = res.run_id ? `（Run #${res.run_id.slice(0, 8)}）` : "";
        setBuilderNotice(
          <div className="space-y-2">
            <div>
              已创建 <strong>{res.created_count}</strong> 条待确认 Proposal{runInfo}，拒绝{" "}
              <strong>{res.rejected_count}</strong> 条。
            </div>
            <div>
              <button
                onClick={() => navigate("/review")}
                className="inline-flex items-center gap-1.5 rounded-full bg-indigo-600 px-3 py-1.5 text-xs text-white hover:bg-indigo-700"
              >
                去 Review Center 确认 →
              </button>
            </div>
          </div>
        );
        await Promise.all([
          loadUnfinished(),
          getSystemDiagnostics()
            .then(setDiagnostics)
            .catch(() => null),
        ]);
      }
    } catch (e) {
      setBuilderError(buildRuntimeActionError("创建模型更新 Proposal", e, "review"));
    } finally {
      setLoading(false);
    }
  };

  const handleRejectSignals = () => {
    setBuilderError(null);
    setBuilderNotice("这轮理解已暂存到未完成会话，还没有写入人生模型。你可以稍后回来继续审阅。");
    setReviewMode(false);
    setPendingSignals([]);
    setFinished(false);
    setResultModel(null);
    setPrompt(null);
    setProgress(null);
    setAnalysis(null);
    setMode(null);
    setSessionId(crypto.randomUUID());
    loadUnfinished();
    getSystemDiagnostics()
      .then(setDiagnostics)
      .catch(() => null);
  };

  const allGoals = [
    ...(resultModel?.goals?.short_term ?? []),
    ...(resultModel?.goals?.medium_term ?? []),
    ...(resultModel?.goals?.long_term ?? []),
    ...(resultModel?.goals?.life_goals ?? []),
  ];

  const modeLabel = (m?: string) => {
    if (m === "Quick") return "快速构建";
    if (m === "Socratic") return "苏格拉底对话";
    return "渐进构建";
  };

  const radarValues = analysis
    ? [
        analysis.completion.identity,
        analysis.completion.goals,
        analysis.completion.capabilities,
        analysis.completion.state,
      ]
    : completion
      ? [completion.identity, completion.goals, completion.capabilities, completion.state]
      : [0, 0, 0, 0];
  const activeCompletion = analysis?.completion ?? completion ?? null;
  const stageSuggestions = buildStageSuggestions(activeCompletion, analysis);
  const buildOutcome = resultModel
    ? [
        {
          label: "身份线索",
          value: [
            resultModel.identity.name,
            resultModel.identity.role_definition.primary_role,
            resultModel.identity.mission_statement || resultModel.identity.life_philosophy,
          ].filter(Boolean).length,
        },
        { label: "价值观", value: resultModel.identity.values.length },
        { label: "目标", value: allGoals.length },
        {
          label: "能力资产",
          value:
            resultModel.capabilities.skills.length +
            resultModel.capabilities.resources.length +
            resultModel.capabilities.tools.length +
            resultModel.capabilities.knowledge_domains.length,
        },
        {
          label: "状态信号",
          value:
            Number(Boolean(resultModel.state.current_focus)) +
            resultModel.state.focus_areas.length +
            resultModel.state.alerts.length,
        },
      ]
    : [];
  const postBuildActions = [
    {
      title: "去人生模型核对结果",
      detail: "先确认这次提取的身份、目标和能力是否贴近你的真实状态。",
      to: "/",
    },
    {
      title: "去仪表盘查看下一步",
      detail: "看 4D 完成度、今日行动和推荐路线，决定接下来补哪一块。",
      to: "/dashboard",
    },
    {
      title: "开始第一次个性化对话",
      detail: "让 OpenLife 基于刚建立的模型做今日规划、复盘或决策陪跑。",
      to: "/chat",
    },
    {
      title: mode === "incremental" ? "继续补下一个维度" : "去校准查看建议",
      detail:
        mode === "incremental"
          ? "回到构建页继续补强下一个薄弱维度，让模型更完整。"
          : "如果你想继续精修模型，可以在周期校准里查看建议变更。",
      to: mode === "incremental" ? "/builder" : "/calibration",
    },
  ];

  const navigateAfterBuilder = (to: string) => {
    navigate(to, {
      state: {
        builderAppliedAt: Date.now(),
        refreshFromBuilder: true,
      },
    });
  };

  return (
    <div className="h-full overflow-auto bg-white p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <h2 className="text-xl font-bold text-gray-900 flex items-center gap-2">
          <Hammer className="text-indigo-600" size={22} />
          人生模型构建
        </h2>

        {safeMode && (
          <div className="rounded-xl border border-amber-200 bg-amber-50 p-4">
            <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
              <div>
                <div className="text-sm font-semibold text-amber-900">
                  Safe Mode：构建写入已暂停
                </div>
                <div className="mt-1 text-sm text-amber-800">
                  {safeModeReason} 你仍然可以查看当前构建页面，但新的构建会话、继续回答和 Review
                  应用都会被拦截。
                </div>
              </div>
              <a
                href="#/settings"
                className="inline-flex items-center gap-2 rounded-md border border-amber-300 bg-white px-3 py-2 text-sm font-medium text-amber-900 hover:bg-amber-100"
              >
                去恢复控制台 <ArrowRight size={15} />
              </a>
            </div>
          </div>
        )}

        {builderError && (
          <div className="rounded-xl border border-rose-100 bg-rose-50 p-4 text-sm text-rose-800">
            <ErrorBanner
              message={builderError}
              severity="error"
              onClose={() => setBuilderError(null)}
              className="border-0 bg-transparent p-0"
            />
            {lastStart && (
              <button
                onClick={() =>
                  start(lastStart.mode, lastStart.sessionId, lastStart.targetDimension)
                }
                className="mt-2 rounded-md bg-rose-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-rose-700"
              >
                重试启动
              </button>
            )}
          </div>
        )}

        {builderNotice && (
          <div className="rounded-xl border border-indigo-100 bg-indigo-50 p-4 text-sm text-indigo-800">
            {builderNotice}
          </div>
        )}

        {!mode && (
          <>
            {completion && completion.overall < 30 && (
              <div className="bg-amber-50 border border-amber-200 rounded-xl p-4 flex items-start gap-3">
                <AlertCircle className="text-amber-600 shrink-0 mt-0.5" size={18} />
                <div className="text-sm text-amber-800">
                  <div className="font-medium">人生模型完成度较低</div>
                  <div className="text-amber-700 mt-0.5">
                    建议通过快速构建或苏格拉底对话完善模型，以便获得更精准的对话体验。
                  </div>
                </div>
              </div>
            )}

            {unfinished.length > 0 && (
              <div className="space-y-3">
                <div className="text-sm text-gray-600 font-medium">继续未完成的会话</div>
                <div className="grid grid-cols-1 gap-3">
                  {unfinished.map(s => {
                    const totalSteps = s.mode === "Quick" ? 6 : s.mode === "Socratic" ? 8 : 1;
                    const pct = Math.min(100, Math.round((s.step_index / totalSteps) * 100));
                    const isPendingReview = s.finished && (s.pending_signals?.length ?? 0) > 0;
                    return (
                      <div
                        key={s.session_id}
                        className="border rounded-xl p-4 flex items-center justify-between bg-gray-50"
                      >
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2">
                            <div className="text-sm font-semibold text-gray-800 truncate">
                              {modeLabel(s.mode)}
                            </div>
                            {isPendingReview && (
                              <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-medium text-amber-800">
                                待确认 Review
                              </span>
                            )}
                          </div>
                          <div className="text-xs text-gray-500">
                            {isPendingReview
                              ? "已完成问题收集，等待你确认并应用模型建议"
                              : `已进行 ${s.step_index} 步`}
                          </div>
                          {s.current_prompt && (
                            <div className="text-xs text-gray-500 mt-1 line-clamp-2 max-w-md">
                              {isPendingReview
                                ? `待确认内容：${s.current_prompt}`
                                : `当前问题：${s.current_prompt}`}
                            </div>
                          )}
                          <div className="w-32 bg-gray-200 rounded-full h-1.5 mt-2">
                            <div
                              className="bg-indigo-500 h-1.5 rounded-full transition-all"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                        </div>
                        <div className="flex items-center gap-2 shrink-0 ml-3">
                          <button
                            onClick={() => resume(s)}
                            disabled={safeMode}
                            className="bg-indigo-600 text-white px-3 py-1.5 rounded-md text-sm hover:bg-indigo-700"
                          >
                            {isPendingReview ? "去审阅" : "恢复"}
                          </button>
                          <button
                            onClick={() => removeSession(s.session_id)}
                            className="text-gray-400 hover:text-red-600 px-2"
                            title="删除"
                          >
                            <Trash2 size={16} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {completion && (
              <div className="border rounded-xl p-5 bg-gray-50 space-y-4">
                <div className="flex items-center justify-between">
                  <div className="font-semibold text-gray-800">人生模型四维完成度</div>
                  <div className="text-sm text-gray-500">总体 {completion.overall}%</div>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <CompletionBar
                    label="Identity（身份认同）"
                    value={completion.identity}
                    colorClass="bg-indigo-500"
                  />
                  <CompletionBar
                    label="Goals（目标体系）"
                    value={completion.goals}
                    colorClass="bg-green-500"
                  />
                  <CompletionBar
                    label="Capabilities（能力资源）"
                    value={completion.capabilities}
                    colorClass="bg-yellow-500"
                  />
                  <CompletionBar
                    label="State（当前状态）"
                    value={completion.state}
                    colorClass="bg-purple-500"
                  />
                </div>
                {stageSuggestions.length > 0 && (
                  <div className="rounded-xl border border-indigo-100 bg-white px-4 py-3">
                    <div className="text-sm font-medium text-indigo-800 mb-2">推荐下一步</div>
                    <div className="space-y-1">
                      {stageSuggestions.map(item => (
                        <div key={item} className="text-sm text-gray-700">
                          {item}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            <div className="space-y-3">
              <div>
                <div className="text-lg font-semibold text-gray-900">选择一种构建方式</div>
                <div className="mt-1 text-sm text-gray-500">
                  不是所有人都适合从长问卷开始。你可以先快速建立可用模型，也可以一点点补全，或者做一次深度自我探索。
                </div>
              </div>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                {[
                  {
                    title: "快速构建",
                    subtitle: "我想马上开始",
                    duration: "5-10 分钟",
                    icon: <RefreshCw size={18} />,
                    tone: "from-stone-900 to-stone-700 text-amber-50",
                    detail: "少量高密度问题，先生成一个能用的人生模型。适合首次试用。",
                    bullets: ["建立四维轮廓", "低风险字段默认确认", "结束后进入 Review"],
                    action: () => start("quick", crypto.randomUUID()),
                  },
                  {
                    title: "渐进构建",
                    subtitle: "我想一点点完善",
                    duration: "每次 3-5 分钟",
                    icon: <Footprints size={18} />,
                    tone: "from-emerald-700 to-teal-600 text-white",
                    detail: "一次只补一个维度。适合已经有基础模型，想逐步精修。",
                    bullets: [
                      "选择 Identity / Goals / Capabilities / State",
                      "补弱项",
                      "应用后回到维度选择",
                    ],
                    action: () => setMode("incremental"),
                  },
                  {
                    title: "苏格拉底对话",
                    subtitle: "我想深入理解自己",
                    duration: "15-30 分钟",
                    icon: <Brain size={18} />,
                    tone: "from-amber-600 to-orange-500 text-white",
                    detail: "从峰值体验、价值冲突和人生叙事中提炼更深层的自我理解。",
                    bullets: ["开放追问", "价值排序", "先解释再写入"],
                    action: () => start("socratic", crypto.randomUUID()),
                  },
                ].map(option => (
                  <button
                    key={option.title}
                    onClick={option.action}
                    disabled={safeMode}
                    className="group overflow-hidden rounded-2xl border border-gray-200 bg-white text-left shadow-sm transition hover:-translate-y-1 hover:shadow-lg disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:translate-y-0 disabled:hover:shadow-sm"
                  >
                    <div className={`bg-gradient-to-br ${option.tone} p-4`}>
                      <div className="flex items-center justify-between gap-3">
                        <div className="inline-flex h-9 w-9 items-center justify-center rounded-xl bg-white/18">
                          {option.icon}
                        </div>
                        <span className="rounded-full bg-white/18 px-2 py-1 text-[11px] font-medium">
                          {option.duration}
                        </span>
                      </div>
                      <div className="mt-4 text-lg font-semibold">{option.title}</div>
                      <div className="text-xs opacity-85">{option.subtitle}</div>
                    </div>
                    <div className="p-4">
                      <p className="text-sm leading-6 text-gray-600">{option.detail}</p>
                      <div className="mt-3 space-y-1">
                        {option.bullets.map(item => (
                          <div key={item} className="flex items-center gap-2 text-xs text-gray-500">
                            <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                            {item}
                          </div>
                        ))}
                      </div>
                      <div className="mt-4 inline-flex items-center gap-1 text-sm font-medium text-gray-900 group-hover:text-indigo-700">
                        开始 <ArrowRight size={14} />
                      </div>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          </>
        )}

        {mode === "incremental" && !incrementalDimension && !loading && (
          <div className="space-y-6">
            {/* 渐进构建安全提示 */}
            <div className="bg-blue-50 border border-blue-100 rounded-xl p-4 flex items-start gap-3">
              <ShieldCheck className="text-blue-600 shrink-0 mt-0.5" size={18} />
              <div className="text-sm text-blue-800">
                <span className="font-medium">安全提示：</span>
                渐进构建模式下，你选择的维度只会补充该维度的内容，不会覆盖或删除其他维度已有的数据。每次构建前后都会自动创建快照，你可以随时撤销。
              </div>
            </div>
            <div className="border rounded-xl p-5 bg-gray-50 space-y-4">
              <div className="flex items-center justify-between">
                <div className="font-semibold text-gray-800">人生模型构建进度</div>
                <div className="text-sm text-gray-500">总体 {completion?.overall ?? 0}%</div>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <CompletionBar
                  label="Identity（身份认同）"
                  value={completion?.identity ?? 0}
                  colorClass="bg-indigo-500"
                />
                <CompletionBar
                  label="Goals（目标体系）"
                  value={completion?.goals ?? 0}
                  colorClass="bg-green-500"
                />
                <CompletionBar
                  label="Capabilities（能力资源）"
                  value={completion?.capabilities ?? 0}
                  colorClass="bg-yellow-500"
                />
                <CompletionBar
                  label="State（当前状态）"
                  value={completion?.state ?? 0}
                  colorClass="bg-purple-500"
                />
              </div>
              {stageSuggestions.length > 0 && (
                <div className="rounded-xl border border-indigo-100 bg-white px-4 py-3">
                  <div className="text-sm font-medium text-indigo-800 mb-2">推荐下一步</div>
                  <div className="space-y-1">
                    {stageSuggestions.map(item => (
                      <div key={item} className="text-sm text-gray-700">
                        {item}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              {[
                {
                  key: "identity" as const,
                  label: "Identity",
                  title: "我是谁",
                  color: "indigo",
                  icon: <Zap size={18} />,
                  subItems: ["价值观", "身份角色", "人生叙事"],
                },
                {
                  key: "goals" as const,
                  label: "Goals",
                  title: "我要去哪里",
                  color: "green",
                  icon: <Target size={18} />,
                  subItems: ["长期方向", "中期项目", "短期行动"],
                },
                {
                  key: "capabilities" as const,
                  label: "Capabilities",
                  title: "我有什么",
                  color: "yellow",
                  icon: <Heart size={18} />,
                  subItems: ["能力", "资源", "限制", "学习路径"],
                },
                {
                  key: "state" as const,
                  label: "State",
                  title: "我现在怎么样",
                  color: "purple",
                  icon: <Sparkles size={18} />,
                  subItems: ["当前状态", "能量", "压力", "习惯"],
                },
              ].map(dim => {
                const pct = (completion?.[dim.key as keyof Model4DCompletion] as number) ?? 0;
                const styles = dimensionStyleMap[dim.key];
                const isRecommended = stageSuggestions.some(s =>
                  s.toLowerCase().includes(dim.label.toLowerCase())
                );
                const dimGaps =
                  analysis?.gaps?.filter(g => g.toLowerCase().includes(dim.key.toLowerCase())) ??
                  [];
                const lowCompletion = pct < 40;
                return (
                  <button
                    key={dim.key}
                    data-testid={`incremental-dim-${dim.key}`}
                    onClick={() => {
                      setIncrementalDimension(dim.key);
                      start("incremental", crypto.randomUUID(), dim.key);
                    }}
                    disabled={safeMode}
                    className={`border rounded-xl p-5 text-left transition relative ${styles.hover} ${isRecommended ? "ring-2 ring-indigo-200" : ""} disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:bg-white disabled:hover:border-gray-200`}
                  >
                    {isRecommended && (
                      <span className="absolute top-2 right-2 text-xs bg-indigo-100 text-indigo-700 px-2 py-0.5 rounded-full">
                        推荐
                      </span>
                    )}
                    <div className={`flex items-center gap-2 font-semibold ${styles.text} mb-2`}>
                      {dim.icon} {dim.label}
                    </div>
                    <div className="text-sm text-gray-600 mb-2">{dim.title}</div>
                    <div className="flex flex-wrap gap-1 mb-3">
                      {dim.subItems.map(item => (
                        <span
                          key={item}
                          className="text-[10px] bg-gray-100 text-gray-600 px-1.5 py-0.5 rounded"
                        >
                          {item}
                        </span>
                      ))}
                    </div>
                    {(dimGaps.length > 0 || lowCompletion) && (
                      <div className="mb-2 text-xs space-y-0.5">
                        {dimGaps.length > 0 && (
                          <div className="text-amber-700">
                            缺口：{dimGaps.slice(0, 2).join("、")}
                          </div>
                        )}
                        {lowCompletion && !dimGaps.length && (
                          <div className="text-amber-700">完成度较低，建议优先补充</div>
                        )}
                      </div>
                    )}
                    <div className="w-full bg-gray-200 rounded-full h-2">
                      <div
                        className={`${styles.bar} h-2 rounded-full transition-all`}
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                    <div className="text-xs text-gray-500 mt-1">{pct}% 完成</div>
                  </button>
                );
              })}
            </div>

            <button
              onClick={() => {
                setMode(null);
                setIncrementalDimension(null);
              }}
              className="text-sm text-gray-500 hover:text-gray-700"
            >
              ← 返回构建首页
            </button>
          </div>
        )}

        {mode && loading && !prompt && (
          <div className="py-12">
            <LoadingSpinner text="正在启动构建会话..." />
          </div>
        )}

        {mode && prompt && !finished && (
          <div className="space-y-4">
            {progress && progress.total_steps > 0 && (
              <div className="space-y-3">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-gray-600">{progress.current_step_label}</span>
                  <span className="text-gray-500">
                    {progress.step_index} / {progress.total_steps}
                  </span>
                </div>
                <ModeStepper mode={mode} progress={progress} />
                <div className="w-full bg-gray-200 rounded-full h-2">
                  <div
                    className="bg-indigo-600 h-2 rounded-full transition-all"
                    style={{ width: `${Math.max(5, Math.min(100, progress.progress * 100))}%` }}
                  />
                </div>
              </div>
            )}

            {(analysis || completion) && (
              <div className="border rounded-xl p-4 bg-gray-50">
                <div className="flex items-center gap-4">
                  <RadarChart values={radarValues} size={120} />
                  <div className="flex-1 space-y-2">
                    <div className="text-sm font-medium text-gray-800">
                      当前 4D 完成度: {analysis?.completion.overall ?? completion?.overall ?? 0}%
                    </div>
                    <div className="grid grid-cols-2 gap-2 text-xs text-gray-600">
                      <div>
                        Identity {analysis?.completion.identity ?? completion?.identity ?? 0}%
                      </div>
                      <div>Goals {analysis?.completion.goals ?? completion?.goals ?? 0}%</div>
                      <div>
                        Capabilities{" "}
                        {analysis?.completion.capabilities ?? completion?.capabilities ?? 0}%
                      </div>
                      <div>State {analysis?.completion.state ?? completion?.state ?? 0}%</div>
                    </div>
                  </div>
                </div>
                {analysis && analysis.gaps.length > 0 && (
                  <div className="mt-3 pt-3 border-t text-sm">
                    <div className="text-gray-700 font-medium mb-1">待补充维度</div>
                    <div className="flex flex-wrap gap-2">
                      {analysis.gaps.map((g, i) => (
                        <span
                          key={i}
                          className="bg-white text-gray-700 px-2 py-1 rounded-md text-xs border"
                        >
                          {g}
                        </span>
                      ))}
                    </div>
                  </div>
                )}
                {stageSuggestions.length > 0 && (
                  <div className="mt-3 pt-3 border-t text-sm">
                    <div className="text-gray-700 font-medium mb-2">当前阶段建议</div>
                    <div className="space-y-1">
                      {stageSuggestions.map(item => (
                        <div key={item} className="text-gray-600">
                          {item}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            {progress?.waiting_phase_confirmation && progress?.phase_summary && (
              <div className="bg-amber-50 border border-amber-200 rounded-xl p-5 space-y-3">
                <div className="flex items-center gap-2 text-amber-800 font-semibold">
                  <Sparkles size={18} className="text-amber-600" />
                  阶段性理解确认
                </div>
                <div className="text-sm text-amber-900 whitespace-pre-line">
                  {progress.phase_summary}
                </div>
                <div className="flex gap-3">
                  <button
                    onClick={() => sendReply("确认")}
                    className="bg-amber-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-amber-700"
                  >
                    确认，继续
                  </button>
                </div>
              </div>
            )}
            <div className="bg-gray-50 border rounded-xl p-5 text-sm text-gray-800 whitespace-pre-line">
              {prompt}
            </div>
            {progress?.waiting_pairwise && (
              <div className="flex flex-wrap gap-3">
                <button
                  onClick={() => sendReply("A")}
                  className="bg-white border border-indigo-200 text-indigo-700 px-4 py-2 rounded-lg hover:bg-indigo-50"
                >
                  选 A
                </button>
                <button
                  onClick={() => sendReply("B")}
                  className="bg-white border border-indigo-200 text-indigo-700 px-4 py-2 rounded-lg hover:bg-indigo-50"
                >
                  选 B
                </button>
              </div>
            )}
            <div className="flex gap-3">
              <textarea
                value={input}
                onChange={e => setInput(e.target.value)}
                onKeyDown={e => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    sendReply();
                  }
                }}
                rows={3}
                placeholder={
                  progress?.waiting_pairwise ? "输入 A、B 或你的描述..." : "输入你的回答..."
                }
                disabled={safeMode}
                className="flex-1 resize-none border rounded-lg px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
              />
              <button
                onClick={() => sendReply()}
                disabled={loading || !input.trim() || safeMode}
                className="bg-indigo-600 text-white px-5 py-2 rounded-lg hover:bg-indigo-700 disabled:opacity-50 flex items-center gap-2"
              >
                {loading ? (
                  <RefreshCw size={16} className="animate-spin" />
                ) : (
                  <ArrowRight size={16} />
                )}
                下一步
              </button>
            </div>
          </div>
        )}

        {reviewMode && pendingSignals.length > 0 && (
          <div className="bg-white border rounded-xl p-6 shadow-sm">
            <BuilderPatchReview
              signals={pendingSignals}
              summary={{
                identity_summary: `基于 ${pendingSignals.filter(s => s.dimension === "Identity").length} 个信号`,
                goals_summary: `基于 ${pendingSignals.filter(s => s.dimension === "Goals").length} 个信号`,
                capabilities_summary: `基于 ${pendingSignals.filter(s => s.dimension === "Capabilities").length} 个信号`,
                state_summary: `基于 ${pendingSignals.filter(s => s.dimension === "State").length} 个信号`,
                assumptions: ["用户通过快速构建流程提供"],
                unresolved_questions: [],
                recommended_next_steps: ["审阅并确认信号", "可选择进入渐进构建继续完善"],
              }}
              onCreateProposals={handleCreateProposals}
              onReject={handleRejectSignals}
            />
          </div>
        )}

        {finished && (
          <div className="space-y-6">
            <div className="bg-green-50 border border-green-200 rounded-xl p-6 text-center space-y-3">
              <CheckCircle2 className="mx-auto text-green-600" size={40} />
              <div className="text-green-800 font-semibold">构建完成！</div>
              <p className="text-sm text-green-700">
                你的人生模型已更新，可以到"人生模型"页面查看和编辑。
              </p>
              <button
                onClick={reset}
                className="bg-white border px-4 py-2 rounded-lg text-sm hover:bg-gray-50"
              >
                再建一次
              </button>
            </div>

            {beforeBuildCompletion && completion && (
              <div className="border rounded-xl p-5 bg-gray-50 space-y-4">
                <div className="flex items-center gap-2 text-gray-900 font-semibold">
                  <Sparkles className="text-indigo-600" size={18} />
                  四维完成度变化
                </div>
                <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
                  {[
                    { key: "identity" as const, label: "Identity", color: "indigo" },
                    { key: "goals" as const, label: "Goals", color: "green" },
                    { key: "capabilities" as const, label: "Capabilities", color: "yellow" },
                    { key: "state" as const, label: "State", color: "purple" },
                  ].map(dim => {
                    const before = beforeBuildCompletion[dim.key];
                    const after = completion?.[dim.key] ?? 0;
                    const delta = after - before;
                    const colorClass =
                      dim.color === "indigo"
                        ? "text-indigo-600"
                        : dim.color === "green"
                          ? "text-green-600"
                          : dim.color === "yellow"
                            ? "text-amber-600"
                            : "text-purple-600";
                    return (
                      <div key={dim.key} className="bg-white rounded-lg p-3 border text-center">
                        <div className="text-xs text-gray-500 mb-1">{dim.label}</div>
                        <div className={`text-lg font-bold ${colorClass}`}>{after}%</div>
                        <div className="text-xs mt-1">
                          <span className="text-gray-400">{before}%</span>
                          {delta > 0 ? (
                            <span className="text-green-600 ml-1 font-medium">
                              +{delta.toFixed(1)}%
                            </span>
                          ) : delta < 0 ? (
                            <span className="text-rose-600 ml-1 font-medium">
                              {delta.toFixed(1)}%
                            </span>
                          ) : (
                            <span className="text-gray-400 ml-1">—</span>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {appliedFields.length > 0 && (
              <div className="border rounded-xl p-5 bg-white space-y-3">
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
                  <div className="rounded-lg border border-green-100 bg-green-50 px-4 py-3">
                    <div className="text-xs text-green-700">已写入</div>
                    <div className="mt-1 text-2xl font-semibold text-green-800">
                      {appliedFields.length}
                    </div>
                    <div className="mt-1 text-[11px] text-green-700">
                      这部分已经进入你的人生模型，可直接用于后续对话。
                    </div>
                  </div>
                  <div className="rounded-lg border border-indigo-100 bg-indigo-50 px-4 py-3">
                    <div className="text-xs text-indigo-700">已合并</div>
                    <div className="mt-1 text-2xl font-semibold text-indigo-800">
                      {mergedFields.length}
                    </div>
                    <div className="mt-1 text-[11px] text-indigo-700">
                      系统尝试保留你已有内容，而不是整段覆盖。
                    </div>
                  </div>
                  <div className="rounded-lg border border-amber-100 bg-amber-50 px-4 py-3">
                    <div className="text-xs text-amber-700">待处理</div>
                    <div className="mt-1 text-2xl font-semibold text-amber-800">
                      {skippedFields.length}
                    </div>
                    <div className="mt-1 text-[11px] text-amber-700">
                      这些内容暂时没写入，通常是因为结构不完整或字段类型不匹配。
                    </div>
                  </div>
                  <div className="rounded-lg border border-sky-100 bg-sky-50 px-4 py-3">
                    <div className="text-xs text-sky-700">已编辑</div>
                    <div className="mt-1 text-2xl font-semibold text-sky-800">{editedCount}</div>
                    <div className="mt-1 text-[11px] text-sky-700">
                      这些字段经过你的手动确认或修订后再写入。
                    </div>
                  </div>
                  <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3">
                    <div className="text-xs text-slate-700">已拒绝</div>
                    <div className="mt-1 text-2xl font-semibold text-slate-800">
                      {rejectedCount}
                    </div>
                    <div className="mt-1 text-[11px] text-slate-700">
                      这些内容本轮明确不写入，后续仍可继续构建补充。
                    </div>
                  </div>
                </div>

                <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700">
                  <div className="font-medium text-slate-900">本轮写入结果</div>
                  <div className="mt-1 text-xs leading-relaxed">
                    先看“已写入”，确认核心内容已经进入模型；再看“已合并”和“已编辑”，理解系统如何保护旧数据并尊重你的确认；如果有“待处理”，建议去人生模型页或继续渐进构建补齐。
                  </div>
                </div>

                <div className="flex items-center gap-2 text-gray-900 font-semibold">
                  <CheckCircle2 className="text-green-600" size={18} />
                  已写入的人生模型字段
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  {appliedFields.map((field, i) => (
                    <div
                      key={i}
                      className="flex items-center gap-2 text-sm text-gray-700 bg-gray-50 rounded-lg px-3 py-2"
                    >
                      <span className="w-1.5 h-1.5 rounded-full bg-green-500 shrink-0" />
                      <span className="truncate" title={field}>
                        {field}
                      </span>
                    </div>
                  ))}
                </div>
                {mergedFields.length > 0 && (
                  <div className="text-xs text-emerald-700 bg-emerald-50 rounded-lg px-3 py-2 space-y-1">
                    <div className="font-medium">已合并的字段</div>
                    <div>系统在保留旧内容的前提下，吸收了这轮新信息：{mergedFields.join("、")}</div>
                  </div>
                )}
                {skippedFields.length > 0 && (
                  <div className="text-xs text-amber-800 bg-amber-50 rounded-lg px-3 py-2 space-y-1">
                    <div className="font-medium">这次先没写入的内容</div>
                    {skippedFields.map((field, i) => (
                      <div key={`${field.path}-${i}`}>
                        {field.path}: {field.reason}
                        {field.expected ? `（期望：${field.expected}）` : ""}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {buildOutcome.length > 0 && (
              <div className="border rounded-xl p-5 bg-white space-y-4">
                <div className="flex items-center gap-2 text-gray-900 font-semibold">
                  <Sparkles className="text-indigo-600" size={18} />
                  本轮沉淀
                </div>
                <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
                  {buildOutcome.map(item => (
                    <div
                      key={item.label}
                      className="rounded-lg border bg-gray-50 px-3 py-3 text-center"
                    >
                      <div className="text-xs text-gray-500">{item.label}</div>
                      <div className="mt-1 text-xl font-semibold text-gray-900">{item.value}</div>
                    </div>
                  ))}
                </div>
                <div className="text-xs text-gray-500">
                  这些数字表示这轮构建至少沉淀出了多少条可用于后续对话与校准的模型线索。
                </div>
              </div>
            )}

            {resultModel && (
              <div className="border rounded-xl p-5 space-y-5">
                <div className="flex items-center gap-2 text-gray-900 font-semibold">
                  <Sparkles className="text-indigo-600" size={20} />
                  提取结果摘要
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="bg-slate-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-slate-800 font-medium mb-2">
                      <Zap size={16} /> 身份与使命
                    </div>
                    <div className="space-y-1 text-sm text-slate-900">
                      <div>
                        主角色：{resultModel.identity.role_definition.primary_role || "未提取"}
                      </div>
                      <div>使命：{resultModel.identity.mission_statement || "未提取"}</div>
                    </div>
                  </div>

                  <div className="bg-indigo-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-indigo-800 font-medium mb-2">
                      <Heart size={16} /> 核心价值观
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {resultModel.identity.values.length ? (
                        resultModel.identity.values.map((v, i) => (
                          <span
                            key={i}
                            className="bg-white text-indigo-700 px-2 py-1 rounded-md text-sm border border-indigo-100"
                          >
                            {v.name} {v.weight > 0 ? `(权重${v.weight})` : ""}
                          </span>
                        ))
                      ) : (
                        <EmptyState
                          title="暂无价值观"
                          description="本次构建未提取到核心价值观"
                          className="py-2"
                        />
                      )}
                    </div>
                  </div>

                  <div className="bg-green-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-green-800 font-medium mb-2">
                      <Target size={16} /> 目标
                    </div>
                    <div className="space-y-1">
                      {allGoals.length ? (
                        allGoals.slice(0, 3).map((g, i) => (
                          <div key={i} className="text-sm text-green-900 truncate">
                            • {g.name}
                          </div>
                        ))
                      ) : (
                        <EmptyState
                          title="暂无目标"
                          description="本次构建未提取到目标"
                          className="py-2"
                        />
                      )}
                      {allGoals.length > 3 && (
                        <div className="text-xs text-green-700">+{allGoals.length - 3} 个目标</div>
                      )}
                    </div>
                  </div>

                  <div className="bg-yellow-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-yellow-800 font-medium mb-2">
                      <Zap size={16} /> 技能
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {resultModel.capabilities.skills.length ? (
                        resultModel.capabilities.skills.map((s, i) => (
                          <span
                            key={i}
                            className="bg-white text-yellow-700 px-2 py-1 rounded-md text-sm border border-yellow-100"
                          >
                            {s.name} {s.proficiency > 0 ? `(熟练度${s.proficiency})` : ""}
                          </span>
                        ))
                      ) : (
                        <EmptyState
                          title="暂无技能"
                          description="本次构建未提取到技能"
                          className="py-2"
                        />
                      )}
                    </div>
                  </div>

                  <div className="bg-purple-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-purple-800 font-medium mb-2">
                      <Brain size={16} /> 当前状态
                    </div>
                    <div className="text-sm text-purple-900 space-y-1">
                      <div>心情: {resultModel.state.emotional_state.current_mood || "—"}</div>
                      <div>压力: {resultModel.state.emotional_state.stress_level}/10</div>
                      <div>满足度: {resultModel.state.emotional_state.fulfillment_score}/10</div>
                      <div>精力: {resultModel.state.health_status.energy_level}/10</div>
                    </div>
                  </div>

                  <div className="bg-amber-50 rounded-lg p-4">
                    <div className="flex items-center gap-2 text-amber-800 font-medium mb-2">
                      <Brain size={16} /> 能力与知识域
                    </div>
                    <div className="space-y-1">
                      {resultModel.capabilities.skills.length ? (
                        resultModel.capabilities.skills.slice(0, 3).map((skill, i) => (
                          <div key={i} className="text-sm text-amber-900">
                            • {skill.name} ({skill.proficiency}/10)
                          </div>
                        ))
                      ) : (
                        <EmptyState
                          title="暂无能力画像"
                          description="本次构建未提取到关键能力"
                          className="py-2"
                        />
                      )}
                      {resultModel.capabilities.knowledge_domains.length > 0 && (
                        <div className="pt-2 text-xs text-amber-700">
                          知识域：
                          {resultModel.capabilities.knowledge_domains
                            .slice(0, 3)
                            .map(d => d.domain)
                            .join("、")}
                        </div>
                      )}
                    </div>
                  </div>
                </div>

                <div className="rounded-xl border border-indigo-100 bg-indigo-50/60 p-4 space-y-3">
                  <div className="text-sm font-medium text-indigo-800">下一步建议</div>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    {postBuildActions.map(action => (
                      <button
                        key={action.title}
                        onClick={() => navigateAfterBuilder(action.to)}
                        className="flex items-center gap-2 rounded-lg border border-indigo-200 bg-white px-4 py-3 text-sm text-indigo-700 hover:bg-indigo-50 transition-colors text-left"
                      >
                        <ArrowRight size={16} />
                        <span>
                          <span className="block font-medium">{action.title}</span>
                          <span className="mt-0.5 block text-xs text-indigo-500">
                            {action.detail}
                          </span>
                        </span>
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
