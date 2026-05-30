import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
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

  function LocationProbe() {
    const location = useLocation();
    return <div data-testid="location-path">{location.pathname}</div>;
  }

  const renderSettingsWithLocation = () =>
    render(
      <MemoryRouter>
        <SettingsPage />
        <LocationProbe />
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
    expect(screen.getByText(/聊天不可用/)).toBeInTheDocument();
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

  it("runs multi-strategy preview from the experimental panel and links to the run trace", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.resolve({
          runId: "run-preview-ui-1",
          strategyKind: "planExecute",
          payloadKind: "planExecute",
          proposalIds: [],
          warnings: ["preview runtime forces allowWrites=false"],
          metadataSafeSummary: {
            taskKind: "conversation",
            reasonCode: "write_like_intent",
            riskLevel: "medium",
            hasHsPacket: true,
          },
          governanceDecisionKind: "warn",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettingsWithLocation();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: /MultiStrategy Preview/ }));

    fireEvent.change(screen.getByLabelText("userText"), {
      target: { value: "Draft a safe preview plan for tomorrow" },
    });
    fireEvent.click(screen.getByLabelText("allowPlanning"));
    fireEvent.click(screen.getByLabelText("localModelAvailable"));
    fireEvent.click(screen.getByRole("button", { name: "Run Preview" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_multi_strategy_agent_preview", {
        input: expect.objectContaining({
          sessionId: expect.stringMatching(/^runtime-preview-/),
          userText: "Draft a safe preview plan for tomorrow",
          toolsPrompt: "No developer tools catalog supplied for this preview.",
          allowPlanning: true,
          localModelAvailable: true,
          layer: "L2",
          executionBudget: expect.objectContaining({ allowWrites: false }),
        }),
      });
    });

    expect(await screen.findByText("run-preview-ui-1")).toBeInTheDocument();
    expect(screen.getByText("Strategy: planExecute")).toBeInTheDocument();
    expect(screen.getByText("Payload: planExecute")).toBeInTheDocument();
    expect(screen.getByText("Governance: warn")).toBeInTheDocument();
    expect(screen.getByText("preview runtime forces allowWrites=false")).toBeInTheDocument();
    expect(screen.getByLabelText("userText")).toHaveValue("");

    fireEvent.click(screen.getByRole("button", { name: "View Run Trace" }));
    expect(screen.getByTestId("location-path")).toHaveTextContent("/runs/run-preview-ui-1");
  });

  it("renders runtime migration gate pass status from the experimental panel", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Runtime Migration Gate")).toBeInTheDocument();
    expect(screen.getByText(/Read-only migration diagnostic/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Runtime Migration Gate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_runtime_migration_gate", { input: {} });
    });

    expect(await screen.findByText("defaultChatUnchanged")).toBeInTheDocument();
    expect(screen.getByText("previewPathHealthy")).toBeInTheDocument();
    expect(screen.getByText("metadataSafeTraceReady")).toBeInTheDocument();
    expect(screen.getByText("fallbackAvailable")).toBeInTheDocument();
    expect(screen.getByText("noExternalWrites")).toBeInTheDocument();
    expect(screen.getByText("proposalFirstPreserved")).toBeInTheDocument();
    expect(screen.getAllByText("Pass")).toHaveLength(6);
    expect(screen.getByText("No blocking reasons returned.")).toBeInTheDocument();
  });

  it("renders runtime migration gate blocking reasons", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_runtime_migration_gate") {
        return Promise.resolve({
          defaultChatUnchanged: true,
          previewPathHealthy: false,
          metadataSafeTraceReady: true,
          fallbackAvailable: true,
          noExternalWrites: false,
          proposalFirstPreserved: true,
          blockingReasons: [
            "latest preview run is missing metadata-safe audit",
            "preview audit indicates external writes",
          ],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Check Runtime Migration Gate" }));

    expect(await screen.findAllByText("Block")).toHaveLength(2);
    expect(
      screen.getByText("latest preview run is missing metadata-safe audit")
    ).toBeInTheDocument();
    expect(screen.getByText("preview audit indicates external writes")).toBeInTheDocument();
  });

  it("renders controlled Chat pilot eligibility as eligible from the experimental panel", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Pilot eligibility")).toBeInTheDocument();
    expect(screen.getByText(/controlled Chat migration pilot/)).toBeInTheDocument();
    expect(screen.getAllByText(/not a Chat switching control/).length).toBeGreaterThanOrEqual(1);
    fireEvent.click(screen.getByRole("button", { name: "Check Pilot Eligibility" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_controlled_chat_pilot_eligibility", {
        input: {},
      });
    });

    expect(await screen.findByText("Eligible")).toBeInTheDocument();
    expect(screen.getByText("3 / 3 clean runs")).toBeInTheDocument();
    expect(screen.getByText("run-preview-clean-3")).toBeInTheDocument();
    expect(screen.getByText("run-preview-clean-2")).toBeInTheDocument();
    expect(screen.getByText("run-preview-clean-1")).toBeInTheDocument();
    expect(screen.getByText("No pilot eligibility blockers returned.")).toBeInTheDocument();
    expect(
      screen.getByText(/Even when eligible, default Chat is not replaced automatically/)
    ).toBeInTheDocument();
  });

  it("renders controlled Chat pilot eligibility blocking reasons", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: false,
          requiredCleanRuns: 3,
          cleanRunCount: 2,
          checkedRunIds: ["run-preview-clean-3", "run-preview-blocked-2", "run-preview-clean-1"],
          blockingReasons: ["run-preview-blocked-2:external_write_risk_detected"],
          lastGateReport: {
            defaultChatUnchanged: true,
            previewPathHealthy: true,
            metadataSafeTraceReady: true,
            fallbackAvailable: true,
            noExternalWrites: true,
            proposalFirstPreserved: true,
            blockingReasons: [],
          },
          defaultChatUnchanged: true,
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Check Pilot Eligibility" }));

    expect(await screen.findByText("Blocked")).toBeInTheDocument();
    expect(screen.getByText("2 / 3 clean runs")).toBeInTheDocument();
    expect(screen.getByText("run-preview-blocked-2")).toBeInTheDocument();
    expect(
      screen.getByText("run-preview-blocked-2:external_write_risk_detected")
    ).toBeInTheDocument();
  });

  it("renders metadata-safe promotion evidence summary from the experimental panel", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Promotion evidence summary")).toBeInTheDocument();
    expect(
      screen.getByText(/metadata-safe evidence recorded after reviewed promotion/)
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Promotion Evidence" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_controlled_pilot_promotion_evidence_summary",
        undefined
      );
    });

    expect(await screen.findByText("Promoted count")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-2")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-1")).toBeInTheDocument();
    expect(screen.getByText("Latest promotion timestamp")).toBeInTheDocument();
    expect(screen.getByText("2026-05-30T01:02:03Z")).toBeInTheDocument();
    expect(screen.getByText("Source/target mismatch blocks")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("clears stale runtime migration gate evidence when starting a new preview", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_runtime_migration_gate") {
        return Promise.resolve({
          defaultChatUnchanged: true,
          previewPathHealthy: false,
          metadataSafeTraceReady: true,
          fallbackAvailable: true,
          noExternalWrites: true,
          proposalFirstPreserved: true,
          blockingReasons: ["stale preview evidence"],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Check Runtime Migration Gate" }));
    expect(await screen.findByText("stale preview evidence")).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: /MultiStrategy Preview/ }));
    fireEvent.change(screen.getByLabelText("userText"), {
      target: { value: "Start a new preview after checking stale gate evidence" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run Preview" }));

    await waitFor(() => {
      expect(screen.queryByText("stale preview evidence")).not.toBeInTheDocument();
    });
  });

  it("checks runtime migration gate against the latest explicit preview result when available", async () => {
    renderSettingsWithLocation();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: /MultiStrategy Preview/ }));
    fireEvent.change(screen.getByLabelText("userText"), {
      target: { value: "Create a preview run before checking the gate" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run Preview" }));

    expect(await screen.findByText("run-preview-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Runtime Migration Gate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_runtime_migration_gate", {
        input: { previewRunId: "run-preview-1" },
      });
    });
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
});
