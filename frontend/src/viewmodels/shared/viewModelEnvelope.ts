import type {
  BackendEntityRef as BackendEntityRefContract,
  DebugAction as DebugActionContract,
  EvidenceRef as EvidenceRefContract,
  EvidenceSensitivity as EvidenceSensitivityContract,
  EvidenceSource as EvidenceSourceContract,
  ProductAction as ProductActionContract,
  ProductActionKind as ProductActionKindContract,
  ProductRiskLevel as ProductRiskLevelContract,
  PermissionDecisionContext as PermissionDecisionContextContract,
  PermissionTransmissionBoundary as PermissionTransmissionBoundaryContract,
  ProviderPrivacyBoundarySummary as ProviderPrivacyBoundarySummaryContract,
  ReviewAction as ReviewActionContract,
  ReviewActionBase as ReviewActionBaseContract,
  ReviewActionKindEffectInvariant as ReviewActionKindEffectInvariantContract,
  ReviewDecisionContext as ReviewDecisionContextContract,
  ReviewItemMaterializationStatus as ReviewItemMaterializationStatusContract,
  ViewModelEnvelope as ViewModelEnvelopeContract,
  ViewModelStatus as ViewModelStatusContract,
  ViewModelWarning as ViewModelWarningContract,
  ViewModelWarningSeverity as ViewModelWarningSeverityContract,
} from "../../tauri";

// Transitional frontend import path. The canonical contract owner is
// openlife-core/src/agent/product_read_model.rs and frontend/src/tauri.ts mirrors
// its serialized shape for TypeScript consumers.
export type ViewModelStatus = ViewModelStatusContract;
export type EvidenceSource = EvidenceSourceContract;
export type EvidenceSensitivity = EvidenceSensitivityContract;
export type EvidenceRef = EvidenceRefContract;
export type ViewModelWarningSeverity = ViewModelWarningSeverityContract;
export type ViewModelWarning = ViewModelWarningContract;
export type ProductActionKind = ProductActionKindContract;
export type ProductAction = ProductActionContract;
export type ReviewItemMaterializationStatus = ReviewItemMaterializationStatusContract;
export type ReviewActionBase = ReviewActionBaseContract;
export type ReviewActionKindEffectInvariant = ReviewActionKindEffectInvariantContract;
export type ReviewAction = ReviewActionContract;
export type ReviewDecisionContext = ReviewDecisionContextContract;
export type PermissionDecisionContext = PermissionDecisionContextContract;
export type PermissionTransmissionBoundary = PermissionTransmissionBoundaryContract;
export type DebugAction = DebugActionContract;
export type ViewModelEnvelope<T> = ViewModelEnvelopeContract<T>;
export type RiskLevel = Exclude<ProductRiskLevelContract, "none" | "unknown">;
export type ProviderPrivacyBoundarySummary = ProviderPrivacyBoundarySummaryContract;
export type BackendEntityRef = BackendEntityRefContract;
