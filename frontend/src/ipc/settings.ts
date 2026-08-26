import type {
  AppConfig,
  ArtifactOutputDirectorySelection,
  CredentialRecoveryReport,
  LifeStateProjection,
  LlmConnectionTestResult,
  ProductDiagnosticsViewModel,
  ProviderConnectionsViewModel,
  ProviderPrivacyBoundarySummary,
  SaveProviderConnectionInput,
  ToolPermissionViewModel,
  ViewModelEnvelope,
} from "../tauri";
import { safeInvoke } from "./invoke";

export async function getConfig(): Promise<AppConfig> {
  return safeInvoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return safeInvoke("save_config", { config });
}

export async function getProviderConnections(): Promise<ProviderConnectionsViewModel> {
  return safeInvoke<ProviderConnectionsViewModel>("get_provider_connections");
}

export async function saveProviderConnection(
  input: SaveProviderConnectionInput
): Promise<ProviderConnectionsViewModel> {
  return safeInvoke<ProviderConnectionsViewModel>("save_provider_connection", { input });
}

export async function deleteProviderConnection(
  connectionId: string
): Promise<ProviderConnectionsViewModel> {
  return safeInvoke<ProviderConnectionsViewModel>("delete_provider_connection", {
    connectionId,
  });
}

export async function testProviderConnection(
  connectionId: string,
  profileId: string
): Promise<LlmConnectionTestResult> {
  return safeInvoke<LlmConnectionTestResult>("test_provider_connection", {
    connectionId,
    profileId,
  });
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

export async function getToolPermissionViewModel(): Promise<
  ViewModelEnvelope<ToolPermissionViewModel>
> {
  return safeInvoke<ViewModelEnvelope<ToolPermissionViewModel>>("get_tool_permission_view_model");
}

export async function revokeToolPermission(permissionId: string): Promise<void> {
  return safeInvoke("revoke_tool_permission", { permissionId });
}

export async function getProviderPrivacyBoundarySummary(
  conversationId?: string,
  turnId?: string
): Promise<ViewModelEnvelope<ProviderPrivacyBoundarySummary>> {
  return safeInvoke<ViewModelEnvelope<ProviderPrivacyBoundarySummary>>(
    "get_provider_privacy_boundary_summary",
    { conversationId: conversationId ?? null, turnId: turnId ?? null }
  );
}
