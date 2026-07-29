import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig, LlmConnectionTestResult, ReviewItem } from "@/tauri";

const tauriMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getLifeStateProjection: vi.fn(),
  getProviderPrivacyBoundarySummary: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
  recoverRequiredCredentialAccess: vi.fn(),
  saveConfig: vi.fn(),
  testLlmConnection: vi.fn(),
}));

vi.mock("@/tauri", () => tauriMocks);

import { tauriSettingsPrivacyDataSource } from "./settingsPrivacyDataSource";

const config: AppConfig = {
  llm: {
    provider: "deepseek",
    openai_base: "https://api.deepseek.com",
    openai_key: "***",
    embedding_model: "text-embedding",
    chat_model: "deepseek-chat",
  },
  prefer_local_model: false,
  local_model: "qwen2.5:14b",
};

describe("Tauri settings privacy data source", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.getLifeStateProjection.mockResolvedValue({
      safeMode: { active: false, reason: "", sourceRefs: [] },
    });
  });

  it("loads sanitized config and boundary independently and fails closed on a partial read", async () => {
    tauriMocks.getConfig.mockResolvedValue(config);
    tauriMocks.getProviderPrivacyBoundarySummary.mockRejectedValue(
      new Error("boundary unavailable")
    );

    const snapshot = await tauriSettingsPrivacyDataSource.loadSettingsPrivacy();

    expect(snapshot.config).toEqual(config);
    expect(snapshot.boundaryEnvelope).toMatchObject({ status: "error", data: null });
    expect(snapshot.safeMode).toEqual({ active: false, reason: "", sourceRefs: [] });
    expect(tauriMocks.recoverRequiredCredentialAccess).not.toHaveBeenCalled();
    expect(snapshot.diagnostics).toContainEqual({
      id: "provider_privacy_boundary",
      status: "failed",
      message: "boundary unavailable",
    });
  });

  it("invokes the sole backend credential initializer only from an explicit action", async () => {
    const report = {
      items: [],
      initializationCompletedForRestart: true,
      restartRequired: true,
      cleanupStatus: "not_required",
      bootstrapSnapshotDigest: "a".repeat(64),
    };
    tauriMocks.recoverRequiredCredentialAccess.mockResolvedValue(report);

    await expect(tauriSettingsPrivacyDataSource.initializeRequiredCredentials?.()).resolves.toEqual(
      report
    );
    expect(tauriMocks.recoverRequiredCredentialAccess).toHaveBeenCalledTimes(1);
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

    const snapshot = await tauriSettingsPrivacyDataSource.loadSettingsPrivacy();

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
    tauriMocks.testLlmConnection.mockResolvedValue(testResult);
    tauriMocks.getReviewCenterViewModel.mockResolvedValue({
      data: { items: [other, exact] },
      status: "ready",
    });

    const outcome = await tauriSettingsPrivacyDataSource.testProviderConnection(config);

    expect(outcome.reviewResolution).toBe("resolved");
    expect(outcome.reviewItem).toBe(exact);
    expect(tauriMocks.getReviewCenterViewModel).toHaveBeenCalledTimes(1);
  });

  it("does not guess a review target when the exact proposal is missing", async () => {
    tauriMocks.testLlmConnection.mockResolvedValue({
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

    const outcome = await tauriSettingsPrivacyDataSource.testProviderConnection(config);

    expect(outcome).toMatchObject({ reviewItem: null, reviewResolution: "missing" });
  });

  it("fails closed when more than one ReviewItem references the proposal", async () => {
    tauriMocks.testLlmConnection.mockResolvedValue({
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

    const outcome = await tauriSettingsPrivacyDataSource.testProviderConnection(config);

    expect(outcome).toMatchObject({ reviewItem: null, reviewResolution: "ambiguous" });
  });
});
