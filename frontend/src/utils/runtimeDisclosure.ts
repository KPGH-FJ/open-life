import type {
  AgentRun,
  MainChatAgentIngressDecision,
  MainChatAgentTaskState,
  MainChatTaskSummary,
} from "../tauri";
import type { ProductTone } from "../components/product/ProductPrimitives";

export type RuntimeDisclosureView = {
  routeLabel: string;
  routeTone: ProductTone;
  boundaryLabel: string;
  boundaryTone: ProductTone;
  outcomeLabel: string;
  outcomeTone: ProductTone;
  toolsLabel: string;
  proposalsLabel: string;
  blockersLabel: string;
  nextActionLabel: string;
  providerLabel: string;
  modelLabel: string;
  routeReason?: string;
  fallbackReason?: string;
  memoryLabel?: string;
  technicalRows: Array<{ label: string; value: string }>;
};

function routeTypeLabel(routeType?: string): { label: string; tone: ProductTone } {
  if (routeType === "local") return { label: "本地路线", tone: "ready" };
  if (routeType === "cloud") return { label: "云端路线", tone: "warning" };
  if (routeType === "auto") return { label: "自动路由", tone: "info" };
  return { label: "路线未记录", tone: "neutral" };
}

function statusLabel(status?: string): { label: string; tone: ProductTone } {
  if (status === "completed") return { label: "已完成", tone: "ready" };
  if (status === "waiting_permission") return { label: "等待确认", tone: "warning" };
  if (status === "failed") return { label: "失败", tone: "danger" };
  if (status === "cancelled") return { label: "已取消", tone: "neutral" };
  if (status === "running") return { label: "运行中", tone: "info" };
  return { label: "状态未记录", tone: "neutral" };
}

function boundaryLabel(
  run: AgentRun | null,
  ingress?: MainChatAgentIngressDecision | null
): { label: string; tone: ProductTone } {
  const privacy = ingress?.privacyRisk;
  const route = run?.modelRoute;
  if (privacy?.localOnlyRequired) {
    return { label: "LocalOnly · 不调用云端", tone: "ready" };
  }
  if (route?.routeType === "local") {
    return { label: "留在本机", tone: "ready" };
  }
  if (route?.routeType === "cloud") {
    return { label: "会离开本机", tone: "warning" };
  }
  if (privacy?.externalWriteLike || privacy?.writeLike) {
    return { label: "外部/写入需确认", tone: "warning" };
  }
  if (privacy?.riskLevel && privacy.riskLevel !== "none" && privacy.riskLevel !== "low") {
    return { label: `隐私边界：${privacy.riskLevel}`, tone: "warning" };
  }
  return { label: "边界未记录", tone: "neutral" };
}

function blockerCount(
  taskState?: MainChatAgentTaskState | null,
  taskSummary?: MainChatTaskSummary | null
): number {
  const stateCount = taskState?.session?.pendingBlockers?.length ?? 0;
  const summaryCount = taskSummary?.pendingBlockerCount ?? 0;
  return Math.max(stateCount, summaryCount);
}

function proposalCount(run: AgentRun | null, taskSummary?: MainChatTaskSummary | null): number {
  return Math.max(run?.generatedProposals?.length ?? 0, taskSummary?.pendingProposalCount ?? 0);
}

function nextAction(
  run: AgentRun | null,
  taskState?: MainChatAgentTaskState | null,
  taskSummary?: MainChatTaskSummary | null
): string {
  if (taskSummary?.nextRecommendedControl) {
    return taskSummary.nextRecommendedControl.replace(/_/g, " ");
  }
  if (taskState?.canResume) return "需要继续";
  if (taskState?.canRetry || (run?.status === "failed" && run.error?.recoverable)) return "可重试";
  if (taskState?.canCancel || run?.status === "running") return "可取消";
  if (run?.status === "waiting_permission") return "需要确认";
  if (proposalCount(run, taskSummary) > 0) return "去 Review 处理";
  return run?.status === "completed" ? "无需操作" : "查看详情";
}

export function buildRuntimeDisclosure(
  run: AgentRun | null,
  options: {
    taskState?: MainChatAgentTaskState | null;
    taskSummary?: MainChatTaskSummary | null;
    ingress?: MainChatAgentIngressDecision | null;
  } = {}
): RuntimeDisclosureView {
  const route = routeTypeLabel(run?.modelRoute?.routeType);
  const boundary = boundaryLabel(run, options.ingress);
  const outcome = statusLabel(run?.status);
  const tools = run?.toolCallCount ?? run?.actions?.length ?? 0;
  const toolsLabel =
    tools > 0
      ? `工具 ${tools}`
      : run?.contextSummary?.usedToolsPrompt
        ? "工具提示已注入"
        : "未调用工具";
  const proposals = proposalCount(run, options.taskSummary);
  const blockers = blockerCount(options.taskState, options.taskSummary);
  const providerLabel = run?.modelRoute?.provider || "provider 未记录";
  const modelLabel = run?.modelRoute?.model || "model 未记录";
  const memoryHits = run?.contextSummary?.memoryHitCount ?? 0;
  const routeReason = run?.modelRoute?.reason || options.ingress?.reasonSummary;
  const fallbackReason = run?.modelRoute?.fallbackReason;
  const technicalRows = [
    { label: "Run ID", value: run?.id ?? "未记录" },
    { label: "Provider", value: providerLabel },
    { label: "Model", value: modelLabel },
    { label: "Route reason", value: routeReason || "未记录" },
    { label: "Privacy class", value: options.ingress?.privacyRisk?.privacyClass ?? "未记录" },
    {
      label: "Pending blockers",
      value: options.taskState?.session?.pendingBlockers?.join(", ") || `${blockers}`,
    },
  ];

  if (fallbackReason) {
    technicalRows.push({ label: "Fallback", value: fallbackReason });
  }
  if (run?.modelRoute?.retryCount) {
    technicalRows.push({ label: "Retry", value: `${run.modelRoute.retryCount}` });
  }

  return {
    routeLabel: `${route.label} · ${providerLabel}`,
    routeTone: route.tone,
    boundaryLabel: boundary.label,
    boundaryTone: boundary.tone,
    outcomeLabel: outcome.label,
    outcomeTone: outcome.tone,
    toolsLabel,
    proposalsLabel: proposals > 0 ? `待确认 ${proposals}` : "无新提案",
    blockersLabel: blockers > 0 ? `阻断 ${blockers}` : "无阻断",
    nextActionLabel: nextAction(run, options.taskState, options.taskSummary),
    providerLabel,
    modelLabel,
    routeReason,
    fallbackReason,
    memoryLabel: `参考记忆 ${memoryHits} 条`,
    technicalRows,
  };
}
