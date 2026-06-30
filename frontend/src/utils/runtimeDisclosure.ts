import type {
  AgentRun,
  MainChatAgentIngressDecision,
  MainChatAgentTaskState,
  MainChatTaskSummary,
  RunEvidenceView,
  RuntimeRouteEvidence,
  RouteIdentity,
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
  if (routeType === "agent_runtime") return { label: "运行时事实", tone: "info" };
  if (routeType === "scripted") return { label: "脚本 proof", tone: "info" };
  if (routeType === "auto") return { label: "自动路由", tone: "info" };
  return { label: "路线未验证", tone: "neutral" };
}

function runtimeRouteEvidenceFromRun(run: AgentRun | null): RuntimeRouteEvidence | null {
  const raw = run?.reasoningTrace?.generation_result?.runtimeRouteEvidence;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  return raw as RuntimeRouteEvidence;
}

function primaryEvidenceRoute(evidence?: RuntimeRouteEvidence | null): RouteIdentity | null {
  return (
    evidence?.actual_route ?? evidence?.last_completed_route ?? evidence?.planned_route ?? null
  );
}

function boundaryFromEvidence(evidence?: RuntimeRouteEvidence | null): {
  label: string;
  tone: ProductTone;
} | null {
  if (!evidence) return null;
  if (evidence.external_transmission === "sent") {
    return { label: "运行证据：已外发", tone: "warning" };
  }
  if (evidence.external_transmission === "not_sent") {
    return { label: "运行证据：未外发", tone: "ready" };
  }
  if (evidence.external_transmission === "unknown") {
    return { label: "外发状态未知", tone: "neutral" };
  }
  return { label: "外发记录未接入", tone: "neutral" };
}

function statusLabel(status?: string): { label: string; tone: ProductTone } {
  if (status === "completed") return { label: "已完成", tone: "ready" };
  if (status === "waiting_permission") return { label: "等待确认", tone: "warning" };
  if (status === "blocked") return { label: "已阻断", tone: "warning" };
  if (status === "timed_out") return { label: "已超时", tone: "danger" };
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
  taskSummary?: MainChatTaskSummary | null,
  evidenceView?: RunEvidenceView | null
): number {
  if (evidenceView) return evidenceView.blockers?.length ?? 0;
  const stateCount = taskState?.session?.pendingBlockers?.length ?? 0;
  const summaryCount = taskSummary?.pendingBlockerCount ?? 0;
  return Math.max(stateCount, summaryCount);
}

function proposalCount(
  run: AgentRun | null,
  taskSummary?: MainChatTaskSummary | null,
  evidenceView?: RunEvidenceView | null
): number {
  if (evidenceView) return evidenceView.proposals?.length ?? 0;
  return Math.max(run?.generatedProposals?.length ?? 0, taskSummary?.pendingProposalCount ?? 0);
}

function nextAction(
  run: AgentRun | null,
  taskState?: MainChatAgentTaskState | null,
  taskSummary?: MainChatTaskSummary | null,
  evidenceView?: RunEvidenceView | null
): string {
  if (evidenceView?.nextRecommendedControl) {
    return evidenceView.nextRecommendedControl.replace(/_/g, " ");
  }
  if (taskSummary?.nextRecommendedControl) {
    return taskSummary.nextRecommendedControl.replace(/_/g, " ");
  }
  if (taskState?.canResume) return "需要继续";
  if (taskState?.canRetry || (run?.status === "failed" && run.error?.recoverable)) return "可重试";
  if (taskState?.canCancel || run?.status === "running") return "可取消";
  if (run?.status === "waiting_permission") return "需要确认";
  if (proposalCount(run, taskSummary, evidenceView) > 0) return "去 Review 处理";
  return run?.status === "completed" ? "无需操作" : "查看详情";
}

export function buildRuntimeDisclosure(
  run: AgentRun | null,
  options: {
    taskState?: MainChatAgentTaskState | null;
    taskSummary?: MainChatTaskSummary | null;
    evidenceView?: RunEvidenceView | null;
    ingress?: MainChatAgentIngressDecision | null;
    runtimeRouteEvidence?: RuntimeRouteEvidence | null;
    strictRuntimeRouteEvidence?: boolean;
  } = {}
): RuntimeDisclosureView {
  const runtimeRouteEvidence =
    options.runtimeRouteEvidence ??
    options.evidenceView?.routeEvidence ??
    runtimeRouteEvidenceFromRun(run);
  const strictRuntimeRouteEvidence =
    options.strictRuntimeRouteEvidence || Boolean(options.evidenceView);
  const evidenceRoute = primaryEvidenceRoute(runtimeRouteEvidence);
  const route = routeTypeLabel(
    evidenceRoute?.route_type ??
      (strictRuntimeRouteEvidence ? undefined : run?.modelRoute?.routeType)
  );
  const boundary =
    boundaryFromEvidence(runtimeRouteEvidence) ??
    (strictRuntimeRouteEvidence
      ? { label: "外发记录未接入", tone: "neutral" as ProductTone }
      : boundaryLabel(run, options.ingress));
  const outcome = statusLabel(options.evidenceView?.lifecycleState ?? run?.status);
  const tools =
    options.evidenceView?.actionCount ?? run?.toolCallCount ?? run?.actions?.length ?? 0;
  const toolsLabel =
    tools > 0
      ? `工具 ${tools}`
      : run?.contextSummary?.usedToolsPrompt
        ? "工具提示已注入"
        : "未调用工具";
  const proposals = proposalCount(run, options.taskSummary, options.evidenceView);
  const blockers = blockerCount(options.taskState, options.taskSummary, options.evidenceView);
  const providerLabel =
    evidenceRoute?.provider ||
    (runtimeRouteEvidence || strictRuntimeRouteEvidence
      ? "provider 未验证"
      : run?.modelRoute?.provider) ||
    "provider 未验证";
  const modelLabel =
    evidenceRoute?.model ||
    (runtimeRouteEvidence || strictRuntimeRouteEvidence
      ? "model 未验证"
      : run?.modelRoute?.model) ||
    "model 未验证";
  const memoryHits = run?.contextSummary?.memoryHitCount ?? 0;
  const routeReason =
    evidenceRoute?.reason ||
    (strictRuntimeRouteEvidence ? undefined : run?.modelRoute?.reason) ||
    options.ingress?.reasonSummary;
  const fallbackReason =
    runtimeRouteEvidence?.fallback?.reason ||
    (strictRuntimeRouteEvidence ? undefined : run?.modelRoute?.fallbackReason);
  const technicalRows = [
    { label: "Run ID", value: run?.id ?? "未记录" },
    { label: "Provider", value: providerLabel },
    { label: "Model", value: modelLabel },
    { label: "Route reason", value: routeReason || "未记录" },
    {
      label: "Route confidence",
      value: runtimeRouteEvidence?.truth_confidence ?? "未记录",
    },
    {
      label: "External transmission",
      value: runtimeRouteEvidence?.external_transmission ?? "未记录",
    },
    { label: "Privacy class", value: options.ingress?.privacyRisk?.privacyClass ?? "未记录" },
    {
      label: "Pending blockers",
      value:
        options.evidenceView?.blockers?.join(", ") ||
        options.taskState?.session?.pendingBlockers?.join(", ") ||
        `${blockers}`,
    },
  ];

  if (runtimeRouteEvidence?.evidence_id) {
    technicalRows.push({ label: "Evidence ID", value: runtimeRouteEvidence.evidence_id });
  }
  if (options.evidenceView?.redactionState) {
    technicalRows.push({ label: "Redaction", value: options.evidenceView.redactionState });
  }
  if (fallbackReason) {
    technicalRows.push({ label: "Fallback", value: fallbackReason });
  }
  if (!strictRuntimeRouteEvidence && run?.modelRoute?.retryCount) {
    technicalRows.push({ label: "Retry", value: `${run.modelRoute.retryCount}` });
  }

  return {
    routeLabel: route.label === "路线未验证" ? route.label : `${route.label} · ${providerLabel}`,
    routeTone: route.tone,
    boundaryLabel: boundary.label,
    boundaryTone: boundary.tone,
    outcomeLabel: outcome.label,
    outcomeTone: outcome.tone,
    toolsLabel,
    proposalsLabel: proposals > 0 ? `待确认 ${proposals}` : "无新提案",
    blockersLabel: blockers > 0 ? `阻断 ${blockers}` : "无阻断",
    nextActionLabel: nextAction(run, options.taskState, options.taskSummary, options.evidenceView),
    providerLabel,
    modelLabel,
    routeReason,
    fallbackReason,
    memoryLabel: `参考记忆 ${memoryHits} 条`,
    technicalRows,
  };
}
