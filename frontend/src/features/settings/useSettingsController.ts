import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  initialSettingsOrchestrationState,
  settingsOrchestrationReducer,
  type SettingsOrchestrationState,
} from "@/contracts/settingsOrchestrationContract";
import type {
  AppConfig,
  CredentialRecoveryReport,
  ProviderConnectionsViewModel,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import { productErrorCode as errorCode } from "@/shared/productError";
import {
  buildSettingsErrorSnapshot,
  type SettingsDataSource,
  type SettingsSnapshot,
  type ProviderConnectionDataSource,
} from "./settingsDataSource";
import {
  cloneSettingsConfig,
  searchCredentialState,
  searchProviderIdentity,
  settingsConfigMatchesSavedDraft,
  settingsProductActions,
  unknownDraftBoundaryEnvelope,
  unknownSettingsProtectionBoundaryEnvelope,
  validateSettingsDraft,
  type SettingsDraftValidation,
} from "./settingsPresentation";

type Announce = (message: string) => void;

export type SettingsProtectionState = "loading" | "normal" | "active" | "unknown";

export type SettingsDraftEdit =
  | { field: "prefer_local"; value: boolean }
  | { field: "local_model"; value: string }
  | { field: "agent_memory_enabled"; value: boolean }
  | { field: "network_enabled"; value: boolean }
  | { field: "network_default"; value: "ask" | "allow" | "deny" }
  | {
      field: "search_provider";
      value: NonNullable<NonNullable<AppConfig["system"]>["search_provider"]>;
    }
  | { field: "search_credential"; value: string }
  | { field: "searxng_url"; value: string };

export type SettingsController = {
  snapshot: SettingsSnapshot | null;
  draft: AppConfig | null;
  state: SettingsOrchestrationState;
  loading: boolean;
  protectionState: SettingsProtectionState;
  validation: SettingsDraftValidation;
  actions: ReturnType<typeof settingsProductActions>;
  effectiveBoundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  credentialInitialization: {
    phase: "idle" | "running" | "restart_required" | "blocked" | "failed";
    report: CredentialRecoveryReport | null;
    error: string | null;
  };
  eligibleCredentialPurposes: string[];
  artifactDirectorySelection: { phase: "idle" | "selecting" | "failed"; error: string | null };
  permissionRevocation: { permissionId: string | null; error: string | null };
  providerConnectionDataSource: ProviderConnectionDataSource | null;
  providerConnections: ProviderConnectionsViewModel | null;
  setProviderConnections: (viewModel: ProviderConnectionsViewModel) => void;
  load: (announceResult?: boolean) => Promise<SettingsSnapshot>;
  ensureLoaded: () => Promise<SettingsEnsureLoadedResult>;
  edit: (edit: SettingsDraftEdit) => void;
  initializeRequiredCredentials: () => void;
  save: () => void;
  retryBoundaryRefresh: () => void;
  selectArtifactOutputDirectory: () => void;
  revokeToolPermission: (permissionId: string) => void;
};

export type SettingsEnsureLoadedResult = {
  snapshot: SettingsSnapshot;
  loadedFromSource: boolean;
  retainedUnsavedDraft: boolean;
};

type SettingsOperationToken = {
  kind:
    | "save"
    | "boundary_refresh"
    | "credential_initialization"
    | "artifact_directory"
    | "permission_revocation";
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
    case "prefer_local":
      next.prefer_local_model = edit.value;
      return next;
    case "local_model":
      next.local_model = edit.value;
      return next;
    case "agent_memory_enabled":
      next.system = { ...next.system, agent_memory_enabled: edit.value };
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
    case "search_provider":
      next.system = { ...next.system, search_provider: edit.value };
      return next;
    case "search_credential":
      next.system = { ...next.system, search_provider_key: edit.value };
      return next;
    case "searxng_url":
      next.system = { ...next.system, searxng_url: edit.value };
      return next;
  }
}

function protectionStateForSnapshot(
  snapshot: SettingsSnapshot | null,
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

export function useSettingsController(
  dataSource: SettingsDataSource | undefined,
  announce: Announce
): SettingsController {
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [providerConnections, setProviderConnections] =
    useState<ProviderConnectionsViewModel | null>(null);
  const [state, dispatch] = useReducer(
    settingsOrchestrationReducer,
    initialSettingsOrchestrationState
  );
  const [loading, setLoading] = useState(false);
  const [credentialInitialization, setCredentialInitialization] = useState<{
    phase: "idle" | "running" | "restart_required" | "blocked" | "failed";
    report: CredentialRecoveryReport | null;
    error: string | null;
  }>({ phase: "idle", report: null, error: null });
  const [artifactDirectorySelection, setArtifactDirectorySelection] = useState<{
    phase: "idle" | "selecting" | "failed";
    error: string | null;
  }>({ phase: "idle", error: null });
  const [permissionRevocation, setPermissionRevocation] = useState<{
    permissionId: string | null;
    error: string | null;
  }>({ permissionId: null, error: null });
  const requestRef = useRef(0);
  const sourceGenerationRef = useRef(0);
  const operationSequenceRef = useRef(0);
  const operationRef = useRef<SettingsOperationToken | null>(null);
  const snapshotRef = useRef<SettingsSnapshot | null>(null);
  const stateRef = useRef(state);
  const activeLoadPromiseRef = useRef<Promise<SettingsSnapshot> | null>(null);
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
    setProviderConnections(null);
    setLoading(false);
    setCredentialInitialization({ phase: "idle", report: null, error: null });
    setArtifactDirectorySelection({ phase: "idle", error: null });
    setPermissionRevocation({ permissionId: null, error: null });
    dispatch({ type: "reset" });
    return () => {
      sourceGenerationRef.current += 1;
      requestRef.current += 1;
      snapshotRef.current = null;
      activeLoadPromiseRef.current = null;
    };
  }, [dataSource]);

  const load = useCallback(
    (announceResult = true): Promise<SettingsSnapshot> => {
      const requestId = ++requestRef.current;
      setLoading(true);
      const loadPromise = (async () => {
        let next: SettingsSnapshot;
        try {
          next = dataSource
            ? await dataSource.loadSettings()
            : buildSettingsErrorSnapshot("settings_privacy_data_source_unavailable");
        } catch (error) {
          next = buildSettingsErrorSnapshot(error);
        }
        if (requestId === requestRef.current) {
          snapshotRef.current = next;
          setSnapshot(next);
          setDraft(next.config ? cloneSettingsConfig(next.config) : null);
          setLoading(false);
          if (announceResult) {
            announce(
              next.config && next.boundaryEnvelope.status !== "error"
                ? "设置与模型传输边界已从系统重新读取。"
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
        announce("当前不能修改设置；请等待系统配置读取或当前操作结束。");
        return;
      }
      const previousSearchIdentity = searchProviderIdentity(draft);
      const next = applyDraftEdit(draft, change);
      const searchIdentityChanged = searchProviderIdentity(next) !== previousSearchIdentity;
      const matchesStoredSearchIdentity =
        snapshot?.config !== null &&
        snapshot?.config !== undefined &&
        searchCredentialState(snapshot.config) === "stored" &&
        searchProviderIdentity(next) === searchProviderIdentity(snapshot.config);
      const restoreStoredSearchCredential =
        matchesStoredSearchIdentity &&
        (searchIdentityChanged || (change.field === "search_credential" && !change.value.trim()));
      if (searchIdentityChanged) {
        next.system = {
          ...next.system,
          search_provider_key: "",
          search_provider_key_ref: undefined,
        };
      }
      if (restoreStoredSearchCredential) {
        next.system = {
          ...next.system,
          search_provider_key: snapshot?.config?.system?.search_provider_key === "***" ? "***" : "",
          search_provider_key_ref: snapshot?.config?.system?.search_provider_key_ref,
        };
      }
      setDraft(next);
      pendingSaveAttestationRef.current = null;
      dispatch({ type: "edit" });
      if (searchIdentityChanged) {
        announce(
          restoreStoredSearchCredential
            ? "已返回原始网页搜索目标；同一目标继续使用系统保存的独立搜索凭据。"
            : "网页搜索目标已更改；旧搜索凭据已清除，不会带到新的搜索服务。"
        );
      } else {
        announce("设置草稿已更改；保存并刷新边界前，不把草稿解释为当前产品状态。");
      }
    },
    [announce, draft, loading, snapshot?.config]
  );

  const validation = useMemo(
    () => validateSettingsDraft(draft, providerConnections),
    [draft, providerConnections]
  );
  const eligibleCredentialPurposes = useMemo(() => {
    if (!dataSource?.initializeRequiredCredentials) return [];
    const purposes = snapshot?.credentialBootstrap?.purposes ?? [];
    const unavailable = purposes
      .filter(purpose => purpose.status === "unavailable")
      .map(purpose => purpose.purpose)
      .sort();
    if (unavailable.length > 0) return unavailable;
    return purposes
      .filter(purpose => purpose.status === "initialization_required")
      .map(purpose => purpose.purpose)
      .sort();
  }, [dataSource, snapshot?.credentialBootstrap]);
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
        ? "系统安全模式仍在生效；设置保存保持关闭。"
        : protectionState === "loading"
          ? "正在读取 LifeStateProjection 保护状态。"
          : "LifeStateProjection 保护状态未知；设置保存保持关闭。";
    return {
      save: { ...base.save, enabled: false, disabledReason },
    };
  }, [protectionState, state, validation]);

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
        let refreshed: SettingsSnapshot;
        try {
          refreshed = await dataSource.loadSettings();
        } catch (error) {
          refreshed = buildSettingsErrorSnapshot(error);
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
            ? "保存后的配置与模型传输边界已经由系统重新确认。"
            : "设置已保存，但系统返回的模型传输边界仍未知；当前不显示本地确定态。"
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
      let refreshed: SettingsSnapshot;
      try {
        refreshed = await dataSource.loadSettings();
      } catch (error) {
        refreshed = buildSettingsErrorSnapshot(error);
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
          : "已重新读取精确配置，但系统边界仍未知；当前不显示本地确定态。"
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
      announce("当前系统快照没有可执行的凭据初始化或访问恢复操作。");
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
              ? "本次凭据读取已经获得系统许可；必须完全重启 OpenLife，并由非交互启动读取验证持续访问。"
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

  const selectArtifactOutputDirectory = useCallback(() => {
    if (!dataSource?.selectArtifactOutputDirectory || operationRef.current) {
      announce("当前不能选择 artifact 输出目录；请等待系统或当前操作结束。");
      return;
    }
    if (protectionState !== "normal") {
      announce("当前保护状态不允许修改 artifact 输出目录。");
      return;
    }
    if (state.draftRevision !== state.savedRevision) {
      announce("请先保存或撤销当前设置草稿，再选择 artifact 输出目录。");
      return;
    }
    const operationToken: SettingsOperationToken = {
      kind: "artifact_directory",
      sourceGeneration: sourceGenerationRef.current,
      sequence: ++operationSequenceRef.current,
    };
    operationRef.current = operationToken;
    setArtifactDirectorySelection({ phase: "selecting", error: null });
    announce("正在打开系统文件夹选择器；尚未选择时不会保存路径。");
    void dataSource
      .selectArtifactOutputDirectory()
      .then(async result => {
        if (operationRef.current !== operationToken) return;
        if (result.cancelled) {
          setArtifactDirectorySelection({ phase: "idle", error: null });
          announce("已取消文件夹选择；artifact 输出目录没有改变。");
          return;
        }
        await load(false);
        if (operationRef.current !== operationToken) return;
        setArtifactDirectorySelection({ phase: "idle", error: null });
        announce("artifact 输出目录已由系统保存并重新读取。");
      })
      .catch(error => {
        if (operationRef.current !== operationToken) return;
        const code = errorCode(error);
        setArtifactDirectorySelection({ phase: "failed", error: code });
        announce("artifact 输出目录没有保存；现有路径保持不变。");
      })
      .finally(() => {
        if (operationRef.current === operationToken) operationRef.current = null;
      });
  }, [announce, dataSource, load, protectionState, state.draftRevision, state.savedRevision]);

  const revokeToolPermission = useCallback(
    (permissionId: string) => {
      if (!dataSource?.revokeToolPermission) {
        announce("权限管理命令不可用；现有权限没有改变。");
        return;
      }
      if (
        operationRef.current ||
        protectionState !== "normal" ||
        state.draftRevision !== state.savedRevision
      ) {
        announce("当前不能撤销权限；请先完成其他操作并保存设置草稿。");
        return;
      }
      const operationToken: SettingsOperationToken = {
        kind: "permission_revocation",
        sourceGeneration: sourceGenerationRef.current,
        sequence: ++operationSequenceRef.current,
      };
      operationRef.current = operationToken;
      setPermissionRevocation({ permissionId, error: null });
      announce("正在撤销精确工具权限；完成前不会改变前端推断状态。");
      void dataSource
        .revokeToolPermission(permissionId)
        .then(async () => {
          if (operationRef.current !== operationToken) return;
          const refreshed = await load(false);
          if (operationRef.current !== operationToken) return;
          if (
            refreshed.toolPermissionEnvelope.status === "error" ||
            !refreshed.toolPermissionEnvelope.data ||
            refreshed.toolPermissionEnvelope.data.items.some(item => item.id === permissionId)
          ) {
            setPermissionRevocation({
              permissionId: null,
              error: "tool_permission_revocation_refresh_unconfirmed",
            });
            announce("撤销请求已经返回，但权限读模型没有确认结果；当前状态保持未知。");
            return;
          }
          setPermissionRevocation({ permissionId: null, error: null });
          announce("工具权限已由系统撤销并重新读取。");
        })
        .catch(error => {
          if (operationRef.current !== operationToken) return;
          setPermissionRevocation({ permissionId: null, error: errorCode(error) });
          announce("工具权限未撤销；现有系统状态保持不变。");
        })
        .finally(() => {
          if (operationRef.current === operationToken) operationRef.current = null;
        });
    },
    [announce, dataSource, load, protectionState, state.draftRevision, state.savedRevision]
  );

  const hasUnsavedDraft = state.draftRevision !== state.savedRevision;
  const providerConnectionDataSource = useMemo<ProviderConnectionDataSource | null>(() => {
    if (
      !dataSource?.loadProviderConnections ||
      !dataSource.saveProviderConnection ||
      !dataSource.deleteProviderConnection ||
      !dataSource.testSavedProviderConnection
    ) {
      return null;
    }
    return {
      loadProviderConnections: () => dataSource.loadProviderConnections!(),
      saveProviderConnection: input => dataSource.saveProviderConnection!(input),
      deleteProviderConnection: connectionId => dataSource.deleteProviderConnection!(connectionId),
      testSavedProviderConnection: (connectionId, profileId) =>
        dataSource.testSavedProviderConnection!(connectionId, profileId),
    };
  }, [dataSource]);
  const effectiveBoundaryEnvelope = useMemo(() => {
    if (!snapshot) return loadingBoundaryEnvelope();
    if (protectionState === "loading") return loadingBoundaryEnvelope();
    if (protectionState === "active" || protectionState === "unknown") {
      return unknownSettingsProtectionBoundaryEnvelope(
        protectionState === "active"
          ? "系统安全模式仍在生效；不能把配置或 Provider 边界解释为正常运行证明。"
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
    protectionState,
    validation,
    actions,
    effectiveBoundaryEnvelope,
    credentialInitialization,
    eligibleCredentialPurposes,
    artifactDirectorySelection,
    permissionRevocation,
    providerConnectionDataSource,
    providerConnections,
    setProviderConnections,
    load,
    ensureLoaded,
    edit,
    initializeRequiredCredentials,
    save,
    retryBoundaryRefresh,
    selectArtifactOutputDirectory,
    revokeToolPermission,
  };
}
