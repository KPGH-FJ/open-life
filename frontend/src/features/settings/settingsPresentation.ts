import type {
  AppConfig,
  CloudProviderId,
  ProductAction,
  ProviderConnectionsViewModel,
  ProviderPrivacyBoundarySummary,
  ViewModelEnvelope,
} from "@/tauri";
import type { SettingsOrchestrationState } from "@/contracts/settingsOrchestrationContract";

export type SettingsSurfaceId = "model-provider" | "privacy-network" | "diagnostics";

export const settingsBoundaryLabels = {
  routeType: {
    local: "本机路由",
    cloud: "外部供应商",
    hybrid: "本机与外部组合",
    auto: "由系统自动选择",
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

export const settingsProviderLabels: Record<CloudProviderId, string> = {
  deepseek: "DeepSeek",
  openai: "OpenAI",
  openrouter: "OpenRouter",
  gemini: "Google Gemini",
  siliconflow: "SiliconFlow",
  moonshot: "Moonshot / Kimi",
  dashscope: "DashScope",
  zhipu: "智谱 AI",
  custom: "自定义 OpenAI 兼容服务",
};

export const settingsProviderOptions = Object.entries(settingsProviderLabels).map(
  ([value, label]) => ({
    value: value as CloudProviderId,
    label,
  })
);

export const settingsSearchProviderLabels: Record<
  NonNullable<NonNullable<AppConfig["system"]>["search_provider"]>,
  string
> = {
  auto: "自动（使用当前模型路由）",
  duckduckgo: "DuckDuckGo（无需凭据，可能遇到挑战页）",
  deepseek: "DeepSeek Web Search",
  brave: "Brave Search API",
  searxng: "SearXNG",
};

export const settingsSearchProviderOptions = Object.entries(settingsSearchProviderLabels).map(
  ([value, label]) => ({
    value: value as NonNullable<NonNullable<AppConfig["system"]>["search_provider"]>,
    label,
  })
);

function cloneNetworkPolicy(config: AppConfig): AppConfig["system"] {
  if (!config.system) return undefined;
  return {
    ...config.system,
    additional_read_roots: config.system.additional_read_roots
      ? [...config.system.additional_read_roots]
      : undefined,
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
    system: cloneNetworkPolicy(config),
  };
}

export function searchProviderIdentity(config: AppConfig): string {
  const provider = (config.system?.search_provider ?? "auto").trim().toLowerCase();
  if (provider !== "searxng") return provider;
  const endpoint = config.system?.searxng_url?.trim() ?? "";
  try {
    return `${provider}|${new URL(endpoint).toString()}`;
  } catch {
    return `${provider}|${endpoint}`;
  }
}

export function searchCredentialState(config: AppConfig): "stored" | "entered" | "missing" {
  const credential = config.system?.search_provider_key?.trim() ?? "";
  if (credential && credential !== "***") return "entered";
  if (credential === "***" || Boolean(config.system?.search_provider_key_ref?.trim())) {
    return "stored";
  }
  return "missing";
}

export function selectedHostedSearchRoute(
  viewModel: ProviderConnectionsViewModel | null,
  configuredSearch: string
) {
  const configured = configuredSearch.trim().toLowerCase();
  const selectedConnection = viewModel?.connections.find(connection =>
    connection.models.some(model => model.selected)
  );
  if (!selectedConnection || selectedConnection.credentialState !== "stored") return null;
  const selectedModel = selectedConnection.models.find(model => model.selected);
  if (!selectedModel || selectedModel.validationState !== "ready") return null;
  const selectedProvider = selectedConnection.providerId.trim().toLowerCase();
  if (
    !["deepseek", "openrouter"].includes(selectedProvider) ||
    (configured !== "auto" && configured !== selectedProvider)
  ) {
    return null;
  }
  try {
    const url = new URL(selectedConnection.endpoint.trim());
    if (
      url.protocol !== "https:" ||
      (url.port && url.port !== "443") ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    ) {
      return null;
    }
    if (selectedProvider === "deepseek") {
      return url.hostname.toLowerCase() === "api.deepseek.com" &&
        ["", "/v1"].includes(url.pathname.replace(/\/$/, ""))
        ? selectedConnection
        : null;
    }
    return url.hostname.toLowerCase() === "openrouter.ai" &&
      url.pathname.replace(/\/$/, "") === "/api/v1"
      ? selectedConnection
      : null;
  } catch {
    return null;
  }
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
  _previousConfig: AppConfig,
  savedDraft: AppConfig,
  refreshedConfig: AppConfig
): boolean {
  const attestableConfig = (config: AppConfig) => {
    const system = config.system ? { ...config.system } : undefined;
    if (system) {
      delete system.search_provider_key;
      delete system.search_provider_key_ref;
    }
    return {
      ...config,
      ...(system
        ? {
            system: {
              ...system,
              searchCredentialPresence:
                searchCredentialState(config) === "missing" ? "missing" : "present",
            },
          }
        : {}),
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

export type SettingsDraftValidation = {
  canSave: boolean;
  saveDisabledReason?: string;
};

export function validateSettingsDraft(
  config: AppConfig | null,
  providerConnections: ProviderConnectionsViewModel | null = null
): SettingsDraftValidation {
  if (!config) {
    return {
      canSave: false,
      saveDisabledReason: "尚未读取到系统配置。",
    };
  }
  if (config.prefer_local_model && !config.local_model.trim()) {
    return {
      canSave: false,
      saveDisabledReason: "启用本地优先时必须填写本地模型。",
    };
  }
  const searchProvider = config.system?.search_provider ?? "auto";
  if (
    (searchProvider === "deepseek" || searchProvider === "brave") &&
    searchCredentialState(config) === "missing" &&
    !selectedHostedSearchRoute(providerConnections, searchProvider)
  ) {
    return {
      canSave: false,
      saveDisabledReason: "当前网页搜索供应商需要单独的搜索凭据。",
    };
  }
  if (searchProvider === "searxng" && !endpointHost(config.system?.searxng_url ?? "")) {
    return {
      canSave: false,
      saveDisabledReason: "SearXNG 地址必须是完整的 HTTP 或 HTTPS 地址。",
    };
  }
  return { canSave: true };
}

export function settingsProductActions(
  state: SettingsOrchestrationState,
  validation: SettingsDraftValidation
): { save: ProductAction } {
  const busy = ["saving", "refreshing_boundary"].includes(state.phase);
  const saveablePhase = state.phase === "dirty" && state.draftRevision !== state.savedRevision;
  const saveEnabled = validation.canSave && saveablePhase && !busy;
  return {
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
                ? "当前没有可保存的更改。"
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
      providerLabel: "待系统确认",
      modelLabel: "待系统确认",
      privacyLabel: "设置草稿尚未成为系统边界事实",
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
        ? "系统安全模式仍在生效"
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
