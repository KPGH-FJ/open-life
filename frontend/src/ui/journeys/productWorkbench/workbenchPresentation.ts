import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  TaskViewModelItem,
  TasksViewModel,
  ViewModelEnvelope,
  ViewModelStatus,
} from "@/tauri";
import type { FoundationStatus } from "@/ui/foundation";
import type {
  WorkbenchBoundarySummary,
  WorkbenchContextSummary,
  WorkbenchEvidenceReference,
} from "@/ui/shell";

export type ProductStatusPresentation = {
  label: string;
  status: FoundationStatus;
  verified?: boolean;
};

function uniqueEvidence(refs: ReadonlyArray<EvidenceRef | undefined>): EvidenceRef[] {
  const seen = new Set<string>();
  return refs.filter((ref): ref is EvidenceRef => {
    if (!ref || seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}

export function collectBoundaryEvidence(
  envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>
): EvidenceRef[] {
  return uniqueEvidence([...(envelope.evidenceRefs ?? []), ...(envelope.data?.evidenceRefs ?? [])]);
}

export function toWorkbenchEvidence(ref: EvidenceRef): WorkbenchEvidenceReference {
  const sourceLabels: Record<EvidenceRef["source"], string> = {
    "backend-readmodel": "后端读模型",
    audit: "审计记录",
    task: "任务记录",
    review: "审核记录",
    memory: "记忆记录",
    lifemodel: "LifeModel",
    settings: "设置记录",
    provider: "模型路由",
  };
  const sensitivityLabels: Record<NonNullable<EvidenceRef["sensitivity"]>, string> = {
    public: "公开",
    local_private: "本机私密",
    sensitive: "敏感",
    redacted: "已脱敏",
  };
  return {
    id: ref.id,
    label: ref.label,
    source: sourceLabels[ref.source],
    sensitivity: ref.sensitivity ? sensitivityLabels[ref.sensitivity] : "未标注",
  };
}

export function boundaryPresentation(
  envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>
): WorkbenchBoundarySummary {
  if (envelope.status === "loading") {
    return {
      label: "正在读取传输边界",
      detail: "读取完成前不判断是否保持本地。",
      status: "neutral",
    };
  }
  if (envelope.status === "error" || envelope.data === null) {
    return {
      label: "传输边界未知",
      detail: "后端边界读取失败；外部动作保持关闭。",
      status: envelope.status === "error" ? "error" : "unknown",
    };
  }
  if (envelope.status === "stale") {
    return {
      label: "传输边界已陈旧",
      detail: "刷新成功前不使用旧边界授权外部动作。",
      status: "stale",
    };
  }

  const boundary = envelope.data;
  const evidencePresent = collectBoundaryEvidence(envelope).length > 0;
  const riskKnown = boundary.risk !== "unknown";
  if (
    boundary.routeType === "local" &&
    boundary.externalTransmission === "not_sent" &&
    riskKnown &&
    evidencePresent
  ) {
    return {
      label: "本地路由，未外传",
      detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
      status: "success",
      verified: true,
    };
  }
  if (boundary.externalTransmission === "possible") {
    return {
      label: "可能发生外部传输",
      detail: "目标或传输结果仍需后端证据确认；外部动作保持关闭。",
      status: "unknown",
    };
  }
  if (
    boundary.externalTransmission === "unknown" ||
    boundary.routeType === "unknown" ||
    !riskKnown ||
    !evidencePresent
  ) {
    return {
      label: "是否外传未知",
      detail: "当前证据不足，不能显示本地确定态；外部动作保持关闭。",
      status: "unknown",
    };
  }
  if (boundary.externalTransmission === "sent") {
    return {
      label: "已发生外部传输",
      detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
      status: "waiting",
    };
  }
  return {
    label: "外部路由当前未发送",
    detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
    status: "neutral",
  };
}

function envelopeStatusPresentation(status: ViewModelStatus): ProductStatusPresentation {
  switch (status) {
    case "loading":
      return { label: "正在读取", status: "neutral" };
    case "stale":
      return { label: "数据已陈旧", status: "stale" };
    case "error":
      return { label: "读取失败", status: "error" };
    case "empty":
      return { label: "暂无内容", status: "neutral" };
    case "ready":
      return { label: "已读取", status: "neutral" };
  }
}

export function tasksContext(envelope: ViewModelEnvelope<TasksViewModel>): WorkbenchContextSummary {
  const base = envelopeStatusPresentation(envelope.status);
  if (envelope.status !== "ready") {
    return { eyebrow: "任务连续性", title: "任务", status: base };
  }
  const needsAttention = envelope.data
    ? envelope.data.summary.waitingPermissionCount +
      envelope.data.summary.waitingReviewCount +
      envelope.data.summary.blockedCount +
      envelope.data.summary.pendingReviewCount +
      envelope.data.summary.completedNeedsEvidenceCount
    : 0;
  return {
    eyebrow: "任务连续性",
    title: "任务",
    status:
      needsAttention > 0 ? { label: `${needsAttention} 项需要处理`, status: "waiting" } : base,
  };
}

export function taskNeedsAttention(item: TaskViewModelItem): boolean {
  return (
    item.needsAttention === true ||
    [
      "waiting_permission",
      "waiting_review",
      "blocked",
      "failed",
      "remote_unknown",
      "interrupted",
      "completed_with_pending_review",
      "completed_needs_evidence",
      "unknown",
    ].includes(item.lifecycleStatus) ||
    item.pendingBlockers.length > 0 ||
    item.pendingReviewItemRefs.length > 0
  );
}

export function taskLifecyclePresentation(item: TaskViewModelItem): ProductStatusPresentation {
  switch (item.lifecycleStatus) {
    case "running":
      return { label: "运行中", status: "neutral" };
    case "waiting_review":
      return { label: "等待审核", status: "waiting" };
    case "waiting_permission":
      return { label: "等待确认", status: "waiting" };
    case "blocked":
      return { label: "已阻断", status: "blocked" };
    case "failed":
      return { label: "失败", status: "error" };
    case "remote_unknown":
      return { label: "远端结果未知", status: "unknown" };
    case "cancelled":
      return { label: "已取消", status: "neutral" };
    case "interrupted":
      return { label: "已中断，可重试", status: "blocked" };
    case "completed_with_pending_review":
      return { label: "待审核，未完成", status: "waiting" };
    case "completed_needs_evidence":
      return { label: "缺少完成证据", status: "blocked" };
    case "completed":
      if (item.finalDeliveryEvidencePresent && item.terminalDeliveryStatus === "delivered") {
        return { label: "已完成", status: "success", verified: true };
      }
      return { label: "完成证据不足", status: "blocked" };
    case "unknown":
      return { label: "状态未知", status: "unknown" };
  }
}

export function taskPrimaryQuestion(envelope: ViewModelEnvelope<TasksViewModel>): string {
  if (envelope.status === "error") return "任务状态暂时无法确认";
  if (envelope.status === "stale") return "当前任务列表已陈旧，只用于查看";
  if (envelope.status === "empty" || envelope.data?.items.length === 0) {
    return "当前没有可展示的任务";
  }
  if (envelope.data?.summary.waitingPermissionCount) return "哪些任务在等待你的确认";
  if (envelope.data?.summary.waitingReviewCount) return "哪些结果在等待你的审核";
  if (envelope.data?.summary.blockedCount) return "哪些任务需要解除阻塞";
  if (envelope.data?.summary.completedNeedsEvidenceCount) return "哪些结果仍缺少完成证据";
  return "哪些任务需要我或可以继续";
}

export function formatBackendTime(value?: string | null): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
