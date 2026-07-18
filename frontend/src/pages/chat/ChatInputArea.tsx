import { useRef } from "react";
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
  Paperclip,
  X,
  Square,
} from "lucide-react";
import type { ImportedResourceReceipt, SystemDiagnostics } from "../../tauri";

interface ChatInputAreaProps {
  input: string;
  sending: boolean;
  streamInterrupted: boolean;
  diagnostics: SystemDiagnostics | null;
  selectedSkillId: string;
  attachments: ImportedResourceReceipt[];
  resourceImportBusy: boolean;
  resourceImportError?: string | null;
  resourceImportNotice?: string | null;
  removingResourceIds?: string[];
  companionMode?: boolean;
  onInputChange: (value: string) => void;
  onSelectedSkillIdChange: (value: string) => void;
  onAttachResources: () => void;
  onCancelResourceImport: () => void;
  onRemoveResource: (resourceId: string) => void;
  onComposerFocus?: () => void;
  onSend: () => void;
  canCancel?: boolean;
  cancelBusy?: boolean;
  onCancel?: () => void;
  onContinueStream: () => void;
  onRetryLastMessage: () => void;
  getFixSuggestion: (
    diagnostics: SystemDiagnostics | null
  ) => { text: string; action: string; link: string } | null;
}

function formatByteCount(byteCount: number): string {
  if (byteCount < 1024) return `${byteCount} B`;
  if (byteCount < 1024 * 1024) return `${(byteCount / 1024).toFixed(1)} KB`;
  return `${(byteCount / (1024 * 1024)).toFixed(1)} MB`;
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
  attachments,
  resourceImportBusy,
  resourceImportError = null,
  resourceImportNotice = null,
  removingResourceIds = [],
  companionMode = false,
  onInputChange,
  onSelectedSkillIdChange,
  onAttachResources,
  onCancelResourceImport,
  onRemoveResource,
  onComposerFocus,
  onSend,
  canCancel = false,
  cancelBusy = false,
  onCancel,
  onContinueStream,
  onRetryLastMessage,
  getFixSuggestion,
}: ChatInputAreaProps) {
  const isComposingRef = useRef(false);
  const showCancelButton = sending && canCancel && Boolean(onCancel);

  return (
    <div
      className={
        companionMode
          ? "border-t border-stone-200 bg-[#fffefa] px-4 py-4 sm:px-5"
          : "border-t bg-white px-4 py-4 sm:px-6"
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
        {(attachments.length > 0 ||
          resourceImportBusy ||
          resourceImportError ||
          resourceImportNotice) && (
          <div className="space-y-2" data-testid="resource-attachment-surface">
            {attachments.length > 0 && (
              <div className="flex flex-wrap gap-2" aria-label="本轮附件">
                {attachments.map(resource => {
                  const removing = removingResourceIds.includes(resource.resourceId);
                  return (
                    <div
                      key={resource.resourceId}
                      className="flex max-w-full items-center gap-2 rounded-lg border border-indigo-100 bg-indigo-50 px-2.5 py-1.5 text-xs text-indigo-950"
                      data-testid={`resource-attachment-${resource.resourceId}`}
                    >
                      <FileText size={13} className="shrink-0 text-indigo-500" />
                      <span
                        className="max-w-[18rem] truncate font-medium"
                        title={resource.filename}
                      >
                        {resource.filename}
                      </span>
                      <span className="shrink-0 text-indigo-500">
                        {formatByteCount(resource.byteCount)}
                      </span>
                      <button
                        type="button"
                        aria-label={`移除附件 ${resource.filename}`}
                        title="从本轮移除；仅在没有其他引用时删除 OpenLife 中的文件副本"
                        disabled={sending || resourceImportBusy || removing}
                        onClick={() => onRemoveResource(resource.resourceId)}
                        className="rounded p-0.5 text-indigo-500 hover:bg-indigo-100 hover:text-indigo-800 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        {removing ? (
                          <Loader2 size={13} className="animate-spin" aria-hidden="true" />
                        ) : (
                          <X size={13} aria-hidden="true" />
                        )}
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
            {resourceImportError && (
              <p role="alert" className="text-xs text-red-600">
                {resourceImportError}
              </p>
            )}
            {resourceImportNotice && (
              <p role="status" className="text-xs text-gray-500">
                {resourceImportNotice}
              </p>
            )}
          </div>
        )}
        <div className="flex min-w-0 items-end gap-2 sm:gap-3">
          <button
            data-testid="attach-resource-button"
            type="button"
            aria-label={resourceImportBusy ? "附件导入进行中" : "添加文件"}
            title={resourceImportBusy ? "附件导入进行中" : "添加文件（最多 5 个）"}
            onClick={onAttachResources}
            disabled={sending || resourceImportBusy}
            className={
              companionMode
                ? "inline-flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-stone-300 bg-white text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-50"
                : "inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-lg border border-gray-200 bg-white text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
            }
          >
            {resourceImportBusy ? (
              <Loader2 size={18} className="animate-spin" aria-hidden="true" />
            ) : (
              <Paperclip size={18} aria-hidden="true" />
            )}
          </button>
          <textarea
            data-testid="chat-input"
            aria-label="消息输入"
            value={input}
            onChange={e => onInputChange(e.target.value)}
            onFocus={onComposerFocus}
            onCompositionStart={() => {
              isComposingRef.current = true;
            }}
            onCompositionEnd={() => {
              isComposingRef.current = false;
            }}
            onKeyDown={e => {
              if (e.key === "Enter" && !e.shiftKey) {
                const nativeEvent = e.nativeEvent as KeyboardEvent;
                if (
                  isComposingRef.current ||
                  nativeEvent.isComposing ||
                  nativeEvent.keyCode === 229
                ) {
                  return;
                }
                e.preventDefault();
                onSend();
              }
            }}
            rows={companionMode ? 1 : 2}
            placeholder={companionMode ? "输入消息" : "输入消息，按 Enter 发送..."}
            className={
              companionMode
                ? "max-h-36 min-h-14 min-w-0 flex-1 resize-none overflow-y-auto rounded-lg border border-stone-300 bg-white px-4 py-4 text-sm text-stone-950 placeholder:text-stone-400 focus:outline-none focus:ring-2 focus:ring-stone-900/20"
                : "max-h-36 min-w-0 flex-1 resize-none overflow-y-auto rounded-lg border px-4 py-3 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            }
          />
          {showCancelButton && (
            <button
              data-testid="cancel-send-button"
              type="button"
              aria-label="停止生成"
              title="停止生成"
              onClick={onCancel}
              disabled={cancelBusy}
              className={
                companionMode
                  ? "inline-flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-stone-300 bg-white text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-50"
                  : "inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-lg border border-gray-200 bg-white text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50"
              }
            >
              {cancelBusy ? (
                <Loader2 size={18} className="animate-spin" aria-hidden="true" />
              ) : (
                <Square size={17} aria-hidden="true" />
              )}
            </button>
          )}
          <button
            data-testid="send-button"
            type="button"
            aria-label={sending ? "正在发送消息" : "发送消息"}
            title={sending ? "正在发送消息" : "发送消息"}
            onClick={onSend}
            disabled={sending || resourceImportBusy || (!input.trim() && attachments.length === 0)}
            className={
              companionMode
                ? "flex h-14 w-14 shrink-0 items-center justify-center rounded-lg bg-stone-900 text-white hover:bg-stone-800 disabled:opacity-50"
                : "inline-flex h-12 w-12 shrink-0 items-center justify-center rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50"
            }
          >
            {sending ? (
              <Loader2 size={18} className="animate-spin" aria-hidden="true" />
            ) : (
              <Send size={18} aria-hidden="true" />
            )}
          </button>
        </div>
        {resourceImportBusy && (
          <div className="flex items-center justify-between gap-3 text-xs text-gray-500">
            <span>正在读取并解析所选文件；发送前不会交给模型。</span>
            <button
              type="button"
              onClick={onCancelResourceImport}
              className="shrink-0 rounded-md border border-gray-200 bg-white px-2 py-1 text-gray-700 hover:bg-gray-50"
            >
              请求停止导入
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
