import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProviderPrivacyBoundarySummary, TasksViewModel, ViewModelEnvelope } from "@/tauri";
import { makeDailyGoal, makeLifeStateProjection } from "@/viewmodels/today/todayViewModel.fixtures";
import { boundaryPresentation } from "./readOnlySpinePresentation";

const tauriMocks = vi.hoisted(() => ({
  getLifeStateProjection: vi.fn(),
  getDailyGoals: vi.fn(),
  getProviderPrivacyBoundarySummary: vi.fn(),
  getTasksViewModel: vi.fn(),
}));

vi.mock("@/tauri", () => tauriMocks);

import { tauriReadOnlySpineDataSource } from "./readOnlySpineDataSource";

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

function boundaryEnvelope(
  data: ProviderPrivacyBoundarySummary
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data,
    status: "ready",
    lastUpdatedAt: "2026-07-18T08:30:00.000Z",
    source: "backend-readmodel",
    evidenceRefs: data.evidenceRefs,
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function tasksEnvelope(): ViewModelEnvelope<TasksViewModel> {
  return {
    data: {
      items: [],
      summary: {
        total: 0,
        needsAttentionCount: 0,
        activeCount: 0,
        waitingReviewCount: 0,
        waitingPermissionCount: 0,
        blockedCount: 0,
        pendingReviewCount: 0,
        completedCount: 0,
        completedNeedsEvidenceCount: 0,
        failedCount: 0,
        cancelledCount: 0,
        byLifecycleStatus: {},
      },
      sourceRefs: [],
      contractLimitations: [],
    },
    status: "empty",
    lastUpdatedAt: "2026-07-18T08:30:00.000Z",
    source: "backend-readmodel",
    evidenceRefs: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

describe("Workbench Tauri read-only data source", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.getLifeStateProjection.mockResolvedValue(makeLifeStateProjection());
    tauriMocks.getDailyGoals.mockResolvedValue([makeDailyGoal()]);
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue(
      boundaryEnvelope(unknownBoundary)
    );
    tauriMocks.getTasksViewModel.mockResolvedValue(tasksEnvelope());
  });

  it("fails Today closed when LifeStateProjection cannot be loaded", async () => {
    tauriMocks.getLifeStateProjection.mockRejectedValue(new Error("projection unavailable"));

    const snapshot = await tauriReadOnlySpineDataSource.loadToday();

    expect(snapshot.envelope.status).toBe("error");
    expect(snapshot.envelope.data).toBeNull();
    expect(snapshot.diagnostics).toContainEqual(
      expect.objectContaining({ id: "life_state_projection", status: "failed" })
    );
  });

  it("marks Today stale when a required compatibility source fails", async () => {
    tauriMocks.getDailyGoals.mockRejectedValue(new Error("daily goals unavailable"));

    const snapshot = await tauriReadOnlySpineDataSource.loadToday();

    expect(snapshot.envelope.status).toBe("stale");
    expect(snapshot.envelope.warnings).toEqual(
      expect.arrayContaining([expect.objectContaining({ code: "today.daily_goals_load_failed" })])
    );
  });

  it("does not turn an unknown ready boundary envelope into verified local", async () => {
    const snapshot = await tauriReadOnlySpineDataSource.loadToday();

    expect(snapshot.boundaryEnvelope.status).toBe("ready");
    expect(boundaryPresentation(snapshot.boundaryEnvelope).status).toBe("unknown");
  });

  it("returns an error envelope instead of deriving Tasks from another source", async () => {
    tauriMocks.getTasksViewModel.mockRejectedValue(new Error("tasks unavailable"));

    const snapshot = await tauriReadOnlySpineDataSource.loadTasks();

    expect(snapshot.envelope.status).toBe("error");
    expect(snapshot.envelope.data).toBeNull();
    expect(snapshot.diagnostics).toContainEqual(
      expect.objectContaining({ id: "tasks_view_model", status: "failed" })
    );
  });
});
