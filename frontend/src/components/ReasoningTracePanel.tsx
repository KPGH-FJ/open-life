import { Lightbulb, Clock, AlertCircle, Terminal, Compass, Target } from "lucide-react";
import type { ReasoningTrace } from "../tauri";

interface Props {
  trace: ReasoningTrace;
  show: boolean;
  onToggle: () => void;
}

function formatTimingMs(ms?: number): string {
  if (ms === undefined || ms === null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function boundedTraceString(value: unknown): string {
  if (typeof value !== "string") return "";
  return value
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .trim()
    .slice(0, 140);
}

function traceStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map(boundedTraceString).filter(Boolean).slice(0, 8);
}

function runtimeRouteRows(generation: any): Array<{ label: string; value: string }> {
  if (!generation || typeof generation !== "object") return [];
  const routeLabels = traceStringArray(generation.routeLabels);
  const rows = routeLabels.map(label => {
    const [prefix, ...rest] = label.split(":");
    return {
      label: prefix ? prefix.replace(/_/g, " ") : "route evidence",
      value: rest.join(":").trim() || label,
    };
  });
  const preflightStatus = boundedTraceString(generation.providerPreflightStatus);
  if (preflightStatus) {
    const blockers = traceStringArray(generation.providerPreflightBlockers).join(", ");
    rows.push({
      label: "provider preflight",
      value: blockers ? `${preflightStatus} (${blockers})` : preflightStatus,
    });
  }
  return rows.slice(0, 6);
}

function LayerBlock({
  icon: Icon,
  label,
  color,
  text,
  timingKey,
  timings,
}: {
  icon: React.ElementType;
  label: string;
  color: string;
  text?: string;
  timingKey?: string;
  timings?: Record<string, number>;
}) {
  if (!text) return null;
  const timing = timingKey && timings ? timings[timingKey] : undefined;
  return (
    <div className={`rounded-lg border ${color} bg-white/60 p-3`}>
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 font-medium">
          <Icon size={14} />
          {label}
        </div>
        {timing !== undefined && (
          <span className="inline-flex items-center gap-1 rounded-full bg-white px-2 py-0.5 text-[10px] font-medium text-gray-600 shadow-sm">
            <Clock size={10} />
            {formatTimingMs(timing)}
          </span>
        )}
      </div>
      <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed opacity-90">{text}</div>
    </div>
  );
}

export default function ReasoningTracePanel({ trace, show, onToggle }: Props) {
  const meaningText =
    trace.meaning_result?.text ??
    (typeof trace.meaning_result === "string" ? trace.meaning_result : "");
  const strategyText =
    trace.strategy_result?.text ??
    (typeof trace.strategy_result === "string" ? trace.strategy_result : "");
  const generationText =
    trace.generation_result?.text ??
    (typeof trace.generation_result === "string" ? trace.generation_result : "");
  const alignedValues = trace.meaning_result?.aligned_values ?? [];
  const alignedGoals = trace.strategy_result?.aligned_goals ?? [];
  const planSteps = trace.strategy_result?.plan_steps ?? [];
  const stableSteps = trace.stable_steps ?? [];
  const needsTools = trace.strategy_result?.needs_tools;
  const toolPlan = trace.tool_plan ?? trace.strategy_result?.suggested_tools ?? [];
  const safetyCheckWarnings = trace.safety_check_result?.warnings ?? [];
  const runtimeRouteEvidenceRows = runtimeRouteRows(trace.generation_result);
  const sourceChip = boundedTraceString(trace.generation_result?.uiPrimarySourceChip);
  const uiStatus = boundedTraceString(trace.generation_result?.uiStatus);
  const hasContent =
    trace.input ||
    meaningText ||
    strategyText ||
    generationText ||
    runtimeRouteEvidenceRows.length > 0 ||
    trace.output ||
    toolPlan.length > 0 ||
    safetyCheckWarnings.length > 0 ||
    (trace.errors && trace.errors.length > 0);

  const totalMs = trace.layer_timings_ms
    ? Object.values(trace.layer_timings_ms).reduce((a, b) => a + (b || 0), 0)
    : 0;
  const summaryItems = [
    Array.isArray(alignedValues) && alignedValues.length > 0
      ? `参考价值观：${alignedValues.slice(0, 2).join("、")}`
      : "",
    Array.isArray(alignedGoals) && alignedGoals.length > 0
      ? `对齐目标：${alignedGoals.slice(0, 2).join("、")}`
      : "",
    Array.isArray(toolPlan) && toolPlan.length > 0
      ? `计划工具：${toolPlan.slice(0, 2).join("、")}`
      : "无需外部工具",
    sourceChip ? `来源：${sourceChip}` : "",
    uiStatus ? `状态：${uiStatus}` : "",
  ].filter(Boolean);

  return (
    <div className="max-w-2xl px-4 py-3 rounded-xl text-sm bg-indigo-50 text-indigo-900 border border-indigo-100">
      <button
        onClick={onToggle}
        className="flex items-center gap-2 font-medium mb-1 w-full"
        aria-expanded={show}
      >
        <Lightbulb size={16} />
        <span className="flex-1 text-left">为什么这样回答</span>
        {totalMs > 0 && (
          <span className="inline-flex items-center gap-1 rounded-full bg-white px-2 py-0.5 text-[10px] font-medium text-gray-600 shadow-sm">
            <Clock size={10} />
            总耗时 {formatTimingMs(totalMs)}
          </span>
        )}
        <span className="text-xs">{show ? "▲" : "▼"}</span>
      </button>
      <div className="flex flex-wrap gap-2 pt-1">
        {summaryItems.map(item => (
          <span
            key={item}
            className="rounded-full bg-white/80 px-2.5 py-1 text-[10px] font-medium text-indigo-700 border border-indigo-100"
          >
            {item}
          </span>
        ))}
        {safetyCheckWarnings.length > 0 && (
          <span className="rounded-full bg-amber-50 px-2.5 py-1 text-[10px] font-medium text-amber-700 border border-amber-100">
            有 {safetyCheckWarnings.length} 条不确定性提醒
          </span>
        )}
        {trace.errors && trace.errors.length > 0 && (
          <span className="rounded-full bg-red-50 px-2.5 py-1 text-[10px] font-medium text-red-700 border border-red-100">
            有 {trace.errors.length} 条错误
          </span>
        )}
      </div>
      {show && (
        <div className="space-y-3 text-xs opacity-95 pt-1">
          {hasContent ? (
            <>
              <div className="rounded-lg border border-indigo-100 bg-white/70 p-3 text-gray-700">
                <div className="font-medium text-indigo-800">回答思路</div>
                <p className="mt-1 leading-relaxed">
                  下面是 OpenLife
                  思考你问题的过程。它不代表绝对判断，而是让你知道回答从哪里来、有哪些不确定性。
                </p>
              </div>
              {trace.input && (
                <div className="rounded-lg border border-gray-200 bg-white/70 p-3">
                  <div className="flex items-center gap-2 font-medium text-gray-700">
                    <Terminal size={14} />
                    输入
                  </div>
                  <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed text-gray-800">
                    {trace.input}
                  </div>
                </div>
              )}
              <LayerBlock
                icon={Compass}
                label="理解你"
                color="border-indigo-200"
                text={meaningText}
                timingKey="Meaning"
                timings={trace.layer_timings_ms}
              />
              {Array.isArray(alignedValues) && alignedValues.length > 0 && (
                <div className="rounded-lg border border-indigo-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-indigo-700">对齐价值观</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {alignedValues.map((value: string) => (
                      <span
                        key={value}
                        className="rounded-full bg-indigo-50 px-2 py-0.5 text-[10px] text-indigo-700 border border-indigo-100"
                      >
                        {value}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              <LayerBlock
                icon={Target}
                label="规划思路"
                color="border-purple-200"
                text={strategyText}
                timingKey="Strategy"
                timings={trace.layer_timings_ms}
              />
              {Array.isArray(alignedGoals) && alignedGoals.length > 0 && (
                <div className="rounded-lg border border-purple-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-purple-700">优先目标</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {alignedGoals.map((goal: string) => (
                      <span
                        key={goal}
                        className="rounded-full bg-purple-50 px-2 py-0.5 text-[10px] text-purple-700 border border-purple-100"
                      >
                        {goal}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {Array.isArray(planSteps) && planSteps.length > 0 && (
                <div className="rounded-lg border border-purple-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-purple-700">计划步骤</div>
                  <ol className="mt-2 list-decimal pl-4 text-xs text-gray-700 space-y-1">
                    {planSteps.map((step: string, idx: number) => (
                      <li key={`${idx}-${step}`}>{step}</li>
                    ))}
                  </ol>
                  {typeof needsTools === "boolean" && (
                    <div className="mt-2 text-[11px] text-gray-500">
                      工具需求：{needsTools ? "需要外部工具/信息" : "当前无需外部工具"}
                    </div>
                  )}
                </div>
              )}
              {Array.isArray(stableSteps) && stableSteps.length > 0 && (
                <div className="rounded-lg border border-indigo-100 bg-indigo-50/60 p-3">
                  <div className="text-[11px] font-medium text-indigo-700">
                    稳定步骤计划（已规范化）
                  </div>
                  <ol className="mt-2 list-decimal pl-4 text-xs text-gray-700 space-y-1">
                    {stableSteps.map((step: string, idx: number) => (
                      <li key={`stable-${idx}-${step}`}>{step}</li>
                    ))}
                  </ol>
                </div>
              )}
              {Array.isArray(toolPlan) && toolPlan.length > 0 && (
                <div className="rounded-lg border border-amber-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-amber-700">建议工具链</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {toolPlan.map((tool: string) => (
                      <span
                        key={tool}
                        className="rounded-full bg-amber-50 px-2 py-0.5 text-[10px] text-amber-700 border border-amber-100"
                      >
                        {tool}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {runtimeRouteEvidenceRows.length > 0 && (
                <div className="rounded-lg border border-cyan-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-cyan-800">模型路线证据</div>
                  <dl className="mt-2 space-y-1.5">
                    {runtimeRouteEvidenceRows.map(row => (
                      <div
                        key={`${row.label}-${row.value}`}
                        className="grid gap-1 text-xs text-gray-700 sm:grid-cols-[150px_1fr]"
                      >
                        <dt className="font-medium text-cyan-700">{row.label}</dt>
                        <dd className="break-words">{row.value}</dd>
                      </div>
                    ))}
                  </dl>
                </div>
              )}
              <LayerBlock
                icon={Terminal}
                label="组织回答"
                color="border-emerald-200"
                text={generationText}
                timingKey="Execution"
                timings={trace.layer_timings_ms}
              />
              {trace.output && (
                <div className="rounded-lg border border-emerald-200 bg-emerald-50/60 p-3">
                  <div className="flex items-center gap-2 font-medium text-emerald-800">
                    <Terminal size={14} />
                    最终输出
                  </div>
                  <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed text-emerald-900">
                    {trace.output}
                  </div>
                </div>
              )}
              {Array.isArray(safetyCheckWarnings) && safetyCheckWarnings.length > 0 && (
                <div className="rounded-lg border border-amber-200 bg-amber-50/70 p-3">
                  <div className="flex items-center gap-2 font-semibold text-amber-700">
                    <AlertCircle size={14} />
                    仲裁提醒
                  </div>
                  <ul className="mt-2 list-disc pl-4 text-amber-700">
                    {safetyCheckWarnings.map((warning: string, i: number) => (
                      <li key={i}>{warning}</li>
                    ))}
                  </ul>
                </div>
              )}
              {trace.errors && trace.errors.length > 0 && (
                <div className="rounded-lg border border-red-200 bg-red-50/70 p-3">
                  <div className="flex items-center gap-2 font-semibold text-red-700">
                    <AlertCircle size={14} />
                    错误
                  </div>
                  <ul className="mt-2 list-disc pl-4 text-red-700">
                    {trace.errors.map((e, i) => (
                      <li key={i}>{e}</li>
                    ))}
                  </ul>
                </div>
              )}
            </>
          ) : (
            <div className="text-gray-500">暂无思考详情</div>
          )}
        </div>
      )}
    </div>
  );
}
