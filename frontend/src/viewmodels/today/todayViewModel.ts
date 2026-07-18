import type {
  BackendEntityRef,
  EvidenceRef,
  ProductAction,
  ProviderPrivacyBoundarySummary,
  ReviewAction,
  RiskLevel,
  ViewModelEnvelope,
} from "../shared/viewModelEnvelope";

export type TodayReadinessState =
  | "ready"
  | "limited"
  | "blocked"
  | "safe_mode"
  | "empty"
  | "unknown";

export type TodayDailyStateSummary = {
  headline: string;
  summary: string;
  readiness: TodayReadinessState;
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary;
  evidenceRefs: EvidenceRef[];
};

export type TodaySafeModeSummary = {
  active: boolean;
  reason: string | null;
  blocksExternalActions: boolean;
  blocksDurableWrites: boolean;
  evidenceRefs: EvidenceRef[];
};

export type TodayTaskPressureSummary = {
  activeCount: number;
  waitingPermissionCount: number;
  blockedCount: number;
  staleCount: number;
  highestRisk: RiskLevel | "none" | "unknown";
  evidenceRefs: EvidenceRef[];
};

export type TodayBlockerCategory =
  | "safe_mode"
  | "waiting_review"
  | "waiting_permission"
  | "blocked_task"
  | "provider_privacy"
  | "missing_context"
  | "unknown";

export type TodayBlockerSummary = {
  id: string;
  category: TodayBlockerCategory;
  title: string;
  nextAction: ProductAction | ReviewAction | null;
  evidenceRefs: EvidenceRef[];
};

export type TodaySuggestionTargetSurface =
  | "workspace"
  | "review_center"
  | "tasks"
  | "lifemodel"
  | "memory"
  | "settings";

export type TodaySuggestion = {
  id: string;
  title: string;
  reason: string;
  targetSurface: TodaySuggestionTargetSurface;
  action: ProductAction;
  evidenceRefs: EvidenceRef[];
};

export type TodayDailyGoalStatus =
  | "not_started"
  | "in_progress"
  | "blocked"
  | "done"
  | "stale"
  | "unknown";

export type TodayDailyGoalPriority = "low" | "medium" | "high" | "unknown";

export type TodayDailyGoalSummary = {
  goalRef: BackendEntityRef;
  title: string;
  status: TodayDailyGoalStatus;
  priority: TodayDailyGoalPriority;
  backendClassification: "unknown" | "PHASE_2_REQUIRED" | string;
  evidenceRefs: EvidenceRef[];
};

export type TodayViewModel = {
  dailyStateSummary: TodayDailyStateSummary;
  safeMode: TodaySafeModeSummary;
  pendingReviewCount: number;
  currentTaskPressure: TodayTaskPressureSummary;
  blockers: TodayBlockerSummary[];
  suggestions: TodaySuggestion[];
  primaryDailyGoal: TodayDailyGoalSummary | null;
  nextRecommendedAction: ProductAction | null;
  workspaceLink: ProductAction;
  reviewCenterLink: ProductAction;
  sourceRefs: EvidenceRef[];
};

export type TodayViewModelEnvelope = ViewModelEnvelope<TodayViewModel>;
