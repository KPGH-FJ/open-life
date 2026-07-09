import type {
  BackendEntityRef,
  EvidenceRef,
  ProductAction,
  ReviewItemMaterializationStatus,
  ViewModelEnvelope,
} from "../shared/viewModelEnvelope";

export type LifeModelTruthMode =
  | "canonical"
  | "current_compatibility"
  | "candidate"
  | "pending_review"
  | "manual_override"
  | "unknown"
  | "unavailable";

export type LifeModelDimensionId = "identity" | "goals" | "capabilities" | "state";

export type LifeModelConfidence = "low" | "medium" | "high" | "unknown";

export type LifeModelLimitedOwnerStatus = "PARTIAL" | "PHASE_2_REQUIRED" | "UNKNOWN";

export type LifeModelLimitedProvenance = "limited" | "unknown" | "PHASE_2_REQUIRED";

export type LifeModelReviewItemRef = BackendEntityRef & {
  kind: "review_item";
};

export type LifeModelCanonicalSummary = {
  lifeModelRef: BackendEntityRef;
  title: string;
  summary: string;
  versionLabel: string;
  lastMaterializedAt: string | null;
  evidenceRefs: EvidenceRef[];
};

export type LifeModelCurrentViewSummary = {
  currentViewRef: BackendEntityRef;
  compatibilityMode: boolean;
  label: string;
  summary: string;
  divergenceFromCanonical: "none" | "minor" | "material" | "unknown";
  evidenceRefs: EvidenceRef[];
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelDimensionSummary = {
  id: LifeModelDimensionId;
  label: string;
  summary: string;
  confidence: LifeModelConfidence;
  stale: boolean;
  pendingReviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  provenance: LifeModelLimitedProvenance;
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelTrustQualityState = {
  readiness: "not_built" | "limited" | "usable_with_limits" | "ready" | "stale" | "unknown";
  completionScore: number | null;
  missingDimensionCount: number;
  staleDimensionCount: number;
  warningRefs: EvidenceRef[];
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelPendingUpdateCounts = {
  candidate: number;
  pendingReview: number;
  approvedNotApplied: number;
  failedMaterialization: number;
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelCandidateChange = {
  changeRef: BackendEntityRef;
  title: string;
  changeKind: "add" | "update" | "remove" | "merge" | "manual_override" | "unknown";
  affectedDimensionIds: string[];
  reviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  decisionStatus: "pending" | "accepted" | "edited" | "postponed" | "unknown";
};

export type LifeModelMaterializedChange = {
  changeRef: BackendEntityRef;
  title: string;
  materializationStatus: ReviewItemMaterializationStatus;
  materializedAt: string | null;
  rollbackAvailable: boolean;
  evidenceRefs: EvidenceRef[];
};

export type LifeModelManualOverrideState = {
  active: boolean;
  blockedReason?: string;
  draftRef: BackendEntityRef | null;
  saveAction: ProductAction | null;
  reviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelMemoryLinkageSummary = {
  linkedMemoryCount: number;
  candidateMemoryCount: number;
  materializedMemoryCount: number;
  conflictCount: number;
  memoryRefs: BackendEntityRef[];
  evidenceRefs: EvidenceRef[];
  linkageStatus: "partial" | "unknown";
  tierSummary: {
    total: number | null;
    tier1: number | null;
    tier2: number | null;
    tier3: number | null;
    archived: number | null;
  };
  ownerStatus: LifeModelLimitedOwnerStatus;
};

export type LifeModelViewModel = {
  truthMode: LifeModelTruthMode;
  canonicalSummary: LifeModelCanonicalSummary | null;
  currentViewSummary: LifeModelCurrentViewSummary | null;
  dimensionSummaries: LifeModelDimensionSummary[];
  trustQualityState: LifeModelTrustQualityState;
  pendingUpdateCounts: LifeModelPendingUpdateCounts;
  provenanceRefs: EvidenceRef[];
  candidateChanges: LifeModelCandidateChange[];
  materializedChanges: LifeModelMaterializedChange[];
  manualOverrideState: LifeModelManualOverrideState | null;
  relatedReviewItemRefs: LifeModelReviewItemRef[];
  memoryLinkage: LifeModelMemoryLinkageSummary;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

export type LifeModelViewModelEnvelope = ViewModelEnvelope<LifeModelViewModel>;
