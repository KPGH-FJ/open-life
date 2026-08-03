import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  initialSettingsOrchestrationState,
  settingsOrchestrationReducer,
  type SettingsOrchestrationState,
} from "@/contracts/settingsOrchestrationContract";
import type {
  AppConfig,
  CredentialRecoveryReport,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import { journeyErrorCode as errorCode } from "@/ui/journeys/journeyError";
import {
  buildSettingsPrivacyErrorSnapshot,
  type SettingsConnectionTestOutcome,
  type SettingsPrivacyDataSource,
  type SettingsPrivacySnapshot,
} from "./settingsPrivacyDataSource";
import {
  cloneSettingsConfig,
  connectionTestPresentation,
  credentialState,
  endpointHost,
  providerIdentity,
  settingsConfigMatchesSavedDraft,
  settingsProductActions,
  unknownDraftBoundaryEnvelope,
  unknownSettingsProtectionBoundaryEnvelope,
  validateSettingsDraft,
  type SettingsDraftValidation,
  type SettingsTestPresentation,
} from "./settingsPrivacyPresentation";

type Announce = (message: string) => void;

export type SettingsProtectionState = "loading" | "normal" | "active" | "unknown";

export type SettingsDraftEdit =
  | { field: "provider"; value: NonNullable<AppConfig["llm"]["provider"]> }
  | { field: "endpoint"; value: string }
  | { field: "chat_model"; value: string }
  | { field: "credential"; value: string }
  | { field: "prefer_local"; value: boolean }
  | { field: "local_model"; value: string }
  | { field: "network_enabled"; value: boolean }
  | { field: "network_default"; value: "ask" | "allow" | "deny" };

export type SettingsPrivacyJourneyController = {
  snapshot: SettingsPrivacySnapshot | null;
  draft: AppConfig | null;
  state: SettingsOrchestrationState;
  loading: boolean;
  lastTestOutcome: SettingsConnectionTestOutcome | null;
  testPresentation: SettingsTestPresentation | null;
  protectionState: SettingsProtectionState;
  validation: SettingsDraftValidation;
  actions: ReturnType<typeof settingsProductActions>;
  effectiveBoundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  testConfirmationOpen: boolean;
  credentialInitialization: {
    phase: "idle" | "running" | "restart_required" | "blocked" | "failed";
    report: CredentialRecoveryReport | null;
    error: string | null;
  };
  eligibleCredentialPurposes: string[];
  load: (announceResult?: boolean) => Promise<SettingsPrivacySnapshot>;
  ensureLoaded: () => Promise<SettingsEnsureLoadedResult>;
  edit: (edit: SettingsDraftEdit) => void;
  requestTest: () => void;
  confirmTest: () => void;
  cancelTest: () => void;
  initializeRequiredCredentials: () => void;
  save: () => void;
  retryBoundaryRefresh: () => void;
};

export type SettingsEnsureLoadedResult = {
  snapshot: SettingsPrivacySnapshot;
  loadedFromSource: boolean;
  retainedUnsavedDraft: boolean;
};

type SettingsOperationToken = {
  kind: "test" | "save" | "boundary_refresh" | "credential_initialization";
  sourceGeneration: number;
  sequence: number;
};

function loadingBoundaryEnvelope(): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data: null,
    status: "loading",
    lastUpdatedAt: null,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function applyDraftEdit(config: AppConfig, edit: SettingsDraftEdit): AppConfig {
  const next = cloneSettingsConfig(config);
  switch (edit.field) {
    case "provider":
      next.llm.provider = edit.value;
      return next;
    case "endpoint":
      next.llm.openai_base = edit.value;
      return next;
    case "chat_model":
      next.llm.chat_model = edit.value;
      return next;
    case "credential":
      next.llm.openai_key = edit.value;
      return next;
    case "prefer_local":
      next.prefer_local_model = edit.value;
      return next;
    case "local_model":
      next.local_model = edit.value;
      return next;
    case "network_enabled":
      next.system = {
        ...next.system,
        network_policy: { ...next.system?.network_policy, enabled: edit.value },
      };
      return next;
    case "network_default":
      next.system = {
        ...next.system,
        network_policy: { ...next.system?.network_policy, default_decision: edit.value },
      };
      return next;
  }
}

function protectionStateForSnapshot(
  snapshot: SettingsPrivacySnapshot | null,
  loading = false
): SettingsProtectionState {
  if (loading) return "loading";
  if (!snapshot) return "unknown";
  const projectionLoaded = snapshot.diagnostics.some(
    diagnostic => diagnostic.id === "life_state_projection" && diagnostic.status === "loaded"
  );
  if (!projectionLoaded || !snapshot.safeMode) return "unknown";
  return snapshot.safeMode.active ? "active" : "normal";
}

function boundaryIsKnown(boundary: ProviderPrivacyBoundarySummary): boolean {
  return (
    boundary.routeType !== "unknown" &&
    boundary.externalTransmission !== "unknown" &&
    boundary.risk !== "unknown"
  );
}

export function useSettingsPrivacyJourney(
  dataSource: SettingsPrivacyDataSource | undefined,
  announce: Announce
): SettingsPrivacyJourneyController {
  const [snapshot, setSnapshot] = useState<SettingsPrivacySnapshot | null>(null);
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [state, dispatch] = useReducer(
    settingsOrchestrationReducer,
    initialSettingsOrchestrationState
  );
  const [loading, setLoading] = useState(false);
  const [lastTestOutcome, setLastTestOutcome] = useState<SettingsConnectionTestOutcome | null>(
    null
  );
  const [testConfirmationOpen, setTestConfirmationOpen] = useState(false);
  const [credentialInitialization, setCredentialInitialization] = useState<{
    phase: "idle" | "running" | "restart_required" | "blocked" | "failed";
    report: CredentialRecoveryReport | null;
    error: string | null;
  }>({ phase: "idle", report: null, error: null });
  const requestRef = useRef(0);
  const sourceGenerationRef = useRef(0);
  const operationSequenceRef = useRef(0);
  const operationRef = useRef<SettingsOperationToken | null>(null);
  const snapshotRef = useRef<SettingsPrivacySnapshot | null>(null);
  const stateRef = useRef(state);
  const activeLoadPromiseRef = useRef<Promise<SettingsPrivacySnapshot> | null>(null);
  const pendingSaveAttestationRef = useRef<{
    previousConfig: AppConfig;
    submittedConfig: AppConfig;
  } | null>(null);

  snapshotRef.current = snapshot;
  stateRef.current = state;

  useEffect(() => {
    sourceGenerationRef.current += 1;
    requestRef.current += 1;
    operationRef.current = null;
    snapshotRef.current = null;
    activeLoadPromiseRef.current = null;
    pendingSaveAttestationRef.current = null;
    setSnapshot(null);
    setDraft(null);
    setLoading(false);
    setLastTestOutcome(null);
    setTestConfirmationOpen(false);
    setCredentialInitialization({ phase: "idle", report: null, error: null });
    dispatch({ type: "reset" });
    return () => {
      sourceGenerationRef.current += 1;
      requestRef.current += 1;
      snapshotRef.current = null;
      activeLoadPromiseRef.current = null;
    };
  }, [dataSource]);

  const load = useCallback(
    (announceResult = true): Promise<SettingsPrivacySnapshot> => {
      const requestId = ++requestRef.current;
      setLoading(true);
      const loadPromise = (async () => {
        let next: SettingsPrivacySnapshot;
        try {
          next = dataSource
            ? await dataSource.loadSettingsPrivacy()
            : buildSettingsPrivacyErrorSnapshot("settings_privacy_data_source_unavailable");
        } catch (error) {
          next = buildSettingsPrivacyErrorSnapshot(error);
        }
        if (requestId === requestRef.current) {
          snapshotRef.current = next;
          setSnapshot(next);
          setDraft(next.config ? cloneSettingsConfig(next.config) : null);
          setLoading(false);
          if (announceResult) {
            announce(
              next.config && next.boundaryEnvelope.status !== "error"
                ? "设置与模型传输边界已从后端重新读取。"
                : "设置读取不完整；测试、保存和本地确定态保持关闭。"
            );
          }
        }
        return next;
      })();
      const trackedLoadPromise = loadPromise.finally(() => {
        if (activeLoadPromiseRef.current === trackedLoadPromise) {
          activeLoadPromiseRef.current = null;
        }
      });
      activeLoadPromiseRef.current = trackedLoadPromise;
      return trackedLoadPromise;
    },
    [announce, dataSource]
  );

  const ensureLoaded = useCallback(async (): Promise<SettingsEnsureLoadedResult> => {
    const activeLoad = activeLoadPromiseRef.current;
    if (activeLoad) {
      return {
        snapshot: await activeLoad,
        loadedFromSource: true,
        retainedUnsavedDraft: false,
      };
    }

    const currentSnapshot = snapshotRef.current;
    if (currentSnapshot) {
      const currentState = stateRef.current;
      return {
        snapshot: currentSnapshot,
        loadedFromSource: false,
        retainedUnsavedDraft: currentState.draftRevision !== currentState.savedRevision,
      };
    }

    return {
      snapshot: await load(false),
      loadedFromSource: true,
      retainedUnsavedDraft: false,
    };
  }, [load]);

  const edit = useCallback(
    (change: SettingsDraftEdit) => {
      if (!draft || loading || operationRef.current) {
        announce("当前不能修改设置；请等待后端配置读取或当前操作结束。");
        return;
      }
      const previousIdentity = providerIdentity(draft);
      const next = applyDraftEdit(draft, change);
      const identityChanged = providerIdentity(next) !== previousIdentity;
      const matchesStoredIdentity =
        snapshot?.config !== null &&
        snapshot?.config !== undefined &&
        credentialState(snapshot.config) === "stored" &&
        providerIdentity(next) === providerIdentity(snapshot.config);
      const restoreStoredCredential =
        matchesStoredIdentity &&
        (identityChanged || (change.field === "credential" && !change.value.trim()));
      if (identityChanged) {
        next.llm.openai_key = "";
        next.llm.openai_key_ref = undefined;
      }
      if (restoreStoredCredential) {
        next.llm.openai_key = snapshot?.config?.llm.openai_key === "***" ? "***" : "";
        next.llm.openai_key_ref = snapshot?.config?.llm.openai_key_ref;
      }
      setDraft(next);
      setLastTestOutcome(null);
      pendingSaveAttestationRef.current = null;
      dispatch({ type: "edit" });
      if (identityChanged) {
        announce(
          restoreStoredCredential
            ? "已返回原始供应商目标；同一目标继续使用后端保存的凭据，传输边界仍等待保存后确认。"
            : "供应商目标已更改；旧凭据已清除，当前传输边界等待后端重新确认。"
        );
      } else {
        announce("设置草稿已更改；保存并刷新边界前，不把草稿解释为当前产品状态。");
      }
    },
    [announce, draft, loading, snapshot?.config]
  );

  const validation = useMemo(() => validateSettingsDraft(draft), [draft]);
  const eligibleCredentialPurposes = useMemo(
    () =>
      dataSource?.initializeRequiredCredentials
        ? (snapshot?.credentialBootstrap?.purposes ?? [])
            .filter(
              purpose =>
                purpose.status === "initialization_required" || purpose.status === "unavailable"
            )
            .map(purpose => purpose.purpose)
            .sort()
        : [],
    [dataSource, snapshot?.credentialBootstrap]
  );
  const credentialAccessRecoveryRequired = useMemo(
    () =>
      (snapshot?.credentialBootstrap?.purposes ?? []).some(
        purpose => purpose.status === "unavailable"
      ),
    [snapshot?.credentialBootstrap]
  );
  const protectionState = protectionStateForSnapshot(snapshot, loading);
  const actions = useMemo(() => {
    const base = settingsProductActions(state, validation);
    if (protectionState === "normal") return base;
    const disabledReason =
      protectionState === "active"
        ? "后端安全模式仍在生效；连接测试和设置保存保持关闭。"
        : protectionState === "loading"
          ? "正在读取 LifeStateProjection 保护状态。"
          : "LifeStateProjection 保护状态未知；连接测试和设置保存保持关闭。";
    return {
      test: { ...base.test, enabled: false, disabledReason },
      save: { ...base.save, enabled: false, disabledReason },
    };
  }, [protectionState, state, validation]);

  const executeTest = useCallback(async () => {
    if (!dataSource || !draft || operationRef.current || !actions.test.enabled) return;
    const draftIsAlreadySaved = state.savedRevision === state.draftRevision;
    const operationToken: SettingsOperationToken = {
      kind: "test",
      sourceGeneration: sourceGenerationRef.current,
      sequence: ++operationSequenceRef.current,
    };
    operationRef.current = operationToken;
    const operationIsCurrent = () =>
      operationRef.current === operationToken &&
      sourceGenerationRef.current === operationToken.sourceGeneration;
    dispatch({ type: "test_requested" });
    setLastTestOutcome(null);
    announce("正在验证这一份设置草稿；测试不会保存任何配置。");
    try {
      const outcome = await dataSource.testProviderConnection(cloneSettingsConfig(draft));
      if (!operationIsCurrent()) return;
      setLastTestOutcome(outcome);
      const result = outcome.result;
      const receipt = result.provider_invocation_receipt;
      const verified =
        result.ok &&
        result.validation_status === "validated" &&
        receipt?.status === "completed" &&
        !receipt.simulated;
      if (verified) {
        dispatch({
          type: "test_succeeded",
          result: { ok: true, message: result.message },
        });
        announce(
          draftIsAlreadySaved
            ? "本次连接验证已有可信回执；当前已保存设置未被测试改变。"
            : "本次连接验证已有可信回执；设置仍未保存。"
        );
      } else {
        dispatch({
          type: "test_failed",
          errorCode: result.validation_status || "provider_test_not_verified",
        });
        announce(
          result.validation_status === "consent_required"
            ? outcome.reviewItem
              ? "测试请求需要审核；精确待决定项已经找到，请按需查看。"
              : "测试请求需要审核，但当前无法解析精确待决定项；不会跳转到猜测目标。"
            : result.validation_status === "remote_unknown"
              ? "外部结果未知；当前不会自动重试或显示连接可用。"
              : "连接没有通过可信验证；当前不会显示可用，也不会自动保存。"
        );
      }
    } catch (error) {
      if (!operationIsCurrent()) return;
      dispatch({ type: "test_failed", errorCode: errorCode(error) });
      announce("连接测试命令失败；当前配置没有可用性证明。");
    } finally {
      if (operationIsCurrent()) operationRef.current = null;
    }
  }, [actions.test.enabled, announce, dataSource, draft, state.draftRevision, state.savedRevision]);

  const requestTest = useCallback(() => {
    if (!actions.test.enabled) {
      announce(`当前不能测试连接：${actions.test.disabledReason ?? "设置草稿不可用。"}`);
      return;
    }
    if (validation.mayTransmitExternally) {
      setTestConfirmationOpen(true);
      announce("等待你确认这次可能发生的外部连接；尚未发送请求。");
      return;
    }
    void executeTest();
  }, [actions.test, announce, executeTest, validation.mayTransmitExternally]);

  const confirmTest = useCallback(() => {
    setTestConfirmationOpen(false);
    void executeTest();
  }, [executeTest]);

  const cancelTest = useCallback(() => {
    setTestConfirmationOpen(false);
    announce("已取消连接测试；没有发送网络请求，也没有保存设置。");
  }, [announce]);

  const save = useCallback(() => {
    if (
      !dataSource ||
      !snapshot?.config ||
      !draft ||
      operationRef.current ||
      !actions.save.enabled
    ) {
      announce(`当前不能保存：${actions.save.disabledReason ?? "设置草稿不可用。"}`);
      return;
    }
    const submitted = cloneSettingsConfig(draft);
    const previousConfig = cloneSettingsConfig(snapshot.config);
    const operationToken: SettingsOperationToken = {
      kind: "save",
      sourceGeneration: sourceGenerationRef.current,
      sequence: ++operationSequenceRef.current,
    };
    operationRef.current = operationToken;
    const operationIsCurrent = () =>
      operationRef.current === operationToken &&
      sourceGenerationRef.current === operationToken.sourceGeneration;
    pendingSaveAttestationRef.current = {
      previousConfig,
      submittedConfig: cloneSettingsConfig(submitted),
    };
    dispatch({ type: "save_requested" });
    announce("正在保存设置；命令返回不代表模型边界已经确认。");
    void (async () => {
      try {
        await dataSource.saveSettings(submitted);
        if (!operationIsCurrent()) return;
        dispatch({ type: "save_succeeded" });
        announce("设置命令已返回，正在重新读取配置与模型传输边界。");
        let refreshed: SettingsPrivacySnapshot;
        try {
          refreshed = await dataSource.loadSettingsPrivacy();
        } catch (error) {
          refreshed = buildSettingsPrivacyErrorSnapshot(error);
        }
        if (!operationIsCurrent()) return;
        snapshotRef.current = refreshed;
        setSnapshot(refreshed);
        if (
          !refreshed.config ||
          refreshed.boundaryEnvelope.status !== "ready" ||
          !refreshed.boundaryEnvelope.data ||
          protectionStateForSnapshot(refreshed) !== "normal" ||
          !settingsConfigMatchesSavedDraft(previousConfig, submitted, refreshed.config)
        ) {
          dispatch({
            type: "boundary_refresh_failed",
            errorCode: `settings_refresh_${refreshed.boundaryEnvelope.status}`,
          });
          announce("设置命令已返回，但配置或边界没有完成核对；当前保持未知。");
          return;
        }
        setDraft(cloneSettingsConfig(refreshed.config));
        pendingSaveAttestationRef.current = null;
        dispatch({ type: "boundary_refreshed", boundary: refreshed.boundaryEnvelope.data });
        const boundary = refreshed.boundaryEnvelope.data;
        const known = boundaryIsKnown(boundary);
        announce(
          known
            ? "保存后的配置与模型传输边界已经由后端重新确认。"
            : "设置已保存，但后端返回的模型传输边界仍未知；当前不显示本地确定态。"
        );
      } catch (error) {
        if (!operationIsCurrent()) return;
        pendingSaveAttestationRef.current = null;
        dispatch({ type: "save_failed", errorCode: errorCode(error) });
        announce("设置保存失败；草稿仍保留，当前产品边界没有改变。");
      } finally {
        if (operationIsCurrent()) operationRef.current = null;
      }
    })();
  }, [actions.save, announce, dataSource, draft, snapshot?.config]);

  const retryBoundaryRefresh = useCallback(() => {
    const retryable =
      state.phase === "unknown" &&
      state.failureStage === "boundary_refresh" &&
      !state.boundaryAppliesToSavedRevision;
    const attestation = pendingSaveAttestationRef.current;
    if (!dataSource || !attestation || operationRef.current || !retryable) {
      announce("当前没有可重新核对的保存结果；页面不会猜测配置或边界状态。");
      return;
    }
    const operationToken: SettingsOperationToken = {
      kind: "boundary_refresh",
      sourceGeneration: sourceGenerationRef.current,
      sequence: ++operationSequenceRef.current,
    };
    operationRef.current = operationToken;
    const operationIsCurrent = () =>
      operationRef.current === operationToken &&
      sourceGenerationRef.current === operationToken.sourceGeneration;
    dispatch({ type: "boundary_refresh_retry_requested" });
    announce("正在重新读取已保存配置、LifeStateProjection 与模型传输边界。");
    void (async () => {
      let refreshed: SettingsPrivacySnapshot;
      try {
        refreshed = await dataSource.loadSettingsPrivacy();
      } catch (error) {
        refreshed = buildSettingsPrivacyErrorSnapshot(error);
      }
      if (!operationIsCurrent()) return;
      snapshotRef.current = refreshed;
      setSnapshot(refreshed);
      if (
        !refreshed.config ||
        refreshed.boundaryEnvelope.status !== "ready" ||
        !refreshed.boundaryEnvelope.data ||
        protectionStateForSnapshot(refreshed) !== "normal" ||
        !settingsConfigMatchesSavedDraft(
          attestation.previousConfig,
          attestation.submittedConfig,
          refreshed.config
        )
      ) {
        dispatch({
          type: "boundary_refresh_failed",
          errorCode: `settings_refresh_${refreshed.boundaryEnvelope.status}`,
        });
        announce("重新读取仍未证明精确的已保存配置与边界；当前继续保持未知。");
        if (operationIsCurrent()) operationRef.current = null;
        return;
      }
      setDraft(cloneSettingsConfig(refreshed.config));
      pendingSaveAttestationRef.current = null;
      dispatch({ type: "boundary_refreshed", boundary: refreshed.boundaryEnvelope.data });
      announce(
        boundaryIsKnown(refreshed.boundaryEnvelope.data)
          ? "已重新确认精确的已保存配置与模型传输边界。"
          : "已重新读取精确配置，但后端边界仍未知；当前不显示本地确定态。"
      );
      if (operationIsCurrent()) operationRef.current = null;
    })();
  }, [announce, dataSource, state]);

  const initializeRequiredCredentials = useCallback(() => {
    if (
      !dataSource?.initializeRequiredCredentials ||
      operationRef.current ||
      eligibleCredentialPurposes.length === 0 ||
      credentialInitialization.phase === "restart_required"
    ) {
      announce("当前后端快照没有可执行的凭据初始化或访问恢复操作。");
      return;
    }
    const operationToken: SettingsOperationToken = {
      kind: "credential_initialization",
      sourceGeneration: sourceGenerationRef.current,
      sequence: ++operationSequenceRef.current,
    };
    operationRef.current = operationToken;
    setCredentialInitialization({ phase: "running", report: null, error: null });
    announce(
      credentialAccessRecoveryRequired
        ? "正在等待系统原生确认；恢复只读取既有凭据，不会创建、覆盖或返回密钥。"
        : "正在等待系统原生确认；取消不会写入或删除任何凭据。"
    );
    void dataSource
      .initializeRequiredCredentials()
      .then(report => {
        if (operationRef.current !== operationToken) return;
        setCredentialInitialization({
          phase: report.restartRequired ? "restart_required" : "blocked",
          report,
          error: report.blockedReason ?? null,
        });
        announce(
          report.restartRequired
            ? credentialAccessRecoveryRequired
              ? "既有凭据访问已经恢复；必须完全重启 OpenLife 后才能重新判断可用状态。"
              : "系统凭据初始化已经完成；必须完全重启 OpenLife 后才能重新判断可用状态。"
            : "凭据恢复未完成；当前继续保持阻塞。"
        );
      })
      .catch(error => {
        if (operationRef.current !== operationToken) return;
        setCredentialInitialization({
          phase: "failed",
          report: null,
          error: errorCode(error),
        });
        announce("凭据初始化或访问恢复被取消或失败；当前状态没有被标记为可用。");
      })
      .finally(() => {
        if (operationRef.current === operationToken) operationRef.current = null;
      });
  }, [
    announce,
    credentialAccessRecoveryRequired,
    credentialInitialization.phase,
    dataSource,
    eligibleCredentialPurposes.length,
  ]);

  const hasUnsavedDraft = state.draftRevision !== state.savedRevision;
  const effectiveBoundaryEnvelope = useMemo(() => {
    if (!snapshot) return loadingBoundaryEnvelope();
    if (protectionState === "loading") return loadingBoundaryEnvelope();
    if (protectionState === "active" || protectionState === "unknown") {
      return unknownSettingsProtectionBoundaryEnvelope(
        protectionState === "active"
          ? "后端安全模式仍在生效；不能把配置或 Provider 边界解释为正常运行证明。"
          : "LifeStateProjection 未提供可核对的保护状态；不能显示本地、未外传或可执行结论。",
        protectionState
      );
    }
    if (state.phase === "refreshing_boundary") return loadingBoundaryEnvelope();
    if (
      hasUnsavedDraft ||
      state.phase === "saving" ||
      (state.phase === "failed" && state.failureStage === "save")
    ) {
      return unknownDraftBoundaryEnvelope(
        "当前存在尚未由保存后读模型确认的设置；不能沿用之前的本地或外传结论。"
      );
    }
    if (state.failureStage === "boundary_refresh" && !state.boundaryAppliesToSavedRevision) {
      return unknownDraftBoundaryEnvelope(
        "保存后的配置或模型传输边界没有完成核对；不能沿用之前的边界结论。"
      );
    }
    if (
      (state.phase === "ready" || state.phase === "unknown") &&
      state.boundaryAppliesToSavedRevision
    ) {
      return snapshot.boundaryEnvelope;
    }
    return snapshot.boundaryEnvelope;
  }, [hasUnsavedDraft, protectionState, snapshot, state]);

  return {
    snapshot,
    draft,
    state,
    loading,
    lastTestOutcome,
    testPresentation: connectionTestPresentation(lastTestOutcome?.result ?? null),
    protectionState,
    validation,
    actions,
    effectiveBoundaryEnvelope,
    testConfirmationOpen,
    credentialInitialization,
    eligibleCredentialPurposes,
    load,
    ensureLoaded,
    edit,
    requestTest,
    confirmTest,
    cancelTest,
    initializeRequiredCredentials,
    save,
    retryBoundaryRefresh,
  };
}

export function settingsTestConfirmationTarget(controller: SettingsPrivacyJourneyController): {
  provider: string;
  host: string;
  model: string;
} {
  const draft = controller.draft;
  return {
    provider: draft?.llm.provider ?? "未知供应商",
    host: draft ? (endpointHost(draft.llm.openai_base) ?? "无法确认的目标") : "无法确认的目标",
    model: draft?.llm.chat_model || "未知模型",
  };
}
