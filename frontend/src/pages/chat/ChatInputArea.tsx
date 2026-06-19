import { Link } from "react-router-dom";
import {
  Send,
  Loader2,
  Target,
  Activity,
  Wifi,
  WifiOff,
  Cloud,
  Server,
  FileText,
  X,
} from "lucide-react";
import type { SystemDiagnostics } from "../../tauri";

interface ChatInputAreaProps {
  input: string;
  sending: boolean;
  streamInterrupted: boolean;
  diagnostics: SystemDiagnostics | null;
  selectedSkillId: string;
  companionMode?: boolean;
  onInputChange: (value: string) => void;
  onSelectedSkillIdChange: (value: string) => void;
  onComposerFocus?: () => void;
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
  selectedSkillId,
  companionMode = false,
  onInputChange,
  onSelectedSkillIdChange,
  onComposerFocus,
  onSend,
  onContinueStream,
  onRetryLastMessage,
  getFixSuggestion,
}: ChatInputAreaProps) {
  return (
    <div
      className={
        companionMode
          ? "border-t border-stone-200 bg-[#fffefa] px-5 py-4"
          : "border-t bg-white px-6 py-4"
      }
    >
      <div className={companionMode ? "w-full space-y-3" : "mx-auto max-w-3xl space-y-2"}>
        {/* Network status indicator */}
        {!companionMode && (
          <div className="flex items-center justify-between">
            <NetworkStatusIndicator diagnostics={diagnostics} />
            {diagnostics?.chat_ready &&
              diagnostics.prefer_local_model &&
              diagnostics.ollama_online && (
                <span className="text-[10px] text-gray-400">本地优先模式</span>
              )}
          </div>
        )}
        {!companionMode &&
          diagnostics &&
          !diagnostics.chat_ready &&
          diagnostics.readiness_issues.length > 0 && (
            <div className="rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              <div className="mb-1 font-medium">普通对话暂不可用，快捷指令仍可使用：</div>
              <ul className="list-disc space-y-1 pl-4">
                {diagnostics.readiness_issues.map(issue => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </div>
          )}
        {!companionMode &&
          (() => {
            const fix = getFixSuggestion(diagnostics);
            if (!fix) return null;
            return (
              <div className="flex items-center justify-between rounded-lg border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-800">
                <span>{fix.text}</span>
                <Link
                  to={fix.link}
                  className="ml-3 rounded-md bg-blue-600 px-2 py-1 text-[10px] font-medium text-white hover:bg-blue-700"
                >
                  {fix.action}
                </Link>
              </div>
            );
          })()}
        {streamInterrupted && (
          <div className="flex items-center justify-between rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
            <span>对话被中断。你可以点击继续，或重新输入。</span>
            <button
              onClick={onContinueStream}
              className="ml-3 px-2 py-1 rounded-md bg-amber-600 text-white text-[10px] font-medium hover:bg-amber-700"
            >
              继续生成
            </button>
          </div>
        )}
        {!companionMode && (
          <div className="flex flex-wrap items-center gap-2 text-xs text-gray-500">
            <div className="flex items-center gap-2">
              <span className="font-medium text-gray-600">快捷指令:</span>
              <button
                onClick={() => onInputChange("/goal ")}
                className="inline-flex items-center gap-1 rounded-md bg-gray-100 px-2 py-1 text-gray-700 hover:bg-gray-200"
                title="查看今日目标"
              >
                <Target size={12} /> /goal
              </button>
              <button
                onClick={() => onInputChange("/state ")}
                className="inline-flex items-center gap-1 rounded-md bg-gray-100 px-2 py-1 text-gray-700 hover:bg-gray-200"
                title="记录状态"
              >
                <Activity size={12} /> /state
              </button>
              <button
                onClick={onRetryLastMessage}
                disabled={sending}
                className="inline-flex items-center gap-1 rounded-md bg-gray-100 px-2 py-1 text-gray-700 hover:bg-gray-200 disabled:opacity-40"
                title="重新填入上一条用户消息"
              >
                重试上一条
              </button>
            </div>
            <label
              data-testid="skill-context-control"
              data-selected-skill-id={selectedSkillId.trim()}
              className="ml-auto flex min-w-[180px] max-w-[260px] flex-1 items-center gap-1.5 rounded-md border border-gray-200 bg-white px-2 py-1 text-gray-600"
            >
              <FileText size={12} className="shrink-0 text-gray-400" />
              <span className="shrink-0 font-medium text-gray-500">SKILL.md</span>
              <input
                data-testid="skill-context-input"
                aria-label="Skill context"
                value={selectedSkillId}
                onChange={e => onSelectedSkillIdChange(e.target.value)}
                placeholder="weekly-planning"
                className="min-w-0 flex-1 bg-transparent text-xs text-gray-700 placeholder:text-gray-300 focus:outline-none"
              />
              {selectedSkillId.trim() && (
                <button
                  type="button"
                  onClick={() => onSelectedSkillIdChange("")}
                  className="rounded p-0.5 text-gray-400 hover:bg-gray-100 hover:text-gray-600"
                  title="清除技能上下文"
                >
                  <X size={12} />
                </button>
              )}
            </label>
          </div>
        )}
        <div className="flex gap-3">
          <textarea
            data-testid="chat-input"
            value={input}
            onChange={e => onInputChange(e.target.value)}
            onFocus={onComposerFocus}
            onKeyDown={e => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                onSend();
              }
            }}
            rows={companionMode ? 1 : 2}
            placeholder={companionMode ? "输入消息" : "输入消息，按 Enter 发送..."}
            className={
              companionMode
                ? "min-h-14 flex-1 resize-none rounded-lg border border-stone-300 bg-white px-4 py-4 text-sm text-stone-950 placeholder:text-stone-400 focus:outline-none focus:ring-2 focus:ring-stone-900/20"
                : "flex-1 resize-none rounded-lg border px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            }
          />
          <button
            data-testid="send-button"
            onClick={onSend}
            disabled={sending || !input.trim()}
            className={
              companionMode
                ? "flex h-14 w-14 shrink-0 items-center justify-center rounded-lg bg-stone-900 text-white hover:bg-stone-800 disabled:opacity-50"
                : "rounded-lg bg-indigo-600 px-4 py-2 text-white hover:bg-indigo-700 disabled:opacity-50"
            }
          >
            {sending ? <Loader2 size={18} className="animate-spin" /> : <Send size={18} />}
          </button>
        </div>
      </div>
    </div>
  );
}
