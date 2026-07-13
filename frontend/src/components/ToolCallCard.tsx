import { useState } from "react";
import {
  Wrench,
  CheckCircle2,
  XCircle,
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  ShieldCheck,
  Info,
} from "lucide-react";
import type { ToolActionEffect, ToolCallResult } from "../tauri";

interface Props {
  call: ToolCallResult;
}

function StatusBadge({ call }: { call: ToolCallResult }) {
  const status = call.status;

  if (status === "not_dispatched") {
    return (
      <span className="inline-flex items-center gap-1 text-stone-600 text-xs bg-stone-100 px-1.5 py-0.5 rounded">
        <Info size={12} /> 未执行
      </span>
    );
  }
  if (status === "locally_aborted") {
    return (
      <span className="inline-flex items-center gap-1 text-amber-700 text-xs bg-amber-50 px-1.5 py-0.5 rounded">
        <AlertTriangle size={12} /> 本地已中止
      </span>
    );
  }
  if (status === "remote_unknown") {
    return (
      <span className="inline-flex items-center gap-1 text-amber-700 text-xs bg-amber-50 px-1.5 py-0.5 rounded">
        <AlertTriangle size={12} /> 远端状态未知
      </span>
    );
  }
  if (status === "effect_unknown") {
    return (
      <span className="inline-flex items-center gap-1 text-amber-700 text-xs bg-amber-50 px-1.5 py-0.5 rounded">
        <AlertTriangle size={12} /> 效果未知
      </span>
    );
  }
  if (status === "success") {
    return (
      <span className="inline-flex items-center gap-1 text-green-600 text-xs bg-green-50 px-1.5 py-0.5 rounded">
        <CheckCircle2 size={12} /> 成功
      </span>
    );
  }
  if (status === "failed") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-xs bg-red-50 px-1.5 py-0.5 rounded">
        <XCircle size={12} /> 失败
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-stone-600 text-xs bg-stone-100 px-1.5 py-0.5 rounded">
      <Info size={12} /> 状态未知
    </span>
  );
}

function RiskBadge({ effect }: { effect: ToolActionEffect }) {
  if (effect === "external_mutation" || effect === "unknown") {
    return (
      <span className="inline-flex items-center gap-1 text-red-600 text-xs bg-red-50 px-1.5 py-0.5 rounded">
        <AlertTriangle size={12} /> 高风险
      </span>
    );
  }
  if (effect === "local_mutation" || effect === "proposal_only") {
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

export default function ToolCallCard({ call }: Props) {
  const [expanded, setExpanded] = useState(false);
  const receipt = call.executionReceipt;
  const outputReceipt = call.outputReceipt;
  const toolLabel = call.toolRef.id === "unknown_tool" ? "Governed tool" : call.toolRef.id;

  const isFailed = call.status === "failed";
  const isUncertain = ["unknown", "remote_unknown", "locally_aborted", "effect_unknown"].includes(
    call.status
  );

  const resultPreview =
    call.status === "success" && outputReceipt
      ? `${outputReceipt.byteCount} bytes · ${outputReceipt.digest}`
      : null;

  return (
    <div className="border rounded-lg p-3 bg-white/60 text-sm space-y-2">
      {/* Header row */}
      <div className="flex items-center gap-2 flex-wrap">
        <Wrench size={14} className="text-gray-500 flex-shrink-0" />
        <span className="font-medium">{toolLabel}</span>
        <StatusBadge call={call} />
        <div className="ml-auto">
          <RiskBadge effect={receipt?.actionEffect ?? "unknown"} />
        </div>
      </div>

      {/* Result preview (when not expanded) */}
      {resultPreview && !expanded && (
        <div className="text-xs text-gray-600 bg-green-50/50 rounded px-2 py-1.5 border border-green-100">
          <span className="font-medium text-green-700">结果摘要: </span>
          <span className="line-clamp-2">{resultPreview}</span>
          {outputReceipt && !outputReceipt.verified && (
            <span className="ml-2 text-amber-700">未持久化校验</span>
          )}
        </div>
      )}

      {isFailed && !expanded && (
        <div className="text-xs text-red-600 bg-red-50 rounded px-2 py-1.5 border border-red-100">
          <span className="font-medium">错误状态: </span>
          <span>{call.failureCode ?? "tool_failed"}</span>
        </div>
      )}

      {isUncertain && (
        <div className="rounded-md bg-amber-50 border border-amber-100 p-3 space-y-2">
          <div className="flex items-start gap-2">
            <Info size={14} className="text-orange-600 mt-0.5 flex-shrink-0" />
            <p className="text-xs text-orange-800">
              {call.status === "remote_unknown"
                ? "本地等待已经结束，但远端是否停止无法确认；不会把它显示为成功或已远端取消。"
                : call.status === "locally_aborted"
                  ? "本地等待已经中止；这不等于远端执行已确认停止。"
                  : call.status === "effect_unknown"
                    ? "调用返回了失败，但副作用是否发生无法确认；不会把它显示为安全失败或自动重试。"
                    : "缺少可验证的执行投影，因此当前工具状态保持未知。"}
            </p>
          </div>
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
          {receipt && (
            <div className="text-xs text-gray-600 bg-gray-50 rounded p-2">
              <div className="font-medium mb-1">Execution receipt:</div>
              <div>Source: {call.toolRef.source}</div>
              <div>Transport: {receipt.transportStatus}</div>
              <div>Effect: {receipt.effectStatus}</div>
              <div>Outcome: {receipt.outcome}</div>
              <div>Dispatch attempts: {receipt.dispatchAttemptCount}</div>
              <div>Request digest: {receipt.requestDigest}</div>
              <div>Verified: {receipt.verified ? "yes" : "no"}</div>
            </div>
          )}
          {outputReceipt && (
            <div className="text-xs text-gray-700 bg-green-50 rounded p-2">
              <div className="font-medium mb-1">Output receipt:</div>
              <div>{outputReceipt.kind}</div>
              <div>{outputReceipt.digest}</div>
              <div>{outputReceipt.byteCount} bytes</div>
            </div>
          )}
          <div className="flex flex-wrap gap-x-3 text-xs text-gray-400">
            <span>Action ref: {call.actionRef}</span>
            {call.runRef && <span>Run ref: {call.runRef}</span>}
          </div>
        </div>
      )}
    </div>
  );
}
