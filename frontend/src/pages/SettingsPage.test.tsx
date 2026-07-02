import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import SettingsPage from "./SettingsPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";

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
    vi.stubGlobal("__OPENLIFE_INTERNAL_DEBUG__", true);
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

  const TAB_ALIASES: Record<string, string> = {
    数据与恢复: "Privacy & Data",
    隐私与权限: "Privacy & Data",
    模型路由: "Models",
    高级扩展: "Advanced",
    实验: "Advanced",
  };

  const clickTab = async (tabName: string) => {
    const tab = await screen.findByRole("button", { name: TAB_ALIASES[tabName] ?? tabName });
    fireEvent.click(tab);
  };

  it("renders settings title and checklist", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });

    expect(screen.getByText(/启动检查清单/)).toBeInTheDocument();
    expect(screen.getByText(/使用闭环定义/)).toBeInTheDocument();
    expect(screen.getByText(/核心链路已就绪/)).toBeInTheDocument();

    await clickTab("数据与恢复");
    expect(screen.getByText(/导出全部数据/)).toBeInTheDocument();
    expect(await screen.findByText(/旧 run 可能未接入/)).toBeInTheDocument();
  });

  it("hides internal multi-strategy and default Chat migration surfaces by default", async () => {
    vi.stubGlobal("__OPENLIFE_INTERNAL_DEBUG__", false);
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("Settings")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: "实验" })).not.toBeInTheDocument();
    expect(screen.queryByText(/MultiStrategy/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Chat migration/i)).not.toBeInTheDocument();
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
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
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

    expect(await screen.findByText(/启动检查清单/)).toBeInTheDocument();
    expect(screen.getByText(/聊天不可用/)).toBeInTheDocument();
  });

  it("exports data with optional app version metadata", async () => {
    vi.mocked(save).mockResolvedValue("/tmp/openlife-export.json");
    vi.mocked(writeTextFile).mockResolvedValue(undefined);

    renderSettings();

    await clickTab("数据与恢复");
    const exportButton = await screen.findByRole("button", { name: "导出全部数据" });
    fireEvent.click(exportButton);

    expect(
      await screen.findByRole("dialog", { name: "动作预检：导出全部数据" })
    ).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "export_all_data")).toBe(false);
    expect(writeTextFile).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "继续执行" }));

    await waitFor(() => {
      expect(writeTextFile).toHaveBeenCalled();
    });
    expect(await screen.findByText(/应用版本 0.1.0/)).toBeInTheDocument();
  });

  it("requires typed confirmation before importing data", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/openlife-import.json");
    vi.mocked(readTextFile).mockResolvedValue(
      JSON.stringify({
        version: "2.0",
        app_version: "0.1.0",
        messages: [],
        vectors: [],
      })
    );

    renderSettings();

    await clickTab("数据与恢复");
    fireEvent.click(await screen.findByRole("button", { name: "导入覆盖备份" }));

    expect(
      await screen.findByRole("dialog", { name: "动作预检：导入覆盖备份" })
    ).toBeInTheDocument();
    expect(open).not.toHaveBeenCalled();
    expect(readTextFile).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "import_all_data")).toBe(false);

    const preflightContinue = screen.getByRole("button", { name: "继续执行" });
    expect(preflightContinue).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/输入 IMPORT 以继续/), {
      target: { value: "WRONG" },
    });
    expect(preflightContinue).toBeDisabled();
    expect(open).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText(/输入 IMPORT 以继续/), {
      target: { value: "IMPORT" },
    });
    fireEvent.click(preflightContinue);

    expect(await screen.findByRole("dialog", { name: "确认覆盖导入全部数据" })).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "import_all_data")).toBe(false);

    const confirmButton = screen.getByRole("button", { name: "覆盖导入" });
    expect(confirmButton).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/输入 IMPORT 以继续/), { target: { value: "IMPORT" } });
    fireEvent.click(confirmButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "import_all_data",
        expect.objectContaining({
          payload: expect.objectContaining({ version: "2.0" }),
        })
      );
    });
  });

  it("shows audit action preflights on first click before final commands", async () => {
    renderSettings();

    await clickTab("数据与恢复");

    fireEvent.click(await screen.findByRole("button", { name: "导出审计" }));
    expect(await screen.findByRole("dialog", { name: "动作预检：导出审计" })).toBeInTheDocument();
    expect(screen.getByText(/工具输入参数文本/)).toBeInTheDocument();
    expect(screen.getByText(/工具执行结果文本/)).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "export_mcp_audit_logs")).toBe(
      false
    );
    fireEvent.click(screen.getByRole("button", { name: "返回" }));

    fireEvent.click(screen.getByRole("button", { name: "清理旧日志" }));
    expect(await screen.findByRole("dialog", { name: "动作预检：清理旧日志" })).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "cleanup_mcp_audit_logs")).toBe(
      false
    );
    fireEvent.click(screen.getByRole("button", { name: "返回" }));

    fireEvent.click(screen.getByRole("button", { name: "轮换密钥" }));
    expect(await screen.findByRole("dialog", { name: "动作预检：轮换密钥" })).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "rotate_mcp_audit_key")).toBe(
      false
    );
  });

  it("shows feedback evolution report as read-only candidates, not applied rules", async () => {
    renderSettings();

    await clickTab("数据与恢复");
    fireEvent.click(await screen.findByRole("button", { name: "生成进化报告" }));

    expect(await screen.findByText(/只读进化报告：候选 3 条，已应用 0 条/)).toBeInTheDocument();
    expect(screen.queryByText(/已应用规则/)).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("generate_evolution_report", undefined);
  });

  it("tests the current DeepSeek form config instead of only saved config", async () => {
    renderSettings();

    await clickTab("模型路由");
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

    await clickTab("模型路由");
    expect(await screen.findByText(/当前选择的是 DeepSeek 推理模型/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "一键改为 deepseek-chat" }));
    expect(screen.getByDisplayValue("deepseek-chat")).toBeInTheDocument();
  });

  it("explains DeepSeek embedding fallback in settings", async () => {
    renderSettings();

    await clickTab("模型路由");
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
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          usage_ready: false,
          usage_readiness_issues: [
            "数据存储曾在启动时降级：请先确认数据目录和数据库状态，再继续深度使用。",
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
    expect(screen.getByText(/Builder 待确认项/)).toBeInTheDocument();
    expect(screen.getByText(/语义记忆为空/)).toBeInTheDocument();
    expect(screen.getByText(/先导出完整备份/)).toBeInTheDocument();
  });

  it("shows usage flow steps and follow-up actions", async () => {
    renderSettings();

    expect(await screen.findByText("使用闭环定义")).toBeInTheDocument();
    expect(screen.getByText("1. 完成设置与诊断")).toBeInTheDocument();
    expect(screen.getByText("2. 完成人生模型构建")).toBeInTheDocument();
    expect(screen.getByText("3. 跑通第一次对话")).toBeInTheDocument();
    expect(screen.getByText("4. 查看校准或版本回滚")).toBeInTheDocument();
  });

  it("runs current Main Chat runtime debug actions from the experimental panel", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_runtime_status") {
        return Promise.resolve({
          statusVersion: 2,
          authoritativeRuntime: "main_chat_kernel",
          defaultSendPath: "main_chat_kernel",
          startStreamPath: "main_chat_kernel",
          sourceOfTruth: "main_chat_turn_pipeline",
          kernelEvidence: {
            kernelBackedDefault: false,
            finalGateEvidencePresent: false,
            finalGateReady: false,
            latestKernelRouteObserved: false,
            legacyFallbackFreeSinceStartup: true,
          },
          latestRouteEvidence: {
            status: "not_observed",
            directAnswerObserved: false,
            governedBlockerObserved: false,
            agentLoopObserved: false,
            kernelBackedDefaultObserved: false,
            legacyFallbackUsed: false,
          },
          legacyFallback: {
            mode: "explicit_only",
            allowedByDefault: false,
            usedCountSinceStartup: 0,
          },
          finalGateReadiness: {
            authority: "main_chat_final_acceptance_gate",
            status: "not_run",
            blockers: [],
            lastReportRunId: null,
          },
        });
      }
      if (cmd === "check_runtime_migration_gate") {
        return Promise.resolve({
          defaultChatUnchanged: true,
          previewPathHealthy: true,
          metadataSafeTraceReady: true,
          fallbackAvailable: false,
          noExternalWrites: true,
          proposalFirstPreserved: true,
          blockingReasons: ["missing_live_provider_evidence"],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Main Chat Runtime Debug")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show" }));
    fireEvent.click(screen.getByRole("button", { name: "Runtime status" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_main_chat_runtime_status", undefined);
    });
    expect(await screen.findByText("Runtime Status")).toBeInTheDocument();
    expect(screen.getAllByText(/main_chat_kernel/).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "Migration gate" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_runtime_migration_gate", { input: {} });
    });
    expect(await screen.findByText("Migration Gate")).toBeInTheDocument();
    expect(screen.getByText(/missing_live_provider_evidence/)).toBeInTheDocument();
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
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: true,
          chat_session_count: 1,
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
    expect(screen.getByText(/先处理确认建议，比重新开始更合适/)).toBeInTheDocument();
  });

  it("routes vector rebuild through preflight and blocks final command in safe mode", async () => {
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
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
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

    expect(
      await screen.findByRole("dialog", { name: "动作预检：重建向量索引" })
    ).toBeInTheDocument();
    expect(screen.getByText(/Safe Mode 已阻断最终执行入口/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Safe Mode 已阻断" })).toBeDisabled();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "rebuild_memory_index")).toBe(
      false
    );
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
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
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

    await clickTab("数据与恢复");
    const importButton = await screen.findByRole("button", { name: "导入覆盖备份" });
    expect(importButton).not.toBeDisabled();
    fireEvent.click(importButton);

    expect(
      await screen.findByRole("dialog", { name: "动作预检：导入覆盖备份" })
    ).toBeInTheDocument();
    expect(screen.getByText(/Safe Mode 已阻断最终执行入口/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Safe Mode 已阻断" })).toBeDisabled();
    expect(open).not.toHaveBeenCalled();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "import_all_data")).toBe(false);
  });
});
