import { useState } from "react";
import {
  Clock,
  Play,
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  FileText,
  Layers,
  Terminal,
  ChevronDown,
  ChevronUp,
  Shield,
  EyeOff,
  Ban,
  Key,
} from "lucide-react";
import type { AgentRunEvent, AgentRunEventType, RedactionSummary } from "@/types";
import { TypedBadge, getTypedEventDetailViewModel } from "../utils/typedContract";
import EventExplanationBlock from "./EventExplanationBlock";

interface Props {
  events: AgentRunEvent[];
  runId: string;
  show: boolean;
  onToggle: () => void;
}

const SENSITIVE_KEYS = new Set([
  "email",
  "phone",
  "token",
  "secret",
  "api_key",
  "apikey",
  "authorization",
  "password",
  "life_model",
  "prompt",
  "raw_output",
  "content",
]);

function isSensitiveKey(key: string): boolean {
  const lower = key.toLowerCase();
  if (SENSITIVE_KEYS.has(lower)) return true;
  return (
    lower.includes("email") ||
    lower.includes("phone") ||
    lower.includes("token") ||
    lower.includes("secret") ||
    lower.includes("api_key") ||
    lower.includes("password")
  );
}

function fieldIsRemoved(path: string, fieldsRemoved: string[]): boolean {
  for (const removed of fieldsRemoved) {
    if (removed === path) return true;
    if (removed.startsWith(path + ".")) return true;
    if (path.startsWith(removed + ".")) return true;
  }
  return false;
}

function safeScalarPreview(value: unknown, maxLen: number): string {
  if (value === undefined || value === null) return "[空]";
  const s = String(value);
  return s.length > maxLen ? s.slice(0, maxLen) + "…" : s;
}

function safePayloadPreview(
  value: unknown,
  redaction: RedactionSummary | undefined,
  path: string,
  budget: number
): { text: string; truncated: boolean } {
  const isRedacted = redaction?.redacted === true;
  const fieldsRemoved = redaction?.fieldsRemoved ?? [];
  const HIDDEN = "[已隐藏]";
  const MAX_BUDGET = 1600;

  if (budget <= 0) return { text: "…", truncated: true };

  if (value === undefined || value === null) {
    return { text: "[空]", truncated: false };
  }

  // Primitives
  if (typeof value !== "object") {
    const s = String(value);
    if (s.length <= budget) return { text: s, truncated: false };
    return { text: s.slice(0, budget) + "…", truncated: true };
  }

  // Array
  if (Array.isArray(value)) {
    const maxItems = 10;
    let result = "[";
    let used = 1;
    let truncated = false;
    for (let i = 0; i < Math.min(value.length, maxItems); i++) {
      if (i > 0) {
        result += ", ";
        used += 2;
      }
      const itemPath = `${path}[${i}]`;
      const itemIsRemoved = isRedacted && fieldIsRemoved(itemPath, fieldsRemoved);
      if (itemIsRemoved) {
        result += HIDDEN;
        used += HIDDEN.length;
      } else {
        const child = safePayloadPreview(
          value[i],
          redaction,
          itemPath,
          Math.max(budget - used, 20)
        );
        result += child.text;
        used += child.text.length;
        if (child.truncated) truncated = true;
      }
      if (used >= budget) {
        truncated = true;
        break;
      }
    }
    result += value.length > maxItems ? ", …" : "";
    result += "]";
    return { text: result, truncated: truncated || value.length > maxItems };
  }

  // Object
  if (typeof value === "object" && value !== null) {
    const entries = Object.entries(value as Record<string, unknown>);
    const maxKeys = 15;
    let result = "{";
    let used = 1;
    let truncated = false;

    for (let i = 0; i < Math.min(entries.length, maxKeys); i++) {
      const [key, val] = entries[i];
      const childPath = path ? `${path}.${key}` : key;

      // Determine if this key+subtree should be hidden
      let hideEntireEntry = false;
      if (isRedacted) {
        hideEntireEntry = fieldIsRemoved(childPath, fieldsRemoved) || isSensitiveKey(key);
      }

      if (i > 0) {
        result += ", ";
        used += 2;
      }

      if (hideEntireEntry) {
        const entry = `${key}: ${HIDDEN}`;
        result += entry;
        used += entry.length;
      } else if (typeof val === "object" && val !== null) {
        result += `${key}: `;
        used += key.length + 2;
        const child = safePayloadPreview(val, redaction, childPath, Math.max(budget - used, 30));
        result += child.text;
        used += child.text.length;
        if (child.truncated) truncated = true;
      } else {
        const preview = safeScalarPreview(val, Math.min(80, Math.max(budget - used, 10)));
        const entry = `${key}: ${preview}`;
        result += entry;
        used += entry.length;
        if (preview.endsWith("…")) truncated = true;
      }

      if (used >= MAX_BUDGET) {
        truncated = true;
        break;
      }
    }

    result += entries.length > maxKeys ? ", …" : "";
    result += "}";
    return { text: result, truncated: truncated || entries.length > maxKeys };
  }

  return { text: "(unknown)", truncated: false };
}

const EVENT_ICONS: Record<AgentRunEventType, React.ReactNode> = {
  "run.created": <Play size={14} className="text-blue-400" />,
  "context.assembled": <FileText size={14} className="text-slate-400" />,
  "agent_spec.selected": <FileText size={14} className="text-indigo-400" />,
  "prompt_stack.assembled": <FileText size={14} className="text-teal-400" />,
  "context_governance.applied": <FileText size={14} className="text-teal-400" />,
  "model.route_selected": <FileText size={14} className="text-purple-400" />,
  "model.call_started": <Play size={14} className="text-purple-400" />,
  "model.call_completed": <CheckCircle size={14} className="text-green-400" />,
  "model.call_failed": <XCircle size={14} className="text-red-400" />,
  "model.failed": <XCircle size={14} className="text-red-400" />,
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
  "plan.execution_started": <Play size={14} className="text-cyan-400" />,
  "plan.step_started": <Play size={14} className="text-blue-400" />,
  "plan.step_completed": <CheckCircle size={14} className="text-green-400" />,
  "plan.step_failed": <XCircle size={14} className="text-red-400" />,
  "plan.deviation_recorded": <AlertTriangle size={14} className="text-amber-400" />,
  "plan.execution_completed": <CheckCircle size={14} className="text-green-400" />,
  "plan.execution_failed": <XCircle size={14} className="text-red-400" />,
  "plan.cancel_requested": <AlertTriangle size={14} className="text-amber-400" />,
  "plan.cancelled": <XCircle size={14} className="text-gray-400" />,
  "plan.retry_requested": <RefreshCw size={14} className="text-amber-400" />,
  "plan.retry_started": <RefreshCw size={14} className="text-blue-400" />,
  "plan.continuation_requested": <Play size={14} className="text-emerald-400" />,
  "plan.action_replayed": <RefreshCw size={14} className="text-green-400" />,
  "plan.action_replay_requested": <Play size={14} className="text-amber-400" />,
  "replay.started": <RefreshCw size={14} className="text-blue-400" />,
  "replay.completed": <CheckCircle size={14} className="text-green-400" />,
  "replay.failed": <XCircle size={14} className="text-red-400" />,
  "compaction.created": <Layers size={14} className="text-orange-400" />,
  "shell.blocked": <Terminal size={14} className="text-red-400" />,
  "shell.completed": <Terminal size={14} className="text-green-400" />,
  "run.completed": <CheckCircle size={14} className="text-green-400" />,
  "run.failed": <XCircle size={14} className="text-red-400" />,
  unknown: <FileText size={14} className="text-slate-500" />,
};

function eventBorderClass(eventType: AgentRunEventType): string {
  if (eventType.startsWith("model.")) return "border-l-purple-400";
  if (eventType.startsWith("tool.") || eventType.startsWith("shell."))
    return "border-l-emerald-400";
  if (eventType.startsWith("plan.")) return "border-l-cyan-400";
  if (eventType.startsWith("replay.")) return "border-l-blue-400";
  if (eventType.startsWith("compaction.")) return "border-l-orange-400";
  if (eventType.startsWith("fallback.") || eventType.startsWith("json_repair."))
    return "border-l-amber-400";
  if (eventType.startsWith("proposal.")) return "border-l-yellow-400";
  if (eventType.startsWith("run.")) return "border-l-blue-400";
  if (
    eventType.startsWith("agent_spec.") ||
    eventType.startsWith("prompt_stack.") ||
    eventType.startsWith("context")
  )
    return "border-l-slate-400";
  return "border-l-slate-400";
}

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

function hasTruncatedMarker(payload: Record<string, unknown>): boolean {
  return payload?.truncated === true || payload?.output_truncated === true;
}

function badgeColorClass(kind: string, rawReason: string): string {
  if (kind === "proposal_reason") {
    return "bg-blue-900/40 text-blue-300 border border-blue-700/40";
  }
  if (kind === "failure_kind") {
    return "bg-red-900/40 text-red-300 border border-red-700/40";
  }
  // block_reason – per-reason visual distinction
  switch (rawReason) {
    case "agent_spec_denied":
    case "agent_spec_missing":
      return "bg-purple-900/40 text-purple-300 border border-purple-700/40";
    case "tool_permission_denied":
      return "bg-amber-900/40 text-amber-300 border border-amber-700/40";
    case "network_policy_denied":
    case "domain_blocked":
      return "bg-red-900/40 text-red-300 border border-red-700/40";
    case "sandbox_denied":
      return "bg-orange-900/40 text-orange-300 border border-orange-700/40";
    case "missing_mcp_client":
      return "bg-red-900/40 text-red-300 border border-red-700/40";
    case "disabled_manifest":
    case "declarative_only":
      return "bg-slate-700/40 text-slate-300 border border-slate-600/40";
    case "replay_spec_missing":
      return "bg-rose-900/40 text-rose-300 border border-rose-700/40";
    case "path_not_safe":
    case "pii_detected":
      return "bg-red-900/40 text-red-300 border border-red-700/40";
    default:
      return "bg-slate-700/40 text-slate-300 border border-slate-600/40";
  }
}

function typedBadgeSpan(badge: TypedBadge | null): React.ReactNode {
  if (!badge) return null;
  return (
    <span
      className={`text-[10px] px-1.5 py-0.5 rounded font-medium ${badgeColorClass(badge.kind, badge.rawReason)}`}
    >
      {badge.label}
    </span>
  );
}

function toneColor(tone: string): string {
  switch (tone) {
    case "error":
      return "text-red-400";
    case "warning":
      return "text-amber-400";
    case "success":
      return "text-green-400";
    default:
      return "text-slate-400";
  }
}

function detailIcon(kind: string, titleIconTone: string): React.ReactNode {
  const cls = toneColor(titleIconTone);
  switch (kind) {
    case "tool_call_blocked":
      return <Ban size={10} className={cls} />;
    case "replay_started":
    case "replay_completed":
      return <CheckCircle size={10} className={cls} />;
    case "replay_failed":
      return <XCircle size={10} className={cls} />;
    default:
      return null;
  }
}

function TypedEventDetailBlock({ vm }: { vm: ReturnType<typeof getTypedEventDetailViewModel> }) {
  if (vm.kind === "unknown") return null;

  return (
    <div className="rounded bg-slate-800/50 px-2 py-2 space-y-1.5">
      {/* Title bar */}
      <div className="text-slate-400 text-[10px] font-medium flex items-center gap-1.5">
        {detailIcon(vm.kind, vm.titleIconTone)}
        {vm.title}
      </div>

      {/* Meta rows: kind-specific fields */}
      {(vm.statusLabel || vm.toolName) && (
        <div className="flex flex-wrap gap-2 text-[10px]">
          {vm.statusLabel && (
            <span className="text-slate-400">
              {vm.kind === "replay_started"
                ? "状态"
                : vm.kind === "replay_completed"
                  ? "结果"
                  : "状态"}
              : <span className={toneColor(vm.statusTone ?? "info")}>{vm.statusLabel}</span>
            </span>
          )}
          {vm.toolName && (
            <span className="text-slate-400">
              工具: <span className="text-slate-200">{vm.toolName}</span>
            </span>
          )}
          {vm.source && (
            <span className="text-slate-400">
              来源: <span className="text-slate-200">{vm.source}</span>
            </span>
          )}
        </div>
      )}

      {/* MCP wrapper/target info */}
      {vm.targetToolName && (
        <div className="flex flex-wrap gap-2 text-[10px]">
          <span className="text-amber-400">
            MCP 包装: <span className="text-amber-200">{vm.wrapperToolName}</span>
          </span>
          <span className="text-amber-400">
            目标工具: <span className="text-amber-200">{vm.targetToolName}</span>
          </span>
          <span className="text-amber-400">
            目标源: <span className="text-amber-200">{vm.targetSource}</span>
          </span>
        </div>
      )}

      {/* Replay-specific meta: replay_of_action_id, agentSpec */}
      {vm.kind.startsWith("replay") && (vm.replayOfActionId || vm.agentSpecId) && (
        <div className="flex flex-wrap gap-2 text-[10px]">
          {vm.replayOfActionId && (
            <span className="text-slate-400">
              重放动作: <span className="text-slate-200">{vm.replayOfActionId.slice(0, 12)}…</span>
            </span>
          )}
          {vm.agentSpecId && (
            <span className="text-slate-400">
              AgentSpec: <span className="text-slate-300">{vm.agentSpecId}</span>
            </span>
          )}
        </div>
      )}

      {/* Badges */}
      {vm.badges.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {vm.badges.map((badge, i) => (
            <span key={`badge-${badge.kind}-${badge.rawReason}-${i}`}>{typedBadgeSpan(badge)}</span>
          ))}
        </div>
      )}

      {/* Block/tool_call_blocked specific: agentSpec + proposal below badges */}
      {vm.kind === "tool_call_blocked" && vm.agentSpecId && (
        <div className="text-[10px] text-slate-500">
          AgentSpec: <span className="text-slate-300">{vm.agentSpecId}</span>
        </div>
      )}
      {vm.kind === "tool_call_blocked" && vm.proposalId && (
        <div className="text-[10px] text-blue-400 flex items-center gap-1">
          <Key size={10} />
          Proposal: <span className="text-blue-300">{vm.proposalId}</span>
        </div>
      )}

      {/* humanMessage as auxiliary */}
      {vm.humanMessage && (
        <div className="text-[10px] text-slate-500 italic mt-1">
          {vm.kind === "tool_call_blocked" ? "辅助说明: " : "说明: "}
          {vm.humanMessage}
        </div>
      )}
    </div>
  );
}

export default function RunTracePanel({ events, runId, show, onToggle }: Props) {
  const [expandedId, setExpandedId] = useState<string | null>(null);

  if (!events.length) return null;

  return (
    <div className="border border-slate-700/50 rounded-lg bg-slate-900/50 overflow-hidden">
      <button
        onClick={onToggle}
        aria-label="Toggle run trace"
        data-testid="run-trace-toggle"
        className="w-full flex items-center justify-between px-3 py-1.5 text-xs text-slate-400 hover:bg-slate-800/50 transition-colors"
      >
        <span className="flex items-center gap-2">
          <Clock size={12} />
          <span>Run Trace ({events.length} events)</span>
        </span>
        <span className="text-[10px] text-slate-500">{runId.slice(0, 8)}…</span>
      </button>
      {show && (
        <div className="max-h-96 overflow-y-auto border-t border-slate-700/50">
          {events.map(evt => (
            <div key={evt.id} className={`border-l-2 ${eventBorderClass(evt.eventType)}`}>
              <button
                onClick={() => setExpandedId(expandedId === evt.id ? null : evt.id)}
                data-testid={`event-row-${evt.id}`}
                className="w-full flex items-start gap-2 px-3 py-1.5 text-[11px] hover:bg-slate-800/30 transition-colors border-b border-slate-800/30 last:border-b-0 text-left"
              >
                <span className="mt-0.5 shrink-0">
                  {EVENT_ICONS[evt.eventType] ?? <FileText size={14} className="text-slate-500" />}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-slate-300 truncate">{evt.summary}</span>
                    {evt.redaction?.redacted && (
                      <span className="text-[10px] text-amber-500 flex items-center gap-0.5">
                        <EyeOff size={10} /> 脱敏
                      </span>
                    )}
                    {hasTruncatedMarker(evt.payload) && (
                      <span className="text-[10px] text-slate-500 flex items-center gap-0.5">
                        已截断
                      </span>
                    )}
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
                <span className="text-slate-500 mt-0.5 shrink-0">
                  {expandedId === evt.id ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                </span>
              </button>

              {expandedId === evt.id && (
                <div className="px-3 py-2 bg-slate-950/50 border-b border-slate-800/30 space-y-2 text-[11px]">
                  {/* Event-level explanation (user-facing first, from typed contract) */}
                  <EventExplanationBlock event={evt} />

                  {/* Typed contract event detail (driven by view model) */}
                  <TypedEventDetailBlock vm={getTypedEventDetailViewModel(evt)} />

                  {Object.keys(evt.payload).length > 0 && (
                    <div className="rounded bg-slate-800/50 px-2 py-2">
                      <div className="text-slate-500 text-[10px] font-medium mb-1 flex items-center gap-1">
                        <FileText size={10} />
                        事件载荷
                        {evt.redaction?.redacted && (
                          <span className="text-amber-500 flex items-center gap-0.5">
                            <EyeOff size={10} /> 已脱敏
                          </span>
                        )}
                      </div>
                      <pre className="text-slate-400 font-mono whitespace-pre-wrap break-all">
                        {safePayloadPreview(evt.payload, evt.redaction, "", 1600).text}
                      </pre>
                      {safePayloadPreview(evt.payload, evt.redaction, "", 1600).truncated && (
                        <div className="text-slate-600 mt-1 text-[10px]">
                          …输出已截断（预算限制）
                        </div>
                      )}
                    </div>
                  )}

                  {evt.redaction?.redacted && (
                    <div className="rounded bg-amber-950/30 border border-amber-800/30 px-2 py-1.5 text-amber-400 text-[10px] flex items-center gap-1.5">
                      <Shield size={12} />
                      <span>
                        敏感字段已脱敏/隐藏 — 载荷中标记为 [已隐藏] 的值为敏感信息，不在 UI
                        中展示原值。
                      </span>
                    </div>
                  )}

                  {hasTruncatedMarker(evt.payload) && (
                    <div className="rounded bg-amber-950/30 border border-amber-800/30 px-2 py-1.5 text-amber-400 flex items-center gap-1.5">
                      <AlertTriangle size={12} />
                      <span>输出已截断。完整输出大小超出显示限制。</span>
                    </div>
                  )}

                  {evt.redaction && evt.redaction.redacted && (
                    <div className="rounded bg-amber-950/30 border border-amber-800/30 px-2 py-2 space-y-1">
                      <div className="text-amber-400 font-medium flex items-center gap-1">
                        <Shield size={12} /> 脱敏信息
                      </div>
                      <div className="text-amber-300/70">原因: {evt.redaction.reason}</div>
                      {evt.redaction.fieldsRemoved.length > 0 && (
                        <div className="text-amber-300/70">
                          移除字段: {evt.redaction.fieldsRemoved.join(", ")}
                        </div>
                      )}
                    </div>
                  )}

                  {(evt.eventType === "shell.blocked" || evt.eventType === "shell.completed") && (
                    <div className="rounded bg-slate-800/50 px-2 py-1.5 text-slate-400 flex items-center gap-1.5">
                      <Terminal size={12} />
                      <span>Shell 事件 — 受治理的执行环境。不提供交互式终端。</span>
                    </div>
                  )}

                  <div className="text-slate-600 text-[10px] flex items-center gap-2">
                    <span>Event ID: {evt.id}</span>
                    {evt.parentEventId && <span>· Parent: {evt.parentEventId.slice(0, 8)}</span>}
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
