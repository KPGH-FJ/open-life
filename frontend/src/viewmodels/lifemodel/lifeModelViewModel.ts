import type {
  LifeModelCandidateChange as LifeModelCandidateChangeContract,
  LifeModelCanonicalSummary as LifeModelCanonicalSummaryContract,
  LifeModelManualOverrideState as LifeModelManualOverrideStateContract,
  LifeModelMaterializedChange as LifeModelMaterializedChangeContract,
  LifeModelMemoryLinkageSummary as LifeModelMemoryLinkageSummaryContract,
  LifeModelOwnerStatus as LifeModelOwnerStatusContract,
  LifeModelPendingUpdateCounts as LifeModelPendingUpdateCountsContract,
  LifeModelReviewItemRef as LifeModelReviewItemRefContract,
  LifeModelTrustQualityState as LifeModelTrustQualityStateContract,
  LifeModelTruthMode as LifeModelTruthModeContract,
  LifeModelViewModel as LifeModelViewModelContract,
  ViewModelEnvelope,
} from "../../tauri";

// Transitional frontend import path. The canonical contract owner is
// openlife-core/src/agent/life_model_view_model.rs and frontend/src/tauri.ts
// mirrors its serialized shape for TypeScript consumers.
export type LifeModelTruthMode = LifeModelTruthModeContract;
export type LifeModelLimitedOwnerStatus = LifeModelOwnerStatusContract;
export type LifeModelReviewItemRef = LifeModelReviewItemRefContract;
export type LifeModelCanonicalSummary = LifeModelCanonicalSummaryContract;
export type LifeModelTrustQualityState = LifeModelTrustQualityStateContract;
export type LifeModelPendingUpdateCounts = LifeModelPendingUpdateCountsContract;
export type LifeModelCandidateChange = LifeModelCandidateChangeContract;
export type LifeModelMaterializedChange = LifeModelMaterializedChangeContract;
export type LifeModelManualOverrideState = LifeModelManualOverrideStateContract;
export type LifeModelMemoryLinkageSummary = LifeModelMemoryLinkageSummaryContract;
export type LifeModelViewModel = LifeModelViewModelContract;
export type LifeModelViewModelEnvelope = ViewModelEnvelope<LifeModelViewModel>;
