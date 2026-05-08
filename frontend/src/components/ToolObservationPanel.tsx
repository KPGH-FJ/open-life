import { useState } from "react";
import { Link } from "react-router-dom";
import {
  Wrench,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  Shield,
  Eye,
  Info,
  EyeOff,
} from "lucide-react";
import type { AgentRun } from "../tauri";

interface Props {
  run: AgentRun;
}

function riskBadge(level?: string) {
  const label = level === "high" ? "高" : level === "medium" ? "中" : level === "low" ? "低" : "?";
  const cls =
    level === "high"
      ? "bg-red-50 text-red-600 border-red-100"
      : level === "medium"
        ? "bg-amber-50 text-amber-600 border-amber-100"
        : "bg-stone-50 text-stone-500 border-stone-100";
  return (
    <span
      className={`inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded border ${cls}`}
    >
      <Shield size={10} /> {label}风险
    </span>
  );
}

function statusBadge(status: string, permissionDecision?: string) {
  if (status === "needs_confirmation" || permissionDecision === "ask_every_time") {
    return (
      <span className="inline-flex items-center gap-1 text-orange-600 text-[10px] bg-orange-50 px-1.5 py-0.5 rounded border border-orange-100">
        <AlertTriangle size={10} /> 待授权
      </span>
    );
  }
  if (status === "blocked" || permissionDecision === "deny") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-[10px] bg-red-50 px-1.5 py-0.5 rounded border border-red-100">
        <Shield size={10} /> 已阻断
      </span>
    );
  }
  if (status === "succeeded" || status === "completed") {
    return (
      <span className="inline-flex items-center gap-1 text-green-600 text-[10px] bg-green-50 px-1.5 py-0.5 rounded border border-green-100">
        <CheckCircle2 size={10} /> 成功
      </span>
    );
  }
  if (status === "failed") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-[10px] bg-red-50 px-1.5 py-0.5 rounded border border-red-100">
        <XCircle size={10} /> 失败
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-stone-500 text-[10px] bg-stone-50 px-1.5 py-0.5 rounded border border-stone-100">
      {status}
    </span>
  );
}

function isDeclarativeOnly(toolScope: AgentRun["actions"][number]["toolScope"]): boolean {
  return toolScope?.capabilities?.includes("declarative_only") ?? false;
}

function boundedPreview(
  value: unknown,
  maxChars = 800
): { display: string; truncated: boolean; estimatedLength: number } {
  if (value === undefined || value === null) {
    return { display: "[空]", truncated: false, estimatedLength: 0 };
  }
  if (typeof value === "string") {
    const len = value.length;
    if (len <= maxChars) return { display: value, truncated: false, estimatedLength: len };
    return { display: value.slice(0, maxChars) + "…", truncated: true, estimatedLength: len };
  }
  if (typeof value === "number" || typeof value === "boolean") {
    const s = String(value);
    return { display: s, truncated: false, estimatedLength: s.length };
  }

  // Budget-based recursive walk for objects and arrays.
  // estimatedLength is accumulated during traversal, never via full JSON.stringify.
  let budget = maxChars;
  let truncated = false;
  let estimatedLength = 0;

  function walk(val: unknown, depth: number): string {
    if (budget <= 0) {
      truncated = true;
      return "…";
    }
    if (depth > 3) {
      estimatedLength += 6;
      return "[嵌套过深]";
    }

    if (val === undefined || val === null) {
      estimatedLength += 4;
      return "[空]";
    }
    if (typeof val === "string") {
      const len = val.length;
      estimatedLength += Math.min(len, budget);
      if (len + 2 <= budget) {
        budget -= len + 2;
        return JSON.stringify(val);
      }
      truncated = true;
      return JSON.stringify(val.slice(0, Math.max(budget - 4, 4))) + "…";
    }
    if (typeof val === "number" || typeof val === "boolean") {
      const s = String(val);
      estimatedLength += s.length;
      budget -= s.length;
      return s;
    }

    if (Array.isArray(val)) {
      const maxItems = Math.min(val.length, 20);
      let parts: string[] = [];
      estimatedLength += 2; // for []
      budget -= 2;
      for (let i = 0; i < maxItems && budget > 2; i++) {
        if (i > 0) {
          estimatedLength += 2;
          budget -= 2;
        }
        if (budget <= 2) break;
        const item = walk(val[i], depth + 1);
        parts.push(item);
        if (budget <= 0) {
          truncated = true;
          break;
        }
      }
      if (val.length > maxItems) {
        parts.push("…");
        truncated = true;
        estimatedLength += 1;
      }
      return `[${parts.join(", ")}]`;
    }

    if (typeof val === "object" && val !== null) {
      const entries = Object.entries(val as Record<string, unknown>);
      const maxKeys = Math.min(entries.length, 20);
      let parts: string[] = [];
      estimatedLength += 2; // for {}
      budget -= 2;
      for (let i = 0; i < maxKeys && budget > 2; i++) {
        const [k, v] = entries[i];
        const keyStr = JSON.stringify(k);
        if (i > 0) {
          estimatedLength += 2;
          budget -= 2;
        }
        estimatedLength += keyStr.length + 2;
        budget -= keyStr.length + 2;
        if (budget <= 0) {
          truncated = true;
          break;
        }
        const valStr = walk(v, depth + 1);
        parts.push(`${keyStr}: ${valStr}`);
        if (budget <= 0) {
          truncated = true;
          break;
        }
      }
      if (entries.length > maxKeys) {
        parts.push("…");
        truncated = true;
        estimatedLength += 1;
      }
      return `{${parts.join(", ")}}`;
    }

    const s = String(val);
    estimatedLength += s.length;
    return s;
  }

  const display = walk(value, 0);
  return { display, truncated, estimatedLength };
}

export default function ToolObservationPanel({ run }: Props) {
  const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set());
  const [showObservations, setShowObservations] = useState(false);

  const toggleExpand = (id: string) => {
    const next = new Set(expandedTools);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setExpandedTools(next);
  };

  const toolActions = run.actions.filter(
    a => a.actionType === "mcp_tool_call" || a.actionType === "tool_call" || a.toolScope
  );

  if (toolActions.length === 0 && run.observations.length === 0) {
    return (
      <div className="mb-6">
        <h3 className="text-sm font-semibold text-stone-700 mb-2 flex items-center gap-2">
          <Wrench size={14} /> 工具调用与观察
        </h3>
        <div className="text-xs text-stone-400 py-3">此运行中没有工具调用或观察记录。</div>
      </div>
    );
  }

  return (
    <div className="mb-6">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold text-stone-700 flex items-center gap-2">
          <Wrench size={14} /> 工具调用与观察
        </h3>
        <span className="text-xs text-stone-400">
          {toolActions.length} 工具 · {run.observations.length} 观察
        </span>
      </div>

      <div className="space-y-2">
        {toolActions.map(action => {
          const scope = action.toolScope;
          const isBlocked =
            action.status === "blocked" ||
            action.status === "needs_confirmation" ||
            action.permissionDecision === "deny" ||
            action.permissionDecision === "ask_every_time";
          const isFailed = action.status === "failed" || action.error;
          const isExpanded = expandedTools.has(action.id);

          return (
            <div
              key={action.id}
              className={`rounded-lg border p-3 text-sm ${
                isBlocked
                  ? "border-amber-200 bg-amber-50/50"
                  : isFailed
                    ? "border-red-200 bg-red-50/50"
                    : "border-stone-200 bg-white"
              }`}
            >
              {/* Header */}
              <button
                onClick={() => toggleExpand(action.id)}
                className="w-full flex items-center gap-3 text-left"
              >
                <span
                  className={`shrink-0 w-2 h-2 rounded-full ${
                    isBlocked
                      ? "bg-amber-400"
                      : isFailed
                        ? "bg-red-400"
                        : action.status === "succeeded" || action.status === "completed"
                          ? "bg-emerald-400"
                          : "bg-stone-300"
                  }`}
                />
                <span className="font-medium text-stone-800 truncate flex-1">
                  {scope?.toolName || action.actionType?.replace("_", " ") || "unknown"}
                </span>
                {statusBadge(action.status, action.permissionDecision)}
                {scope && riskBadge(scope.riskLevel)}
                {scope && isDeclarativeOnly(scope) && (
                  <span className="text-[10px] text-stone-400 bg-stone-100 px-1.5 py-0.5 rounded border border-stone-200">
                    声明-only
                  </span>
                )}
                <span className="text-[10px] text-stone-400">
                  {new Date(
                    action.startedAt || action.timestamp || run.startedAt
                  ).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}
                </span>
                {isExpanded ? (
                  <ChevronUp size={14} className="text-stone-400" />
                ) : (
                  <ChevronDown size={14} className="text-stone-400" />
                )}
              </button>

              {/* Expanded detail */}
              {isExpanded && (
                <div className="mt-3 pt-3 border-t border-stone-100 space-y-2 text-xs">
                  {/* Tool scope */}
                  {scope && (
                    <div className="flex flex-wrap gap-x-4 gap-y-1 text-stone-500">
                      <span>来源: {scope.source}</span>
                      <span>类型: {scope.actionType}</span>
                      {scope.capabilities.length > 0 && (
                        <span>能力: {scope.capabilities.join(", ")}</span>
                      )}
                    </div>
                  )}

                  {/* Status info */}
                  <div className="flex flex-wrap gap-x-4 gap-y-1 text-stone-500">
                    <span>状态: {action.status}</span>
                    {action.permissionDecision && <span>权限: {action.permissionDecision}</span>}
                    {action.startedAt && (
                      <span>开始: {new Date(action.startedAt).toLocaleString("zh-CN")}</span>
                    )}
                    {action.finishedAt && (
                      <span>完成: {new Date(action.finishedAt).toLocaleString("zh-CN")}</span>
                    )}
                  </div>

                  {/* Block reason */}
                  {isBlocked && (
                    <div className="rounded bg-orange-100/50 border border-orange-200 px-3 py-2 space-y-1">
                      <div className="font-medium text-orange-800 flex items-center gap-1.5">
                        <Info size={12} /> 阻断原因
                      </div>
                      <div className="text-orange-700">
                        {action.status === "blocked"
                          ? "该工具调用被权限策略或沙盒规则阻断。"
                          : "该工具调用需要用户授权确认。"}
                      </div>
                      {scope && isDeclarativeOnly(scope) && (
                        <div className="text-orange-600 text-[11px]">
                          <EyeOff size={10} className="inline mr-1" />
                          此工具为声明式 (declarative-only)，不具备实际执行能力。
                        </div>
                      )}
                      <Link
                        to="/review"
                        className="inline-flex items-center gap-1 text-orange-700 hover:text-orange-900 underline text-[11px]"
                      >
                        <Eye size={10} /> 查看权限/提案
                      </Link>
                    </div>
                  )}

                  {/* Error (bounded) */}
                  {isFailed && action.error && (
                    <div className="rounded bg-red-100/50 border border-red-200 px-3 py-2">
                      <div className="font-medium text-red-800 mb-1">错误</div>
                      {(() => {
                        const preview = boundedPreview(action.error, 500);
                        return (
                          <>
                            <div className="text-red-700 whitespace-pre-wrap break-all text-[11px]">
                              {preview.display}
                            </div>
                            {preview.truncated && (
                              <div className="text-[10px] text-red-500 mt-1">
                                已截断（估算长度 {preview.estimatedLength} 字符）
                              </div>
                            )}
                          </>
                        );
                      })()}
                    </div>
                  )}

                  {/* Output (bounded preview) */}
                  {action.output !== undefined && action.output !== null && (
                    <div className="rounded bg-stone-50 border border-stone-200 px-3 py-2">
                      <div className="font-medium text-stone-600 mb-1">输出</div>
                      {(() => {
                        const preview = boundedPreview(action.output);
                        return (
                          <>
                            <pre className="text-stone-700 whitespace-pre-wrap break-all text-[11px]">
                              {preview.display}
                            </pre>
                            {preview.truncated && (
                              <div className="text-[10px] text-stone-400 mt-1">
                                已截断 — 预览 {preview.display.length - 1} / 估算{" "}
                                {preview.estimatedLength} 字符
                              </div>
                            )}
                          </>
                        );
                      })()}
                    </div>
                  )}

                  {/* Linked proposal */}
                  {(() => {
                    let pid: string | null = null;
                    if (action.output) {
                      if (typeof action.output === "object" && action.output !== null) {
                        const d = (action.output as Record<string, unknown>).proposal_id;
                        if (typeof d === "string") pid = d;
                      }
                    }
                    if (!pid && run.generatedProposals.length > 0) {
                      pid = run.generatedProposals[0];
                    }
                    return pid ? (
                      <div className="text-[11px] text-blue-600 flex items-center gap-1">
                        <Eye size={10} />
                        <Link to={`/review?proposal=${pid}`} className="hover:underline">
                          关联提案: {pid.slice(0, 8)}…
                        </Link>
                      </div>
                    ) : null;
                  })()}
                </div>
              )}
            </div>
          );
        })}

        {/* Observations */}
        {run.observations.length > 0 && (
          <div className="mt-3">
            <button
              onClick={() => setShowObservations(!showObservations)}
              className="w-full flex items-center justify-between rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600 hover:bg-stone-100 transition"
            >
              <span className="flex items-center gap-2">
                <Eye size={12} />
                观察记录 ({run.observations.length})
              </span>
              {showObservations ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
            </button>
            {showObservations && (
              <div className="mt-2 space-y-2">
                {run.observations.map(obs => {
                  const contentPreview = boundedPreview(obs.content, 500);
                  const structurePreview = obs.structuredResult
                    ? boundedPreview(obs.structuredResult, 500)
                    : null;

                  return (
                    <div
                      key={obs.id}
                      className="rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs"
                    >
                      <div className="flex items-center justify-between mb-1">
                        <span className="font-medium text-stone-700">来源: {obs.source}</span>
                        <span className="text-stone-400">
                          {new Date(obs.timestamp).toLocaleTimeString("zh-CN", {
                            hour: "2-digit",
                            minute: "2-digit",
                          })}
                        </span>
                      </div>
                      <div className="text-stone-600 whitespace-pre-wrap break-all text-[11px]">
                        {contentPreview.display}
                      </div>
                      {contentPreview.truncated && (
                        <div className="text-[10px] text-stone-400 mt-1">
                          已截断 — 估算长度 {contentPreview.estimatedLength} 字符
                        </div>
                      )}
                      {obs.actionId && (
                        <div className="text-stone-400 mt-1">
                          关联 Action: {obs.actionId.slice(0, 8)}…
                        </div>
                      )}
                      {structurePreview && (
                        <details className="mt-2">
                          <summary className="text-stone-500 cursor-pointer">
                            结构化结果
                            {structurePreview.truncated && (
                              <span className="text-stone-400 ml-1">
                                (已截断, {structurePreview.estimatedLength} 字符)
                              </span>
                            )}
                          </summary>
                          <pre className="mt-1 bg-stone-50 rounded p-2 text-stone-600 text-[11px] whitespace-pre-wrap break-all">
                            {structurePreview.display}
                          </pre>
                        </details>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
