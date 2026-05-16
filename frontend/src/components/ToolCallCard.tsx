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
  Ban,
} from "lucide-react";
import type { ToolCallResult } from "../tauri";
import { getTypedToolCallViewModel } from "../utils/typedContract";

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

function TypedReasonBlock({
  blockReasonLabel,
  proposalReasonLabel,
  failureKindLabel,
  agentSpecId,
  proposalId,
}: {
  blockReasonLabel: string | null;
  proposalReasonLabel: string | null;
  failureKindLabel: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
}) {
  if (!blockReasonLabel && !proposalReasonLabel && !failureKindLabel && !agentSpecId && !proposalId)
    return null;
  return (
    <div className="rounded-md bg-slate-100 border border-slate-200 p-2 space-y-1 text-[10px]">
      <div className="font-medium text-slate-600 flex items-center gap-1">
        <Ban size={10} />
        Typed Reason
      </div>
      <div className="flex flex-wrap items-center gap-1">
        {blockReasonLabel && (
          <span className="px-1 py-0.5 rounded bg-red-100 text-red-700 font-medium">
            {blockReasonLabel}
          </span>
        )}
        {proposalReasonLabel && (
          <span className="px-1 py-0.5 rounded bg-blue-100 text-blue-700 font-medium">
            {proposalReasonLabel}
          </span>
        )}
        {failureKindLabel && (
          <span className="px-1 py-0.5 rounded bg-red-100 text-red-700 font-medium">
            {failureKindLabel}
          </span>
        )}
      </div>
      {agentSpecId && <div className="text-slate-500">AgentSpec: {agentSpecId}</div>}
      {proposalId && <div className="text-blue-600">Proposal: {proposalId}</div>}
    </div>
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

export default function ToolCallCard({ call, onExecute, onReplay }: Props) {
  const [executing, setExecuting] = useState(false);
  const [replaying, setReplaying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);

  const isBlocked =
    call.status === "blocked" ||
    call.status === "needs_confirmation" ||
    call.requires_confirmation ||
    call.permission_decision === "deny" ||
    call.permission_decision === "ask_every_time";

  const isFailed = !call.success && (call.status === "error" || call.error);

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

  const typedInfo = getTypedToolCallViewModel(call);

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
    window.location.hash = "/review";
  };

  // Result preview (truncated)
  const resultPreview =
    call.success && call.output
      ? typeof call.output === "string"
        ? call.output.slice(0, 120) + (call.output.length > 120 ? "..." : "")
        : null
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
          <span className="font-medium text-green-700">结果: </span>
          <span className="line-clamp-2">{resultPreview}</span>
        </div>
      )}

      {/* Error preview (when not expanded) */}
      {isFailed && !expanded && call.error && (
        <div className="text-xs text-red-600 bg-red-50 rounded px-2 py-1.5 border border-red-100">
          <span className="font-medium">错误: </span>
          <span className="line-clamp-2">{call.error}</span>
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
          <TypedReasonBlock
            blockReasonLabel={typedInfo.blockReasonLabel}
            proposalReasonLabel={typedInfo.proposalReasonLabel}
            failureKindLabel={typedInfo.failureKindLabel}
            agentSpecId={typedInfo.agentSpecId}
            proposalId={typedInfo.proposalId}
          />
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
              <ExternalLink size={12} />去 Review Center 授权
            </button>
          </div>
        </div>
      )}

      {/* Replay button for failed tools */}
      {isFailed && !isBlocked && onReplay && (
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
              <div className="font-medium mb-1">完整结果:</div>
              <pre className="whitespace-pre-wrap break-all">
                {typeof call.output === "string" ? call.output : JSON.stringify(call.output)}
              </pre>
            </div>
          )}
          {call.error && (
            <div className="text-xs text-red-700 bg-red-50 rounded p-2">
              <div className="font-medium mb-1">错误详情:</div>
              <pre className="whitespace-pre-wrap break-all">{call.error}</pre>
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
