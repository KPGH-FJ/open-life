import type { EvidenceRef, ReviewItem } from "@/tauri";
import type { FoundationStatus } from "@/ui/foundation";
import type {
  WorkbenchContextSummary,
  WorkbenchEvidenceReference,
  WorkbenchInspectorModel,
} from "@/ui/shell";
import { toWorkbenchEvidence } from "@/ui/journeys/readOnly/readOnlySpinePresentation";
import type { DurableTruthSnapshot } from "./durableTruthDataSource";

export type DurableTruthLifecycle =
  | "pending_review"
  | "deferred"
  | "approved_not_applied"
  | "applying"
  | "applied"
  | "failed"
  | "rolled_back"
  | "rejected"
  | "unknown"
  | "none";

export type DurableTruthLifecyclePresentation = {
  lifecycle: DurableTruthLifecycle;
  label: string;
  status: FoundationStatus;
  verified?: boolean;
  detail: string;
};

const durableTypes = new Set<ReviewItem["type"]>([
  "goal_update",
  "state_update",
  "preference_update",
  "capability_update",
  "memory_write",
  "memory_archive",
  "life_model_update",
]);

export function isDurableReviewItem(item: ReviewItem): boolean {
  return durableTypes.has(item.type);
}

export function durableReviewItems(snapshot: DurableTruthSnapshot | null): ReviewItem[] {
  if (!snapshot || !["ready", "stale"].includes(snapshot.reviewEnvelope.status)) return [];
  return (snapshot.reviewEnvelope.data?.items ?? []).filter(isDurableReviewItem);
}

function requiredOwnerReady(snapshot: DurableTruthSnapshot, item: ReviewItem): boolean {
  if (snapshot.reviewEnvelope.status !== "ready") return false;
  if (item.type === "memory_write" || item.type === "memory_archive") {
    return snapshot.memoryEnvelope.status === "ready";
  }
  return snapshot.lifeModelEnvelope.status === "ready";
}

function hasExactLifeModelAppliedProof(snapshot: DurableTruthSnapshot, item: ReviewItem): boolean {
  if (snapshot.lifeModelEnvelope.status !== "ready") return false;
  const expectedChangeRef = `proposal:${item.source.proposalId}`;
  return Boolean(
    snapshot.lifeModelEnvelope.data?.materializedChanges.some(
      change =>
        change.changeRef.id === expectedChangeRef && change.materializationStatus === "applied"
    )
  );
}

export function durableLifecyclePresentation(
  snapshot: DurableTruthSnapshot | null,
  item: ReviewItem | null,
  owner: "all" | "life_model" | "memory" = "all"
): DurableTruthLifecyclePresentation {
  if (!snapshot) {
    return {
      lifecycle: "unknown",
      label: "正在读取",
      status: "neutral",
      detail: "LifeModel、Memory 与审核状态尚未完成核对。",
    };
  }
  const ownerStatuses = item
    ? [
        snapshot.reviewEnvelope.status,
        item.type === "memory_write" || item.type === "memory_archive"
          ? snapshot.memoryEnvelope.status
          : snapshot.lifeModelEnvelope.status,
      ]
    : [snapshot.reviewEnvelope.status];
  if (ownerStatuses.some(status => status === "error")) {
    return {
      lifecycle: "unknown",
      label: "状态不可用",
      status: "error",
      detail: "至少一个后端读模型读取失败，当前不能确认长期状态。",
    };
  }
  if (ownerStatuses.some(status => status === "stale")) {
    return {
      lifecycle: "unknown",
      label: "状态已陈旧",
      status: "stale",
      detail: "刷新成功前不使用旧数据确认决定、应用或回滚结果。",
    };
  }
  if (ownerStatuses.some(status => status === "loading")) {
    return {
      lifecycle: "unknown",
      label: "正在核对",
      status: "neutral",
      detail: "至少一个长期状态读模型仍在读取，当前不确认变更结果。",
    };
  }
  if (!item) {
    const lifeModel = snapshot.lifeModelEnvelope.data;
    const memory = snapshot.memoryEnvelope.data;
    const unresolvedLifeModelReferences = Boolean(
      (lifeModel?.pendingUpdateCounts.candidate ?? 0) > 0 ||
      (lifeModel?.pendingUpdateCounts.pendingReview ?? 0) > 0 ||
      (lifeModel?.pendingUpdateCounts.approvedNotApplied ?? 0) > 0 ||
      (lifeModel?.pendingUpdateCounts.failedMaterialization ?? 0) > 0 ||
      (lifeModel?.candidateChanges.length ?? 0) > 0 ||
      (lifeModel?.relatedReviewItemRefs.length ?? 0) > 0
    );
    const unresolvedMemoryReferences = Boolean(
      (memory?.summary.reviewRequiredCount ?? 0) > 0 ||
      (memory?.summary.pendingMaterializationCount ?? 0) > 0 ||
      (memory?.summary.failedMaterializationCount ?? 0) > 0 ||
      (memory?.reviewItemRefs.length ?? 0) > 0
    );
    const unresolvedReferences =
      (owner !== "memory" && unresolvedLifeModelReferences) ||
      (owner !== "life_model" && unresolvedMemoryReferences);
    if (unresolvedReferences) {
      return {
        lifecycle: "unknown",
        label: "变更状态不完整",
        status: "unknown",
        detail: "长期状态读模型报告了待处理引用，但审核中心缺少对应审核项。",
      };
    }
    return {
      lifecycle: "none",
      label: "没有待核对变更",
      status: "neutral",
      detail: "后端没有提供可展示的长期状态审核项。",
    };
  }
  if (!requiredOwnerReady(snapshot, item)) {
    return {
      lifecycle: "unknown",
      label: "状态待核对",
      status: "unknown",
      detail: "对应长期状态读模型尚未处于可用状态。",
    };
  }
  if (item.status === "pending" || item.status === "edited") {
    return {
      lifecycle: "pending_review",
      label: "等待决定",
      status: "waiting",
      detail: "建议尚未批准，也没有写入长期状态。",
    };
  }
  if (item.status === "deferred") {
    return {
      lifecycle: "deferred",
      label: "稍后处理",
      status: "waiting",
      detail: "决定已延期；当前长期状态没有因此改变。",
    };
  }
  if (item.status === "rejected") {
    return {
      lifecycle: "rejected",
      label: "已拒绝",
      status: "neutral",
      detail: "建议已拒绝，没有应用到长期状态。",
    };
  }
  if (item.status !== "approved") {
    return {
      lifecycle: "unknown",
      label: "决定状态未知",
      status: "unknown",
      detail: "后端没有提供可确认的决定状态。",
    };
  }

  if (item.materializationStatus === "applying") {
    return {
      lifecycle: "applying",
      label: "正在应用",
      status: "waiting",
      detail: "后端确认应用过程已经开始，但尚未形成最终长期状态。",
    };
  }
  if (item.materializationStatus === "failed") {
    return {
      lifecycle: "failed",
      label: "应用失败",
      status: "error",
      detail: "决定仍为已批准，但本次应用没有成功。",
    };
  }
  if (item.materializationStatus === "rolled_back") {
    return {
      lifecycle: "rolled_back",
      label: "已回滚",
      status: "waiting",
      detail: "后端读模型确认此前应用已回滚；当前没有可调用的再次应用动作。",
    };
  }
  if (item.materializationStatus === "applied") {
    const exactProof =
      item.type === "memory_write" || item.type === "memory_archive"
        ? snapshot.memoryEnvelope.status === "ready"
        : hasExactLifeModelAppliedProof(snapshot, item);
    return exactProof
      ? {
          lifecycle: "applied",
          label: "已应用",
          status: "success",
          verified: true,
          detail: "刷新后的精确审核项与长期状态读模型共同确认变更已经应用。",
        }
      : {
          lifecycle: "unknown",
          label: "应用证明不完整",
          status: "unknown",
          detail: "审核项报告已应用，但对应长期状态读模型缺少精确匹配证明。",
        };
  }
  if (
    item.materializationStatus === "not_started" ||
    item.materializationStatus === "not_applicable"
  ) {
    return {
      lifecycle: "approved_not_applied",
      label: "已批准，尚未应用",
      status: "neutral",
      detail: "批准只记录决定；长期状态仍等待独立应用结果。",
    };
  }
  return {
    lifecycle: "unknown",
    label: "应用状态未知",
    status: "unknown",
    detail: "后端没有提供可确认的应用结果。",
  };
}

function uniqueEvidence(
  refs: ReadonlyArray<EvidenceRef | undefined>
): WorkbenchEvidenceReference[] {
  const seen = new Set<string>();
  return refs
    .filter((ref): ref is EvidenceRef => {
      if (!ref || seen.has(ref.id)) return false;
      seen.add(ref.id);
      return true;
    })
    .map(toWorkbenchEvidence);
}

export function durableTruthContext(
  snapshot: DurableTruthSnapshot | null,
  item: ReviewItem | null
): WorkbenchContextSummary {
  const state = durableLifecyclePresentation(snapshot, item);
  return {
    eyebrow: "个人智能",
    title: "关于我与 Agent 记忆",
    status: { label: state.label, status: state.status, verified: state.verified },
  };
}

export function durableTruthInspector(
  snapshot: DurableTruthSnapshot | null,
  item: ReviewItem | null,
  selectedEvidence: string,
  builderError?: string | null
): WorkbenchInspectorModel {
  if (!snapshot) {
    return {
      title: "个人智能依据",
      conclusion: "正在分别读取 LifeModel、Agent Memory 与审核状态。",
      risk: "读取完成前不确认长期状态。",
      nextAction: "等待三个后端读模型返回。",
      evidence: [],
    };
  }
  const state = durableLifecyclePresentation(snapshot, item);
  const lifeModel = snapshot.lifeModelEnvelope.data;
  const memory = snapshot.memoryEnvelope.data;
  const evidence = uniqueEvidence([
    ...(snapshot.lifeModelEnvelope.evidenceRefs ?? []),
    ...(lifeModel?.sourceRefs ?? []),
    ...(lifeModel?.provenanceRefs ?? []),
    ...(snapshot.memoryEnvelope.evidenceRefs ?? []),
    ...(memory?.sourceRefs ?? []),
    ...(snapshot.reviewEnvelope.evidenceRefs ?? []),
    ...(item?.evidenceRefs ?? []),
    ...(item?.decisionContext.evidenceRefs ?? []),
  ]);
  const limitations = [
    ...(lifeModel?.contractLimitations ?? []),
    ...(memory?.contractLimitations ?? []),
  ];

  return {
    title: item?.decisionContext.title ?? "个人智能依据",
    conclusion: `${state.label}。${state.detail}`,
    risk:
      state.lifecycle === "unknown"
        ? "缺失、陈旧或不一致的来源不能证明变更已经进入长期状态。"
        : item
          ? `风险级别由后端标记为 ${item.risk}；页面不重新分级。`
          : "当前没有精确审核项，不能从汇总数量推断单条变更状态。",
    nextAction:
      state.lifecycle === "pending_review" || state.lifecycle === "deferred"
        ? "打开精确审核项，比较当前值、建议值、原因和影响后再决定。"
        : state.lifecycle === "approved_not_applied"
          ? "等待后端提供应用结果；没有 typed apply action 时保持只读。"
          : state.lifecycle === "failed"
            ? "查看失败证据；等待后端提供可验证的重试或回滚动作。"
            : "刷新并核对来源；不要从历史命令回调推断新状态。",
    evidence,
    evidenceFeedback: selectedEvidence
      ? `已选择 ${selectedEvidence}；这里只展示引用元数据，不展开或修改敏感内容。`
      : evidence.length === 0
        ? "当前没有可展示的后端证据引用。"
        : undefined,
    technicalDetails: [
      { label: "lifeModelStatus", value: snapshot.lifeModelEnvelope.status },
      { label: "memoryStatus", value: snapshot.memoryEnvelope.status },
      { label: "reviewStatus", value: snapshot.reviewEnvelope.status },
      { label: "truthMode", value: lifeModel?.truthMode ?? "unknown" },
      { label: "reviewItemId", value: item?.id ?? "none" },
      { label: "proposalId", value: item?.source.proposalId ?? "none" },
      { label: "decision", value: item?.status ?? "none" },
      { label: "materialization", value: item?.materializationStatus ?? "none" },
      { label: "lifecycle", value: state.lifecycle },
      { label: "builderError", value: builderError ?? "none" },
      { label: "limitations", value: limitations.join(" | ") || "none" },
      {
        label: "diagnostics",
        value:
          snapshot.diagnostics
            .map(
              entry => `${entry.id}:${entry.status}${entry.message ? ` (${entry.message})` : ""}`
            )
            .join(" | ") || "none",
      },
    ],
  };
}
