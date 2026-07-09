export type ViewModelStatus = "loading" | "ready" | "empty" | "error" | "stale";

export type EvidenceSource =
  | "backend-readmodel"
  | "audit"
  | "task"
  | "review"
  | "memory"
  | "lifemodel"
  | "settings"
  | "provider";

export type EvidenceSensitivity = "public" | "local_private" | "sensitive" | "redacted";

export type EvidenceRef = {
  id: string;
  label: string;
  source: EvidenceSource;
  sensitivity?: EvidenceSensitivity;
};

export type ViewModelWarningSeverity = "info" | "warning" | "error";

export type ViewModelWarning = {
  code: string;
  message: string;
  severity: ViewModelWarningSeverity;
  evidenceRefs?: EvidenceRef[];
};

export type ProductActionKind =
  | "open"
  | "start"
  | "continue"
  | "retry"
  | "cancel"
  | "refresh"
  | "inspect"
  | "configure";

export type ProductAction = {
  id: string;
  label: string;
  kind: ProductActionKind;
  enabled: boolean;
  disabledReason?: string;
  targetRef?: string;
};

export type ReviewItemMaterializationStatus =
  | "not_applicable"
  | "not_started"
  | "applying"
  | "applied"
  | "failed"
  | "rolled_back"
  | "unknown";

export type ReviewActionBase = {
  id: string;
  label: string;
  enabled: boolean;
  disabledReason?: string;
  requiresConfirmation?: boolean;
  targetReviewItemId: string;
  expectedMaterializationStatusAfterDispatch?: ReviewItemMaterializationStatus;
};

export type ReviewActionKindEffectInvariant =
  | { kind: "approve" | "reject" | "edit" | "later" | "revoke"; effect: "decision_only" }
  | { kind: "apply"; effect: "materialization_request" }
  | { kind: "resume"; effect: "task_resume_request" }
  | { kind: "view_evidence"; effect: "evidence_only" };

export type ReviewAction = ReviewActionBase & ReviewActionKindEffectInvariant;

export type DebugAction = {
  id: string;
  label: string;
  kind: "raw_trace" | "raw_json" | "export" | "provider_health" | "route_evidence" | "transcript";
  enabled: boolean;
  developerOnly?: boolean;
  targetRef?: string;
};

export type ViewModelEnvelope<T> = {
  data: T | null;
  status: ViewModelStatus;
  lastUpdatedAt: string | null;
  source: "backend-readmodel";
  evidenceRefs?: EvidenceRef[];
  warnings?: ViewModelWarning[];
  actions: {
    primary: ProductAction[];
    review?: ReviewAction[];
    debugOnly?: DebugAction[];
  };
};

export type RiskLevel = "low" | "medium" | "high" | "critical";

export type ProviderPrivacyBoundarySummary = {
  routeType: "local" | "cloud" | "hybrid" | "auto" | "unknown";
  externalTransmission: "not_sent" | "sent" | "possible" | "unknown";
  providerLabel: string;
  modelLabel: string;
  privacyLabel: string;
  risk: RiskLevel | "none" | "unknown";
  localOnlyRequired: boolean;
  blockedReason?: string;
  evidenceRefs: EvidenceRef[];
};

export type BackendEntityRef = {
  id: string;
  kind:
    | "task"
    | "run"
    | "conversation"
    | "review_item"
    | "memory"
    | "lifemodel"
    | "proposal"
    | "tool_permission"
    | "evidence";
  label: string;
  href?: string;
};
