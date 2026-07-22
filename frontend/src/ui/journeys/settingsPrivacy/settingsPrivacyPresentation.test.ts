import { describe, expect, it } from "vitest";
import type { AppConfig, LlmConnectionTestResult } from "@/tauri";
import {
  connectionTestPresentation,
  credentialState,
  settingsConfigMatchesSavedDraft,
  settingsProductActions,
  validateSettingsDraft,
} from "./settingsPrivacyPresentation";
import { initialSettingsOrchestrationState } from "@/contracts/settingsOrchestrationContract";

function config(overrides: Partial<AppConfig["llm"]> = {}): AppConfig {
  return {
    llm: {
      provider: "deepseek",
      openai_base: "https://api.deepseek.com",
      openai_key: "***",
      embedding_model: "text-embedding",
      chat_model: "deepseek-chat",
      ...overrides,
    },
    prefer_local_model: false,
    local_model: "qwen2.5:14b",
  };
}

function result(overrides: Partial<LlmConnectionTestResult>): LlmConnectionTestResult {
  return {
    ok: false,
    provider: "deepseek",
    message: "not verified",
    validation_status: "blocked",
    ...overrides,
  };
}

describe("settings privacy presentation", () => {
  it("requires an exact, completed, non-simulated provider receipt for success", () => {
    const incomplete = connectionTestPresentation(
      result({ ok: true, validation_status: "validated" })
    );
    const simulated = connectionTestPresentation(
      result({
        ok: true,
        validation_status: "validated",
        provider_invocation_receipt: {
          request_id: "request-simulated",
          provider: "deepseek",
          model: "deepseek-chat",
          status: "completed",
          started_at: "2026-07-21T00:00:00Z",
          finished_at: "2026-07-21T00:00:01Z",
          simulated: true,
        },
      })
    );
    const verified = connectionTestPresentation(
      result({
        ok: true,
        validation_status: "validated",
        provider_invocation_receipt: {
          request_id: "request-real",
          provider: "deepseek",
          model: "deepseek-chat",
          status: "completed",
          started_at: "2026-07-21T00:00:00Z",
          finished_at: "2026-07-21T00:00:01Z",
          simulated: false,
        },
      })
    );

    expect(incomplete).toMatchObject({ status: "unknown" });
    expect(incomplete?.verified).toBeUndefined();
    expect(simulated).toMatchObject({ status: "unknown" });
    expect(simulated?.verified).toBeUndefined();
    expect(verified).toMatchObject({ status: "success", verified: true });
  });

  it("keeps consent-required and remote-unknown results fail-closed", () => {
    expect(
      connectionTestPresentation(
        result({ validation_status: "consent_required", review_proposal_id: "proposal-1" })
      )
    ).toMatchObject({ status: "waiting" });
    expect(
      connectionTestPresentation(result({ validation_status: "consent_required" }))?.verified
    ).toBeUndefined();
    const remoteUnknown = connectionTestPresentation(
      result({ validation_status: "remote_unknown" })
    );
    expect(remoteUnknown).toMatchObject({ status: "unknown" });
    expect(remoteUnknown?.verified).toBeUndefined();
  });

  it("keeps test and save actions separate and contract-shaped", () => {
    const validation = validateSettingsDraft(config());
    const dirtyState = {
      ...initialSettingsOrchestrationState,
      phase: "dirty" as const,
      draftRevision: 1,
    };
    const actions = settingsProductActions(dirtyState, validation);

    expect(actions.test).toMatchObject({
      id: "settings.provider.test_connection",
      kind: "configure",
      enabled: true,
      targetRef: "settings-draft:1",
    });
    expect(actions.save).toMatchObject({
      id: "settings.provider.save_config",
      kind: "configure",
      enabled: true,
      targetRef: "AppConfig",
    });
  });

  it("does not allow testing without a credential", () => {
    const validation = validateSettingsDraft(config({ openai_key: "" }));

    expect(validation.canSave).toBe(true);
    expect(validation.canTest).toBe(false);
    expect(validation.testDisabledReason).toContain("API 凭据");
  });

  it("accepts the real sanitized config shape without exposing a secret", () => {
    const sanitized = config({
      openai_key: undefined,
      openai_key_ref: "keychain://com.openlife.desktop/provider-api-key",
    });

    expect(credentialState(sanitized)).toBe("stored");
    expect(validateSettingsDraft(sanitized).canTest).toBe(true);
  });

  it("attests the refreshed sanitized config without comparing secret material", () => {
    const previous = config({ openai_key: "***", credential_version: 7 });
    const submitted = config({ openai_key: "replacement-secret", credential_version: 7 });
    const refreshed = config({ openai_key: "***", credential_version: 8 });

    expect(settingsConfigMatchesSavedDraft(previous, submitted, refreshed)).toBe(true);
    expect(
      settingsConfigMatchesSavedDraft(previous, submitted, {
        ...refreshed,
        llm: { ...refreshed.llm, chat_model: "different-model" },
      })
    ).toBe(false);
    expect(
      settingsConfigMatchesSavedDraft(previous, submitted, {
        ...refreshed,
        llm: {
          ...refreshed.llm,
          openai_key: undefined,
          openai_key_ref: undefined,
          credential_version: 7,
        },
      })
    ).toBe(false);

    const storedSubmission = config({ openai_key: "***", credential_version: 7 });
    expect(
      settingsConfigMatchesSavedDraft(
        previous,
        storedSubmission,
        config({ openai_key: "***", credential_version: 7 })
      )
    ).toBe(true);
    expect(
      settingsConfigMatchesSavedDraft(
        previous,
        storedSubmission,
        config({ openai_key: "***", credential_version: 8 })
      )
    ).toBe(false);
    expect(
      settingsConfigMatchesSavedDraft(
        config({ credential_version: undefined }),
        config({ credential_version: undefined }),
        config({ credential_version: 0 })
      )
    ).toBe(false);
  });
});
