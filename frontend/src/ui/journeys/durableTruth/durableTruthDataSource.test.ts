import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriMocks = vi.hoisted(() => ({
  getLifeModelViewModel: vi.fn(),
  getMemoryViewModel: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
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
});
