import {
  getConfig,
  getLifeStateProjection,
  getProviderPrivacyBoundarySummary,
  getReviewCenterViewModel,
  recoverRequiredCredentialAccess,
  saveConfig,
  testLlmConnection,
  type AppConfig,
  type CredentialRecoveryReport,
  type LifeSafeModeProjection,
  type LlmConnectionTestResult,
  type ProviderPrivacyBoundarySummary,
  type ReviewItem,
  type ViewModelEnvelope,
} from "@/tauri";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import { buildReadModelErrorEnvelope } from "@/ui/journeys/readOnly/readOnlySpineDataSource";

export type SettingsPrivacyDiagnostic = {
  id:
    | "sanitized_config"
    | "provider_privacy_boundary"
    | "life_state_projection"
    | "review_item_resolution";
  status: "loaded" | "failed" | "not_requested" | "missing";
  message?: string;
};

export type SettingsPrivacySnapshot = {
  config: AppConfig | null;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  safeMode: LifeSafeModeProjection | null;
  diagnostics: SettingsPrivacyDiagnostic[];
};

export type SettingsConnectionTestOutcome = {
  result: LlmConnectionTestResult;
  reviewItem: ReviewItem | null;
  reviewResolution: "not_requested" | "resolved" | "missing" | "ambiguous" | "unavailable";
  reviewResolutionMessage?: string;
};

export interface SettingsPrivacyDataSource {
  loadSettingsPrivacy(): Promise<SettingsPrivacySnapshot>;
  testProviderConnection(config: AppConfig): Promise<SettingsConnectionTestOutcome>;
  saveSettings(config: AppConfig): Promise<void>;
  recoverRequiredCredentialAccess(): Promise<CredentialRecoveryReport>;
}

function boundaryErrorEnvelope(message: string): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return buildReadModelErrorEnvelope(
    "ProviderPrivacyBoundarySummary",
    "provider_privacy_boundary.load_failed",
    `ProviderPrivacyBoundarySummary could not be loaded: ${message}`
  );
}

export function buildSettingsPrivacyErrorSnapshot(error: unknown): SettingsPrivacySnapshot {
  const message = errorText(error);
  return {
    config: null,
    boundaryEnvelope: boundaryErrorEnvelope(message),
    safeMode: null,
    diagnostics: [
      { id: "sanitized_config", status: "failed", message },
      { id: "provider_privacy_boundary", status: "failed", message },
      { id: "life_state_projection", status: "failed", message },
      { id: "review_item_resolution", status: "not_requested" },
    ],
  };
}

async function loadSettingsPrivacy(): Promise<SettingsPrivacySnapshot> {
  const [configResult, boundaryResult, projectionResult] = await Promise.allSettled([
    getConfig(),
    getProviderPrivacyBoundarySummary(),
    getLifeStateProjection(),
  ]);
  const configError = configResult.status === "rejected" ? errorText(configResult.reason) : null;
  const boundaryError =
    boundaryResult.status === "rejected" ? errorText(boundaryResult.reason) : null;
  const projectionError =
    projectionResult.status === "rejected" ? errorText(projectionResult.reason) : null;

  return {
    config: configResult.status === "fulfilled" ? configResult.value : null,
    boundaryEnvelope:
      boundaryResult.status === "fulfilled"
        ? boundaryResult.value
        : boundaryErrorEnvelope(boundaryError ?? "unknown_error"),
    safeMode: projectionResult.status === "fulfilled" ? projectionResult.value.safeMode : null,
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

export const tauriSettingsPrivacyDataSource: SettingsPrivacyDataSource = {
  loadSettingsPrivacy,
  testProviderConnection,
  saveSettings: saveConfig,
  recoverRequiredCredentialAccess,
};
