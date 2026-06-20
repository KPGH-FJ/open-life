import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
  AlertTriangle,
  Compass,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Sparkles,
  Heart,
  CheckCircle2,
  ShieldCheck,
  XCircle,
  RotateCw,
  Ban,
  Play,
  FileText,
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
  getPendingProposals,
  acceptProposal,
  rejectProposal,
  editProposal,
  draftEditMemoryProposal,
  postponeProposal,
  runMultiStrategyAgentPreview,
  checkControlledChatPilotEligibility,
  recordControlledPilotPromotionEvidence,
  getMainChatAgentTaskState,
  resumeMainChatAgentTask,
  cancelMainChatAgentTask,
  retryMainChatAgentAction,
  listMainChatAgentTasks,
  getMainChatAgentTaskDetail,
  refreshMainChatAgentTaskContext,
  rollbackMemoryAsset,
  listMainChatAgentEvents,
  getMainChatAgentStateSnapshot,
  finalizePlanExecuteSession,
  updatePlanExecuteSessionDraft,
  executePlanExecuteStep,
  skipPlanExecuteStep,
  cancelPlanExecuteSession,
  reviewPlanExecuteSession,
  listMainChatSkills,
  getMainChatSkillDetail,
  selectMainChatSkill,
  clearMainChatSkill,
  listMainChatToolCandidates,
} from "../tauri";
import type {
  AgentRun,
  AgentProposal,
  ChatSession,
  ReasoningTrace,
  StreamMessageDonePayload,
  StreamMessageStartPayload,
  SystemDiagnostics,
  ToolCallResult,
  MainChatAgentIngressDecision,
  MainChatExecutionTranscriptEntry,
  MainChatAgentTaskState,
  MainChatTaskSummary,
  MainChatTaskDetail,
  MainChatAgentStateSnapshot,
  MainChatAgentDurableEvent,
  MainChatSkillSummary,
  MainChatSkillDetail,
  MainChatSelectedSkill,
  MainChatToolCandidateList,
} from "../tauri";
import type {
  ControlledPilotPromotionEvidenceInput,
  ControlledChatPilotEligibilityReport,
  MultiStrategyAgentPreviewLayer,
  MultiStrategyAgentPreviewOutput,
} from "../types";
import { getModelEmptyState } from "../utils/modelEmpty";
import { listen } from "@tauri-apps/api/event";
import ReasoningTracePanel from "../components/ReasoningTracePanel";
import ToolCallCard from "../components/ToolCallCard";
import AgentStateIndicator from "../components/AgentStateIndicator";
import AgentControlPlane from "../components/AgentControlPlane";
import type { AgentStageState } from "../components/AgentStage";
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

function companionRouteStatus(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "状态读取中";
  if (diagnostics.chat_ready && diagnostics.prefer_local_model && diagnostics.ollama_online) {
    return "本地优先";
  }
  if (
    diagnostics.chat_ready &&
    (diagnostics.cloud_api_configured || diagnostics.cloud_api_validated)
  ) {
    return "云端可用";
  }
  return "需要配置";
}

function companionLifeModelStatus(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "Life Model 检查中";
  return diagnostics.life_model_ready && !diagnostics.model_empty
    ? "Life Model 已加载"
    : "Life Model 待构建";
}

function companionPendingStatus(
  diagnostics: SystemDiagnostics | null,
  pendingProposals: AgentProposal[]
): string {
  const count = pendingProposals.length || diagnostics?.pending_proposal_count || 0;
  return count > 0 ? `有信等你回 ${count}` : "没有待回复的信";
}

function companionModelRouteSummary(run: AgentRun | null): string {
  if (!run?.modelRoute) return "模型路线：未读取";
  const route = run.modelRoute;
  const routeLabel =
    route.routeType === "local" || route.preferLocal ? "本地" : route.provider ? "云端" : "未读取";
  return `模型路线：${routeLabel} / ${route.provider || "unknown"} / ${route.model || "unknown"}`;
}

function companionRunSummary(run: AgentRun | null): string[] {
  const context = run?.contextSummary;
  const proposalCount = run?.generatedProposals?.length ?? 0;
  return [
    `使用 Life Model：${context ? (context.lifeModelEmpty ? "否" : "是") : "未读取"}`,
    `参考记忆：${context?.memoryHitCount ?? 0} 条`,
    companionModelRouteSummary(run),
    `产生待确认：${proposalCount > 0 ? "是" : "否"}`,
  ];
}

function recordArrayLength(value: Record<string, unknown> | null | undefined, key: string): number {
  const item = value?.[key];
  return Array.isArray(item) ? item.length : 0;
}

function taskContinuityFinalDeliverySections(
  value: Record<string, unknown> | null | undefined
): string[] {
  if (!value) return [];
  const metadata = value.metadata;
  const metrics =
    metadata && typeof metadata === "object" && !Array.isArray(metadata)
      ? { ...value, ...(metadata as Record<string, unknown>) }
      : value;
  return [
    recordArrayLength(metrics, "completedActions") > 0 ? "Completed actions" : null,
    recordArrayLength(metrics, "observationsUsed") > 0 ? "Sources used" : null,
    recordArrayLength(metrics, "proposalsCreated") > 0 ? "Proposals created" : null,
    recordArrayLength(metrics, "blockers") > 0 ? "Blocked items" : null,
    recordArrayLength(metrics, "skippedWork") > 0 ? "Skipped work" : null,
    recordArrayLength(metrics, "pendingUserActions") > 0 ? "Pending user actions" : null,
    recordArrayLength(metrics, "durableChanges") > 0 ? "Durable changes" : null,
    recordArrayLength(metrics, "nextSteps") > 0 ? "Next steps" : null,
  ].filter((section): section is string => Boolean(section));
}

function isStreamDonePayload(value: unknown): value is StreamMessageDonePayload {
  return (
    !!value &&
    typeof value === "object" &&
    typeof (value as StreamMessageDonePayload).session_id === "string" &&
    typeof (value as StreamMessageDonePayload).run_id === "string" &&
    typeof (value as StreamMessageDonePayload).reply === "string"
  );
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
        text: `人生模型还没有真正写入，但你有 ${diagnostics.pending_builder_review_sessions} 个构建内容待确认。`,
        action: "回构建页查看",
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

const CHAT_PREVIEW_NO_TOOLS_PROMPT = "No developer tools catalog supplied for this chat preview.";
const CONTROLLED_PILOT_FALLBACK_COPY =
  "Use normal Send for the stable Chat path. The pilot will not retry automatically.";
const CONTROLLED_PILOT_RERUN_COPY =
  "Rerun Controlled Pilot in this session before promoting, or switch back to the source session.";
const CHAT_PREVIEW_SAFE_SUMMARY_KEYS = [
  "taskKind",
  "reasonCode",
  "riskLevel",
  "hasHsPacket",
  "policyReasonCode",
  "governanceDecisionKind",
];

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

function formatMainChatStrategy(
  strategy: MainChatAgentIngressDecision["selectedStrategy"]
): string {
  switch (strategy) {
    case "direct_answer":
      return "Direct";
    case "react_tool_execution":
      return "ReAct";
    case "plan_execute":
      return "Plan";
    case "memory_proposal":
      return "Memory";
    case "life_model_proposal":
      return "LifeModel";
    case "review_maturation":
      return "Review";
    case "blocked_confirmation":
      return "Blocked";
    default:
      return strategy;
  }
}

function formatTranscriptKind(kind: MainChatExecutionTranscriptEntry["kind"]): string {
  switch (kind) {
    case "route_decision":
      return "Route";
    case "permission_request":
      return "Permission";
    case "proposal_request":
      return "Proposal";
    case "final_result":
      return "Final";
    case "follow_up":
      return "Follow-up";
    case "user_input":
      return "Input";
    default:
      return kind.replace(/_/g, " ");
  }
}

function formatMainChatMetadataEntries(metadata?: Record<string, unknown>): string[] {
  if (!metadata) return [];
  return Object.entries(metadata).flatMap(([key, value]) => {
    if (value === undefined || value === null) return [];
    if (["string", "number", "boolean"].includes(typeof value)) {
      return [`${key}: ${String(value)}`];
    }
    if (Array.isArray(value)) {
      return [`${key}: ${value.length} items`];
    }
    return [`${key}: ${typeof value}`];
  });
}

function mainChatActionStatusClass(status: string): string {
  switch (status) {
    case "observed":
    case "completed":
      return "border-emerald-200 bg-emerald-50 text-emerald-800";
    case "failed":
      return "border-rose-200 bg-rose-50 text-rose-800";
    case "pending_permission":
      return "border-amber-200 bg-amber-50 text-amber-800";
    case "executing":
    case "retrying":
      return "border-blue-200 bg-blue-50 text-blue-800";
    case "cancelled":
      return "border-stone-300 bg-stone-100 text-stone-500";
    default:
      return "border-stone-200 bg-white text-stone-700";
  }
}

function mainChatTaskStatusClass(status: string, staleState?: string): string {
  if (staleState === "stale") return "border-amber-200 bg-amber-50 text-amber-900";
  switch (status) {
    case "completed":
      return "border-emerald-200 bg-emerald-50 text-emerald-800";
    case "blocked":
    case "waiting_permission":
      return "border-amber-200 bg-amber-50 text-amber-900";
    case "failed":
    case "cancelled":
      return "border-rose-200 bg-rose-50 text-rose-800";
    case "running":
      return "border-blue-200 bg-blue-50 text-blue-800";
    default:
      return "border-stone-200 bg-white text-stone-700";
  }
}

function formatContinuityControl(control: string): string {
  switch (control) {
    case "refresh_context":
      return "refresh context";
    case "open_trace":
      return "open trace";
    case "review_permission":
      return "review permission";
    default:
      return control.replace(/_/g, " ");
  }
}

const PLANNING_STAGE_PATTERN =
  /(规划|计划|安排|今日|今天|目标|日程|下一步|拆解|里程碑|weekly|plan|goal|schedule)/i;
const MEMORY_STAGE_PATTERN = /(记忆|人生模型|life\s*model|lifemodel|加入记忆|依据|回忆|memory)/i;

function inferStageFromText(text: string): AgentStageState | null {
  if (!text.trim()) return null;
  if (MEMORY_STAGE_PATTERN.test(text)) return "memory";
  if (PLANNING_STAGE_PATTERN.test(text)) return "planning";
  return null;
}

function inferStageFromToolCalls(toolCalls: ToolCallResult[]): AgentStageState | null {
  if (toolCalls.some(call => call.requires_confirmation || call.permission_level === "high")) {
    return "privacy";
  }
  return null;
}

export type ChatPageProps = {
  companionMode?: boolean;
  onCompanionStageChange?: (state: AgentStageState) => void;
};

type MainChatEventStreamStatus =
  | "loading_snapshot"
  | "subscribed"
  | "receiving_event"
  | "replaying_events"
  | "event_gap_detected"
  | "snapshot_refresh_required"
  | "stream_disconnected"
  | "stream_recovered";

type MainChatEventStreamViewState = {
  status: MainChatEventStreamStatus;
  taskSessionId?: string;
  lastAppliedSequence: number;
  events: MainChatAgentDurableEvent[];
};

function readablePreviewError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    if ("message" in error && typeof (error as any).message === "string") {
      return (error as any).message;
    }
    if ("error" in error && typeof (error as any).error === "string") {
      return (error as any).error;
    }
  }
  return String(error);
}

function safeSummaryEntries(summary: Record<string, unknown>): Array<[string, string]> {
  return CHAT_PREVIEW_SAFE_SUMMARY_KEYS.flatMap(key => {
    const value = summary[key];
    if (value === undefined || value === null) return [];
    if (!["string", "number", "boolean"].includes(typeof value)) return [];
    return [[key, String(value)]];
  });
}

function getPilotPromotionKey(result: MultiStrategyAgentPreviewOutput | null): string {
  if (!result) return "";
  return result.runId ?? "";
}

function checksumText(value: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `checksum:${hash.toString(16).padStart(8, "0")}`;
}

function buildControlledPilotSourceMismatchMessage(
  sourceSessionId: string | null,
  targetSessionId: string | null
): string {
  return `Promotion blocked: source session ${sourceSessionId ?? "unknown"} does not match target session ${targetSessionId ?? "unknown"}. ${CONTROLLED_PILOT_RERUN_COPY}`;
}

function hasPromotablePilotResponse(
  result: MultiStrategyAgentPreviewOutput | null
): result is MultiStrategyAgentPreviewOutput & { userOutput: string; runId: string } {
  if (!result?.userOutput?.trim()) return false;
  if (!result.runId?.trim()) return false;
  return result.payloadKind !== "blocked" && result.governanceDecisionKind !== "block";
}

export default function ChatPage({
  companionMode = false,
  onCompanionStageChange,
}: ChatPageProps = {}) {
  const location = useLocation();
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string>("default");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [selectedSkillId, setSelectedSkillId] = useState("");
  const [skillSummaries, setSkillSummaries] = useState<MainChatSkillSummary[]>([]);
  const [selectedSkillEvidence, setSelectedSkillEvidence] = useState<MainChatSelectedSkill | null>(
    null
  );
  const [inspectedSkillDetail, setInspectedSkillDetail] = useState<MainChatSkillDetail | null>(
    null
  );
  const [toolCandidateSurface, setToolCandidateSurface] =
    useState<MainChatToolCandidateList | null>(null);
  const [skillToolSurfaceAvailable, setSkillToolSurfaceAvailable] = useState(false);
  const [skillToolBusy, setSkillToolBusy] = useState(false);
  const [skillToolError, setSkillToolError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(true);
  const [preferLocal, setPreferLocal] = useState<boolean>(true);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [reasoningTrace, setReasoningTrace] = useState<ReasoningTrace | null>(null);
  const [showReasoningTrace, setShowReasoningTrace] = useState(false);
  const [toolCalls, setToolCalls] = useState<ToolCallResult[]>([]);
  const [showToolCalls, setShowToolCalls] = useState(false);
  const [currentRunId, setCurrentRunId] = useState<string | null>(null);
  const [model, setModel] = useState<LifeModel | null>(null);
  const [showGuide, setShowGuide] = useState(true);
  const [chatMode, setChatMode] = useState<string | null>(null);
  const [streamingReply, setStreamingReply] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const [streamInterrupted, setStreamInterrupted] = useState(false);
  const [currentRun, setCurrentRun] = useState<AgentRun | null>(null);
  const [currentAgentIngress, setCurrentAgentIngress] =
    useState<MainChatAgentIngressDecision | null>(null);
  const [currentExecutionTranscript, setCurrentExecutionTranscript] = useState<
    MainChatExecutionTranscriptEntry[]
  >([]);
  const [currentAgentTaskState, setCurrentAgentTaskState] = useState<MainChatAgentTaskState | null>(
    null
  );
  const [taskContinuitySummaries, setTaskContinuitySummaries] = useState<MainChatTaskSummary[]>([]);
  const [taskContinuityDetail, setTaskContinuityDetail] = useState<MainChatTaskDetail | null>(null);
  const [taskContinuityBusy, setTaskContinuityBusy] = useState(false);
  const [taskContinuityError, setTaskContinuityError] = useState<string | null>(null);
  const [currentAgentState, setCurrentAgentState] = useState<MainChatAgentStateSnapshot | null>(
    null
  );
  const [agentEventStreamState, setAgentEventStreamState] =
    useState<MainChatEventStreamViewState | null>(null);
  const [agentTaskControlBusy, setAgentTaskControlBusy] = useState(false);
  const [agentTaskControlError, setAgentTaskControlError] = useState<string | null>(null);
  const [legacyFallbackUsed, setLegacyFallbackUsed] = useState(false);
  const [pendingProposals, setPendingProposals] = useState<AgentProposal[]>([]);
  const [feedbackGiven, setFeedbackGiven] = useState<Record<number, "up" | "down">>({});
  const [governedPreviewOpen, setGovernedPreviewOpen] = useState(false);
  const [governedPreviewAllowPlanning, setGovernedPreviewAllowPlanning] = useState(false);
  const [governedPreviewLocalModelAvailable, setGovernedPreviewLocalModelAvailable] =
    useState(false);
  const [governedPreviewLayer, setGovernedPreviewLayer] =
    useState<MultiStrategyAgentPreviewLayer>("L2");
  const [governedPreviewSubmitting, setGovernedPreviewSubmitting] = useState(false);
  const [governedPreviewError, setGovernedPreviewError] = useState<string | null>(null);
  const [governedPreviewResult, setGovernedPreviewResult] =
    useState<MultiStrategyAgentPreviewOutput | null>(null);
  const [controlledPilotSubmitting, setControlledPilotSubmitting] = useState(false);
  const [controlledPilotError, setControlledPilotError] = useState<string | null>(null);
  const [controlledPilotFallback, setControlledPilotFallback] = useState<string | null>(null);
  const [controlledPilotEligibility, setControlledPilotEligibility] =
    useState<ControlledChatPilotEligibilityReport | null>(null);
  const [controlledPilotResult, setControlledPilotResult] =
    useState<MultiStrategyAgentPreviewOutput | null>(null);
  const [controlledPilotSourceSessionId, setControlledPilotSourceSessionId] = useState<
    string | null
  >(null);
  const [controlledPilotPromotionReviewOpen, setControlledPilotPromotionReviewOpen] =
    useState(false);
  const [controlledPilotPromoting, setControlledPilotPromoting] = useState(false);
  const [controlledPilotPromotionError, setControlledPilotPromotionError] = useState<string | null>(
    null
  );
  const [promotedControlledPilotKeys, setPromotedControlledPilotKeys] = useState<
    Record<string, boolean>
  >({});
  const [savedControlledPilotPromotionKeys, setSavedControlledPilotPromotionKeys] = useState<
    Record<string, boolean>
  >({});
  const [openEvidenceRunId, setOpenEvidenceRunId] = useState<string | null>(null);

  // Throttle streaming updates to reduce React re-render pressure
  const streamingBufferRef = useRef("");
  const streamingRafRef = useRef<number | null>(null);
  const diagnosticsRef = useRef<SystemDiagnostics | null>(null);
  const streamErrorHandledRef = useRef(false);
  const handledStreamDoneKeysRef = useRef<Set<string>>(new Set());
  const appliedMainChatEventIdsRef = useRef<Set<string>>(new Set());
  const lastAppliedMainChatEventSequenceRef = useRef(0);
  const currentAgentTaskSessionIdRef = useRef<string | null>(null);
  const lastUserMessageRef = useRef<ChatMessage | null>(null);
  const currentSessionIdRef = useRef<string>(currentSessionId);
  const promotedControlledPilotKeysRef = useRef<Record<string, boolean>>({});
  const savedControlledPilotPromotionKeysRef = useRef<Record<string, boolean>>({});
  const inFlightControlledPilotPromotionKeysRef = useRef<Set<string>>(new Set());

  const applyMainChatAgentStateSnapshot = useCallback(
    (
      snapshot: MainChatAgentStateSnapshot | null,
      status: MainChatEventStreamStatus = "subscribed"
    ) => {
      setCurrentAgentState(snapshot);
      if (!snapshot) {
        currentAgentTaskSessionIdRef.current = null;
        lastAppliedMainChatEventSequenceRef.current = 0;
        appliedMainChatEventIdsRef.current = new Set();
        setAgentEventStreamState(null);
        return;
      }
      currentAgentTaskSessionIdRef.current = snapshot.task.taskId;
      const snapshotSequence =
        status === "snapshot_refresh_required" || status === "loading_snapshot"
          ? snapshot.sequence
          : 0;
      lastAppliedMainChatEventSequenceRef.current = snapshotSequence;
      appliedMainChatEventIdsRef.current = new Set();
      setAgentEventStreamState({
        status,
        taskSessionId: snapshot.task.taskId,
        lastAppliedSequence: snapshotSequence,
        events: [],
      });
    },
    []
  );

  const applyMainChatAgentEvents = useCallback(
    (
      events: MainChatAgentDurableEvent[],
      status: MainChatEventStreamStatus = "receiving_event"
    ) => {
      if (events.length === 0) return;
      setAgentEventStreamState(prev => {
        const currentTask = currentAgentTaskSessionIdRef.current;
        if (!currentTask) return prev;
        const nextEvents = prev?.events ? [...prev.events] : [];
        let lastSequence = lastAppliedMainChatEventSequenceRef.current;
        let changed = false;
        for (const event of events) {
          if (event.taskSessionId !== currentTask) continue;
          if (appliedMainChatEventIdsRef.current.has(event.eventId)) continue;
          if (event.sequence <= lastSequence) continue;
          appliedMainChatEventIdsRef.current.add(event.eventId);
          lastSequence = event.sequence;
          nextEvents.push(event);
          changed = true;
        }
        if (!changed) return prev;
        lastAppliedMainChatEventSequenceRef.current = lastSequence;
        return {
          status,
          taskSessionId: currentTask,
          lastAppliedSequence: lastSequence,
          events: nextEvents.slice(-50),
        };
      });
    },
    []
  );

  const handleMainChatAgentEvent = useCallback(
    async (event: MainChatAgentDurableEvent) => {
      const currentTask = currentAgentTaskSessionIdRef.current;
      if (!currentTask || event.taskSessionId !== currentTask) return;
      if (appliedMainChatEventIdsRef.current.has(event.eventId)) return;
      const lastSequence = lastAppliedMainChatEventSequenceRef.current;
      if (event.sequence <= lastSequence) return;
      if (event.sequence > lastSequence + 1) {
        setAgentEventStreamState(prev =>
          prev
            ? { ...prev, status: "event_gap_detected" }
            : {
                status: "event_gap_detected",
                taskSessionId: currentTask,
                lastAppliedSequence: lastSequence,
                events: [],
              }
        );
        try {
          const replayed = await listMainChatAgentEvents(currentTask, lastSequence, 100);
          const expectedSequence = lastSequence + 1;
          const replayCoversGap =
            replayed.length > 0 &&
            replayed[0]?.sequence === expectedSequence &&
            replayed.every(
              (item, index) => index === 0 || item.sequence === replayed[index - 1].sequence + 1
            ) &&
            replayed.some(item => item.sequence >= event.sequence);
          if (replayCoversGap) {
            setAgentEventStreamState(prev =>
              prev
                ? { ...prev, status: "replaying_events" }
                : {
                    status: "replaying_events",
                    taskSessionId: currentTask,
                    lastAppliedSequence: lastSequence,
                    events: [],
                  }
            );
            applyMainChatAgentEvents(replayed, "stream_recovered");
            return;
          }
          setAgentEventStreamState(prev =>
            prev
              ? { ...prev, status: "snapshot_refresh_required" }
              : {
                  status: "snapshot_refresh_required",
                  taskSessionId: currentTask,
                  lastAppliedSequence: lastSequence,
                  events: [],
                }
          );
          const snapshot = await getMainChatAgentStateSnapshot(currentTask);
          applyMainChatAgentStateSnapshot(snapshot, "snapshot_refresh_required");
        } catch {
          setAgentEventStreamState(prev =>
            prev
              ? { ...prev, status: "snapshot_refresh_required" }
              : {
                  status: "snapshot_refresh_required",
                  taskSessionId: currentTask,
                  lastAppliedSequence: lastSequence,
                  events: [],
                }
          );
          try {
            const snapshot = await getMainChatAgentStateSnapshot(currentTask);
            applyMainChatAgentStateSnapshot(snapshot, "snapshot_refresh_required");
          } catch {
            setAgentEventStreamState(prev =>
              prev
                ? { ...prev, status: "stream_disconnected" }
                : {
                    status: "stream_disconnected",
                    taskSessionId: currentTask,
                    lastAppliedSequence: lastSequence,
                    events: [],
                  }
            );
          }
        }
        return;
      }
      applyMainChatAgentEvents([event], "receiving_event");
    },
    [applyMainChatAgentEvents, applyMainChatAgentStateSnapshot]
  );

  const emitCompanionStage = useCallback(
    (state: AgentStageState) => {
      onCompanionStageChange?.(state);
    },
    [onCompanionStageChange]
  );

  useEffect(() => {
    currentSessionIdRef.current = currentSessionId;
  }, [applyMainChatAgentStateSnapshot, currentSessionId]);

  useEffect(() => {
    promotedControlledPilotKeysRef.current = promotedControlledPilotKeys;
  }, [promotedControlledPilotKeys]);

  useEffect(() => {
    savedControlledPilotPromotionKeysRef.current = savedControlledPilotPromotionKeys;
  }, [savedControlledPilotPromotionKeys]);

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

  const loadMainChatTaskState = async (
    taskSessionId: string | undefined,
    sourceSessionId = currentSessionIdRef.current
  ) => {
    if (!taskSessionId) {
      setCurrentAgentTaskState(null);
      return;
    }
    try {
      const state = await getMainChatAgentTaskState(taskSessionId);
      if (currentSessionIdRef.current === sourceSessionId) {
        setCurrentAgentTaskState(state);
        if (state.transcript?.length) {
          setCurrentExecutionTranscript(state.transcript);
        }
      }
    } catch {
      if (currentSessionIdRef.current === sourceSessionId) {
        setCurrentAgentTaskState(null);
      }
    }
  };

  const loadTaskContinuityList = useCallback(async () => {
    try {
      const summaries = await listMainChatAgentTasks(
        {
          includeTerminal: true,
          includeStale: true,
        },
        50,
        0
      );
      setTaskContinuitySummaries(summaries);
      setTaskContinuityError(null);
    } catch (e) {
      setTaskContinuitySummaries([]);
      setTaskContinuityError(`Task continuity failed: ${readablePreviewError(e)}`);
    }
  }, []);

  const loadTaskContinuityDetail = useCallback(async (taskSessionId: string) => {
    setTaskContinuityBusy(true);
    setTaskContinuityError(null);
    try {
      const detail = await getMainChatAgentTaskDetail(taskSessionId);
      setTaskContinuityDetail(detail);
    } catch (e) {
      setTaskContinuityError(`Task detail failed: ${readablePreviewError(e)}`);
    } finally {
      setTaskContinuityBusy(false);
    }
  }, []);

  const refreshPendingProposals = async () => {
    try {
      const proposals = await getPendingProposals(10);
      setPendingProposals(proposals);
    } catch {
      setPendingProposals([]);
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
    if (diagnostics && isSafeMode(diagnostics)) {
      emitCompanionStage("privacy");
    }
  }, [diagnostics, emitCompanionStage]);

  useEffect(() => {
    if (pendingProposals.length > 0) {
      emitCompanionStage("review");
    }
  }, [pendingProposals.length, emitCompanionStage]);

  useEffect(() => {
    const toolStage = inferStageFromToolCalls(toolCalls);
    if (toolStage) {
      emitCompanionStage(toolStage);
    }
  }, [toolCalls, emitCompanionStage]);

  useEffect(() => {
    if (currentRun?.status === "failed") {
      emitCompanionStage("error");
      return;
    }
    if (currentRun?.status === "waiting_permission") {
      emitCompanionStage("privacy");
      return;
    }
    if ((currentRun?.generatedProposals?.length ?? 0) > 0) {
      emitCompanionStage("review");
      return;
    }
    if (currentRun?.status === "running") {
      emitCompanionStage("sorting");
    }
  }, [currentRun, emitCompanionStage]);

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
    refreshPendingProposals();
    loadTaskContinuityList();
  }, [loadTaskContinuityList]);

  useEffect(() => {
    const refreshChatContext = () => {
      getLifeModel()
        .then(setModel)
        .catch(() => {});
      getSystemDiagnostics()
        .then(setDiagnostics)
        .catch(() => {});
      loadSessions(currentSessionIdRef.current);
      refreshPendingProposals();
      loadTaskContinuityList();
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
  }, [loadTaskContinuityList]);

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
    applyMainChatAgentStateSnapshot(null);
    setTaskContinuityDetail(null);
    setLoadingHistory(true);
    refreshAgentRuns(currentSessionId);
    loadTaskContinuityList();
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
  }, [currentSessionId, applyMainChatAgentStateSnapshot, loadTaskContinuityList]);

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
    let unlistenAgentEvent: (() => void) | null = null;

    (async () => {
      unlistenAgentEvent = await listen<MainChatAgentDurableEvent>(
        "main-chat-agent-event",
        async event => {
          await handleMainChatAgentEvent(event.payload);
        }
      );
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
            setCurrentAgentIngress(event.payload.agent_ingress ?? null);
            applyMainChatAgentStateSnapshot(event.payload.agent_state ?? null);
            setCurrentExecutionTranscript(event.payload.execution_transcript ?? []);
            setLegacyFallbackUsed(Boolean(event.payload.legacy_fallback_used));
            await loadMainChatTaskState(
              event.payload.agent_ingress?.agentTaskSessionId,
              event.payload.session_id
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
          const doneKey = `${event.payload.session_id}:${event.payload.run_id}`;
          if (handledStreamDoneKeysRef.current.has(doneKey)) {
            return;
          }
          handledStreamDoneKeysRef.current.add(doneKey);
          if (handledStreamDoneKeysRef.current.size > 100) {
            const oldestKey = handledStreamDoneKeysRef.current.values().next().value;
            if (oldestKey) handledStreamDoneKeysRef.current.delete(oldestKey);
          }
          const nextStage =
            inferStageFromToolCalls(event.payload.tool_calls ?? []) ??
            inferStageFromText(event.payload.reply) ??
            "idle";
          flushStreaming();
          setMessages(prev => {
            if (
              prev.some(
                message => message.role === "assistant" && message.run_id === event.payload.run_id
              )
            ) {
              return prev;
            }
            return [
              ...prev,
              { role: "assistant", content: event.payload.reply, run_id: event.payload.run_id },
            ];
          });
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
          setCurrentAgentIngress(event.payload.agent_ingress ?? null);
          applyMainChatAgentStateSnapshot(event.payload.agent_state ?? null);
          setCurrentExecutionTranscript(event.payload.execution_transcript ?? []);
          setLegacyFallbackUsed(Boolean(event.payload.legacy_fallback_used));
          setStreamInterrupted(false);
          emitCompanionStage(nextStage);
          await loadMainChatTaskState(
            event.payload.agent_ingress?.agentTaskSessionId,
            event.payload.session_id
          );
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
            applyMainChatAgentStateSnapshot(null);
            emitCompanionStage("error");
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
      if (unlistenAgentEvent) unlistenAgentEvent();
    };
  }, [
    applyMainChatAgentStateSnapshot,
    currentSessionId,
    emitCompanionStage,
    handleMainChatAgentEvent,
  ]);

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

  const handleNewSession = useCallback(async () => {
    const id = generateSessionId();
    try {
      await createChatSession(id, "新会话");
      await loadSessions();
      setCurrentSessionId(id);
    } catch (e) {
      console.error("创建会话失败", e);
    }
  }, []);

  const handleDeleteSession = useCallback(async (id: string) => {
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
  }, []);

  const startEditTitle = useCallback((s: ChatSession) => {
    setEditingId(s.session_id);
    setEditingTitle(s.title);
  }, []);

  const commitEditTitle = useCallback(async () => {
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
  }, [editingId, editingTitle]);

  const handleExecuteToolCall = useCallback(
    async (index: number) => {
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
        loadAgentRunForSession(call.run_id, currentSessionIdRef.current);
        refreshAgentRuns(currentSessionIdRef.current);
      } catch (e) {
        const errMsg = String(e);
        // 如果是因为未授权，保持 pending 状态，不改为 error
        if (errMsg.includes("not authorized") || errMsg.includes("Review Center")) {
          // 保持 requires_confirmation: true，让用户去 Review Center 授权
          console.warn("Tool call still needs authorization:", errMsg);
          throw e; // 抛出错误让 ToolCallCard 显示提示
        }
        // 其他错误才标记为失败
        setToolCalls(prev =>
          prev.map((item, idx) =>
            idx === index
              ? {
                  ...item,
                  success: false,
                  error: errMsg,
                  status: "error",
                  requires_confirmation: false,
                }
              : item
          )
        );
      }
    },
    [toolCalls]
  );

  const tryHandleQuickCommand = useCallback(async (text: string): Promise<string | null> => {
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
  }, []);

  const handleSend = useCallback(async () => {
    if (!input.trim() || sending) return;
    if (!currentSessionId || typeof currentSessionId !== "string") {
      emitCompanionStage("error");
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
      emitCompanionStage("error");
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
    setCurrentAgentIngress(null);
    applyMainChatAgentStateSnapshot(null);
    setCurrentExecutionTranscript([]);
    setCurrentAgentTaskState(null);
    setAgentTaskControlError(null);
    setLegacyFallbackUsed(false);
    emitCompanionStage("sorting");

    try {
      const selectedSkillOption = selectedSkillId.trim() || undefined;
      // The streaming backend persists the user message before model execution.
      // Saving it here as well creates duplicate user rows in history and memory retrieval.
      const browserE2eDone = await startStreamMessage(currentSessionId, nextMessages, {
        selectedSkillId: selectedSkillOption,
      });
      if (isStreamDonePayload(browserE2eDone) && browserE2eDone.session_id === currentSessionId) {
        const nextStage =
          inferStageFromToolCalls(browserE2eDone.tool_calls ?? []) ??
          inferStageFromText(browserE2eDone.reply) ??
          "idle";
        flushStreaming();
        setMessages(prev => {
          if (
            prev.some(
              message => message.role === "assistant" && message.run_id === browserE2eDone.run_id
            )
          ) {
            return prev;
          }
          return [
            ...prev,
            {
              role: "assistant",
              content: browserE2eDone.reply,
              run_id: browserE2eDone.run_id,
            },
          ];
        });
        setStreamingReply("");
        setSending(false);
        setReasoningTrace(browserE2eDone.reasoning_trace ?? null);
        setCurrentRunId(browserE2eDone.run_id);
        setToolCalls(
          (browserE2eDone.tool_calls ?? []).map(call => ({
            ...call,
            run_id: browserE2eDone.run_id,
          }))
        );
        setCurrentAgentIngress(browserE2eDone.agent_ingress ?? null);
        applyMainChatAgentStateSnapshot(browserE2eDone.agent_state ?? null);
        setCurrentExecutionTranscript(browserE2eDone.execution_transcript ?? []);
        setLegacyFallbackUsed(Boolean(browserE2eDone.legacy_fallback_used));
        setStreamInterrupted(false);
        emitCompanionStage(nextStage);
        await loadMainChatTaskState(
          browserE2eDone.agent_ingress?.agentTaskSessionId,
          browserE2eDone.session_id
        );
        await loadAgentRunForSession(browserE2eDone.run_id, browserE2eDone.session_id);
        refreshAgentRuns(browserE2eDone.session_id);
        logAnalyticsEvent("send_message", currentSessionId, undefined).catch(() => {});
      }
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
      emitCompanionStage("error");
    }
  }, [
    input,
    sending,
    currentSessionId,
    messages,
    diagnostics,
    selectedSkillId,
    tryHandleQuickCommand,
    emitCompanionStage,
    applyMainChatAgentStateSnapshot,
  ]);

  const retryLastUserMessage = useCallback(() => {
    const last =
      lastUserMessageRef.current ?? [...messages].reverse().find(m => m.role === "user") ?? null;
    if (!last || sending) return;
    setInput(last.content);
  }, [messages, sending]);

  const handleContinueStream = useCallback(async () => {
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
    setCurrentAgentIngress(null);
    applyMainChatAgentStateSnapshot(null);
    setCurrentExecutionTranscript([]);
    setCurrentAgentTaskState(null);
    setAgentTaskControlError(null);
    setLegacyFallbackUsed(false);
    emitCompanionStage("sorting");
    try {
      const selectedSkillOption = selectedSkillId.trim() || undefined;
      await startStreamMessage(currentSessionId, retryMessages, {
        selectedSkillId: selectedSkillOption,
      });
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
      emitCompanionStage("error");
    }
  }, [
    currentSessionId,
    messages,
    selectedSkillId,
    sending,
    emitCompanionStage,
    applyMainChatAgentStateSnapshot,
  ]);

  const currentMainChatTaskSessionId = useCallback(() => {
    return (
      currentAgentIngress?.agentTaskSessionId ??
      currentAgentState?.task?.taskId ??
      currentAgentTaskState?.session?.id
    );
  }, [
    currentAgentIngress?.agentTaskSessionId,
    currentAgentState?.task?.taskId,
    currentAgentTaskState?.session?.id,
  ]);

  const loadSkillToolSurface = useCallback(
    async (taskSessionId?: string) => {
      try {
        const [skills, candidates] = await Promise.all([
          listMainChatSkills(currentSessionIdRef.current),
          listMainChatToolCandidates(taskSessionId),
        ]);
        setSkillSummaries(skills);
        setToolCandidateSurface(candidates);
        setSkillToolSurfaceAvailable(
          skills.length > 0 ||
            candidates.candidates.length > 0 ||
            candidates.blockedTools.length > 0 ||
            Boolean(candidates.failureRecovery)
        );
        setSkillToolError(null);
        const selected = skills.find(skill => skill.selected);
        if (selected && selected.skillId !== selectedSkillId) {
          setSelectedSkillId(selected.skillId);
        }
      } catch {
        setSkillToolSurfaceAvailable(false);
        setSkillSummaries([]);
        setToolCandidateSurface(null);
      }
    },
    [selectedSkillId]
  );

  useEffect(() => {
    if (companionMode) return;
    void loadSkillToolSurface(currentMainChatTaskSessionId());
  }, [companionMode, currentMainChatTaskSessionId, currentSessionId, loadSkillToolSurface]);

  const handleInspectSkill = useCallback(async (skillId: string) => {
    setSkillToolBusy(true);
    setSkillToolError(null);
    try {
      const detail = await getMainChatSkillDetail(skillId);
      setInspectedSkillDetail(detail);
    } catch (error) {
      setSkillToolError(error instanceof Error ? error.message : String(error));
    } finally {
      setSkillToolBusy(false);
    }
  }, []);

  const handleSelectSkill = useCallback(
    async (skillId: string) => {
      setSkillToolBusy(true);
      setSkillToolError(null);
      try {
        const selection = await selectMainChatSkill(currentSessionIdRef.current, skillId);
        setSelectedSkillEvidence(selection);
        setSelectedSkillId(selection.selectedSkillId ?? "");
        const detail = await getMainChatSkillDetail(skillId);
        setInspectedSkillDetail(detail);
        await loadSkillToolSurface(currentMainChatTaskSessionId());
      } catch (error) {
        setSkillToolError(error instanceof Error ? error.message : String(error));
      } finally {
        setSkillToolBusy(false);
      }
    },
    [currentMainChatTaskSessionId, loadSkillToolSurface]
  );

  const handleClearSelectedSkill = useCallback(async () => {
    setSkillToolBusy(true);
    setSkillToolError(null);
    try {
      const selection = await clearMainChatSkill(currentSessionIdRef.current);
      setSelectedSkillEvidence(selection);
      setSelectedSkillId("");
      await loadSkillToolSurface(currentMainChatTaskSessionId());
    } catch (error) {
      setSkillToolError(error instanceof Error ? error.message : String(error));
    } finally {
      setSkillToolBusy(false);
    }
  }, [currentMainChatTaskSessionId, loadSkillToolSurface]);

  const refreshMainChatControlState = useCallback(
    async (taskSessionId?: string) => {
      await refreshPendingProposals();
      if (taskSessionId) {
        try {
          const snapshot = await getMainChatAgentStateSnapshot(taskSessionId);
          applyMainChatAgentStateSnapshot(snapshot, "snapshot_refresh_required");
        } catch {
          // Task-state refresh still runs below; snapshot reload is best-effort after controls.
        }
        await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
      }
      await loadTaskContinuityList();
    },
    [applyMainChatAgentStateSnapshot, loadTaskContinuityList]
  );

  const handleResumeMainChatTask = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    if (!taskSessionId || agentTaskControlBusy) return;
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const state = await resumeMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await refreshPendingProposals();
    } catch (e) {
      setAgentTaskControlError(`Resume failed: ${readablePreviewError(e)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [agentTaskControlBusy, currentMainChatTaskSessionId]);

  const handleCancelMainChatTask = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    if (!taskSessionId || agentTaskControlBusy) return;
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const state = await cancelMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
    } catch (e) {
      setAgentTaskControlError(`Cancel failed: ${readablePreviewError(e)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [agentTaskControlBusy, currentMainChatTaskSessionId]);

  const handleRetryMainChatAction = useCallback(
    async (target?: { actionId?: string }) => {
      const taskSessionId = currentMainChatTaskSessionId();
      const actionId =
        target?.actionId ??
        currentAgentState?.blockers.find(blocker => blocker.affectedActionId)?.affectedActionId ??
        currentAgentTaskState?.actions.find(action => action.status === "failed")?.id;
      if (!taskSessionId || !actionId || agentTaskControlBusy) return;
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        const state = await retryMainChatAgentAction(taskSessionId, actionId);
        setCurrentAgentTaskState(state);
        setCurrentExecutionTranscript(state.transcript ?? []);
      } catch (e) {
        setAgentTaskControlError(`Retry failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [
      agentTaskControlBusy,
      currentAgentState?.blockers,
      currentMainChatTaskSessionId,
      currentAgentTaskState?.actions,
    ]
  );

  const handleRefreshTaskContinuityContext = useCallback(async () => {
    const taskSessionId = taskContinuityDetail?.taskSession.id;
    if (!taskSessionId || taskContinuityBusy) return;
    setTaskContinuityBusy(true);
    setTaskContinuityError(null);
    try {
      const detail = await refreshMainChatAgentTaskContext(taskSessionId);
      setTaskContinuityDetail(detail);
      await loadTaskContinuityList();
    } catch (e) {
      setTaskContinuityError(`Refresh context failed: ${readablePreviewError(e)}`);
    } finally {
      setTaskContinuityBusy(false);
    }
  }, [loadTaskContinuityList, taskContinuityBusy, taskContinuityDetail?.taskSession.id]);

  const handleResumeTaskContinuityDetail = useCallback(async () => {
    const taskSessionId = taskContinuityDetail?.taskSession.id;
    if (!taskSessionId || taskContinuityBusy) return;
    setTaskContinuityBusy(true);
    setTaskContinuityError(null);
    try {
      const state = await resumeMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await loadTaskContinuityDetail(taskSessionId);
      await loadTaskContinuityList();
    } catch (e) {
      setTaskContinuityError(`Resume failed: ${readablePreviewError(e)}`);
    } finally {
      setTaskContinuityBusy(false);
    }
  }, [
    loadTaskContinuityDetail,
    loadTaskContinuityList,
    taskContinuityBusy,
    taskContinuityDetail?.taskSession.id,
  ]);

  const handleRetryTaskContinuityDetail = useCallback(async () => {
    const taskSessionId = taskContinuityDetail?.taskSession.id;
    const actionId =
      taskContinuityDetail?.lastSafeResumePoint ??
      taskContinuityDetail?.actions.find(action => action.status === "failed")?.id;
    if (!taskSessionId || !actionId || taskContinuityBusy) return;
    setTaskContinuityBusy(true);
    setTaskContinuityError(null);
    try {
      const state = await retryMainChatAgentAction(taskSessionId, actionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await loadTaskContinuityDetail(taskSessionId);
      await loadTaskContinuityList();
    } catch (e) {
      setTaskContinuityError(`Retry failed: ${readablePreviewError(e)}`);
    } finally {
      setTaskContinuityBusy(false);
    }
  }, [
    loadTaskContinuityDetail,
    loadTaskContinuityList,
    taskContinuityBusy,
    taskContinuityDetail?.actions,
    taskContinuityDetail?.lastSafeResumePoint,
    taskContinuityDetail?.taskSession.id,
  ]);

  const handleCancelTaskContinuityDetail = useCallback(async () => {
    const taskSessionId = taskContinuityDetail?.taskSession.id;
    if (!taskSessionId || taskContinuityBusy) return;
    setTaskContinuityBusy(true);
    setTaskContinuityError(null);
    try {
      const state = await cancelMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await loadTaskContinuityDetail(taskSessionId);
      await loadTaskContinuityList();
    } catch (e) {
      setTaskContinuityError(`Cancel failed: ${readablePreviewError(e)}`);
    } finally {
      setTaskContinuityBusy(false);
    }
  }, [
    loadTaskContinuityDetail,
    loadTaskContinuityList,
    taskContinuityBusy,
    taskContinuityDetail?.taskSession.id,
  ]);

  const handleRejectTaskContinuityProposal = useCallback(
    async (proposalId: string) => {
      const taskSessionId = taskContinuityDetail?.taskSession.id;
      if (!taskSessionId || taskContinuityBusy) return;
      setTaskContinuityBusy(true);
      setTaskContinuityError(null);
      try {
        await rejectProposal(proposalId);
        await loadTaskContinuityDetail(taskSessionId);
        await loadTaskContinuityList();
        await refreshPendingProposals();
      } catch (e) {
        setTaskContinuityError(`Reject proposal failed: ${readablePreviewError(e)}`);
      } finally {
        setTaskContinuityBusy(false);
      }
    },
    [
      loadTaskContinuityDetail,
      loadTaskContinuityList,
      taskContinuityBusy,
      taskContinuityDetail?.taskSession.id,
    ]
  );

  const handleDeferTaskContinuityProposal = useCallback(
    async (proposalId: string) => {
      const taskSessionId = taskContinuityDetail?.taskSession.id;
      if (!taskSessionId || taskContinuityBusy) return;
      setTaskContinuityBusy(true);
      setTaskContinuityError(null);
      try {
        await postponeProposal(proposalId);
        await loadTaskContinuityDetail(taskSessionId);
        await loadTaskContinuityList();
        await refreshPendingProposals();
      } catch (e) {
        setTaskContinuityError(`Defer proposal failed: ${readablePreviewError(e)}`);
      } finally {
        setTaskContinuityBusy(false);
      }
    },
    [
      loadTaskContinuityDetail,
      loadTaskContinuityList,
      taskContinuityBusy,
      taskContinuityDetail?.taskSession.id,
    ]
  );

  const handleApproveOnceMainChatPermission = useCallback(
    async (target: { proposalId: string; actionId: string; blockerId: string }) => {
      const taskSessionId = currentMainChatTaskSessionId();
      if (!taskSessionId || agentTaskControlBusy) return;
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await acceptProposal(target.proposalId);
        const state = await resumeMainChatAgentTask(taskSessionId);
        setCurrentAgentTaskState(state);
        setCurrentExecutionTranscript(state.transcript ?? []);
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Approve once failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatControlState]
  );

  const handleDenyMainChatControl = useCallback(
    async (target: { proposalId?: string }) => {
      if (!target.proposalId || agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await rejectProposal(target.proposalId);
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Deny failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatControlState]
  );

  const handleDeferMainChatControl = useCallback(
    async (target: { proposalId?: string }) => {
      if (!target.proposalId || agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await postponeProposal(target.proposalId);
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Defer failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatControlState]
  );

  const handleAcceptAgentProposal = useCallback(
    async (proposalId: string) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      const proposal = currentAgentState?.proposals.find(item => item.proposalId === proposalId);
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await acceptProposal(proposalId);
        if (
          proposal?.proposalType === "tool_permission" &&
          proposal.actionIds.length > 0 &&
          taskSessionId
        ) {
          const state = await resumeMainChatAgentTask(taskSessionId);
          setCurrentAgentTaskState(state);
          setCurrentExecutionTranscript(state.transcript ?? []);
        }
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Accept proposal failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [
      agentTaskControlBusy,
      currentAgentState?.proposals,
      currentMainChatTaskSessionId,
      refreshMainChatControlState,
    ]
  );

  const handleRejectAgentProposal = useCallback(
    async (proposalId: string) => {
      await handleDenyMainChatControl({ proposalId });
    },
    [handleDenyMainChatControl]
  );

  const handleEditAgentProposal = useCallback(
    async (proposalId: string) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        const proposal =
          pendingProposals.find(item => item.id === proposalId) ??
          (await getPendingProposals(100)).find(item => item.id === proposalId);
        if (!proposal) {
          throw new Error("proposal evidence not found");
        }
        const draft = window.prompt(
          "Edit proposal JSON",
          JSON.stringify(proposal.after ?? {}, null, 2)
        );
        if (draft === null) return;
        const parsed = JSON.parse(draft);
        if (
          proposal.proposalType === "memory_write" ||
          proposal.proposalType === "preference_update"
        ) {
          await draftEditMemoryProposal(proposalId, parsed);
        } else {
          await editProposal(proposalId, parsed);
        }
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Edit proposal failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [
      agentTaskControlBusy,
      currentMainChatTaskSessionId,
      pendingProposals,
      refreshMainChatControlState,
    ]
  );

  const handleRollbackMemory = useCallback(
    async (memoryId: string) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await rollbackMemoryAsset(
          memoryId,
          "User requested rollback from Main Chat control plane."
        );
        await refreshMainChatControlState(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Rollback failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatControlState]
  );

  const refreshMainChatSnapshot = useCallback(
    async (taskSessionId?: string) => {
      if (!taskSessionId) return;
      await refreshMainChatControlState(taskSessionId);
    },
    [refreshMainChatControlState]
  );

  const handleConfirmPlan = useCallback(
    async (target: { planSessionId: string; baseRevision: number }) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await finalizePlanExecuteSession(target.planSessionId, target.baseRevision);
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Confirm plan failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const handleEditPlanStep = useCallback(
    async (target: {
      planSessionId: string;
      baseRevision: number;
      stepId: string;
      title: string;
    }) => {
      if (agentTaskControlBusy) return;
      const nextTitle = window.prompt("Edit plan step", target.title);
      if (nextTitle === null || !nextTitle.trim()) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await updatePlanExecuteSessionDraft({
          sessionId: target.planSessionId,
          baseRevision: target.baseRevision,
          steps: [{ stepId: target.stepId, title: nextTitle.trim() }],
        });
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Edit plan failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const handleExecutePlanStep = useCallback(
    async (target: { planSessionId: string; baseRevision: number; stepId: string }) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await executePlanExecuteStep({
          sessionId: target.planSessionId,
          stepId: target.stepId,
          baseRevision: target.baseRevision,
        });
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Execute plan step failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const handleSkipPlanStep = useCallback(
    async (target: { planSessionId: string; baseRevision: number; stepId: string }) => {
      if (agentTaskControlBusy) return;
      const reason = window.prompt("Skip reason", "");
      if (reason === null || !reason.trim()) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await skipPlanExecuteStep({
          sessionId: target.planSessionId,
          stepId: target.stepId,
          baseRevision: target.baseRevision,
          reason: reason.trim(),
        });
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Skip plan step failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const handleCancelPlan = useCallback(
    async (target: { planSessionId: string; baseRevision: number }) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await cancelPlanExecuteSession(target.planSessionId, target.baseRevision);
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Cancel plan failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const handleReviewPlan = useCallback(
    async (target: { planSessionId: string; baseRevision: number }) => {
      if (agentTaskControlBusy) return;
      const taskSessionId = currentMainChatTaskSessionId();
      setAgentTaskControlBusy(true);
      setAgentTaskControlError(null);
      try {
        await reviewPlanExecuteSession(target.planSessionId, target.baseRevision);
        await refreshMainChatSnapshot(taskSessionId);
      } catch (e) {
        setAgentTaskControlError(`Review plan failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const readiness = useMemo(() => buildReadinessSummary(diagnostics), [diagnostics]);
  const governedPreviewSummaryEntries = useMemo(
    () => safeSummaryEntries(governedPreviewResult?.metadataSafeSummary ?? {}),
    [governedPreviewResult]
  );
  const controlledPilotSummaryEntries = useMemo(
    () => safeSummaryEntries(controlledPilotResult?.metadataSafeSummary ?? {}),
    [controlledPilotResult]
  );
  const controlledPilotCanPromote = hasPromotablePilotResponse(controlledPilotResult);
  const controlledPilotPromotionKey = getPilotPromotionKey(controlledPilotResult);
  const controlledPilotPromoted = Boolean(
    controlledPilotPromotionKey && promotedControlledPilotKeys[controlledPilotPromotionKey]
  );
  const controlledPilotPromotionMessageSaved = Boolean(
    controlledPilotPromotionKey && savedControlledPilotPromotionKeys[controlledPilotPromotionKey]
  );
  const controlledPilotTargetSessionId = currentSessionId || null;
  const controlledPilotSessionMismatch = Boolean(
    controlledPilotResult &&
    (!controlledPilotSourceSessionId ||
      controlledPilotSourceSessionId !== controlledPilotTargetSessionId)
  );
  const controlledPilotSessionBlockingMessage = controlledPilotSessionMismatch
    ? buildControlledPilotSourceMismatchMessage(
        controlledPilotSourceSessionId,
        controlledPilotTargetSessionId
      )
    : "";
  const controlledPilotGovernanceSummary = controlledPilotResult
    ? [
        `decision=${controlledPilotResult.governanceDecisionKind ?? "unknown"}`,
        ...controlledPilotSummaryEntries.map(([key, value]) => `${key}=${value}`),
      ].join(" · ")
    : "";
  const controlledPilotPayloadSummary = controlledPilotResult
    ? [
        `payloadKind=${controlledPilotResult.payloadKind}`,
        `proposalIds=${controlledPilotResult.proposalIds.length}`,
        `warnings=${controlledPilotResult.warnings.length}`,
      ].join(" · ")
    : "";
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

  const fillPrompt = useCallback(
    (prompt: string) => {
      setInput(prompt);
      emitCompanionStage(inferStageFromText(prompt) ?? "listening");
    },
    [emitCompanionStage]
  );

  const selectChatMode = (mode: string) => {
    setChatMode(mode);
    const found = chatModes.find(m => m.key === mode);
    if (found) {
      if (mode === "free") {
        setInput("");
        emitCompanionStage("listening");
      } else {
        setInput(found.prompt);
        emitCompanionStage(inferStageFromText(found.prompt) ?? "listening");
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

  const handleSaveAsDailyGoal = useCallback(async (content: string) => {
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
  }, []);

  const handleRunGovernedPreview = useCallback(async () => {
    const trimmedInput = input.trim();
    if (!trimmedInput) {
      setGovernedPreviewError("Enter a chat draft before running governed preview.");
      return;
    }

    setGovernedPreviewSubmitting(true);
    setGovernedPreviewError(null);
    setGovernedPreviewResult(null);

    try {
      const output = await runMultiStrategyAgentPreview({
        sessionId: `chat-governed-preview-${Date.now()}`,
        userText: trimmedInput,
        toolsPrompt: CHAT_PREVIEW_NO_TOOLS_PROMPT,
        allowPlanning: governedPreviewAllowPlanning,
        localModelAvailable: governedPreviewLocalModelAvailable,
        layer: governedPreviewLayer,
        executionBudget: {
          allowWrites: false,
        },
      });
      setGovernedPreviewResult(output);
    } catch (e) {
      setGovernedPreviewError(`Preview failed: ${readablePreviewError(e)}`);
    } finally {
      setGovernedPreviewSubmitting(false);
    }
  }, [
    governedPreviewAllowPlanning,
    governedPreviewLayer,
    governedPreviewLocalModelAvailable,
    input,
  ]);

  const handleRunControlledPilot = useCallback(async () => {
    const trimmedInput = input.trim();
    if (!trimmedInput) {
      setControlledPilotError("Enter a chat draft before running controlled pilot.");
      setControlledPilotFallback(CONTROLLED_PILOT_FALLBACK_COPY);
      return;
    }

    setControlledPilotSubmitting(true);
    setControlledPilotError(null);
    setControlledPilotFallback(null);
    setControlledPilotEligibility(null);
    setControlledPilotResult(null);
    setControlledPilotSourceSessionId(null);
    setControlledPilotPromotionReviewOpen(false);
    setControlledPilotPromotionError(null);

    try {
      const sourceSessionId = currentSessionIdRef.current;
      const eligibility = await checkControlledChatPilotEligibility();
      setControlledPilotEligibility(eligibility);

      if (!eligibility.eligible) {
        setControlledPilotError("Controlled Pilot blocked.");
        setControlledPilotFallback(CONTROLLED_PILOT_FALLBACK_COPY);
        return;
      }

      const output = await runMultiStrategyAgentPreview({
        sessionId: `chat-controlled-pilot-${Date.now()}`,
        userText: trimmedInput,
        toolsPrompt: CHAT_PREVIEW_NO_TOOLS_PROMPT,
        allowPlanning: false,
        localModelAvailable: false,
        layer: "L2",
        executionBudget: {
          allowWrites: false,
        },
      });
      setControlledPilotSourceSessionId(sourceSessionId);
      setControlledPilotResult(output);
    } catch (e) {
      setControlledPilotSourceSessionId(null);
      setControlledPilotError(`Controlled Pilot failed: ${readablePreviewError(e)}`);
      setControlledPilotFallback(CONTROLLED_PILOT_FALLBACK_COPY);
    } finally {
      setControlledPilotSubmitting(false);
    }
  }, [input]);

  const handleConfirmControlledPilotPromotion = useCallback(async () => {
    if (
      !hasPromotablePilotResponse(controlledPilotResult) ||
      !currentSessionId ||
      controlledPilotPromoting
    ) {
      return;
    }
    const targetSessionId = currentSessionId;
    const promotionKey = getPilotPromotionKey(controlledPilotResult);
    if (
      !promotionKey ||
      promotedControlledPilotKeysRef.current[promotionKey] ||
      inFlightControlledPilotPromotionKeysRef.current.has(promotionKey)
    ) {
      return;
    }

    if (!controlledPilotSourceSessionId || controlledPilotSourceSessionId !== targetSessionId) {
      setControlledPilotPromotionReviewOpen(true);
      setControlledPilotPromotionError(
        buildControlledPilotSourceMismatchMessage(controlledPilotSourceSessionId, targetSessionId)
      );
      setControlledPilotFallback(CONTROLLED_PILOT_RERUN_COPY);
      return;
    }

    const assistantMsg: ChatMessage = {
      role: "assistant",
      content: controlledPilotResult.userOutput.trim(),
      ...(controlledPilotResult.runId ? { run_id: controlledPilotResult.runId } : {}),
    };
    const evidenceInput: ControlledPilotPromotionEvidenceInput = {
      pilotRunId: controlledPilotResult.runId,
      sourceSessionId: controlledPilotSourceSessionId,
      targetSessionId,
      strategyKind: controlledPilotResult.strategyKind,
      payloadKind: controlledPilotResult.payloadKind,
      governanceDecisionKind: controlledPilotResult.governanceDecisionKind ?? "unknown",
      promotedMessageLength: assistantMsg.content.length,
      promotedMessageHash: checksumText(assistantMsg.content),
      promotedAt: new Date().toISOString(),
    };

    inFlightControlledPilotPromotionKeysRef.current.add(promotionKey);
    setControlledPilotPromoting(true);
    setControlledPilotPromotionError(null);
    setControlledPilotFallback(null);
    try {
      const messageAlreadySaved = Boolean(
        savedControlledPilotPromotionKeysRef.current[promotionKey]
      );
      if (!messageAlreadySaved) {
        await saveChatMessage(targetSessionId, assistantMsg);
        if (currentSessionIdRef.current === targetSessionId) {
          setMessages(prev => [...prev, assistantMsg]);
        }
        setSavedControlledPilotPromotionKeys(prev => {
          const next = { ...prev, [promotionKey]: true };
          savedControlledPilotPromotionKeysRef.current = next;
          return next;
        });
      }
      await recordControlledPilotPromotionEvidence(evidenceInput);
      setPromotedControlledPilotKeys(prev => {
        const next = { ...prev, [promotionKey]: true };
        promotedControlledPilotKeysRef.current = next;
        return next;
      });
      setControlledPilotPromotionReviewOpen(false);
      await loadSessions();
    } catch (e) {
      if (savedControlledPilotPromotionKeysRef.current[promotionKey]) {
        setControlledPilotPromotionError(
          `Promotion evidence recording failed: ${readablePreviewError(e)}. Retry will only record evidence; it will not save another chat message.`
        );
      } else {
        setControlledPilotPromotionError(`Promotion failed: ${readablePreviewError(e)}`);
      }
    } finally {
      inFlightControlledPilotPromotionKeysRef.current.delete(promotionKey);
      setControlledPilotPromoting(false);
    }
  }, [
    controlledPilotPromoting,
    controlledPilotResult,
    controlledPilotSourceSessionId,
    currentSessionId,
  ]);

  const handleIndexMemory = useCallback(
    async (content: string) => {
      emitCompanionStage(isSafeMode(diagnostics) ? "privacy" : "memory");
      if (isSafeMode(diagnostics)) {
        setMessages(prev => [
          ...prev,
          {
            role: "assistant",
            content: `当前处于 Safe Mode，${getSafeModeReason(diagnostics)} 建议先去设置页恢复控制台处理数据风险，再执行"加入记忆"。`,
          },
        ]);
        return;
      }
      try {
        await indexMemoryChunk(currentSessionId, content, "chat");
      } catch (e) {
        console.error("加入记忆失败", e);
      }
    },
    [diagnostics, currentSessionId, emitCompanionStage]
  );

  const handleInputChange = useCallback(
    (value: string) => {
      setInput(value);
      emitCompanionStage(inferStageFromText(value) ?? "listening");
    },
    [emitCompanionStage]
  );

  const buildAssistantActionPrompt = useCallback(
    (kind: "continue" | "action" | "state" | "goal", content: string) => {
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
    },
    []
  );

  const handleFeedback = useCallback(
    async (index: number, type: "up" | "down") => {
      const msg = messages[index];
      if (!msg || msg.role !== "assistant") return;
      try {
        await saveFeedback(currentSessionId, index, type, msg.content.slice(0, 200));
        setFeedbackGiven(prev => ({ ...prev, [index]: type }));
      } catch (e) {
        console.error("反馈保存失败", e);
      }
    },
    [messages, currentSessionId]
  );

  return (
    <div
      data-testid={companionMode ? "companion-chat-runtime" : "chat-page"}
      className={classNames("h-full flex", companionMode ? "bg-transparent" : "bg-white")}
    >
      {!companionMode && (
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
      )}

      {/* Chat area */}
      <div className="flex min-w-0 flex-1 flex-col">
        {companionMode ? (
          <div className="flex min-h-[88px] shrink-0 items-center border-b border-stone-200 bg-[#fffefa] px-6 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-3">
                <div
                  aria-hidden="true"
                  className="flex h-11 w-11 items-center justify-center rounded-lg bg-stone-900 text-base font-bold text-white"
                >
                  O
                </div>
                <div>
                  <div className="text-base font-semibold leading-5 text-stone-950">OpenLife</div>
                  <div className="mt-1 text-sm font-medium leading-4 text-stone-500">在线</div>
                </div>
              </div>
              <div className="mt-2 flex flex-wrap gap-2 text-xs font-medium text-stone-600">
                {[
                  companionRouteStatus(diagnostics),
                  companionLifeModelStatus(diagnostics),
                  companionPendingStatus(diagnostics, pendingProposals),
                ].map(label => (
                  <span
                    key={label}
                    className="inline-flex h-6 items-center rounded-md border border-stone-200 bg-white px-2"
                  >
                    {label}
                  </span>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <>
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
                      本地：{diagnostics.resolved_local_model || diagnostics.local_model} · 云端
                      API：
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
          </>
        )}
        {!companionMode && diagnostics && isSafeMode(diagnostics) && (
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
        {/* Pending Proposals Alert */}
        {!companionMode && pendingProposals.length > 0 && (
          <div className="border-b border-indigo-100 bg-indigo-50 px-6 py-2">
            <div className="max-w-3xl text-xs text-indigo-800 flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <ShieldCheck size={14} />
                <span className="font-medium">{pendingProposals.length} 个待确认</span>
                <span className="text-indigo-600">
                  （{pendingProposals[0].affectedPath || pendingProposals[0].proposalType}）
                </span>
              </div>
              <Link to="/mailbox" className="underline font-medium">
                去邮箱确认
              </Link>
            </div>
          </div>
        )}

        {/* Chat mode selector */}
        <div className={companionMode ? "hidden" : "border-b bg-white px-6 py-2"}>
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
        <div
          className={
            companionMode ? "hidden" : "border-b border-amber-100 bg-amber-50/60 px-6 py-2"
          }
        >
          <div className="max-w-4xl space-y-3">
            <button
              type="button"
              onClick={() => setGovernedPreviewOpen(value => !value)}
              aria-expanded={governedPreviewOpen}
              className="flex w-full items-center justify-between gap-3 text-left"
            >
              <span className="flex min-w-0 items-center gap-2">
                {governedPreviewOpen ? (
                  <ChevronDown size={15} className="shrink-0 text-amber-700" />
                ) : (
                  <ChevronRight size={15} className="shrink-0 text-amber-700" />
                )}
                <span className="min-w-0">
                  <span className="block text-xs font-semibold text-amber-950">
                    Governed Preview
                  </span>
                  <span className="block text-[11px] leading-4 text-amber-800">
                    Preview/Beta · write-disabled runtime check, separate from normal Chat
                  </span>
                </span>
              </span>
              <span className="shrink-0 rounded-full bg-white px-2 py-0.5 text-[10px] font-medium text-amber-800 ring-1 ring-amber-200">
                Preview/Beta
              </span>
            </button>

            {governedPreviewOpen && (
              <div className="space-y-3 rounded-lg border border-amber-200 bg-white p-3">
                <div className="flex items-start gap-2 text-xs leading-5 text-amber-900">
                  <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                  <div>
                    This preview uses the current chat draft only after explicit trigger. It forces
                    external writes off and shows metadata-safe governance output only.
                  </div>
                </div>

                <div className="grid gap-2 md:grid-cols-3">
                  <label className="block">
                    <span className="text-[11px] font-medium text-stone-600">Layer</span>
                    <select
                      value={governedPreviewLayer}
                      onChange={event =>
                        setGovernedPreviewLayer(
                          event.target.value as MultiStrategyAgentPreviewLayer
                        )
                      }
                      className="mt-1 w-full rounded-md border border-stone-200 px-2 py-1.5 text-xs text-stone-800 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                    >
                      <option value="L1">L1</option>
                      <option value="L2">L2</option>
                      <option value="L3">L3</option>
                    </select>
                  </label>

                  <label className="flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-xs text-stone-700">
                    <input
                      type="checkbox"
                      checked={governedPreviewAllowPlanning}
                      onChange={event => setGovernedPreviewAllowPlanning(event.target.checked)}
                      className="rounded border-stone-300"
                    />
                    <span>Allow planning</span>
                  </label>

                  <label className="flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-xs text-stone-700">
                    <input
                      type="checkbox"
                      checked={governedPreviewLocalModelAvailable}
                      onChange={event =>
                        setGovernedPreviewLocalModelAvailable(event.target.checked)
                      }
                      className="rounded border-stone-300"
                    />
                    <span>Local model available</span>
                  </label>
                </div>

                <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-emerald-100 bg-emerald-50 px-3 py-2 text-xs text-emerald-900">
                  <div className="flex items-center gap-2">
                    <ShieldCheck size={14} />
                    <span>No external write operations. No default full tools catalog.</span>
                  </div>
                  <button
                    type="button"
                    onClick={handleRunGovernedPreview}
                    disabled={governedPreviewSubmitting || !input.trim()}
                    className={classNames(
                      "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium",
                      governedPreviewSubmitting || !input.trim()
                        ? "bg-stone-200 text-stone-500"
                        : "bg-stone-900 text-white hover:bg-stone-800"
                    )}
                  >
                    {governedPreviewSubmitting && <Loader2 size={13} className="animate-spin" />}
                    Run Governed Preview
                  </button>
                </div>

                <div className="space-y-2 rounded-md border border-sky-100 bg-sky-50 px-3 py-2 text-xs text-sky-950">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-2">
                      <ShieldCheck size={14} className="shrink-0 text-sky-700" />
                      <div>
                        <div className="font-semibold">Controlled Pilot</div>
                        <div className="text-[11px] leading-4 text-sky-800">
                          Eligibility-gated single turn. Normal Send remains unchanged.
                        </div>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={handleRunControlledPilot}
                      disabled={controlledPilotSubmitting || !input.trim()}
                      className={classNames(
                        "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium",
                        controlledPilotSubmitting || !input.trim()
                          ? "bg-sky-100 text-sky-400"
                          : "bg-sky-900 text-white hover:bg-sky-800"
                      )}
                    >
                      {controlledPilotSubmitting && <Loader2 size={13} className="animate-spin" />}
                      Run Controlled Pilot
                    </button>
                  </div>

                  {controlledPilotEligibility && (
                    <div className="rounded-md border border-sky-100 bg-white px-3 py-2 text-sky-900">
                      Eligibility: {controlledPilotEligibility.cleanRunCount}/
                      {controlledPilotEligibility.requiredCleanRuns} clean preview runs
                    </div>
                  )}

                  {controlledPilotError && (
                    <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-red-700">
                      {controlledPilotError}
                    </div>
                  )}

                  {controlledPilotEligibility &&
                    !controlledPilotEligibility.eligible &&
                    controlledPilotEligibility.blockingReasons.length > 0 && (
                      <div className="rounded-md border border-amber-100 bg-amber-50 px-3 py-2 text-amber-900">
                        <div className="font-medium">Blocking reasons</div>
                        <ul className="mt-1 list-disc space-y-1 pl-4">
                          {controlledPilotEligibility.blockingReasons.map(reason => (
                            <li key={reason}>{reason}</li>
                          ))}
                        </ul>
                      </div>
                    )}

                  {controlledPilotFallback && (
                    <div className="rounded-md border border-stone-200 bg-white px-3 py-2 text-stone-700">
                      {controlledPilotFallback}
                    </div>
                  )}

                  {controlledPilotResult && (
                    <div
                      data-testid="controlled-pilot-response"
                      className="space-y-3 rounded-lg border border-sky-200 bg-white p-3"
                    >
                      <div className="flex flex-wrap items-center justify-between gap-3">
                        <div>
                          <div className="text-xs font-semibold text-stone-900">Pilot response</div>
                          <div className="mt-0.5 text-[11px] text-stone-500">
                            {controlledPilotPromoted
                              ? "Promoted to chat history as a normal assistant message with metadata-safe evidence."
                              : controlledPilotPromotionMessageSaved
                                ? "Message saved. Promotion evidence is degraded until the recorder succeeds."
                                : "Separate pilot output. It is not saved as a normal assistant message."}
                          </div>
                        </div>
                        <div className="flex flex-wrap items-center gap-2">
                          {controlledPilotPromoted && (
                            <span className="inline-flex items-center gap-1.5 rounded-md bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-800 ring-1 ring-emerald-100">
                              <CheckCircle2 size={13} />
                              Promoted to chat history
                            </span>
                          )}
                          {!controlledPilotPromoted && controlledPilotCanPromote && (
                            <button
                              type="button"
                              onClick={() => {
                                setControlledPilotPromotionReviewOpen(true);
                                setControlledPilotPromotionError(
                                  controlledPilotSessionMismatch
                                    ? controlledPilotSessionBlockingMessage
                                    : null
                                );
                                setControlledPilotFallback(
                                  controlledPilotSessionMismatch
                                    ? CONTROLLED_PILOT_RERUN_COPY
                                    : null
                                );
                              }}
                              className="inline-flex items-center gap-1.5 rounded-md bg-emerald-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-800"
                            >
                              <CheckCircle2 size={13} />
                              Promote Pilot Response
                            </button>
                          )}
                          {controlledPilotResult.runId && (
                            <Link
                              to={`/runs/${controlledPilotResult.runId}`}
                              className="inline-flex items-center gap-1.5 rounded-md bg-sky-50 px-3 py-1.5 text-xs font-medium text-sky-800 ring-1 ring-sky-100 hover:bg-sky-100"
                            >
                              <ExternalLink size={13} />
                              View Run Trace
                            </Link>
                          )}
                        </div>
                      </div>

                      <div className="whitespace-pre-wrap rounded-md bg-stone-50 px-3 py-2 text-sm leading-6 text-stone-900">
                        {controlledPilotResult.userOutput ?? "No pilot response returned."}
                      </div>

                      <div className="grid gap-2 text-xs md:grid-cols-2">
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          <div className="text-[10px] uppercase text-stone-400">runId</div>
                          <div className="mt-1 font-mono text-stone-900">
                            {controlledPilotResult.runId ?? "not returned"}
                          </div>
                        </div>
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          Strategy: {controlledPilotResult.strategyKind}
                        </div>
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          Payload: {controlledPilotResult.payloadKind}
                        </div>
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          Governance: {controlledPilotResult.governanceDecisionKind ?? "unknown"}
                        </div>
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          Source session: {controlledPilotSourceSessionId ?? "unknown"}
                        </div>
                        <div className="rounded-md bg-stone-50 px-3 py-2 text-stone-700">
                          Target session: {currentSessionId || "unknown"}
                        </div>
                      </div>

                      {controlledPilotSummaryEntries.length > 0 && (
                        <div className="flex flex-wrap gap-2 text-xs">
                          {controlledPilotSummaryEntries.map(([key, value]) => (
                            <span
                              key={key}
                              className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                            >
                              {key}: {value}
                            </span>
                          ))}
                        </div>
                      )}

                      {controlledPilotPromotionError && (
                        <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-700">
                          {controlledPilotPromotionError}
                        </div>
                      )}

                      {controlledPilotPromotionReviewOpen &&
                        controlledPilotCanPromote &&
                        !controlledPilotPromoted && (
                          <div className="space-y-3 rounded-md border border-emerald-200 bg-emerald-50 px-3 py-3 text-xs text-emerald-950">
                            <div>
                              <div className="font-semibold">Review pilot promotion</div>
                              <div className="mt-0.5 text-emerald-800">
                                确认后将写入当前聊天历史，成为普通 assistant message。
                              </div>
                            </div>

                            {controlledPilotSessionMismatch && !controlledPilotPromotionError && (
                              <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-amber-900">
                                {controlledPilotSessionBlockingMessage}
                              </div>
                            )}

                            <div>
                              <div className="text-[10px] uppercase text-emerald-700">
                                Pilot response text
                              </div>
                              <div className="mt-1 whitespace-pre-wrap rounded-md bg-white px-3 py-2 text-sm leading-6 text-stone-900">
                                {controlledPilotResult.userOutput?.trim()}
                              </div>
                            </div>

                            <div className="grid gap-2 md:grid-cols-2">
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">
                                  Source session
                                </div>
                                <div className="mt-1 font-mono text-stone-900">
                                  {controlledPilotSourceSessionId ?? "unknown"}
                                </div>
                              </div>
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">
                                  Target session
                                </div>
                                <div className="mt-1 font-mono text-stone-900">
                                  {currentSessionId || "unknown"}
                                </div>
                              </div>
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">runId</div>
                                <div className="mt-1 font-mono text-stone-900">
                                  {controlledPilotResult.runId ?? "not returned"}
                                </div>
                              </div>
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">
                                  Selected strategy
                                </div>
                                <div className="mt-1 text-stone-900">
                                  {controlledPilotResult.strategyKind}
                                </div>
                              </div>
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">
                                  Governance summary
                                </div>
                                <div className="mt-1 text-stone-900">
                                  {controlledPilotGovernanceSummary}
                                </div>
                              </div>
                              <div className="rounded-md bg-white px-3 py-2">
                                <div className="text-[10px] uppercase text-emerald-700">
                                  Payload summary
                                </div>
                                <div className="mt-1 text-stone-900">
                                  {controlledPilotPayloadSummary}
                                </div>
                              </div>
                            </div>

                            <div className="flex flex-wrap justify-end gap-2">
                              <button
                                type="button"
                                onClick={() => {
                                  setControlledPilotPromotionReviewOpen(false);
                                  setControlledPilotPromotionError(null);
                                }}
                                disabled={controlledPilotPromoting}
                                className="rounded-md border border-emerald-200 bg-white px-3 py-1.5 text-xs font-medium text-emerald-900 hover:bg-emerald-100 disabled:opacity-50"
                              >
                                Cancel Promotion
                              </button>
                              <button
                                type="button"
                                onClick={handleConfirmControlledPilotPromotion}
                                disabled={controlledPilotPromoting}
                                className="inline-flex items-center gap-1.5 rounded-md bg-emerald-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-800 disabled:opacity-50"
                              >
                                {controlledPilotPromoting && (
                                  <Loader2 size={13} className="animate-spin" />
                                )}
                                Confirm Promotion
                              </button>
                            </div>
                          </div>
                        )}
                    </div>
                  )}
                </div>

                {governedPreviewError && (
                  <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-700">
                    {governedPreviewError}
                  </div>
                )}

                {governedPreviewResult && (
                  <div className="space-y-3 rounded-lg border border-stone-200 bg-stone-50 p-3">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div>
                        <div className="text-xs font-semibold text-stone-900">Preview result</div>
                        <div className="mt-0.5 text-[11px] text-stone-500">
                          Metadata-safe fields only. Raw prompts, memory context, PII, mail bodies,
                          and file content are not rendered here.
                        </div>
                      </div>
                      {governedPreviewResult.runId && (
                        <Link
                          to={`/runs/${governedPreviewResult.runId}`}
                          className="inline-flex items-center gap-1.5 rounded-md bg-white px-3 py-1.5 text-xs font-medium text-stone-700 ring-1 ring-stone-200 hover:bg-stone-100"
                        >
                          <ExternalLink size={13} />
                          View Run Trace
                        </Link>
                      )}
                    </div>

                    <div className="grid gap-2 text-xs md:grid-cols-2">
                      <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                        <div className="text-[10px] uppercase text-stone-400">runId</div>
                        <div className="mt-1 font-mono text-stone-900">
                          {governedPreviewResult.runId ?? "not returned"}
                        </div>
                      </div>
                      <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                        Strategy: {governedPreviewResult.strategyKind}
                      </div>
                      <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                        Payload: {governedPreviewResult.payloadKind}
                      </div>
                      <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                        Governance: {governedPreviewResult.governanceDecisionKind ?? "unknown"}
                      </div>
                    </div>

                    {governedPreviewSummaryEntries.length > 0 && (
                      <div className="flex flex-wrap gap-2 text-xs">
                        {governedPreviewSummaryEntries.map(([key, value]) => (
                          <span
                            key={key}
                            className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                          >
                            {key}: {value}
                          </span>
                        ))}
                      </div>
                    )}

                    <div>
                      <div className="text-xs font-medium text-stone-700">Warnings</div>
                      {governedPreviewResult.warnings.length > 0 ? (
                        <div className="mt-1 space-y-1">
                          {governedPreviewResult.warnings.map(warning => (
                            <div
                              key={warning}
                              className="rounded-md border border-amber-100 bg-amber-50 px-2 py-1 text-xs text-amber-800"
                            >
                              {warning}
                            </div>
                          ))}
                        </div>
                      ) : (
                        <div className="mt-1 text-xs text-stone-500">No warnings returned.</div>
                      )}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
        <div
          className={
            companionMode
              ? "flex-1 space-y-7 overflow-auto bg-[#fffefa] px-6 py-8"
              : "flex-1 space-y-4 overflow-auto px-6 py-4"
          }
        >
          {!companionMode && !loadingHistory && (
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
          {!companionMode && reasoningTrace && (
            <div className="flex justify-start">
              <ReasoningTracePanel
                trace={reasoningTrace}
                show={showReasoningTrace}
                onToggle={() => setShowReasoningTrace(s => !s)}
              />
            </div>
          )}
          {!companionMode && toolCalls.length > 0 && (
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
          {!companionMode && !loadingHistory && messages.length <= 1 && !sending && (
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
          {!companionMode && showGuide && getModelEmptyState(model, diagnostics) && (
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
            <div
              key={i}
              className={classNames(
                "flex",
                m.role === "user" ? "justify-end" : "justify-start",
                companionMode && "items-start gap-3"
              )}
            >
              {companionMode && m.role === "assistant" && (
                <div
                  aria-hidden="true"
                  className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-stone-900 text-sm font-bold text-white"
                >
                  O
                </div>
              )}
              <div
                data-testid={m.role === "assistant" ? "assistant-message" : "user-message"}
                className={classNames(
                  "max-w-2xl rounded-xl px-4 py-3 text-sm",
                  companionMode
                    ? m.role === "user"
                      ? "rounded-tr-none bg-stone-900 text-white"
                      : "rounded-tl-none border border-stone-200 bg-white text-stone-900"
                    : m.role === "user"
                      ? "rounded-br-none bg-indigo-600 text-white"
                      : "rounded-bl-none bg-gray-100 text-gray-800"
                )}
              >
                <div className="whitespace-pre-wrap">{m.content}</div>
                {m.role === "assistant" && companionMode && m.run_id && (
                  <div className="mt-3 border-t border-stone-100 pt-2">
                    <button
                      type="button"
                      onClick={() =>
                        setOpenEvidenceRunId(current =>
                          current === m.run_id ? null : (m.run_id ?? null)
                        )
                      }
                      className="text-xs font-semibold text-stone-600 underline-offset-4 hover:text-stone-950 hover:underline"
                    >
                      查看依据
                    </button>
                    {openEvidenceRunId === m.run_id && (
                      <div className="mt-2 rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-700">
                        {companionRunSummary(
                          currentRun && currentRun.id === m.run_id ? currentRun : null
                        ).map(line => (
                          <div key={line}>{line}</div>
                        ))}
                        <Link
                          to={`/runs/${m.run_id}`}
                          className="mt-2 inline-flex font-semibold text-stone-900 underline-offset-4 hover:underline"
                        >
                          查看完整记录
                        </Link>
                      </div>
                    )}
                  </div>
                )}
                {m.role === "assistant" && !companionMode && (
                  <div className="mt-3 space-y-2">
                    {(() => {
                      const runMatches = m.run_id && currentRun && currentRun.id === m.run_id;
                      const isLast = i === messages.length - 1;
                      if ((!runMatches && !isLast) || !currentRun) return null;
                      const run = currentRun;
                      return (
                        <>
                          <div className="flex items-center gap-3 text-[10px] text-gray-400">
                            <span className="flex items-center gap-1">
                              <Activity size={10} />
                              {run.modelRoute?.provider || "unknown"}
                              {run.modelRoute?.preferLocal && " (local)"}
                            </span>
                            <span>{run.actions?.length || 0} 工具</span>
                            <span>{run.generatedProposals?.length || 0} 待确认</span>
                            {run.modelRoute?.fallbackReason && (
                              <span className="text-amber-500">fallback</span>
                            )}
                          </div>
                          {/* Execution summary line */}
                          <div className="flex items-center gap-2">
                            <span
                              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium ${
                                run.status === "completed"
                                  ? "bg-green-50 text-green-700"
                                  : run.status === "waiting_permission"
                                    ? "bg-amber-50 text-amber-700"
                                    : run.status === "failed"
                                      ? "bg-red-50 text-red-700"
                                      : "bg-blue-50 text-blue-700"
                              }`}
                            >
                              {run.status === "completed" && (
                                <>
                                  <CheckCircle2 size={12} />
                                  已完成 · {run.stepCount || 0}步 · {run.toolCallCount || 0}工具
                                </>
                              )}
                              {run.status === "waiting_permission" && (
                                <>
                                  <ShieldCheck size={12} />
                                  等待确认 ·{" "}
                                  {run.actions?.filter(
                                    (a: any) => a.status === "needs_confirmation"
                                  ).length || 0}
                                  个权限请求
                                </>
                              )}
                              {run.status === "failed" && (
                                <>
                                  <XCircle size={12} />
                                  失败 · {run.error?.phase || "unknown"}
                                </>
                              )}
                              {run.status === "running" && (
                                <>
                                  <Loader2 size={12} className="animate-spin" />
                                  运行中...
                                </>
                              )}
                            </span>
                          </div>
                        </>
                      );
                    })()}
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
                      {!companionMode && (
                        <>
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
                        </>
                      )}
                    </div>
                    <div className="flex items-center justify-end gap-2">
                      <button
                        onClick={() => handleFeedback(i, "up")}
                        className={`transition ${
                          feedbackGiven[i] === "up"
                            ? "text-green-600"
                            : "text-gray-500 hover:text-green-600"
                        }`}
                        title="有帮助"
                      >
                        <ThumbsUp size={14} />
                      </button>
                      <button
                        onClick={() => handleFeedback(i, "down")}
                        className={`transition ${
                          feedbackGiven[i] === "down"
                            ? "text-red-600"
                            : "text-gray-500 hover:text-red-600"
                        }`}
                        title="没帮助"
                      >
                        <ThumbsDown size={14} />
                      </button>
                    </div>
                  </div>
                )}
              </div>
              {companionMode && m.role === "user" && (
                <div
                  aria-hidden="true"
                  className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-stone-100 text-sm font-semibold text-stone-700"
                >
                  你
                </div>
              )}
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
              <div className="bg-gray-100 text-gray-800 px-4 py-3 rounded-xl rounded-bl-none text-sm">
                <AgentStateIndicator
                  sessionId={currentSessionId}
                  runId={currentRunId || undefined}
                  isActive={sending}
                />
              </div>
            </div>
          )}
          {!companionMode && currentAgentState && (
            <div className="px-4 py-2">
              <AgentControlPlane
                state={currentAgentState}
                busy={agentTaskControlBusy}
                canResume={Boolean(currentAgentTaskState?.canResume)}
                canRetry={Boolean(currentAgentTaskState?.canRetry)}
                canCancel={Boolean(currentAgentTaskState?.canCancel)}
                eventStream={agentEventStreamState ?? undefined}
                onResume={handleResumeMainChatTask}
                onRetry={handleRetryMainChatAction}
                onCancel={handleCancelMainChatTask}
                onApproveOnce={handleApproveOnceMainChatPermission}
                onDeny={handleDenyMainChatControl}
                onDefer={handleDeferMainChatControl}
                onAcceptProposal={handleAcceptAgentProposal}
                onRejectProposal={handleRejectAgentProposal}
                onEditProposal={handleEditAgentProposal}
                onRollbackMemory={handleRollbackMemory}
                onConfirmPlan={handleConfirmPlan}
                onEditPlanStep={handleEditPlanStep}
                onExecutePlanStep={handleExecutePlanStep}
                onSkipPlanStep={handleSkipPlanStep}
                onCancelPlan={handleCancelPlan}
                onReviewPlan={handleReviewPlan}
              />
              {agentTaskControlError && (
                <div className="border-b border-rose-200 bg-rose-50 px-4 py-2 text-xs text-rose-800">
                  {agentTaskControlError}
                </div>
              )}
            </div>
          )}
          {!companionMode && !currentAgentState && currentAgentIngress && (
            <div className="px-4 py-2">
              <div
                data-testid="agent-diagnostic-task-shell"
                data-agent-state-source="ingress-task-state-diagnostic"
                className="border-y border-stone-200 bg-stone-50/75 px-3 py-3 text-xs text-stone-700"
              >
                <div className="flex flex-wrap items-start gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="inline-flex h-6 items-center rounded-md bg-stone-900 px-2 font-semibold text-white">
                        Diagnostic task shell
                      </span>
                      <span className="inline-flex h-6 items-center rounded-md border border-stone-300 bg-white px-2 font-semibold text-stone-800">
                        {formatMainChatStrategy(currentAgentIngress.selectedStrategy)}
                      </span>
                      <span className="font-medium">
                        {currentAgentIngress.privacyRisk.riskLevel} risk
                      </span>
                      <span className="text-stone-500">
                        {Math.round(currentAgentIngress.confidence * 100)}% confidence
                      </span>
                      {currentAgentIngress.agentTaskSessionId && (
                        <span className="text-stone-500">
                          Session {currentAgentIngress.agentTaskSessionId.slice(-8)}
                        </span>
                      )}
                      {currentAgentTaskState?.session?.status && (
                        <span className="inline-flex h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-700">
                          {currentAgentTaskState.session.status.replace(/_/g, " ")}
                        </span>
                      )}
                      {currentAgentTaskState && (
                        <>
                          <span className="text-stone-500">
                            {currentAgentTaskState.activeToolCount} active
                          </span>
                          <span className="text-stone-500">
                            {currentAgentTaskState.pendingApprovalCount} pending
                          </span>
                        </>
                      )}
                    </div>
                    <div className="mt-2 border-l-2 border-stone-300 bg-white/70 px-2 py-1 text-stone-700">
                      Typed AgentControlPlane snapshot is not available yet; this shell is derived
                      from AgentIngress and task detail only.
                    </div>
                    <div className="mt-2 grid gap-2 md:grid-cols-2">
                      {currentAgentTaskState?.session?.userGoal && (
                        <div className="min-w-0">
                          <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                            Goal
                          </div>
                          <div className="truncate text-stone-900">
                            {currentAgentTaskState.session.userGoal}
                          </div>
                        </div>
                      )}
                      {currentAgentTaskState?.session?.currentPlanSummary && (
                        <div className="min-w-0">
                          <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                            Current plan
                          </div>
                          <div className="truncate text-stone-900">
                            {currentAgentTaskState.session.currentPlanSummary}
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                  {currentAgentTaskState && (
                    <div className="ml-auto flex shrink-0 items-center gap-1">
                      <button
                        type="button"
                        aria-label="Resume task"
                        title="Resume task"
                        disabled={!currentAgentTaskState.canResume || agentTaskControlBusy}
                        onClick={handleResumeMainChatTask}
                        className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <Play size={14} />
                      </button>
                      <button
                        type="button"
                        aria-label="Retry failed action"
                        title="Retry failed action"
                        disabled={!currentAgentTaskState.canRetry || agentTaskControlBusy}
                        onClick={() => handleRetryMainChatAction()}
                        className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <RotateCw size={14} />
                      </button>
                      <button
                        type="button"
                        aria-label="Cancel task"
                        title="Cancel task"
                        disabled={!currentAgentTaskState.canCancel || agentTaskControlBusy}
                        onClick={handleCancelMainChatTask}
                        className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <Ban size={14} />
                      </button>
                    </div>
                  )}
                </div>
                {legacyFallbackUsed && (
                  <div className="mt-2 border-l-2 border-amber-400 bg-amber-50 px-2 py-1 text-amber-900">
                    <span className="font-semibold">Fallback notice</span>: response used the
                    visible legacy fallback path.
                  </div>
                )}
                {agentTaskControlError && (
                  <div className="mt-2 border-l-2 border-rose-400 bg-rose-50 px-2 py-1 text-rose-900">
                    {agentTaskControlError}
                  </div>
                )}
                {currentAgentTaskState?.actions?.length ? (
                  <div className="mt-3">
                    <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                      Diagnostic queue preview
                    </div>
                    <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white/75">
                      {currentAgentTaskState.actions.map(action => {
                        const metadataEntries = formatMainChatMetadataEntries(
                          action.observationMetadata
                        );
                        const needsReview =
                          action.policy.requiresProposal || action.policy.requiresConfirmation;
                        return (
                          <div key={action.id} className="grid gap-2 py-2 md:grid-cols-[1fr_auto]">
                            <div className="min-w-0 px-2">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="font-semibold text-stone-950">
                                  {action.action.actionType}
                                </span>
                                <span
                                  className={classNames(
                                    "inline-flex h-5 items-center rounded-md border px-1.5 font-medium",
                                    mainChatActionStatusClass(action.status)
                                  )}
                                >
                                  {action.status.replace(/_/g, " ")}
                                </span>
                                <span className="text-stone-500">{action.policy.reasonCode}</span>
                                {action.policy.requiresProposal && (
                                  <span className="inline-flex h-5 items-center rounded-md border border-amber-200 bg-amber-50 px-1.5 font-medium text-amber-900">
                                    Proposal required
                                  </span>
                                )}
                                {action.policy.requiresConfirmation && (
                                  <span className="inline-flex h-5 items-center rounded-md border border-amber-200 bg-amber-50 px-1.5 font-medium text-amber-900">
                                    Permission required
                                  </span>
                                )}
                                {needsReview && (
                                  <Link
                                    to="/review"
                                    state={{
                                      mainChatTaskSessionId:
                                        currentAgentTaskState?.session?.id ??
                                        currentAgentIngress.agentTaskSessionId,
                                      returnTo: "/chat",
                                    }}
                                    className="inline-flex h-5 items-center gap-1 rounded-md border border-stone-200 bg-white px-1.5 font-medium text-stone-800 hover:bg-stone-100"
                                  >
                                    <ExternalLink size={12} />
                                    Open Review Center
                                  </Link>
                                )}
                              </div>
                              <div className="mt-1 text-stone-700">{action.action.description}</div>
                              {metadataEntries.length > 0 && (
                                <div className="mt-1 flex flex-wrap gap-1">
                                  {metadataEntries.map(entry => (
                                    <span
                                      key={`${action.id}-${entry}`}
                                      className="inline-flex h-5 items-center rounded-md bg-stone-100 px-1.5 text-stone-600"
                                    >
                                      {entry}
                                    </span>
                                  ))}
                                </div>
                              )}
                              {action.error && (
                                <div className="mt-1 text-rose-700">{action.error}</div>
                              )}
                            </div>
                            <div className="px-2 text-right text-stone-500">
                              attempt {action.attempts}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ) : null}
                {currentAgentTaskState?.session?.pendingBlockers?.length ? (
                  <div className="mt-3">
                    <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                      Pending blockers
                    </div>
                    <div className="flex flex-wrap gap-1">
                      {currentAgentTaskState.session.pendingBlockers.map(blocker => (
                        <span
                          key={blocker}
                          className="inline-flex h-6 items-center rounded-md border border-amber-200 bg-amber-50 px-2 font-medium text-amber-900"
                        >
                          {blocker}
                        </span>
                      ))}
                    </div>
                  </div>
                ) : null}
                {currentExecutionTranscript.length > 0 && (
                  <div className="mt-3">
                    <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                      Diagnostic transcript
                    </div>
                    <div className="grid gap-1 sm:grid-cols-2">
                      {currentExecutionTranscript.slice(-4).map(entry => (
                        <div
                          key={entry.id}
                          className="min-w-0 border-l border-stone-300 bg-white/70 px-2 py-1"
                        >
                          <div className="flex items-center gap-2">
                            <span className="shrink-0 font-semibold text-stone-900">
                              {formatTranscriptKind(entry.kind)}
                            </span>
                            <span className="truncate text-stone-600">{entry.summary}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>
          )}
          {!companionMode && skillToolSurfaceAvailable && (
            <section className="px-4 py-2" aria-label="Skills and tools">
              <div className="border-y border-stone-200 bg-white px-3 py-3 text-xs text-stone-700">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="inline-flex h-6 items-center gap-1 rounded-md bg-stone-950 px-2 font-semibold text-white">
                        <Hammer size={13} />
                        Skills & tools
                      </span>
                      {selectedSkillId.trim() ? (
                        <span className="inline-flex h-6 max-w-full items-center rounded-md border border-emerald-200 bg-emerald-50 px-2 font-medium text-emerald-900">
                          <span className="truncate">Selected {selectedSkillId.trim()}</span>
                        </span>
                      ) : (
                        <span className="inline-flex h-6 items-center rounded-md border border-stone-200 bg-stone-50 px-2 font-medium text-stone-600">
                          No selected skill
                        </span>
                      )}
                      {selectedSkillEvidence?.selectionReason && (
                        <span className="text-stone-500">
                          {selectedSkillEvidence.selectionReason}
                        </span>
                      )}
                    </div>
                    {selectedSkillEvidence && (
                      <div className="mt-2 flex flex-wrap gap-1 text-stone-600">
                        {selectedSkillEvidence.selectedSkillDigest && (
                          <span className="inline-flex h-5 max-w-full items-center rounded-md border border-stone-200 bg-stone-50 px-1.5">
                            <span className="truncate">
                              {selectedSkillEvidence.selectedSkillDigest}
                            </span>
                          </span>
                        )}
                        <span className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5">
                          {selectedSkillEvidence.includedAsBoundedContextOnly
                            ? "bounded context only"
                            : "no selected skill context"}
                        </span>
                        <span className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5">
                          unselected injected:{" "}
                          {selectedSkillEvidence.unselectedSkillsInjected ? "yes" : "no"}
                        </span>
                      </div>
                    )}
                  </div>
                  {selectedSkillId.trim() && (
                    <button
                      type="button"
                      aria-label="Clear selected skill"
                      title="Clear selected skill"
                      disabled={skillToolBusy}
                      onClick={handleClearSelectedSkill}
                      className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <X size={14} />
                    </button>
                  )}
                </div>

                {skillToolError && (
                  <div className="mt-2 border-l-2 border-rose-400 bg-rose-50 px-2 py-1 text-rose-900">
                    {skillToolError}
                  </div>
                )}

                {skillSummaries.length > 0 && (
                  <div className="mt-3">
                    <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                      Local skills
                    </div>
                    <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white">
                      {skillSummaries.slice(0, 6).map(skill => (
                        <div
                          key={skill.skillId}
                          className="grid gap-2 py-2 md:grid-cols-[1fr_auto]"
                        >
                          <div className="min-w-0 px-2">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-semibold text-stone-950">{skill.name}</span>
                              <span className="text-stone-500">{skill.skillId}</span>
                              <span
                                className={classNames(
                                  "inline-flex h-5 items-center rounded-md border px-1.5 font-medium",
                                  skill.available
                                    ? "border-emerald-200 bg-emerald-50 text-emerald-800"
                                    : "border-amber-200 bg-amber-50 text-amber-900"
                                )}
                              >
                                {skill.available ? "available" : "blocked"}
                              </span>
                              <span className="text-stone-500">{skill.sourceKind}</span>
                            </div>
                            <div className="mt-1 line-clamp-2 text-stone-600">
                              {skill.description}
                            </div>
                            <div className="mt-1 truncate text-stone-500">
                              {skill.instructionDigest}
                            </div>
                          </div>
                          <div className="flex items-start justify-end gap-1 px-2">
                            <button
                              type="button"
                              aria-label={`Inspect skill ${skill.name}`}
                              title="Inspect skill"
                              disabled={skillToolBusy}
                              onClick={() => handleInspectSkill(skill.skillId)}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                            >
                              <FileText size={14} />
                            </button>
                            {skill.available && !skill.selected && (
                              <button
                                type="button"
                                aria-label={`Select skill ${skill.name}`}
                                title="Select skill"
                                disabled={skillToolBusy}
                                onClick={() => handleSelectSkill(skill.skillId)}
                                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                              >
                                <CheckCircle2 size={14} />
                              </button>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {inspectedSkillDetail && (
                  <div className="mt-3 border-l border-emerald-300 bg-emerald-50/70 px-3 py-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-semibold text-stone-950">
                        {inspectedSkillDetail.skillId}
                      </span>
                      <span className="text-stone-500">
                        {inspectedSkillDetail.redactionSummary}
                      </span>
                      <span className="text-stone-500">{inspectedSkillDetail.evidenceDigest}</span>
                    </div>
                    <div className="mt-2 whitespace-pre-wrap text-stone-700">
                      {inspectedSkillDetail.boundedInstructionsPreview}
                    </div>
                    <div className="mt-2 flex flex-wrap gap-1">
                      {inspectedSkillDetail.policyNotes.map(note => (
                        <span
                          key={note}
                          className="inline-flex min-h-5 max-w-full items-center rounded-md border border-emerald-200 bg-white px-1.5 text-emerald-900"
                        >
                          <span className="truncate">{note}</span>
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                {toolCandidateSurface && (
                  <div className="mt-3 grid gap-2 md:grid-cols-2">
                    {toolCandidateSurface.candidates.length > 0 && (
                      <div className="min-w-0 border-l border-sky-300 bg-sky-50/70 px-2 py-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="font-semibold text-stone-950">Safe read candidates</span>
                          <span className="text-stone-500">
                            {toolCandidateSurface.evidenceDigest}
                          </span>
                        </div>
                        <div className="mt-1 space-y-1">
                          {toolCandidateSurface.candidates.slice(0, 5).map(candidate => (
                            <div key={candidate.candidateId} className="min-w-0">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="font-semibold text-stone-800">
                                  {candidate.toolName}
                                </span>
                                <span className="text-stone-500">{candidate.policyDecision}</span>
                                <span className="text-stone-500">{candidate.selectionReason}</span>
                              </div>
                              <div className="truncate text-stone-500">
                                {candidate.candidateDigest}
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {(toolCandidateSurface.blockedTools.length > 0 ||
                      toolCandidateSurface.failureRecovery) && (
                      <div className="min-w-0 border-l border-amber-300 bg-amber-50/70 px-2 py-1">
                        <div className="font-semibold text-stone-950">Policy and recovery</div>
                        {toolCandidateSurface.blockedTools.slice(0, 5).map(tool => (
                          <div key={`${tool.toolName}-${tool.reasonCode}`} className="mt-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-semibold text-amber-950">{tool.toolName}</span>
                              <span className="text-amber-900">{tool.reasonCode}</span>
                            </div>
                          </div>
                        ))}
                        {toolCandidateSurface.failureRecovery && (
                          <div className="mt-2 border-t border-amber-200 pt-2 text-amber-950">
                            <div className="font-semibold">
                              {toolCandidateSurface.failureRecovery.failureReason}
                            </div>
                            <div className="mt-1">
                              failed: {toolCandidateSurface.failureRecovery.failedCandidateId}
                              {toolCandidateSurface.failureRecovery.alternativeCandidateId
                                ? ` · alternative: ${toolCandidateSurface.failureRecovery.alternativeCandidateId}`
                                : ""}
                            </div>
                            <div className="mt-1 flex flex-wrap gap-1">
                              {toolCandidateSurface.failureRecovery.controls.map(control => (
                                <span
                                  key={control}
                                  className="inline-flex h-5 items-center rounded-md border border-amber-200 bg-white px-1.5 text-amber-900"
                                >
                                  {control}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            </section>
          )}
          {!companionMode && (taskContinuitySummaries.length > 0 || taskContinuityError) && (
            <div data-testid="task-continuity" className="px-4 py-2">
              <div className="border-y border-stone-200 bg-white px-3 py-3 text-xs text-stone-700">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="inline-flex h-6 items-center rounded-md bg-stone-900 px-2 font-semibold text-white">
                    Task continuity
                  </span>
                  <span className="text-stone-500">{taskContinuitySummaries.length} tracked</span>
                  {taskContinuityBusy && (
                    <span className="inline-flex items-center gap-1 text-stone-500">
                      <Loader2 size={12} className="animate-spin" />
                      Loading
                    </span>
                  )}
                </div>
                {taskContinuityError && (
                  <div className="mt-2 border-l-2 border-rose-400 bg-rose-50 px-2 py-1 text-rose-900">
                    {taskContinuityError}
                  </div>
                )}
                <div className="mt-3 grid gap-3 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)]">
                  <div className="min-w-0 divide-y divide-stone-200 border-y border-stone-200">
                    {taskContinuitySummaries.map(summary => (
                      <button
                        key={summary.taskSessionId}
                        type="button"
                        data-testid="task-continuity-summary"
                        data-task-session-id={summary.taskSessionId}
                        data-run-id={summary.runId}
                        data-task-status={summary.status}
                        data-task-strategy={summary.strategy}
                        data-next-control={summary.nextRecommendedControl}
                        aria-label={`Open task ${summary.title}`}
                        onClick={() => loadTaskContinuityDetail(summary.taskSessionId)}
                        className={classNames(
                          "grid w-full min-w-0 gap-2 px-2 py-2 text-left hover:bg-stone-50",
                          taskContinuityDetail?.taskSession.id === summary.taskSessionId &&
                            "bg-stone-50"
                        )}
                      >
                        <div className="flex min-w-0 flex-wrap items-center gap-2">
                          <span className="truncate font-semibold text-stone-950">
                            {summary.title}
                          </span>
                          <span
                            className={classNames(
                              "inline-flex h-5 items-center rounded-md border px-1.5 font-medium",
                              mainChatTaskStatusClass(summary.status, summary.staleState)
                            )}
                          >
                            {summary.staleState === "stale"
                              ? "stale"
                              : summary.status.replace(/_/g, " ")}
                          </span>
                          <span className="text-stone-500">
                            {formatMainChatStrategy(summary.strategy)}
                          </span>
                          <span className="text-stone-500">
                            next {formatContinuityControl(summary.nextRecommendedControl)}
                          </span>
                        </div>
                        <div className="min-w-0 truncate text-stone-600">
                          {summary.lastObservationPreview}
                        </div>
                        <div className="flex min-w-0 flex-wrap gap-2 text-[11px] text-stone-500">
                          <span>{summary.pendingBlockerCount} blockers</span>
                          <span>{summary.pendingProposalCount} proposals</span>
                          <span className="truncate">digest {summary.resumeSafetyDigest}</span>
                        </div>
                      </button>
                    ))}
                  </div>
                  {taskContinuityDetail && (
                    <div
                      data-testid="task-continuity-detail"
                      data-task-session-id={taskContinuityDetail.taskSession.id}
                      data-run-id={
                        taskContinuitySummaries.find(
                          summary => summary.taskSessionId === taskContinuityDetail.taskSession.id
                        )?.runId ?? taskContinuityDetail.taskSession.id
                      }
                      data-task-strategy={taskContinuityDetail.taskSession.selectedStrategy}
                      data-task-status={taskContinuityDetail.taskSession.status}
                      data-next-control={taskContinuityDetail.nextRecommendedControl}
                      className="min-w-0 border-y border-stone-200 bg-stone-50/80 px-2 py-2"
                    >
                      <div className="flex flex-wrap items-start gap-2">
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-semibold text-stone-950">
                            {taskContinuityDetail.taskSession.userGoal}
                          </div>
                          <div className="mt-1 flex flex-wrap gap-1">
                            <span
                              className={classNames(
                                "inline-flex h-5 items-center rounded-md border px-1.5 font-medium",
                                mainChatTaskStatusClass(
                                  taskContinuityDetail.taskSession.status,
                                  taskContinuityDetail.continuityDiagnostics.staleContext
                                    ? "stale"
                                    : "fresh"
                                )
                              )}
                            >
                              {taskContinuityDetail.continuityDiagnostics.staleContext
                                ? "stale"
                                : taskContinuityDetail.taskSession.status.replace(/_/g, " ")}
                            </span>
                            <span className="inline-flex h-5 items-center rounded-md bg-white px-1.5 text-stone-600">
                              next{" "}
                              {formatContinuityControl(taskContinuityDetail.nextRecommendedControl)}
                            </span>
                          </div>
                        </div>
                        <div className="flex shrink-0 flex-wrap gap-1">
                          {taskContinuityDetail.allowedControls.includes("resume") && (
                            <button
                              type="button"
                              aria-label="Resume task from continuity detail"
                              disabled={taskContinuityBusy}
                              onClick={handleResumeTaskContinuityDetail}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                            >
                              <Play size={14} />
                            </button>
                          )}
                          {taskContinuityDetail.allowedControls.includes("retry") && (
                            <button
                              type="button"
                              aria-label="Retry task action"
                              disabled={taskContinuityBusy}
                              onClick={handleRetryTaskContinuityDetail}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                            >
                              <RotateCw size={14} />
                            </button>
                          )}
                          {taskContinuityDetail.allowedControls.includes("cancel") && (
                            <button
                              type="button"
                              aria-label="Cancel task from continuity detail"
                              disabled={taskContinuityBusy}
                              onClick={handleCancelTaskContinuityDetail}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                            >
                              <Ban size={14} />
                            </button>
                          )}
                          {taskContinuityDetail.allowedControls.includes("refresh_context") && (
                            <button
                              type="button"
                              aria-label="Refresh task context"
                              disabled={taskContinuityBusy}
                              onClick={handleRefreshTaskContinuityContext}
                              className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                            >
                              <RotateCw size={14} />
                            </button>
                          )}
                        </div>
                      </div>
                      {taskContinuityDetail.blockers.length > 0 && (
                        <div className="mt-2 flex flex-wrap gap-1">
                          {taskContinuityDetail.blockers.map(blocker => (
                            <span
                              key={blocker}
                              className="inline-flex h-6 items-center rounded-md border border-amber-200 bg-amber-50 px-2 font-medium text-amber-900"
                            >
                              {blocker}
                            </span>
                          ))}
                        </div>
                      )}
                      {taskContinuityDetail.continuityDiagnostics.reasonCodes.length > 0 && (
                        <div className="mt-2 flex flex-wrap gap-1">
                          {taskContinuityDetail.continuityDiagnostics.reasonCodes.map(code => (
                            <span
                              key={code}
                              className="inline-flex h-5 items-center rounded-md bg-white px-1.5 text-stone-600"
                            >
                              {code}
                            </span>
                          ))}
                        </div>
                      )}
                      <div className="mt-2 grid gap-2 sm:grid-cols-2">
                        <div className="min-w-0">
                          <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                            Last safe point
                          </div>
                          <div className="truncate text-stone-800">
                            {taskContinuityDetail.lastSafeResumePoint ?? "none"}
                          </div>
                        </div>
                        <div className="min-w-0">
                          <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                            Context digest
                          </div>
                          <div className="truncate text-stone-800">
                            {taskContinuityDetail.contextDigest}
                          </div>
                        </div>
                      </div>
                      {taskContinuityDetail.actions.length > 0 && (
                        <div className="mt-2 divide-y divide-stone-200 border-y border-stone-200 bg-white/70">
                          {taskContinuityDetail.actions.map(action => (
                            <div key={action.id} className="grid gap-1 px-2 py-1">
                              <div className="flex min-w-0 flex-wrap items-center gap-2">
                                <span className="font-semibold text-stone-900">
                                  {action.action.actionType}
                                </span>
                                <span
                                  className={classNames(
                                    "inline-flex h-5 items-center rounded-md border px-1.5 font-medium",
                                    mainChatActionStatusClass(action.status)
                                  )}
                                >
                                  {action.status.replace(/_/g, " ")}
                                </span>
                              </div>
                              <div className="truncate text-stone-600">
                                {action.action.description}
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                      {taskContinuityDetail.proposals.length > 0 && (
                        <div
                          data-testid="task-continuity-proposals"
                          className="mt-2 divide-y divide-stone-200 border-y border-stone-200 bg-white/70"
                        >
                          {taskContinuityDetail.proposals.map(proposal => (
                            <div key={proposal.id} className="grid gap-1 px-2 py-1">
                              <div className="flex min-w-0 flex-wrap items-center gap-2">
                                <span className="truncate font-semibold text-stone-900">
                                  {proposal.proposalType.replace(/_/g, " ")} proposal
                                </span>
                                <span className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 font-medium text-stone-700">
                                  {proposal.status.replace(/_/g, " ")}
                                </span>
                              </div>
                              <div className="truncate text-stone-600">{proposal.reason}</div>
                              {proposal.status === "pending" && (
                                <div className="flex flex-wrap gap-1">
                                  <button
                                    type="button"
                                    disabled={taskContinuityBusy}
                                    onClick={() => handleRejectTaskContinuityProposal(proposal.id)}
                                    className="inline-flex min-h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                                  >
                                    Reject proposal
                                  </button>
                                  <button
                                    type="button"
                                    disabled={taskContinuityBusy}
                                    onClick={() => handleDeferTaskContinuityProposal(proposal.id)}
                                    className="inline-flex min-h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                                  >
                                    Defer
                                  </button>
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                      {taskContinuityDetail.finalDelivery && (
                        <div
                          data-testid="task-continuity-final-delivery"
                          data-final-delivery-section-titles={taskContinuityFinalDeliverySections(
                            taskContinuityDetail.finalDelivery
                          ).join("|")}
                          className="mt-2 border-l border-emerald-300 bg-white/80 px-2 py-1"
                        >
                          <div className="font-semibold text-stone-950">Final delivery</div>
                          <div className="mt-1 flex flex-wrap gap-1 text-stone-600">
                            {taskContinuityFinalDeliverySections(taskContinuityDetail.finalDelivery)
                              .length > 0 ? (
                              taskContinuityFinalDeliverySections(
                                taskContinuityDetail.finalDelivery
                              ).map(section => (
                                <span
                                  key={section}
                                  className="inline-flex h-5 items-center rounded-md border border-emerald-200 bg-emerald-50 px-1.5 text-emerald-900"
                                >
                                  {section}
                                </span>
                              ))
                            ) : (
                              <span>Recorded</span>
                            )}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
          {!companionMode && currentRun && (
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
          selectedSkillId={selectedSkillId}
          onInputChange={handleInputChange}
          onSelectedSkillIdChange={setSelectedSkillId}
          onComposerFocus={() => emitCompanionStage("listening")}
          onSend={handleSend}
          onContinueStream={handleContinueStream}
          onRetryLastMessage={retryLastUserMessage}
          getFixSuggestion={getFixSuggestion}
          companionMode={companionMode}
        />
      </div>
    </div>
  );
}
