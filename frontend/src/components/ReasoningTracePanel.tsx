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

function sanitizeTraceText(value: string, maxLength: number): string {
  const cleaned = value
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .replace(/\/Users\/[^\s]+/g, "[workspace path]")
    .replace(/[A-Za-z]:\\[^\s]+/g, "[workspace path]")
    .trim()
    .slice(0, maxLength);
  if (cleaned.startsWith("/") || /^[A-Za-z]:[\\/]/.test(cleaned)) {
    return "workspace item";
  }
  return cleaned;
}

function boundedTraceString(value: unknown): string {
  if (typeof value !== "string" && typeof value !== "boolean" && typeof value !== "number") {
    return "";
  }
  return sanitizeTraceText(String(value), 140);
}

function boundedTraceText(value: unknown): string {
  if (typeof value !== "string" && typeof value !== "boolean" && typeof value !== "number") {
    return "";
  }
  return sanitizeTraceText(String(value), 900);
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

function runtimeToolRows(generation: any): Array<{ label: string; value: string }> {
  if (!generation || typeof generation !== "object") return [];
  const rows = traceStringArray(generation.toolAvailabilityLabels).map(label => {
    const [prefix, ...rest] = label.split(":");
    return {
      label: prefix ? prefix.replace(/_/g, " ") : "tool evidence",
      value: rest.join(":").trim() || label,
    };
  });
  const webPolicy = boundedTraceString(generation.toolWebPolicyAllowed);
  const webReachability = boundedTraceString(generation.toolWebReachabilityStatus);
  const webAvailable = boundedTraceString(generation.toolWebAvailable);
  if (webPolicy || webReachability || webAvailable) {
    rows.push({
      label: "web availability",
      value: [
        webPolicy ? `policy=${webPolicy}` : "",
        webReachability ? `reachability=${webReachability}` : "",
        webAvailable ? `available=${webAvailable}` : "",
      ]
        .filter(Boolean)
        .join(" · "),
    });
  }
  const mcpSafeReadCount = boundedTraceString(generation.toolMcpSafeReadCandidateCount);
  const mcpServerStatus = boundedTraceString(generation.toolMcpServerStatus);
  const mcpAvailable = boundedTraceString(generation.toolMcpAvailable);
  if (mcpSafeReadCount || mcpServerStatus || mcpAvailable) {
    rows.push({
      label: "mcp availability",
      value: [
        mcpSafeReadCount ? `safeRead=${mcpSafeReadCount}` : "",
        mcpServerStatus ? `server=${mcpServerStatus}` : "",
        mcpAvailable ? `available=${mcpAvailable}` : "",
      ]
        .filter(Boolean)
        .join(" · "),
    });
  }
  const writeAvailable = boundedTraceString(generation.toolWriteAvailable);
  const writeRequiresPermission = boundedTraceString(generation.toolWriteRequiresPermission);
  if (writeAvailable || writeRequiresPermission) {
    rows.push({
      label: "write policy",
      value: [
        writeAvailable ? `available=${writeAvailable}` : "",
        writeRequiresPermission ? `requiresPermission=${writeRequiresPermission}` : "",
      ]
        .filter(Boolean)
        .join(" · "),
    });
  }
  return rows.slice(0, 8);
}

function runtimeSelfStateRows(generation: any): Array<{ label: string; value: string }> {
  if (!generation || typeof generation !== "object") return [];
  const rows: Array<{ label: string; value: string }> = [];
  const taskStatus = boundedTraceString(generation.taskStatus);
  const runStatus = boundedTraceString(generation.runStatus);
  const deliveryStatus = boundedTraceString(generation.deliveryStatus);
  if (taskStatus || runStatus || deliveryStatus) {
    rows.push({
      label: "task status",
      value: [
        taskStatus ? `task=${taskStatus}` : "",
        runStatus ? `run=${runStatus}` : "",
        deliveryStatus ? `delivery=${deliveryStatus}` : "",
      ]
        .filter(Boolean)
        .join(" · "),
    });
  }
  const pendingPermissionCount = boundedTraceString(generation.pendingPermissionCount);
  const pendingProposalCount = boundedTraceString(generation.pendingProposalCount);
  const durableChangeStatus = boundedTraceString(generation.durableChangeStatus);
  if (pendingPermissionCount || pendingProposalCount || durableChangeStatus) {
    rows.push({
      label: "pending state",
      value: [
        pendingPermissionCount ? `permission=${pendingPermissionCount}` : "",
        pendingProposalCount ? `proposal=${pendingProposalCount}` : "",
        durableChangeStatus ? `durable=${durableChangeStatus}` : "",
      ]
        .filter(Boolean)
        .join(" · "),
    });
  }
  const lastActionSummary = boundedTraceString(generation.lastActionSummary);
  const observationCount = boundedTraceString(generation.observationCount);
  if (lastActionSummary || observationCount) {
    rows.push({
      label: "last action",
      value: [lastActionSummary, observationCount ? `observations=${observationCount}` : ""]
        .filter(Boolean)
        .join(" · "),
    });
  }
  const traceGapCode = boundedTraceString(generation.traceGapCode);
  if (generation.runtimeFactTraceGap === true || traceGapCode) {
    rows.push({
      label: "trace gap",
      value: traceGapCode || "true",
    });
  }
  const evidenceLabels = traceStringArray(generation.selfStateEvidenceLabels);
  if (evidenceLabels.length > 0) {
    rows.push({
      label: "evidence",
      value: evidenceLabels.join(", "),
    });
  }
  return rows.slice(0, 8);
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
  const meaningText = boundedTraceText(
    trace.meaning_result?.text ??
      (typeof trace.meaning_result === "string" ? trace.meaning_result : "")
  );
  const strategyText = boundedTraceText(
    trace.strategy_result?.text ??
      (typeof trace.strategy_result === "string" ? trace.strategy_result : "")
  );
  const generationText = boundedTraceText(
    trace.generation_result?.text ??
      (typeof trace.generation_result === "string" ? trace.generation_result : "")
  );
  const outputText = boundedTraceText(trace.output);
  const alignedValues = traceStringArray(trace.meaning_result?.aligned_values);
  const alignedGoals = traceStringArray(trace.strategy_result?.aligned_goals);
  const planSteps = traceStringArray(trace.strategy_result?.plan_steps);
  const stableSteps = traceStringArray(trace.stable_steps);
  const needsTools = trace.strategy_result?.needs_tools;
  const toolPlan = traceStringArray(trace.tool_plan ?? trace.strategy_result?.suggested_tools);
  const safetyCheckWarnings = traceStringArray(trace.safety_check_result?.warnings);
  const errorLabels = traceStringArray(trace.errors);
  const runtimeRouteEvidenceRows = runtimeRouteRows(trace.generation_result);
  const runtimeToolEvidenceRows = runtimeToolRows(trace.generation_result);
  const runtimeSelfStateEvidenceRows = runtimeSelfStateRows(trace.generation_result);
  const sourceChip = boundedTraceString(trace.generation_result?.uiPrimarySourceChip);
  const uiStatus = boundedTraceString(trace.generation_result?.uiStatus);
  const hasHiddenInput = typeof trace.input === "string" && trace.input.trim().length > 0;
  const hasContent =
    hasHiddenInput ||
    meaningText ||
    strategyText ||
    generationText ||
    runtimeRouteEvidenceRows.length > 0 ||
    runtimeToolEvidenceRows.length > 0 ||
    runtimeSelfStateEvidenceRows.length > 0 ||
    outputText ||
    toolPlan.length > 0 ||
    safetyCheckWarnings.length > 0 ||
    errorLabels.length > 0;

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
        {errorLabels.length > 0 && (
          <span className="rounded-full bg-red-50 px-2.5 py-1 text-[10px] font-medium text-red-700 border border-red-100">
            有 {errorLabels.length} 条错误
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
              {hasHiddenInput && (
                <div className="rounded-lg border border-gray-200 bg-white/70 p-3">
                  <div className="flex items-center gap-2 font-medium text-gray-700">
                    <Terminal size={14} />
                    输入已隐藏
                  </div>
                  <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed text-gray-800">
                    原始输入不在 trace 中展示；请以结构化状态和证据字段为准。
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
              {runtimeToolEvidenceRows.length > 0 && (
                <div className="rounded-lg border border-sky-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-sky-800">工具可用性证据</div>
                  <dl className="mt-2 space-y-1.5">
                    {runtimeToolEvidenceRows.map(row => (
                      <div
                        key={`${row.label}-${row.value}`}
                        className="grid gap-1 text-xs text-gray-700 sm:grid-cols-[150px_1fr]"
                      >
                        <dt className="font-medium text-sky-700">{row.label}</dt>
                        <dd className="break-words">{row.value}</dd>
                      </div>
                    ))}
                  </dl>
                </div>
              )}
              {runtimeSelfStateEvidenceRows.length > 0 && (
                <div className="rounded-lg border border-teal-100 bg-white/70 p-3">
                  <div className="text-[11px] font-medium text-teal-800">任务状态证据</div>
                  <dl className="mt-2 space-y-1.5">
                    {runtimeSelfStateEvidenceRows.map(row => (
                      <div
                        key={`${row.label}-${row.value}`}
                        className="grid gap-1 text-xs text-gray-700 sm:grid-cols-[150px_1fr]"
                      >
                        <dt className="font-medium text-teal-700">{row.label}</dt>
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
              {outputText && (
                <div className="rounded-lg border border-emerald-200 bg-emerald-50/60 p-3">
                  <div className="flex items-center gap-2 font-medium text-emerald-800">
                    <Terminal size={14} />
                    最终输出
                  </div>
                  <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed text-emerald-900">
                    {outputText}
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
              {errorLabels.length > 0 && (
                <div className="rounded-lg border border-red-200 bg-red-50/70 p-3">
                  <div className="flex items-center gap-2 font-semibold text-red-700">
                    <AlertCircle size={14} />
                    错误
                  </div>
                  <ul className="mt-2 list-disc pl-4 text-red-700">
                    {errorLabels.map((e, i) => (
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
