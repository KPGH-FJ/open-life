import {
  draftMemoryArchiveProposal,
  draftMemoryCorrectionProposal,
  draftMemoryStopRecallProposal,
  draftLegacyLifeModelMigration,
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
  getReviewCenterViewModel,
  privacyEraseMemoryAsset,
  restoreArchivedMemory,
  rollbackMemoryAsset,
  type LifeModelViewModel,
  type DraftLegacyLifeModelMigrationRequest,
  type DraftLifeModelV2ChangeRequest,
  type DraftLifeModelV2ExportRequest,
  type DraftLifeModelV2RollbackRequest,
  type MemoryViewModel,
  type ReviewCenterViewModel,
  type ViewModelEnvelope,
} from "@/tauri";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import { buildReadModelErrorEnvelope } from "@/ui/journeys/readOnly/readOnlySpineDataSource";

export type DurableTruthDiagnostic = {
  id: "life_model_view_model" | "memory_view_model" | "review_center_view_model";
  status: "loaded" | "failed";
  message?: string;
};

export type DurableTruthSnapshot = {
  lifeModelEnvelope: ViewModelEnvelope<LifeModelViewModel>;
  memoryEnvelope: ViewModelEnvelope<MemoryViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  diagnostics: DurableTruthDiagnostic[];
};

export interface DurableTruthDataSource {
  loadDurableTruth(): Promise<DurableTruthSnapshot>;
  correctMemory(memoryId: string, content: string): Promise<void>;
  archiveMemory(memoryId: string): Promise<void>;
  stopRecall(memoryId: string): Promise<void>;
  restoreMemory(memoryId: string): Promise<void>;
  rollbackMemory(memoryId: string, reason: string): Promise<void>;
  privacyEraseMemory(memoryId: string): Promise<void>;
  draftLegacyLifeModelMigration(request: DraftLegacyLifeModelMigrationRequest): Promise<string>;
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

function requireReviewedMemoryProposal(
  receipt: { memoryId: string; action: string; status: string },
  memoryId: string,
  action: string
) {
  if (
    receipt.memoryId !== memoryId ||
    receipt.action !== action ||
    receipt.status !== "review_required"
  ) {
    throw new Error(`memory_${action}_proposal_receipt_unverified`);
  }
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

export function buildDurableTruthErrorSnapshot(error: unknown): DurableTruthSnapshot {
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
    diagnostics: [
      { id: "life_model_view_model", status: "failed", message },
      { id: "memory_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
    ],
  };
}

async function loadDurableTruth(): Promise<DurableTruthSnapshot> {
  const [lifeModelResult, memoryResult, reviewResult] = await Promise.allSettled([
    getLifeModelViewModel(),
    getMemoryViewModel(),
    getReviewCenterViewModel(),
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
    reviewEnvelope: settledEnvelope(
      reviewResult,
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed"
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
    ],
  };
}

export const tauriDurableTruthDataSource: DurableTruthDataSource = {
  loadDurableTruth,
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
  async draftLegacyLifeModelMigration(request) {
    const receipt = await draftLegacyLifeModelMigration(request);
    if (
      receipt.status !== "review_required" ||
      receipt.sourceDigest !== request.sourceDigest ||
      receipt.includedCount + receipt.excludedCount !== request.selections.length
    ) {
      throw new Error("lifemodel_migration_proposal_receipt_unverified");
    }
    return receipt.proposalId;
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
    const receipt = await draftMemoryCorrectionProposal(memoryId, content);
    requireReviewedMemoryProposal(receipt, memoryId, "correct");
  },
  async archiveMemory(memoryId) {
    const receipt = await draftMemoryArchiveProposal(memoryId);
    requireReviewedMemoryProposal(receipt, memoryId, "archive");
  },
  async stopRecall(memoryId) {
    const receipt = await draftMemoryStopRecallProposal(memoryId);
    requireReviewedMemoryProposal(receipt, memoryId, "stop_recall");
  },
  async restoreMemory(memoryId) {
    const receipt = await restoreArchivedMemory({
      ownerKind: "memory_lifecycle",
      ownerId: memoryId,
    });
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
