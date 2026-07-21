import { describe, expect, it } from "vitest";
import type { AppConfig, LlmConnectionTestResult } from "@/tauri";
import {
  connectionTestPresentation,
  credentialState,
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
    expect(actions.recovery).toMatchObject({
      id: "settings.safe_mode.recover_required_credential_access",
      kind: "retry",
      enabled: false,
      disabledReason: "后端没有证明当前处于安全模式。",
      targetRef: "credential-store:required-integrity-keys",
    });
  });

  it("keeps credential recovery contract-shaped and mutually exclusive with settings actions", () => {
    const actions = settingsProductActions(
      { ...initialSettingsOrchestrationState, phase: "dirty", draftRevision: 1 },
      validateSettingsDraft(config()),
      { safeModeActive: true, phase: "confirming", readyForRestart: false }
    );

    expect(actions.recovery).toMatchObject({ enabled: false, kind: "retry" });
    expect(actions.test).toMatchObject({
      enabled: false,
      disabledReason: "系统凭据检查正在进行。",
    });
    expect(actions.save).toMatchObject({
      enabled: false,
      disabledReason: "系统凭据检查正在进行。",
    });

    const restartActions = settingsProductActions(
      initialSettingsOrchestrationState,
      validateSettingsDraft(config()),
      { safeModeActive: true, phase: "complete", readyForRestart: true }
    );
    expect(restartActions.recovery).toMatchObject({
      enabled: false,
      disabledReason: "本次凭据检查已完成；请完全退出并重启 OpenLife 后重新核对。",
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
});
