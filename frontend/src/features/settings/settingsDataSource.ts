import {
  type AppConfig,
  type ArtifactOutputDirectorySelection,
  type CredentialBootstrapSnapshot,
  type CredentialRecoveryReport,
  type LifeSafeModeProjection,
  type LlmConnectionTestResult,
  type ProviderPrivacyBoundarySummary,
  type ProductDiagnosticsViewModel,
  type ReviewItem,
  type ViewModelEnvelope,
} from "@/tauri";
import { getReviewCenterViewModel } from "@/ipc/review";
import {
  getConfig,
  getLifeStateProjection,
  getProductDiagnosticsViewModel,
  getProviderPrivacyBoundarySummary,
  recoverRequiredCredentialAccess,
  saveConfig,
  selectArtifactOutputDirectory,
  testLlmConnection,
} from "@/ipc/settings";
import { productErrorCode as errorText } from "@/shared/productError";
import { buildReadModelErrorEnvelope } from "@/shared/readModelEnvelope";

export type SettingsDiagnostic = {
  id:
    | "sanitized_config"
    | "provider_privacy_boundary"
    | "life_state_projection"
    | "review_item_resolution"
    | "product_diagnostics";
  status: "loaded" | "failed" | "not_requested" | "missing";
  message?: string;
};

export type SettingsSnapshot = {
  config: AppConfig | null;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  safeMode: LifeSafeModeProjection | null;
  credentialBootstrap?: CredentialBootstrapSnapshot | null;
  productDiagnostics?: ProductDiagnosticsViewModel | null;
  diagnostics: SettingsDiagnostic[];
};

export type SettingsConnectionTestOutcome = {
  result: LlmConnectionTestResult;
  reviewItem: ReviewItem | null;
  reviewResolution: "not_requested" | "resolved" | "missing" | "ambiguous" | "unavailable";
  reviewResolutionMessage?: string;
};

export interface SettingsDataSource {
  loadSettings(): Promise<SettingsSnapshot>;
  initializeRequiredCredentials?(): Promise<CredentialRecoveryReport>;
  testProviderConnection(config: AppConfig): Promise<SettingsConnectionTestOutcome>;
  saveSettings(config: AppConfig): Promise<void>;
  selectArtifactOutputDirectory?(): Promise<ArtifactOutputDirectorySelection>;
}

function boundaryErrorEnvelope(message: string): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return buildReadModelErrorEnvelope(
    "ProviderPrivacyBoundarySummary",
    "provider_privacy_boundary.load_failed",
    `ProviderPrivacyBoundarySummary could not be loaded: ${message}`
  );
}

export function buildSettingsErrorSnapshot(error: unknown): SettingsSnapshot {
  const message = errorText(error);
  return {
    config: null,
    boundaryEnvelope: boundaryErrorEnvelope(message),
    safeMode: null,
    credentialBootstrap: null,
    productDiagnostics: null,
    diagnostics: [
      { id: "sanitized_config", status: "failed", message },
      { id: "provider_privacy_boundary", status: "failed", message },
      { id: "life_state_projection", status: "failed", message },
      { id: "review_item_resolution", status: "not_requested" },
    ],
  };
}

async function loadSettings(): Promise<SettingsSnapshot> {
  const [configResult, boundaryResult, projectionResult, diagnosticsResult] =
    await Promise.allSettled([
      getConfig(),
      getProviderPrivacyBoundarySummary(),
      getLifeStateProjection(),
      getProductDiagnosticsViewModel(),
    ]);
  const configError = configResult.status === "rejected" ? errorText(configResult.reason) : null;
  const boundaryError =
    boundaryResult.status === "rejected" ? errorText(boundaryResult.reason) : null;
  const projectionError =
    projectionResult.status === "rejected" ? errorText(projectionResult.reason) : null;
  const diagnosticsError =
    diagnosticsResult.status === "rejected" ? errorText(diagnosticsResult.reason) : null;

  return {
    config: configResult.status === "fulfilled" ? configResult.value : null,
    boundaryEnvelope:
      boundaryResult.status === "fulfilled"
        ? boundaryResult.value
        : boundaryErrorEnvelope(boundaryError ?? "unknown_error"),
    safeMode: projectionResult.status === "fulfilled" ? projectionResult.value.safeMode : null,
    credentialBootstrap:
      projectionResult.status === "fulfilled"
        ? (projectionResult.value.credentialBootstrap ?? null)
        : null,
    productDiagnostics: diagnosticsResult.status === "fulfilled" ? diagnosticsResult.value : null,
    diagnostics: [
      configError
        ? { id: "sanitized_config", status: "failed", message: configError }
        : { id: "sanitized_config", status: "loaded" },
      boundaryError
        ? { id: "provider_privacy_boundary", status: "failed", message: boundaryError }
        : { id: "provider_privacy_boundary", status: "loaded" },
      projectionError
        ? { id: "life_state_projection", status: "failed", message: projectionError }
        : { id: "life_state_projection", status: "loaded" },
      { id: "review_item_resolution", status: "not_requested" },
      diagnosticsError
        ? { id: "product_diagnostics", status: "failed", message: diagnosticsError }
        : { id: "product_diagnostics", status: "loaded" },
    ],
  };
}

async function resolveReviewItem(
  result: LlmConnectionTestResult
): Promise<
  Pick<SettingsConnectionTestOutcome, "reviewItem" | "reviewResolution" | "reviewResolutionMessage">
> {
  const proposalId = result.review_proposal_id?.trim();
  if (!proposalId) {
    return { reviewItem: null, reviewResolution: "not_requested" };
  }

  try {
    const envelope = await getReviewCenterViewModel();
    if (envelope.status !== "ready" || !envelope.data) {
      return {
        reviewItem: null,
        reviewResolution: "unavailable",
        reviewResolutionMessage: `ReviewCenterViewModel status is ${envelope.status}.`,
      };
    }
    const matches = envelope.data.items.filter(
      candidate => candidate.source.proposalId === proposalId
    );
    if (matches.length === 1) {
      return { reviewItem: matches[0], reviewResolution: "resolved" };
    }
    return matches.length === 0
      ? {
          reviewItem: null,
          reviewResolution: "missing",
          reviewResolutionMessage: "The exact proposal was not present in ReviewCenterViewModel.",
        }
      : {
          reviewItem: null,
          reviewResolution: "ambiguous",
          reviewResolutionMessage:
            "Multiple ReviewItems referenced the same proposal; navigation remains disabled.",
        };
  } catch (error) {
    return {
      reviewItem: null,
      reviewResolution: "unavailable",
      reviewResolutionMessage: errorText(error),
    };
  }
}

async function testProviderConnection(config: AppConfig): Promise<SettingsConnectionTestOutcome> {
  const result = await testLlmConnection(config);
  return { result, ...(await resolveReviewItem(result)) };
}

export const tauriSettingsDataSource: SettingsDataSource = {
  loadSettings,
  initializeRequiredCredentials: recoverRequiredCredentialAccess,
  testProviderConnection,
  saveSettings: saveConfig,
  selectArtifactOutputDirectory,
};
