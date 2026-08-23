import { useEffect } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CredentialBootstrapStatus } from "@/tauri";
import type { SettingsDataSource, SettingsSnapshot } from "./settingsDataSource";
import { SettingsView } from "./SettingsView";
import { useSettingsController } from "./useSettingsController";

function SafeModeSettings({
  source,
  surface = "model-provider",
}: {
  source: SettingsDataSource;
  surface?: "model-provider" | "privacy-network" | "diagnostics";
}) {
  const controller = useSettingsController(source, vi.fn());
  useEffect(() => {
    void controller.ensureLoaded();
  }, [controller.ensureLoaded]);
  return (
    <SettingsView
      controller={controller}
      surface={surface}
      onOpenReview={vi.fn()}
      onOpenInspector={vi.fn()}
    />
  );
}

function credentialSettingsSource(
  status: CredentialBootstrapStatus | null,
  safeModeActive = false
): SettingsDataSource {
  return {
    loadSettings: vi.fn().mockResolvedValue({
      config: {
        llm: {
          provider: "custom",
          openai_base: "http://127.0.0.1:11434/v1",
          openai_key: "***",
          embedding_model: "local",
          chat_model: "local",
        },
        prefer_local_model: true,
        local_model: "local",
      },
      boundaryEnvelope: {
        data: null,
        status: "empty",
        lastUpdatedAt: null,
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: {
        active: safeModeActive,
        reason: safeModeActive ? "credential_initialization_required" : "",
        sourceRefs: [],
      },
      credentialBootstrap:
        status === null
          ? null
          : {
              version: "credential_bootstrap_v1",
              digest: "a".repeat(64),
              purposes: [
                { purpose: "canonical_task_receipts", status },
                { purpose: "mcp_audit", status },
                { purpose: "provider_api_key", status },
                { purpose: "search_provider_api_key", status },
              ],
            },
      diagnostics: [
        { id: "sanitized_config", status: "loaded" },
        { id: "provider_privacy_boundary", status: "loaded" },
        { id: "life_state_projection", status: "loaded" },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    }),
    initializeRequiredCredentials: vi.fn().mockResolvedValue({
      items: [],
      initializationCompletedForRestart: true,
      restartRequired: true,
      cleanupStatus: "not_required",
      bootstrapSnapshotDigest: "a".repeat(64),
    }),
    testProviderConnection: vi.fn(),
    saveSettings: vi.fn(),
  };
}

describe("SettingsView", () => {
  it("offers one global Agent Memory setting", async () => {
    const source = credentialSettingsSource(null);
    const snapshot = await source.loadSettings();
    source.loadSettings = vi.fn().mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        system: { ...snapshot.config?.system, agent_memory_enabled: true },
      },
    });

    render(<SafeModeSettings source={source} surface="privacy-network" />);

    const memorySwitch = await screen.findByRole("switch", {
      name: "允许 OpenLife 使用 Agent 记忆",
    });
    expect(memorySwitch).toHaveAttribute("aria-checked", "true");
    fireEvent.click(memorySwitch);
    expect(memorySwitch).toHaveAttribute("aria-checked", "false");
  });

  it("renders only canonical product diagnostics and reports blockers explicitly", async () => {
    const source = credentialSettingsSource(null);
    const snapshot = await source.loadSettings();
    source.loadSettings = vi.fn().mockResolvedValue({
      ...snapshot,
      productDiagnostics: {
        generatedAt: "2026-08-14T00:00:00Z",
        status: "degraded",
        appVersion: "0.1.0",
        runtimeBuild: {
          profile: "qa",
          gitSha: "abc123",
          buildTime: "2026-08-14T00:00:00Z",
          currentExe: "/Applications/OpenLife.app/Contents/MacOS/openlife-tauri",
          binaryKind: "release_bundle",
          frontendMode: "bundled_dist",
          devUrl: "",
          frontendDist: "frontend/dist",
          dataDir: "/tmp/openlife",
          devExtensionsEnabled: false,
          arbitraryMcpRegistrationEnabled: false,
          bundleIdentifier: "ai.openlife.desktop",
          productName: "OpenLife",
        },
        persistenceMode: "read_only_degraded",
        canonicalWritesAllowed: false,
        providerDispatchAllowed: false,
        toolDispatchAllowed: false,
        stores: [
          { store: "ConversationStore", status: "read_write_canonical" },
          {
            store: "CanonicalTaskRuntimeStore",
            status: "read_only_canonical",
            reasonCode: "runtime_io_failure",
          },
        ],
        counts: { projectCount: 1, conversationCount: 2, taskCount: 3, activeTaskCount: 1 },
        credentialBootstrap: { version: "v1", digest: "digest", purposes: [] },
        blockerCodes: ["store:CanonicalTaskRuntimeStore:runtime_io_failure"],
      },
    });

    const diagnosticsView = render(<SafeModeSettings source={source} surface="diagnostics" />);

    expect(await screen.findByRole("heading", { name: "产品诊断" })).toBeInTheDocument();
    expect(screen.getByText("部分能力降级")).toBeInTheDocument();
    expect(screen.getByText("ai.openlife.desktop")).toBeInTheDocument();
    expect(screen.getByText("隔离的本地 profile 文件（0600）")).toBeInTheDocument();
    expect(
      screen.getAllByText("操作没有完成。你可以重试，或在详情中查看技术信息。").length
    ).toBeGreaterThan(0);
    expect(
      screen.queryByText("store:CanonicalTaskRuntimeStore:runtime_io_failure")
    ).not.toBeInTheDocument();
    diagnosticsView.unmount();
    render(<SafeModeSettings source={source} />);
    expect(await screen.findByText("开发凭据与正式产品隔离")).toBeInTheDocument();
    expect(screen.getByText(/不会从正式 profile 自动复制/)).toBeInTheDocument();
  });

  it("shows explicit Search Provider controls and fail-closed artifact output state", async () => {
    const source = credentialSettingsSource(null);
    source.loadSettings = vi.fn().mockResolvedValue({
      ...(await source.loadSettings()),
      config: {
        ...((await source.loadSettings()).config ?? {}),
        llm: {
          provider: "deepseek",
          openai_base: "https://api.deepseek.com",
          openai_key: "***",
          embedding_model: "local",
          chat_model: "deepseek-chat",
        },
        prefer_local_model: false,
        local_model: "local",
        system: { search_provider: "duckduckgo", safe_paths: [] },
      },
    });

    const model = render(<SafeModeSettings source={source} />);
    expect(await screen.findByRole("heading", { name: "网页搜索" })).toBeInTheDocument();
    expect(screen.getByLabelText("Search Provider")).toHaveValue("duckduckgo");
    expect(screen.getByText(/不保证可用/)).toBeInTheDocument();
    model.unmount();

    render(<SafeModeSettings source={source} surface="privacy-network" />);
    expect(await screen.findByRole("heading", { name: "Artifact 输出目录" })).toBeInTheDocument();
    expect(screen.getByText(/生成 artifact 时会被系统明确阻止/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "选择输出文件夹" })).toBeEnabled();
  });

  it("renders the sanitized native config shape when the secret field is omitted", async () => {
    const snapshot: SettingsSnapshot = {
      config: {
        llm: {
          provider: "openai",
          openai_base: "https://api.openai.com/v1",
          openai_key_ref: "keychain://com.openlife.desktop/provider-api-key",
          embedding_model: "text-embedding-3-small",
          chat_model: "gpt-4o-mini",
        },
        prefer_local_model: false,
        local_model: "qwen2.5:14b",
      },
      boundaryEnvelope: {
        data: null,
        status: "error",
        lastUpdatedAt: null,
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: { active: false, reason: "", sourceRefs: [] },
      diagnostics: [
        { id: "sanitized_config", status: "loaded" },
        { id: "provider_privacy_boundary", status: "loaded" },
        { id: "life_state_projection", status: "loaded" },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    const rendered = render(<SafeModeSettings source={source} />);

    expect(await screen.findByRole("heading", { name: "模型与传输边界" })).toBeInTheDocument();
    expect(screen.getByLabelText("API 凭据")).toHaveValue("");
    expect(screen.getByText(/系统返回遮罩凭据/)).toBeInTheDocument();

    rendered.unmount();
    const initializationSource = credentialSettingsSource("initialization_required");
    render(<SafeModeSettings source={initializationSource} />);
    const action = await screen.findByRole("button", { name: "初始化系统凭据" });
    expect(initializationSource.initializeRequiredCredentials).not.toHaveBeenCalled();
    fireEvent.click(action);
    expect(initializationSource.initializeRequiredCredentials).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("初始化完成，需要重启")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "等待重启" })).toBeDisabled();
  });

  it("does not infer credential recovery eligibility from generic Safe Mode", async () => {
    const snapshot: SettingsSnapshot = {
      config: null,
      boundaryEnvelope: {
        data: null,
        status: "error",
        lastUpdatedAt: null,
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: {
        active: true,
        reason: "credential_store_unavailable",
        sourceRefs: ["safe-mode:credential-store"],
      },
      diagnostics: [
        { id: "sanitized_config", status: "failed", message: "config unavailable" },
        { id: "provider_privacy_boundary", status: "failed", message: "boundary unavailable" },
        { id: "life_state_projection", status: "loaded" },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    const rendered = render(<SafeModeSettings source={source} />);

    expect(await screen.findByText("安全模式保持生效")).toBeInTheDocument();
    expect(screen.getByText("设置暂不可用")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "检查系统凭据" })).not.toBeInTheDocument();
    expect(screen.getByText(/不会从自由文本原因推导/)).toBeInTheDocument();

    rendered.unmount();
    for (const status of [
      "available",
      "missing_existing_data",
      "invalid",
      "unknown",
      null,
    ] satisfies Array<CredentialBootstrapStatus | null>) {
      const blockedSource = credentialSettingsSource(status);
      const blocked = render(<SafeModeSettings source={blockedSource} />);
      expect(await screen.findByRole("heading", { name: "模型与传输边界" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "初始化系统凭据" })).not.toBeInTheDocument();
      expect(blockedSource.initializeRequiredCredentials).not.toHaveBeenCalled();
      blocked.unmount();
    }
  });

  it("offers typed credential access recovery for unavailable bootstrap purposes", async () => {
    const source = credentialSettingsSource("unavailable", true);
    render(<SafeModeSettings source={source} />);

    const action = await screen.findByRole("button", { name: "恢复凭据访问" });
    expect(screen.getByRole("heading", { name: "凭据访问恢复" })).toBeInTheDocument();
    expect(screen.getByText(/不创建、不覆盖且不返回密钥/)).toBeInTheDocument();
    fireEvent.click(action);

    expect(source.initializeRequiredCredentials).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("已请求访问，等待重启验证")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "等待重启" })).toBeDisabled();
  });

  it("binds a mixed credential snapshot to the unavailable recovery subset only", async () => {
    const source = credentialSettingsSource("unavailable", true);
    const snapshot = await source.loadSettings();
    source.loadSettings = vi.fn().mockResolvedValue({
      ...snapshot,
      credentialBootstrap: {
        ...snapshot.credentialBootstrap!,
        purposes: [
          { purpose: "canonical_task_receipts", status: "unavailable" },
          { purpose: "mcp_audit", status: "initialization_required" },
          { purpose: "provider_api_key", status: "unavailable" },
          { purpose: "search_provider_api_key", status: "initialization_required" },
        ],
      },
    });

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByRole("heading", { name: "凭据访问恢复" })).toBeInTheDocument();
    expect(screen.getByText(/系统确认有 2 类既有凭据需要恢复访问/)).toBeInTheDocument();
    expect(screen.queryByText(/5 类既有凭据需要恢复访问/)).not.toBeInTheDocument();
  });

  it("does not contradict an explicit credential initialization eligibility", async () => {
    const source = credentialSettingsSource("initialization_required", true);
    render(<SafeModeSettings source={source} />);

    expect(await screen.findByText("安全模式保持生效")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "初始化系统凭据" })).toBeEnabled();
    expect(screen.getByText(/下方只开放系统启动快照明确列出的/)).toBeInTheDocument();
    expect(screen.queryByText(/当前读模型没有提供凭据恢复资格/)).not.toBeInTheDocument();
  });

  it("shows unknown protection and closes actions when LifeStateProjection is unavailable", async () => {
    const snapshot: SettingsSnapshot = {
      config: {
        llm: {
          provider: "custom",
          openai_base: "http://127.0.0.1:11434/v1",
          openai_key: "***",
          embedding_model: "nomic-embed-text",
          chat_model: "qwen2.5:14b",
        },
        prefer_local_model: true,
        local_model: "qwen2.5:14b",
      },
      boundaryEnvelope: {
        data: {
          routeType: "local",
          externalTransmission: "not_sent",
          providerLabel: "本机模型服务",
          modelLabel: "qwen2.5:14b",
          privacyLabel: "仅本机处理",
          risk: "none",
          localOnlyRequired: true,
          evidenceRefs: [],
        },
        status: "ready",
        lastUpdatedAt: "2026-07-22T00:00:00Z",
        source: "backend-readmodel",
        evidenceRefs: [],
        warnings: [],
        actions: { primary: [], review: [], debugOnly: [] },
      },
      safeMode: null,
      diagnostics: [
        { id: "sanitized_config", status: "loaded" },
        { id: "provider_privacy_boundary", status: "loaded" },
        {
          id: "life_state_projection",
          status: "failed",
          message: "projection unavailable",
        },
        { id: "review_item_resolution", status: "not_requested" },
      ],
    };
    const source: SettingsDataSource = {
      loadSettings: vi.fn().mockResolvedValue(snapshot),
      testProviderConnection: vi.fn(),
      saveSettings: vi.fn(),
    };

    render(<SafeModeSettings source={source} />);

    expect(await screen.findByText("保护状态未知")).toBeInTheDocument();
    expect(screen.getByText(/LifeStateProjection 没有提供可核对的保护状态/)).toBeInTheDocument();
    expect(screen.getByText("是否外传未知")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "测试连接" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存设置" })).toBeDisabled();
  });
});
