import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  AppConfig,
  LlmConnectionTestResult,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import type {
  SettingsConnectionTestOutcome,
  SettingsPrivacyDataSource,
  SettingsPrivacySnapshot,
} from "./settingsPrivacyDataSource";
import { settingsPrivacyContext, settingsPrivacyInspector } from "./settingsPrivacyShellModel";
import { useSettingsPrivacyJourney } from "./useSettingsPrivacyJourney";

function config(): AppConfig {
  return {
    llm: {
      provider: "deepseek",
      openai_base: "https://api.deepseek.com",
      openai_key: "***",
      credential_version: 7,
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
  it("deduplicates concurrent cold-entry loads", async () => {
    const initialSnapshot = snapshot(config(), boundary());
    let finishLoad: (value: SettingsPrivacySnapshot) => void = () => undefined;
    const delayedSnapshot = new Promise<SettingsPrivacySnapshot>(resolve => {
      finishLoad = resolve;
    });
    const loadSettingsPrivacy = vi.fn(() => delayedSnapshot);
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));

    let firstLoad: ReturnType<typeof result.current.ensureLoaded> | undefined;
    let secondLoad: ReturnType<typeof result.current.ensureLoaded> | undefined;
    act(() => {
      firstLoad = result.current.ensureLoaded();
      secondLoad = result.current.ensureLoaded();
    });
    expect(loadSettingsPrivacy).toHaveBeenCalledTimes(1);

    await act(async () => {
      finishLoad(initialSnapshot);
      await Promise.all([firstLoad, secondLoad]);
    });
    expect(result.current.snapshot).toBe(initialSnapshot);
  });

  it("waits for an explicit in-flight reload instead of reusing its old snapshot", async () => {
    const originalSnapshot = snapshot(config(), boundary());
    const refreshedConfig = {
      ...config(),
      llm: { ...config().llm, chat_model: "refreshed-model" },
    };
    const refreshedSnapshot = snapshot(refreshedConfig, boundary());
    let finishReload: (value: SettingsPrivacySnapshot) => void = () => undefined;
    const delayedReload = new Promise<SettingsPrivacySnapshot>(resolve => {
      finishReload = resolve;
    });
    const loadSettingsPrivacy = vi
      .fn()
      .mockResolvedValueOnce(originalSnapshot)
      .mockImplementationOnce(() => delayedReload);
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    let reloadPromise: Promise<SettingsPrivacySnapshot> | undefined;
    let ensurePromise: ReturnType<typeof result.current.ensureLoaded> | undefined;
    act(() => {
      reloadPromise = result.current.load(false);
      ensurePromise = result.current.ensureLoaded();
    });
    expect(loadSettingsPrivacy).toHaveBeenCalledTimes(2);

    let ensureSettled = false;
    void ensurePromise?.then(() => {
      ensureSettled = true;
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(ensureSettled).toBe(false);

    await act(async () => {
      finishReload(refreshedSnapshot);
      await Promise.all([reloadPromise, ensurePromise]);
    });
    expect(await ensurePromise).toMatchObject({
      snapshot: refreshedSnapshot,
      loadedFromSource: true,
    });
    expect(result.current.draft?.llm.chat_model).toBe("refreshed-model");
  });

  it("invalidates a cached snapshot when the data source changes", async () => {
    const firstConfig = config();
    const secondConfig = {
      ...firstConfig,
      llm: { ...firstConfig.llm, chat_model: "replacement-source-model" },
    };
    const firstLoad = vi.fn().mockResolvedValue(snapshot(firstConfig, boundary()));
    const secondLoad = vi.fn().mockResolvedValue(snapshot(secondConfig, boundary()));
    const firstSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const secondSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: secondLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsPrivacyDataSource }) =>
        useSettingsPrivacyJourney(source, vi.fn()),
      { initialProps: { source: firstSource } }
    );

    await act(async () => {
      await result.current.ensureLoaded();
    });
    expect(result.current.draft?.llm.chat_model).toBe("deepseek-chat");

    rerender({ source: secondSource });
    await act(async () => {
      await result.current.ensureLoaded();
    });

    expect(firstLoad).toHaveBeenCalledTimes(1);
    expect(secondLoad).toHaveBeenCalledTimes(1);
    expect(result.current.draft?.llm.chat_model).toBe("replacement-source-model");
  });

  it("ignores an in-flight connection test from a replaced data source", async () => {
    let finishTest: (value: SettingsConnectionTestOutcome) => void = () => undefined;
    const delayedTest = new Promise<SettingsConnectionTestOutcome>(resolve => {
      finishTest = resolve;
    });
    const firstSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(() => delayedTest),
      saveSettings: vi.fn(),
    };
    const replacementConfig = {
      ...config(),
      llm: { ...config().llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsPrivacyDataSource }) =>
        useSettingsPrivacyJourney(source, vi.fn()),
      { initialProps: { source: firstSource } }
    );
    await act(async () => {
      await result.current.ensureLoaded();
    });
    act(() => result.current.edit({ field: "chat_model", value: "first-source-test" }));
    act(() => result.current.requestTest());
    act(() => result.current.confirmTest());
    expect(firstSource.testProviderConnection).toHaveBeenCalledTimes(1);

    rerender({ source: secondSource });
    await act(async () => {
      await result.current.ensureLoaded();
    });
    await act(async () => {
      finishTest({ result: verifiedResult, reviewItem: null, reviewResolution: "not_requested" });
      await delayedTest;
    });

    expect(result.current.draft?.llm.chat_model).toBe("replacement-source-model");
    expect(result.current.lastTestOutcome).toBeNull();
    expect(result.current.state.phase).toBe("idle");
  });

  it("does not let a stale save clear a replacement-source operation lock", async () => {
    let finishSave: () => void = () => undefined;
    const delayedSave = new Promise<void>(resolve => {
      finishSave = resolve;
    });
    let finishReplacementTest: (value: SettingsConnectionTestOutcome) => void = () => undefined;
    const delayedReplacementTest = new Promise<SettingsConnectionTestOutcome>(resolve => {
      finishReplacementTest = resolve;
    });
    const firstLoad = vi.fn().mockResolvedValue(snapshot(config(), boundary()));
    const firstSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(() => delayedSave),
    };
    const replacementConfig = {
      ...config(),
      llm: { ...config().llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(() => delayedReplacementTest),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsPrivacyDataSource }) =>
        useSettingsPrivacyJourney(source, announce),
      { initialProps: { source: firstSource } }
    );
    await act(async () => {
      await result.current.ensureLoaded();
    });
    act(() => result.current.edit({ field: "chat_model", value: "first-source-save" }));
    act(() => result.current.save());
    expect(firstSource.saveSettings).toHaveBeenCalledTimes(1);

    rerender({ source: secondSource });
    await act(async () => {
      await result.current.ensureLoaded();
    });
    act(() => result.current.edit({ field: "chat_model", value: "replacement-source-test" }));
    act(() => result.current.requestTest());
    act(() => result.current.confirmTest());
    expect(secondSource.testProviderConnection).toHaveBeenCalledTimes(1);

    await act(async () => {
      finishSave();
      await delayedSave;
    });
    expect(firstLoad).toHaveBeenCalledTimes(1);
    expect(result.current.state.phase).toBe("testing");
    act(() => result.current.edit({ field: "chat_model", value: "must-not-apply" }));
    expect(result.current.draft?.llm.chat_model).toBe("replacement-source-test");
    expect(announce).toHaveBeenLastCalledWith(
      "当前不能修改设置；请等待后端配置读取或当前操作结束。"
    );

    await act(async () => {
      finishReplacementTest({
        result: verifiedResult,
        reviewItem: null,
        reviewResolution: "not_requested",
      });
      await delayedReplacementTest;
    });
    await waitFor(() => expect(result.current.state.phase).toBe("tested"));
  });

  it("ignores an in-flight boundary retry from a replaced data source", async () => {
    const original = config();
    const saved = { ...original, llm: { ...original.llm, chat_model: "saved-on-first-source" } };
    const failedRefresh = snapshot(saved, boundary());
    failedRefresh.config = null;
    let finishRetry: (value: SettingsPrivacySnapshot) => void = () => undefined;
    const delayedRetry = new Promise<SettingsPrivacySnapshot>(resolve => {
      finishRetry = resolve;
    });
    const firstLoad = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(failedRefresh)
      .mockImplementationOnce(() => delayedRetry);
    const firstSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn().mockResolvedValue(undefined),
    };
    const replacementConfig = {
      ...original,
      llm: { ...original.llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsPrivacyDataSource }) =>
        useSettingsPrivacyJourney(source, announce),
      { initialProps: { source: firstSource } }
    );
    await act(async () => {
      await result.current.ensureLoaded();
    });
    act(() => result.current.edit({ field: "chat_model", value: "saved-on-first-source" }));
    act(() => result.current.save());
    await waitFor(() => expect(result.current.state.failureStage).toBe("boundary_refresh"));
    act(() => result.current.retryBoundaryRefresh());
    expect(firstLoad).toHaveBeenCalledTimes(3);

    rerender({ source: secondSource });
    await act(async () => {
      await result.current.ensureLoaded();
    });
    await act(async () => {
      finishRetry(snapshot(saved, boundary()));
      await delayedRetry;
    });

    expect(result.current.draft?.llm.chat_model).toBe("replacement-source-model");
    expect(result.current.state.phase).toBe("idle");
    expect(announce).not.toHaveBeenCalledWith("已重新确认精确的已保存配置与模型传输边界。");
  });

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
      .mockResolvedValueOnce(failedRefresh)
      .mockResolvedValueOnce(snapshot(edited, boundary()));
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

    act(() => result.current.retryBoundaryRefresh());
    await waitFor(() => expect(result.current.state.phase).toBe("ready"));

    expect(loadSettingsPrivacy).toHaveBeenCalledTimes(3);
    expect(result.current.state.boundaryAppliesToSavedRevision).toBe(true);
    expect(result.current.state.failureStage).toBeNull();
  });

  it("keeps boundary and settings actions closed when LifeStateProjection is missing", async () => {
    const missingProjection = snapshot(config(), boundary({ routeType: "local" }));
    missingProjection.safeMode = null;
    missingProjection.diagnostics = missingProjection.diagnostics.map(diagnostic =>
      diagnostic.id === "life_state_projection"
        ? { ...diagnostic, status: "failed", message: "projection unavailable" }
        : diagnostic
    );
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(missingProjection),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));

    await act(async () => {
      await result.current.load(false);
    });

    expect(result.current.protectionState).toBe("unknown");
    expect(result.current.actions.test).toMatchObject({ enabled: false });
    expect(result.current.actions.test.disabledReason).toContain("LifeStateProjection");
    expect(result.current.actions.save).toMatchObject({ enabled: false });
    expect(result.current.effectiveBoundaryEnvelope.data).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      risk: "unknown",
    });
    expect(result.current.effectiveBoundaryEnvelope.data?.blockedReason).toContain(
      "LifeStateProjection"
    );
  });

  it("keeps a ready config closed while backend Safe Mode is active", async () => {
    const safeModeSnapshot = snapshot(config(), boundary({ routeType: "local" }));
    safeModeSnapshot.safeMode = {
      active: true,
      reason: "persistence_unavailable",
      sourceRefs: ["safe-mode:persistence"],
    };
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi.fn().mockResolvedValue(safeModeSnapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, vi.fn()));

    await act(async () => {
      await result.current.load(false);
    });

    expect(result.current.protectionState).toBe("active");
    expect(result.current.actions.test.enabled).toBe(false);
    expect(result.current.actions.save.enabled).toBe(false);
    expect(result.current.effectiveBoundaryEnvelope.data).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      privacyLabel: "后端安全模式仍在生效",
      risk: "unknown",
    });
    expect(result.current.effectiveBoundaryEnvelope.warnings?.[0]?.code).toBe(
      "settings.safe_mode_active"
    );
  });

  it("locks the previous snapshot while a settings reload is in flight", async () => {
    const original = config();
    const refreshed = {
      ...original,
      llm: { ...original.llm, chat_model: "deepseek-chat-v2" },
    };
    let finishReload: (value: SettingsPrivacySnapshot) => void = () => undefined;
    const delayedReload = new Promise<SettingsPrivacySnapshot>(resolve => {
      finishReload = resolve;
    });
    const source: SettingsPrivacyDataSource = {
      loadSettingsPrivacy: vi
        .fn()
        .mockResolvedValueOnce(snapshot(original, boundary()))
        .mockImplementationOnce(() => delayedReload),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsPrivacyJourney(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    let reloadPromise: Promise<SettingsPrivacySnapshot> | undefined;
    act(() => {
      reloadPromise = result.current.load(false);
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    expect(result.current.protectionState).toBe("loading");
    expect(result.current.actions.test.enabled).toBe(false);
    expect(result.current.actions.save.enabled).toBe(false);
    expect(result.current.effectiveBoundaryEnvelope.status).toBe("loading");
    expect(settingsPrivacyContext(result.current, "model-provider").status).toMatchObject({
      label: "正在读取",
      status: "neutral",
    });
    const loadingInspector = settingsPrivacyInspector(result.current, "model-provider", "");
    expect(loadingInspector.conclusion).toContain("旧快照不作为当前确定态");
    expect(loadingInspector.nextAction).toContain("不修改、不测试、不保存");
    act(() => result.current.edit({ field: "chat_model", value: "must-not-win" }));
    expect(result.current.draft?.llm.chat_model).toBe("deepseek-chat");
    expect(announce).toHaveBeenLastCalledWith(
      "当前不能修改设置；请等待后端配置读取或当前操作结束。"
    );

    await act(async () => {
      finishReload(snapshot(refreshed, boundary()));
      await reloadPromise;
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.draft?.llm.chat_model).toBe("deepseek-chat-v2");
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
