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
} from "lucide-react";
import type { ToolCallResult } from "../tauri";

interface Props {
  call: ToolCallResult;
  onExecute?: () => Promise<void>;
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

export default function ToolCallCard({ call, onExecute }: Props) {
  const isHighRisk = call.permission_level === "high" || call.requires_confirmation;
  const [executing, setExecuting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

  const isBlocked =
    call.status === "blocked" ||
    call.status === "needs_confirmation" ||
    call.requires_confirmation ||
    call.permission_decision === "deny" ||
    call.permission_decision === "ask_every_time";

  const handleExecute = async () => {
    if (!onExecute || executing) return;
    setExecuting(true);
    setError(null);
    try {
      await onExecute();
    } catch (e) {
      const errMsg = String(e);
      if (errMsg.includes("not authorized") || errMsg.includes("Review Center")) {
        setError("请在 Review Center 授权后重新执行");
      } else {
        setError(errMsg);
      }
    } finally {
      setExecuting(false);
    }
  };

  const openReviewCenter = () => {
    window.location.hash = "/review";
  };

  return (
    <div className="border rounded-lg p-3 bg-white/60 text-sm space-y-2">
      <div className="flex items-center gap-2 font-medium">
        <Wrench size={14} className="text-gray-500" />
        <span>{call.name}</span>
        <StatusBadge call={call} />
        {isHighRisk && (
          <span className="ml-auto inline-flex items-center gap-1 text-orange-600 text-xs bg-orange-50 px-1.5 py-0.5 rounded">
            <AlertTriangle size={12} /> 高风险
          </span>
        )}
      </div>

      {/* Permission decision line */}
      {call.permission_decision && (
        <div className="text-xs text-gray-500">
          权限策略: {call.permission_decision}
          {call.permission_decision === "allow_once" && (
            <span className="text-amber-600 ml-1">(一次性授权)</span>
          )}
        </div>
      )}

      {/* Blocked / Needs Confirmation state */}
      {isBlocked && (
        <div className="rounded-md bg-orange-50 border border-orange-100 p-3 space-y-2">
          <p className="text-xs text-orange-800">
            {call.status === "blocked" || call.permission_decision === "deny"
              ? "该工具调用已被权限策略阻断。"
              : "该工具调用需要授权确认。"}
          </p>
          {call.privacy_warnings && call.privacy_warnings.length > 0 && (
            <div className="text-xs text-orange-900 bg-white/80 rounded p-2">
              <div className="font-medium mb-1">隐私提醒:</div>
              <ul className="list-disc pl-4 space-y-1">
                {call.privacy_warnings.map(warning => (
                  <li key={warning}>{warning}</li>
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
          <div className="flex gap-2">
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
              <ExternalLink size={12} />去 Review Center 授权
            </button>
          </div>
        </div>
      )}

      {/* Expandable details for all states */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700"
      >
        {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        {expanded ? "收起详情" : "展开详情"}
      </button>

      {expanded && (
        <div className="space-y-2">
          {call.arguments && Object.keys(call.arguments).length > 0 && (
            <div className="text-xs text-gray-600 bg-gray-50 rounded p-2">
              <div className="font-medium mb-1">参数:</div>
              <pre className="whitespace-pre-wrap break-all">
                {JSON.stringify(call.arguments, null, 2)}
              </pre>
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
          {call.success && call.output && (
            <div className="text-xs text-gray-700 bg-green-50 rounded p-2">
              <div className="font-medium mb-1">结果:</div>
              <pre className="whitespace-pre-wrap break-all">{call.output}</pre>
            </div>
          )}
          {call.error && (
            <div className="text-xs text-red-700 bg-red-50 rounded p-2">
              <div className="font-medium mb-1">错误:</div>
              {call.error}
            </div>
          )}
          {call.action_id && (
            <div className="text-xs text-gray-400">Action ID: {call.action_id}</div>
          )}
          {call.run_id && <div className="text-xs text-gray-400">Run ID: {call.run_id}</div>}
        </div>
      )}
    </div>
  );
}
