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
  SettingsDataSource,
  SettingsSnapshot,
} from "./settingsDataSource";
import { settingsContext, settingsInspector } from "./settingsShellModel";
import { useSettingsController } from "./useSettingsController";

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
): SettingsSnapshot {
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
    toolPermissionEnvelope: {
      data: {
        items: [],
        totalCount: 0,
        activeCount: 0,
        revocableCount: 0,
        contractLimitations: [],
      },
      status: "empty",
      lastUpdatedAt: "2026-07-21T00:00:00Z",
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    },
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

describe("settings controller", () => {
  it("deduplicates concurrent cold-entry loads", async () => {
    const initialSnapshot = snapshot(config(), boundary());
    let finishLoad: (value: SettingsSnapshot) => void = () => undefined;
    const delayedSnapshot = new Promise<SettingsSnapshot>(resolve => {
      finishLoad = resolve;
    });
    const loadSettings = vi.fn(() => delayedSnapshot);
    const source: SettingsDataSource = {
      loadSettings,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));

    let firstLoad: ReturnType<typeof result.current.ensureLoaded> | undefined;
    let secondLoad: ReturnType<typeof result.current.ensureLoaded> | undefined;
    act(() => {
      firstLoad = result.current.ensureLoaded();
      secondLoad = result.current.ensureLoaded();
    });
    expect(loadSettings).toHaveBeenCalledTimes(1);

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
    let finishReload: (value: SettingsSnapshot) => void = () => undefined;
    const delayedReload = new Promise<SettingsSnapshot>(resolve => {
      finishReload = resolve;
    });
    const loadSettings = vi
      .fn()
      .mockResolvedValueOnce(originalSnapshot)
      .mockImplementationOnce(() => delayedReload);
    const source: SettingsDataSource = {
      loadSettings,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    let reloadPromise: Promise<SettingsSnapshot> | undefined;
    let ensurePromise: ReturnType<typeof result.current.ensureLoaded> | undefined;
    act(() => {
      reloadPromise = result.current.load(false);
      ensurePromise = result.current.ensureLoaded();
    });
    expect(loadSettings).toHaveBeenCalledTimes(2);

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
    const firstSource: SettingsDataSource = {
      loadSettings: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const secondSource: SettingsDataSource = {
      loadSettings: secondLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsDataSource }) => useSettingsController(source, vi.fn()),
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
    const firstSource: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(() => delayedTest),
      saveSettings: vi.fn(),
    };
    const replacementConfig = {
      ...config(),
      llm: { ...config().llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsDataSource }) => useSettingsController(source, vi.fn()),
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
    const firstSource: SettingsDataSource = {
      loadSettings: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(() => delayedSave),
    };
    const replacementConfig = {
      ...config(),
      llm: { ...config().llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(() => delayedReplacementTest),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsDataSource }) => useSettingsController(source, announce),
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
      "当前不能修改设置；请等待系统配置读取或当前操作结束。"
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
    let finishRetry: (value: SettingsSnapshot) => void = () => undefined;
    const delayedRetry = new Promise<SettingsSnapshot>(resolve => {
      finishRetry = resolve;
    });
    const firstLoad = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(failedRefresh)
      .mockImplementationOnce(() => delayedRetry);
    const firstSource: SettingsDataSource = {
      loadSettings: firstLoad,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn().mockResolvedValue(undefined),
    };
    const replacementConfig = {
      ...original,
      llm: { ...original.llm, chat_model: "replacement-source-model" },
    };
    const secondSource: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(replacementConfig, boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result, rerender } = renderHook(
      ({ source }: { source: SettingsDataSource }) => useSettingsController(source, announce),
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
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection,
      saveSettings,
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsController(source, announce));
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

  it("does not describe an already-saved config as unsaved after a successful test", async () => {
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn().mockResolvedValue({
        result: verifiedResult,
        reviewItem: null,
        reviewResolution: "not_requested",
      }),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsController(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.requestTest());
    act(() => result.current.confirmTest());
    await waitFor(() => expect(result.current.testPresentation?.verified).toBe(true));

    expect(result.current.state.phase).toBe("idle");
    expect(announce).toHaveBeenLastCalledWith(
      "本次连接验证已有可信回执；当前已保存设置未被测试改变。"
    );
  });

  it("keeps save unknown until the post-save boundary read succeeds", async () => {
    const original = config();
    const edited = { ...original, llm: { ...original.llm, chat_model: "deepseek-chat-v2" } };
    const loadSettings = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(
        snapshot(
          edited,
          boundary({ routeType: "unknown", externalTransmission: "unknown", risk: "unknown" })
        )
      );
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const source: SettingsDataSource = {
      loadSettings,
      testProviderConnection: vi.fn(),
      saveSettings,
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });
    act(() => result.current.edit({ field: "chat_model", value: "deepseek-chat-v2" }));

    act(() => result.current.save());
    await waitFor(() => expect(result.current.state.phase).toBe("unknown"));

    expect(saveSettings).toHaveBeenCalledWith(edited);
    expect(loadSettings).toHaveBeenCalledTimes(2);
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
    const loadSettings = vi
      .fn()
      .mockResolvedValueOnce(snapshot(original, boundary()))
      .mockResolvedValueOnce(failedRefresh)
      .mockResolvedValueOnce(snapshot(edited, boundary()));
    const source: SettingsDataSource = {
      loadSettings,
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn().mockResolvedValue(undefined),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
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

    expect(loadSettings).toHaveBeenCalledTimes(3);
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
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(missingProjection),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));

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
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(safeModeSnapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));

    await act(async () => {
      await result.current.load(false);
    });

    expect(result.current.protectionState).toBe("active");
    expect(result.current.actions.test.enabled).toBe(false);
    expect(result.current.actions.save.enabled).toBe(false);
    expect(result.current.effectiveBoundaryEnvelope.data).toMatchObject({
      routeType: "unknown",
      externalTransmission: "unknown",
      privacyLabel: "系统安全模式仍在生效",
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
    let finishReload: (value: SettingsSnapshot) => void = () => undefined;
    const delayedReload = new Promise<SettingsSnapshot>(resolve => {
      finishReload = resolve;
    });
    const source: SettingsDataSource = {
      loadSettings: vi
        .fn()
        .mockResolvedValueOnce(snapshot(original, boundary()))
        .mockImplementationOnce(() => delayedReload),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsController(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    let reloadPromise: Promise<SettingsSnapshot> | undefined;
    act(() => {
      reloadPromise = result.current.load(false);
    });
    await waitFor(() => expect(result.current.loading).toBe(true));

    expect(result.current.protectionState).toBe("loading");
    expect(result.current.actions.test.enabled).toBe(false);
    expect(result.current.actions.save.enabled).toBe(false);
    expect(result.current.effectiveBoundaryEnvelope.status).toBe("loading");
    expect(settingsContext(result.current, "model-provider").status).toMatchObject({
      label: "正在读取",
      status: "neutral",
    });
    const loadingInspector = settingsInspector(result.current, "model-provider", "");
    expect(loadingInspector.conclusion).toContain("旧快照不作为当前确定态");
    expect(loadingInspector.nextAction).toContain("不修改、不测试、不保存");
    act(() => result.current.edit({ field: "chat_model", value: "must-not-win" }));
    expect(result.current.draft?.llm.chat_model).toBe("deepseek-chat");
    expect(announce).toHaveBeenLastCalledWith(
      "当前不能修改设置；请等待系统配置读取或当前操作结束。"
    );

    await act(async () => {
      finishReload(snapshot(refreshed, boundary()));
      await reloadPromise;
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.draft?.llm.chat_model).toBe("deepseek-chat-v2");
  });

  it("clears a masked credential when the provider identity changes", async () => {
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.edit({ field: "provider", value: "openai" }));

    expect(result.current.draft?.llm.openai_key).toBe("");
    expect(result.current.actions.test.enabled).toBe(false);
  });

  it("preserves masked credential semantics for the same normalized destination", async () => {
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(config(), boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
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

  it("does not carry a stored search credential to a different search provider", async () => {
    const current = {
      ...config(),
      system: {
        search_provider: "deepseek" as const,
        search_provider_key: "***",
        search_provider_key_ref: "keychain://com.openlife.desktop/search-provider-api-key",
      },
    };
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot(current, boundary())),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.edit({ field: "search_provider", value: "brave" }));
    expect(result.current.draft?.system?.search_provider_key).toBe("");
    expect(result.current.draft?.system?.search_provider_key_ref).toBeUndefined();

    act(() => result.current.edit({ field: "search_provider", value: "deepseek" }));
    expect(result.current.draft?.system?.search_provider_key).toBe("***");
  });

  it("reloads backend-owned artifact output state after native folder selection", async () => {
    const initial = { ...config(), system: { artifact_output_directory: undefined } };
    const refreshed = {
      ...config(),
      system: { artifact_output_directory: "/tmp/openlife-artifacts" },
    };
    const loadSettings = vi
      .fn()
      .mockResolvedValueOnce(snapshot(initial, boundary()))
      .mockResolvedValueOnce(snapshot(refreshed, boundary()));
    const source: SettingsDataSource = {
      loadSettings,
      selectArtifactOutputDirectory: vi.fn().mockResolvedValue({
        cancelled: false,
        selectedPath: "/tmp/openlife-artifacts",
      }),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };
    const { result } = renderHook(() => useSettingsController(source, vi.fn()));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.selectArtifactOutputDirectory());
    await waitFor(() =>
      expect(result.current.draft?.system?.artifact_output_directory).toBe(
        "/tmp/openlife-artifacts"
      )
    );
    expect(source.selectArtifactOutputDirectory).toHaveBeenCalledTimes(1);
    expect(loadSettings).toHaveBeenCalledTimes(2);
  });

  it("revokes an exact reusable permission and accepts only the refreshed backend model", async () => {
    const permissionId = "00000000-0000-4000-8000-000000000001";
    const before = snapshot(config(), boundary());
    before.toolPermissionEnvelope = {
      ...before.toolPermissionEnvelope,
      status: "ready",
      data: {
        items: [
          {
            id: permissionId,
            toolName: "web.search",
            source: "builtin",
            riskLevel: "medium",
            actionType: "network",
            policy: "allow_until_revoked",
            lifecycleState: "active",
            createdAt: "2026-08-24T00:00:00Z",
            revocable: true,
          },
        ],
        totalCount: 1,
        activeCount: 1,
        revocableCount: 1,
        contractLimitations: [],
      },
    };
    const after = snapshot(config(), boundary());
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValueOnce(before).mockResolvedValueOnce(after),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
      revokeToolPermission: vi.fn().mockResolvedValue(undefined),
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsController(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.revokeToolPermission(permissionId));

    await waitFor(() => expect(result.current.permissionRevocation.permissionId).toBeNull());
    expect(source.revokeToolPermission).toHaveBeenCalledWith(permissionId);
    expect(source.loadSettings).toHaveBeenCalledTimes(2);
    expect(result.current.snapshot?.toolPermissionEnvelope.data?.items).toEqual([]);
    expect(announce).toHaveBeenLastCalledWith("工具权限已由系统撤销并重新读取。");
  });

  it("does not claim revocation when the refreshed permission model cannot confirm it", async () => {
    const permissionId = "00000000-0000-4000-8000-000000000002";
    const before = snapshot(config(), boundary());
    before.toolPermissionEnvelope = {
      ...before.toolPermissionEnvelope,
      status: "ready",
      data: {
        items: [
          {
            id: permissionId,
            toolName: "web.fetch",
            source: "builtin",
            riskLevel: "medium",
            actionType: "network",
            policy: "allow_until_revoked",
            lifecycleState: "active",
            createdAt: "2026-08-24T00:00:00Z",
            revocable: true,
          },
        ],
        totalCount: 1,
        activeCount: 1,
        revocableCount: 1,
        contractLimitations: [],
      },
    };
    const unconfirmed = snapshot(config(), boundary());
    unconfirmed.toolPermissionEnvelope = {
      data: null,
      status: "error",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValueOnce(before).mockResolvedValueOnce(unconfirmed),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
      revokeToolPermission: vi.fn().mockResolvedValue(undefined),
    };
    const announce = vi.fn();
    const { result } = renderHook(() => useSettingsController(source, announce));
    await act(async () => {
      await result.current.load(false);
    });

    act(() => result.current.revokeToolPermission(permissionId));

    await waitFor(() =>
      expect(result.current.permissionRevocation.error).toBe(
        "tool_permission_revocation_refresh_unconfirmed"
      )
    );
    expect(announce).toHaveBeenLastCalledWith(
      "撤销请求已经返回，但权限读模型没有确认结果；当前状态保持未知。"
    );
  });
});
