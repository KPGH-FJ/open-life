import { Clock, Play, AlertTriangle, CheckCircle, XCircle, RefreshCw, FileText } from "lucide-react";
import type { AgentRunEvent, AgentRunEventType } from "@/types";

interface Props {
  events: AgentRunEvent[];
  runId: string;
  show: boolean;
  onToggle: () => void;
}

const EVENT_ICONS: Record<AgentRunEventType, React.ReactNode> = {
  "run.created": <Play size={14} className="text-blue-400" />,
  "context.assembled": <FileText size={14} className="text-slate-400" />,
  "model.route_selected": <FileText size={14} className="text-purple-400" />,
  "model.call_started": <Play size={14} className="text-purple-400" />,
  "model.call_completed": <CheckCircle size={14} className="text-green-400" />,
  "model.call_failed": <XCircle size={14} className="text-red-400" />,
  "tool.call_started": <Play size={14} className="text-emerald-400" />,
  "tool.call_blocked": <AlertTriangle size={14} className="text-amber-400" />,
  "tool.call_completed": <CheckCircle size={14} className="text-emerald-400" />,
  "tool.call_failed": <XCircle size={14} className="text-red-400" />,
  "observation.created": <FileText size={14} className="text-slate-400" />,
  "proposal.created": <FileText size={14} className="text-yellow-400" />,
  "fallback.started": <RefreshCw size={14} className="text-amber-400" />,
  "fallback.completed": <CheckCircle size={14} className="text-amber-400" />,
  "json_repair.started": <RefreshCw size={14} className="text-amber-400" />,
  "json_repair.completed": <CheckCircle size={14} className="text-amber-400" />,
  "plan.created": <FileText size={14} className="text-cyan-400" />,
  "plan.confirmation_requested": <AlertTriangle size={14} className="text-yellow-400" />,
  "plan.confirmation_resolved": <CheckCircle size={14} className="text-green-400" />,
  "run.completed": <CheckCircle size={14} className="text-green-400" />,
  "run.failed": <XCircle size={14} className="text-red-400" />,
  "unknown": <FileText size={14} className="text-slate-500" />,
};

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  } catch {
    return iso;
  }
}

function actorLabel(actor: AgentRunEvent["actor"]): string {
  if (typeof actor === "string") return actor;
  if ("sub_agent" in actor) return `sub:${actor.sub_agent}`;
  if ("tool" in actor) return `tool:${actor.tool}`;
  return "unknown";
}

export default function RunTracePanel({ events, runId, show, onToggle }: Props) {
  if (!events.length) return null;

  return (
    <div className="border border-slate-700/50 rounded-lg bg-slate-900/50 overflow-hidden">
      <button
        onClick={onToggle}
        className="w-full flex items-center justify-between px-3 py-2 text-xs text-slate-400 hover:bg-slate-800/50 transition-colors"
      >
        <span className="flex items-center gap-2">
          <Clock size={12} />
          <span>Run Trace ({events.length} events)</span>
        </span>
        <span className="text-[10px] text-slate-500">{runId.slice(0, 8)}…</span>
      </button>
      {show && (
        <div className="max-h-64 overflow-y-auto border-t border-slate-700/50">
          {events.map((evt) => (
            <div
              key={evt.id}
              className="flex items-start gap-2 px-3 py-1.5 text-[11px] hover:bg-slate-800/30 transition-colors border-b border-slate-800/30 last:border-b-0"
            >
              <span className="mt-0.5 shrink-0">{EVENT_ICONS[evt.eventType] ?? <FileText size={14} className="text-slate-500" />}</span>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-slate-300 truncate">{evt.summary}</span>
                </div>
                <div className="flex items-center gap-2 text-[10px] text-slate-500 mt-0.5">
                  <span>{evt.eventType}</span>
                  <span>by {actorLabel(evt.actor)}</span>
                  {evt.phase && <span className="text-purple-400">{evt.phase}</span>}
                </div>
              </div>
              <span className="text-[10px] text-slate-600 shrink-0 mt-0.5">
                {formatTimestamp(evt.createdAt)}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
