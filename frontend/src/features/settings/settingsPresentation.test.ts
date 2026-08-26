import { describe, expect, it } from "vitest";
import type { AppConfig } from "@/tauri";
import {
  selectedHostedSearchRoute,
  settingsConfigMatchesSavedDraft,
  settingsProductActions,
  validateSettingsDraft,
} from "./settingsPresentation";
import { initialSettingsOrchestrationState } from "@/contracts/settingsOrchestrationContract";

function config(): AppConfig {
  return {
    prefer_local_model: false,
    local_model: "qwen2.5:14b",
  };
}

describe("settings privacy presentation", () => {
  it("keeps AppConfig save separate from provider Connection lifecycle actions", () => {
    const validation = validateSettingsDraft(config());
    const dirtyState = {
      ...initialSettingsOrchestrationState,
      phase: "dirty" as const,
      draftRevision: 1,
    };
    const actions = settingsProductActions(dirtyState, validation);

    expect(actions.save).toMatchObject({
      id: "settings.provider.save_config",
      kind: "configure",
      enabled: true,
      targetRef: "AppConfig",
    });
  });

  it("accepts the editable config without a Provider credential surface", () => {
    expect(validateSettingsDraft(config()).canSave).toBe(true);
  });

  it("reuses only a ready selected persistent Connection on an official hosted-search route", () => {
    const deepseekSearch = {
      ...config(),
      system: { search_provider: "deepseek" as const, search_provider_key: "" },
    };
    const connections = {
      connections: [
        {
          id: "connection-1",
          providerId: "deepseek" as const,
          displayName: "DeepSeek",
          endpoint: "https://api.deepseek.com",
          credentialState: "stored" as const,
          validationState: "ready",
          models: [
            {
              profileId: "profile-1",
              modelId: "deepseek-chat",
              displayName: "DeepSeek Chat",
              selected: true,
              validationState: "ready",
            },
          ],
        },
      ],
    };
    expect(selectedHostedSearchRoute(connections, "deepseek")?.id).toBe("connection-1");
    expect(validateSettingsDraft(deepseekSearch, connections).canSave).toBe(true);

    const customGateway = {
      connections: [
        {
          ...connections.connections[0],
          endpoint: "https://deepseek-proxy.example.com",
        },
      ],
    };
    expect(selectedHostedSearchRoute(customGateway, "deepseek")).toBeNull();
    expect(validateSettingsDraft(deepseekSearch, customGateway)).toMatchObject({
      canSave: false,
      saveDisabledReason: expect.stringContaining("搜索凭据"),
    });
  });

  it("attests an independent search credential without comparing secret material", () => {
    const previous = {
      ...config(),
      system: { search_provider: "brave" as const, search_provider_key: "***" },
    };
    const submitted = {
      ...previous,
      system: { ...previous.system, search_provider_key: "replacement-search-secret" },
    };
    const refreshed = {
      ...previous,
      system: {
        search_provider: "brave" as const,
        search_provider_key: "***",
        search_provider_key_ref: "keychain://com.openlife.desktop/search-provider-api-key",
      },
    };

    expect(settingsConfigMatchesSavedDraft(previous, submitted, refreshed)).toBe(true);
    expect(
      settingsConfigMatchesSavedDraft(previous, submitted, {
        ...refreshed,
        system: { search_provider: "brave", search_provider_key: "" },
      })
    ).toBe(false);
  });

  it("attests only the editable AppConfig fields", () => {
    const previous = config();
    const submitted = config();
    const refreshed = config();

    expect(settingsConfigMatchesSavedDraft(previous, submitted, refreshed)).toBe(true);
    expect(
      settingsConfigMatchesSavedDraft(previous, submitted, {
        ...refreshed,
        local_model: "different-local-model",
      })
    ).toBe(false);
  });
});
