import { useState } from "react";
import {
  Wrench,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  ExternalLink,
  ChevronDown,
  ChevronUp,
  Shield,
  Clock,
  ShieldCheck,
  RotateCcw,
  Info,
} from "lucide-react";
import type { ToolCallResult } from "../tauri";
import { mailboxRoute } from "../productShellContract";

interface Props {
  call: ToolCallResult;
  onExecute?: () => Promise<void>;
  onReplay?: () => Promise<void>;
}

function StatusBadge({ call }: { call: ToolCallResult }) {
  const status = call.status;
  const decision = call.permission_decision;

  if (status === "needs_confirmation" || decision === "ask_every_time") {
    return (
      <span className="inline-flex items-center gap-1 text-orange-600 text-xs bg-orange-50 px-1.5 py-0.5 rounded">
        <Clock size={12} /> 待授权
      </span>
    );
  }
  if (status === "blocked" || decision === "deny") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-xs bg-red-50 px-1.5 py-0.5 rounded">
        <Shield size={12} /> 已阻断
      </span>
    );
  }
  if (status === "pending") {
    return (
      <span className="inline-flex items-center gap-1 text-amber-600 text-xs bg-amber-50 px-1.5 py-0.5 rounded">
        <Clock size={12} /> 执行中
      </span>
    );
  }
  if (call.success || status === "success") {
    return (
      <span className="inline-flex items-center gap-1 text-green-600 text-xs bg-green-50 px-1.5 py-0.5 rounded">
        <CheckCircle2 size={12} /> 成功
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-red-600 text-xs bg-red-50 px-1.5 py-0.5 rounded">
      <XCircle size={12} /> 失败
    </span>
  );
}

function RiskBadge({ level }: { level?: string }) {
  if (level === "high") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-xs bg-red-50 px-1.5 py-0.5 rounded">
        <AlertTriangle size={12} /> 高风险
      </span>
    );
  }
  if (level === "medium") {
    return (
      <span className="inline-flex items-center gap-1 text-amber-600 text-xs bg-amber-50 px-1.5 py-0.5 rounded">
        <ShieldCheck size={12} /> 中风险
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-green-600 text-xs bg-green-50 px-1.5 py-0.5 rounded">
      <ShieldCheck size={12} /> 低风险
    </span>
  );
}

function PermissionLabel({ decision }: { decision?: string }) {
  const labels: Record<string, string> = {
    allow: "允许",
    deny: "拒绝",
    ask_every_time: "每次询问",
    allow_once: "允许一次",
  };
  if (!decision) return null;
  return (
    <span className="text-xs text-gray-500">
      权限: {labels[decision] || decision}
      {decision === "allow_once" && <span className="text-amber-600 ml-1">(一次性)</span>}
    </span>
  );
}

function redactInline(text: string): string {
  return text
    .replace(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi, "[redacted]")
    .replace(/raw[-_\s][^\s,，。;；)）]+/gi, "[redacted]");
}

export default function ToolCallCard({ call, onExecute, onReplay }: Props) {
  const [executing, setExecuting] = useState(false);
  const [replaying, setReplaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const trace = call.react_trace;

  const isBlocked =
    call.status === "blocked" ||
    call.status === "needs_confirmation" ||
    call.requires_confirmation ||
    call.permission_decision === "deny" ||
    call.permission_decision === "ask_every_time";

  const isFailed = !call.success && (call.status === "error" || call.error);
  const canReplay = Boolean(onReplay && call.replayable);

  const handleExecute = async () => {
    if (!onExecute || executing) return;
    setExecuting(true);
    setError(null);
    try {
      await onExecute();
    } catch (e) {
      const errMsg = String(e);
      if (errMsg.includes("not authorized") || errMsg.includes("Mailbox")) {
        setError("请在 Mailbox 授权后重新执行");
      } else {
        setError(errMsg);
      }
    } finally {
      setExecuting(false);
    }
  };

  const handleReplay = async () => {
    if (!onReplay || replaying) return;
    setReplaying(true);
    setError(null);
    try {
      await onReplay();
    } catch (e) {
      setError(String(e));
    } finally {
      setReplaying(false);
    }
  };

  const openReviewCenter = () => {
    window.location.hash = mailboxRoute();
  };

  // Result preview (truncated)
  const resultPreview = call.success
    ? (trace?.outputPreview ?? (call.output ? `${call.output.length} bytes redacted` : null))
    : null;

  return (
    <div className="border rounded-lg p-3 bg-white/60 text-sm space-y-2">
      {/* Header row */}
      <div className="flex items-center gap-2 flex-wrap">
        <Wrench size={14} className="text-gray-500 flex-shrink-0" />
        <span className="font-medium">{call.name}</span>
        <StatusBadge call={call} />
        <div className="ml-auto">
          <RiskBadge
            level={call.permission_level || (call.requires_confirmation ? "high" : "low")}
          />
        </div>
      </div>

      {/* Permission decision */}
      {call.permission_decision && <PermissionLabel decision={call.permission_decision} />}

      {/* Result preview (when not expanded) */}
      {resultPreview && !expanded && (
        <div className="text-xs text-gray-600 bg-green-50/50 rounded px-2 py-1.5 border border-green-100">
          <span className="font-medium text-green-700">结果摘要: </span>
          <span className="line-clamp-2">{resultPreview}</span>
          {trace?.outputHash && <span className="ml-2 text-green-700">{trace.outputHash}</span>}
        </div>
      )}

      {/* Error preview (when not expanded) */}
      {isFailed && !expanded && call.error && (
        <div className="text-xs text-red-600 bg-red-50 rounded px-2 py-1.5 border border-red-100">
          <span className="font-medium">错误: </span>
          <span className="line-clamp-2">{redactInline(call.error)}</span>
        </div>
      )}

      {/* Blocked / Needs Confirmation state */}
      {isBlocked && (
        <div className="rounded-md bg-orange-50 border border-orange-100 p-3 space-y-2">
          <div className="flex items-start gap-2">
            <Info size={14} className="text-orange-600 mt-0.5 flex-shrink-0" />
            <p className="text-xs text-orange-800">
              {call.status === "blocked" || call.permission_decision === "deny"
                ? "该工具调用已被权限策略阻断。"
                : "该工具调用需要授权确认。"}
            </p>
          </div>
          {call.privacy_warnings && call.privacy_warnings.length > 0 && (
            <div className="text-xs text-orange-900 bg-white/80 rounded p-2">
              <div className="font-medium mb-1">隐私提醒:</div>
              <ul className="list-disc pl-4 space-y-1">
                {call.privacy_warnings.map(warning => (
                  <li key={warning}>{redactInline(warning)}</li>
                ))}
              </ul>
            </div>
          )}
          {call.sanitized_arguments && Object.keys(call.sanitized_arguments).length > 0 && (
            <div className="text-xs text-gray-700 bg-white/80 rounded p-2">
              <div className="font-medium mb-1">脱敏后参数预览:</div>
              <pre className="whitespace-pre-wrap break-all">
                {JSON.stringify(call.sanitized_arguments, null, 2)}
              </pre>
            </div>
          )}
          {error && <div className="text-xs text-red-600 bg-red-50 rounded p-2">{error}</div>}
          <div className="flex gap-2 flex-wrap">
            {call.permission_decision !== "deny" && (
              <button
                onClick={handleExecute}
                disabled={executing}
                className="px-3 py-1.5 rounded bg-orange-600 text-white text-xs hover:bg-orange-700 disabled:opacity-50"
              >
                {executing ? "执行中..." : "重新执行"}
              </button>
            )}
            <button
              onClick={openReviewCenter}
              className="px-3 py-1.5 rounded border border-orange-300 text-orange-700 text-xs hover:bg-orange-100 inline-flex items-center gap-1"
            >
              <ExternalLink size={12} />去 Mailbox 授权
            </button>
          </div>
        </div>
      )}

      {/* Replay button for failed tools */}
      {isFailed && !isBlocked && canReplay && (
        <div className="flex items-center gap-2">
          <button
            onClick={handleReplay}
            disabled={replaying}
            className="inline-flex items-center gap-1 px-3 py-1.5 rounded border border-gray-300 text-gray-700 text-xs hover:bg-gray-50 disabled:opacity-50"
          >
            <RotateCcw size={12} />
            {replaying ? "重试中..." : "重试"}
          </button>
        </div>
      )}

      {/* Expandable details */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700"
      >
        {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        {expanded ? "收起详情" : "展开详情"}
      </button>

      {expanded && (
        <div className="space-y-2">
          {trace && (
            <div className="text-xs text-gray-600 bg-gray-50 rounded p-2">
              <div className="font-medium mb-1">Trace 摘要:</div>
              <div>Source: {trace.toolSource}</div>
              <div>Risk: {trace.riskLevel}</div>
              <div>Status: {trace.status}</div>
              {trace.outputPreview && <div>Output: {trace.outputPreview}</div>}
              {trace.outputHash && <div>Hash: {trace.outputHash}</div>}
              {trace.outputByteCount !== undefined && <div>Bytes: {trace.outputByteCount}</div>}
              {trace.proposalId && <div>Proposal: {trace.proposalId}</div>}
            </div>
          )}
          {call.sanitized_arguments && Object.keys(call.sanitized_arguments).length > 0 && (
            <div className="text-xs text-gray-600 bg-slate-50 rounded p-2">
              <div className="font-medium mb-1">脱敏后参数:</div>
              <pre className="whitespace-pre-wrap break-all">
                {JSON.stringify(call.sanitized_arguments, null, 2)}
              </pre>
            </div>
          )}
          {call.success && (trace?.outputPreview || trace?.outputHash) && (
            <div className="text-xs text-gray-700 bg-green-50 rounded p-2">
              <div className="font-medium mb-1">结果摘要:</div>
              {trace?.outputPreview && <div>{trace.outputPreview}</div>}
              {trace?.outputHash && <div>{trace.outputHash}</div>}
            </div>
          )}
          {call.error && (
            <div className="text-xs text-red-700 bg-red-50 rounded p-2">
              <div className="font-medium mb-1">错误详情:</div>
              <pre className="whitespace-pre-wrap break-all">{redactInline(call.error)}</pre>
            </div>
          )}
          <div className="flex flex-wrap gap-x-3 text-xs text-gray-400">
            {call.action_id && <span>Action ID: {call.action_id}</span>}
            {call.run_id && <span>Run ID: {call.run_id}</span>}
          </div>
        </div>
      )}
    </div>
  );
}
