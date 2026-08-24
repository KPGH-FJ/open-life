import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriMocks = vi.hoisted(() => ({
  getLifeModelViewModel: vi.fn(),
  getMemoryViewModel: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
  getProviderPrivacyBoundarySummary: vi.fn(),
  draftLifeModelV2Change: vi.fn(),
  draftLegacyLifeModelMigration: vi.fn(),
  draftLifeModelV2Rollback: vi.fn(),
  draftLifeModelV2Export: vi.fn(),
  confirmLifeModelLearningCandidate: vi.fn(),
  stageLifeModelLearningCandidate: vi.fn(),
  deleteLifeModelLearningCandidate: vi.fn(),
  rejectLifeModelLearningCandidate: vi.fn(),
  pauseLifeModelLearningSuggestionClass: vi.fn(),
  correctMemory: vi.fn(),
  archiveMemory: vi.fn(),
  restoreMemory: vi.fn(),
  rollbackMemoryAsset: vi.fn(),
  privacyEraseMemoryAsset: vi.fn(),
}));

vi.mock("@/tauri", () => ({}));
vi.mock("@/ipc/personalIntelligence", () => ({
  archiveMemory: tauriMocks.archiveMemory,
  confirmLifeModelLearningCandidate: tauriMocks.confirmLifeModelLearningCandidate,
  correctMemory: tauriMocks.correctMemory,
  deleteLifeModelLearningCandidate: tauriMocks.deleteLifeModelLearningCandidate,
  draftLifeModelV2Change: tauriMocks.draftLifeModelV2Change,
  draftLegacyLifeModelMigration: tauriMocks.draftLegacyLifeModelMigration,
  draftLifeModelV2Export: tauriMocks.draftLifeModelV2Export,
  draftLifeModelV2Rollback: tauriMocks.draftLifeModelV2Rollback,
  getLifeModelViewModel: tauriMocks.getLifeModelViewModel,
  getMemoryViewModel: tauriMocks.getMemoryViewModel,
  pauseLifeModelLearningSuggestionClass: tauriMocks.pauseLifeModelLearningSuggestionClass,
  privacyEraseMemoryAsset: tauriMocks.privacyEraseMemoryAsset,
  rejectLifeModelLearningCandidate: tauriMocks.rejectLifeModelLearningCandidate,
  restoreMemory: tauriMocks.restoreMemory,
  rollbackMemoryAsset: tauriMocks.rollbackMemoryAsset,
  stageLifeModelLearningCandidate: tauriMocks.stageLifeModelLearningCandidate,
}));
vi.mock("@/ipc/review", () => ({
  getReviewCenterViewModel: tauriMocks.getReviewCenterViewModel,
}));
vi.mock("@/ipc/settings", () => ({
  getProviderPrivacyBoundarySummary: tauriMocks.getProviderPrivacyBoundarySummary,
}));

import {
  personalIntelligenceReviewEnvelope,
  tauriPersonalIntelligenceDataSource,
} from "./personalIntelligenceDataSource";

function emptyEnvelope() {
  return {
    data: null,
    status: "empty" as const,
    lastUpdatedAt: "2026-07-21T00:00:00Z",
    source: "backend-readmodel" as const,
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

describe("Personal Intelligence Tauri data source", () => {
  beforeEach(() => vi.clearAllMocks());

  it("keeps only Life Model and Agent Memory review items on this route", () => {
    const lifeModel = {
      id: "review:life-model",
      type: "life_model_update",
      status: "pending",
      materializationStatus: "not_started",
      risk: "medium",
      allowedActions: [],
    };
    const external = {
      id: "review:file",
      type: "external_write_action",
      status: "pending",
      materializationStatus: "not_started",
      risk: "medium",
      allowedActions: [],
    };
    const result = personalIntelligenceReviewEnvelope({
      ...emptyEnvelope(),
      status: "ready",
      data: {
        items: [lifeModel, external],
        batches: [
          {
            id: "batch:life-model",
            domain: "life_model",
            itemIds: [lifeModel.id],
            actionRequiredCount: 1,
            highestRisk: "medium",
          },
          {
            id: "batch:file",
            domain: "external_action",
            itemIds: [external.id],
            actionRequiredCount: 1,
            highestRisk: "medium",
          },
        ],
        summary: {
          total: 2,
          actionRequiredCount: 2,
          blockedActionCount: 0,
          byStatus: { pending: 2 },
          byRisk: { medium: 2 },
          byMaterializationStatus: { not_started: 2 },
        },
      },
    } as never);

    expect(result.data?.items.map(item => item.id)).toEqual([lifeModel.id]);
    expect(result.data?.batches.map(batch => batch.id)).toEqual(["batch:life-model"]);
    expect(result.data?.summary).toMatchObject({ total: 1, actionRequiredCount: 1 });
  });

  it("loads LifeModel, Memory, and Review Center as separate backend owners", async () => {
    tauriMocks.getLifeModelViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getMemoryViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getReviewCenterViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue(emptyEnvelope());

    const snapshot = await tauriPersonalIntelligenceDataSource.loadPersonalIntelligence();

    expect(tauriMocks.getLifeModelViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getMemoryViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getReviewCenterViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getProviderPrivacyBoundarySummary).toHaveBeenCalledOnce();
    expect(snapshot.diagnostics.every(item => item.status === "loaded")).toBe(true);
  });

  it("preserves one failed owner as an error envelope instead of borrowing another truth", async () => {
    tauriMocks.getLifeModelViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getMemoryViewModel.mockRejectedValue(new Error("memory unavailable"));
    tauriMocks.getReviewCenterViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue(emptyEnvelope());

    const snapshot = await tauriPersonalIntelligenceDataSource.loadPersonalIntelligence();

    expect(snapshot.lifeModelEnvelope.status).toBe("empty");
    expect(snapshot.memoryEnvelope.status).toBe("error");
    expect(snapshot.memoryEnvelope.data).toBeNull();
    expect(snapshot.diagnostics).toContainEqual({
      id: "memory_view_model",
      status: "failed",
      message: "memory unavailable",
    });
  });

  it("accepts candidate deletion only when the receipt proves Proposal and LifeModel stayed unchanged", async () => {
    tauriMocks.deleteLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      deleted: true,
      proposalDeleted: false,
      canonicalLifeModelChanged: false,
    });

    await tauriPersonalIntelligenceDataSource.deleteLifeModelLearningCandidate("candidate:one");

    expect(tauriMocks.deleteLifeModelLearningCandidate).toHaveBeenCalledWith("candidate:one");

    tauriMocks.deleteLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      deleted: true,
      proposalDeleted: false,
      canonicalLifeModelChanged: true,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.deleteLifeModelLearningCandidate("candidate:one")
    ).rejects.toThrow("lifemodel_learning_candidate_delete_receipt_unverified");
  });

  it("accepts explicit candidate feedback without Proposal or LifeModel credit", async () => {
    tauriMocks.confirmLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      status: "reviewable",
      sourceKind: "user_feedback",
      proposalCreated: false,
      canonicalLifeModelChanged: false,
    });

    await tauriPersonalIntelligenceDataSource.confirmLifeModelLearningCandidate("candidate:one");

    expect(tauriMocks.confirmLifeModelLearningCandidate).toHaveBeenCalledWith("candidate:one");

    tauriMocks.confirmLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      status: "reviewable",
      sourceKind: "user_feedback",
      proposalCreated: true,
      canonicalLifeModelChanged: false,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.confirmLifeModelLearningCandidate("candidate:one")
    ).rejects.toThrow("lifemodel_learning_candidate_confirm_receipt_unverified");
  });

  it("credits candidate staging only when Review exists and canonical LifeModel stayed unchanged", async () => {
    tauriMocks.stageLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      proposalId: "proposal:one",
      status: "review_required",
      baseVersion: 2,
      baseDocumentDigest: "sha256:base",
      resultDocumentDigest: "sha256:result",
      canonicalLifeModelChanged: false,
    });

    await expect(
      tauriPersonalIntelligenceDataSource.stageLifeModelLearningCandidate("candidate:one")
    ).resolves.toBe("proposal:one");

    tauriMocks.stageLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      proposalId: "proposal:two",
      status: "review_required",
      resultDocumentDigest: "sha256:result",
      canonicalLifeModelChanged: true,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.stageLifeModelLearningCandidate("candidate:one")
    ).rejects.toThrow("lifemodel_learning_stage_receipt_unverified");
  });

  it("accepts candidate suppression only with scrubbed content and no Proposal or LifeModel change", async () => {
    tauriMocks.rejectLifeModelLearningCandidate.mockResolvedValueOnce({
      candidateId: "candidate:one",
      changed: true,
      status: "rejected",
      suppressionKind: "exact_candidate",
      contentScrubbed: true,
      proposalChanged: false,
      canonicalLifeModelChanged: false,
    });
    tauriMocks.pauseLifeModelLearningSuggestionClass.mockResolvedValueOnce({
      candidateId: "candidate:two",
      changed: true,
      status: "rejected",
      suppressionKind: "suggestion_class",
      contentScrubbed: true,
      proposalChanged: false,
      canonicalLifeModelChanged: false,
    });

    await tauriPersonalIntelligenceDataSource.rejectLifeModelLearningCandidate("candidate:one");
    await tauriPersonalIntelligenceDataSource.pauseLifeModelLearningSuggestionClass(
      "candidate:two"
    );

    expect(tauriMocks.rejectLifeModelLearningCandidate).toHaveBeenCalledWith("candidate:one");
    expect(tauriMocks.pauseLifeModelLearningSuggestionClass).toHaveBeenCalledWith("candidate:two");
  });

  it("uses direct verified receipts for reversible Memory controls", async () => {
    tauriMocks.correctMemory.mockResolvedValue({
      memoryId: "memory:replacement",
      replacedMemoryId: "memory:one",
      canonicalCommitted: true,
      projectionState: "applied",
      undoAvailable: true,
    });
    tauriMocks.archiveMemory.mockResolvedValue({
      owner: { ownerKind: "memory_lifecycle", ownerId: "memory:one" },
      disposition: "archived",
      changed: true,
      canonicalCommitted: true,
      projectionState: "applied",
    });
    tauriMocks.restoreMemory.mockResolvedValue({
      owner: { ownerKind: "memory_lifecycle", ownerId: "memory:one" },
      disposition: "active",
      changed: true,
      canonicalCommitted: true,
      projectionState: "applied",
    });
    tauriMocks.rollbackMemoryAsset.mockResolvedValue({
      canonicalCommitted: true,
      projectionState: "applied",
    });
    tauriMocks.privacyEraseMemoryAsset.mockResolvedValue({
      canonicalCommitted: true,
      projectionState: "applied",
    });

    await tauriPersonalIntelligenceDataSource.correctMemory("memory:one", "corrected");
    await tauriPersonalIntelligenceDataSource.archiveMemory("memory:one");
    await tauriPersonalIntelligenceDataSource.restoreMemory("memory:one");
    await tauriPersonalIntelligenceDataSource.rollbackMemory("memory:one", "user correction");
    await tauriPersonalIntelligenceDataSource.privacyEraseMemory("memory:one");

    expect(tauriMocks.correctMemory).toHaveBeenCalledWith("memory:one", "corrected");
    expect(tauriMocks.archiveMemory).toHaveBeenCalledWith("memory:one");
    expect(tauriMocks.restoreMemory).toHaveBeenCalledWith("memory:one");
    expect(tauriMocks.rollbackMemoryAsset).toHaveBeenCalledWith("memory:one", "user correction");
    expect(tauriMocks.privacyEraseMemoryAsset).toHaveBeenCalledWith("memory:one");
  });

  it("rejects a direct Memory control when its canonical projection is not applied", async () => {
    tauriMocks.correctMemory.mockResolvedValue({
      memoryId: "memory:replacement",
      replacedMemoryId: "memory:one",
      canonicalCommitted: true,
      projectionState: "pending",
      undoAvailable: true,
    });

    await expect(
      tauriPersonalIntelligenceDataSource.correctMemory("memory:one", "corrected")
    ).rejects.toThrow("memory_correct_projection_pending");
  });

  it("accepts only exact Review-required LifeModel v2 operation receipts", async () => {
    const change = {
      baseVersion: 4,
      baseDocumentDigest: "sha256:base",
      change: { operation: "clear" as const },
    };
    tauriMocks.draftLifeModelV2Change.mockResolvedValue({
      proposalId: "proposal:change",
      status: "review_required",
      baseVersion: 4,
    });
    await expect(tauriPersonalIntelligenceDataSource.draftLifeModelChange(change)).resolves.toBe(
      "proposal:change"
    );

    const rollback = {
      baseVersion: 4,
      baseDocumentDigest: "sha256:base",
      targetVersion: 2,
      targetDocumentDigest: "sha256:target",
    };
    tauriMocks.draftLifeModelV2Rollback.mockResolvedValue({
      proposalId: "proposal:rollback",
      status: "review_required",
      baseVersion: 4,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.draftLifeModelRollback(rollback)
    ).resolves.toBe("proposal:rollback");

    const exportRequest = {
      modelVersion: 4,
      documentDigest: "sha256:base",
      projectionDigest: null,
      format: "json" as const,
      targetPath: "/safe/lifemodel.json",
    };
    tauriMocks.draftLifeModelV2Export.mockResolvedValue({
      proposalId: "proposal:export",
      status: "review_required",
      baseVersion: 3,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.draftLifeModelExport(exportRequest)
    ).rejects.toThrow("lifemodel_v2_proposal_receipt_unverified");
  });

  it("accepts a legacy migration draft only when its source and decision receipt are exact", async () => {
    const request = {
      sourceDigest: "sha256:legacy",
      selections: [
        {
          candidateId: "legacy-candidate:one",
          decision: "include" as const,
          editedValue: { kind: "statement" as const, value: { statement: "Alice" } },
        },
        {
          candidateId: "legacy-candidate:two",
          decision: "exclude" as const,
          editedValue: null,
        },
      ],
      nonLifemodelItemsAcknowledged: true,
    };
    tauriMocks.draftLegacyLifeModelMigration.mockResolvedValueOnce({
      proposalId: "proposal:migration",
      status: "review_required",
      sourceDigest: "sha256:legacy",
      resultDocumentDigest:
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
      includedCandidateCount: 1,
      excludedCandidateCount: 1,
      durableWriteExecuted: false,
    });

    await expect(
      tauriPersonalIntelligenceDataSource.draftLegacyLifeModelMigration(request)
    ).resolves.toBe("proposal:migration");

    tauriMocks.draftLegacyLifeModelMigration.mockResolvedValueOnce({
      proposalId: "proposal:migration",
      status: "review_required",
      sourceDigest: "sha256:legacy",
      resultDocumentDigest: "missing-digest-scheme",
      includedCandidateCount: 2,
      excludedCandidateCount: 0,
      durableWriteExecuted: false,
    });
    await expect(
      tauriPersonalIntelligenceDataSource.draftLegacyLifeModelMigration(request)
    ).rejects.toThrow("lifemodel_v2_migration_proposal_receipt_unverified");
  });

  it("does not report a direct Memory action as complete while its projection is pending", async () => {
    tauriMocks.privacyEraseMemoryAsset.mockResolvedValue({
      canonicalCommitted: true,
      projectionState: "pending",
    });

    await expect(
      tauriPersonalIntelligenceDataSource.privacyEraseMemory("memory:one")
    ).rejects.toThrow("memory_privacy_erase_projection_pending");
  });
});
