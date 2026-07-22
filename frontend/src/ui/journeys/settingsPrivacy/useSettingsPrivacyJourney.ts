import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  initialSettingsOrchestrationState,
  settingsOrchestrationReducer,
  type SettingsOrchestrationState,
} from "@/contracts/settingsOrchestrationContract";
import type { AppConfig, ProviderPrivacyBoundarySummary, ViewModelEnvelope } from "@/tauri";
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
  settingsProductActions,
  unknownDraftBoundaryEnvelope,
  validateSettingsDraft,
  type SettingsDraftValidation,
  type SettingsTestPresentation,
} from "./settingsPrivacyPresentation";

type Announce = (message: string) => void;

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
  validation: SettingsDraftValidation;
  actions: ReturnType<typeof settingsProductActions>;
  effectiveBoundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  testConfirmationOpen: boolean;
  load: (announceResult?: boolean) => Promise<SettingsPrivacySnapshot>;
  ensureLoaded: () => void;
  edit: (edit: SettingsDraftEdit) => void;
  requestTest: () => void;
  confirmTest: () => void;
  cancelTest: () => void;
  save: () => void;
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
  const requestRef = useRef(0);
  const operationRef = useRef<"test" | "save" | null>(null);

  useEffect(() => {
    requestRef.current += 1;
    operationRef.current = null;
    setSnapshot(null);
    setDraft(null);
    setLoading(false);
    setLastTestOutcome(null);
    setTestConfirmationOpen(false);
    dispatch({ type: "reset" });
    return () => {
      requestRef.current += 1;
    };
  }, [dataSource]);

  const load = useCallback(
    async (announceResult = true): Promise<SettingsPrivacySnapshot> => {
      const requestId = ++requestRef.current;
      setLoading(true);
      let next: SettingsPrivacySnapshot;
      try {
        next = dataSource
          ? await dataSource.loadSettingsPrivacy()
          : buildSettingsPrivacyErrorSnapshot("settings_privacy_data_source_unavailable");
      } catch (error) {
        next = buildSettingsPrivacyErrorSnapshot(error);
      }
      if (requestId === requestRef.current) {
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
    },
    [announce, dataSource]
  );

  const ensureLoaded = useCallback(() => {
    if (!snapshot && !loading) void load(false);
  }, [load, loading, snapshot]);

  const edit = useCallback(
    (change: SettingsDraftEdit) => {
      if (!draft || operationRef.current) {
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
    [announce, draft, snapshot?.config]
  );

  const validation = useMemo(() => validateSettingsDraft(draft), [draft]);
  const actions = useMemo(() => settingsProductActions(state, validation), [state, validation]);

  const executeTest = useCallback(async () => {
    if (!dataSource || !draft || operationRef.current || !actions.test.enabled) return;
    operationRef.current = "test";
    dispatch({ type: "test_requested" });
    setLastTestOutcome(null);
    announce("正在验证这一份设置草稿；测试不会保存任何配置。");
    try {
      const outcome = await dataSource.testProviderConnection(cloneSettingsConfig(draft));
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
        announce("本次连接验证已有可信回执；设置仍未保存。");
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
      dispatch({ type: "test_failed", errorCode: errorCode(error) });
      announce("连接测试命令失败；当前配置没有可用性证明。");
    } finally {
      operationRef.current = null;
    }
  }, [actions.test.enabled, announce, dataSource, draft]);

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
    if (!dataSource || !draft || operationRef.current || !actions.save.enabled) {
      announce(`当前不能保存：${actions.save.disabledReason ?? "设置草稿不可用。"}`);
      return;
    }
    const submitted = cloneSettingsConfig(draft);
    operationRef.current = "save";
    dispatch({ type: "save_requested" });
    announce("正在保存设置；命令返回不代表模型边界已经确认。");
    void (async () => {
      try {
        await dataSource.saveSettings(submitted);
        dispatch({ type: "save_succeeded" });
        announce("设置命令已返回，正在重新读取配置与模型传输边界。");
        let refreshed: SettingsPrivacySnapshot;
        try {
          refreshed = await dataSource.loadSettingsPrivacy();
        } catch (error) {
          refreshed = buildSettingsPrivacyErrorSnapshot(error);
        }
        setSnapshot(refreshed);
        if (
          !refreshed.config ||
          refreshed.boundaryEnvelope.status !== "ready" ||
          !refreshed.boundaryEnvelope.data
        ) {
          dispatch({
            type: "boundary_refresh_failed",
            errorCode: `settings_refresh_${refreshed.boundaryEnvelope.status}`,
          });
          announce("设置命令已返回，但配置或边界没有完成核对；当前保持未知。");
          return;
        }
        setDraft(cloneSettingsConfig(refreshed.config));
        dispatch({ type: "boundary_refreshed", boundary: refreshed.boundaryEnvelope.data });
        const boundary = refreshed.boundaryEnvelope.data;
        const known =
          boundary.routeType !== "unknown" &&
          boundary.externalTransmission !== "unknown" &&
          boundary.risk !== "unknown";
        announce(
          known
            ? "保存后的配置与模型传输边界已经由后端重新确认。"
            : "设置已保存，但后端返回的模型传输边界仍未知；当前不显示本地确定态。"
        );
      } catch (error) {
        dispatch({ type: "save_failed", errorCode: errorCode(error) });
        announce("设置保存失败；草稿仍保留，当前产品边界没有改变。");
      } finally {
        operationRef.current = null;
      }
    })();
  }, [actions.save, announce, dataSource, draft]);

  const hasUnsavedDraft = state.draftRevision !== state.savedRevision;
  const effectiveBoundaryEnvelope = useMemo(() => {
    if (!snapshot) return loadingBoundaryEnvelope();
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
  }, [hasUnsavedDraft, snapshot, state]);

  return {
    snapshot,
    draft,
    state,
    loading,
    lastTestOutcome,
    testPresentation: connectionTestPresentation(lastTestOutcome?.result ?? null),
    validation,
    actions,
    effectiveBoundaryEnvelope,
    testConfirmationOpen,
    load,
    ensureLoaded,
    edit,
    requestTest,
    confirmTest,
    cancelTest,
    save,
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
