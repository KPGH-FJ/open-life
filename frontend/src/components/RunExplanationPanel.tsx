import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  Info,
  Ban,
  RefreshCw,
  Shield,
  Terminal,
} from "lucide-react";
import type { AgentRunEvent } from "@/types";
import type { TypedRunExplanationViewModel } from "../utils/typedContract";
import { getTypedRunExplanation } from "../utils/typedContract";

interface Props {
  events: AgentRunEvent[];
  run?: { status: string; kind: string; outputPreview?: string };
}

function toneIcon(tone: string, size: number = 16) {
  const cls = "shrink-0";
  switch (tone) {
    case "error":
      return <XCircle size={size} className={`${cls} text-red-400`} />;
    case "warning":
      return <AlertTriangle size={size} className={`${cls} text-amber-400`} />;
    case "success":
      return <CheckCircle size={size} className={`${cls} text-green-400`} />;
    case "info":
    default:
      return <Info size={size} className={`${cls} text-slate-400`} />;
  }
}

function toneBorderClass(tone: string): string {
  switch (tone) {
    case "error":
      return "border-red-500/40";
    case "warning":
      return "border-amber-500/40";
    case "success":
      return "border-green-500/40";
    default:
      return "border-slate-500/40";
  }
}

function toneBgClass(tone: string): string {
  switch (tone) {
    case "error":
      return "bg-red-950/20";
    case "warning":
      return "bg-amber-950/20";
    case "success":
      return "bg-green-950/20";
    default:
      return "bg-slate-900/30";
  }
}

function nextActionIcon(kind: string, size: number = 12) {
  const cls = "shrink-0";
  switch (kind) {
    case "review_proposal":
    case "grant_permission":
      return <Shield size={size} className={`${cls} text-blue-400`} />;
    case "adjust_agent_spec":
      return <Ban size={size} className={`${cls} text-purple-400`} />;
    case "retry_replay":
      return <RefreshCw size={size} className={`${cls} text-amber-400`} />;
    case "inspect_trace":
      return <Terminal size={size} className={`${cls} text-slate-400`} />;
    default:
      return <Info size={size} className={`${cls} text-slate-400`} />;
  }
}

function nextActionSeverityClass(severity: string): string {
  switch (severity) {
    case "error":
      return "border-red-600/30 bg-red-950/20 text-red-300";
    case "warning":
      return "border-amber-600/30 bg-amber-950/20 text-amber-300";
    default:
      return "border-slate-600/30 bg-slate-900/20 text-slate-300";
  }
}

function RunExplanationPanelInternal({
  explanation,
}: {
  explanation: TypedRunExplanationViewModel;
}) {
  return (
    <div
      data-testid="run-explanation-panel"
      className={`rounded-xl border ${toneBorderClass(explanation.outcomeTone)} ${toneBgClass(explanation.outcomeTone)} p-4`}
    >
      {/* Headline */}
      <div className="flex items-center gap-2.5 mb-3">
        {toneIcon(explanation.outcomeTone)}
        <h3 className="text-sm font-semibold text-slate-200">{explanation.headline}</h3>
      </div>

      {/* Primary reason */}
      {explanation.primaryReason && (
        <div className="mb-3 text-xs flex items-center gap-2">
          <AlertTriangle size={12} className="text-amber-400 shrink-0" />
          <span className="text-amber-300">{explanation.primaryReason}</span>
        </div>
      )}

      {/* User-facing bullets */}
      <div className="mb-3">
        <div className="text-[10px] text-slate-500 font-medium mb-1.5">运行摘要</div>
        <ul className="space-y-1 list-disc list-inside">
          {explanation.userFacingBullets.map((bullet, idx) => (
            <li key={idx} className="text-[11px] text-slate-300 pl-1">
              {bullet}
            </li>
          ))}
        </ul>
      </div>

      {/* Next actions — only show when error/warning and actions exist */}
      {explanation.nextActions.length > 0 && (
        <div className="mb-3">
          <div className="text-[10px] text-slate-500 font-medium mb-1.5">建议操作</div>
          <div className="space-y-1.5">
            {explanation.nextActions.map((action, idx) => (
              <div
                key={idx}
                className={`flex items-center gap-2 text-[11px] rounded px-2 py-1.5 border ${nextActionSeverityClass(action.severity)}`}
              >
                {nextActionIcon(action.kind)}
                <span>{action.label}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Developer bullets (collapsible) */}
      <details className="text-[10px] text-slate-500 cursor-pointer group">
        <summary className="hover:text-slate-400 transition-colors select-none">开发者信息</summary>
        <div className="mt-2 space-y-1 ml-2">
          {explanation.developerBullets.map((bullet, idx) => (
            <div key={idx} className="text-slate-400">
              {bullet}
            </div>
          ))}
          {explanation.toolSummary && (
            <div className="text-slate-500">
              工具统计: {explanation.toolSummary.started}S / {explanation.toolSummary.completed}C /{" "}
              {explanation.toolSummary.blocked}B / {explanation.toolSummary.failed}F /{" "}
              {explanation.toolSummary.needsConfirmation}NC
            </div>
          )}
          {explanation.replaySummary && explanation.replaySummary.started > 0 && (
            <div className="text-slate-500">
              重放统计: {explanation.replaySummary.started}S / {explanation.replaySummary.completed}
              C / {explanation.replaySummary.failed}F / {explanation.replaySummary.blocked}B /{" "}
              {explanation.replaySummary.needsConfirmation}NC
            </div>
          )}
        </div>
      </details>
    </div>
  );
}

export default function RunExplanationPanel({ events, run }: Props) {
  const explanation = getTypedRunExplanation(events, run);
  return <RunExplanationPanelInternal explanation={explanation} />;
}

// Export for testing
export { RunExplanationPanelInternal };
