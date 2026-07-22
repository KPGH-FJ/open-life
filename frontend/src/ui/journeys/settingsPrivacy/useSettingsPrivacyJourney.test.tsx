import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  AppConfig,
  LlmConnectionTestResult,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import type {
  SettingsPrivacyDataSource,
  SettingsPrivacySnapshot,
} from "./settingsPrivacyDataSource";
import { useSettingsPrivacyJourney } from "./useSettingsPrivacyJourney";

function config(): AppConfig {
  return {
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
}

function boundary(
  overrides: Partial<ProviderPrivacyBoundarySummary> = {}
): ProviderPrivacyBoundarySummary {
  return {
    routeType: "cloud",
    externalTransmission: "possible",
    providerLabel: "DeepSeek",
    modelLabel: "deepseek-chat",
    privacyLabel: "可能发送到外部",
    risk: "medium",
    localOnlyRequired: false,
    evidenceRefs: [],
    ...overrides,
  };
}

function snapshot(
  currentConfig: AppConfig,
  currentBoundary: ProviderPrivacyBoundarySummary
): SettingsPrivacySnapshot {
  const envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary> = {
    data: currentBoundary,
    status: "ready",
    lastUpdatedAt: "2026-07-21T00:00:00Z",
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
  return {
    config: currentConfig,
    boundaryEnvelope: envelope,
    safeMode: { active: false, reason: "", sourceRefs: [] },
    diagnostics: [
      { id: "sanitized_config", status: "loaded" },
      { id: "provider_privacy_boundary", status: "loaded" },
      { id: "life_state_projection", status: "loaded" },
      { id: "review_item_resolution", status: "not_requested" },
    ],
  };
}

const verifiedResult: LlmConnectionTestResult = {
  ok: true,
  provider: "deepseek",
  message: "validated",
  validation_status: "validated",
  provider_invocation_receipt: {
    request_id: "request-real",
    provider: "deepseek",
    model: "deepseek-chat-v2",
    status: "completed",
    started_at: "2026-07-21T00:00:00Z",
    finished_at: "2026-07-21T00:00:01Z",
    simulated: false,
  },
};

describe("settings privacy journey", () => {
  it("tests without saving and requires an explicit external-transmission confirmation", async () => {
    const saveSettings = vi.fn();
    const testProviderConnection = vi.fn().mockResolvedValue({
      result: verifiedResult,
      reviewItem: null,
      reviewResolution: "not_requested",
    });
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection,
      saveSettings,
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.edit({ field: "chat_model", value: "deepseek-chat-v2" }));
    expect(result.current.effectiveBoundaryEnvelope.data?.routeType).toBe("unknown");

    act(() => result.current.requestTest());
    expect(result.current.testConfirmationOpen).toBe(true);
    expect(testProviderConnection).not.toHaveBeenCalled();

    act(() => result.current.confirmTest());
    await waitFor(() => expect(result.current.state.phase).toBe("tested"));

    expect(testProviderConnection).toHaveBeenCalledTimes(1);
    expect(saveSettings).not.toHaveBeenCalled();
    expect(result.current.testPresentation).toMatchObject({
      status: "success",
      verified: true,
    });
  });

  it("keeps save unknown until the post-save boundary read succeeds", async () => {
    const original = config();
    const edited = { ...original, llm: { ...original.llm, chat_model: "deepseek-chat-v2" } };
    const loadSettingsPrivacy = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(
        snapshot(
          edited,
          boundary({ routeType: "unknown", externalTransmission: "unknown", risk: "unknown" })
        )
      );
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy,
      testProviderConnection: vi.fn(),
      saveSettings,
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });
    act(() => result.current.edit({ field: "chat_model", value: "deepseek-chat-v2" }));

    act(() => result.current.save());
    await waitFor(() => expect(result.current.state.phase).toBe("unknown"));

    expect(saveSettings).toHaveBeenCalledWith(edited);
    expect(loadSettingsPrivacy).toHaveBeenCalledTimes(2);
    expect(result.current.state.boundaryAppliesToSavedRevision).toBe(true);
    expect(result.current.effectiveBoundaryEnvelope.data).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      risk: "unknown",
    });
  });

  it("does not reuse a ready envelope after the saved revision fails to refresh", async () => {
    const original = config();
    const edited = { ...original, llm: { ...original.llm, chat_model: "deepseek-chat-v2" } };
    const failedRefresh = snapshot(edited, boundary());
    failedRefresh.config = null;
    const loadSettingsPrivacy = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(failedRefresh);
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn().mockResolvedValue(undefined),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });
    act(() => result.current.edit({ field: "chat_model", value: "deepseek-chat-v2" }));

    act(() => result.current.save());
    await waitFor(() => expect(result.current.state.failureStage).toBe("boundary_refresh"));

    expect(result.current.state.boundaryAppliesToSavedRevision).toBe(false);
    expect(result.current.effectiveBoundaryEnvelope.data).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      risk: "unknown",
    });
    expect(result.current.effectiveBoundaryEnvelope.data?.blockedReason).toContain(
      "保存后的配置或模型传输边界没有完成核对"
    );
  });

  it("clears a masked credential when the provider identity changes", async () => {
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.edit({ field: "provider", value: "openai" }));

    expect(result.current.draft?.llm.openai_key).toBe("");
    expect(result.current.actions.test.enabled).toBe(false);
  });

  it("preserves masked credential semantics for the same normalized destination", async () => {
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.edit({ field: "endpoint", value: "https://api.deepseek.com/" }));
    expect(result.current.draft?.llm.openai_key).toBe("***");

    act(() => result.current.edit({ field: "credential", value: "replacement-key" }));
    act(() => result.current.edit({ field: "credential", value: "" }));

    expect(result.current.draft?.llm.openai_key).toBe("***");
    expect(result.current.actions.test.enabled).toBe(true);

    act(() => result.current.edit({ field: "provider", value: "openai" }));
    expect(result.current.draft?.llm.openai_key).toBe("");
    act(() => result.current.edit({ field: "provider", value: "deepseek" }));
    expect(result.current.draft?.llm.openai_key).toBe("***");
  });
});
