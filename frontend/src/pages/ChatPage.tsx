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
  Compass,
  ExternalLink,
  Sparkles,
  Heart,
  CheckCircle2,
  ShieldCheck,
  RotateCw,
  Ban,
  Play,
  FileText,
} from "lucide-react";
import type { ChatMessage, LifeModel } from "../types";
import LoadingSpinner from "../components/LoadingSpinner";
import {
  mailboxLinkTarget,
  mailboxRoute,
  productRoutePath,
  runDetailRoute,
  secondaryRoutePath,
} from "../productShellContract";
import {
  startStreamMessage,
  getChatHistory,
  getSystemDiagnostics,
  getLifeStateProjection,
  getSchedulerConfig,
  setSchedulerConfig,
  saveFeedback,
  logAnalyticsEvent,
  getLifeModel,
  listChatSessions,
  createChatSession,
  renameChatSession,
  deleteChatSession,
  listAgentRunsForSession,
  getAgentRun,
  getPendingProposals,
  acceptProposal,
  rejectProposal,
  editProposal,
  draftEditMemoryProposal,
  postponeProposal,
  getMainChatAgentTaskState,
  getTasksViewModel,
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
  TaskControl,
  TaskViewModelItem,
  MainChatAgentStateSnapshot,
  MainChatAgentDurableEvent,
  MainChatKernelEvent,
  MainChatSkillSummary,
  MainChatSkillDetail,
  MainChatSelectedSkill,
  MainChatToolCandidateList,
  ProductRunEvidenceView,
  LifeStateProjection,
  AcceptProposalResult,
} from "../tauri";
import { getModelEmptyState } from "../utils/modelEmpty";
import {
  buildCapabilityStatusViewModel,
  explainGovernanceBlocker,
  userFacingAssistantContent,
  type CapabilityTone,
  type CapabilityStatusViewModel,
} from "../utils/capabilityStatus";
import { reviewRequiredCountFromProjection } from "../utils/lifeStateProjection";
import { listen } from "@tauri-apps/api/event";
import ReasoningTracePanel from "../components/ReasoningTracePanel";
import ToolCallCard from "../components/ToolCallCard";
import AgentStateIndicator from "../components/AgentStateIndicator";
import AgentControlPlane from "../components/AgentControlPlane";
import MainChatExecutionEvidence from "../components/MainChatExecutionEvidence";
import type { AgentStageState } from "../components/AgentStage";
import RuntimeDisclosureStrip from "../components/RuntimeDisclosureStrip";
import { buildRuntimeDisclosure } from "../utils/runtimeDisclosure";
import ChatSidebar from "./chat/ChatSidebar";
import ChatInputArea from "./chat/ChatInputArea";
import { useChatResources } from "./chat/useChatResources";

function generateSessionId() {
  return "sess_" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

function buildReadinessSummary(
  diagnostics: SystemDiagnostics | null,
  projection: LifeStateProjection | null
): {
  status: string;
  tone: "ready" | "warning" | "error";
  detail: string;
  usageReady?: boolean;
} {
  if (!diagnostics && !projection) {
    return {
      status: "检测中",
      tone: "warning",
      detail: "正在读取本地模型、云端 API 和人生模型状态。",
    };
  }

  if (projection?.readiness.chatReady) {
    const backend = diagnostics?.ollama_online
      ? `本地模型 ${diagnostics.resolved_local_model || diagnostics.local_model}`
      : "云端模型";
    return {
      status: "聊天就绪",
      tone: "ready",
      detail: `当前可使用 ${backend}。`,
      usageReady: projection.readiness.usageReady,
    };
  }
  if (diagnostics && !diagnostics.ollama_online && !diagnostics.cloud_api_configured) {
    return {
      status: "需要配置",
      tone: "error",
      detail: "本地模型离线，云端 API 也未配置。无法开始聊天。",
    };
  }
  if (diagnostics && !diagnostics.ollama_online) {
    return {
      status: "本地模型离线",
      tone: "warning",
      detail: `未检测到 ${diagnostics.local_model}，将依赖云端 API。`,
    };
  }
  if (diagnostics && !diagnostics.cloud_api_configured) {
    return { status: "云端 API 未配置", tone: "warning", detail: "复杂任务可能只能使用本地模型。" };
  }
  return { status: "需要检查", tone: "warning", detail: "部分运行状态异常，请查看设置页诊断。" };
}

function companionCapabilityChipClass(tone: CapabilityTone): string {
  if (tone === "ready") return "border-emerald-200 bg-emerald-50 text-emerald-800";
  if (tone === "warning") return "border-amber-200 bg-amber-50 text-amber-900";
  if (tone === "error") return "border-rose-200 bg-rose-50 text-rose-800";
  return "border-stone-200 bg-white text-stone-700";
}

function buildCompanionInitialAssistantMessage(
  diagnostics: SystemDiagnostics | null,
  pendingReviewCount: number | null = null,
  loadState: "normal" | "history_unavailable" = "normal"
): string {
  const status: CapabilityStatusViewModel = buildCapabilityStatusViewModel(
    diagnostics,
    pendingReviewCount,
    null
  );

  if (loadState === "history_unavailable") {
    return [
      "你好，我是 OpenLife。",
      "当前无法确认会话历史或运行状态，可能处于浏览器预览或 Tauri bridge 不可用。",
      "在能力恢复前，我不会声称 Life Model 已加载；请以顶部能力状态和设置页诊断为准。",
    ].join("\n");
  }

  if (!diagnostics) {
    return [
      "你好，我是 OpenLife。",
      "我正在检查模型、Life Model 和工具权限；确认前不会声称 Life Model 已加载。",
      "请以顶部能力状态和设置页诊断为准。",
    ].join("\n");
  }

  const lifeModelReady = diagnostics.life_model_ready && !diagnostics.model_empty;
  const modelReady = diagnostics.chat_ready;

  if (modelReady && lifeModelReady) {
    return [
      "你好，我是 OpenLife。",
      "当前对话能力和 Life Model 已通过本地状态检查；我会基于已确认的信息陪你交流。",
      `当前状态：${status.headline}。`,
    ].join("\n");
  }

  if (!modelReady) {
    return [
      "你好，我是 OpenLife。",
      "当前对话能力还未通过检查；我不会声称 Life Model 已加载或完整可用。",
      `状态详情：${status.detail}`,
      `下一步：${status.primaryActionLabel}。`,
    ].join("\n");
  }

  return [
    "你好，我是 OpenLife。",
    "对话入口可用，但 Life Model 仍是待补全或未确认状态；我会先依赖你当前输入。",
    `当前状态：${status.headline}。`,
  ].join("\n");
}

function CompanionTaskControlStrip({
  taskState,
  taskViewItem,
  busy,
  error,
  canResume,
  canRetry,
  canCancel,
  onResume,
  onRetry,
  onCancel,
  onRefresh,
}: {
  taskState: MainChatAgentTaskState | null;
  taskViewItem: TaskViewModelItem | null;
  busy: boolean;
  error: string | null;
  canResume: boolean;
  canRetry: boolean;
  canCancel: boolean;
  onResume: () => void;
  onRetry: () => void;
  onCancel: () => void;
  onRefresh: () => void;
}) {
  const status = taskViewItem?.lifecycleStatus.replace(/_/g, " ") ?? "evidence unavailable";
  const activeToolCount = taskState?.activeToolCount ?? 0;
  const pendingApprovalCount = taskState?.pendingApprovalCount ?? 0;
  const displayError = error ? boundedProductText(error) || "Action failed" : "";

  return (
    <div
      data-testid="companion-task-controls"
      className="mx-4 rounded-lg border border-stone-200 bg-white px-3 py-2 text-xs text-stone-700 shadow-sm"
    >
      <div className="flex flex-wrap items-center gap-2">
        <span className="inline-flex h-6 items-center rounded-md bg-stone-900 px-2 font-semibold text-white">
          {status}
        </span>
        <span className="text-stone-500">{activeToolCount} active</span>
        <span className="text-stone-500">{pendingApprovalCount} pending</span>
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <button
            type="button"
            aria-label="Resume task"
            title="Resume task"
            disabled={!canResume || busy}
            onClick={onResume}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Play size={14} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label="Retry failed action"
            title="Retry failed action"
            disabled={!canRetry || busy}
            onClick={onRetry}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RotateCw size={14} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label="Cancel task"
            title="Cancel task"
            disabled={!canCancel || busy}
            onClick={onCancel}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Ban size={14} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label="Refresh task state"
            title="Refresh task state"
            disabled={busy}
            onClick={onRefresh}
            className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <RotateCw size={14} aria-hidden="true" className={busy ? "animate-spin" : ""} />
          </button>
        </div>
      </div>
      {taskState?.session?.hasPlanSummary && (
        <div className="mt-2 truncate text-stone-600">Plan state is available in the trace.</div>
      )}
      {displayError && (
        <div className="mt-2 rounded-md bg-rose-50 px-2 py-1 text-rose-800">{displayError}</div>
      )}
    </div>
  );
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

function taskContinuityFinalDeliveryStatus(
  value: Record<string, unknown> | null | undefined
): string {
  if (!value) return "missing_status";
  const metadata = value.metadata;
  const metadataStatus =
    metadata && typeof metadata === "object" && !Array.isArray(metadata)
      ? (metadata as Record<string, unknown>).status
      : undefined;
  const status = value.status ?? metadataStatus;
  return typeof status === "string" && status.trim() ? status : "missing_status";
}

function taskContinuityFinalDeliveryStatusLabel(status: string): string {
  const labels: Record<string, string> = {
    completed: "Completed",
    delivered: "Delivered",
    completed_with_pending_items: "Pending review items",
    blocked: "Blocked",
    failed: "Failed",
    cancelled: "Cancelled",
    missing_status: "Missing status evidence",
  };
  return labels[status] ?? status.replace(/_/g, " ");
}

function taskContinuityFinalDeliveryClass(status: string): string {
  if (status === "completed" || status === "delivered") {
    return "border-emerald-300 bg-white/80 text-emerald-900";
  }
  if (status === "blocked" || status === "failed" || status === "cancelled") {
    return "border-rose-300 bg-rose-50 text-rose-900";
  }
  return "border-amber-300 bg-amber-50 text-amber-900";
}

function TaskContinuityFinalDeliveryPanel({ value }: { value: Record<string, unknown> }) {
  const status = taskContinuityFinalDeliveryStatus(value);
  const sections = taskContinuityFinalDeliverySections(value);
  return (
    <div
      data-testid="task-continuity-final-delivery"
      data-final-delivery-status={status}
      data-final-delivery-section-titles={sections.join("|")}
      className={`mt-2 border-l px-2 py-1 ${taskContinuityFinalDeliveryClass(status)}`}
    >
      <div className="font-semibold">Final delivery</div>
      <div className="mt-1 flex flex-wrap gap-1 text-xs">
        <span className="inline-flex h-5 items-center rounded-md border border-current/30 bg-white/70 px-1.5 font-medium">
          {taskContinuityFinalDeliveryStatusLabel(status)}
        </span>
        {sections.map(section => (
          <span
            key={section}
            className="inline-flex h-5 items-center rounded-md border border-current/20 bg-white/60 px-1.5"
          >
            {section}
          </span>
        ))}
      </div>
    </div>
  );
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

function formatStreamDoneFailure(payload: StreamMessageDonePayload): string {
  const blockers = payload.blockers?.length
    ? payload.blockers.map(blocker => `- ${blocker}`).join("\n")
    : "- stream_failed";
  return `Main Chat stream failed before producing a successful reply.\n\nBlockers:\n${blockers}\n\nRun id: ${payload.run_id}`;
}

function formatMalformedStreamCompletion(expectedSessionId: string): string {
  return [
    "Main Chat stream did not return a completed response.",
    "",
    "OpenLife stopped waiting because the native bridge returned an invalid completion payload.",
    `Expected session: ${expectedSessionId}`,
    "",
    "The message was not treated as successful. Check Runs for any completed backend run, then retry.",
  ].join("\n");
}

function getFixSuggestion(
  diagnostics: SystemDiagnostics | null,
  projection: LifeStateProjection | null
): { text: string; action: string; link: string } | null {
  if (!diagnostics) return null;
  if (!diagnostics.ollama_online && !diagnostics.cloud_api_configured) {
    return {
      text: "没有可用的模型后端。",
      action: "去设置页配置",
      link: productRoutePath("Settings"),
    };
  }
  if (!diagnostics.life_model_ready) {
    return {
      text: "人生模型读取失败。",
      action: "去构建人生模型",
      link: secondaryRoutePath("LifeModelBuild"),
    };
  }
  if (diagnostics.model_empty) {
    const pendingBuilderReviewSessions = projection?.readiness.pendingBuilderReviewSessions ?? 0;
    const unfinishedBuilderSessions = projection?.readiness.unfinishedBuilderSessions ?? 0;
    if (pendingBuilderReviewSessions > 0) {
      return {
        text: `人生模型还没有真正写入，但你有 ${pendingBuilderReviewSessions} 个构建内容待确认。`,
        action: "回构建页查看",
        link: secondaryRoutePath("LifeModelBuild"),
      };
    }
    if (unfinishedBuilderSessions > 0) {
      return {
        text: `人生模型还没有真正写入，但你有 ${unfinishedBuilderSessions} 个待继续的构建会话。`,
        action: "回 Builder 继续",
        link: secondaryRoutePath("LifeModelBuild"),
      };
    }
    return {
      text: "人生模型尚未构建。",
      action: "去 Builder 创建",
      link: secondaryRoutePath("LifeModelBuild"),
    };
  }
  if (!diagnostics.ollama_online && diagnostics.prefer_local_model) {
    return {
      text: `优先本地模型设置开启，但 ${diagnostics.local_model} 未运行。`,
      action: "切换云端模型",
      link: productRoutePath("Settings"),
    };
  }
  return null;
}

function formatChatRuntimeError(error: unknown, diagnostics: SystemDiagnostics | null): string {
  if (diagnostics && !diagnostics.chat_ready && diagnostics.readiness_issues?.length) {
    return `暂时无法发送普通对话：\n${diagnostics.readiness_issues.map(issue => `- ${issue}`).join("\n")}\n\n请去设置页查看“启动检查”。`;
  }
  const raw = error instanceof Error ? error.message : String(error);
  const governanceHint = explainGovernanceBlocker(raw, diagnostics);
  if (governanceHint) return governanceHint;
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
  return `${hint}\n\n请去设置页查看“启动检查”。`;
}

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

type MainChatAgentProductStatus =
  | "completed"
  | "cancelled"
  | "running"
  | "waiting_for_user"
  | "restricted"
  | "blocked"
  | "trace_gap"
  | "proposal_pending"
  | "permission_pending";

type MainChatAgentProductAction =
  | "review_proposal"
  | "review_permission"
  | "resume"
  | "retry"
  | "cancel"
  | "refresh_context"
  | "show_trace";

type MainChatAgentStatusView = {
  status: MainChatAgentProductStatus;
  label: string;
  detail: string;
  sourceLabel: string;
  tone: "success" | "info" | "warning" | "danger" | "neutral";
  blockerLabels: string[];
  pendingProposalCount: number;
  pendingPermissionCount: number;
  memoryGovernanceLabels: string[];
  actions: MainChatAgentProductAction[];
  taskSessionId?: string;
  evidenceSummary?: string;
};

function boundedProductLabel(value: unknown, maxLength = 88): string {
  if (typeof value !== "string" && typeof value !== "number" && typeof value !== "boolean") {
    return "";
  }
  const label = String(value)
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  if (!label) return "";
  if (
    label.startsWith("/") ||
    /^[A-Za-z]:[\\/]/.test(label) ||
    label.includes("/Users/") ||
    label.includes("\\Users\\")
  ) {
    return "workspace item";
  }
  return label.slice(0, maxLength);
}

function boundedProductText(value: unknown, maxLength = 180): string {
  if (typeof value !== "string" && typeof value !== "number" && typeof value !== "boolean") {
    return "";
  }
  return String(value)
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .replace(/\/Users\/[^\s]+/g, "[workspace path]")
    .replace(/[A-Za-z]:\\[^\s]+/g, "[workspace path]")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, maxLength);
}

function hasCurrentTaskContinuityEvidence(detail: MainChatTaskDetail | null): boolean {
  if (!detail) return false;
  const evidence = detail.evidenceView;
  return Boolean(
    evidence &&
    evidence.taskSessionId === detail.taskSession.id &&
    boundedProductText(evidence.title) &&
    boundedProductLabel(evidence.lifecycleState) &&
    ["active", "consistent", "projected"].includes(evidence.projectionState) &&
    evidence.identityState === "consistent" &&
    evidence.snapshotState === "stable"
  );
}

function productStatusLabel(status: MainChatAgentProductStatus): string {
  switch (status) {
    case "completed":
      return "Completed";
    case "cancelled":
      return "Canceled";
    case "running":
      return "Running";
    case "waiting_for_user":
      return "Waiting for you";
    case "restricted":
      return "Restricted";
    case "blocked":
      return "Blocked";
    case "trace_gap":
      return "Evidence missing";
    case "proposal_pending":
      return "Proposal pending";
    case "permission_pending":
      return "Permission pending";
    default:
      return "Task status";
  }
}

function productStatusTone(status: MainChatAgentProductStatus): MainChatAgentStatusView["tone"] {
  switch (status) {
    case "completed":
      return "success";
    case "cancelled":
      return "warning";
    case "running":
      return "info";
    case "restricted":
    case "blocked":
      return "danger";
    case "waiting_for_user":
    case "proposal_pending":
    case "permission_pending":
    case "trace_gap":
      return "warning";
    default:
      return "neutral";
  }
}

function taskLifecycleMatchesRunEvidence(
  taskStatus: TaskViewModelItem["lifecycleStatus"],
  evidenceStatus: string
): boolean {
  switch (taskStatus) {
    case "running":
      return evidenceStatus === "running";
    case "waiting_permission":
      return evidenceStatus === "waiting_permission" || evidenceStatus === "waiting_for_user";
    case "blocked":
      return evidenceStatus === "blocked";
    case "failed":
      return ["failed", "timed_out", "interrupted"].includes(evidenceStatus);
    case "cancelled":
      return evidenceStatus === "cancelled";
    case "completed":
      return evidenceStatus === "completed";
    case "completed_with_pending_review":
      return evidenceStatus === "completed_with_pending_items";
    case "completed_needs_evidence":
      return ["completed", "partial_or_unknown"].includes(evidenceStatus);
    case "unknown":
      return evidenceStatus === "partial_or_unknown" || evidenceStatus === "unknown";
    default:
      return false;
  }
}

function sameStringSet(left: string[], right: string[]): boolean {
  const normalize = (values: string[]) => Array.from(new Set(values)).sort();
  const normalizedLeft = normalize(left);
  const normalizedRight = normalize(right);
  return (
    normalizedLeft.length === normalizedRight.length &&
    normalizedLeft.every((value, index) => value === normalizedRight[index])
  );
}

function taskControlMatchesTaskAuthority(
  taskViewItem: TaskViewModelItem,
  control: TaskControl
): boolean {
  const taskSessionId = taskViewItem.taskSessionId;
  return Boolean(
    taskSessionId &&
    taskViewItem.canonicalTaskId === taskSessionId &&
    control.targetTaskId === taskSessionId
  );
}

export function selectAuthoritativeTaskViewItem(
  items: TaskViewModelItem[],
  taskSessionId: string | undefined,
  sourceSessionId: string | undefined
): TaskViewModelItem | null {
  if (taskSessionId) {
    return items.find(task => task.taskSessionId === taskSessionId) ?? null;
  }
  return (
    items.find(task => Boolean(sourceSessionId) && task.conversationId === sourceSessionId) ?? null
  );
}

function hasVerifiedTaskRunEvidence(
  taskViewItem: TaskViewModelItem | null | undefined,
  runEvidence: ProductRunEvidenceView | null | undefined
): taskViewItem is TaskViewModelItem {
  if (!taskViewItem?.taskSessionId || !runEvidence) return false;
  if (taskViewItem.canonicalTaskId !== taskViewItem.taskSessionId) return false;
  if (runEvidence.taskSessionId !== taskViewItem.taskSessionId) return false;
  if (!["active", "consistent", "projected"].includes(runEvidence.projectionState)) return false;
  if (runEvidence.identityState !== "consistent") return false;
  if (runEvidence.snapshotState !== "stable") return false;
  if (runEvidence.redactionState !== "metadata_only") return false;
  if (!taskLifecycleMatchesRunEvidence(taskViewItem.lifecycleStatus, runEvidence.lifecycleState)) {
    return false;
  }
  if (
    taskViewItem.relatedRunIds.length > 0 &&
    (!runEvidence.runId || !taskViewItem.relatedRunIds.includes(runEvidence.runId))
  ) {
    return false;
  }
  if (
    runEvidence.durableSequenceBefore !== null &&
    runEvidence.durableSequenceAfter !== null &&
    runEvidence.durableSequenceAfter < runEvidence.durableSequenceBefore
  ) {
    return false;
  }
  if (
    taskViewItem.allowedControls.some(
      control => control.enabled && !taskControlMatchesTaskAuthority(taskViewItem, control)
    )
  ) {
    return false;
  }
  return sameStringSet(
    taskViewItem.allowedControls.filter(control => control.enabled).map(control => control.kind),
    runEvidence.allowedControls
  );
}

export function buildMainChatAgentStatusView({
  reasoningTrace,
  taskState,
  taskViewItem,
  runEvidence,
  agentState,
  pendingProposals,
  sending,
  canCancel,
}: {
  reasoningTrace: ReasoningTrace | null;
  taskState: MainChatAgentTaskState | null;
  taskViewItem?: TaskViewModelItem | null;
  runEvidence?: ProductRunEvidenceView | null;
  agentState: MainChatAgentStateSnapshot | null;
  pendingProposals: AgentProposal[];
  sending: boolean;
  canCancel: boolean;
}): MainChatAgentStatusView | null {
  const hasDiagnostics = Boolean(
    reasoningTrace || taskState?.session || agentState || pendingProposals.length > 0 || sending
  );
  const unverifiedTaskSessionId = taskViewItem?.taskSessionId;
  const evidenceVerified = hasVerifiedTaskRunEvidence(taskViewItem, runEvidence);
  if (!evidenceVerified) {
    if (!hasDiagnostics && !taskViewItem && !runEvidence) return null;
    return {
      status: "trace_gap",
      label: productStatusLabel("trace_gap"),
      detail: "Required task evidence is missing, so OpenLife will not infer what happened.",
      sourceLabel: "Task evidence unavailable",
      tone: productStatusTone("trace_gap"),
      blockerLabels: [],
      pendingProposalCount: 0,
      pendingPermissionCount: 0,
      memoryGovernanceLabels: [],
      actions: [],
      taskSessionId: unverifiedTaskSessionId,
    };
  }

  // These inputs remain available to diagnostic surfaces, but must never
  // authorize product status, counts, or controls.
  void reasoningTrace;
  void taskState;
  void agentState;
  void pendingProposals;
  void sending;
  void canCancel;
  const sourceLabel = "TasksViewModel + ProductRunEvidenceView";
  const taskSessionId = taskViewItem.taskSessionId;
  const taskStatus = taskViewItem.lifecycleStatus;
  const deliveryStatus = taskViewItem.terminalDeliveryStatus;
  const pendingProposalCount = taskViewItem.pendingReviewItemRefs.length;
  const pendingPermissionCount = taskStatus === "waiting_permission" ? 1 : 0;
  const memoryGovernanceLabels: string[] = [];
  const taskControls = taskViewItem.allowedControls.filter(control => control.enabled);
  const blockerLabels = Array.from(new Set(taskViewItem.pendingBlockers.filter(Boolean))).slice(
    0,
    4
  );
  const hasCompletedEvidence =
    taskStatus === "completed" &&
    taskViewItem.finalDeliveryEvidencePresent &&
    taskViewItem.pendingReviewItemRefs.length === 0 &&
    deliveryStatus === "delivered";
  const hasCancelledEvidence = taskStatus === "cancelled" && deliveryStatus === "cancelled";
  const latestResultPreview = taskViewItem.latestResultPreview;
  const evidenceSummary =
    latestResultPreview && latestResultPreview.status === taskViewItem?.terminalDeliveryStatus
      ? boundedProductText(latestResultPreview.preview)
      : "";

  let status: MainChatAgentProductStatus | null = null;
  if (pendingPermissionCount > 0) {
    status = "permission_pending";
  } else if (taskStatus === "completed_with_pending_review" || pendingProposalCount > 0) {
    status = "proposal_pending";
  } else if (taskStatus === "completed_needs_evidence") {
    status = "trace_gap";
  } else if (hasCancelledEvidence) {
    status = "cancelled";
  } else if (taskStatus === "cancelled") {
    status = "trace_gap";
  } else if (taskStatus === "blocked" || taskStatus === "failed") {
    status = "blocked";
  } else if (taskStatus === "running") {
    status = "running";
  } else if (hasCompletedEvidence) {
    status = "completed";
  }

  const resolvedStatus = status ?? "trace_gap";
  const actions: MainChatAgentProductAction[] = [];
  if (
    taskControls.some(
      control => control.kind === "resume" && control.effect === "task_resume_request"
    )
  ) {
    actions.push("resume");
  }
  if (
    taskControls.some(
      control => control.kind === "retry" && control.effect === "task_retry_request"
    )
  ) {
    actions.push("retry");
  }
  if (
    taskControls.some(
      control => control.kind === "cancel" && control.effect === "task_cancel_request"
    )
  ) {
    actions.push("cancel");
  }
  if (
    taskSessionId &&
    taskControls.some(
      control => control.kind === "refresh_context" && control.effect === "task_refresh_request"
    )
  ) {
    actions.push("refresh_context");
  }
  if (
    taskControls.some(
      control =>
        control.kind === "open_review_item" &&
        (control.effect === "navigation_only" || control.effect === "evidence_only")
    )
  ) {
    if (pendingPermissionCount > 0) actions.push("review_permission");
    if (pendingProposalCount > pendingPermissionCount) actions.push("review_proposal");
  }
  if (
    taskControls.some(
      control =>
        control.kind === "open_trace" &&
        (control.effect === "navigation_only" || control.effect === "evidence_only")
    )
  ) {
    actions.push("show_trace");
  }

  const detail =
    resolvedStatus === "completed"
      ? "Structured run or delivery evidence says this task is complete."
      : resolvedStatus === "cancelled"
        ? "Canonical task and terminal-delivery evidence agree that this task was canceled."
        : resolvedStatus === "running"
          ? "The agent is still executing or streaming; no completion claim yet."
          : resolvedStatus === "permission_pending"
            ? "A tool or action needs explicit permission before it can continue."
            : resolvedStatus === "proposal_pending"
              ? "A proposed durable change is waiting for review; it is not written yet."
              : resolvedStatus === "blocked"
                ? "The task cannot progress without a supported recovery action."
                : resolvedStatus === "trace_gap"
                  ? "Required task evidence is missing, so OpenLife will not infer what happened."
                  : "The agent is waiting for your decision before continuing.";

  return {
    status: resolvedStatus,
    label: productStatusLabel(resolvedStatus),
    detail,
    sourceLabel,
    tone: productStatusTone(resolvedStatus),
    blockerLabels,
    pendingProposalCount,
    pendingPermissionCount,
    memoryGovernanceLabels,
    actions: Array.from(new Set(actions)),
    taskSessionId,
    evidenceSummary: evidenceSummary || undefined,
  };
}

function enabledTaskViewControl(
  taskViewItem: TaskViewModelItem | null | undefined,
  kind: TaskControl["kind"],
  effect: TaskControl["effect"]
): TaskControl | null {
  return (
    taskViewItem?.allowedControls.find(
      control =>
        control.enabled &&
        control.kind === kind &&
        control.effect === effect &&
        taskControlMatchesTaskAuthority(taskViewItem, control)
    ) ?? null
  );
}

function mainChatAgentStatusClass(tone: MainChatAgentStatusView["tone"]): string {
  switch (tone) {
    case "success":
      return "border-emerald-200 bg-emerald-50 text-emerald-900";
    case "info":
      return "border-blue-200 bg-blue-50 text-blue-900";
    case "warning":
      return "border-amber-200 bg-amber-50 text-amber-950";
    case "danger":
      return "border-rose-200 bg-rose-50 text-rose-900";
    default:
      return "border-stone-200 bg-stone-50 text-stone-800";
  }
}

function MainChatAgentStatusSurface({
  view,
  busy,
  error,
  onResume,
  onRetry,
  onCancel,
  onRefreshContext,
  onShowTrace,
}: {
  view: MainChatAgentStatusView;
  busy: boolean;
  error: string | null;
  onResume: () => void;
  onRetry: () => void;
  onCancel: () => void;
  onRefreshContext: () => void;
  onShowTrace: () => void;
}) {
  const statusClass = mainChatAgentStatusClass(view.tone);
  const hasAction = (action: MainChatAgentProductAction) => view.actions.includes(action);
  const displayError = error ? boundedProductText(error) || "Action failed" : "";

  return (
    <section
      data-testid="main-chat-agent-status"
      data-agent-product-status={view.status}
      aria-label="Agent task status"
      className="px-4 py-2"
    >
      <div className={`max-w-2xl rounded-lg border px-3 py-2 text-xs shadow-sm ${statusClass}`}>
        <div className="flex flex-wrap items-start gap-2">
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <span className="inline-flex min-h-6 items-center rounded-md bg-white/80 px-2 font-semibold">
                {view.label}
              </span>
              <span className="inline-flex min-h-6 items-center rounded-md border border-white/70 bg-white/60 px-2 font-medium">
                {view.sourceLabel}
              </span>
              {view.pendingProposalCount > 0 && (
                <span className="inline-flex min-h-6 items-center rounded-md border border-white/70 bg-white/60 px-2 font-medium">
                  {view.pendingProposalCount} proposal
                </span>
              )}
              {view.pendingPermissionCount > 0 && (
                <span className="inline-flex min-h-6 items-center rounded-md border border-white/70 bg-white/60 px-2 font-medium">
                  {view.pendingPermissionCount} permission
                </span>
              )}
              {view.memoryGovernanceLabels.map(label => (
                <span
                  key={label}
                  className="inline-flex min-h-6 items-center rounded-md border border-white/70 bg-white/60 px-2 font-medium"
                >
                  {label}
                </span>
              ))}
            </div>
            <div className="mt-1 leading-5">{view.detail}</div>
            {view.evidenceSummary && (
              <div className="mt-1 rounded-md border border-white/70 bg-white/60 px-2 py-1 leading-5">
                {view.evidenceSummary}
              </div>
            )}
            {view.blockerLabels.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {view.blockerLabels.map(label => (
                  <span
                    key={label}
                    className="inline-flex min-h-5 items-center rounded-md border border-white/80 bg-white/70 px-1.5 font-medium"
                  >
                    {label}
                  </span>
                ))}
              </div>
            )}
            {displayError && (
              <div className="mt-2 rounded-md border border-rose-200 bg-white/70 px-2 py-1 text-rose-800">
                {displayError}
              </div>
            )}
          </div>
          <div className="flex shrink-0 flex-wrap justify-end gap-1">
            {hasAction("review_proposal") && (
              <Link
                {...mailboxLinkTarget({
                  mainChatTaskSessionId: view.taskSessionId,
                  returnTo: productRoutePath("Companion"),
                })}
                aria-label="Open proposal in Mailbox"
                className="inline-flex min-h-7 items-center gap-1 rounded-md border border-white/80 bg-white px-2 font-semibold text-stone-800 hover:bg-stone-50"
              >
                <FileText size={13} />
                Proposal
              </Link>
            )}
            {hasAction("review_permission") && (
              <Link
                {...mailboxLinkTarget({
                  mainChatTaskSessionId: view.taskSessionId,
                  returnTo: productRoutePath("Companion"),
                })}
                aria-label="Open permission in Mailbox"
                className="inline-flex min-h-7 items-center gap-1 rounded-md border border-white/80 bg-white px-2 font-semibold text-stone-800 hover:bg-stone-50"
              >
                <ShieldCheck size={13} />
                Permission
              </Link>
            )}
            {hasAction("resume") && (
              <button
                type="button"
                aria-label="Resume current task"
                title="Resume current task"
                disabled={busy}
                onClick={onResume}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-white/80 bg-white text-stone-800 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Play size={14} />
              </button>
            )}
            {hasAction("retry") && (
              <button
                type="button"
                aria-label="Retry current action"
                title="Retry current action"
                disabled={busy}
                onClick={onRetry}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-white/80 bg-white text-stone-800 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RotateCw size={14} />
              </button>
            )}
            {hasAction("cancel") && (
              <button
                type="button"
                aria-label="Cancel current task"
                title="Cancel current task"
                disabled={busy}
                onClick={onCancel}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-white/80 bg-white text-stone-800 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Ban size={14} />
              </button>
            )}
            {hasAction("refresh_context") && (
              <button
                type="button"
                aria-label="Refresh current task context"
                title="Refresh current task context"
                disabled={busy}
                onClick={onRefreshContext}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-white/80 bg-white text-stone-800 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RotateCw size={14} className={busy ? "animate-spin" : ""} />
              </button>
            )}
            {hasAction("show_trace") && (
              <button
                type="button"
                aria-label="Show structured trace"
                title="Show structured trace"
                onClick={onShowTrace}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-white/80 bg-white text-stone-800 hover:bg-stone-50"
              >
                <ExternalLink size={14} />
              </button>
            )}
          </div>
        </div>
      </div>
    </section>
  );
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
      return "Mailbox";
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
  if (
    toolCalls.some(
      call =>
        call.requiresConfirmation ||
        call.executionReceipt?.actionEffect === "external_mutation" ||
        call.executionReceipt?.actionEffect === "unknown"
    )
  ) {
    return "privacy";
  }
  if (toolCalls.length > 0) return "tool";
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

function proposalAcceptancePendingReason(result: AcceptProposalResult): string | null {
  const persistence = result.memoryPersistence;
  if (persistence?.canonicalCommitted && persistence.projectionState !== "applied") {
    return `Memory 已写入 canonical store，但派生视图仍为 ${persistence.projectionState}；完成状态保持等待。`;
  }
  if (result.proposalProjectionStatus !== "confirmed") {
    return "副作用已确认，但审阅状态仍在后端对账；系统不会重复执行，完成状态保持等待。";
  }
  return null;
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
  const [lifeStateProjection, setLifeStateProjection] = useState<LifeStateProjection | null>(null);
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
  const [runById, setRunById] = useState<Record<string, AgentRun>>({});
  const [currentAgentIngress, setCurrentAgentIngress] =
    useState<MainChatAgentIngressDecision | null>(null);
  const [currentExecutionTranscript, setCurrentExecutionTranscript] = useState<
    MainChatExecutionTranscriptEntry[]
  >([]);
  const [currentAgentTaskState, setCurrentAgentTaskState] = useState<MainChatAgentTaskState | null>(
    null
  );
  const [currentTaskViewItem, setCurrentTaskViewItem] = useState<TaskViewModelItem | null>(null);
  const [currentProductRunEvidence, setCurrentProductRunEvidence] =
    useState<ProductRunEvidenceView | null>(null);
  const [taskContinuitySummaries, setTaskContinuitySummaries] = useState<MainChatTaskSummary[]>([]);
  const [taskContinuityDetail, setTaskContinuityDetail] = useState<MainChatTaskDetail | null>(null);
  const [taskContinuityBusy, setTaskContinuityBusy] = useState(false);
  const [taskContinuityError, setTaskContinuityError] = useState<string | null>(null);
  const [currentAgentState, setCurrentAgentState] = useState<MainChatAgentStateSnapshot | null>(
    null
  );
  const [currentKernelEvents, setCurrentKernelEvents] = useState<MainChatKernelEvent[]>([]);
  const [showMainChatDiagnostics, setShowMainChatDiagnostics] = useState(false);
  const [agentEventStreamState, setAgentEventStreamState] =
    useState<MainChatEventStreamViewState | null>(null);
  const [agentTaskControlBusy, setAgentTaskControlBusy] = useState(false);
  const [agentTaskControlError, setAgentTaskControlError] = useState<string | null>(null);
  const [pendingProposals, setPendingProposals] = useState<AgentProposal[]>([]);
  const [feedbackGiven, setFeedbackGiven] = useState<Record<number, "up" | "down">>({});

  // Throttle streaming updates to reduce React re-render pressure
  const streamingBufferRef = useRef("");
  const streamingRafRef = useRef<number | null>(null);
  const diagnosticsRef = useRef<SystemDiagnostics | null>(null);
  const streamErrorHandledRef = useRef(false);
  const handledStreamDoneKeysRef = useRef<Set<string>>(new Set());
  const appliedMainChatEventIdsRef = useRef<Set<string>>(new Set());
  const lastAppliedMainChatEventSequenceRef = useRef(0);
  const currentAgentTaskSessionIdRef = useRef<string | null>(null);
  const currentKernelEventSessionRef = useRef<string | null>(null);
  const lastUserMessageRef = useRef<ChatMessage | null>(null);
  const pendingTurnOperationRef = useRef<{
    sessionId: string;
    userContent: string;
    operationId: string;
  } | null>(null);
  const activeTurnOperationRef = useRef<{ sessionId: string; operationId: string } | null>(null);
  const currentSessionIdRef = useRef<string>(currentSessionId);
  const taskAuthorityLoadGenerationRef = useRef(0);
  const projectionSurface = companionMode ? "companion" : "chat";
  const projectionPendingReviewCount = reviewRequiredCountFromProjection(
    lifeStateProjection,
    projectionSurface
  );
  const initialAssistantMessage = useMemo(
    () => buildCompanionInitialAssistantMessage(diagnostics, projectionPendingReviewCount),
    [diagnostics, projectionPendingReviewCount]
  );
  const {
    currentDraft: currentResourceDraft,
    currentResources,
    importBusy: resourceImportBusy,
    currentError: currentResourceImportError,
    currentNotice: currentResourceImportNotice,
    removingResourceIds,
    attachResources: handleAttachResources,
    cancelImport: handleCancelResourceImport,
    removeResource: handleRemoveResource,
    completeTurn: completeResourceTurn,
  } = useChatResources({
    sessionId: currentSessionId,
    interactionBlocked: sending || activeTurnOperationRef.current?.sessionId === currentSessionId,
  });

  const applyMainChatAgentStateSnapshot = useCallback(
    (
      snapshot: MainChatAgentStateSnapshot | null,
      status: MainChatEventStreamStatus = "subscribed"
    ) => {
      setCurrentAgentState(snapshot);
      if (!snapshot) {
        currentAgentTaskSessionIdRef.current = null;
        currentKernelEventSessionRef.current = null;
        setCurrentKernelEvents([]);
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

  const handleMainChatKernelEvent = useCallback((event: MainChatKernelEvent) => {
    if (event.type === "turn_started") {
      if (event.session_id !== currentSessionIdRef.current) return;
      currentKernelEventSessionRef.current = event.session_id;
      setCurrentKernelEvents([event]);
      return;
    }
    if (currentKernelEventSessionRef.current !== currentSessionIdRef.current) return;
    setCurrentKernelEvents(prev => [...prev, event].slice(-40));
  }, []);

  const emitCompanionStage = useCallback(
    (state: AgentStageState) => {
      onCompanionStageChange?.(state);
    },
    [onCompanionStageChange]
  );

  useEffect(() => {
    currentSessionIdRef.current = currentSessionId;
  }, [applyMainChatAgentStateSnapshot, currentSessionId]);

  const refreshAgentRuns = async (sessionId = currentSessionIdRef.current) => {
    try {
      const runs = await listAgentRunsForSession(sessionId, 10);
      if (currentSessionIdRef.current === sessionId) {
        setCurrentRun(runs[0] ?? null);
        if (runs.length > 0) {
          setRunById(prev => ({
            ...prev,
            ...Object.fromEntries(runs.map(run => [run.id, run])),
          }));
        }
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
        if (run) {
          setRunById(prev => ({ ...prev, [run.id]: run }));
        }
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
      setCurrentProductRunEvidence(null);
      void loadCurrentTaskViewItem(undefined, sourceSessionId);
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
    void loadCurrentTaskViewItem(taskSessionId, sourceSessionId);
  };

  const loadCurrentTaskViewItem = async (
    taskSessionId: string | undefined,
    sourceSessionId = currentSessionIdRef.current
  ): Promise<TaskViewModelItem | null> => {
    const loadGeneration = ++taskAuthorityLoadGenerationRef.current;
    try {
      const envelope = await getTasksViewModel();
      const item = selectAuthoritativeTaskViewItem(
        envelope.data?.items ?? [],
        taskSessionId,
        sourceSessionId
      );
      let runEvidence: ProductRunEvidenceView | null = null;
      if (item?.taskSessionId) {
        try {
          const detail = await getMainChatAgentTaskDetail(item.taskSessionId);
          if (
            detail?.taskSession?.id === item.taskSessionId &&
            detail.evidenceView?.taskSessionId === item.taskSessionId
          ) {
            runEvidence = detail.evidenceView;
          }
        } catch {
          runEvidence = null;
        }
      }
      if (
        currentSessionIdRef.current !== sourceSessionId ||
        taskAuthorityLoadGenerationRef.current !== loadGeneration
      ) {
        return null;
      }
      setCurrentTaskViewItem(item);
      setCurrentProductRunEvidence(runEvidence);
      return hasVerifiedTaskRunEvidence(item, runEvidence) ? item : null;
    } catch {
      if (
        currentSessionIdRef.current === sourceSessionId &&
        taskAuthorityLoadGenerationRef.current === loadGeneration
      ) {
        setCurrentTaskViewItem(null);
        setCurrentProductRunEvidence(null);
      }
      return null;
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

  const refreshLifeStateProjection = async () => {
    try {
      const projection = await getLifeStateProjection();
      setLifeStateProjection(projection);
    } catch {
      setLifeStateProjection(null);
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
    if (lifeStateProjection?.safeMode.active) {
      emitCompanionStage("privacy");
    }
  }, [lifeStateProjection?.safeMode.active, emitCompanionStage]);

  useEffect(() => {
    if (projectionPendingReviewCount != null && projectionPendingReviewCount > 0) {
      emitCompanionStage("review");
    }
  }, [projectionPendingReviewCount, emitCompanionStage]);

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
      return;
    }
    if (currentRun?.status === "completed") {
      emitCompanionStage("done");
    }
  }, [currentRun, emitCompanionStage]);

  useEffect(() => {
    (async () => {
      try {
        const [diag, cfg, projection] = await Promise.all([
          getSystemDiagnostics(),
          getSchedulerConfig(),
          getLifeStateProjection().catch(() => null),
        ]);
        setDiagnostics(diag);
        setLifeStateProjection(projection);
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
    refreshLifeStateProjection();
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
      refreshLifeStateProjection();
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
    refreshLifeStateProjection();
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
              content: initialAssistantMessage,
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
            content: buildCompanionInitialAssistantMessage(
              diagnostics,
              projectionPendingReviewCount,
              "history_unavailable"
            ),
          },
        ]);
      })
      .finally(() => setLoadingHistory(false));
  }, [
    currentSessionId,
    applyMainChatAgentStateSnapshot,
    diagnostics,
    initialAssistantMessage,
    projectionPendingReviewCount,
    loadTaskContinuityList,
  ]);

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
    let unlistenKernelEvent: (() => void) | null = null;

    (async () => {
      unlistenAgentEvent = await listen<MainChatAgentDurableEvent>(
        "main-chat-agent-event",
        async event => {
          await handleMainChatAgentEvent(event.payload);
        }
      );
      unlistenKernelEvent = await listen<MainChatKernelEvent>("main-chat-kernel-event", event => {
        handleMainChatKernelEvent(event.payload);
      });
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
          if (event.payload.status === "failed") {
            flushStreaming();
            setMessages(prev => [
              ...prev,
              {
                role: "assistant",
                content: formatStreamDoneFailure(event.payload),
                run_id: event.payload.run_id,
              },
            ]);
            setStreamingReply("");
            setSending(false);
            if (activeTurnOperationRef.current?.sessionId === event.payload.session_id) {
              activeTurnOperationRef.current = null;
            }
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
            setStreamInterrupted(true);
            streamErrorHandledRef.current = true;
            emitCompanionStage("error");
            await loadMainChatTaskState(
              event.payload.agent_ingress?.agentTaskSessionId,
              event.payload.session_id
            );
            await loadAgentRunForSession(event.payload.run_id, event.payload.session_id);
            refreshAgentRuns(event.payload.session_id);
            return;
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
          const completedTurnOperation =
            activeTurnOperationRef.current?.sessionId === event.payload.session_id
              ? activeTurnOperationRef.current.operationId
              : null;
          if (completedTurnOperation) {
            activeTurnOperationRef.current = null;
            if (pendingTurnOperationRef.current?.operationId === completedTurnOperation) {
              pendingTurnOperationRef.current = null;
            }
            completeResourceTurn(event.payload.session_id, completedTurnOperation);
          }
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
            if (activeTurnOperationRef.current?.sessionId === event.payload.session_id) {
              activeTurnOperationRef.current = null;
            }
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
      if (unlistenKernelEvent) unlistenKernelEvent();
    };
  }, [
    applyMainChatAgentStateSnapshot,
    completeResourceTurn,
    currentSessionId,
    emitCompanionStage,
    handleMainChatAgentEvent,
    handleMainChatKernelEvent,
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

  const handleSend = useCallback(async () => {
    const resourceDraft = currentResourceDraft;
    const resources = resourceDraft?.resources ?? [];
    if (
      (!input.trim() && resources.length === 0) ||
      sending ||
      resourceImportBusy ||
      activeTurnOperationRef.current?.sessionId === currentSessionId
    ) {
      return;
    }
    if (!currentSessionId || typeof currentSessionId !== "string") {
      emitCompanionStage("error");
      setMessages(prev => [
        ...prev,
        { role: "assistant", content: "错误: 当前会话 ID 无效，请刷新页面或切换会话后重试。" },
      ]);
      return;
    }
    const text = input.trim() || "请总结这些附件，并在结论后标注对应来源。";
    const userMsg: ChatMessage = { role: "user", content: text };
    const nextMessages = [...messages, userMsg];
    lastUserMessageRef.current = userMsg;
    setMessages(nextMessages);
    setInput("");

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
    setShowMainChatDiagnostics(false);
    setCurrentExecutionTranscript([]);
    setCurrentAgentTaskState(null);
    taskAuthorityLoadGenerationRef.current += 1;
    setCurrentTaskViewItem(null);
    setCurrentProductRunEvidence(null);
    setAgentTaskControlError(null);
    emitCompanionStage("sorting");

    let invokedTurnOperationId: string | null = null;
    try {
      const selectedSkillOption = selectedSkillId.trim() || undefined;
      const pendingTurnOperation = pendingTurnOperationRef.current;
      const turnOperationId = resourceDraft?.turnOperationId
        ? resourceDraft.turnOperationId
        : pendingTurnOperation?.sessionId === currentSessionId &&
            pendingTurnOperation.userContent === text
          ? pendingTurnOperation.operationId
          : crypto.randomUUID();
      pendingTurnOperationRef.current = {
        sessionId: currentSessionId,
        userContent: text,
        operationId: turnOperationId,
      };
      invokedTurnOperationId = turnOperationId;
      // The streaming backend persists the user message before model execution.
      // Saving it here as well creates duplicate user rows in history and memory retrieval.
      let browserE2eDone: StreamMessageDonePayload;
      activeTurnOperationRef.current = {
        sessionId: currentSessionId,
        operationId: turnOperationId,
      };
      browserE2eDone = await startStreamMessage(currentSessionId, nextMessages, {
        operationId: turnOperationId,
        selectedSkillId: selectedSkillOption,
      });
      if (activeTurnOperationRef.current?.operationId === turnOperationId) {
        activeTurnOperationRef.current = null;
      }
      if (!isStreamDonePayload(browserE2eDone) || browserE2eDone.session_id !== currentSessionId) {
        flushStreaming();
        setMessages(prev => [
          ...prev,
          { role: "assistant", content: formatMalformedStreamCompletion(currentSessionId) },
        ]);
        setStreamingReply("");
        setSending(false);
        setStreamInterrupted(true);
        streamErrorHandledRef.current = true;
        emitCompanionStage("error");
        refreshAgentRuns(currentSessionId);
        await loadSessions();
        return;
      }
      if (browserE2eDone.status === "failed") {
        flushStreaming();
        setMessages(prev => [
          ...prev,
          {
            role: "assistant",
            content: formatStreamDoneFailure(browserE2eDone),
            run_id: browserE2eDone.run_id,
          },
        ]);
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
        setStreamInterrupted(true);
        streamErrorHandledRef.current = true;
        emitCompanionStage("error");
        await loadMainChatTaskState(
          browserE2eDone.agent_ingress?.agentTaskSessionId,
          browserE2eDone.session_id
        );
        await loadAgentRunForSession(browserE2eDone.run_id, browserE2eDone.session_id);
        refreshAgentRuns(browserE2eDone.session_id);
        await loadSessions();
        return;
      }
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
      setStreamInterrupted(false);
      pendingTurnOperationRef.current = null;
      if (resourceDraft?.turnOperationId === turnOperationId) {
        completeResourceTurn(currentSessionId, turnOperationId);
      }
      emitCompanionStage(nextStage);
      await loadMainChatTaskState(
        browserE2eDone.agent_ingress?.agentTaskSessionId,
        browserE2eDone.session_id
      );
      await loadAgentRunForSession(browserE2eDone.run_id, browserE2eDone.session_id);
      refreshAgentRuns(browserE2eDone.session_id);
      logAnalyticsEvent("send_message", currentSessionId, undefined).catch(() => {});
      await loadSessions();
    } catch (e) {
      if (
        invokedTurnOperationId &&
        activeTurnOperationRef.current?.operationId === invokedTurnOperationId
      ) {
        activeTurnOperationRef.current = null;
      }
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
    currentResourceDraft,
    resourceImportBusy,
    completeResourceTurn,
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
    if (
      !lastUser ||
      sending ||
      resourceImportBusy ||
      activeTurnOperationRef.current?.sessionId === currentSessionId
    ) {
      return;
    }
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
    setShowMainChatDiagnostics(false);
    setCurrentExecutionTranscript([]);
    setCurrentAgentTaskState(null);
    taskAuthorityLoadGenerationRef.current += 1;
    setCurrentTaskViewItem(null);
    setCurrentProductRunEvidence(null);
    setAgentTaskControlError(null);
    emitCompanionStage("sorting");
    let invokedTurnOperationId: string | null = null;
    try {
      const selectedSkillOption = selectedSkillId.trim() || undefined;
      const lastUserContent = lastUser.content;
      const pendingTurnOperation = pendingTurnOperationRef.current;
      const turnOperationId =
        pendingTurnOperation?.sessionId === currentSessionId &&
        pendingTurnOperation.userContent === lastUserContent
          ? pendingTurnOperation.operationId
          : null;
      if (!turnOperationId) {
        throw new Error(
          "turn_operation_identity_unavailable: cannot safely retry without the original operation id"
        );
      }
      pendingTurnOperationRef.current = {
        sessionId: currentSessionId,
        userContent: lastUserContent,
        operationId: turnOperationId,
      };
      invokedTurnOperationId = turnOperationId;
      let retryDone: StreamMessageDonePayload;
      activeTurnOperationRef.current = {
        sessionId: currentSessionId,
        operationId: turnOperationId,
      };
      retryDone = await startStreamMessage(currentSessionId, retryMessages, {
        operationId: turnOperationId,
        selectedSkillId: selectedSkillOption,
      });
      if (activeTurnOperationRef.current?.operationId === turnOperationId) {
        activeTurnOperationRef.current = null;
      }
      if (!isStreamDonePayload(retryDone) || retryDone.session_id !== currentSessionId) {
        flushStreaming();
        setMessages(prev => [
          ...prev,
          { role: "assistant", content: formatMalformedStreamCompletion(currentSessionId) },
        ]);
        setStreamingReply("");
        setSending(false);
        setStreamInterrupted(true);
        streamErrorHandledRef.current = true;
        emitCompanionStage("error");
        refreshAgentRuns(currentSessionId);
        await loadSessions();
        return;
      }
      if (retryDone.status === "failed") {
        flushStreaming();
        setMessages(prev => [
          ...prev,
          {
            role: "assistant",
            content: formatStreamDoneFailure(retryDone),
            run_id: retryDone.run_id,
          },
        ]);
        setStreamingReply("");
        setSending(false);
        setReasoningTrace(retryDone.reasoning_trace ?? null);
        setCurrentRunId(retryDone.run_id);
        setToolCalls(
          (retryDone.tool_calls ?? []).map(call => ({
            ...call,
            run_id: retryDone.run_id,
          }))
        );
        setCurrentAgentIngress(retryDone.agent_ingress ?? null);
        applyMainChatAgentStateSnapshot(retryDone.agent_state ?? null);
        setCurrentExecutionTranscript(retryDone.execution_transcript ?? []);
        setStreamInterrupted(true);
        streamErrorHandledRef.current = true;
        emitCompanionStage("error");
        await loadMainChatTaskState(
          retryDone.agent_ingress?.agentTaskSessionId,
          retryDone.session_id
        );
        await loadAgentRunForSession(retryDone.run_id, retryDone.session_id);
        refreshAgentRuns(retryDone.session_id);
        await loadSessions();
        return;
      }
      const nextStage =
        inferStageFromToolCalls(retryDone.tool_calls ?? []) ??
        inferStageFromText(retryDone.reply) ??
        "idle";
      flushStreaming();
      setMessages(prev => {
        if (
          prev.some(message => message.role === "assistant" && message.run_id === retryDone.run_id)
        ) {
          return prev;
        }
        return [
          ...prev,
          {
            role: "assistant",
            content: retryDone.reply,
            run_id: retryDone.run_id,
          },
        ];
      });
      setStreamingReply("");
      setSending(false);
      setReasoningTrace(retryDone.reasoning_trace ?? null);
      setCurrentRunId(retryDone.run_id);
      setToolCalls(
        (retryDone.tool_calls ?? []).map(call => ({
          ...call,
          run_id: retryDone.run_id,
        }))
      );
      setCurrentAgentIngress(retryDone.agent_ingress ?? null);
      applyMainChatAgentStateSnapshot(retryDone.agent_state ?? null);
      setCurrentExecutionTranscript(retryDone.execution_transcript ?? []);
      setStreamInterrupted(false);
      pendingTurnOperationRef.current = null;
      completeResourceTurn(currentSessionId, turnOperationId);
      emitCompanionStage(nextStage);
      await loadMainChatTaskState(
        retryDone.agent_ingress?.agentTaskSessionId,
        retryDone.session_id
      );
      await loadAgentRunForSession(retryDone.run_id, retryDone.session_id);
      refreshAgentRuns(retryDone.session_id);
      await loadSessions();
    } catch (e) {
      if (
        invokedTurnOperationId &&
        activeTurnOperationRef.current?.operationId === invokedTurnOperationId
      ) {
        activeTurnOperationRef.current = null;
      }
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
    resourceImportBusy,
    completeResourceTurn,
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

  const authoritativeCurrentTaskViewItem = hasVerifiedTaskRunEvidence(
    currentTaskViewItem,
    currentProductRunEvidence
  )
    ? currentTaskViewItem
    : null;

  const handleRefreshCurrentMainChatTask = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    if (!taskSessionId || agentTaskControlBusy) return;
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      await refreshMainChatControlState(taskSessionId);
    } catch (error) {
      setAgentTaskControlError(`Refresh failed: ${readablePreviewError(error)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatControlState]);

  const handleRefreshCurrentMainChatTaskContext = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    if (!taskSessionId || agentTaskControlBusy) return;
    if (
      !enabledTaskViewControl(
        authoritativeCurrentTaskViewItem,
        "refresh_context",
        "task_refresh_request"
      )
    ) {
      setAgentTaskControlError("Context refresh is not enabled by verified task evidence.");
      return;
    }
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const detail = await refreshMainChatAgentTaskContext(taskSessionId);
      setTaskContinuityDetail(detail);
      await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
      await loadTaskContinuityList();
    } catch (error) {
      setAgentTaskControlError(`Refresh context failed: ${readablePreviewError(error)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [
    agentTaskControlBusy,
    authoritativeCurrentTaskViewItem,
    currentMainChatTaskSessionId,
    loadMainChatTaskState,
    loadTaskContinuityList,
  ]);

  const handleShowMainChatStructuredTrace = useCallback(() => {
    setShowMainChatDiagnostics(true);
    setShowReasoningTrace(true);
  }, []);

  useEffect(() => {
    if (!companionMode || !sending) return;
    const taskSessionId = currentMainChatTaskSessionId();
    if (!taskSessionId) return;

    const timer = window.setInterval(() => {
      void loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
    }, 3000);

    return () => window.clearInterval(timer);
  }, [companionMode, currentMainChatTaskSessionId, sending]);

  const handleResumeMainChatTask = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    const resumeControl = enabledTaskViewControl(
      authoritativeCurrentTaskViewItem,
      "resume",
      "task_resume_request"
    );
    if (!taskSessionId || agentTaskControlBusy) return;
    if (!resumeControl) {
      setAgentTaskControlError("Resume is not enabled by the backend task read model.");
      return;
    }
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const state = await resumeMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await refreshPendingProposals();
      await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
    } catch (e) {
      setAgentTaskControlError(`Resume failed: ${readablePreviewError(e)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [
    agentTaskControlBusy,
    currentMainChatTaskSessionId,
    authoritativeCurrentTaskViewItem,
    loadMainChatTaskState,
  ]);

  const handleCancelMainChatTask = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    const cancelControl = enabledTaskViewControl(
      authoritativeCurrentTaskViewItem,
      "cancel",
      "task_cancel_request"
    );
    if (!taskSessionId || agentTaskControlBusy) return;
    if (!cancelControl) {
      setAgentTaskControlError("Cancel is not enabled by the backend task read model.");
      return;
    }
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const state = await cancelMainChatAgentTask(taskSessionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      setSending(false);
      setStreamingReply("");
      setStreamInterrupted(false);
      await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
    } catch (e) {
      setAgentTaskControlError(`Cancel failed: ${readablePreviewError(e)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [
    agentTaskControlBusy,
    currentMainChatTaskSessionId,
    authoritativeCurrentTaskViewItem,
    loadMainChatTaskState,
  ]);

  const handleRetryMainChatAction = useCallback(async () => {
    const taskSessionId = currentMainChatTaskSessionId();
    const retryControl = enabledTaskViewControl(
      authoritativeCurrentTaskViewItem,
      "retry",
      "task_retry_request"
    );
    const actionId = retryControl?.targetActionId;
    if (!taskSessionId || agentTaskControlBusy) return;
    if (!retryControl || !actionId) {
      setAgentTaskControlError(
        "Retry is not enabled with an exact action target by the backend task read model."
      );
      return;
    }
    setAgentTaskControlBusy(true);
    setAgentTaskControlError(null);
    try {
      const state = await retryMainChatAgentAction(taskSessionId, actionId);
      setCurrentAgentTaskState(state);
      setCurrentExecutionTranscript(state.transcript ?? []);
      await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
    } catch (e) {
      setAgentTaskControlError(`Retry failed: ${readablePreviewError(e)}`);
    } finally {
      setAgentTaskControlBusy(false);
    }
  }, [
    agentTaskControlBusy,
    authoritativeCurrentTaskViewItem,
    currentMainChatTaskSessionId,
    loadMainChatTaskState,
  ]);

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
    const actionId = taskContinuityDetail?.retryTargetActionId;
    if (!taskSessionId || taskContinuityBusy) return;
    if (!actionId) {
      setTaskContinuityError("Backend task detail did not provide an exact retry action target.");
      return;
    }
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
    taskContinuityDetail?.retryTargetActionId,
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

  const handleAcceptTaskContinuityProposal = useCallback(
    async (proposalId: string) => {
      const taskSessionId = taskContinuityDetail?.taskSession.id;
      const proposal = taskContinuityDetail?.proposals.find(item => item.id === proposalId);
      if (!taskSessionId || taskContinuityBusy) return;
      if (proposal?.proposalType !== "tool_permission") {
        setTaskContinuityError("Accept proposal is only available for ToolPermission task resume.");
        return;
      }
      setTaskContinuityBusy(true);
      setTaskContinuityError(null);
      try {
        const acceptance = await acceptProposal(proposalId);
        await refreshPendingProposals();
        const pendingReason = proposalAcceptancePendingReason(acceptance);
        if (pendingReason) {
          await loadTaskContinuityDetail(taskSessionId);
          await loadTaskContinuityList();
          setTaskContinuityError(pendingReason);
          return;
        }
        const refreshedTaskViewItem = await loadCurrentTaskViewItem(
          taskSessionId,
          currentSessionIdRef.current
        );
        if (!enabledTaskViewControl(refreshedTaskViewItem, "resume", "task_resume_request")) {
          throw new Error(
            "Backend task read model did not enable resume after proposal acceptance."
          );
        }
        const state = await resumeMainChatAgentTask(taskSessionId);
        setCurrentAgentTaskState(state);
        setCurrentExecutionTranscript(state.transcript ?? []);
        await loadMainChatTaskState(taskSessionId, currentSessionIdRef.current);
        await loadTaskContinuityDetail(taskSessionId);
        await loadTaskContinuityList();
      } catch (e) {
        setTaskContinuityError(`Accept proposal failed: ${readablePreviewError(e)}`);
      } finally {
        setTaskContinuityBusy(false);
      }
    },
    [
      loadTaskContinuityDetail,
      loadTaskContinuityList,
      loadCurrentTaskViewItem,
      loadMainChatTaskState,
      refreshPendingProposals,
      taskContinuityBusy,
      taskContinuityDetail?.proposals,
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
        const acceptance = await acceptProposal(target.proposalId);
        const pendingReason = proposalAcceptancePendingReason(acceptance);
        if (pendingReason) {
          await refreshMainChatControlState(taskSessionId);
          setAgentTaskControlError(pendingReason);
          return;
        }
        const refreshedTaskViewItem = await loadCurrentTaskViewItem(
          taskSessionId,
          currentSessionIdRef.current
        );
        if (!enabledTaskViewControl(refreshedTaskViewItem, "resume", "task_resume_request")) {
          throw new Error(
            "Backend task read model did not enable resume after permission approval."
          );
        }
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
    [
      agentTaskControlBusy,
      currentMainChatTaskSessionId,
      loadCurrentTaskViewItem,
      refreshMainChatControlState,
    ]
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
        const acceptance = await acceptProposal(proposalId);
        const pendingReason = proposalAcceptancePendingReason(acceptance);
        if (pendingReason) {
          await refreshMainChatControlState(taskSessionId);
          setAgentTaskControlError(pendingReason);
          return;
        }
        if (
          proposal?.proposalType === "tool_permission" &&
          proposal.actionIds.length > 0 &&
          taskSessionId
        ) {
          const refreshedTaskViewItem = await loadCurrentTaskViewItem(
            taskSessionId,
            currentSessionIdRef.current
          );
          if (!enabledTaskViewControl(refreshedTaskViewItem, "resume", "task_resume_request")) {
            throw new Error(
              "Backend task read model did not enable resume after proposal acceptance."
            );
          }
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
      loadCurrentTaskViewItem,
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
        setAgentTaskControlError(`Plan confirmation failed: ${readablePreviewError(e)}`);
      } finally {
        setAgentTaskControlBusy(false);
      }
    },
    [agentTaskControlBusy, currentMainChatTaskSessionId, refreshMainChatSnapshot]
  );

  const readiness = useMemo(
    () => buildReadinessSummary(diagnostics, lifeStateProjection),
    [diagnostics, lifeStateProjection]
  );
  const capabilityStatus = useMemo(
    () => buildCapabilityStatusViewModel(diagnostics, projectionPendingReviewCount, currentRun),
    [currentRun, diagnostics, projectionPendingReviewCount]
  );
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

  const hasMainChatExecutionEvidence =
    Boolean(currentAgentState) ||
    Boolean(currentAgentTaskState?.session) ||
    currentKernelEvents.length > 0 ||
    toolCalls.length > 0;
  const hasMainChatDiagnostics =
    Boolean(currentAgentState) ||
    Boolean(currentAgentIngress) ||
    Boolean(currentRun) ||
    Boolean(reasoningTrace) ||
    currentExecutionTranscript.length > 0 ||
    Boolean(agentEventStreamState?.events.length) ||
    toolCalls.length > 0;
  const canResumeCurrentMainChatTask = Boolean(
    enabledTaskViewControl(authoritativeCurrentTaskViewItem, "resume", "task_resume_request")
  );
  const canRetryCurrentMainChatTask = Boolean(
    enabledTaskViewControl(authoritativeCurrentTaskViewItem, "retry", "task_retry_request")
  );
  const canCancelCurrentMainChatTask = Boolean(
    enabledTaskViewControl(authoritativeCurrentTaskViewItem, "cancel", "task_cancel_request")
  );
  const mainChatAgentStatusView = useMemo(
    () =>
      buildMainChatAgentStatusView({
        reasoningTrace,
        taskState: currentAgentTaskState,
        taskViewItem: currentTaskViewItem,
        runEvidence: currentProductRunEvidence,
        agentState: currentAgentState,
        pendingProposals,
        sending,
        canCancel: canCancelCurrentMainChatTask,
      }),
    [
      canCancelCurrentMainChatTask,
      currentAgentState,
      currentAgentTaskState,
      currentProductRunEvidence,
      currentTaskViewItem,
      pendingProposals,
      reasoningTrace,
      sending,
    ]
  );
  const safeAgentTaskControlError = agentTaskControlError
    ? boundedProductText(agentTaskControlError) || "Action failed"
    : null;
  const taskContinuityEvidenceCurrent = hasCurrentTaskContinuityEvidence(taskContinuityDetail);
  const taskContinuityEvidenceTitle = taskContinuityEvidenceCurrent
    ? boundedProductText(taskContinuityDetail?.evidenceView.title) || "Task evidence unavailable"
    : "Task evidence unavailable";
  const taskContinuityAllowedControls = taskContinuityEvidenceCurrent
    ? (taskContinuityDetail?.evidenceView.allowedControls ?? [])
    : [];

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
                  <div className="mt-1 text-sm font-medium leading-4 text-stone-600">
                    {capabilityStatus.headline}
                  </div>
                </div>
              </div>
              <div className="mt-2 flex flex-wrap gap-2 text-xs font-medium">
                {capabilityStatus.chips.map(chip => (
                  <span
                    key={chip.label}
                    title={chip.detail}
                    className={[
                      "inline-flex min-h-6 items-center rounded-md border px-2 py-0.5",
                      companionCapabilityChipClass(chip.tone),
                    ].join(" ")}
                  >
                    {chip.label}
                  </span>
                ))}
              </div>
              <div className="mt-2 max-w-3xl text-xs leading-5 text-stone-500">
                {capabilityStatus.detail}
                <Link
                  to={capabilityStatus.primaryActionHref}
                  className="ml-2 font-semibold text-stone-700 underline-offset-4 hover:underline"
                >
                  {capabilityStatus.primaryActionLabel}
                </Link>
              </div>
            </div>
          </div>
        ) : (
          <>
            <div className="border-b px-6 py-2 flex items-center justify-between bg-gray-50 gap-3">
              <div className={`text-sm border rounded-lg px-3 py-2 flex-1 ${readinessClass}`}>
                <div className="flex items-center gap-2">
                  <span className="font-medium">{readiness.status}</span>
                  {readiness.usageReady === true && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-100 text-blue-700 font-medium">
                      使用准备就绪
                    </span>
                  )}
                  {readiness.usageReady === false && readiness.tone === "ready" && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-amber-100 text-amber-700 font-medium">
                      使用准备待完善
                    </span>
                  )}
                </div>
                <div className="text-xs mt-0.5">
                  {capabilityStatus.detail}
                  {diagnostics && (
                    <span className="ml-2">
                      本地：{diagnostics.resolved_local_model || diagnostics.local_model} · 云端
                      API：{capabilityStatus.cloudApiStatusLabel}
                    </span>
                  )}
                  <Link
                    to={capabilityStatus.primaryActionHref}
                    className="ml-2 underline font-medium"
                  >
                    {capabilityStatus.primaryActionLabel}
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
        {!companionMode && lifeStateProjection?.safeMode.active && (
          <div className="border-b border-amber-200 bg-amber-50 px-6 py-2">
            <div className="max-w-3xl text-xs text-amber-800 flex flex-wrap items-center justify-between gap-2">
              <div>
                <span className="font-medium">Safe Mode：</span>
                {lifeStateProjection.safeMode.reason}
                <span className="ml-2">普通对话仍可继续，但“加入记忆”等写入操作建议先暂停。</span>
              </div>
              <Link to={productRoutePath("Settings")} className="underline font-medium">
                打开恢复控制台
              </Link>
            </div>
          </div>
        )}
        {/* Pending Proposals Alert */}
        {!companionMode && projectionPendingReviewCount == null && (
          <div className="border-b border-stone-200 bg-stone-50 px-6 py-2">
            <div className="max-w-3xl text-xs text-stone-700 flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <ShieldCheck size={14} />
                <span className="font-medium">待确认状态读取中</span>
                <span className="text-stone-500">暂不显示确定数量。</span>
              </div>
              <Link to={mailboxRoute()} className="underline font-medium">
                打开 Mailbox
              </Link>
            </div>
          </div>
        )}
        {!companionMode &&
          projectionPendingReviewCount != null &&
          projectionPendingReviewCount > 0 && (
            <div className="border-b border-indigo-100 bg-indigo-50 px-6 py-2">
              <div className="max-w-3xl text-xs text-indigo-800 flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <ShieldCheck size={14} />
                  <span className="font-medium">
                    {projectionPendingReviewCount} 个待确认/已修改
                  </span>
                  <span className="text-indigo-600">
                    {pendingProposals[0]
                      ? `（${pendingProposals[0].affectedPath || pendingProposals[0].proposalType}）`
                      : "（进入 Mailbox 处理）"}
                  </span>
                </div>
                <Link to={mailboxRoute()} className="underline font-medium">
                  去 Mailbox 确认
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
                      <Link
                        to={secondaryRoutePath("LifeModelBuild")}
                        className="ml-2 font-semibold underline"
                      >
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
          {!companionMode && showMainChatDiagnostics && reasoningTrace && (
            <div className="flex justify-start">
              <ReasoningTracePanel
                trace={reasoningTrace}
                show={showReasoningTrace}
                onToggle={() => setShowReasoningTrace(s => !s)}
              />
            </div>
          )}
          {!companionMode && showMainChatDiagnostics && toolCalls.length > 0 && (
            <div className="flex justify-start">
              <div className="max-w-2xl px-4 py-3 rounded-xl text-sm bg-gray-50 text-gray-900 border border-gray-200 w-full">
                <button
                  onClick={() => setShowToolCalls(s => !s)}
                  className="flex items-center gap-2 font-medium mb-2"
                >
                  <Hammer size={16} /> 工具调用 {showToolCalls ? "▲" : "▼"}
                </button>
                {toolCalls.some(
                  call =>
                    call.executionReceipt?.actionEffect === "external_mutation" ||
                    call.executionReceipt?.actionEffect === "unknown"
                ) && (
                  <div className="mb-3 rounded-md bg-orange-50 border border-orange-100 p-2 text-xs text-orange-700 flex items-center gap-2">
                    <span className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-orange-200 text-orange-700 font-bold">
                      !
                    </span>
                    检测到高风险 MCP 操作，请先在 Mailbox 完成审查，再通过任务控制继续。
                  </div>
                )}
                {showToolCalls && (
                  <div className="space-y-2">
                    {toolCalls.map((call, idx) => (
                      <ToolCallCard key={idx} call={call} />
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
                    to={secondaryRoutePath("LifeModelBuild")}
                    className="inline-flex items-center gap-1 bg-indigo-600 text-white px-3 py-1.5 rounded-md text-xs hover:bg-indigo-700"
                  >
                    去构建 <ArrowRight size={14} />
                  </Link>
                  <Link
                    to={productRoutePath("Today")}
                    className="inline-flex items-center gap-1 border border-indigo-200 bg-white text-indigo-700 px-3 py-1.5 rounded-md text-xs hover:bg-indigo-50"
                  >
                    先看今日页 <ArrowRight size={14} />
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
          {messages.map((m, i) => {
            const displayContent =
              m.role === "assistant"
                ? userFacingAssistantContent(m.content, diagnostics)
                : m.content;
            const runForMessage = m.run_id
              ? (runById[m.run_id] ?? (currentRun?.id === m.run_id ? currentRun : null))
              : null;
            return (
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
                  <div className="whitespace-pre-wrap">{displayContent}</div>
                  {m.role === "assistant" && companionMode && m.run_id && (
                    <div className="mt-3 border-t border-stone-100 pt-2">
                      {runForMessage ? (
                        <RuntimeDisclosureStrip
                          view={buildRuntimeDisclosure(runForMessage, {
                            taskState: currentAgentTaskState,
                            ingress: currentAgentIngress,
                          })}
                          runId={m.run_id}
                        />
                      ) : (
                        <div className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
                          正在读取运行记录。
                          <Link
                            to={runDetailRoute(m.run_id)}
                            className="ml-2 font-semibold text-stone-900 underline-offset-4 hover:underline"
                          >
                            打开 Runs
                          </Link>
                        </div>
                      )}
                    </div>
                  )}
                  {m.role === "assistant" && !companionMode && (
                    <div className="mt-3 space-y-2">
                      {(() => {
                        const runMatches = Boolean(runForMessage);
                        const isLast = i === messages.length - 1;
                        if ((!runMatches && !isLast) || !runForMessage) return null;
                        const run = runForMessage;
                        return (
                          <RuntimeDisclosureStrip
                            view={buildRuntimeDisclosure(run, {
                              taskState: currentAgentTaskState,
                              ingress: currentAgentIngress,
                            })}
                            runId={run.id}
                            compact
                          />
                        );
                      })()}
                      <div className="flex flex-wrap gap-2">
                        <button
                          onClick={() =>
                            fillPrompt(buildAssistantActionPrompt("continue", displayContent))
                          }
                          className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        >
                          <MessageSquare size={12} /> 继续追问
                        </button>
                        <button
                          onClick={() =>
                            fillPrompt(buildAssistantActionPrompt("action", displayContent))
                          }
                          className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        >
                          <CheckCircle2 size={12} /> 提炼行动
                        </button>
                        <button
                          onClick={() =>
                            fillPrompt(buildAssistantActionPrompt("state", displayContent))
                          }
                          className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        >
                          <Activity size={12} /> 记录状态
                        </button>
                        <button
                          onClick={() =>
                            fillPrompt(buildAssistantActionPrompt("goal", displayContent))
                          }
                          className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                        >
                          <Target size={12} /> 拆成目标
                        </button>
                        {!companionMode && (
                          <>
                            <button
                              onClick={() =>
                                fillPrompt(
                                  "请把上一条助手回复中值得保留的内容整理成一条记忆提案；不要直接写入，先让我审核。"
                                )
                              }
                              className="inline-flex items-center gap-1 rounded-full bg-white px-2.5 py-1 text-[11px] font-medium text-gray-600 hover:bg-gray-50"
                              title="基于助手回复草拟记忆提案，审核后再写入"
                            >
                              <Sparkles size={12} /> 草拟记忆提案
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
            );
          })}
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
          {!companionMode && mainChatAgentStatusView && (
            <MainChatAgentStatusSurface
              view={mainChatAgentStatusView}
              busy={agentTaskControlBusy}
              error={safeAgentTaskControlError}
              onResume={handleResumeMainChatTask}
              onRetry={() => handleRetryMainChatAction()}
              onCancel={handleCancelMainChatTask}
              onRefreshContext={handleRefreshCurrentMainChatTaskContext}
              onShowTrace={handleShowMainChatStructuredTrace}
            />
          )}
          {!companionMode && hasMainChatExecutionEvidence && (
            <MainChatExecutionEvidence
              state={currentAgentState}
              taskState={currentAgentTaskState}
              kernelEvents={currentKernelEvents}
              toolCalls={toolCalls}
              sending={sending}
              diagnosticsOpen={showMainChatDiagnostics}
              hasDiagnostics={hasMainChatDiagnostics}
              canCancel={canCancelCurrentMainChatTask}
              cancelBusy={agentTaskControlBusy}
              cancelError={safeAgentTaskControlError}
              onCancel={handleCancelMainChatTask}
              onToggleDiagnostics={() => setShowMainChatDiagnostics(open => !open)}
            />
          )}
          {companionMode && (sending || currentAgentTaskState) && (
            <CompanionTaskControlStrip
              taskState={currentAgentTaskState}
              taskViewItem={authoritativeCurrentTaskViewItem}
              busy={agentTaskControlBusy}
              error={safeAgentTaskControlError}
              canResume={canResumeCurrentMainChatTask}
              canRetry={canRetryCurrentMainChatTask}
              canCancel={canCancelCurrentMainChatTask}
              onResume={handleResumeMainChatTask}
              onRetry={() => handleRetryMainChatAction()}
              onCancel={handleCancelMainChatTask}
              onRefresh={handleRefreshCurrentMainChatTask}
            />
          )}
          {!companionMode && showMainChatDiagnostics && currentAgentState && (
            <div className="px-4 py-2">
              <AgentControlPlane
                state={currentAgentState}
                busy={agentTaskControlBusy}
                canResume={canResumeCurrentMainChatTask}
                canRetry={canRetryCurrentMainChatTask}
                canCancel={canCancelCurrentMainChatTask}
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
              {safeAgentTaskControlError && (
                <div className="border-b border-rose-200 bg-rose-50 px-4 py-2 text-xs text-rose-800">
                  {safeAgentTaskControlError}
                </div>
              )}
            </div>
          )}
          {!companionMode &&
            showMainChatDiagnostics &&
            !currentAgentState &&
            currentAgentIngress && (
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
                        {currentAgentTaskState?.session && (
                          <div className="min-w-0">
                            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                              Task
                            </div>
                            <div className="truncate text-stone-900">Main Chat task</div>
                          </div>
                        )}
                        {currentAgentTaskState?.session?.hasPlanSummary && (
                          <div className="min-w-0">
                            <div className="text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                              Current plan
                            </div>
                            <div className="truncate text-stone-900">Available in task trace</div>
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
                          disabled={!canResumeCurrentMainChatTask || agentTaskControlBusy}
                          onClick={handleResumeMainChatTask}
                          className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <Play size={14} />
                        </button>
                        <button
                          type="button"
                          aria-label="Retry failed action"
                          title="Retry failed action"
                          disabled={!canRetryCurrentMainChatTask || agentTaskControlBusy}
                          onClick={() => handleRetryMainChatAction()}
                          className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <RotateCw size={14} />
                        </button>
                        <button
                          type="button"
                          aria-label="Cancel task"
                          title="Cancel task"
                          disabled={!canCancelCurrentMainChatTask || agentTaskControlBusy}
                          onClick={handleCancelMainChatTask}
                          className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          <Ban size={14} />
                        </button>
                      </div>
                    )}
                  </div>
                  {safeAgentTaskControlError && (
                    <div className="mt-2 border-l-2 border-rose-400 bg-rose-50 px-2 py-1 text-rose-900">
                      {safeAgentTaskControlError}
                    </div>
                  )}
                  {currentAgentTaskState?.actions?.length ? (
                    <div className="mt-3">
                      <div className="mb-1 text-[10px] font-semibold uppercase tracking-wide text-stone-500">
                        Diagnostic queue preview
                      </div>
                      <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white/75">
                        {currentAgentTaskState.actions.map(action => {
                          const needsReview =
                            action.policy.requiresProposal || action.policy.requiresConfirmation;
                          return (
                            <div
                              key={action.id}
                              className="grid gap-2 py-2 md:grid-cols-[1fr_auto]"
                            >
                              <div className="min-w-0 px-2">
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="font-semibold text-stone-950">
                                    {action.actionType}
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
                                      {...mailboxLinkTarget({
                                        mainChatTaskSessionId:
                                          currentAgentTaskState?.session?.id ??
                                          currentAgentIngress.agentTaskSessionId,
                                        returnTo: productRoutePath("Companion"),
                                      })}
                                      className="inline-flex h-5 items-center gap-1 rounded-md border border-stone-200 bg-white px-1.5 font-medium text-stone-800 hover:bg-stone-100"
                                    >
                                      <ExternalLink size={12} />
                                      Open Mailbox
                                    </Link>
                                  )}
                                </div>
                                <div className="mt-1 text-stone-700">
                                  {action.policy.reasonCode}
                                </div>
                                {action.failureCode && (
                                  <div className="mt-1 text-rose-700">{action.failureCode}</div>
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
                      data-evidence-state={taskContinuityEvidenceCurrent ? "current" : "unknown"}
                      className="min-w-0 border-y border-stone-200 bg-stone-50/80 px-2 py-2"
                    >
                      <div className="flex flex-wrap items-start gap-2">
                        <div className="min-w-0 flex-1">
                          <div className="truncate font-semibold text-stone-950">
                            {taskContinuityEvidenceTitle}
                          </div>
                          {!taskContinuityEvidenceCurrent && (
                            <div className="mt-1 text-amber-800">
                              Product run evidence is missing or does not match this task. Controls
                              stay disabled until the backend projection is current.
                            </div>
                          )}
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
                          {taskContinuityAllowedControls.includes("resume") && (
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
                          {taskContinuityAllowedControls.includes("retry") &&
                            taskContinuityDetail.retryTargetActionId && (
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
                          {taskContinuityAllowedControls.includes("cancel") && (
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
                          {taskContinuityAllowedControls.includes("refresh_context") && (
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
                                  {action.actionType}
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
                                {action.policy.reasonCode}
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
                              <div className="truncate text-stone-600">
                                {proposal.source.replace(/_/g, " ")} · {proposal.riskLevel} risk
                              </div>
                              {taskContinuityEvidenceCurrent && proposal.status === "pending" && (
                                <div className="flex flex-wrap gap-1">
                                  {proposal.proposalType === "tool_permission" && (
                                    <button
                                      type="button"
                                      disabled={taskContinuityBusy}
                                      onClick={() =>
                                        handleAcceptTaskContinuityProposal(proposal.id)
                                      }
                                      className="inline-flex min-h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
                                    >
                                      Accept proposal
                                    </button>
                                  )}
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
                        <TaskContinuityFinalDeliveryPanel
                          value={taskContinuityDetail.finalDelivery}
                        />
                      )}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
          {!companionMode && showMainChatDiagnostics && currentRun && (
            <div className="px-4 py-2">
              <RuntimeDisclosureStrip
                view={buildRuntimeDisclosure(currentRun, {
                  taskState: currentAgentTaskState,
                  ingress: currentAgentIngress,
                })}
                runId={currentRun.id}
                compact
              />
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
          attachments={currentResources}
          resourceImportBusy={resourceImportBusy}
          resourceImportError={currentResourceImportError}
          resourceImportNotice={currentResourceImportNotice}
          removingResourceIds={removingResourceIds}
          onInputChange={handleInputChange}
          onSelectedSkillIdChange={setSelectedSkillId}
          onAttachResources={handleAttachResources}
          onCancelResourceImport={handleCancelResourceImport}
          onRemoveResource={handleRemoveResource}
          onComposerFocus={() => emitCompanionStage("listening")}
          onSend={handleSend}
          canCancel={canCancelCurrentMainChatTask}
          cancelBusy={agentTaskControlBusy}
          onCancel={handleCancelMainChatTask}
          onContinueStream={handleContinueStream}
          onRetryLastMessage={retryLastUserMessage}
          getFixSuggestion={diag => getFixSuggestion(diag, lifeStateProjection)}
          companionMode={companionMode}
        />
      </div>
    </div>
  );
}
