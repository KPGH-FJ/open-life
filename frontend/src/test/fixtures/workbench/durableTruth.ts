import type {
  EvidenceRef,
  LifeModelViewModel,
  MemoryLaneSummary,
  MemoryViewModel,
  ReviewAction,
  ReviewCenterViewModel,
  ReviewItem,
  ViewModelEnvelope,
  ViewModelStatus,
} from "@/tauri";
import type { DurableTruthSnapshot } from "@/ui/journeys/durableTruth";
import type { WorkbenchFixtureId } from "./readOnly";

export type DurableFixtureStage =
  | "pending"
  | "approved_not_applied"
  | "applying"
  | "applied"
  | "failed"
  | "rolled_back"
  | "rejected"
  | "deferred";

export const durableReviewItemId = "review-lifemodel-focus-preference";
const durableProposalId = "proposal-focus-morning";
const generatedAt = "2026-07-21T08:45:00.000Z";

const conversationEvidence: EvidenceRef = {
  id: "conversation:weekly-planning:focus-pattern",
  label: "最近四次周计划中的专注时段",
  source: "audit",
  sensitivity: "local_private",
};

const preferenceEvidence: EvidenceRef = {
  id: `proposal:${durableProposalId}`,
  label: "上午深度工作偏好建议",
  source: "review",
  sensitivity: "local_private",
};

const lifeModelEvidence: EvidenceRef = {
  id: "lifemodel:current-compatibility-view",
  label: "当前 LifeModel 兼容视图",
  source: "lifemodel",
  sensitivity: "local_private",
};

const memoryEvidence: EvidenceRef = {
  id: "memory:writing-feedback:conclusion-first",
  label: "先结论后细节的写作反馈",
  source: "memory",
  sensitivity: "local_private",
};

function reviewAction(
  kind: "approve" | "reject" | "later" | "apply" | "view_evidence",
  enabled = true,
  disabledReason?: string
): ReviewAction {
  const labels = {
    approve: "批准变更",
    reject: "拒绝",
    later: "稍后处理",
    apply: "应用变更",
    view_evidence: "查看依据",
  } as const;
  const effect =
    kind === "apply"
      ? "materialization_request"
      : kind === "view_evidence"
        ? "evidence_only"
        : "decision_only";
  return {
    id: `${durableReviewItemId}:${kind}`,
    label: labels[kind],
    kind,
    effect,
    enabled,
    ...(enabled ? {} : { disabledReason: disabledReason ?? "当前动作不可用。" }),
    requiresConfirmation: kind === "approve" || kind === "apply",
    targetReviewItemId: durableReviewItemId,
    expectedMaterializationStatusAfterDispatch:
      kind === "approve" ? "not_started" : kind === "apply" ? "applying" : "not_started",
    completionProofAfterDispatch: false,
  } as ReviewAction;
}

function materializationStatus(stage: DurableFixtureStage): ReviewItem["materializationStatus"] {
  if (stage === "applying") return "applying";
  if (stage === "applied") return "applied";
  if (stage === "failed") return "failed";
  if (stage === "rolled_back") return "rolled_back";
  return "not_started";
}

export function durableReviewItem(stage: DurableFixtureStage): ReviewItem {
  const status: ReviewItem["status"] =
    stage === "pending"
      ? "pending"
      : stage === "deferred"
        ? "deferred"
        : stage === "rejected"
          ? "rejected"
          : "approved";
  const awaitingDecision = status === "pending" || status === "deferred";
  const allowedActions: ReviewAction[] = awaitingDecision
    ? [
        reviewAction("approve"),
        reviewAction("reject"),
        reviewAction("later", status !== "deferred", "这项建议已经设为稍后处理。"),
        reviewAction("view_evidence"),
      ]
    : status === "approved" && stage === "approved_not_applied"
      ? [
          reviewAction(
            "apply",
            false,
            "后端尚未为该审核项提供可调用的应用命令；批准不等于已应用。"
          ),
          reviewAction("view_evidence"),
        ]
      : [reviewAction("view_evidence")];

  return {
    id: durableReviewItemId,
    type: "life_model_update",
    source: {
      kind: "proposal",
      proposalId: durableProposalId,
      proposalSource: "main_chat_agent",
      sourceDetail: "从近期周计划对话归纳出的工作偏好建议",
      runId: "run-weekly-planning-04",
    },
    status,
    materializationStatus: materializationStatus(stage),
    decisionContext: {
      reviewItemId: durableReviewItemId,
      title: "把上午作为优先深度工作时段",
      summary: "建议在工作日计划中优先把需要持续专注的任务安排到上午。",
      before: {
        kind: "text",
        summary: "尚未记录稳定的深度工作时间偏好",
        sensitivity: "local_private",
        truncated: false,
      },
      after: {
        kind: "text",
        summary: "工作日上午优先安排需要持续专注的任务",
        sensitivity: "local_private",
        truncated: false,
      },
      reasonSummary: "最近四次周计划中，上午的长任务完成度更稳定，且较少被会议打断。",
      sourceSummary: "来自本机保存的近期周计划对话摘要，不包含外部数据。",
      impactSummary: "后续计划建议会优先使用上午时段；不会自动移动日历或创建任务。",
      affectedObjectLabels: ["LifeModel · 工作偏好", "后续计划建议"],
      evidenceRefs: [conversationEvidence, preferenceEvidence],
    },
    allowedActions,
    risk: "low",
    evidenceRefs: [conversationEvidence, preferenceEvidence],
    targetRefs: [
      { id: "lifemodel:preferences:focus-time", kind: "lifemodel", label: "工作时段偏好" },
      { id: durableProposalId, kind: "proposal", label: "上午深度工作建议" },
    ],
  };
}

function lifeModel(stage: DurableFixtureStage): LifeModelViewModel {
  const pending = stage === "pending" || stage === "deferred";
  const approvedNotApplied = stage === "approved_not_applied" || stage === "applying";
  const failed = stage === "failed";
  const applied = stage === "applied";
  return {
    truthMode: "unknown",
    canonicalSummary: null,
    versionHistory: [],
    legacyMigrationPreview: null,
    trustQualityState: {
      readiness: "usable_with_limits",
      warningRefs: [],
      ownerStatus: "PARTIAL",
    },
    pendingUpdateCounts: {
      candidate: pending ? 1 : 0,
      pendingReview: pending ? 1 : 0,
      approvedNotApplied: approvedNotApplied ? 1 : 0,
      failedMaterialization: failed ? 1 : 0,
      ownerStatus: "PARTIAL",
    },
    provenanceRefs: [lifeModelEvidence, conversationEvidence],
    candidateChanges:
      pending || approvedNotApplied || failed
        ? [
            {
              changeRef: {
                id: `proposal:${durableProposalId}`,
                kind: "proposal",
                label: "上午深度工作偏好",
              },
              title: "把上午作为优先深度工作时段",
              changeKind: "update",
              affectedDimensionIds: ["goals", "state"],
              reviewItemRefs: [
                { id: durableReviewItemId, kind: "review_item", label: "工作偏好建议" },
              ],
              evidenceRefs: [conversationEvidence, preferenceEvidence],
              decisionStatus:
                stage === "pending" ? "pending" : stage === "deferred" ? "postponed" : "accepted",
            },
          ]
        : [],
    materializedChanges: applied
      ? [
          {
            changeRef: {
              id: `proposal:${durableProposalId}`,
              kind: "proposal",
              label: "上午深度工作偏好",
            },
            title: "把上午作为优先深度工作时段",
            materializationStatus: "applied",
            materializedAt: "2026-07-21T08:46:00.000Z",
            rollbackAvailable: false,
            evidenceRefs: [preferenceEvidence, lifeModelEvidence],
          },
        ]
      : [],
    manualOverrideState: {
      active: false,
      blockedReason: "手动修改需要独立的受治理保存契约。",
      draftRef: null,
      saveAction: null,
      reviewItemRefs: [],
      evidenceRefs: [],
      ownerStatus: "PARTIAL",
    },
    relatedReviewItemRefs: [
      { id: durableReviewItemId, kind: "review_item", label: "工作偏好建议" },
    ],
    memoryLinkage: {
      linkedMemoryCount: 18,
      candidateMemoryCount: pending ? 1 : 0,
      materializedMemoryCount: applied ? 1 : 0,
      conflictCount: 0,
      memoryRefs: [{ id: memoryEvidence.id, kind: "memory", label: "先结论后细节的写作反馈" }],
      evidenceRefs: [memoryEvidence],
      linkageStatus: "partial",
      tierSummary: { total: 18, tier1: 6, tier2: 8, tier3: 4, archived: 2 },
      ownerStatus: "PHASE_2_REQUIRED",
    },
    sourceRefs: [lifeModelEvidence, memoryEvidence, conversationEvidence],
    contractLimitations: [
      "当前只提供兼容视图，不代表完整 canonical LifeModel。",
      "只有精确 materialized change 证明才能显示已应用。",
    ],
  };
}

function emptyLifeModel(): LifeModelViewModel {
  const base = lifeModel("pending");
  return {
    ...base,
    truthMode: "unknown",
    canonicalSummary: null,
    versionHistory: [],
    legacyMigrationPreview: null,
    trustQualityState: {
      readiness: "not_built",
      warningRefs: [],
      ownerStatus: "UNKNOWN",
    },
    pendingUpdateCounts: {
      candidate: 0,
      pendingReview: 0,
      approvedNotApplied: 0,
      failedMaterialization: 0,
      ownerStatus: "UNKNOWN",
    },
    provenanceRefs: [],
    candidateChanges: [],
    materializedChanges: [],
    manualOverrideState: {
      active: false,
      blockedReason: "尚未建立 LifeModel；首次建立只会创建审核建议。",
      draftRef: null,
      saveAction: null,
      reviewItemRefs: [],
      evidenceRefs: [],
      ownerStatus: "UNKNOWN",
    },
    relatedReviewItemRefs: [],
    memoryLinkage: {
      linkedMemoryCount: 0,
      candidateMemoryCount: 0,
      materializedMemoryCount: 0,
      conflictCount: 0,
      memoryRefs: [],
      evidenceRefs: [],
      linkageStatus: "unknown",
      tierSummary: { total: null, tier1: null, tier2: null, tier3: null, archived: null },
      ownerStatus: "UNKNOWN",
    },
    sourceRefs: [],
    contractLimitations: [
      "该 fixture 只证明首次建立交互，不代表后端已有 LifeModel。",
      "Builder 候选必须先进入审核，不能直接成为长期状态。",
    ],
  };
}

function lane(
  laneId: MemoryLaneSummary["lane"],
  label: string,
  overrides: Partial<MemoryLaneSummary> = {}
): MemoryLaneSummary {
  return {
    lane: laneId,
    label,
    totalCount: 0,
    activeCount: 0,
    candidateCount: 0,
    pendingReviewCount: 0,
    confirmedCount: 0,
    materializedCount: 0,
    rolledBackCount: 0,
    archivedCount: 0,
    reviewItemRefs: [],
    evidenceRefs: [],
    ...overrides,
  };
}

function memory(stage: DurableFixtureStage): MemoryViewModel {
  const pending = stage === "pending" || stage === "deferred";
  const pendingMaterialization = stage === "approved_not_applied" || stage === "applying";
  const applied = stage === "applied";
  const failed = stage === "failed";
  const rolledBack = stage === "rolled_back";
  return {
    summary: {
      totalLifecycleRecords: 24,
      activeMemoryCount: 18,
      reviewRequiredCount: pending ? 1 : 0,
      materializedCount: 12 + (applied ? 1 : 0),
      pendingMaterializationCount: pendingMaterialization ? 1 : 0,
      failedMaterializationCount: failed ? 1 : 0,
      rolledBackCount: rolledBack ? 1 : 0,
      archivedVectorCount: 2,
      conflictCount: 0,
      tierSummary: { total: 18, tier1: 6, tier2: 8, tier3: 4, archived: 2 },
    },
    lifecycleSummary: {
      candidateCount: pending ? 1 : 0,
      pendingReviewCount: pending ? 1 : 0,
      editedPendingReviewCount: 0,
      acceptedCount: pendingMaterialization ? 1 : 0,
      confirmedCount: 12 + (applied ? 1 : 0),
      pendingMaterializationCount: pendingMaterialization ? 1 : 0,
      materializedCount: 12 + (applied ? 1 : 0),
      materializationFailedCount: failed ? 1 : 0,
      rejectedCount: stage === "rejected" ? 1 : 0,
      deferredCount: stage === "deferred" ? 1 : 0,
      supersededCount: 1,
      rolledBackCount: rolledBack ? 1 : 0,
      expiredCount: 0,
      archivedCount: 2,
      byStatus: {},
      byMaterializationStatus: {},
    },
    laneSummaries: [
      lane("semantic_fact_preference", "事实与偏好", {
        totalCount: 9,
        activeCount: 8,
        candidateCount: pending ? 1 : 0,
        pendingReviewCount: pending ? 1 : 0,
        confirmedCount: 6,
        materializedCount: 6 + (applied ? 1 : 0),
        rolledBackCount: rolledBack ? 1 : 0,
        reviewItemRefs: pending
          ? [{ id: durableReviewItemId, kind: "review_item", label: "工作偏好建议" }]
          : [],
        evidenceRefs: [memoryEvidence, conversationEvidence],
      }),
      lane("procedural_rule", "做事方式", {
        totalCount: 6,
        activeCount: 5,
        confirmedCount: 5,
        materializedCount: 5,
        evidenceRefs: [memoryEvidence],
      }),
      lane("episodic_life_event", "经历与事件", {
        totalCount: 5,
        activeCount: 4,
        confirmedCount: 1,
        materializedCount: 1,
      }),
      lane("evidence_record", "依据记录", {
        totalCount: 4,
        activeCount: 1,
        archivedCount: 2,
        evidenceRefs: [conversationEvidence],
      }),
    ],
    recentMemoryRefs: [{ id: memoryEvidence.id, kind: "memory", label: "先结论后细节的写作反馈" }],
    reviewItemRefs: pending
      ? [{ id: durableReviewItemId, kind: "review_item", label: "工作偏好建议" }]
      : [],
    lifeModelLinkage: {
      linkedMemoryCount: 18,
      candidateMemoryCount: pending ? 1 : 0,
      materializedMemoryCount: applied ? 1 : 0,
      conflictCount: 0,
      boundaryMemoryCount: 0,
      linkageStatus: "partial",
      memoryRefs: [{ id: memoryEvidence.id, kind: "memory", label: "先结论后细节的写作反馈" }],
      evidenceRefs: [memoryEvidence],
    },
    items: [
      {
        memoryId: memoryEvidence.id,
        content: "输出建议时先给结论，再补充依据。",
        scope: "project",
        category: "preference",
        status: "materialized",
        materializationStatus: "materialized",
        recallState: rolledBack ? "historical" : "active",
        sensitivity: "internal",
        whyRemembered: "用户在 Review 中确认了这条工作偏好。",
        recallExplanation: "只有当前项目与任务相关时才会参与混合检索，并在每个回合重新排序。",
        acceptedAt: generatedAt,
        evidenceIds: [conversationEvidence.id],
        sourceRefs: [memoryEvidence, conversationEvidence],
        privacyErased: false,
        canCorrect: !rolledBack,
        canStopRecall: !rolledBack,
        canArchive: !rolledBack,
        canRestore: false,
        canRollback: !rolledBack,
        canPrivacyErase: true,
      },
    ],
    sourceRefs: [memoryEvidence, conversationEvidence],
    contractLimitations: [
      "MemoryViewModel 的单条动作能力来自后端字段；fixture 不证明真实原生确认。",
      "汇总数量不能证明某个建议已经应用。",
    ],
  };
}

function envelope<T>(
  data: T | null,
  status: ViewModelStatus,
  target: string,
  evidenceRefs: EvidenceRef[]
): ViewModelEnvelope<T> {
  const unavailable = status === "error";
  const stale = status === "stale";
  return {
    data: unavailable ? null : data,
    status,
    lastUpdatedAt: stale ? "2026-07-17T08:45:00.000Z" : generatedAt,
    source: "backend-readmodel",
    evidenceRefs: unavailable ? [] : evidenceRefs,
    warnings: unavailable
      ? [
          {
            code: `fixture.${target}.load_failed`,
            message: `${target} fixture is unavailable.`,
            severity: "error",
            evidenceRefs: [],
          },
        ]
      : stale
        ? [
            {
              code: `fixture.${target}.stale`,
              message: `${target} fixture is stale.`,
              severity: "warning",
              evidenceRefs,
            },
          ]
        : [],
    actions: {
      primary: [
        {
          id: `${target}.refresh`,
          label: `Refresh ${target}`,
          kind: "refresh",
          enabled: true,
          targetRef: target,
        },
      ],
      review: [],
      debugOnly: [],
    },
  };
}

export function initialDurableFixtureStage(id: WorkbenchFixtureId): DurableFixtureStage {
  if (id === "fixture-durable-approved") return "approved_not_applied";
  if (id === "fixture-durable-applying") return "applying";
  if (id === "fixture-durable-applied") return "applied";
  if (id === "fixture-durable-failed") return "failed";
  if (id === "fixture-durable-rolled-back") return "rolled_back";
  return "pending";
}

export function buildDurableFixtureSnapshot(
  id: WorkbenchFixtureId,
  stage: DurableFixtureStage,
  builderReviewItems: ReviewItem[] = []
): DurableTruthSnapshot {
  const status: ViewModelStatus =
    id === "fixture-error" ? "error" : id === "fixture-stale" ? "stale" : "ready";
  const item = durableReviewItem(stage);
  const empty = id === "fixture-empty";
  const reviewItems = [...(empty ? [] : [item]), ...builderReviewItems];
  const review: ReviewCenterViewModel = {
    batches: [
      ...(!empty
        ? [
            {
              id: "batch:lifemodel-focus-preference",
              domain: "life_model" as const,
              itemIds: [durableReviewItemId],
              actionRequiredCount: ["pending", "edited", "deferred"].includes(item.status) ? 1 : 0,
              highestRisk: item.risk,
            },
          ]
        : []),
      ...(builderReviewItems.length > 0
        ? [
            {
              id: "batch:lifemodel-builder",
              domain: "life_model" as const,
              itemIds: builderReviewItems.map(candidate => candidate.id),
              actionRequiredCount: builderReviewItems.filter(candidate =>
                ["pending", "edited", "deferred"].includes(candidate.status)
              ).length,
              highestRisk: builderReviewItems.some(candidate => candidate.risk === "high")
                ? ("high" as const)
                : builderReviewItems.some(candidate => candidate.risk === "medium")
                  ? ("medium" as const)
                  : ("low" as const),
            },
          ]
        : []),
    ],
    items: reviewItems,
    summary: {
      total: reviewItems.length,
      actionRequiredCount: reviewItems.filter(candidate =>
        ["pending", "edited", "deferred"].includes(candidate.status)
      ).length,
      blockedActionCount: reviewItems.filter(candidate =>
        candidate.allowedActions.some(action => !action.enabled)
      ).length,
      byStatus: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
        counts[candidate.status] = (counts[candidate.status] ?? 0) + 1;
        return counts;
      }, {}),
      byRisk: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
        counts[candidate.risk] = (counts[candidate.risk] ?? 0) + 1;
        return counts;
      }, {}),
      byMaterializationStatus: reviewItems.reduce<Record<string, number>>((counts, candidate) => {
        counts[candidate.materializationStatus] =
          (counts[candidate.materializationStatus] ?? 0) + 1;
        return counts;
      }, {}),
    },
  };
  return {
    lifeModelEnvelope: envelope(
      empty ? emptyLifeModel() : lifeModel(stage),
      status,
      "LifeModelViewModel",
      empty ? [] : [lifeModelEvidence, preferenceEvidence]
    ),
    memoryEnvelope: envelope(memory(stage), status, "MemoryViewModel", [memoryEvidence]),
    reviewEnvelope: envelope(
      review,
      status === "ready" && reviewItems.length === 0 ? "empty" : status,
      "ReviewCenterViewModel",
      [preferenceEvidence]
    ),
    diagnostics: [
      {
        id: "life_model_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
      {
        id: "memory_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
      {
        id: "review_center_view_model",
        status: status === "error" ? "failed" : "loaded",
        message: status === "error" ? "Static error fixture." : undefined,
      },
    ],
  };
}
