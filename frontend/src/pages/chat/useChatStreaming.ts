import { useCallback, useEffect, useRef, useState } from "react";
import {
  startStreamMessage,
  logAnalyticsEvent,
  type StreamMessageStartPayload,
  type StreamMessageDonePayload,
  type SystemDiagnostics,
  type ToolCallResult,
  type ReasoningTrace,
} from "../../tauri";
import type { ChatMessage } from "../../types";
import { listen } from "@tauri-apps/api/event";

function formatChatRuntimeError(error: unknown, diagnostics: SystemDiagnostics | null): string {
  if (diagnostics && !diagnostics.chat_ready && diagnostics.readiness_issues?.length) {
    return `暂时无法发送普通对话：\n${diagnostics.readiness_issues.map(issue => `- ${issue}`).join("\n")}\n\n请去设置页查看\u201c试用就绪检查\u201d。`;
  }
  const raw = error instanceof Error ? error.message : String(error);
  const lower = raw.toLowerCase();
  let hint = raw;
  const provider = diagnostics?.cloud_provider ?? "云端模型";
  const providerLower = provider.toLowerCase();
  const looksLikeAuthError =
    lower.includes("api key") ||
    lower.includes("invalid api key") ||
    lower.includes("unauthorized") ||
    lower.includes("401") ||
    lower.includes("403");
  if (lower.includes("deepseek") || providerLower.includes("deepseek")) {
    if (looksLikeAuthError) {
      hint =
        "DeepSeek 鉴权失败。请去设置页确认 API Key 已保存，Provider 选择 DeepSeek，Base URL 为 https://api.deepseek.com，模型为 deepseek-chat。";
    } else if (lower.includes("model") || lower.includes("400")) {
      hint =
        "DeepSeek 请求被拒绝。请去设置页确认模型名为 deepseek-chat，Base URL 为 https://api.deepseek.com，并重新测试连接。";
    } else {
      hint = `DeepSeek 对话请求失败：${raw}`;
    }
  } else if (looksLikeAuthError || lower.includes("openrouter") || lower.includes("openai")) {
    hint = `${provider} 鉴权失败。请去设置页配置 API Key，或切回可用的本地模型。`;
  } else if (
    lower.includes("429") ||
    lower.includes("rate limit") ||
    lower.includes("too many requests")
  ) {
    hint = "请求过于频繁（Rate Limit）。请稍等片刻再试，或切换到另一模型后端。";
  } else if (
    lower.includes("ollama") ||
    lower.includes("connection refused") ||
    lower.includes("11434")
  ) {
    hint = "本地 Ollama 不可用。请启动 Ollama，或安装/切换到已下载的本地模型。";
  } else if (lower.includes("timeout") || lower.includes("timed out")) {
    hint = "模型响应超时。请检查网络连接，或尝试切换更快的模型后端。";
  } else if (
    lower.includes("500") ||
    lower.includes("502") ||
    lower.includes("503") ||
    lower.includes("504")
  ) {
    hint = "云端模型服务暂时不可用（服务器错误）。请稍后重试，或切换到本地模型。";
  } else if (
    lower.includes("network") ||
    lower.includes("fetch") ||
    lower.includes("econnrefused")
  ) {
    hint = "网络连接异常。请检查网络状态，或切换到本地模型以离线使用。";
  } else if (
    lower.includes("no backend") ||
    lower.includes("backend") ||
    lower.includes("未配置")
  ) {
    hint =
      "没有可用的模型后端。请在设置页配置 DeepSeek/OpenAI/OpenRouter API Key，或启动本地 Ollama。";
  }
  return `${hint}\n\n请去设置页查看\u201c试用就绪检查\u201d。`;
}

interface UseChatStreamingOpts {
  currentSessionId: string;
  onSetMessages: React.Dispatch<React.SetStateAction<ChatMessage[]>>;
  onSetReasoningTrace: React.Dispatch<React.SetStateAction<ReasoningTrace | null>>;
  onSetCurrentRunId: React.Dispatch<React.SetStateAction<string | null>>;
  onSetToolCalls: React.Dispatch<React.SetStateAction<ToolCallResult[]>>;
  onSetStreamInterrupted: React.Dispatch<React.SetStateAction<boolean>>;
  loadAgentRunForSession: (runId: string | undefined, sessionId: string) => Promise<void>;
  refreshAgentRuns: (sessionId?: string) => Promise<void>;
  diagnosticsRef: React.MutableRefObject<SystemDiagnostics | null>;
}

export function useChatStreaming(opts: UseChatStreamingOpts) {
  const {
    currentSessionId,
    onSetMessages,
    onSetReasoningTrace,
    onSetCurrentRunId,
    onSetToolCalls,
    onSetStreamInterrupted,
    loadAgentRunForSession,
    refreshAgentRuns,
    diagnosticsRef,
  } = opts;

  const [sending, setSending] = useState(false);
  const [streamingReply, setStreamingReply] = useState("");
  const [streamInterrupted, setStreamInterruptedLocal] = useState(false);
  const streamErrorHandledRef = useRef(false);

  // Streaming buffer management
  const streamingBufferRef = useRef("");
  const streamingRafRef = useRef<number | null>(null);

  const flushStreaming = useCallback(() => {
    if (streamingRafRef.current !== null) {
      cancelAnimationFrame(streamingRafRef.current);
      streamingRafRef.current = null;
    }
    if (streamingBufferRef.current) {
      setStreamingReply(prev => prev + streamingBufferRef.current);
      streamingBufferRef.current = "";
    }
  }, []);

  const scheduleFlushStreaming = useCallback(() => {
    if (streamingRafRef.current !== null) return;
    streamingRafRef.current = requestAnimationFrame(() => {
      streamingRafRef.current = null;
      if (streamingBufferRef.current) {
        setStreamingReply(prev => prev + streamingBufferRef.current);
        streamingBufferRef.current = "";
      }
    });
  }, []);

  // Register stream listeners once per session
  useEffect(() => {
    let unlisteners: (() => void)[] = [];
    let cancelled = false;

    Promise.all([
      listen<StreamMessageStartPayload>("stream-message-start", async event => {
        if (event.payload.session_id === currentSessionId) {
          onSetReasoningTrace(event.payload.reasoning_trace ?? null);
          onSetCurrentRunId(event.payload.run_id);
          onSetToolCalls(
            (event.payload.tool_calls ?? []).map(call => ({
              ...call,
              run_id: event.payload.run_id,
            }))
          );
        }
      }),
      listen<{ session_id: string; chunk: string }>("stream-message-chunk", event => {
        if (event.payload.session_id === currentSessionId) {
          streamingBufferRef.current += event.payload.chunk;
          scheduleFlushStreaming();
        }
      }),
      listen<StreamMessageDonePayload>("stream-message-done", async event => {
        if (event.payload.session_id === currentSessionId) {
          flushStreaming();
          onSetMessages(prev => [
            ...prev,
            { role: "assistant", content: event.payload.reply, run_id: event.payload.run_id },
          ]);
          setStreamingReply("");
          setSending(false);
          onSetReasoningTrace(event.payload.reasoning_trace ?? null);
          onSetCurrentRunId(event.payload.run_id);
          onSetToolCalls(
            (event.payload.tool_calls ?? []).map(call => ({
              ...call,
              run_id: event.payload.run_id,
            }))
          );
          onSetStreamInterrupted(false);
          setStreamInterruptedLocal(false);
          await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
          refreshAgentRuns(event.payload.session_id);
          logAnalyticsEvent("send_message", currentSessionId, undefined).catch(() => {});
        }
      }),
      listen<{ session_id: string; run_id?: string; error: string }>(
        "stream-message-error",
        async event => {
          if (event.payload.session_id === currentSessionId) {
            flushStreaming();
            onSetMessages(prev => [
              ...prev,
              {
                role: "assistant",
                content: formatChatRuntimeError(event.payload.error, diagnosticsRef.current),
              },
            ]);
            streamErrorHandledRef.current = true;
            setStreamingReply("");
            setSending(false);
            onSetStreamInterrupted(true);
            setStreamInterruptedLocal(true);
            await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
            refreshAgentRuns(event.payload.session_id);
          }
        }
      ),
    ]).then(results => {
      if (!cancelled) {
        unlisteners = results;
      } else {
        results.forEach(fn => fn());
      }
    });

    return () => {
      cancelled = true;
      flushStreaming();
      unlisteners.forEach(fn => fn());
    };
  }, [currentSessionId]);

  const prepareForSend = useCallback(() => {
    streamErrorHandledRef.current = false;
    onSetStreamInterrupted(false);
    setStreamInterruptedLocal(false);
    setStreamingReply("");
    streamingBufferRef.current = "";
    onSetReasoningTrace(null);
    onSetToolCalls([]);
  }, []);

  const handleStreamError = useCallback(
    (e: unknown) => {
      flushStreaming();
      if (!streamErrorHandledRef.current) {
        onSetMessages(prev => [
          ...prev,
          { role: "assistant", content: formatChatRuntimeError(e, diagnosticsRef.current) },
        ]);
      }
      setStreamingReply("");
      setSending(false);
    },
    [flushStreaming]
  );

  const startSend = useCallback(
    async (sessionId: string, msgs: ChatMessage[], onDone?: () => void) => {
      setSending(true);
      prepareForSend();
      try {
        await startStreamMessage(sessionId, msgs);
        if (onDone) onDone();
      } catch (e) {
        handleStreamError(e);
      }
    },
    [prepareForSend, handleStreamError]
  );

  return {
    sending,
    setSending,
    streamingReply,
    setStreamingReply,
    streamInterrupted,
    setStreamInterrupted: setStreamInterruptedLocal,
    streamErrorHandledRef,
    streamingBufferRef,
    flushStreaming,
    scheduleFlushStreaming,
    prepareForSend,
    startSend,
    handleStreamError,
  };
}

export { formatChatRuntimeError };
