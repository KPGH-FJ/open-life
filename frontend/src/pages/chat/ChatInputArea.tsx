import { Link } from "react-router-dom";
import { Send, Loader2, Target, Activity, Wifi, WifiOff, Cloud, Server } from "lucide-react";
import type { SystemDiagnostics } from "../../tauri";

interface ChatInputAreaProps {
  input: string;
  sending: boolean;
  streamInterrupted: boolean;
  diagnostics: SystemDiagnostics | null;
  onInputChange: (value: string) => void;
  onSend: () => void;
  onContinueStream: () => void;
  onRetryLastMessage: () => void;
  getFixSuggestion: (
    diagnostics: SystemDiagnostics | null
  ) => { text: string; action: string; link: string } | null;
}

function NetworkStatusIndicator({ diagnostics }: { diagnostics: SystemDiagnostics | null }) {
  if (!diagnostics) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-gray-400">
        <WifiOff size={12} />
        <span>检查中...</span>
      </div>
    );
  }

  if (diagnostics.chat_ready) {
    const backend = diagnostics.ollama_online
      ? `本地 ${diagnostics.resolved_local_model || diagnostics.local_model}`
      : diagnostics.cloud_provider || "云端";
    return (
      <div className="flex items-center gap-1.5 text-xs text-green-600">
        <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
        {diagnostics.ollama_online ? <Server size={12} /> : <Cloud size={12} />}
        <span>{backend}</span>
      </div>
    );
  }

  if (!diagnostics.ollama_online && !diagnostics.cloud_api_configured) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-red-600">
        <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
        <WifiOff size={12} />
        <span>无可用后端</span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1.5 text-xs text-amber-600">
      <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />
      <Wifi size={12} />
      <span>{!diagnostics.ollama_online ? "本地离线" : "云端未配置"}</span>
    </div>
  );
}

export default function ChatInputArea({
  input,
  sending,
  streamInterrupted,
  diagnostics,
  onInputChange,
  onSend,
  onContinueStream,
  onRetryLastMessage,
  getFixSuggestion,
}: ChatInputAreaProps) {
  return (
    <div className="border-t px-6 py-4 bg-white">
      <div className="max-w-3xl mx-auto space-y-2">
        {/* Network status indicator */}
        <div className="flex items-center justify-between">
          <NetworkStatusIndicator diagnostics={diagnostics} />
          {diagnostics?.chat_ready &&
            diagnostics.prefer_local_model &&
            diagnostics.ollama_online && (
              <span className="text-[10px] text-gray-400">本地优先模式</span>
            )}
        </div>
        {diagnostics && !diagnostics.chat_ready && diagnostics.readiness_issues.length > 0 && (
          <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
            <div className="font-medium mb-1">普通对话暂不可用，快捷指令仍可使用：</div>
            <ul className="list-disc pl-4 space-y-1">
              {diagnostics.readiness_issues.map(issue => (
                <li key={issue}>{issue}</li>
              ))}
            </ul>
          </div>
        )}
        {(() => {
          const fix = getFixSuggestion(diagnostics);
          if (!fix) return null;
          return (
            <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-800 flex items-center justify-between">
              <span>{fix.text}</span>
              <Link
                to={fix.link}
                className="ml-3 px-2 py-1 rounded-md bg-blue-600 text-white text-[10px] font-medium hover:bg-blue-700"
              >
                {fix.action}
              </Link>
            </div>
          );
        })()}
        {streamInterrupted && (
          <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800 flex items-center justify-between">
            <span>对话被中断。你可以点击继续，或重新输入。</span>
            <button
              onClick={onContinueStream}
              className="ml-3 px-2 py-1 rounded-md bg-amber-600 text-white text-[10px] font-medium hover:bg-amber-700"
            >
              继续生成
            </button>
          </div>
        )}
        <div className="flex items-center gap-2 text-xs text-gray-500">
          <span className="font-medium text-gray-600">快捷指令:</span>
          <button
            onClick={() => onInputChange("/goal ")}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 hover:bg-gray-200 text-gray-700"
            title="查看今日目标"
          >
            <Target size={12} /> /goal
          </button>
          <button
            onClick={() => onInputChange("/state ")}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 hover:bg-gray-200 text-gray-700"
            title="记录状态"
          >
            <Activity size={12} /> /state
          </button>
          <button
            onClick={onRetryLastMessage}
            disabled={sending}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 hover:bg-gray-200 text-gray-700 disabled:opacity-40"
            title="重新填入上一条用户消息"
          >
            重试上一条
          </button>
        </div>
        <div className="flex gap-3">
          <textarea
            value={input}
            onChange={e => onInputChange(e.target.value)}
            onKeyDown={e => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                onSend();
              }
            }}
            rows={2}
            placeholder={
              diagnostics === null
                ? "正在检测模型状态..."
                : diagnostics.chat_ready
                  ? "输入消息，按 Enter 发送..."
                  : "请先前往设置页配置模型后端..."
            }
            aria-label="输入消息"
            className="flex-1 resize-none border rounded-lg px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />
          <button
            onClick={onSend}
            disabled={sending || !input.trim()}
            className="bg-indigo-600 text-white px-4 py-2 rounded-lg hover:bg-indigo-700 disabled:opacity-50"
            aria-label="发送消息"
            aria-keyshortcuts="Enter"
          >
            {sending ? <Loader2 size={18} className="animate-spin" /> : <Send size={18} />}
          </button>
        </div>
      </div>
    </div>
  );
}
