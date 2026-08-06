import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriMocks = vi.hoisted(() => ({
  getLifeModelViewModel: vi.fn(),
  getMemoryViewModel: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
  draftMemoryCorrectionProposal: vi.fn(),
  draftMemoryArchiveProposal: vi.fn(),
  draftMemoryStopRecallProposal: vi.fn(),
  restoreArchivedMemory: vi.fn(),
  rollbackMemoryAsset: vi.fn(),
  privacyEraseMemoryAsset: vi.fn(),
}));

vi.mock("@/tauri", () => tauriMocks);

import { tauriDurableTruthDataSource } from "./durableTruthDataSource";

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

describe("durable truth Tauri data source", () => {
  beforeEach(() => vi.clearAllMocks());

  it("loads LifeModel, Memory, and Review Center as separate backend owners", async () => {
    tauriMocks.getLifeModelViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getMemoryViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getReviewCenterViewModel.mockResolvedValue(emptyEnvelope());

    const snapshot = await tauriDurableTruthDataSource.loadDurableTruth();

    expect(tauriMocks.getLifeModelViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getMemoryViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getReviewCenterViewModel).toHaveBeenCalledOnce();
    expect(snapshot.diagnostics.every(item => item.status === "loaded")).toBe(true);
  });

  it("preserves one failed owner as an error envelope instead of borrowing another truth", async () => {
    tauriMocks.getLifeModelViewModel.mockResolvedValue(emptyEnvelope());
    tauriMocks.getMemoryViewModel.mockRejectedValue(new Error("memory unavailable"));
    tauriMocks.getReviewCenterViewModel.mockResolvedValue(emptyEnvelope());

    const snapshot = await tauriDurableTruthDataSource.loadDurableTruth();

    expect(snapshot.lifeModelEnvelope.status).toBe("empty");
    expect(snapshot.memoryEnvelope.status).toBe("error");
    expect(snapshot.memoryEnvelope.data).toBeNull();
    expect(snapshot.diagnostics).toContainEqual({
      id: "memory_view_model",
      status: "failed",
      message: "memory unavailable",
    });
  });

  it("keeps correction and archive reviewed while restore, rollback, and erase use exact owners", async () => {
    tauriMocks.draftMemoryCorrectionProposal.mockResolvedValue({
      memoryId: "memory:one",
      action: "correct",
      status: "review_required",
    });
    tauriMocks.draftMemoryArchiveProposal.mockResolvedValue({
      memoryId: "memory:one",
      action: "archive",
      status: "review_required",
    });
    tauriMocks.draftMemoryStopRecallProposal.mockResolvedValue({
      memoryId: "memory:one",
      action: "stop_recall",
      status: "review_required",
    });
    tauriMocks.restoreArchivedMemory.mockResolvedValue({
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

    await tauriDurableTruthDataSource.correctMemory("memory:one", "corrected");
    await tauriDurableTruthDataSource.archiveMemory("memory:one");
    await tauriDurableTruthDataSource.stopRecall("memory:one");
    await tauriDurableTruthDataSource.restoreMemory("memory:one");
    await tauriDurableTruthDataSource.rollbackMemory("memory:one", "user correction");
    await tauriDurableTruthDataSource.privacyEraseMemory("memory:one");

    expect(tauriMocks.draftMemoryCorrectionProposal).toHaveBeenCalledWith(
      "memory:one",
      "corrected"
    );
    expect(tauriMocks.draftMemoryArchiveProposal).toHaveBeenCalledWith("memory:one");
    expect(tauriMocks.draftMemoryStopRecallProposal).toHaveBeenCalledWith("memory:one");
    expect(tauriMocks.restoreArchivedMemory).toHaveBeenCalledWith({
      ownerKind: "memory_lifecycle",
      ownerId: "memory:one",
    });
    expect(tauriMocks.rollbackMemoryAsset).toHaveBeenCalledWith("memory:one", "user correction");
    expect(tauriMocks.privacyEraseMemoryAsset).toHaveBeenCalledWith("memory:one");
  });

  it("does not report a direct Memory action as complete while its projection is pending", async () => {
    tauriMocks.privacyEraseMemoryAsset.mockResolvedValue({
      canonicalCommitted: true,
      projectionState: "pending",
    });

    await expect(tauriDurableTruthDataSource.privacyEraseMemory("memory:one")).rejects.toThrow(
      "memory_privacy_erase_projection_pending"
    );
  });
});
