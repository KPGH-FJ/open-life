import { AlertTriangle, CheckCircle, XCircle, Info, RefreshCw, HelpCircle } from "lucide-react";
import type { AgentRunEvent } from "@/types";
import { getTypedEventExplanation } from "../utils/typedContract";

interface Props {
  event: AgentRunEvent;
}

function toneIcon(tone: string, size: number = 12) {
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

function toneBgClass(tone: string): string {
  switch (tone) {
    case "error":
      return "border-red-600/30 bg-red-950/20";
    case "warning":
      return "border-amber-600/30 bg-amber-950/20";
    case "success":
      return "border-green-600/30 bg-green-950/20";
    default:
      return "border-slate-600/30 bg-slate-900/30";
  }
}

export default function EventExplanationBlock({ event }: Props) {
  const explanation = getTypedEventExplanation(event);

  return (
    <div
      data-testid={`event-explanation-${event.id}`}
      className={`rounded border px-2.5 py-2 space-y-2 ${toneBgClass(explanation.tone)}`}
    >
      {/* Title row */}
      <div className="flex items-center gap-2">
        {toneIcon(explanation.tone)}
        <span className="text-xs font-medium text-slate-200">{explanation.title}</span>
      </div>

      {/* What happened (user-facing primary info) */}
      <div className="text-[11px] text-slate-300 leading-relaxed">{explanation.whatHappened}</div>

      {/* Why (typed reason only) */}
      {explanation.why && (
        <div className="text-[10px] text-slate-400 flex items-start gap-1.5">
          <HelpCircle size={10} className="shrink-0 mt-0.5 text-slate-500" />
          <span>{explanation.why}</span>
        </div>
      )}

      {/* Impact */}
      {explanation.impact && (
        <div className="text-[10px] text-slate-400 flex items-start gap-1.5">
          <AlertTriangle size={10} className="shrink-0 mt-0.5 text-amber-500" />
          <span>{explanation.impact}</span>
        </div>
      )}

      {/* Next step (actionable) */}
      {explanation.nextStep && (
        <div className="text-[10px] text-blue-300 flex items-start gap-1.5">
          <RefreshCw size={10} className="shrink-0 mt-0.5 text-blue-400" />
          <span>{explanation.nextStep}</span>
        </div>
      )}

      {/* Debug facts (developer-oriented, collapsible) */}
      {explanation.debugFacts.length > 0 && (
        <details className="text-[10px] text-slate-500 cursor-pointer group">
          <summary className="hover:text-slate-400 transition-colors select-none">
            开发者详情 ({explanation.debugFacts.length})
          </summary>
          <div className="mt-1.5 space-y-0.5 ml-2">
            {explanation.debugFacts.map((fact, idx) => (
              <div key={`${fact.label}-${idx}`} className="flex gap-2">
                <span className="text-slate-600 shrink-0">{fact.label}:</span>
                <span className="text-slate-400 break-all">{fact.value}</span>
              </div>
            ))}
          </div>
        </details>
      )}
    </div>
  );
}
