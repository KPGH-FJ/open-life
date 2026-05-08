import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import SettingsPage from "./SettingsPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  writeTextFile: vi.fn(),
  readTextFile: vi.fn(),
}));

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  const renderSettings = () =>
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>
    );

  const clickTab = async (tabName: string) => {
    const tab = await screen.findByRole("button", { name: tabName });
    fireEvent.click(tab);
  };

  it("renders trial console title and checklist", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("试用控制台")).toBeInTheDocument();
    });

    expect(screen.getByText(/试用路径 Checklist/)).toBeInTheDocument();
    expect(screen.getByText(/试用闭环定义/)).toBeInTheDocument();
    expect(screen.getByText(/核心链路已就绪/)).toBeInTheDocument();

    await clickTab("数据");
    expect(screen.getByText(/导出全部数据/)).toBeInTheDocument();
  });

  it("shows readiness issues when diagnostics reports chat is not ready", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 1,
          memory_chunk_count: 42,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: "llama3",
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: false,
          cloud_provider: "DeepSeek",
          cloud_api_validated: false,
          cloud_api_last_error: null,
          chat_ready: false,
          readiness_issues: ["聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。"],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: ["核心聊天链路未就绪，请先修复试用就绪检查中的问题。"],
          builder_completion: {
            identity: 0,
            goals: 0,
            capabilities: 0,
            state: 0,
            overall: 0,
            lowest_dimension: "identity",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    expect(await screen.findByText(/试用路径 Checklist/)).toBeInTheDocument();
    expect(screen.getAllByText(/聊天不可用/).length).toBeGreaterThanOrEqual(1);
  });

  it("exports data with optional app version metadata", async () => {
    vi.mocked(save).mockResolvedValue("/tmp/openlife-export.json");
    vi.mocked(writeTextFile).mockResolvedValue(undefined);

    renderSettings();

    await clickTab("数据");
    const exportButton = await screen.findByText("导出全部数据");
    fireEvent.click(exportButton);

    await waitFor(() => {
      expect(writeTextFile).toHaveBeenCalled();
    });
    expect(await screen.findByText(/应用版本 0.1.0/)).toBeInTheDocument();
  });

  it("tests the current DeepSeek form config instead of only saved config", async () => {
    renderSettings();

    await clickTab("模型");
    const keyInput = await screen.findByPlaceholderText("sk-...");
    fireEvent.change(keyInput, { target: { value: "sk-deepseek-form" } });
    fireEvent.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("test_llm_connection", {
        config: expect.objectContaining({
          llm: expect.objectContaining({
            provider: "deepseek",
            openai_base: "https://api.deepseek.com",
            openai_key: "sk-deepseek-form",
            chat_model: "deepseek-chat",
            embedding_enabled: false,
          }),
        }),
      });
    });
    expect(await screen.findByText(/DeepSeek: 连接成功/)).toBeInTheDocument();
  });

  it("warns when DeepSeek reasoner is selected and lets the user switch back to deepseek-chat", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_config") {
        return Promise.resolve({
          llm: {
            provider: "deepseek",
            openai_base: "https://api.deepseek.com",
            openai_key: "sk-test",
            embedding_model: "",
            chat_model: "deepseek-reasoner",
            embedding_enabled: false,
          },
          prefer_local_model: false,
          local_model: "",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("模型");
    expect(await screen.findByText(/当前选择的是 DeepSeek 推理模型/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "一键改为 deepseek-chat" }));
    expect(screen.getByDisplayValue("deepseek-chat")).toBeInTheDocument();
  });

  it("explains DeepSeek embedding fallback in settings", async () => {
    renderSettings();

    await clickTab("模型");
    expect(await screen.findByText(/DeepSeek 主要用于聊天/)).toBeInTheDocument();
    expect(screen.getByText(/请先保存当前设置，再去恢复控制台重建向量索引/)).toBeInTheDocument();
  });

  it("shows recovery console when diagnostics reports degraded storage", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          vector_corrupt_embedding_count: 3,
          unfinished_builder_sessions: 1,
          pending_builder_review_sessions: 1,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [
            "应用正在以降级数据模式运行：memory.db 初始化失败，正在使用临时数据库",
          ],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [
            "数据存储曾在启动时降级：请先确认数据目录和数据库状态，再继续深度试用。",
          ],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    expect(await screen.findByText("恢复控制台")).toBeInTheDocument();
    expect(screen.getByText(/向量索引损坏/)).toBeInTheDocument();
    expect(screen.getByText(/Builder 待确认 Review/)).toBeInTheDocument();
    expect(screen.getByText(/语义记忆为空/)).toBeInTheDocument();
    expect(screen.getByText(/先导出完整备份/)).toBeInTheDocument();
  });

  it("shows beta flow steps and follow-up actions", async () => {
    renderSettings();

    expect(await screen.findByText("试用闭环定义")).toBeInTheDocument();
    expect(screen.getByText("1. 完成设置与诊断")).toBeInTheDocument();
    expect(screen.getByText("2. 完成人生模型构建")).toBeInTheDocument();
    expect(screen.getByText("3. 跑通第一次对话")).toBeInTheDocument();
    expect(screen.getByText("4. 查看校准或版本回滚")).toBeInTheDocument();
  });

  it("prioritizes continuing Builder when unfinished builder sessions exist", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 28,
          vector_corrupt_embedding_count: 0,
          unfinished_builder_sessions: 1,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: true,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 0,
            goals: 0,
            capabilities: 0,
            state: 50,
            overall: 12.5,
            lowest_dimension: "identity",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    expect(await screen.findByText(/有 1 个待继续的 Builder 会话/)).toBeInTheDocument();
    expect(screen.getAllByText("继续 Builder").length).toBeGreaterThan(0);
    expect(screen.getByText(/先把 Review 应用掉，比重新开始更合适/)).toBeInTheDocument();
  });

  it("rebuilds vector index from recovery console", async () => {
    vi.stubGlobal(
      "confirm",
      vi.fn(() => true)
    );

    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 42,
          vector_corrupt_embedding_count: 2,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    fireEvent.click(await screen.findByText("重建向量索引"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("rebuild_memory_index", undefined);
    });
    expect(await screen.findByText(/向量索引重建完成/)).toBeInTheDocument();
  });

  it("blocks destructive import action in safe mode", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 42,
          vector_corrupt_embedding_count: 2,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("数据");
    const importButton = await screen.findByRole("button", { name: "导入全部数据" });
    expect(importButton).toBeDisabled();
  });

  it("navigates to provider tab when overview readiness action is clicked", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("模型/Provider 就绪")).toBeInTheDocument();
    });

    const configBtns = screen.getAllByText("配置模型");
    fireEvent.click(configBtns[0]);

    await waitFor(() => {
      expect(screen.getByText("Layer 1 路由状态")).toBeInTheDocument();
    });
  });

  it("navigates to data tab when overview diagnostic export action is clicked", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("诊断导出")).toBeInTheDocument();
    });

    const exportDiagBtns = screen.getAllByText("导出诊断");
    fireEvent.click(exportDiagBtns[0]);

    await waitFor(() => {
      expect(screen.getByText("导出诊断报告")).toBeInTheDocument();
    });
  });

  it("exports diagnostics with whitelisted fields only, no sensitive data leaked", async () => {
    const sentinelApiKey = "sk-top-secret-sentinel-key";
    const sentinelRawMessages = "user: my secret message";
    const sentinelRawMemory = "user personal memory content";
    const sentinelToolOutput = "sensitive tool output data";

    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: true,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 2,
          mcp_tool_count: 5,
          mcp_recent_audit_count: 10,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 100,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: null,
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 3,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 3,
          pending_builder_review_sessions: 0,
          config_source: "file",
          beta_ready: true,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 1.2,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.8,
            mcp_audit_db_exists: true,
            mcp_audit_db_size_mb: 0.1,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [{ name: "llama3", size_mb: 4500 }],
          agent_run_count: 5,
          agent_run_store_status: "ok",
          pending_proposal_count: 0,
          high_risk_pending_proposal_count: 0,
          proposal_store_status: "ok",
          api_key: sentinelApiKey,
          secret: "should-not-leak",
          raw_messages: sentinelRawMessages,
          raw_memory: sentinelRawMemory,
          tool_output: sentinelToolOutput,
        });
      }
      return mockInvoke(cmd, args);
    });

    vi.mocked(save).mockResolvedValue("/tmp/openlife-diagnostics.json");
    vi.mocked(writeTextFile).mockResolvedValue(undefined);

    renderSettings();

    await clickTab("数据");
    const exportDiagButton = await screen.findByText("导出诊断报告");
    fireEvent.click(exportDiagButton);

    await waitFor(() => {
      expect(writeTextFile).toHaveBeenCalled();
    });

    const writtenContent = JSON.parse(
      vi.mocked(writeTextFile).mock.calls[
        vi.mocked(writeTextFile).mock.calls.length - 1
      ][1] as string
    );
    const writtenStr = JSON.stringify(writtenContent);

    expect(writtenStr).not.toContain(sentinelApiKey);
    expect(writtenStr).not.toContain(sentinelRawMessages);
    expect(writtenStr).not.toContain(sentinelRawMemory);
    expect(writtenStr).not.toContain(sentinelToolOutput);
    expect(writtenStr).not.toContain("should-not-leak");

    expect(writtenContent.diagnostics.beta_ready).toBe(true);
    expect(writtenContent.diagnostics.chat_ready).toBe(true);
    expect(writtenContent.diagnostics.cloud_provider).toBe("DeepSeek");
    expect(writtenContent.diagnostics.memory_chunk_count).toBe(100);
    expect(writtenContent.privacy_manifest.export_strategy).toBe("explicit-whitelist");
    expect(writtenContent.privacy_manifest.includes_raw_messages).toBe(false);
    expect(writtenContent.privacy_manifest.includes_api_keys).toBe(false);
    expect(writtenContent.privacy_manifest.includes_raw_config).toBe(false);
    expect(writtenContent.privacy_manifest.includes_local_paths).toBe(false);

    expect(writtenStr).toContain("timestamp");
  });

  it("redacts local paths from startup_warnings, readiness_issues, and beta_readiness_issues", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: true,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 2,
          mcp_tool_count: 5,
          mcp_recent_audit_count: 10,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 100,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [
            "应用数据目录创建失败：/Users/alice/Library/Application Support/ai.openlife.app (permission denied)",
            'quoted "/Users/alice/private/config.yaml" failed',
            "path=/Users/alice/private/config.yaml failed",
          ],
          startup_warnings: [
            "fallback db at /tmp/openlife/recovery/memory.db",
            "C:\\Users\\Alice\\AppData\\Roaming\\OpenLife\\memory.db",
            "\\\\NAS\\alice\\openlife\\memory.db is not accessible",
          ],
          data_dir: "/Users/alice/Library/Application Support/ai.openlife.app",
          active_data_dir: "/Users/alice/Library/Application Support/ai.openlife.app",
          legacy_data_dir: null,
          database_status: "ok",
          snapshot_count: 3,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 3,
          pending_builder_review_sessions: 0,
          config_source: "file",
          beta_ready: true,
          beta_readiness_issues: ["file:///Users/alice/private/config.yaml 已损坏，请检查后重试"],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 1.2,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.8,
            mcp_audit_db_exists: true,
            mcp_audit_db_size_mb: 0.1,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [{ name: "llama3", size_mb: 4500 }],
          agent_run_count: 5,
          agent_run_store_status: "ok",
          pending_proposal_count: 0,
          high_risk_pending_proposal_count: 0,
          proposal_store_status: "ok",
        });
      }
      return mockInvoke(cmd, args);
    });

    vi.mocked(save).mockResolvedValue("/tmp/openlife-diagnostics.json");
    vi.mocked(writeTextFile).mockResolvedValue(undefined);

    renderSettings();

    await clickTab("数据");
    const exportDiagButton = await screen.findByText("导出诊断报告");
    fireEvent.click(exportDiagButton);

    await waitFor(() => {
      expect(writeTextFile).toHaveBeenCalled();
    });

    const writtenContent = JSON.parse(
      vi.mocked(writeTextFile).mock.calls[
        vi.mocked(writeTextFile).mock.calls.length - 1
      ][1] as string
    );
    const writtenStr = JSON.stringify(writtenContent);

    expect(writtenStr).not.toContain("/Users/alice");
    expect(writtenStr).not.toContain("Alice");
    expect(writtenStr).not.toContain("AppData");
    expect(writtenStr).not.toContain("\\\\NAS");
    expect(writtenStr).not.toContain("file:///Users");

    expect(writtenStr).toContain("[local-path]");
    expect(writtenStr).toContain("[local-file-url]");

    const diag = writtenContent.diagnostics;
    expect(diag.startup_warnings).toBeInstanceOf(Array);
    for (const w of diag.startup_warnings) {
      expect(String(w)).not.toContain("/tmp");
      expect(String(w)).not.toContain("C:");
      expect(String(w)).not.toContain("\\\\NAS");
      expect(String(w)).toContain("[local-path]");
    }
    for (const issue of diag.readiness_issues) {
      expect(String(issue)).not.toContain("/Users");
      expect(String(issue)).not.toContain("/alice");
      expect(String(issue)).toContain("[local-path]");
    }
    for (const issue of diag.beta_readiness_issues) {
      expect(String(issue)).not.toContain("file:///");
      expect(String(issue)).not.toContain("/alice");
      expect(String(issue)).toContain("[local-file-url]");
    }

    expect(writtenContent.privacy_manifest.includes_local_paths).toBe(false);
  });
});
