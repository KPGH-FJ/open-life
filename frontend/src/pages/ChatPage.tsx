import { useEffect, useRef, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import {
  Loader2,
  ThumbsUp,
  ThumbsDown,
  Hammer,
  ArrowRight,
  X,
  MessageSquare,
  Target,
  Activity,
  Compass,
  Sparkles,
  Heart,
  CheckCircle2,
} from "lucide-react";
import type { ChatMessage, LifeModel } from "../types";
import LoadingSpinner from "../components/LoadingSpinner";
import {
  startStreamMessage,
  getChatHistory,
  getSystemDiagnostics,
  getSchedulerConfig,
  setSchedulerConfig,
  saveFeedback,
  saveChatMessage,
  logAnalyticsEvent,
  getLifeModel,
  listChatSessions,
  createChatSession,
  renameChatSession,
  deleteChatSession,
  getDailyGoals,
  addDailyGoal,
  toggleDailyGoal,
  recordState,
  replayAgentAction,
  indexMemoryChunk,
  listAgentRunsForSession,
  getAgentRun,
} from "../tauri";
import type {
  AgentRun,
  ChatSession,
  ReasoningTrace,
  StreamMessageDonePayload,
  StreamMessageStartPayload,
  SystemDiagnostics,
  ToolCallResult,
} from "../tauri";
import { getModelEmptyState } from "../utils/modelEmpty";
import { listen } from "@tauri-apps/api/event";
import ReasoningTracePanel from "../components/ReasoningTracePanel";
import ToolCallCard from "../components/ToolCallCard";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import ChatSidebar from "./chat/ChatSidebar";
import ChatInputArea from "./chat/ChatInputArea";

function generateSessionId() {
  return "sess_" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

function buildReadinessSummary(diagnostics: SystemDiagnostics | null): {
  status: string;
  tone: "ready" | "warning" | "error";
  detail: string;
  betaReady?: boolean;
} {
  if (!diagnostics) {
    return {
      status: "检测中",
      tone: "warning",
      detail: "正在读取本地模型、云端 API 和人生模型状态。",
    };
  }
  if (diagnostics.chat_ready) {
    const backend = diagnostics.ollama_online
      ? `本地模型 ${diagnostics.resolved_local_model || diagnostics.local_model}`
      : "云端模型";
    return {
      status: "聊天就绪",
      tone: "ready",
      detail: `当前可使用 ${backend}。`,
      betaReady: diagnostics.beta_ready,
    };
  }
  if (!diagnostics.ollama_online && !diagnostics.cloud_api_configured) {
    return {
      status: "需要配置",
      tone: "error",
      detail: "本地模型离线，云端 API 也未配置。无法开始聊天。",
    };
  }
  if (!diagnostics.ollama_online) {
    return {
      status: "本地模型离线",
      tone: "warning",
      detail: `未检测到 ${diagnostics.local_model}，将依赖云端 API。`,
    };
  }
  if (!diagnostics.cloud_api_configured) {
    return { status: "云端 API 未配置", tone: "warning", detail: "复杂任务可能只能使用本地模型。" };
  }
  return { status: "需要检查", tone: "warning", detail: "部分运行状态异常，请查看设置页诊断。" };
}

function getFixSuggestion(
  diagnostics: SystemDiagnostics | null
): { text: string; action: string; link: string } | null {
  if (!diagnostics) return null;
  if (!diagnostics.ollama_online && !diagnostics.cloud_api_configured) {
    return {
      text: "没有可用的模型后端。",
      action: "去设置页配置",
      link: "/settings",
    };
  }
  if (!diagnostics.life_model_ready) {
    return {
      text: "人生模型读取失败。",
      action: "去构建人生模型",
      link: "/builder",
    };
  }
  if (diagnostics.model_empty) {
    if ((diagnostics.pending_builder_review_sessions ?? 0) > 0) {
      return {
        text: `人生模型还没有真正写入，但你有 ${diagnostics.pending_builder_review_sessions} 个待确认的 Builder Review。`,
        action: "回 Builder 审阅",
        link: "/builder",
      };
    }
    if (diagnostics.unfinished_builder_sessions > 0) {
      return {
        text: `人生模型还没有真正写入，但你有 ${diagnostics.unfinished_builder_sessions} 个待继续的构建会话。`,
        action: "回 Builder 继续",
        link: "/builder",
      };
    }
    return {
      text: "人生模型尚未构建。",
      action: "去 Builder 创建",
      link: "/builder",
    };
  }
  if (!diagnostics.ollama_online && diagnostics.prefer_local_model) {
    return {
      text: `优先本地模型设置开启，但 ${diagnostics.local_model} 未运行。`,
      action: "切换云端模型",
      link: "/settings",
    };
  }
  return null;
}

function formatChatRuntimeError(error: unknown, diagnostics: SystemDiagnostics | null): string {
  if (diagnostics && !diagnostics.chat_ready && diagnostics.readiness_issues?.length) {
    return `暂时无法发送普通对话：\n${diagnostics.readiness_issues.map(issue => `- ${issue}`).join("\n")}\n\n请去设置页查看“试用就绪检查”。`;
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
  return `${hint}\n\n请去设置页查看“试用就绪检查”。`;
}

export default function ChatPage() {
  const location = useLocation();
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string>("default");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(true);
  const [preferLocal, setPreferLocal] = useState<boolean>(true);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [reasoningTrace, setReasoningTrace] = useState<ReasoningTrace | null>(null);
  const [showReasoningTrace, setShowReasoningTrace] = useState(false);
  const [toolCalls, setToolCalls] = useState<ToolCallResult[]>([]);
  const [showToolCalls, setShowToolCalls] = useState(false);
  const [, setCurrentRunId] = useState<string | null>(null);
  const [model, setModel] = useState<LifeModel | null>(null);
  const [showGuide, setShowGuide] = useState(true);
  const [chatMode, setChatMode] = useState<string | null>(null);
  const [streamingReply, setStreamingReply] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const [streamInterrupted, setStreamInterrupted] = useState(false);
  const [currentRun, setCurrentRun] = useState<AgentRun | null>(null);

  // Throttle streaming updates to reduce React re-render pressure
  const streamingBufferRef = useRef("");
  const streamingRafRef = useRef<number | null>(null);
  const diagnosticsRef = useRef<SystemDiagnostics | null>(null);
  const streamErrorHandledRef = useRef(false);
  const lastUserMessageRef = useRef<ChatMessage | null>(null);
  const currentSessionIdRef = useRef<string>(currentSessionId);

  useEffect(() => {
    currentSessionIdRef.current = currentSessionId;
  }, [currentSessionId]);

  const refreshAgentRuns = async (sessionId = currentSessionIdRef.current) => {
    try {
      const runs = await listAgentRunsForSession(sessionId, 10);
      if (currentSessionIdRef.current === sessionId) {
        setCurrentRun(runs[0] ?? null);
      }
    } catch {
      if (currentSessionIdRef.current === sessionId) {
        setCurrentRun(null);
      }
    }
  };

  const loadAgentRunForSession = async (runId: string | undefined, sessionId: string) => {
    if (!runId) return;
    try {
      const run = await getAgentRun(runId);
      if (currentSessionIdRef.current === sessionId) {
        setCurrentRun(run);
      }
    } catch {
      if (currentSessionIdRef.current === sessionId) {
        setCurrentRun(null);
      }
    }
  };

  const flushStreaming = () => {
    if (streamingRafRef.current !== null) {
      cancelAnimationFrame(streamingRafRef.current);
      streamingRafRef.current = null;
    }
    if (streamingBufferRef.current) {
      setStreamingReply(prev => prev + streamingBufferRef.current);
      streamingBufferRef.current = "";
    }
  };

  const scheduleFlushStreaming = () => {
    if (streamingRafRef.current !== null) return;
    streamingRafRef.current = requestAnimationFrame(() => {
      streamingRafRef.current = null;
      if (streamingBufferRef.current) {
        setStreamingReply(prev => prev + streamingBufferRef.current);
        streamingBufferRef.current = "";
      }
    });
  };

  useEffect(() => {
    diagnosticsRef.current = diagnostics;
  }, [diagnostics]);

  useEffect(() => {
    (async () => {
      try {
        const [diag, cfg] = await Promise.all([getSystemDiagnostics(), getSchedulerConfig()]);
        setDiagnostics(diag);
        setPreferLocal(cfg.preferLocal);
      } catch {
        // silently ignore
      }
    })();
  }, []);

  useEffect(() => {
    getLifeModel()
      .then(setModel)
      .catch(() => {});
  }, []);

  useEffect(() => {
    const refreshChatContext = () => {
      getLifeModel()
        .then(setModel)
        .catch(() => {});
      getSystemDiagnostics()
        .then(setDiagnostics)
        .catch(() => {});
      loadSessions(currentSessionIdRef.current);
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        refreshChatContext();
      }
    };
    window.addEventListener("focus", refreshChatContext);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("focus", refreshChatContext);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    if (!(location.state as { refreshFromBuilder?: boolean } | null)?.refreshFromBuilder) return;
    getLifeModel()
      .then(setModel)
      .catch(() => {});
    getSystemDiagnostics()
      .then(setDiagnostics)
      .catch(() => {});
    loadSessions();
  }, [location.state]);

  const loadSessions = async (activeSessionId = currentSessionIdRef.current) => {
    try {
      const list = await listChatSessions();
      setSessions(list);
      if (list.length > 0 && !list.find(s => s.session_id === activeSessionId)) {
        setCurrentSessionId(list[0].session_id);
      }
    } catch (e) {
      console.error("加载会话列表失败", e);
    }
  };

  useEffect(() => {
    loadSessions();
  }, []);

  useEffect(() => {
    setLoadingHistory(true);
    refreshAgentRuns(currentSessionId);
    getChatHistory(currentSessionId)
      .then(history => {
        if (history.length === 0) {
          setMessages([
            {
              role: "assistant",
              content:
                "你好，我是 OpenLife。我已经加载了你的人生模型，随时可以从你的价值观和目标出发进行交流。",
            },
          ]);
        } else {
          setMessages(history);
        }
      })
      .catch(e => {
        console.error("加载历史消息失败", e);
        setMessages([
          {
            role: "assistant",
            content:
              "你好，我是 OpenLife。我已经加载了你的人生模型，随时可以从你的价值观和目标出发进行交流。",
          },
        ]);
      })
      .finally(() => setLoadingHistory(false));
  }, [currentSessionId]);

  // Scroll on significant changes only; avoid smooth scroll during streaming
  useEffect(() => {
    if (bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: sending ? "auto" : "smooth" });
    }
  }, [messages.length, sending]);

  // Register stream listeners once per session to avoid leaks
  useEffect(() => {
    let unlistenStart: (() => void) | null = null;
    let unlistenChunk: (() => void) | null = null;
    let unlistenDone: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    (async () => {
      unlistenStart = await listen<StreamMessageStartPayload>(
        "stream-message-start",
        async event => {
          if (event.payload.session_id === currentSessionId) {
            setReasoningTrace(event.payload.reasoning_trace ?? null);
            setCurrentRunId(event.payload.run_id);
            setToolCalls(
              (event.payload.tool_calls ?? []).map(call => ({
                ...call,
                run_id: event.payload.run_id,
              }))
            );
            await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
          }
        }
      );
      unlistenChunk = await listen<{ session_id: string; chunk: string }>(
        "stream-message-chunk",
        event => {
          if (event.payload.session_id === currentSessionId) {
            streamingBufferRef.current += event.payload.chunk;
            scheduleFlushStreaming();
          }
        }
      );
      unlistenDone = await listen<StreamMessageDonePayload>("stream-message-done", async event => {
        if (event.payload.session_id === currentSessionId) {
          flushStreaming();
          setMessages(prev => [...prev, { role: "assistant", content: event.payload.reply }]);
          setStreamingReply("");
          setSending(false);
          setReasoningTrace(event.payload.reasoning_trace ?? null);
          setCurrentRunId(event.payload.run_id);
          setToolCalls(
            (event.payload.tool_calls ?? []).map(call => ({
              ...call,
              run_id: event.payload.run_id,
            }))
          );
          setStreamInterrupted(false);
          await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
          refreshAgentRuns(event.payload.session_id);
          logAnalyticsEvent("send_message", currentSessionId, undefined).catch(() => {});
        }
      });
      unlistenError = await listen<{ session_id: string; run_id?: string; error: string }>(
        "stream-message-error",
        async event => {
          if (event.payload.session_id === currentSessionId) {
            flushStreaming();
            setMessages(prev => [
              ...prev,
              {
                role: "assistant",
                content: formatChatRuntimeError(event.payload.error, diagnosticsRef.current),
              },
            ]);
            streamErrorHandledRef.current = true;
            setStreamingReply("");
            setSending(false);
            setStreamInterrupted(true);
            await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
            refreshAgentRuns(event.payload.session_id);
          }
        }
      );
    })();

    return () => {
      flushStreaming();
      if (unlistenStart) unlistenStart();
      if (unlistenChunk) unlistenChunk();
      if (unlistenDone) unlistenDone();
      if (unlistenError) unlistenError();
    };
  }, [currentSessionId]);

  const togglePreferLocal = async () => {
    const next = !preferLocal;
    setPreferLocal(next);
    try {
      const cfg = await getSchedulerConfig();
      await setSchedulerConfig(cfg.localModel, next);
      getSystemDiagnostics()
        .then(setDiagnostics)
        .catch(() => {});
    } catch (e) {
      console.error(e);
    }
  };

  const handleNewSession = async () => {
    const id = generateSessionId();
    try {
      await createChatSession(id, "新会话");
      await loadSessions();
      setCurrentSessionId(id);
    } catch (e) {
      console.error("创建会话失败", e);
    }
  };

  const handleDeleteSession = async (id: string) => {
    try {
      await deleteChatSession(id);
      const list = await listChatSessions();
      setSessions(list);
      if (currentSessionIdRef.current === id) {
        setCurrentSessionId(list.length > 0 ? list[0].session_id : "default");
      }
    } catch (e) {
      console.error("删除会话失败", e);
    }
  };

  const startEditTitle = (s: ChatSession) => {
    setEditingId(s.session_id);
    setEditingTitle(s.title);
  };

  const commitEditTitle = async () => {
    if (!editingId) return;
    try {
      await renameChatSession(editingId, editingTitle.trim() || "未命名");
      await loadSessions();
    } catch (e) {
      console.error("重命名失败", e);
    } finally {
      setEditingId(null);
      setEditingTitle("");
    }
  };

  const handleExecuteToolCall = async (index: number) => {
    const call = toolCalls[index];
    if (!call?.requires_confirmation) return;
    if (!call.run_id || !call.action_id) {
      console.error("Tool call missing run_id or action_id");
      return;
    }
    try {
      const result = await replayAgentAction(call.run_id, call.action_id);
      setToolCalls(prev =>
        prev.map((item, idx) =>
          idx === index
            ? {
                ...item,
                success: result.status === "succeeded",
                status: result.status as any,
                requires_confirmation: false,
                output: result.output?.text,
              }
            : item
        )
      );
    } catch (e) {
      setToolCalls(prev =>
        prev.map((item, idx) =>
          idx === index
            ? {
                ...item,
                success: false,
                error: String(e),
                status: "error",
                requires_confirmation: false,
              }
            : item
        )
      );
    }
  };

  const tryHandleQuickCommand = async (text: string): Promise<string | null> => {
    const t = text.trim();
    if (t.startsWith("/goal")) {
      try {
        const goals = await getDailyGoals();
        const renderGoals = (items: typeof goals) => {
          const completed = items.filter(g => g.done).length;
          const list =
            items.map((g, i) => `${i + 1}. ${g.done ? "[x]" : "[ ]"} ${g.name}`).join("\n") ||
            "暂无今日目标。";
          return `📋 今日目标 (${completed}/${items.length} 完成)：\n\n${list}`;
        };
        const findGoalIndex = (query: string) => {
          const normalized = query.trim().toLowerCase();
          return goals.findIndex(goal => {
            const name = goal.name.toLowerCase();
            return name === normalized || name.includes(normalized) || normalized.includes(name);
          });
        };
        const command = t.replace("/goal", "").trim();
        if (!command) {
          return renderGoals(goals);
        }
        if (command === "help") {
          return [
            "📌 /goal 用法：",
            "/goal",
            "/goal add 目标名",
            "/goal done 目标名",
            "/goal undo 目标名",
          ].join("\n");
        }
        if (command.startsWith("add ")) {
          const goalName = command.slice(4).trim();
          if (!goalName) return "请在 /goal add 后面补充目标名称。";
          await addDailyGoal(goalName);
          return `✅ 已添加今日目标：${goalName}`;
        }
        if (command.startsWith("done ") || command.startsWith("finish ")) {
          const query = command.replace(/^done\s+|^finish\s+/, "").trim();
          const idx = findGoalIndex(query);
          if (idx < 0) return `没有找到名为“${query}”的今日目标。`;
          if (!goals[idx].done) {
            await toggleDailyGoal(idx);
          }
          const refreshed = await getDailyGoals();
          return `✅ 已完成今日目标：${refreshed[idx]?.name ?? query}\n\n${renderGoals(refreshed)}`;
        }
        if (command.startsWith("undo ")) {
          const query = command.slice(5).trim();
          const idx = findGoalIndex(query);
          if (idx < 0) return `没有找到名为“${query}”的今日目标。`;
          if (goals[idx].done) {
            await toggleDailyGoal(idx);
          }
          const refreshed = await getDailyGoals();
          return `↩️ 已恢复今日目标：${refreshed[idx]?.name ?? query}\n\n${renderGoals(refreshed)}`;
        }
        return "无法识别 /goal 子命令。输入 `/goal help` 查看可用操作。";
      } catch {
        return "获取今日目标失败。";
      }
    }
    if (t.startsWith("/state")) {
      const rest = t.replace("/state", "").trim();
      if (!rest) {
        return "📝 用法：/state 维度名 数值 单位\n示例：/state 专注度 7.5 分";
      }
      const parts = rest.split(/\s+/);
      if (parts.length < 2) {
        return "格式不正确。用法：/state 维度名 数值 单位";
      }
      const name = parts[0];
      const val = parseFloat(parts[1]);
      if (Number.isNaN(val)) {
        return "数值无法解析，请检查输入。";
      }
      const unit = parts[2] || "单位";
      try {
        await recordState(name, val, unit, undefined, undefined, undefined, undefined);
        return `✅ 已记录状态：${name} = ${val} ${unit}`;
      } catch {
        return "记录状态失败。";
      }
    }
    return null;
  };

  const handleSend = async () => {
    if (!input.trim() || sending) return;
    if (!currentSessionId || typeof currentSessionId !== "string") {
      setMessages(prev => [
        ...prev,
        { role: "assistant", content: "错误: 当前会话 ID 无效，请刷新页面或切换会话后重试。" },
      ]);
      return;
    }
    const text = input.trim();
    const userMsg: ChatMessage = { role: "user", content: text };
    const nextMessages = [...messages, userMsg];
    lastUserMessageRef.current = userMsg;
    setMessages(nextMessages);
    setInput("");

    const quickReply = await tryHandleQuickCommand(text);
    if (quickReply) {
      const assistantMsg: ChatMessage = { role: "assistant", content: quickReply };
      try {
        await saveChatMessage(currentSessionId, userMsg);
        await saveChatMessage(currentSessionId, assistantMsg);
        await loadSessions();
      } catch (e) {
        console.error("保存快捷指令消息失败", e);
      }
      setMessages([...nextMessages, assistantMsg]);
      return;
    }

    if (diagnostics && !diagnostics.chat_ready) {
      const assistantMsg: ChatMessage = {
        role: "assistant",
        content: formatChatRuntimeError("chat not ready", diagnostics),
      };
      setMessages([...nextMessages, assistantMsg]);
      return;
    }

    setSending(true);
    streamErrorHandledRef.current = false;
    setStreamInterrupted(false);
    setStreamingReply("");
    streamingBufferRef.current = "";
    setReasoningTrace(null);
    setToolCalls([]);
    setShowToolCalls(false);

    try {
      // The streaming backend persists the user message before model execution.
      // Saving it here as well creates duplicate user rows in history and memory retrieval.
      await startStreamMessage(currentSessionId, nextMessages);
      await loadSessions();
    } catch (e) {
      flushStreaming();
      if (!streamErrorHandledRef.current) {
        setMessages(prev => [
          ...prev,
          { role: "assistant", content: formatChatRuntimeError(e, diagnosticsRef.current) },
        ]);
      }
      setStreamingReply("");
      setSending(false);
    }
  };

  const retryLastUserMessage = () => {
    const last =
      lastUserMessageRef.current ?? [...messages].reverse().find(m => m.role === "user") ?? null;
    if (!last || sending) return;
    setInput(last.content);
  };

  const handleContinueStream = async () => {
    const lastUser =
      lastUserMessageRef.current ?? [...messages].reverse().find(m => m.role === "user") ?? null;
    if (!lastUser || sending) return;
    const lastUserIndex = messages.map(m => m.role).lastIndexOf("user");
    const retryMessages = lastUserIndex >= 0 ? messages.slice(0, lastUserIndex + 1) : [lastUser];
    setStreamInterrupted(false);
    setSending(true);
    streamErrorHandledRef.current = false;
    setStreamingReply("");
    streamingBufferRef.current = "";
    setReasoningTrace(null);
    setToolCalls([]);
    setShowToolCalls(false);
    try {
      await startStreamMessage(currentSessionId, retryMessages);
    } catch (e) {
      flushStreaming();
      if (!streamErrorHandledRef.current) {
        setMessages(prev => [
          ...prev,
          { role: "assistant", content: formatChatRuntimeError(e, diagnosticsRef.current) },
        ]);
        setStreamInterrupted(true);
      }
      setStreamingReply("");
      setSending(false);
    }
  };

  const readiness = buildReadinessSummary(diagnostics);
  const readinessClass =
    readiness.tone === "ready"
      ? "bg-emerald-50 border-emerald-100 text-emerald-800"
      : readiness.tone === "error"
        ? "bg-rose-50 border-rose-100 text-rose-800"
        : "bg-amber-50 border-amber-100 text-amber-800";

  const conversationStarters = [
    {
      title: "今日规划",
      detail: "把今天切成 3 个可完成的小闭环。",
      prompt:
        "请基于我的人生模型和当前状态，帮我规划今天最值得完成的 3 件事，并给出一个低阻力开场步骤。",
    },
    {
      title: "情绪复盘",
      detail: "整理最近的压力、能量和卡点。",
      prompt: "我想做一次情绪和状态复盘。请用温和的问题帮我看清最近压力、能量和真正卡住我的地方。",
    },
    {
      title: "目标拆解",
      detail: "把一个目标拆成下一步行动。",
      prompt:
        "请帮我拆解一个当前目标：先问我目标是什么，然后把它拆成可执行的里程碑和今天能做的一步。",
    },
    {
      title: "决策陪跑",
      detail: "用价值观和长期目标辅助选择。",
      prompt:
        "我现在有一个选择需要判断。请基于我的价值观、长期目标和当前状态，帮我做一次决策陪跑。",
    },
  ];

  const allGoals = model
    ? [
        ...model.goals.short_term,
        ...model.goals.medium_term,
        ...model.goals.long_term,
        ...model.goals.life_goals,
      ]
    : [];
  const primaryGoal = allGoals.find(goal => goal.status !== "completed") ?? allGoals[0];
  const topValues = model
    ? [...model.identity.values].sort((a, b) => b.weight - a.weight).slice(0, 3)
    : [];
  const modelPulse = [
    {
      label: "身份",
      value: model?.identity.name || model?.identity.role_definition.primary_role || "尚未明确",
    },
    {
      label: "使命",
      value: model?.identity.mission_statement || model?.identity.life_philosophy || "等待构建",
    },
    {
      label: "当前重心",
      value: model?.state.current_focus || "尚未记录",
    },
    {
      label: "首要目标",
      value: primaryGoal?.name || "尚未设定",
    },
  ];
  const conversationContext = [
    {
      label: "价值观过滤",
      detail:
        topValues.length > 0
          ? `优先参考：${topValues.map(value => value.name).join("、")}`
          : "当前还没有足够价值观信号，建议先完成一次构建。",
    },
    {
      label: "当前状态",
      detail: model?.state.current_focus
        ? `会优先围绕“${model.state.current_focus}”来组织建议。`
        : "当前焦点还比较空，这轮对话会更多依赖你的即时输入。",
    },
    {
      label: "目标牵引",
      detail: primaryGoal?.name
        ? `会优先结合当前目标：${primaryGoal.name}`
        : "目标还不够清晰，更适合先做探索型对话。",
    },
  ];

  const fillPrompt = (prompt: string) => {
    setInput(prompt);
  };

  const selectChatMode = (mode: string) => {
    setChatMode(mode);
    const found = chatModes.find(m => m.key === mode);
    if (found) {
      if (mode === "free") {
        setInput("");
      } else {
        setInput(found.prompt);
      }
    }
  };

  const chatModes = [
    {
      key: "today",
      label: "今日规划",
      icon: <Sparkles size={14} />,
      prompt: conversationStarters[0].prompt,
    },
    {
      key: "emotion",
      label: "情绪复盘",
      icon: <Heart size={14} />,
      prompt: conversationStarters[1].prompt,
    },
    {
      key: "goal",
      label: "目标拆解",
      icon: <Target size={14} />,
      prompt: conversationStarters[2].prompt,
    },
    {
      key: "decision",
      label: "决策陪跑",
      icon: <Compass size={14} />,
      prompt: conversationStarters[3].prompt,
    },
    { key: "free", label: "自由聊天", icon: <MessageSquare size={14} />, prompt: "" },
  ];

  const handleSaveAsDailyGoal = async (content: string) => {
    const name = content
      .split(/[。！？\n]/)[0]
      .slice(0, 30)
      .trim();
    if (!name) return;
    try {
      await addDailyGoal(name);
    } catch (e) {
      console.error("保存今日目标失败", e);
    }
  };

  const handleIndexMemory = async (content: string) => {
    if (isSafeMode(diagnostics)) {
      setMessages(prev => [
        ...prev,
        {
          role: "assistant",
          content: `当前处于 Safe Mode，${getSafeModeReason(diagnostics)} 建议先去设置页恢复控制台处理数据风险，再执行“加入记忆”。`,
        },
      ]);
      return;
    }
    try {
      await indexMemoryChunk(currentSessionId, content, "chat");
    } catch (e) {
      console.error("加入记忆失败", e);
    }
  };

  const buildAssistantActionPrompt = (
    kind: "continue" | "action" | "state" | "goal",
    content: string
  ) => {
    if (kind === "continue") {
      return `请继续围绕上一条回复展开，但更具体一点：${content.slice(0, 240)}`;
    }
    if (kind === "action") {
      return `请把上一条回复提炼成今天可以执行的 3 个行动，每个行动都要足够小，并说明第一步。`;
    }
    if (kind === "state") {
      return `请根据上一条对话，帮我总结当前状态：情绪、精力、压力、注意力分别是什么，并给出适合用 /state 记录的建议。`;
    }
    return `请把上一条回复拆成一个目标结构：目标名、为什么重要、里程碑、今天可以做的一步、可能风险。`;
  };

  const handleFeedback = async (index: number, type: "up" | "down") => {
    const msg = messages[index];
    if (!msg || msg.role !== "assistant") return;
    try {
      await saveFeedback(currentSessionId, index, type, msg.content.slice(0, 200));
    } catch (e) {
      console.error("反馈保存失败", e);
    }
  };

  return (
    <div className="h-full flex bg-white">
      <ChatSidebar
        sessions={sessions}
        currentSessionId={currentSessionId}
        editingId={editingId}
        editingTitle={editingTitle}
        onSelectSession={setCurrentSessionId}
        onNewSession={handleNewSession}
        onStartEditTitle={startEditTitle}
        onCommitEditTitle={commitEditTitle}
        onCancelEditTitle={() => {
          setEditingId(null);
          setEditingTitle("");
        }}
        onEditTitleChange={setEditingTitle}
        onDeleteSession={handleDeleteSession}
      />

      {/* Chat area */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="border-b px-6 py-2 flex items-center justify-between bg-gray-50 gap-3">
          <div className={`text-sm border rounded-lg px-3 py-2 flex-1 ${readinessClass}`}>
            <div className="flex items-center gap-2">
              <span className="font-medium">{readiness.status}</span>
              {readiness.betaReady === true && (
                <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-100 text-blue-700 font-medium">
                  Beta 就绪
                </span>
              )}
              {readiness.betaReady === false && readiness.tone === "ready" && (
                <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-amber-100 text-amber-700 font-medium">
                  Beta 待完善
                </span>
              )}
            </div>
            <div className="text-xs mt-0.5">
              {readiness.detail}
              {diagnostics && (
                <span className="ml-2">
                  本地：{diagnostics.resolved_local_model || diagnostics.local_model} · 云端 API：
                  {diagnostics.cloud_api_configured ? "已配置" : "未配置"}
                </span>
              )}
              <Link to="/settings" className="ml-2 underline font-medium">
                去设置页检查
              </Link>
            </div>
          </div>
          <button
            onClick={togglePreferLocal}
            className={`text-xs px-3 py-1 rounded-full border transition ${
              preferLocal
                ? "bg-indigo-50 border-indigo-200 text-indigo-700"
                : "bg-white border-gray-200 text-gray-600"
            }`}
          >
            {preferLocal ? "优先本地模型" : "优先云端模型"}
          </button>
        </div>
        {diagnostics && isSafeMode(diagnostics) && (
          <div className="border-b border-amber-200 bg-amber-50 px-6 py-2">
            <div className="max-w-3xl text-xs text-amber-800 flex flex-wrap items-center justify-between gap-2">
              <div>
                <span className="font-medium">Safe Mode：</span>
                {getSafeModeReason(diagnostics)}
                <span className="ml-2">普通对话仍可继续，但“加入记忆”等写入操作建议先暂停。</span>
              </div>
              <Link to="/settings" className="underline font-medium">
                打开恢复控制台
              </Link>
            </div>
          </div>
        )}
        {/* Chat mode selector */}
        <div className="border-b px-6 py-2 bg-white">
          <div className="flex items-center gap-2 overflow-x-auto">
            {chatModes.map(m => (
              <button
                key={m.key}
                onClick={() => selectChatMode(m.key)}
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium transition whitespace-nowrap ${
                  chatMode === m.key
                    ? "bg-indigo-600 text-white"
                    : "bg-gray-100 text-gray-600 hover:bg-gray-200"
                }`}
              >
                {m.icon}
                {m.label}
              </button>
            ))}
          </div>
        </div>
        <div className="flex-1 overflow-auto px-6 py-4 space-y-4">
          {!loadingHistory && (
            <div className="rounded-3xl border border-stone-200 bg-[#fbf7ef] p-5 shadow-sm">
              <div className="flex flex-col gap-4 xl:flex-row xl:items-stretch">
                <div className="flex-1">
                  <div className="flex items-center gap-2 text-sm font-semibold text-stone-900">
                    <Sparkles size={16} className="text-amber-600" />
                    陪跑现场
                  </div>
                  <p className="mt-1 text-xs leading-5 text-stone-500">
                    OpenLife
                    会优先参考你的人生模型来回答。你可以直接选择一个场景开始，也可以自由输入。
                  </p>
                  <div className="mt-4 grid gap-2 sm:grid-cols-2">
                    {modelPulse.map(item => (
                      <div
                        key={item.label}
                        className="rounded-2xl border border-white bg-white/75 px-3 py-2"
                      >
                        <div className="text-[11px] font-medium text-stone-400">{item.label}</div>
                        <div className="mt-1 line-clamp-2 text-sm font-medium text-stone-800">
                          {item.value}
                        </div>
                      </div>
                    ))}
                  </div>
                  {topValues.length > 0 ? (
                    <div className="mt-3 flex flex-wrap gap-2">
                      {topValues.map(value => (
                        <span
                          key={value.name}
                          className="inline-flex items-center gap-1 rounded-full border border-emerald-100 bg-emerald-50 px-2.5 py-1 text-[11px] font-medium text-emerald-700"
                        >
                          <Heart size={11} />
                          {value.name}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <div className="mt-3 rounded-2xl border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                      人生模型还比较空，建议先完成一次构建，这样对话会更像“懂你的人”。
                      <Link to="/builder" className="ml-2 font-semibold underline">
                        去构建
                      </Link>
                    </div>
                  )}
                </div>
                <div className="w-full xl:w-[360px]">
                  <div className="flex items-center gap-2 text-sm font-semibold text-stone-900">
                    <Compass size={16} className="text-stone-600" />
                    选择陪跑模式
                  </div>
                  <div className="mt-3 grid gap-2">
                    {conversationStarters.map(starter => (
                      <button
                        key={starter.title}
                        onClick={() => fillPrompt(starter.prompt)}
                        className="group rounded-2xl border border-white bg-white/80 px-3 py-2.5 text-left transition hover:-translate-y-0.5 hover:border-stone-300 hover:shadow-sm"
                      >
                        <div className="flex items-center justify-between gap-3">
                          <div className="text-sm font-medium text-stone-900">{starter.title}</div>
                          <ArrowRight
                            size={14}
                            className="text-stone-300 transition group-hover:translate-x-0.5 group-hover:text-stone-600"
                          />
                        </div>
                        <div className="mt-1 text-xs leading-5 text-stone-500">
                          {starter.detail}
                        </div>
                      </button>
                    ))}
                  </div>
                  <div className="mt-4 rounded-2xl border border-stone-200 bg-white/75 p-3">
                    <div className="text-xs font-semibold text-stone-900">这轮对话会优先参考</div>
                    <div className="mt-2 space-y-2">
                      {conversationContext.map(item => (
                        <div
                          key={item.label}
                          className="rounded-xl border border-stone-100 bg-stone-50/80 px-3 py-2"
                        >
                          <div className="text-[11px] font-medium text-stone-500">{item.label}</div>
                          <div className="mt-1 text-xs leading-5 text-stone-700">{item.detail}</div>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
          {reasoningTrace && (
            <div className="flex justify-start">
              <ReasoningTracePanel
                trace={reasoningTrace}
                show={showReasoningTrace}
                onToggle={() => setShowReasoningTrace(s => !s)}
              />
            </div>
          )}
          {toolCalls.length > 0 && (
            <div className="flex justify-start">
              <div className="max-w-2xl px-4 py-3 rounded-xl text-sm bg-gray-50 text-gray-900 border border-gray-200 w-full">
                <button
                  onClick={() => setShowToolCalls(s => !s)}
                  className="flex items-center gap-2 font-medium mb-2"
                >
                  <Hammer size={16} /> 工具调用 {showToolCalls ? "▲" : "▼"}
                </button>
                {toolCalls.some(c => c.permission_level === "high") && (
                  <div className="mb-3 rounded-md bg-orange-50 border border-orange-100 p-2 text-xs text-orange-700 flex items-center gap-2">
                    <span className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-orange-200 text-orange-700 font-bold">
                      !
                    </span>
                    检测到高风险 MCP 操作，请在下方的卡片中逐条确认后再查看结果。
                  </div>
                )}
                {showToolCalls && (
                  <div className="space-y-2">
                    {toolCalls.map((call, idx) => (
                      <ToolCallCard
                        key={idx}
                        call={call}
                        onExecute={() => handleExecuteToolCall(idx)}
                      />
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}
          {!loadingHistory && messages.length <= 1 && !sending && (
            <div className="flex justify-start">
              <div className="max-w-3xl w-full rounded-2xl border border-stone-200 bg-[#fbf7ef] p-5 shadow-sm">
                <div className="text-sm font-semibold text-stone-900">不知道从哪一句开始？</div>
                <div className="mt-1 text-xs text-stone-500">
                  {getModelEmptyState(model, diagnostics)
                    ? "你还没有完成人生模型构建。下面这些问题可以先体验通用对话，但完成构建后会明显更贴近你。"
                    : "选择一个陪跑场景，OpenLife 会按你的人生模型展开对话。"}
                </div>
                <div className="mt-4 grid gap-3 sm:grid-cols-2">
                  {conversationStarters.map(starter => (
                    <button
                      key={starter.title}
                      onClick={() => setInput(starter.prompt)}
                      className="rounded-xl border border-white bg-white/80 p-4 text-left transition hover:-translate-y-0.5 hover:border-stone-300 hover:shadow-sm"
                    >
                      <div className="text-sm font-medium text-stone-900">{starter.title}</div>
                      <div className="mt-1 text-xs leading-5 text-stone-500">{starter.detail}</div>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}
          {showGuide && getModelEmptyState(model, diagnostics) && (
            <div className="flex justify-start">
              <div className="max-w-2xl w-full bg-gradient-to-r from-indigo-50 to-purple-50 border border-indigo-100 rounded-xl p-4 text-sm relative">
                <button
                  onClick={() => setShowGuide(false)}
                  className="absolute top-2 right-2 text-indigo-400 hover:text-indigo-600"
                  title="关闭"
                >
                  <X size={16} />
                </button>
                <div className="flex items-center gap-2 font-semibold text-indigo-900 mb-1">
                  <Hammer size={16} className="text-indigo-600" />
                  先建立你的人生模型
                </div>
                <p className="text-indigo-800 mb-3">
                  OpenLife 的回答会基于你的人生模型进行价值观过滤。模型越完整，对话越贴心。
                </p>
                <div className="flex flex-wrap gap-2">
                  <Link
                    to="/builder"
                    className="inline-flex items-center gap-1 bg-indigo-600 text-white px-3 py-1.5 rounded-md text-xs hover:bg-indigo-700"
                  >
                    去构建 <ArrowRight size={14} />
                  </Link>
                  <Link
                    to="/dashboard"
                    className="inline-flex items-center gap-1 border border-indigo-200 bg-white text-indigo-700 px-3 py-1.5 rounded-md text-xs hover:bg-indigo-50"
                  >
                    先看仪表盘 <ArrowRight size={14} />
                  </Link>
                </div>
                <div className="mt-2 text-xs text-indigo-600">
                  如果你只是想先感受一下，也可以直接使用下面的场景卡开始一次通用对话。
                </div>
              </div>
            </div>
          )}

          {loadingHistory && (
            <div className="flex justify-start">
              <LoadingSpinner text="正在加载历史消息..." />
            </div>
          )}
          {messages.map((m, i) => (
            <div key={i} className={`flex ${m.role === "user" ? "justify-end" : "justify-start"}`}>
              <div
                className={`max-w-2xl px-4 py-3 rounded-xl text-sm ${
                  m.role === "user"
                    ? "bg-indigo-600 text-white rounded-br-none"
                    : "bg-gray-100 text-gray-800 rounded-bl-none"
                }`}
              >
                <div className="whitespace-pre-wrap">{m.content}</div>
                {m.role === "assistant" && (
                  <div className="mt-3 space-y-2">
                    <div className="flex flex-wrap gap-2">
                      <button
                        onClick={() =>
                          fillPrompt(buildAssistantActionPrompt("continue", m.content))
                        }
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                      >
                        <MessageSquare size={12} /> 继续追问
                      </button>
                      <button
                        onClick={() => fillPrompt(buildAssistantActionPrompt("action", m.content))}
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                      >
                        <CheckCircle2 size={12} /> 提炼行动
                      </button>
                      <button
                        onClick={() => fillPrompt(buildAssistantActionPrompt("state", m.content))}
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                      >
                        <Activity size={12} /> 记录状态
                      </button>
                      <button
                        onClick={() => fillPrompt(buildAssistantActionPrompt("goal", m.content))}
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                      >
                        <Target size={12} /> 拆成目标
                      </button>
                      <button
                        onClick={() => handleSaveAsDailyGoal(m.content)}
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        title="将回复首句保存为今日目标"
                      >
                        <CheckCircle2 size={12} /> 设为今日目标
                      </button>
                      <button
                        onClick={() => handleIndexMemory(m.content)}
                        className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        title="将这条回复加入长期记忆"
                      >
                        <Sparkles size={12} /> 加入记忆
                      </button>
                    </div>
                    <div className="flex items-center justify-end gap-2">
                      <button
                        onClick={() => handleFeedback(i, "up")}
                        className="text-gray-500 hover:text-green-600"
                        title="有帮助"
                      >
                        <ThumbsUp size={14} />
                      </button>
                      <button
                        onClick={() => handleFeedback(i, "down")}
                        className="text-gray-500 hover:text-red-600"
                        title="没帮助"
                      >
                        <ThumbsDown size={14} />
                      </button>
                    </div>
                  </div>
                )}
              </div>
            </div>
          ))}
          {sending && streamingReply && (
            <div className="flex justify-start">
              <div className="max-w-2xl px-4 py-3 rounded-xl text-sm bg-gray-100 text-gray-800 rounded-bl-none">
                <div>{streamingReply}</div>
                <div className="flex items-center gap-2 mt-2 text-gray-400 text-xs">
                  <Loader2 size={14} className="animate-spin" /> 生成中...
                </div>
              </div>
            </div>
          )}
          {sending && !streamingReply && (
            <div className="flex justify-start">
              <div className="bg-gray-100 text-gray-500 px-4 py-3 rounded-xl rounded-bl-none text-sm flex items-center gap-2">
                <Loader2 size={16} className="animate-spin" /> 思考中...
              </div>
            </div>
          )}
          {currentRun && (
            <div className="px-4 py-2">
              <div className="text-xs text-gray-400 space-y-1">
                <div className="flex items-center gap-2">
                  <span
                    className={`inline-block w-2 h-2 rounded-full ${
                      currentRun.status === "completed"
                        ? "bg-green-400"
                        : currentRun.status === "failed"
                          ? "bg-red-400"
                          : currentRun.status === "running"
                            ? "bg-yellow-400"
                            : "bg-gray-400"
                    }`}
                  />
                  <span>
                    Run {currentRun.status} · {currentRun.modelRoute?.provider || "unknown"} ·{" "}
                    {currentRun.modelRoute?.model || "unknown"}
                    {currentRun.error && ` · Error: ${currentRun.error.phase}`}
                  </span>
                </div>
                {currentRun.contextSummary && (
                  <div className="text-gray-500">
                    Memory: {currentRun.contextSummary.memoryHitCount} hits · LifeModel:{" "}
                    {currentRun.contextSummary.lifeModelEmpty ? "empty" : "loaded"} · Route:{" "}
                    {currentRun.modelRoute?.reason || "unknown"}
                  </div>
                )}
              </div>
            </div>
          )}
          <div ref={bottomRef} />
        </div>
        <ChatInputArea
          input={input}
          sending={sending}
          streamInterrupted={streamInterrupted}
          diagnostics={diagnostics}
          onInputChange={setInput}
          onSend={handleSend}
          onContinueStream={handleContinueStream}
          onRetryLastMessage={retryLastUserMessage}
          getFixSuggestion={getFixSuggestion}
        />
      </div>
    </div>
  );
}
