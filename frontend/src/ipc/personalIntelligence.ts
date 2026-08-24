import type {
  ConfirmLifeModelLearningCandidateReceipt,
  DeleteLifeModelLearningCandidateReceipt,
  DraftLifeModelV2ChangeRequest,
  DraftLegacyLifeModelMigrationRequest,
  DraftLegacyLifeModelMigrationReceipt,
  DraftLifeModelV2ExportRequest,
  DraftLifeModelV2RollbackRequest,
  LifeModelLearningDecisionReceipt,
  LifeModelLearningReviewDecisionReceipt,
  LifeModelV2ProposalReceipt,
  LifeModelViewModel,
  MemoryCorrectionResult,
  MemoryPrivacyEraseReport,
  MemoryRetrievalMutationResult,
  MemoryRollbackReport,
  MemoryViewModel,
  StageLifeModelLearningCandidateReceipt,
  ViewModelEnvelope,
} from "../tauri";
import { safeInvoke } from "./invoke";

export async function getLifeModelViewModel(): Promise<ViewModelEnvelope<LifeModelViewModel>> {
  return safeInvoke<ViewModelEnvelope<LifeModelViewModel>>("get_life_model_view_model");
}

export async function draftLegacyLifeModelMigration(
  request: DraftLegacyLifeModelMigrationRequest
): Promise<DraftLegacyLifeModelMigrationReceipt> {
  return safeInvoke("draft_legacy_lifemodel_migration", { request });
}

export async function deleteLifeModelLearningCandidate(
  candidateId: string
): Promise<DeleteLifeModelLearningCandidateReceipt> {
  return safeInvoke("delete_lifemodel_learning_candidate", { candidateId });
}

export async function confirmLifeModelLearningCandidate(
  candidateId: string
): Promise<ConfirmLifeModelLearningCandidateReceipt> {
  return safeInvoke("confirm_lifemodel_learning_candidate", { candidateId });
}

export async function stageLifeModelLearningCandidate(
  candidateId: string
): Promise<StageLifeModelLearningCandidateReceipt> {
  return safeInvoke("stage_lifemodel_learning_candidate", { candidateId });
}

export async function editLifeModelLearningProposal(
  proposalId: string,
  statement: string
): Promise<{
  proposalId: string;
  status: "edited_pending_review";
  resultDocumentDigest: string;
  durableWriteExecuted: false;
  learning: LifeModelLearningReviewDecisionReceipt;
}> {
  return safeInvoke("edit_lifemodel_learning_proposal", {
    request: { proposalId, statement },
  });
}

export async function rejectLifeModelLearningCandidate(
  candidateId: string
): Promise<LifeModelLearningDecisionReceipt> {
  return safeInvoke("reject_lifemodel_learning_candidate", { candidateId });
}

export async function pauseLifeModelLearningSuggestionClass(
  candidateId: string
): Promise<LifeModelLearningDecisionReceipt> {
  return safeInvoke("pause_lifemodel_learning_suggestion_class", { candidateId });
}

export async function draftLifeModelV2Change(
  request: DraftLifeModelV2ChangeRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke("draft_lifemodel_v2_change", { request });
}

export async function draftLifeModelV2Rollback(
  request: DraftLifeModelV2RollbackRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke("draft_lifemodel_v2_rollback", { request });
}

export async function draftLifeModelV2Export(
  request: DraftLifeModelV2ExportRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke("draft_lifemodel_v2_export", { request });
}

export async function getMemoryViewModel(): Promise<ViewModelEnvelope<MemoryViewModel>> {
  return safeInvoke<ViewModelEnvelope<MemoryViewModel>>("get_memory_view_model");
}

export async function rollbackMemoryAsset(
  memoryId: string,
  reason: string
): Promise<MemoryRollbackReport> {
  return safeInvoke("rollback_memory_asset", { memoryId, reason });
}

export async function correctMemory(
  memoryId: string,
  content: string
): Promise<MemoryCorrectionResult> {
  return safeInvoke("correct_memory", { memoryId, content });
}

export async function archiveMemory(memoryId: string): Promise<MemoryRetrievalMutationResult> {
  return safeInvoke("archive_memory", { memoryId });
}

export async function restoreMemory(memoryId: string): Promise<MemoryRetrievalMutationResult> {
  return safeInvoke("restore_memory", { memoryId });
}

export async function privacyEraseMemoryAsset(memoryId: string): Promise<MemoryPrivacyEraseReport> {
  return safeInvoke("privacy_erase_memory_asset", { memoryId });
}
