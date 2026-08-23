import {
  type LifeModelViewModel,
  type DraftLifeModelV2ChangeRequest,
  type DraftLifeModelV2ExportRequest,
  type DraftLifeModelV2RollbackRequest,
  type MemoryViewModel,
  type ProviderPrivacyBoundarySummary,
  type ReviewCenterViewModel,
  type ReviewItem,
  type ViewModelEnvelope,
} from "@/tauri";
import {
  archiveMemory,
  correctMemory,
  draftLifeModelV2Change,
  draftLifeModelV2Export,
  draftLifeModelV2Rollback,
  confirmLifeModelLearningCandidate,
  deleteLifeModelLearningCandidate,
  pauseLifeModelLearningSuggestionClass,
  rejectLifeModelLearningCandidate,
  stageLifeModelLearningCandidate,
  getLifeModelViewModel,
  getMemoryViewModel,
  privacyEraseMemoryAsset,
  restoreMemory,
  rollbackMemoryAsset,
} from "@/ipc/personalIntelligence";
import { getReviewCenterViewModel } from "@/ipc/review";
import { getProviderPrivacyBoundarySummary } from "@/ipc/settings";
import { productErrorCode as errorText } from "@/shared/productError";
import { buildReadModelErrorEnvelope } from "@/shared/readModelEnvelope";

export type PersonalIntelligenceDiagnostic = {
  id:
    | "life_model_view_model"
    | "memory_view_model"
    | "review_center_view_model"
    | "provider_privacy_boundary";
  status: "loaded" | "failed";
  message?: string;
};

export type PersonalIntelligenceSnapshot = {
  lifeModelEnvelope: ViewModelEnvelope<LifeModelViewModel>;
  memoryEnvelope: ViewModelEnvelope<MemoryViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: PersonalIntelligenceDiagnostic[];
};

export interface PersonalIntelligenceDataSource {
  loadPersonalIntelligence(): Promise<PersonalIntelligenceSnapshot>;
  correctMemory(memoryId: string, content: string): Promise<void>;
  archiveMemory(memoryId: string): Promise<void>;
  restoreMemory(memoryId: string): Promise<void>;
  rollbackMemory(memoryId: string, reason: string): Promise<void>;
  privacyEraseMemory(memoryId: string): Promise<void>;
  draftLifeModelChange(request: DraftLifeModelV2ChangeRequest): Promise<string>;
  draftLifeModelRollback(request: DraftLifeModelV2RollbackRequest): Promise<string>;
  draftLifeModelExport(request: DraftLifeModelV2ExportRequest): Promise<string>;
  confirmLifeModelLearningCandidate(candidateId: string): Promise<void>;
  deleteLifeModelLearningCandidate(candidateId: string): Promise<void>;
  rejectLifeModelLearningCandidate(candidateId: string): Promise<void>;
  pauseLifeModelLearningSuggestionClass(candidateId: string): Promise<void>;
  stageLifeModelLearningCandidate(candidateId: string): Promise<string>;
}

function requireLifeModelProposalReceipt(
  receipt: { proposalId: string; status: string; baseVersion: number | null },
  expectedBaseVersion: number | null
): string {
  if (
    !receipt.proposalId ||
    receipt.status !== "review_required" ||
    receipt.baseVersion !== expectedBaseVersion
  ) {
    throw new Error("lifemodel_v2_proposal_receipt_unverified");
  }
  return receipt.proposalId;
}

function requireAppliedMemoryProjection(
  receipt: { canonicalCommitted: boolean; projectionState: string },
  action: string
) {
  if (!receipt.canonicalCommitted) {
    throw new Error(`memory_${action}_canonical_commit_unverified`);
  }
  if (receipt.projectionState !== "applied") {
    throw new Error(`memory_${action}_projection_${receipt.projectionState || "unknown"}`);
  }
}

function settledEnvelope<T>(
  result: PromiseSettledResult<ViewModelEnvelope<T>>,
  targetRef: string,
  code: string
): ViewModelEnvelope<T> {
  return result.status === "fulfilled"
    ? result.value
    : buildReadModelErrorEnvelope<T>(
        targetRef,
        code,
        `${targetRef} could not be loaded: ${errorText(result.reason)}`
      );
}

const PERSONAL_INTELLIGENCE_REVIEW_TYPES = new Set([
  "life_model_update",
  "memory_write",
  "memory_archive",
]);

function countBy(items: ReviewItem[], key: (item: ReviewItem) => string): Record<string, number> {
  return items.reduce<Record<string, number>>((counts, item) => {
    const value = key(item);
    counts[value] = (counts[value] ?? 0) + 1;
    return counts;
  }, {});
}

export function personalIntelligenceReviewEnvelope(
  envelope: ViewModelEnvelope<ReviewCenterViewModel>
): ViewModelEnvelope<ReviewCenterViewModel> {
  if (!envelope.data) return envelope;
  const items = envelope.data.items.filter(item =>
    PERSONAL_INTELLIGENCE_REVIEW_TYPES.has(item.type)
  );
  const itemIds = new Set(items.map(item => item.id));
  const batches = envelope.data.batches
    .map(batch => {
      const scopedItemIds = batch.itemIds.filter(itemId => itemIds.has(itemId));
      return {
        ...batch,
        itemIds: scopedItemIds,
        actionRequiredCount: items.filter(
          item =>
            scopedItemIds.includes(item.id) &&
            ["pending", "edited", "deferred"].includes(item.status)
        ).length,
      };
    })
    .filter(batch => batch.itemIds.length > 0);
  const data: ReviewCenterViewModel = {
    batches,
    items,
    summary: {
      total: items.length,
      actionRequiredCount: items.filter(item =>
        ["pending", "edited", "deferred"].includes(item.status)
      ).length,
      blockedActionCount: items.filter(item => item.allowedActions.some(action => !action.enabled))
        .length,
      byStatus: countBy(items, item => item.status),
      byRisk: countBy(items, item => item.risk),
      byMaterializationStatus: countBy(items, item => item.materializationStatus),
    },
  };
  return { ...envelope, data, status: items.length === 0 ? "empty" : envelope.status };
}

export function buildPersonalIntelligenceErrorSnapshot(
  error: unknown
): PersonalIntelligenceSnapshot {
  const message = errorText(error);
  return {
    lifeModelEnvelope: buildReadModelErrorEnvelope(
      "LifeModelViewModel",
      "life_model_view_model.load_failed",
      `LifeModelViewModel could not be loaded: ${message}`
    ),
    memoryEnvelope: buildReadModelErrorEnvelope(
      "MemoryViewModel",
      "memory_view_model.load_failed",
      `MemoryViewModel could not be loaded: ${message}`
    ),
    reviewEnvelope: buildReadModelErrorEnvelope(
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed",
      `ReviewCenterViewModel could not be loaded: ${message}`
    ),
    boundaryEnvelope: buildReadModelErrorEnvelope(
      "ProviderPrivacyBoundarySummary",
      "provider_privacy_boundary.load_failed",
      `ProviderPrivacyBoundarySummary could not be loaded: ${message}`
    ),
    diagnostics: [
      { id: "life_model_view_model", status: "failed", message },
      { id: "memory_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
      { id: "provider_privacy_boundary", status: "failed", message },
    ],
  };
}

async function loadPersonalIntelligence(): Promise<PersonalIntelligenceSnapshot> {
  const [lifeModelResult, memoryResult, reviewResult, boundaryResult] = await Promise.allSettled([
    getLifeModelViewModel(),
    getMemoryViewModel(),
    getReviewCenterViewModel(),
    getProviderPrivacyBoundarySummary(),
  ]);

  return {
    lifeModelEnvelope: settledEnvelope(
      lifeModelResult,
      "LifeModelViewModel",
      "life_model_view_model.load_failed"
    ),
    memoryEnvelope: settledEnvelope(
      memoryResult,
      "MemoryViewModel",
      "memory_view_model.load_failed"
    ),
    reviewEnvelope: personalIntelligenceReviewEnvelope(
      settledEnvelope(reviewResult, "ReviewCenterViewModel", "review_center_view_model.load_failed")
    ),
    boundaryEnvelope: settledEnvelope(
      boundaryResult,
      "ProviderPrivacyBoundarySummary",
      "provider_privacy_boundary.load_failed"
    ),
    diagnostics: [
      lifeModelResult.status === "fulfilled"
        ? { id: "life_model_view_model", status: "loaded" }
        : {
            id: "life_model_view_model",
            status: "failed",
            message: errorText(lifeModelResult.reason),
          },
      memoryResult.status === "fulfilled"
        ? { id: "memory_view_model", status: "loaded" }
        : {
            id: "memory_view_model",
            status: "failed",
            message: errorText(memoryResult.reason),
          },
      reviewResult.status === "fulfilled"
        ? { id: "review_center_view_model", status: "loaded" }
        : {
            id: "review_center_view_model",
            status: "failed",
            message: errorText(reviewResult.reason),
          },
      boundaryResult.status === "fulfilled"
        ? { id: "provider_privacy_boundary", status: "loaded" }
        : {
            id: "provider_privacy_boundary",
            status: "failed",
            message: errorText(boundaryResult.reason),
          },
    ],
  };
}

export const tauriPersonalIntelligenceDataSource: PersonalIntelligenceDataSource = {
  loadPersonalIntelligence,
  async confirmLifeModelLearningCandidate(candidateId) {
    const receipt = await confirmLifeModelLearningCandidate(candidateId);
    if (
      receipt.candidateId !== candidateId ||
      receipt.status !== "reviewable" ||
      receipt.sourceKind !== "user_feedback" ||
      receipt.proposalCreated ||
      receipt.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_candidate_confirm_receipt_unverified");
    }
  },
  async stageLifeModelLearningCandidate(candidateId) {
    const receipt = await stageLifeModelLearningCandidate(candidateId);
    if (
      receipt.candidateId !== candidateId ||
      !receipt.proposalId ||
      receipt.status !== "review_required" ||
      !receipt.resultDocumentDigest ||
      receipt.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_stage_receipt_unverified");
    }
    return receipt.proposalId;
  },
  async deleteLifeModelLearningCandidate(candidateId) {
    const receipt = await deleteLifeModelLearningCandidate(candidateId);
    if (
      receipt.candidateId !== candidateId ||
      !receipt.deleted ||
      receipt.proposalDeleted ||
      receipt.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_candidate_delete_receipt_unverified");
    }
  },
  async rejectLifeModelLearningCandidate(candidateId) {
    const receipt = await rejectLifeModelLearningCandidate(candidateId);
    if (
      receipt.candidateId !== candidateId ||
      !receipt.changed ||
      receipt.status !== "rejected" ||
      receipt.suppressionKind !== "exact_candidate" ||
      !receipt.contentScrubbed ||
      receipt.proposalChanged ||
      receipt.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_candidate_reject_receipt_unverified");
    }
  },
  async pauseLifeModelLearningSuggestionClass(candidateId) {
    const receipt = await pauseLifeModelLearningSuggestionClass(candidateId);
    if (
      receipt.candidateId !== candidateId ||
      !receipt.changed ||
      receipt.status !== "rejected" ||
      receipt.suppressionKind !== "suggestion_class" ||
      !receipt.contentScrubbed ||
      receipt.proposalChanged ||
      receipt.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_suggestion_class_pause_receipt_unverified");
    }
  },
  async draftLifeModelChange(request) {
    const receipt = await draftLifeModelV2Change(request);
    return requireLifeModelProposalReceipt(receipt, request.baseVersion);
  },
  async draftLifeModelRollback(request) {
    const receipt = await draftLifeModelV2Rollback(request);
    return requireLifeModelProposalReceipt(receipt, request.baseVersion);
  },
  async draftLifeModelExport(request) {
    const receipt = await draftLifeModelV2Export(request);
    return requireLifeModelProposalReceipt(receipt, request.modelVersion);
  },
  async correctMemory(memoryId, content) {
    const receipt = await correctMemory(memoryId, content);
    if (receipt.replacedMemoryId !== memoryId || !receipt.memoryId || !receipt.undoAvailable) {
      throw new Error("memory_correct_receipt_unverified");
    }
    requireAppliedMemoryProjection(receipt, "correct");
  },
  async archiveMemory(memoryId) {
    const receipt = await archiveMemory(memoryId);
    if (receipt.owner.ownerKind !== "memory_lifecycle" || receipt.owner.ownerId !== memoryId) {
      throw new Error("memory_archive_owner_unverified");
    }
    requireAppliedMemoryProjection(receipt, "archive");
  },
  async restoreMemory(memoryId) {
    const receipt = await restoreMemory(memoryId);
    if (receipt.owner.ownerKind !== "memory_lifecycle" || receipt.owner.ownerId !== memoryId) {
      throw new Error("memory_restore_owner_unverified");
    }
    requireAppliedMemoryProjection(receipt, "restore");
  },
  async rollbackMemory(memoryId, reason) {
    const receipt = await rollbackMemoryAsset(memoryId, reason);
    requireAppliedMemoryProjection(receipt, "rollback");
  },
  async privacyEraseMemory(memoryId) {
    const receipt = await privacyEraseMemoryAsset(memoryId);
    requireAppliedMemoryProjection(receipt, "privacy_erase");
  },
};
