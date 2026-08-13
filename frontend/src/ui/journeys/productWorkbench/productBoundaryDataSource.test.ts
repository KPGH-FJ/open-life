import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProviderPrivacyBoundarySummary, ViewModelEnvelope } from "@/tauri";
import { boundaryPresentation } from "./workbenchPresentation";

const tauriMocks = vi.hoisted(() => ({
  getProviderPrivacyBoundarySummary: vi.fn(),
}));

vi.mock("@/tauri", () => tauriMocks);

import { tauriProductBoundaryDataSource } from "./productBoundaryDataSource";

const unknownBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "unknown",
  externalTransmission: "unknown",
  providerLabel: "unknown",
  modelLabel: "unknown",
  privacyLabel: "unknown",
  risk: "unknown",
  localOnlyRequired: false,
  blockedReason: "No current route evidence.",
  evidenceRefs: [],
};

function boundaryEnvelope(): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data: unknownBoundary,
    status: "ready",
    lastUpdatedAt: "2026-07-18T08:30:00.000Z",
    source: "backend-readmodel",
    evidenceRefs: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

describe("Workbench boundary data source", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue(boundaryEnvelope());
  });

  it("loads the independent provider/privacy boundary without Today or Tasks", async () => {
    const envelope = await tauriProductBoundaryDataSource.loadBoundary();

    expect(envelope.status).toBe("ready");
    expect(boundaryPresentation(envelope).status).toBe("unknown");
    expect(tauriMocks.getProviderPrivacyBoundarySummary).toHaveBeenCalledTimes(1);
  });

  it("returns a fail-closed envelope when the boundary read fails", async () => {
    tauriMocks.getProviderPrivacyBoundarySummary.mockRejectedValue(new Error("unavailable"));

    const envelope = await tauriProductBoundaryDataSource.loadBoundary();

    expect(envelope.status).toBe("error");
    expect(envelope.data).toBeNull();
    expect(boundaryPresentation(envelope).status).toBe("error");
  });
});
