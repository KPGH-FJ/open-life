import type {
  AppConfig,
  ArtifactOutputDirectorySelection,
  CredentialRecoveryReport,
  LifeStateProjection,
  LlmConnectionTestResult,
  ProductDiagnosticsViewModel,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "../tauri";
import { safeInvoke } from "./invoke";

export async function getConfig(): Promise<AppConfig> {
  return safeInvoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return safeInvoke("save_config", { config });
}

export async function selectArtifactOutputDirectory(): Promise<ArtifactOutputDirectorySelection> {
  return safeInvoke<ArtifactOutputDirectorySelection>("select_artifact_output_directory");
}

export async function recoverRequiredCredentialAccess(): Promise<CredentialRecoveryReport> {
  return safeInvoke<CredentialRecoveryReport>("recover_required_credential_access");
}

export async function getProductDiagnosticsViewModel(): Promise<ProductDiagnosticsViewModel> {
  return safeInvoke<ProductDiagnosticsViewModel>("get_product_diagnostics_view_model");
}

export async function getLifeStateProjection(): Promise<LifeStateProjection> {
  return safeInvoke<LifeStateProjection>("get_life_state_projection");
}

export async function testLlmConnection(config: AppConfig): Promise<LlmConnectionTestResult> {
  return safeInvoke<LlmConnectionTestResult>("test_llm_connection", { config });
}

export async function getProviderPrivacyBoundarySummary(): Promise<
  ViewModelEnvelope<ProviderPrivacyBoundarySummary>
> {
  return safeInvoke<ViewModelEnvelope<ProviderPrivacyBoundarySummary>>(
    "get_provider_privacy_boundary_summary"
  );
}
