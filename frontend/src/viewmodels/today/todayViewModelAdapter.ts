import type { DailyGoal } from "../../types";
import type { LifeStateProjection } from "../../tauri";
import type {
  DebugAction,
  EvidenceRef,
  ProductAction,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
  ViewModelStatus,
  ViewModelWarning,
} from "../shared/viewModelEnvelope";
import type {
  TodayBlockerSummary,
  TodayDailyGoalSummary,
  TodayDailyStateSummary,
  TodayReadinessState,
  TodaySafeModeSummary,
  TodayTaskPressureSummary,
  TodayViewModel,
  TodayViewModelEnvelope,
} from "./todayViewModel";

export type BuildTodayViewModelEnvelopeInput = {
  projection: LifeStateProjection | null;
  dailyGoals?: DailyGoal[] | null;
  providerPrivacyBoundary?: ProviderPrivacyBoundarySummary | null;
  status?: ViewModelStatus;
  errorMessage?: string;
  lastUpdatedAt?: string | null;
};

const BACKEND_SOURCE = "backend-readmodel" as const;
const CURRENT_WORKSPACE_ROUTE_REF = "route:companion";
const CURRENT_REVIEW_ROUTE_REF = "route:mailbox";
const STALE_DISABLED_REASON = "Refresh Today state before using this action.";

// This adapter owns presentation composition only. Every product fact remains
// owned by one of the backend inputs named here.
export const TODAY_VIEW_MODEL_AUTHORITY_CONTRACT = Object.freeze({
  version: "openlife.today-adapter.v1",
  compositionOwner: "strict_frontend_adapter",
  inputs: Object.freeze({
    readinessAndSafeMode: "LifeStateProjection",
    taskPressureAndPendingReview: "LifeStateProjection",
    dailyGoals: "get_daily_goals compatibility projection",
    providerPrivacyBoundary: "ProviderPrivacyBoundarySummary",
  }),
  forbiddenLocalTruth: Object.freeze([
    "proposal status",
    "task lifecycle",
    "provider route",
    "external transmission",
    "durable completion",
  ]),
});

export function buildTodayViewModelEnvelope(
  input: BuildTodayViewModelEnvelopeInput
): TodayViewModelEnvelope {
  if (input.status === "loading") {
    return buildNullEnvelope({
      status: "loading",
      lastUpdatedAt: input.lastUpdatedAt ?? null,
      warnings: [],
      primaryActions: [refreshAction(false, "Today state is still loading.")],
    });
  }

  if (input.status === "error" || !input.projection) {
    const projectionRefs = input.projection ? evidenceRefsFromProjection(input.projection) : [];
    return buildNullEnvelope({
      status: "error",
      lastUpdatedAt: input.lastUpdatedAt ?? input.projection?.generatedAt ?? null,
      evidenceRefs: projectionRefs,
      warnings: [
        {
          code: input.projection ? "today.load_error" : "today.projection_unavailable",
          message:
            input.errorMessage ??
            "TodayViewModel could not load LifeStateProjection; no raw-domain fallback was used.",
          severity: "error",
          evidenceRefs: projectionRefs,
        },
      ],
      primaryActions: [refreshAction(true)],
      debugActions: debugActionsFor(projectionRefs, "error"),
    });
  }

  const dailyGoals = input.dailyGoals ?? [];
  const sourceRefs = [
    ...evidenceRefsFromProjection(input.projection),
    ...dailyGoals.map((goal, index) => evidenceRefFromDailyGoal(goal, index)),
  ];
  const loadedStatus = deriveLoadedStatus(input.projection, dailyGoals, input.status);
  const workspaceLink = buildWorkspaceLink(loadedStatus);
  const reviewCenterLink = buildReviewCenterLink();
  const providerPrivacyBoundary =
    input.providerPrivacyBoundary ?? buildProviderPrivacyBoundary(sourceRefs);
  const primaryDailyGoal = buildPrimaryDailyGoal(dailyGoals);
  const safeMode = buildSafeModeSummary(input.projection);
  const currentTaskPressure = buildTaskPressure(input.projection, sourceRefs);
  const blockers = buildBlockers(input.projection, workspaceLink, reviewCenterLink);
  const warnings = buildLoadedWarnings({
    status: loadedStatus,
    dailyGoalCount: dailyGoals.length,
    sourceRefs,
  });

  const data: TodayViewModel = {
    dailyStateSummary: buildDailyStateSummary({
      status: loadedStatus,
      projection: input.projection,
      primaryDailyGoal,
      providerPrivacyBoundary,
      sourceRefs,
    }),
    safeMode,
    pendingReviewCount: input.projection.pending.totalReviewRequiredCount,
    currentTaskPressure,
    blockers,
    suggestions: [],
    primaryDailyGoal,
    nextRecommendedAction: null,
    workspaceLink,
    reviewCenterLink,
    sourceRefs,
  };

  return {
    data,
    status: loadedStatus,
    lastUpdatedAt: input.lastUpdatedAt ?? input.projection.generatedAt,
    source: BACKEND_SOURCE,
    evidenceRefs: sourceRefs,
    warnings,
    actions: {
      primary: [refreshAction(true), workspaceLink, reviewCenterLink],
      review: [],
      debugOnly: debugActionsFor(sourceRefs, loadedStatus),
    },
  };
}

function buildNullEnvelope({
  status,
  lastUpdatedAt,
  evidenceRefs = [],
  warnings = [],
  primaryActions,
  debugActions = [],
}: {
  status: ViewModelStatus;
  lastUpdatedAt: string | null;
  evidenceRefs?: EvidenceRef[];
  warnings?: ViewModelWarning[];
  primaryActions: ProductAction[];
  debugActions?: DebugAction[];
}): ViewModelEnvelope<TodayViewModel> {
  return {
    data: null,
    status,
    lastUpdatedAt,
    source: BACKEND_SOURCE,
    evidenceRefs,
    warnings,
    actions: {
      primary: primaryActions,
      review: [],
      debugOnly: debugActions,
    },
  };
}

function deriveLoadedStatus(
  projection: LifeStateProjection,
  dailyGoals: DailyGoal[],
  requestedStatus?: ViewModelStatus
): ViewModelStatus {
  if (requestedStatus === "stale") return "stale";
  if (requestedStatus === "ready" || requestedStatus === "empty") return requestedStatus;
  if (
    dailyGoals.length === 0 &&
    projection.taskState.activeCount === 0 &&
    projection.taskState.waitingPermissionCount === 0 &&
    projection.taskState.blockedCount === 0 &&
    !projection.safeMode.active
  ) {
    return "empty";
  }
  return "ready";
}

function buildDailyStateSummary({
  status,
  projection,
  primaryDailyGoal,
  providerPrivacyBoundary,
  sourceRefs,
}: {
  status: ViewModelStatus;
  projection: LifeStateProjection;
  primaryDailyGoal: TodayDailyGoalSummary | null;
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary;
  sourceRefs: EvidenceRef[];
}): TodayDailyStateSummary {
  const readiness = deriveReadiness(status, projection, primaryDailyGoal);
  return {
    headline: headlineFor(readiness),
    summary: summaryFor(readiness),
    readiness,
    providerPrivacyBoundary,
    evidenceRefs: sourceRefs,
  };
}

function deriveReadiness(
  status: ViewModelStatus,
  projection: LifeStateProjection,
  primaryDailyGoal: TodayDailyGoalSummary | null
): TodayReadinessState {
  if (status === "stale") return "limited";
  if (projection.safeMode.active) return "safe_mode";
  if (projection.taskState.blockedCount > 0 || projection.taskState.waitingPermissionCount > 0) {
    return "blocked";
  }
  if (
    !primaryDailyGoal &&
    projection.taskState.activeCount === 0 &&
    projection.pending.totalReviewRequiredCount === 0
  ) {
    return "empty";
  }
  if (
    projection.readiness.readinessIssues.length > 0 ||
    projection.readiness.usageReadinessIssues.length > 0 ||
    !projection.readiness.chatReady ||
    !projection.readiness.usageReady
  ) {
    return "limited";
  }
  return "ready";
}

function headlineFor(readiness: TodayReadinessState): string {
  switch (readiness) {
    case "safe_mode":
      return "Safe mode is active";
    case "blocked":
      return "Today has backend-reported blockers";
    case "empty":
      return "No current daily goal or active task";
    case "limited":
      return "Today state is limited";
    case "ready":
      return "Today state is loaded";
    case "unknown":
      return "Today state is unknown";
  }
}

function summaryFor(readiness: TodayReadinessState): string {
  switch (readiness) {
    case "safe_mode":
      return "LifeStateProjection reports Safe Mode; external or durable actions should remain blocked by product policy.";
    case "blocked":
      return "LifeStateProjection reports task pressure that requires attention.";
    case "empty":
      return "LifeStateProjection loaded successfully, but no daily goal or active task was provided.";
    case "limited":
      return "The limited slice uses projection-backed fields and preserves missing Today-specific fields as unknown.";
    case "ready":
      return "The limited slice is backed by LifeStateProjection and the existing daily-goal input.";
    case "unknown":
      return "The limited slice has no backend-owned summary for this state.";
  }
}

function buildProviderPrivacyBoundary(evidenceRefs: EvidenceRef[]): ProviderPrivacyBoundarySummary {
  return {
    routeType: "unknown",
    externalTransmission: "unknown",
    providerLabel: "unknown",
    modelLabel: "unknown",
    privacyLabel: "PHASE_2_REQUIRED",
    risk: "unknown",
    localOnlyRequired: false,
    blockedReason: "Provider/privacy boundary is not backend-owned by the Today limited slice.",
    evidenceRefs,
  };
}

function buildSafeModeSummary(projection: LifeStateProjection): TodaySafeModeSummary {
  const evidenceRefs = projection.safeMode.sourceRefs.map((sourceRef, index) =>
    evidenceRefFromProjectionSource(sourceRef, index)
  );
  return {
    active: projection.safeMode.active,
    reason: projection.safeMode.active ? projection.safeMode.reason : null,
    blocksExternalActions: projection.safeMode.active,
    blocksDurableWrites: projection.safeMode.active,
    evidenceRefs,
  };
}

function buildTaskPressure(
  projection: LifeStateProjection,
  sourceRefs: EvidenceRef[]
): TodayTaskPressureSummary {
  const hasPressure =
    projection.taskState.activeCount > 0 ||
    projection.taskState.waitingPermissionCount > 0 ||
    projection.taskState.blockedCount > 0;
  return {
    activeCount: projection.taskState.activeCount,
    waitingPermissionCount: projection.taskState.waitingPermissionCount,
    blockedCount: projection.taskState.blockedCount,
    staleCount: 0,
    highestRisk: hasPressure ? "unknown" : "none",
    evidenceRefs: sourceRefs,
  };
}

function buildBlockers(
  projection: LifeStateProjection,
  workspaceLink: ProductAction,
  reviewCenterLink: ProductAction
): TodayBlockerSummary[] {
  const blockers: TodayBlockerSummary[] = [];
  const projectionRefs = evidenceRefsFromProjection(projection);

  if (projection.safeMode.active) {
    blockers.push({
      id: "today.blocker.safe_mode",
      category: "safe_mode",
      title: "Safe Mode is active",
      nextAction: null,
      evidenceRefs: projection.safeMode.sourceRefs.map((sourceRef, index) =>
        evidenceRefFromProjectionSource(sourceRef, index)
      ),
    });
  }

  if (projection.taskState.waitingPermissionCount > 0) {
    blockers.push({
      id: "today.blocker.waiting_permission",
      category: "waiting_permission",
      title: "Tasks are waiting for permission",
      nextAction: reviewCenterLink,
      evidenceRefs: projectionRefs,
    });
  }

  if (projection.taskState.blockedCount > 0) {
    blockers.push({
      id: "today.blocker.blocked_task",
      category: "blocked_task",
      title: "Tasks are blocked",
      nextAction: workspaceLink,
      evidenceRefs: projectionRefs,
    });
  }

  return blockers;
}

function buildPrimaryDailyGoal(dailyGoals: DailyGoal[]): TodayDailyGoalSummary | null {
  const selectedIndex = dailyGoals.findIndex(goal => !goal.done);
  const fallbackIndex = selectedIndex >= 0 ? selectedIndex : dailyGoals.length > 0 ? 0 : -1;
  if (fallbackIndex < 0) return null;

  const goal = dailyGoals[fallbackIndex];
  const evidenceRefs = [evidenceRefFromDailyGoal(goal, fallbackIndex)];
  return {
    goalRef: {
      id: `daily-goal:${fallbackIndex}`,
      kind: "evidence",
      label: `Daily goal ${fallbackIndex + 1}`,
    },
    title: goal.name,
    status: goal.done ? "done" : "not_started",
    priority: "unknown",
    backendClassification: "unknown",
    evidenceRefs,
  };
}

function buildLoadedWarnings({
  status,
  dailyGoalCount,
  sourceRefs,
}: {
  status: ViewModelStatus;
  dailyGoalCount: number;
  sourceRefs: EvidenceRef[];
}): ViewModelWarning[] {
  const warnings: ViewModelWarning[] = [
    {
      code: "today.provider_privacy_boundary_required",
      message:
        "Provider/privacy boundary is PHASE_2_REQUIRED and remains unknown in this limited slice.",
      severity: "info",
      evidenceRefs: sourceRefs,
    },
    {
      code: "today.suggestions_limited",
      message:
        "Suggestions require a backend Today read model; the adapter does not infer them locally.",
      severity: "info",
      evidenceRefs: sourceRefs,
    },
    {
      code: "today.next_action_limited",
      message:
        "Next recommended action requires a backend Today read model; the adapter leaves it null.",
      severity: "info",
      evidenceRefs: sourceRefs,
    },
  ];

  if (dailyGoalCount > 0) {
    warnings.push({
      code: "today.goal_classification_limited",
      message: "Daily-goal classification is not backend-owned in this slice and remains unknown.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    });
  }

  if (status === "stale") {
    warnings.push({
      code: "today.stale",
      message: "TodayViewModel data is stale; risky actions are disabled until refresh.",
      severity: "warning",
      evidenceRefs: sourceRefs,
    });
  }

  return warnings;
}

function buildWorkspaceLink(status: ViewModelStatus): ProductAction {
  const stale = status === "stale";
  return {
    id: "today.open_current_workspace_route",
    label: "Open current workspace route",
    kind: "open",
    enabled: !stale,
    disabledReason: stale ? STALE_DISABLED_REASON : undefined,
    targetRef: CURRENT_WORKSPACE_ROUTE_REF,
  };
}

function buildReviewCenterLink(): ProductAction {
  return {
    id: "today.open_current_review_route",
    label: "Open current review route",
    kind: "open",
    enabled: true,
    targetRef: CURRENT_REVIEW_ROUTE_REF,
  };
}

function refreshAction(enabled: boolean, disabledReason?: string): ProductAction {
  return {
    id: "today.refresh",
    label: "Refresh Today state",
    kind: "refresh",
    enabled,
    disabledReason,
    targetRef: "today",
  };
}

function debugActionsFor(evidenceRefs: EvidenceRef[], status: ViewModelStatus): DebugAction[] {
  if (status === "loading" || evidenceRefs.length === 0) return [];
  return [
    {
      id: "today.inspect_projection_source_refs",
      label: "Inspect projection source refs",
      kind: "raw_json",
      enabled: status !== "stale",
      developerOnly: true,
      targetRef: "LifeStateProjection.sourceRefs",
    },
  ];
}

function evidenceRefsFromProjection(projection: LifeStateProjection): EvidenceRef[] {
  if (projection.sourceRefs.length === 0) {
    return [
      {
        id: "projection:LifeStateProjection",
        label: "LifeStateProjection",
        source: BACKEND_SOURCE,
        sensitivity: "local_private",
      },
    ];
  }
  return projection.sourceRefs.map((sourceRef, index) =>
    evidenceRefFromProjectionSource(sourceRef, index)
  );
}

function evidenceRefFromProjectionSource(sourceRef: string, index: number): EvidenceRef {
  return {
    id: `projection:${index}:${sourceRef}`,
    label: sourceRef,
    source: BACKEND_SOURCE,
    sensitivity: "local_private",
  };
}

function evidenceRefFromDailyGoal(goal: DailyGoal, index: number): EvidenceRef {
  return {
    id: `daily-goals:${index}`,
    label: `daily_goals[${index}]: ${goal.name}`,
    source: BACKEND_SOURCE,
    sensitivity: "local_private",
  };
}
