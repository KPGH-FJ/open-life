import { describe, expect, it } from "vitest";
import {
  buildTodayViewModelEnvelope,
  TODAY_VIEW_MODEL_AUTHORITY_CONTRACT,
} from "./todayViewModelAdapter";
import {
  emptyTodayViewModelInput,
  errorTodayViewModelInput,
  makeDailyGoal,
  makeLifeStateProjection,
  readyTodayViewModelInput,
  safeModeTodayViewModelInput,
  staleTodayViewModelInput,
} from "./todayViewModel.fixtures";

describe("todayViewModelAdapter", () => {
  it("freezes the adapter as composition-only over named backend owners", () => {
    expect(TODAY_VIEW_MODEL_AUTHORITY_CONTRACT).toEqual({
      version: "openlife.today-adapter.v1",
      compositionOwner: "strict_frontend_adapter",
      inputs: {
        readinessAndSafeMode: "LifeStateProjection",
        taskPressureAndPendingReview: "LifeStateProjection",
        dailyGoals: "get_daily_goals compatibility projection",
        providerPrivacyBoundary: "ProviderPrivacyBoundarySummary",
      },
      forbiddenLocalTruth: [
        "proposal status",
        "task lifecycle",
        "provider route",
        "external transmission",
        "durable completion",
      ],
    });
  });

  it("builds a ready envelope from LifeStateProjection and daily goals", () => {
    const envelope = buildTodayViewModelEnvelope(readyTodayViewModelInput);

    expect(envelope.status).toBe("ready");
    expect(envelope.source).toBe("backend-readmodel");
    expect(envelope.lastUpdatedAt).toBe("2026-07-09T00:00:00.000Z");
    expect(envelope.data?.primaryDailyGoal).toMatchObject({
      title: "Draft the weekly planning note",
      status: "not_started",
      priority: "unknown",
      backendClassification: "unknown",
    });
    expect(envelope.data?.dailyStateSummary.readiness).toBe("ready");
    expect(envelope.data?.nextRecommendedAction).toBeNull();
  });

  it("represents empty state when projection has no daily goal or current task", () => {
    const envelope = buildTodayViewModelEnvelope(emptyTodayViewModelInput);

    expect(envelope.status).toBe("empty");
    expect(envelope.data?.primaryDailyGoal).toBeNull();
    expect(envelope.data?.currentTaskPressure).toMatchObject({
      activeCount: 0,
      waitingPermissionCount: 0,
      blockedCount: 0,
      highestRisk: "none",
    });
    expect(envelope.data?.dailyStateSummary.readiness).toBe("empty");
  });

  it("preserves Safe Mode as projection-owned state", () => {
    const envelope = buildTodayViewModelEnvelope(safeModeTodayViewModelInput);

    expect(envelope.status).toBe("ready");
    expect(envelope.data?.safeMode).toMatchObject({
      active: true,
      reason: "LifeModel storage is unavailable; Safe Mode is active.",
      blocksExternalActions: true,
      blocksDurableWrites: true,
    });
    expect(envelope.data?.dailyStateSummary.readiness).toBe("safe_mode");
    expect(envelope.data?.blockers.map(blocker => blocker.category)).toContain("safe_mode");
  });

  it("uses pending review count from projection.pending", () => {
    const projection = makeLifeStateProjection({
      pending: {
        pendingProposalCount: 7,
        editedProposalCount: 1,
        totalReviewRequiredCount: 7,
        highRiskReviewRequiredCount: 2,
        proposalStoreStatus: "ok",
        requiresUserAction: true,
      },
      surfaces: [
        {
          surface: "today",
          pendingReviewCount: 99,
          editedReviewCount: 99,
          totalReviewRequiredCount: 99,
          readinessStatus: "ready",
          taskStatus: "idle",
          safeModeActive: false,
          waitingPermissionCount: 0,
          activeToolPermissionCount: 0,
        },
      ],
    });

    const envelope = buildTodayViewModelEnvelope({
      projection,
      dailyGoals: [makeDailyGoal()],
    });

    expect(envelope.data?.pendingReviewCount).toBe(7);
  });

  it("marks stale state and disables risky actions until refresh", () => {
    const envelope = buildTodayViewModelEnvelope(staleTodayViewModelInput);

    expect(envelope.status).toBe("stale");
    expect(envelope.lastUpdatedAt).toBe("2026-07-08T23:00:00.000Z");
    expect(envelope.data?.workspaceLink.enabled).toBe(false);
    expect(envelope.data?.workspaceLink.disabledReason).toMatch(/Refresh Today state/);
    expect(envelope.actions.primary.find(action => action.id === "today.refresh")?.enabled).toBe(
      true
    );
    expect(envelope.warnings?.map(warning => warning.code)).toContain("today.stale");
  });

  it("keeps error state null instead of falling back to daily-goal input", () => {
    const envelope = buildTodayViewModelEnvelope(errorTodayViewModelInput);

    expect(envelope.status).toBe("error");
    expect(envelope.data).toBeNull();
    expect(envelope.actions.primary).toEqual([
      {
        id: "today.refresh",
        label: "Refresh Today state",
        kind: "refresh",
        enabled: true,
        targetRef: "today",
      },
    ]);
    expect(envelope.warnings?.map(warning => warning.code)).toContain(
      "today.projection_unavailable"
    );
  });

  it("keeps daily goal classification unknown and documents the limitation", () => {
    const envelope = buildTodayViewModelEnvelope({
      projection: makeLifeStateProjection(),
      dailyGoals: [makeDailyGoal({ name: "Review calendar", done: false })],
    });

    expect(envelope.data?.primaryDailyGoal?.backendClassification).toBe("unknown");
    expect(envelope.warnings?.map(warning => warning.code)).toContain(
      "today.goal_classification_limited"
    );
  });

  it("keeps provider/privacy unknown when its backend owner is missing", () => {
    const envelope = buildTodayViewModelEnvelope({
      projection: makeLifeStateProjection(),
      dailyGoals: [makeDailyGoal()],
      providerPrivacyBoundary: null,
    });

    expect(envelope.data?.dailyStateSummary.providerPrivacyBoundary).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      risk: "unknown",
      blockedReason: "Provider/privacy boundary is not backend-owned by the Today limited slice.",
    });
  });

  it("keeps debug-only actions out of primary actions", () => {
    const envelope = buildTodayViewModelEnvelope(readyTodayViewModelInput);
    const primaryIds = new Set(envelope.actions.primary.map(action => action.id));

    expect(envelope.actions.debugOnly).toEqual([
      {
        id: "today.inspect_projection_source_refs",
        label: "Inspect projection source refs",
        kind: "raw_json",
        enabled: true,
        developerOnly: true,
        targetRef: "LifeStateProjection.sourceRefs",
      },
    ]);
    for (const action of envelope.actions.debugOnly ?? []) {
      expect(primaryIds.has(action.id)).toBe(false);
    }
  });

  it("does not invent review actions locally", () => {
    const envelope = buildTodayViewModelEnvelope(safeModeTodayViewModelInput);

    expect(envelope.actions.review).toEqual([]);
    expect(
      envelope.data?.blockers.some(blocker => blocker.nextAction?.id.startsWith("review."))
    ).toBe(false);
  });

  it("preserves projection and daily-goal evidence refs", () => {
    const envelope = buildTodayViewModelEnvelope(readyTodayViewModelInput);
    const evidenceLabels = envelope.evidenceRefs?.map(ref => ref.label) ?? [];

    expect(evidenceLabels).toContain("LifeStateProjection.pending");
    expect(evidenceLabels).toContain("LifeStateProjection.taskState");
    expect(evidenceLabels).toContain("LifeStateProjection.safeMode");
    expect(evidenceLabels).toContain("daily_goals[0]: Draft the weekly planning note");
    expect(envelope.data?.sourceRefs).toHaveLength(4);
  });
});
