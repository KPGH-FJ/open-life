import { useState } from "react";
import { Wrench, CheckCircle2, XCircle, AlertTriangle } from "lucide-react";
import type { ToolCallResult } from "../tauri";

interface Props {
  call: ToolCallResult;
  onExecute?: () => Promise<void>;
}

export default function ToolCallCard({ call, onExecute }: Props) {
  const isHighRisk = call.permission_level === "high" || call.requires_confirmation;
  const [executing, setExecuting] = useState(false);

  const handleExecute = async () => {
    if (!onExecute || executing) return;
    setExecuting(true);
    try {
      await onExecute();
    } finally {
      setExecuting(false);
    }
  };

  return (
    <div className="border rounded-lg p-3 bg-white/60 text-sm space-y-2">
      <div className="flex items-center gap-2 font-medium">
        <Wrench size={14} className="text-gray-500" />
        <span>{call.name}</span>
        {call.requires_confirmation ? (
          <span className="inline-flex items-center gap-1 text-orange-600 text-xs">
            <AlertTriangle size={12} /> 待确认
          </span>
        ) : call.success ? (
          <span className="inline-flex items-center gap-1 text-green-600 text-xs">
            <CheckCircle2 size={12} /> 成功
          </span>
        ) : (
          <span className="inline-flex items-center gap-1 text-red-600 text-xs">
            <XCircle size={12} /> 失败
          </span>
        )}
        {isHighRisk && (
          <span className="ml-auto inline-flex items-center gap-1 text-orange-600 text-xs bg-orange-50 px-1.5 py-0.5 rounded">
            <AlertTriangle size={12} /> 高风险
          </span>
        )}
      </div>

      {call.requires_confirmation && (
        <div className="rounded-md bg-orange-50 border border-orange-100 p-3 space-y-2">
          <p className="text-xs text-orange-800">
            该工具调用涉及高风险操作或敏感参数。确认后才会真正执行。
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
          <div className="flex gap-2">
            <button
              onClick={handleExecute}
              disabled={executing}
              className="px-3 py-1.5 rounded bg-orange-600 text-white text-xs hover:bg-orange-700 disabled:opacity-50"
            >
              {executing ? "执行中..." : "确认执行"}
            </button>
          </div>
        </div>
      )}

      {!call.requires_confirmation && (
        <>
          {call.privacy_warnings && call.privacy_warnings.length > 0 && (
            <div className="text-xs text-amber-800 bg-amber-50 rounded p-2">
              <div className="font-medium mb-1">隐私命中:</div>
              <ul className="list-disc pl-4 space-y-1">
                {call.privacy_warnings.map(warning => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </div>
          )}
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
          {call.success
            ? call.output && (
                <div className="text-xs text-gray-700 bg-green-50 rounded p-2">
                  <div className="font-medium mb-1">结果:</div>
                  <pre className="whitespace-pre-wrap break-all">{call.output}</pre>
                </div>
              )
            : call.error && (
                <div className="text-xs text-red-700 bg-red-50 rounded p-2">
                  <div className="font-medium mb-1">错误:</div>
                  {call.error}
                </div>
              )}
        </>
      )}
    </div>
  );
}
