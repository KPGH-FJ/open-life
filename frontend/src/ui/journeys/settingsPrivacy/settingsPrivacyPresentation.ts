import type {
  AppConfig,
  LlmConnectionTestResult,
  ProductAction,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import type { SettingsOrchestrationState } from "@/contracts/settingsOrchestrationContract";
import type { FoundationStatus } from "@/ui/foundation";

export type SettingsPrivacySurfaceId = "model-provider" | "privacy-network";

export const settingsBoundaryLabels = {
  routeType: {
    local: "本机路由",
    cloud: "外部供应商",
    hybrid: "本机与外部组合",
    auto: "由后端自动选择",
    unknown: "尚未确认",
  },
  externalTransmission: {
    not_sent: "未发送到外部",
    possible: "可能发送到外部",
    sent: "已发送到外部",
    unknown: "尚未确认",
  },
  risk: {
    none: "无已知外部传输风险",
    low: "低风险",
    medium: "需要注意",
    high: "高风险",
    critical: "严重风险",
    unknown: "尚未确认",
  },
} as const satisfies {
  routeType: Record<ProviderPrivacyBoundarySummary["routeType"], string>;
  externalTransmission: Record<ProviderPrivacyBoundarySummary["externalTransmission"], string>;
  risk: Record<ProviderPrivacyBoundarySummary["risk"], string>;
};

export const settingsProviderLabels: Record<NonNullable<AppConfig["llm"]["provider"]>, string> = {
  deepseek: "DeepSeek",
  openai: "OpenAI",
  openrouter: "OpenRouter",
  siliconflow: "SiliconFlow",
  moonshot: "Moonshot / Kimi",
  dashscope: "DashScope",
  zhipu: "智谱 AI",
  custom: "自定义 OpenAI 兼容服务",
};

export const settingsProviderOptions = Object.entries(settingsProviderLabels).map(
  ([value, label]) => ({
    value: value as NonNullable<AppConfig["llm"]["provider"]>,
    label,
  })
);

function cloneNetworkPolicy(config: AppConfig): AppConfig["system"] {
  if (!config.system) return undefined;
  return {
    ...config.system,
    safe_paths: config.system.safe_paths ? [...config.system.safe_paths] : undefined,
    network_policy: config.system.network_policy
      ? {
          ...config.system.network_policy,
          domain_allowlist: config.system.network_policy.domain_allowlist
            ? [...config.system.network_policy.domain_allowlist]
            : undefined,
          domain_denylist: config.system.network_policy.domain_denylist
            ? [...config.system.network_policy.domain_denylist]
            : undefined,
          tool_overrides: config.system.network_policy.tool_overrides
            ? { ...config.system.network_policy.tool_overrides }
            : undefined,
        }
      : undefined,
  };
}

export function cloneSettingsConfig(config: AppConfig): AppConfig {
  return {
    ...config,
    llm: { ...config.llm },
    system: cloneNetworkPolicy(config),
  };
}

export function providerIdentity(config: AppConfig): string {
  const provider = (config.llm.provider ?? "unknown").trim().toLocaleLowerCase("en-US");
  const endpoint = config.llm.openai_base.trim();
  try {
    return `${provider}|${new URL(endpoint).toString()}`;
  } catch {
    return `${provider}|${endpoint}`;
  }
}

export function hasUsableCredential(config: AppConfig): boolean {
  return credentialState(config) !== "missing";
}

export function credentialState(config: AppConfig): "stored" | "entered" | "missing" {
  const credential = config.llm.openai_key?.trim() ?? "";
  if (credential && credential !== "***") return "entered";
  if (credential === "***" || Boolean(config.llm.openai_key_ref?.trim())) return "stored";
  return "missing";
}

function canonicalizeForComparison(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalizeForComparison);
  if (value && typeof value === "object") {
    return Object.keys(value as Record<string, unknown>)
      .sort()
      .reduce<Record<string, unknown>>((result, key) => {
        const entry = (value as Record<string, unknown>)[key];
        if (entry !== undefined) result[key] = canonicalizeForComparison(entry);
        return result;
      }, {});
  }
  return value;
}

export function settingsConfigMatchesSavedDraft(
  previousConfig: AppConfig,
  savedDraft: AppConfig,
  refreshedConfig: AppConfig
): boolean {
  const previousCredentialVersion = previousConfig.llm.credential_version;
  const savedCredentialVersion = savedDraft.llm.credential_version;
  const refreshedCredentialVersion = refreshedConfig.llm.credential_version;
  if (
    previousCredentialVersion === undefined ||
    savedCredentialVersion !== previousCredentialVersion ||
    refreshedCredentialVersion === undefined
  ) {
    return false;
  }
  const credentialGenerationMustAdvance =
    providerIdentity(savedDraft) !== providerIdentity(previousConfig) ||
    credentialState(savedDraft) === "entered";
  const expectedCredentialVersion =
    previousCredentialVersion + (credentialGenerationMustAdvance ? 1 : 0);
  if (refreshedCredentialVersion !== expectedCredentialVersion) return false;

  const attestableConfig = (config: AppConfig) => {
    const llm = { ...config.llm };
    // `save_config` canonicalizes DeepSeek to chat-only because the provider
    // does not use OpenAI's embedding endpoint. Apply that exact backend-owned
    // rule before attestation so a successful save is not reported as unknown.
    if (llm.provider === "deepseek") llm.embedding_enabled = false;
    delete llm.openai_key;
    delete llm.openai_key_ref;
    delete llm.credential_version;
    return {
      ...config,
      llm: {
        ...llm,
        credentialPresence: credentialState(config) === "missing" ? "missing" : "present",
      },
    };
  };

  return (
    JSON.stringify(canonicalizeForComparison(attestableConfig(savedDraft))) ===
    JSON.stringify(canonicalizeForComparison(attestableConfig(refreshedConfig)))
  );
}

export function endpointHost(endpoint: string): string | null {
  try {
    const url = new URL(endpoint);
    return ["http:", "https:"].includes(url.protocol) && url.host ? url.host : null;
  } catch {
    return null;
  }
}

export function endpointMayTransmitExternally(endpoint: string): boolean {
  try {
    const hostname = new URL(endpoint).hostname.toLocaleLowerCase("en-US");
    return !(
      hostname === "localhost" ||
      hostname === "127.0.0.1" ||
      hostname === "::1" ||
      hostname.endsWith(".localhost")
    );
  } catch {
    return true;
  }
}

export type SettingsDraftValidation = {
  canSave: boolean;
  saveDisabledReason?: string;
  canTest: boolean;
  testDisabledReason?: string;
  endpointHost: string | null;
  mayTransmitExternally: boolean;
};

export function validateSettingsDraft(config: AppConfig | null): SettingsDraftValidation {
  if (!config) {
    return {
      canSave: false,
      saveDisabledReason: "尚未读取到后端配置。",
      canTest: false,
      testDisabledReason: "尚未读取到后端配置。",
      endpointHost: null,
      mayTransmitExternally: true,
    };
  }
  if (!config.llm.provider) {
    return {
      canSave: false,
      saveDisabledReason: "请选择模型供应商。",
      canTest: false,
      testDisabledReason: "请选择模型供应商。",
      endpointHost: null,
      mayTransmitExternally: true,
    };
  }
  const host = endpointHost(config.llm.openai_base);
  if (!host) {
    return {
      canSave: false,
      saveDisabledReason: "API 地址必须是完整的 HTTP 或 HTTPS 地址。",
      canTest: false,
      testDisabledReason: "API 地址必须是完整的 HTTP 或 HTTPS 地址。",
      endpointHost: null,
      mayTransmitExternally: true,
    };
  }
  if (!config.llm.chat_model.trim()) {
    return {
      canSave: false,
      saveDisabledReason: "请填写要使用的模型。",
      canTest: false,
      testDisabledReason: "请填写要验证的模型。",
      endpointHost: host,
      mayTransmitExternally: endpointMayTransmitExternally(config.llm.openai_base),
    };
  }
  if (config.prefer_local_model && !config.local_model.trim()) {
    return {
      canSave: false,
      saveDisabledReason: "启用本地优先时必须填写本地模型。",
      canTest: false,
      testDisabledReason: "先补充本地模型配置。",
      endpointHost: host,
      mayTransmitExternally: endpointMayTransmitExternally(config.llm.openai_base),
    };
  }
  const external = endpointMayTransmitExternally(config.llm.openai_base);
  return {
    canSave: true,
    canTest: hasUsableCredential(config),
    ...(hasUsableCredential(config)
      ? {}
      : { testDisabledReason: "请填写 API 凭据；当前不会发起连接。" }),
    endpointHost: host,
    mayTransmitExternally: external,
  };
}

export type SettingsTestPresentation = {
  label: string;
  detail: string;
  status: FoundationStatus;
  verified?: boolean;
};

export function connectionTestPresentation(
  result: LlmConnectionTestResult | null
): SettingsTestPresentation | null {
  if (!result) return null;
  const receipt = result.provider_invocation_receipt;
  if (
    result.ok &&
    result.validation_status === "validated" &&
    receipt?.status === "completed" &&
    !receipt.simulated
  ) {
    return {
      label: "本次连接验证成功",
      detail: "只证明这一次精确的供应商请求；设置尚未因此保存。",
      status: "success",
      verified: true,
    };
  }
  if (result.ok) {
    return {
      label: "连接结果证据不完整",
      detail: "返回值表示成功，但缺少可信的非模拟适配器终态；当前不显示可用。",
      status: "unknown",
    };
  }
  if (result.validation_status === "consent_required") {
    return {
      label: "需要先确认本次外部连接",
      detail: "请求尚未发送。审核决定只授权一次精确请求，批准后仍需重新测试。",
      status: "waiting",
    };
  }
  if (result.validation_status === "remote_unknown" || receipt?.status === "remote_unknown") {
    return {
      label: "外部结果未知",
      detail: "请求可能已经到达外部服务；不要自动重试，先核对回执与网络状态。",
      status: "unknown",
    };
  }
  if (result.validation_status === "runtime_generation_incoherent") {
    return {
      label: "运行配置不一致，已保护性关闭",
      detail: "网络请求没有继续；需要先恢复后端配置与执行代一致性。",
      status: "error",
    };
  }
  if (result.validation_status === "blocked" || result.consent_status === "blocked") {
    return {
      label: "网络策略已阻止测试",
      detail: "没有建立供应商可用性证明。请先核对当前网络策略。",
      status: "blocked",
    };
  }
  if (receipt?.status === "failed") {
    return {
      label: "供应商明确返回失败",
      detail: "本次请求失败；当前配置不能标记为可用。",
      status: "error",
    };
  }
  return {
    label: "尚未建立连接证据",
    detail: "连接没有通过；以返回说明为准，当前配置不显示可用。",
    status: "blocked",
  };
}

export function settingsProductActions(
  state: SettingsOrchestrationState,
  validation: SettingsDraftValidation
): { test: ProductAction; save: ProductAction } {
  const busy = ["testing", "saving", "refreshing_boundary"].includes(state.phase);
  const testEnabled = validation.canTest && !busy;
  const saveablePhase =
    (state.phase === "dirty" || state.phase === "tested") &&
    state.draftRevision !== state.savedRevision;
  const saveEnabled = validation.canSave && saveablePhase && !busy;
  return {
    test: {
      id: "settings.provider.test_connection",
      label: "测试连接",
      kind: "configure",
      enabled: testEnabled,
      ...(!testEnabled
        ? {
            disabledReason: busy
              ? "已有设置操作正在进行。"
              : (validation.testDisabledReason ?? "当前配置不能测试连接。"),
          }
        : {}),
      targetRef: `settings-draft:${state.draftRevision}`,
    },
    save: {
      id: "settings.provider.save_config",
      label: "保存设置",
      kind: "configure",
      enabled: saveEnabled,
      ...(!saveEnabled
        ? {
            disabledReason: busy
              ? "已有设置操作正在进行。"
              : !saveablePhase
                ? state.phase === "failed"
                  ? "测试失败后请先修改配置，再决定是否保存。"
                  : "当前没有可保存的更改。"
                : (validation.saveDisabledReason ?? "当前配置不能保存。"),
          }
        : {}),
      targetRef: "AppConfig",
    },
  };
}

export function unknownDraftBoundaryEnvelope(
  message: string
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data: {
      routeType: "unknown",
      externalTransmission: "unknown",
      providerLabel: "待后端确认",
      modelLabel: "待后端确认",
      privacyLabel: "设置草稿尚未成为后端边界事实",
      risk: "unknown",
      localOnlyRequired: false,
      blockedReason: message,
      evidenceRefs: [],
    },
    status: "ready",
    lastUpdatedAt: null,
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [
      {
        code: "settings.boundary_pending_refresh",
        message,
        severity: "warning",
        evidenceRefs: [],
      },
    ],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

export function unknownSettingsProtectionBoundaryEnvelope(
  message: string,
  protectionState: "active" | "unknown"
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  const envelope = unknownDraftBoundaryEnvelope(message);
  if (envelope.data) {
    envelope.data.privacyLabel =
      protectionState === "active"
        ? "后端安全模式仍在生效"
        : "设置保护状态尚未由 LifeStateProjection 确认";
  }
  envelope.warnings = [
    {
      code:
        protectionState === "active"
          ? "settings.safe_mode_active"
          : "settings.protection_state_unknown",
      message,
      severity: "warning",
      evidenceRefs: [],
    },
  ];
  return envelope;
}
