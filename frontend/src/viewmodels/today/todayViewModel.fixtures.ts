import type { DailyGoal } from "../../types";
import type { LifeStateProjection } from "../../tauri";
import type { BuildTodayViewModelEnvelopeInput } from "./todayViewModelAdapter";

export function makeLifeStateProjection(
  overrides: Partial<LifeStateProjection> = {}
): LifeStateProjection {
  const pending = {
    pendingProposalCount: 2,
    editedProposalCount: 0,
    totalReviewRequiredCount: 2,
    highRiskReviewRequiredCount: 1,
    proposalStoreStatus: "ok",
    requiresUserAction: true,
    ...overrides.pending,
  };
  const readiness = {
    chatReady: true,
    usageReady: true,
    lifeModelReady: true,
    modelEmpty: false,
    pendingBuilderReviewSessions: 0,
    unfinishedBuilderSessions: 0,
    databaseStatus: "ok",
    readinessIssues: [],
    usageReadinessIssues: [],
    ...overrides.readiness,
  };
  const taskState = {
    taskStoreStatus: "ok",
    latestTaskId: "task-current",
    latestTaskStatus: "running",
    runningCount: 1,
    waitingPermissionCount: 0,
    blockedCount: 0,
    failedCount: 0,
    cancelledCount: 0,
    completedCount: 3,
    activeCount: 1,
    ...overrides.taskState,
  };
  const safeMode = {
    active: false,
    reason: "System is not in Safe Mode.",
    sourceRefs: [],
    ...overrides.safeMode,
  };
  const toolPermissions = {
    totalCount: 0,
    activeCount: 0,
    consumedCount: 0,
    allowCount: 0,
    denyCount: 0,
    askEveryTimeCount: 0,
    allowOnceCount: 0,
    allowUntilRevokedCount: 0,
    ...overrides.toolPermissions,
  };
  const surfaces =
    overrides.surfaces ??
    ["today", "mailbox", "chat", "companion", "life_model", "settings"].map(surface => ({
      surface,
      pendingReviewCount: pending.pendingProposalCount,
      editedReviewCount: pending.editedProposalCount,
      totalReviewRequiredCount: pending.totalReviewRequiredCount,
      readinessStatus: safeMode.active ? "blocked" : "ready",
      taskStatus: taskState.latestTaskStatus ?? "idle",
      safeModeActive: safeMode.active,
      waitingPermissionCount: taskState.waitingPermissionCount,
      activeToolPermissionCount: toolPermissions.activeCount,
    }));

  return {
    version: overrides.version ?? "life_state_projection_v1",
    generatedAt: overrides.generatedAt ?? "2026-07-09T00:00:00.000Z",
    persistence: overrides.persistence ?? {
      mode: "isolated_evaluation",
      canonicalWritesAllowed: true,
      providerDispatchAllowed: false,
      toolDispatchAllowed: false,
      liveOrCanonicalCreditEligible: false,
      sealed: true,
      stores: [],
      globalReasonCodes: ["isolated_evaluation"],
    },
    pending,
    readiness,
    taskState,
    safeMode,
    toolPermissions,
    safePaths: overrides.safePaths ?? [],
    surfaces,
    sourceRefs: overrides.sourceRefs ?? [
      "LifeStateProjection.pending",
      "LifeStateProjection.taskState",
      "LifeStateProjection.safeMode",
    ],
  };
}

export function makeDailyGoal(overrides: Partial<DailyGoal> = {}): DailyGoal {
  return {
    name: "Draft the weekly planning note",
    done: false,
    time_block: {
      start: "09:00",
      end: "10:00",
    },
    ...overrides,
  };
}

export const readyTodayViewModelInput: BuildTodayViewModelEnvelopeInput = {
  projection: makeLifeStateProjection(),
  dailyGoals: [makeDailyGoal()],
};

export const emptyTodayViewModelInput: BuildTodayViewModelEnvelopeInput = {
  projection: makeLifeStateProjection({
    pending: {
      pendingProposalCount: 0,
      editedProposalCount: 0,
      totalReviewRequiredCount: 0,
      highRiskReviewRequiredCount: 0,
      proposalStoreStatus: "ok",
      requiresUserAction: false,
    },
    taskState: {
      taskStoreStatus: "ok",
      latestTaskId: null,
      latestTaskStatus: null,
      runningCount: 0,
      waitingPermissionCount: 0,
      blockedCount: 0,
      failedCount: 0,
      cancelledCount: 0,
      completedCount: 0,
      activeCount: 0,
    },
  }),
  dailyGoals: [],
};

export const safeModeTodayViewModelInput: BuildTodayViewModelEnvelopeInput = {
  projection: makeLifeStateProjection({
    safeMode: {
      active: true,
      reason: "LifeModel storage is unavailable; Safe Mode is active.",
      sourceRefs: ["diagnostics.startup_warnings"],
    },
  }),
  dailyGoals: [makeDailyGoal()],
};

export const staleTodayViewModelInput: BuildTodayViewModelEnvelopeInput = {
  projection: makeLifeStateProjection(),
  dailyGoals: [makeDailyGoal()],
  status: "stale",
  lastUpdatedAt: "2026-07-08T23:00:00.000Z",
};

export const errorTodayViewModelInput: BuildTodayViewModelEnvelopeInput = {
  projection: null,
  dailyGoals: [makeDailyGoal()],
  status: "error",
  errorMessage: "LifeStateProjection failed to load.",
};
