import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, LlmConnectionTestResult, ReviewItem } from "@/tauri";

const tauriMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getProviderConnections: vi.fn(),
  getLifeStateProjection: vi.fn(),
  getProviderPrivacyBoundarySummary: vi.fn(),
  getProductDiagnosticsViewModel: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
  getToolPermissionViewModel: vi.fn(),
  recoverRequiredCredentialAccess: vi.fn(),
  revokeToolPermission: vi.fn(),
  saveConfig: vi.fn(),
  saveProviderConnection: vi.fn(),
  deleteProviderConnection: vi.fn(),
  selectArtifactOutputDirectory: vi.fn(),
  testProviderConnection: vi.fn(),
}));

vi.mock("@/tauri", () => ({}));
vi.mock("@/ipc/review", () => ({
  getReviewCenterViewModel: tauriMocks.getReviewCenterViewModel,
}));
vi.mock("@/ipc/settings", () => ({
  deleteProviderConnection: tauriMocks.deleteProviderConnection,
  getConfig: tauriMocks.getConfig,
  getProviderConnections: tauriMocks.getProviderConnections,
  getLifeStateProjection: tauriMocks.getLifeStateProjection,
  getProductDiagnosticsViewModel: tauriMocks.getProductDiagnosticsViewModel,
  getProviderPrivacyBoundarySummary: tauriMocks.getProviderPrivacyBoundarySummary,
  getToolPermissionViewModel: tauriMocks.getToolPermissionViewModel,
  recoverRequiredCredentialAccess: tauriMocks.recoverRequiredCredentialAccess,
  revokeToolPermission: tauriMocks.revokeToolPermission,
  saveConfig: tauriMocks.saveConfig,
  saveProviderConnection: tauriMocks.saveProviderConnection,
  selectArtifactOutputDirectory: tauriMocks.selectArtifactOutputDirectory,
  testProviderConnection: tauriMocks.testProviderConnection,
}));

import { tauriSettingsDataSource } from "./settingsDataSource";

const config: AppConfig = {
  prefer_local_model: false,
  local_model: "qwen2.5:14b",
};

describe("Tauri settings privacy data source", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.getLifeStateProjection.mockResolvedValue({
      safeMode: { active: false, reason: "", sourceRefs: [] },
    });
    tauriMocks.getProductDiagnosticsViewModel.mockResolvedValue({
      status: "ready",
      stores: [],
      counts: {},
      blockerCodes: [],
    });
    tauriMocks.getToolPermissionViewModel.mockResolvedValue({
      data: {
        items: [],
        totalCount: 0,
        activeCount: 0,
        revocableCount: 0,
        contractLimitations: [],
      },
      status: "empty",
      lastUpdatedAt: "2026-08-24T00:00:00Z",
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    });
  });

  it("loads sanitized config and boundary independently and fails closed on a partial read", async () => {
    tauriMocks.getConfig.mockResolvedValue(config);
    tauriMocks.getProviderPrivacyBoundarySummary.mockRejectedValue(
      new Error("boundary unavailable")
    );

    const snapshot = await tauriSettingsDataSource.loadSettings();

    expect(snapshot.config).toEqual(config);
    expect(snapshot.boundaryEnvelope).toMatchObject({ status: "error", data: null });
    expect(snapshot.safeMode).toEqual({ active: false, reason: "", sourceRefs: [] });
    expect(snapshot.credentialBootstrap).toBeNull();
    expect(tauriMocks.recoverRequiredCredentialAccess).not.toHaveBeenCalled();
    expect(snapshot.diagnostics).toContainEqual({
      id: "provider_privacy_boundary",
      status: "failed",
      message: "boundary unavailable",
    });
    const report = {
      items: [],
      initializationCompletedForRestart: true,
      restartRequired: true,
      cleanupStatus: "not_required",
      bootstrapSnapshotDigest: "a".repeat(64),
    };
    tauriMocks.recoverRequiredCredentialAccess.mockResolvedValue(report);

    await expect(tauriSettingsDataSource.initializeRequiredCredentials?.()).resolves.toEqual(
      report
    );
    expect(tauriMocks.recoverRequiredCredentialAccess).toHaveBeenCalledTimes(1);
  });

  it("delegates artifact output selection to the native Tauri command", async () => {
    tauriMocks.selectArtifactOutputDirectory.mockResolvedValue({
      cancelled: false,
      selectedPath: "/tmp/openlife-artifacts",
    });

    await expect(tauriSettingsDataSource.selectArtifactOutputDirectory?.()).resolves.toEqual({
      cancelled: false,
      selectedPath: "/tmp/openlife-artifacts",
    });
    expect(tauriMocks.selectArtifactOutputDirectory).toHaveBeenCalledTimes(1);
  });

  it("loads and revokes only through the canonical permission commands", async () => {
    tauriMocks.getConfig.mockResolvedValue(config);
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue({
      data: null,
      status: "empty",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    });
    const snapshot = await tauriSettingsDataSource.loadSettings();
    expect(snapshot.toolPermissionEnvelope.status).toBe("empty");

    tauriMocks.revokeToolPermission.mockResolvedValue(undefined);
    await expect(
      tauriSettingsDataSource.revokeToolPermission?.("00000000-0000-4000-8000-000000000001")
    ).resolves.toBeUndefined();
    expect(tauriMocks.revokeToolPermission).toHaveBeenCalledWith(
      "00000000-0000-4000-8000-000000000001"
    );
  });

  it("keeps Safe Mode unknown when LifeStateProjection cannot be read", async () => {
    tauriMocks.getConfig.mockResolvedValue(config);
    tauriMocks.getProviderPrivacyBoundarySummary.mockResolvedValue({
      data: null,
      status: "empty",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    });
    tauriMocks.getLifeStateProjection.mockRejectedValue(new Error("projection unavailable"));

    const snapshot = await tauriSettingsDataSource.loadSettings();

    expect(snapshot.config).toEqual(config);
    expect(snapshot.safeMode).toBeNull();
    expect(snapshot.diagnostics).toContainEqual({
      id: "life_state_projection",
      status: "failed",
      message: "projection unavailable",
    });
  });

  it("resolves only the exact ReviewItem referenced by the test result", async () => {
    const testResult: LlmConnectionTestResult = {
      ok: false,
      provider: "deepseek",
      message: "consent required",
      validation_status: "consent_required",
      review_proposal_id: "proposal-exact",
    };
    const other = {
      id: "review-other",
      source: { proposalId: "proposal-other" },
    } as ReviewItem;
    const exact = {
      id: "review-exact",
      source: { proposalId: "proposal-exact" },
    } as ReviewItem;
    tauriMocks.testProviderConnection.mockResolvedValue(testResult);
    tauriMocks.getReviewCenterViewModel.mockResolvedValue({
      data: { items: [other, exact] },
      status: "ready",
    });

    const outcome = await tauriSettingsDataSource.testSavedProviderConnection?.(
      "connection-1",
      "profile-1"
    );

    expect(outcome?.reviewResolution).toBe("resolved");
    expect(outcome?.reviewItem).toBe(exact);
    expect(tauriMocks.testProviderConnection).toHaveBeenCalledWith("connection-1", "profile-1");
    expect(tauriMocks.getReviewCenterViewModel).toHaveBeenCalledTimes(1);
  });

  it("does not guess a review target when the exact proposal is missing", async () => {
    tauriMocks.testProviderConnection.mockResolvedValue({
      ok: false,
      provider: "deepseek",
      message: "consent required",
      validation_status: "consent_required",
      review_proposal_id: "proposal-missing",
    } satisfies LlmConnectionTestResult);
    tauriMocks.getReviewCenterViewModel.mockResolvedValue({
      data: { items: [] },
      status: "ready",
    });

    const outcome = await tauriSettingsDataSource.testSavedProviderConnection?.(
      "connection-1",
      "profile-1"
    );

    expect(outcome).toMatchObject({ reviewItem: null, reviewResolution: "missing" });
  });

  it("fails closed when more than one ReviewItem references the proposal", async () => {
    tauriMocks.testProviderConnection.mockResolvedValue({
      ok: false,
      provider: "deepseek",
      message: "consent required",
      validation_status: "consent_required",
      review_proposal_id: "proposal-duplicate",
    } satisfies LlmConnectionTestResult);
    tauriMocks.getReviewCenterViewModel.mockResolvedValue({
      data: {
        items: [
          { id: "review-a", source: { proposalId: "proposal-duplicate" } },
          { id: "review-b", source: { proposalId: "proposal-duplicate" } },
        ],
      },
      status: "ready",
    });

    const outcome = await tauriSettingsDataSource.testSavedProviderConnection?.(
      "connection-1",
      "profile-1"
    );

    expect(outcome).toMatchObject({ reviewItem: null, reviewResolution: "ambiguous" });
  });
});
