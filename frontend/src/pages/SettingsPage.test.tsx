import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
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
          beta_readiness_issues: ["核心聊天链路未就绪，请先修复启动检查中的问题。"],
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
    expect(screen.getByText(/Builder 待确认 Review/)).toBeInTheDocument();
    expect(screen.getByText(/语义记忆为空/)).toBeInTheDocument();
    expect(screen.getByText(/先导出完整备份/)).toBeInTheDocument();
  });

  it("shows beta flow steps and follow-up actions", async () => {
    renderSettings();

    expect(await screen.findByText("使用闭环定义")).toBeInTheDocument();
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

  it("renders controlled pilot promotion readiness pass/block details from the experimental panel", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Promotion readiness gate")).toBeInTheDocument();
    expect(screen.getByText(/existing promotion evidence only/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Promotion Readiness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_controlled_pilot_promotion_readiness", {
        input: {},
      });
    });

    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.getByText("3 / 3 promotions")).toBeInTheDocument();
    expect(screen.getByText("metadataSafeEvidenceReady: true")).toBeInTheDocument();
    expect(screen.getByText("defaultChatUnchanged: true")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-3")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-2")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-1")).toBeInTheDocument();
    expect(screen.getByText("No promotion readiness blockers returned.")).toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("renders controlled pilot promotion readiness blocking reasons", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_pilot_promotion_readiness") {
        return Promise.resolve({
          ready: false,
          requiredPromotions: 3,
          promotedCount: 2,
          recentPromotedPilotRunIds: ["run-controlled-pilot-2", "run-controlled-pilot-1"],
          latestPromotionTimestamp: "2026-05-30T02:03:04Z",
          sourceTargetMismatchBlockCount: 1,
          metadataSafeEvidenceReady: true,
          defaultChatUnchanged: true,
          blockingReasons: [
            "insufficient_promotion_evidence: required 3 promotions, found 2",
            "source_target_mismatch_blocks_present",
          ],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Check Promotion Readiness" }));

    expect(await screen.findByText("Blocked")).toBeInTheDocument();
    expect(screen.getByText("2 / 3 promotions")).toBeInTheDocument();
    expect(screen.getByText("Mismatch blocks: 1")).toBeInTheDocument();
    expect(
      screen.getByText("insufficient_promotion_evidence: required 3 promotions, found 2")
    ).toBeInTheDocument();
    expect(screen.getByText("source_target_mismatch_blocks_present")).toBeInTheDocument();
  });

  it("renders controlled chat migration plan draft after readiness passes", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Draft Migration Plan")).toBeInTheDocument();
    expect(screen.getByText(/human review draft/)).toBeInTheDocument();
    expect(screen.getByText(/will not switch default Chat/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Draft Migration Plan" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("draft_controlled_chat_migration_plan", {
        input: {},
      });
    });

    expect(await screen.findByText("Draft ready")).toBeInTheDocument();
    expect(screen.getByText("Manual review required")).toBeInTheDocument();
    expect(screen.getByText("Not automatic migration")).toBeInTheDocument();
    expect(screen.getByText("Migration scope")).toBeInTheDocument();
    expect(screen.getByText("Required preconditions")).toBeInTheDocument();
    expect(screen.getByText("Rollback plan")).toBeInTheDocument();
    expect(screen.getByText("Fallback plan")).toBeInTheDocument();
    expect(screen.getByText("Test plan")).toBeInTheDocument();
    expect(screen.getByText(/default Chat remains unchanged/)).toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
    expect(screen.queryByText("toolPayload")).not.toBeInTheDocument();
  });

  it("renders migration plan blockers without executable plan sections", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_controlled_chat_migration_plan") {
        return Promise.resolve({
          draftReady: false,
          readinessReport: {
            ready: false,
            requiredPromotions: 3,
            promotedCount: 1,
            recentPromotedPilotRunIds: ["run-controlled-pilot-1"],
            latestPromotionTimestamp: "2026-05-30T01:02:03Z",
            sourceTargetMismatchBlockCount: 0,
            metadataSafeEvidenceReady: true,
            defaultChatUnchanged: true,
            blockingReasons: ["insufficient_promotion_evidence: required 3 promotions, found 1"],
          },
          migrationScope: [],
          requiredPreconditions: [],
          rollbackPlan: [],
          fallbackPlan: [],
          testPlan: [],
          manualReviewRequired: true,
          notAutomaticMigration: true,
          blockingReasons: ["insufficient_promotion_evidence: required 3 promotions, found 1"],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Draft Migration Plan" }));

    expect(await screen.findByText("Draft blocked")).toBeInTheDocument();
    expect(
      screen.getByText("insufficient_promotion_evidence: required 3 promotions, found 1")
    ).toBeInTheDocument();
    expect(screen.getByText(/No executable migration plan is generated/)).toBeInTheDocument();
    expect(screen.queryByText("Migration scope")).not.toBeInTheDocument();
    expect(screen.queryByText("Rollback plan")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Approve Review Decision" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Reject Review Decision" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Request Rework Review Decision" })
    ).not.toBeInTheDocument();
  });

  it("records migration review decision only after a ready draft and shows latest summary", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Migration Review Decision")).toBeInTheDocument();
    expect(
      screen.getByText(/Approval only allows next-stage implementation discussion/)
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Approve Review Decision" })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Draft Migration Plan" }));
    expect(await screen.findByText("Draft ready")).toBeInTheDocument();

    expect(screen.getByRole("button", { name: "Approve Review Decision" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reject Review Decision" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Request Rework Review Decision" })
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Reviewer note"), {
      target: { value: "Raw reviewer note should be sanitized by backend." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Request Rework Review Decision" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("record_controlled_chat_migration_review_decision", {
        input: {
          decisionKind: "request_rework",
          optionalReviewerNote: "Raw reviewer note should be sanitized by backend.",
        },
      });
    });
    expect(await screen.findByText(/Decision recorded/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Refresh Decision Summary" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_controlled_chat_migration_review_decision_summary",
        undefined
      );
    });

    expect(await screen.findByText("Latest decision")).toBeInTheDocument();
    expect(screen.getByText("request_rework")).toBeInTheDocument();
    expect(screen.getByText("Approved count")).toBeInTheDocument();
    expect(screen.getByText("Rework/reject count")).toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("keeps migration review decision actions hidden for blocked drafts", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_controlled_chat_migration_plan") {
        return Promise.resolve({
          draftReady: false,
          readinessReport: {
            ready: false,
            requiredPromotions: 3,
            promotedCount: 1,
            recentPromotedPilotRunIds: ["run-controlled-pilot-1"],
            latestPromotionTimestamp: "2026-05-30T01:02:03Z",
            sourceTargetMismatchBlockCount: 0,
            metadataSafeEvidenceReady: true,
            defaultChatUnchanged: true,
            blockingReasons: ["insufficient_promotion_evidence: required 3 promotions, found 1"],
          },
          migrationScope: [],
          requiredPreconditions: [],
          rollbackPlan: [],
          fallbackPlan: [],
          testPlan: [],
          manualReviewRequired: true,
          notAutomaticMigration: true,
          blockingReasons: ["insufficient_promotion_evidence: required 3 promotions, found 1"],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Draft Migration Plan" }));

    expect(await screen.findByText("Draft blocked")).toBeInTheDocument();
    expect(
      screen.getByText(/Review decision recording is blocked until draftReady=true/)
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Review decision blocker: insufficient_promotion_evidence: required 3 promotions, found 1"
      )
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Approve Review Decision" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Reject Review Decision" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Request Rework Review Decision" })
    ).not.toBeInTheDocument();
  });

  it("renders controlled chat migration implementation gate as eligible without switching default Chat", async () => {
    renderSettings();

    await clickTab("实验");

    expect(await screen.findByText("Implementation Gate")).toBeInTheDocument();
    expect(screen.getByText(/current Send remains untouched/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Implementation Gate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_controlled_chat_migration_implementation_gate", {
        input: {},
      });
    });

    expect(await screen.findByText("Eligible")).toBeInTheDocument();
    expect(screen.getByText("Latest decision")).toBeInTheDocument();
    expect(screen.getByText("approve")).toBeInTheDocument();
    expect(screen.getByText("draftHashMatched: true")).toBeInTheDocument();
    expect(screen.getByText("approvedAfterLatestDraft: true")).toBeInTheDocument();
    expect(screen.getByText("3 / 3 promotions")).toBeInTheDocument();
    expect(
      screen.getByText("Even when eligible, default Chat will not switch.")
    ).toBeInTheDocument();
    expect(screen.getByText("No implementation gate blockers returned.")).toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("renders implementation gate blocking reasons when latest approval is invalid", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_migration_implementation_gate") {
        return Promise.resolve({
          implementationEligible: false,
          latestDecision: {
            evidenceId: "ev_review_decision_2",
            decisionKind: "request_rework",
            draftReady: true,
            draftHash: "sha256:previous-draft",
            createdAt: "2026-05-31T02:03:04Z",
          },
          readinessReport: {
            ready: false,
            requiredPromotions: 3,
            promotedCount: 2,
            recentPromotedPilotRunIds: ["run-controlled-pilot-2", "run-controlled-pilot-1"],
            latestPromotionTimestamp: "2026-05-30T02:03:04Z",
            sourceTargetMismatchBlockCount: 1,
            metadataSafeEvidenceReady: true,
            defaultChatUnchanged: true,
            blockingReasons: [
              "insufficient_promotion_evidence: required 3 promotions, found 2",
              "source_target_mismatch_blocks_present",
            ],
          },
          draftHashMatched: false,
          approvedAfterLatestDraft: false,
          blockingReasons: [
            "promotion_readiness_currently_blocked",
            "latest_review_decision_is_request_rework",
            "insufficient_promotion_evidence: required 3 promotions, found 2",
          ],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(screen.getByRole("button", { name: "Check Implementation Gate" }));

    expect(await screen.findByText("Blocked")).toBeInTheDocument();
    expect(screen.getByText("request_rework")).toBeInTheDocument();
    expect(screen.getByText("draftHashMatched: false")).toBeInTheDocument();
    expect(screen.getByText("approvedAfterLatestDraft: false")).toBeInTheDocument();
    expect(screen.getByText("2 / 3 promotions")).toBeInTheDocument();
    expect(screen.getByText("promotion_readiness_currently_blocked")).toBeInTheDocument();
    expect(screen.getByText("latest_review_decision_is_request_rework")).toBeInTheDocument();
    expect(
      screen.getByText("insufficient_promotion_evidence: required 3 promotions, found 2")
    ).toBeInTheDocument();
  });

  it("runs controlled migration shadow run explicitly and renders metadata-safe success", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_migration_shadow_run") {
        return Promise.resolve({
          shadowRunReady: true,
          shadowRunId: "run-shadow-settings-1",
          implementationGateReport: {
            implementationEligible: true,
            latestDecision: {
              evidenceId: "ev_review_decision_2",
              decisionKind: "approve",
              draftReady: true,
              draftHash: "sha256:mock-migration-draft",
              createdAt: "2026-05-31T02:03:04Z",
            },
            readinessReport: {
              ready: true,
              requiredPromotions: 3,
              promotedCount: 3,
              recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
              latestPromotionTimestamp: "2026-05-30T03:04:05Z",
              sourceTargetMismatchBlockCount: 0,
              metadataSafeEvidenceReady: true,
              defaultChatUnchanged: true,
              blockingReasons: [],
            },
            draftHashMatched: true,
            approvedAfterLatestDraft: true,
            blockingReasons: [],
          },
          strategyKind: "planExecute",
          payloadKind: "planExecute",
          metadataSafeSummary: {
            descriptorKind: "planning_readiness_probe",
            allowWrites: false,
            metadataSafe: true,
            rawAssistantOutput: "must not render",
          },
          warnings: ["shadow runtime forced allowWrites=false"],
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Shadow Run")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Shadow prompt descriptor"), {
      target: { value: "planning_readiness_probe" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run Shadow Run" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_controlled_chat_migration_shadow_run", {
        input: expect.objectContaining({
          sessionId: expect.stringMatching(/^settings-shadow-run-/),
          boundedTestPromptDescriptor: "planning_readiness_probe",
          requiredPromotions: 3,
        }),
      });
    });

    expect(await screen.findByText("Shadow ready")).toBeInTheDocument();
    expect(screen.getByText("Strategy: planExecute")).toBeInTheDocument();
    expect(screen.getByText("Payload: planExecute")).toBeInTheDocument();
    expect(screen.getByText("descriptorKind: planning_readiness_probe")).toBeInTheDocument();
    expect(screen.getByText("allowWrites: false")).toBeInTheDocument();
    expect(screen.getByText("shadow runtime forced allowWrites=false")).toBeInTheDocument();
    expect(
      screen.getByText(/Not saved to Chat history and does not switch default Chat/)
    ).toBeInTheDocument();
    expect(screen.queryByText("must not render")).not.toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("records shadow review decision explicitly and renders shadow review summary", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_migration_shadow_run") {
        return Promise.resolve({
          shadowRunReady: true,
          shadowRunId: "run-shadow-settings-1",
          implementationGateReport: {
            implementationEligible: true,
            latestDecision: {
              evidenceId: "ev_review_decision_2",
              decisionKind: "approve",
              draftReady: true,
              draftHash: "sha256:mock-migration-draft",
              createdAt: "2026-05-31T02:03:04Z",
            },
            readinessReport: {
              ready: true,
              requiredPromotions: 3,
              promotedCount: 3,
              recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
              latestPromotionTimestamp: "2026-05-30T03:04:05Z",
              sourceTargetMismatchBlockCount: 0,
              metadataSafeEvidenceReady: true,
              defaultChatUnchanged: true,
              blockingReasons: [],
            },
            draftHashMatched: true,
            approvedAfterLatestDraft: true,
            blockingReasons: [],
          },
          strategyKind: "react",
          payloadKind: "react",
          metadataSafeSummary: {
            descriptorKind: "default_readiness_probe",
            allowWrites: false,
            metadataSafe: true,
          },
          warnings: ["shadow runtime forced allowWrites=false"],
          blockingReasons: [],
        });
      }
      if (cmd === "record_controlled_chat_migration_shadow_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_shadow_review_1",
          shadowRunId: args?.input?.shadowRunId,
          decisionKind: args?.input?.decisionKind,
          readinessSummaryDigest: "sha256:shadow-readiness",
          createdAt: "2026-05-31T04:05:06Z",
          blockingReasons: [],
        });
      }
      if (cmd === "get_controlled_chat_migration_shadow_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_shadow_review_1",
            shadowRunId: "run-shadow-settings-1",
            decisionKind: "approve",
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 22,
            reviewerNoteCategory: "brief",
            readinessSummaryDigest: "sha256:shadow-readiness",
            createdAt: "2026-05-31T04:05:06Z",
          },
          approvedCount: 1,
          reworkRejectCount: 0,
          latestTimestamp: "2026-05-31T04:05:06Z",
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Shadow Run" }));
    expect(await screen.findByText("Shadow ready")).toBeInTheDocument();
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(
          ([cmd]) => cmd === "record_controlled_chat_migration_shadow_review_decision"
        )
    ).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Refresh Shadow Review Summary" }));
    expect(await screen.findByText("sha256:shadow-readiness")).toBeInTheDocument();
    expect(screen.getAllByText("run-shadow-settings-1").length).toBeGreaterThan(0);
    expect(screen.getByText("Approved shadow reviews")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Shadow reviewer note"), {
      target: { value: "Looks ready to approve" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve Shadow Review" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_controlled_chat_migration_shadow_review_decision",
        {
          input: {
            shadowRunId: "run-shadow-settings-1",
            decisionKind: "approve",
            optionalReviewerNote: "Looks ready to approve",
          },
        }
      );
    });
    expect(await screen.findByText("Shadow review recorded")).toBeInTheDocument();
    expect(screen.getByText(/approve · run-shadow-settings-1/)).toBeInTheDocument();
  });

  it("checks cutover readiness explicitly and renders planning eligibility", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_cutover_readiness") {
        return Promise.resolve({
          cutoverPlanningEligible: true,
          implementationGateReport: {
            implementationEligible: true,
            latestDecision: {
              evidenceId: "ev_review_decision_2",
              decisionKind: "approve",
              draftReady: true,
              draftHash: "sha256:mock-migration-draft",
              createdAt: "2026-05-31T02:03:04Z",
            },
            readinessReport: {
              ready: true,
              requiredPromotions: 3,
              promotedCount: 3,
              recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
              latestPromotionTimestamp: "2026-05-30T03:04:05Z",
              sourceTargetMismatchBlockCount: 0,
              metadataSafeEvidenceReady: true,
              defaultChatUnchanged: true,
              blockingReasons: [],
            },
            draftHashMatched: true,
            approvedAfterLatestDraft: true,
            blockingReasons: [],
          },
          latestShadowReviewDecision: {
            evidenceId: "ev_shadow_review_1",
            shadowRunId: "run-shadow-settings-1",
            decisionKind: "approve",
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 22,
            reviewerNoteCategory: "brief",
            readinessSummaryDigest: "sha256:shadow-readiness",
            createdAt: "2026-05-31T04:05:06Z",
          },
          verifiedShadowRunId: "run-shadow-settings-1",
          readinessSummaryDigest: "sha256:shadow-readiness",
          defaultChatUnchanged: true,
          requiredEvidenceReady: true,
          blockingReasons: [],
          metadataSafeSummary: {
            metadataSafe: true,
            planningOnly: true,
            cutoverReadinessGate: "controlled_chat_cutover_planning",
            implementationEligible: true,
            shadowRunReady: true,
            latestShadowReviewDecisionKind: "approve",
            contentStorage: "none",
            toolStorage: "none",
            rawOutput: "must not render",
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Cutover Readiness")).toBeInTheDocument();
    expect(screen.getAllByText(/cutover planning readiness/).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Check Cutover Readiness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_controlled_chat_cutover_readiness", {
        input: {},
      });
    });

    expect(await screen.findByText("Cutover Planning Eligible")).toBeInTheDocument();
    expect(screen.getByText("requiredEvidenceReady: true")).toBeInTheDocument();
    expect(screen.getByText("defaultChatUnchanged: true")).toBeInTheDocument();
    expect(screen.getByText("run-shadow-settings-1")).toBeInTheDocument();
    expect(screen.getByText("sha256:shadow-readiness")).toBeInTheDocument();
    expect(screen.getByText("No cutover readiness blockers returned.")).toBeInTheDocument();
    expect(screen.getByText("planningOnly: true")).toBeInTheDocument();
    expect(screen.queryByText("must not render")).not.toBeInTheDocument();
    expect(screen.queryByText("Pilot-only answer")).not.toBeInTheDocument();
  });

  it("renders cutover readiness blockers without triggering shadow run or preview", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_cutover_readiness") {
        return Promise.resolve({
          cutoverPlanningEligible: false,
          implementationGateReport: {
            implementationEligible: true,
            latestDecision: {
              evidenceId: "ev_review_decision_2",
              decisionKind: "approve",
              draftReady: true,
              draftHash: "sha256:mock-migration-draft",
              createdAt: "2026-05-31T02:03:04Z",
            },
            readinessReport: {
              ready: true,
              requiredPromotions: 3,
              promotedCount: 3,
              recentPromotedPilotRunIds: ["run-controlled-pilot-3"],
              latestPromotionTimestamp: "2026-05-30T03:04:05Z",
              sourceTargetMismatchBlockCount: 0,
              metadataSafeEvidenceReady: true,
              defaultChatUnchanged: true,
              blockingReasons: [],
            },
            draftHashMatched: true,
            approvedAfterLatestDraft: true,
            blockingReasons: [],
          },
          latestShadowReviewDecision: {
            evidenceId: "ev_shadow_review_1",
            shadowRunId: "run-shadow-settings-1",
            decisionKind: "request_rework",
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 22,
            reviewerNoteCategory: "brief",
            readinessSummaryDigest: "sha256:shadow-readiness",
            createdAt: "2026-05-31T04:05:06Z",
          },
          verifiedShadowRunId: null,
          readinessSummaryDigest: "sha256:shadow-readiness",
          defaultChatUnchanged: true,
          requiredEvidenceReady: false,
          blockingReasons: ["latest_shadow_review_decision_is_request_rework"],
          metadataSafeSummary: {
            metadataSafe: true,
            planningOnly: true,
          },
        });
      }
      if (
        cmd === "run_controlled_chat_migration_shadow_run" ||
        cmd === "run_multi_strategy_agent_preview"
      ) {
        return Promise.reject(new Error("cutover readiness must not run runtime"));
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Check Cutover Readiness" }));

    expect(await screen.findByText("Cutover Blocked")).toBeInTheDocument();
    expect(screen.getByText("latest_shadow_review_decision_is_request_rework")).toBeInTheDocument();
    expect(screen.getByText("requiredEvidenceReady: false")).toBeInTheDocument();
    expect(
      vi
        .mocked(invoke)
        .mock.calls.some(([cmd]) => cmd === "run_controlled_chat_migration_shadow_run")
    ).toBe(false);
    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "run_multi_strategy_agent_preview")
    ).toBe(false);
  });

  it("runs cutover candidate explicitly and renders contract-shaped metadata", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_cutover_candidate") {
        return Promise.resolve({
          candidateReady: true,
          candidateRunId: "run-candidate-settings-1",
          outputPreview: "Cutover candidate: react / react",
          userOutput: "Candidate-only answer",
          contractShape: "send_message_compatible",
          metadataSafeSummary: {
            candidateAdapter: "controlled_chat_cutover_candidate",
            metadataSafe: true,
            nonDefault: true,
            allowWrites: false,
            maxToolCalls: 0,
            chatHistoryStorage: "none",
            proposalStorage: "none",
            memoryStorage: "none",
            rawOutput: "must not render",
          },
          warnings: ["candidate runtime forced allowWrites=false"],
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Cutover Candidate")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Run Cutover Candidate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_controlled_chat_cutover_candidate", {
        input: expect.objectContaining({
          boundedTestPromptDescriptor: "default_contract_probe",
          requiredPromotions: 3,
        }),
      });
    });

    expect(await screen.findByText("Candidate ready")).toBeInTheDocument();
    expect(screen.getByText("send_message_compatible")).toBeInTheDocument();
    expect(screen.getByText("run-candidate-settings-1")).toBeInTheDocument();
    expect(screen.getByText("Cutover candidate: react / react")).toBeInTheDocument();
    expect(screen.getByText("candidate runtime forced allowWrites=false")).toBeInTheDocument();
    expect(screen.getByText("allowWrites: false")).toBeInTheDocument();
    expect(screen.getByText("maxToolCalls: 0")).toBeInTheDocument();
    expect(screen.getByText("chatHistoryStorage: none")).toBeInTheDocument();
    expect(screen.getByText("No cutover candidate blockers returned.")).toBeInTheDocument();
    expect(screen.queryByText("must not render")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
  });

  it("renders cutover candidate blocked state without runtime success UI", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_cutover_candidate") {
        return Promise.resolve({
          candidateReady: false,
          candidateRunId: null,
          outputPreview: "Candidate blocked before runtime",
          userOutput: null,
          contractShape: "blocked",
          metadataSafeSummary: {
            candidateAdapter: "controlled_chat_cutover_candidate",
            metadataSafe: true,
            blockedBeforeRuntime: true,
            allowWrites: false,
            maxToolCalls: 0,
          },
          warnings: [],
          blockingReasons: ["cutover_readiness_not_eligible"],
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.reject(new Error("blocked candidate must not call preview command"));
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Cutover Candidate" }));

    expect(await screen.findByText("Candidate blocked")).toBeInTheDocument();
    expect(screen.getByText("blocked")).toBeInTheDocument();
    expect(screen.getByText("Candidate blocked before runtime")).toBeInTheDocument();
    expect(screen.getByText("cutover_readiness_not_eligible")).toBeInTheDocument();
    expect(screen.queryByText("Candidate ready")).not.toBeInTheDocument();
    expect(screen.queryByText("send_message_compatible")).not.toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "run_multi_strategy_agent_preview")
    ).toBe(false);
  });

  it("records cutover candidate review decision explicitly from Settings", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_cutover_candidate") {
        return Promise.resolve({
          candidateReady: true,
          candidateRunId: "run-candidate-settings-review",
          outputPreview: "Cutover candidate: react / react",
          userOutput: "Candidate-only answer",
          contractShape: "send_message_compatible",
          metadataSafeSummary: {
            candidateAdapter: "controlled_chat_cutover_candidate",
            metadataSafe: true,
            allowWrites: false,
            maxToolCalls: 0,
          },
          warnings: [],
          blockingReasons: [],
        });
      }
      if (cmd === "record_controlled_chat_cutover_candidate_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_candidate_review_1",
          candidateRunId: args?.input?.candidateRunId,
          decisionKind: args?.input?.decisionKind,
          contractShape: "send_message_compatible",
          candidateSummaryDigest: "sha256:candidate-summary",
          createdAt: "2026-05-31T06:07:08Z",
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Cutover Candidate" }));
    expect(await screen.findByText("Candidate ready")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("candidate review note"), {
      target: { value: "Approved after manual candidate review." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve Candidate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_controlled_chat_cutover_candidate_review_decision",
        {
          input: {
            candidateRunId: "run-candidate-settings-review",
            decisionKind: "approve",
            optionalReviewerNote: "Approved after manual candidate review.",
          },
        }
      );
    });
    expect(await screen.findByText("Candidate review recorded")).toBeInTheDocument();
    expect(screen.getByText(/ev_candidate_review_1/)).toBeInTheDocument();
    expect(screen.getByText("sha256:candidate-summary")).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "save_chat_message")).toBe(false);
    expect(screen.queryByRole("button", { name: /feature flag/i })).not.toBeInTheDocument();
  });

  it("refreshes and renders cutover candidate review summary", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_controlled_chat_cutover_candidate_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_candidate_review_2",
            candidateRunId: "run-candidate-summary-1",
            decisionKind: "request_rework",
            contractShape: "send_message_compatible",
            candidateSummaryDigest: "sha256:candidate-summary-2",
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 18,
            reviewerNoteCategory: "brief",
            createdAt: "2026-05-31T07:08:09Z",
          },
          approvedCount: 1,
          reworkRejectCount: 2,
          latestTimestamp: "2026-05-31T07:08:09Z",
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Cutover Candidate Review")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Candidate Review Summary" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_controlled_chat_cutover_candidate_review_summary",
        undefined
      );
    });
    expect(await screen.findByText("request_rework")).toBeInTheDocument();
    expect(screen.getByText("run-candidate-summary-1")).toBeInTheDocument();
    expect(screen.getByText("sha256:candidate-summary-2")).toBeInTheDocument();
    expect(screen.getByText("Approved: 1")).toBeInTheDocument();
    expect(screen.getByText("Rework / Reject: 2")).toBeInTheDocument();
    expect(screen.getByText("brief")).toBeInTheDocument();
  });

  it("refreshes and renders cutover candidate promotion readiness without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_cutover_candidate_promotion_readiness") {
        return Promise.resolve({
          ready: false,
          cutoverReadinessEligible: true,
          requiredApprovedCandidates: 3,
          approvedCandidateCount: 1,
          latestDecision: {
            evidenceId: "ev_candidate_review_3",
            candidateRunId: "run-candidate-promotion-1",
            decisionKind: "approve",
            contractShape: "send_message_compatible",
            candidateSummaryDigest: "sha256:candidate-summary-3",
            reviewerNoteChecksum: null,
            reviewerNoteLength: 0,
            reviewerNoteCategory: "none",
            createdAt: "2026-05-31T08:09:10Z",
          },
          approvedCandidates: [
            {
              evidenceId: "ev_candidate_review_3",
              candidateRunId: "run-candidate-promotion-1",
              contractShape: "send_message_compatible",
              candidateSummaryDigest: "sha256:candidate-summary-3",
              runReadinessDigest: "sha256:run-readiness",
              decisionCreatedAt: "2026-05-31T08:09:10Z",
              ready: true,
              blockingReasons: [],
            },
          ],
          defaultChatUnchanged: true,
          blockingReasons: ["insufficient_approved_candidate_evidence"],
          metadataSafeSummary: {
            promotionReadinessGate: "controlled_chat_cutover_candidate",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
            defaultChatUnchanged: true,
            approvedCandidateCount: 1,
          },
          checkedAt: "2026-05-31T08:10:00Z",
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Candidate Promotion Readiness")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Candidate Promotion Readiness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "check_controlled_chat_cutover_candidate_promotion_readiness",
        { input: { requiredApprovedCandidates: 1 } }
      );
    });
    expect(await screen.findByText("Promotion blocked")).toBeInTheDocument();
    expect(screen.getByText("Latest decision: approve")).toBeInTheDocument();
    expect(screen.getByText("1 / 3 approved candidates")).toBeInTheDocument();
    expect(screen.getByText("run-candidate-promotion-1")).toBeInTheDocument();
    expect(screen.getByText("insufficient_approved_candidate_evidence")).toBeInTheDocument();
    expect(screen.getByText("readOnly: true")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch default chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
  });

  it("refreshes and renders default chat runtime boundary status without activation controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_default_chat_runtime_boundary_status") {
        return Promise.resolve({
          currentMode: "legacy_stream",
          controlledCandidateAvailable: false,
          defaultChatUnchanged: true,
          candidatePromotionReadinessRequired: true,
          automaticMigrationEnabled: false,
          blockingReasons: [],
          metadataSafeSummary: {
            runtimeBoundary: "default_chat",
            metadataSafe: true,
            readOnly: true,
            currentMode: "legacy_stream",
            automaticMigrationEnabled: false,
            controlledCandidateAvailable: false,
            candidatePromotionReadinessRequired: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Runtime Boundary")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Default Chat Boundary" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_default_chat_runtime_boundary_status", undefined);
    });
    expect(await screen.findByText("currentMode: legacy_stream")).toBeInTheDocument();
    expect(screen.getByText("defaultChatUnchanged: true")).toBeInTheDocument();
    expect(screen.getByText("automaticMigrationEnabled: false")).toBeInTheDocument();
    expect(screen.getByText("candidatePromotionReadinessRequired: true")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("refreshes and renders default chat adapter activation plan draft without activation controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_default_chat_adapter_activation_plan") {
        return Promise.resolve({
          draftReady: true,
          candidatePromotionReadinessReport: {
            ready: true,
            cutoverReadinessEligible: true,
            requiredApprovedCandidates: 1,
            approvedCandidateCount: 1,
            latestDecision: {
              evidenceId: "ev_candidate_review_4",
              candidateRunId: "run-candidate-activation-1",
              decisionKind: "approve",
              contractShape: "send_message_compatible",
              candidateSummaryDigest: "sha256:candidate-summary-4",
              reviewerNoteChecksum: null,
              reviewerNoteLength: 0,
              reviewerNoteCategory: "none",
              createdAt: "2026-05-31T09:10:11Z",
            },
            approvedCandidates: [],
            defaultChatUnchanged: true,
            blockingReasons: [],
            metadataSafeSummary: {
              promotionReadinessGate: "controlled_chat_cutover_candidate",
              metadataSafe: true,
              readOnly: true,
            },
            checkedAt: "2026-05-31T09:11:00Z",
          },
          runtimeBoundaryStatus: {
            currentMode: "legacy_stream",
            controlledCandidateAvailable: false,
            defaultChatUnchanged: true,
            candidatePromotionReadinessRequired: true,
            automaticMigrationEnabled: false,
            blockingReasons: [],
            metadataSafeSummary: {
              runtimeBoundary: "default_chat",
              metadataSafe: true,
              readOnly: true,
            },
          },
          activationScope: ["Human-review-only adapter activation draft."],
          requiredPreconditions: ["W33 candidate promotion readiness remains ready."],
          adapterContractChecks: ["send_message-compatible contract shape remains stable."],
          fallbackPlan: ["Keep default Chat on the legacy stream fallback."],
          rollbackPlan: ["Revert only a separate adapter implementation."],
          observabilityPlan: ["Use metadata-safe activation counters only."],
          testPlan: ["Verify send_message and start_stream_message do not call this command."],
          manualReviewRequired: true,
          notAutomaticMigration: true,
          requiresSeparateImplementation: true,
          blockingReasons: [],
          metadataSafeSummary: {
            activationPlan: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            manualReviewRequired: true,
            notAutomaticMigration: true,
            requiresSeparateImplementation: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Activation Plan")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Activation Plan Draft" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("draft_default_chat_adapter_activation_plan", {
        input: { requiredApprovedCandidates: 1 },
      });
    });
    expect(await screen.findByText("Activation draft ready")).toBeInTheDocument();
    expect(screen.getByText("Human-review-only adapter activation draft.")).toBeInTheDocument();
    expect(
      screen.getByText("W33 candidate promotion readiness remains ready.")
    ).toBeInTheDocument();
    expect(
      screen.getByText("send_message-compatible contract shape remains stable.")
    ).toBeInTheDocument();
    expect(screen.getByText("Use metadata-safe activation counters only.")).toBeInTheDocument();
    expect(screen.getByText("manualReviewRequired: true")).toBeInTheDocument();
    expect(screen.getByText("requiresSeparateImplementation: true")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter activation plan blockers without plan sections", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_default_chat_adapter_activation_plan") {
        return Promise.resolve({
          draftReady: false,
          candidatePromotionReadinessReport: {
            ready: false,
            cutoverReadinessEligible: false,
            requiredApprovedCandidates: 1,
            approvedCandidateCount: 0,
            latestDecision: null,
            approvedCandidates: [],
            defaultChatUnchanged: true,
            blockingReasons: ["candidate_review_decision_missing"],
            metadataSafeSummary: {
              metadataSafe: true,
              readOnly: true,
            },
            checkedAt: "2026-05-31T09:12:00Z",
          },
          runtimeBoundaryStatus: {
            currentMode: "legacy_stream",
            controlledCandidateAvailable: false,
            defaultChatUnchanged: true,
            candidatePromotionReadinessRequired: true,
            automaticMigrationEnabled: false,
            blockingReasons: [],
            metadataSafeSummary: {
              runtimeBoundary: "default_chat",
              metadataSafe: true,
              readOnly: true,
            },
          },
          activationScope: [],
          requiredPreconditions: [],
          adapterContractChecks: [],
          fallbackPlan: [],
          rollbackPlan: [],
          observabilityPlan: [],
          testPlan: [],
          manualReviewRequired: true,
          notAutomaticMigration: true,
          requiresSeparateImplementation: true,
          blockingReasons: [
            "candidate_promotion_readiness_not_ready",
            "candidate_review_decision_missing",
          ],
          metadataSafeSummary: {
            activationPlan: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            draftReady: false,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Refresh Activation Plan Draft" }));

    expect(await screen.findByText("Activation draft blocked")).toBeInTheDocument();
    expect(screen.getByText("candidate_promotion_readiness_not_ready")).toBeInTheDocument();
    expect(screen.getByText("candidate_review_decision_missing")).toBeInTheDocument();
    expect(screen.queryByText("Activation Scope")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("records default chat adapter activation review decisions and refreshes summary", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "record_default_chat_adapter_activation_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_activation_review_1",
          decisionKind: args?.input?.decisionKind ?? "approve",
          draftReady: true,
          activationPlanDigest: "sha256:activation-plan",
          createdAt: "2026-05-31T10:11:12Z",
          blockingReasons: [],
        });
      }
      if (cmd === "get_default_chat_adapter_activation_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_activation_review_1",
            decisionKind: "approve",
            draftReady: true,
            activationPlanDigest: "sha256:activation-plan",
            candidatePromotionReady: true,
            currentMode: "legacy_stream",
            automaticMigrationEnabled: false,
            reviewerNoteChecksum: null,
            reviewerNoteLength: 0,
            reviewerNoteCategory: "none",
            createdAt: "2026-05-31T10:11:12Z",
          },
          approvedCount: 1,
          rejectOrReworkCount: 0,
          latestTimestamp: "2026-05-31T10:11:12Z",
          blockingReasons: [],
          metadataSafeSummary: {
            activationReview: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            approvedCount: 1,
            rejectOrReworkCount: 0,
            latestDecisionPresent: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Activation Review Decision")
    ).toBeInTheDocument();
    fireEvent.change(
      screen.getByPlaceholderText("Optional private note; only metadata is stored."),
      {
        target: { value: "Approved by human reviewer." },
      }
    );
    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_default_chat_adapter_activation_review_decision",
        {
          input: {
            decisionKind: "approve",
            requiredApprovedCandidates: 1,
            optionalReviewerNote: "Approved by human reviewer.",
          },
        }
      );
    });
    expect(await screen.findByText("Activation review decision recorded")).toBeInTheDocument();
    expect(await screen.findAllByText("decisionKind: approve")).toHaveLength(2);
    expect(screen.getAllByText("approvedCount: 1").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("rejectOrReworkCount: 0").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getByText("activationReview: default_chat_adapter_activation")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("renders blocked activation review approval without activating default chat", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "record_default_chat_adapter_activation_review_decision") {
        return Promise.resolve({
          recorded: false,
          evidenceId: null,
          decisionKind: args?.input?.decisionKind ?? "approve",
          draftReady: false,
          activationPlanDigest: "sha256:blocked-activation-plan",
          createdAt: "2026-05-31T10:12:13Z",
          blockingReasons: [
            "candidate_promotion_readiness_not_ready",
            "activation_plan_draft_not_ready_for_approval",
          ],
        });
      }
      if (cmd === "get_default_chat_adapter_activation_review_summary") {
        return Promise.resolve({
          latestDecision: null,
          approvedCount: 0,
          rejectOrReworkCount: 0,
          latestTimestamp: null,
          blockingReasons: ["activation_review_decision_missing"],
          metadataSafeSummary: {
            activationReview: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            latestDecisionPresent: false,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Approve" }));

    expect(await screen.findByText("Activation review decision blocked")).toBeInTheDocument();
    expect(screen.getByText("candidate_promotion_readiness_not_ready")).toBeInTheDocument();
    expect(screen.getByText("activation_plan_draft_not_ready_for_approval")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("checks default chat adapter activation implementation gate without activation controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_activation_implementation_gate") {
        return Promise.resolve({
          implementationGateEligible: true,
          draftReady: true,
          latestDecision: {
            evidenceId: "ev_activation_review_gate_1",
            decisionKind: "approve",
            draftReady: true,
            activationPlanDigest: "sha256:activation-plan-gate",
            candidatePromotionReady: true,
            currentMode: "legacy_stream",
            automaticMigrationEnabled: false,
            reviewerNoteChecksum: null,
            reviewerNoteLength: 0,
            reviewerNoteCategory: "none",
            createdAt: "2026-05-31T12:13:14Z",
          },
          currentActivationPlanDigest: "sha256:activation-plan-gate",
          activationPlanDigestMatched: true,
          defaultChatUnchanged: true,
          automaticMigrationEnabled: false,
          currentMode: "legacy_stream",
          blockingReasons: [],
          metadataSafeSummary: {
            activationImplementationGate: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
            requiresSeparateImplementation: true,
            implementationGateEligible: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Activation Implementation Gate")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Activation Implementation Gate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "check_default_chat_adapter_activation_implementation_gate",
        {
          input: { requiredApprovedCandidates: 1 },
        }
      );
    });
    expect(await screen.findByText("Activation implementation gate eligible")).toBeInTheDocument();
    expect(screen.getAllByText("currentMode: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("activationPlanDigestMatched: true")).toBeInTheDocument();
    expect(screen.getByText("decisionKind: approve")).toBeInTheDocument();
    expect(
      screen.getByText("activationImplementationGate: default_chat_adapter_activation")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter activation implementation gate blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_activation_implementation_gate") {
        return Promise.resolve({
          implementationGateEligible: false,
          draftReady: true,
          latestDecision: {
            evidenceId: "ev_activation_review_gate_2",
            decisionKind: "request_rework",
            draftReady: true,
            activationPlanDigest: "sha256:old-activation-plan",
            candidatePromotionReady: true,
            currentMode: "legacy_stream",
            automaticMigrationEnabled: false,
            reviewerNoteChecksum: "sha256:note",
            reviewerNoteLength: 16,
            reviewerNoteCategory: "brief",
            createdAt: "2026-05-31T12:14:15Z",
          },
          currentActivationPlanDigest: "sha256:new-activation-plan",
          activationPlanDigestMatched: false,
          defaultChatUnchanged: true,
          automaticMigrationEnabled: false,
          currentMode: "legacy_stream",
          blockingReasons: [
            "latest_activation_review_decision_is_request_rework",
            "activation_plan_digest_mismatch",
          ],
          metadataSafeSummary: {
            activationImplementationGate: "default_chat_adapter_activation",
            metadataSafe: true,
            readOnly: true,
            implementationGateEligible: false,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(
      await screen.findByRole("button", { name: "Check Activation Implementation Gate" })
    );

    expect(await screen.findByText("Activation implementation gate blocked")).toBeInTheDocument();
    expect(
      screen.getByText("latest_activation_review_decision_is_request_rework")
    ).toBeInTheDocument();
    expect(screen.getByText("activation_plan_digest_mismatch")).toBeInTheDocument();
    expect(screen.getByText("activationPlanDigestMatched: false")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("refreshes default chat adapter routing status as a disabled scaffold", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_default_chat_adapter_routing_status") {
        return Promise.resolve({
          currentMode: "legacy_stream",
          adapterScaffoldPresent: true,
          controlledAdapterEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          activationImplementationGateEligible: true,
          requiresSeparateCutoverImplementation: true,
          blockingReasons: [],
          metadataSafeSummary: {
            defaultChatAdapterRouting: "disabled_scaffold",
            metadataSafe: true,
            readOnly: true,
            routingMode: "legacy_stream",
            adapterScaffoldPresent: true,
            controlledAdapterEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            activationImplementationGateEligible: true,
            requiresSeparateCutoverImplementation: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Routing Status")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Adapter Routing Status" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_default_chat_adapter_routing_status", {
        input: { requiredApprovedCandidates: 1 },
      });
    });
    expect(await screen.findByText("Controlled adapter disabled")).toBeInTheDocument();
    expect(screen.getAllByText("currentMode: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("adapterScaffoldPresent: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("controlledAdapterEnabled: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("defaultSendPath: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("startStreamPath: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("activationImplementationGateEligible: true").length
    ).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("requiresSeparateCutoverImplementation: true").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("defaultChatAdapterRouting: disabled_scaffold")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter routing blockers without enabling controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_default_chat_adapter_routing_status") {
        return Promise.resolve({
          currentMode: "legacy_stream",
          adapterScaffoldPresent: true,
          controlledAdapterEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          activationImplementationGateEligible: false,
          requiresSeparateCutoverImplementation: true,
          blockingReasons: [
            "activation_implementation_gate_not_eligible",
            "latest_activation_review_decision_missing",
          ],
          metadataSafeSummary: {
            defaultChatAdapterRouting: "disabled_scaffold",
            metadataSafe: true,
            readOnly: true,
            routingMode: "legacy_stream",
            activationImplementationGateEligible: false,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Refresh Adapter Routing Status" }));

    expect(await screen.findByText("Controlled adapter disabled")).toBeInTheDocument();
    expect(screen.getByText("activation_implementation_gate_not_eligible")).toBeInTheDocument();
    expect(screen.getByText("latest_activation_review_decision_missing")).toBeInTheDocument();
    expect(
      screen.getAllByText("activationImplementationGateEligible: false").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("refreshes default chat adapter ordinary entry preflight status without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_default_chat_adapter_ordinary_entry_preflight_status") {
        return Promise.resolve({
          statusReady: true,
          defaultChatUnchanged: true,
          currentMode: "legacy_stream",
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          sendMessagePreflight: {
            callsite: "send_message",
            preflightReady: true,
            contractReady: true,
            legacyEntryAllowed: true,
            ordinaryEntryPath: "legacy_stream",
            requiredEntryPath: "legacy_stream",
            contractShape: "send_message_compatible",
            sideEffectLockEngaged: true,
            defaultChatMigrationAllowed: false,
            controlledAdapterExecutorAttached: false,
            runtimeCallEnabled: false,
            modelCallEnabled: false,
            toolCallEnabled: false,
            allowWrites: false,
            maxToolCalls: 0,
            chatMessageSaved: false,
            agentRunRecorded: false,
            evidenceRecorded: false,
            blockingReasons: [],
          },
          streamMessagePreflight: {
            callsite: "start_stream_message",
            preflightReady: true,
            contractReady: true,
            legacyEntryAllowed: true,
            ordinaryEntryPath: "legacy_stream",
            requiredEntryPath: "legacy_stream",
            contractShape: "stream_message_compatible",
            sideEffectLockEngaged: true,
            defaultChatMigrationAllowed: false,
            controlledAdapterExecutorAttached: false,
            runtimeCallEnabled: false,
            modelCallEnabled: false,
            toolCallEnabled: false,
            allowWrites: false,
            maxToolCalls: 0,
            chatMessageSaved: false,
            agentRunRecorded: false,
            evidenceRecorded: false,
            blockingReasons: [],
          },
          blockingReasons: [],
          metadataSafeSummary: {
            ordinaryEntryPreflight: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            statusReady: true,
            sendPreflightReady: true,
            streamPreflightReady: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Ordinary Entry Preflight")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Ordinary Entry Preflight" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_default_chat_adapter_ordinary_entry_preflight_status",
        undefined
      );
    });
    expect(await screen.findByText("Ordinary entry preflight ready")).toBeInTheDocument();
    expect(screen.getAllByText("statusReady: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("defaultChatUnchanged: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("controlledAdapterEnabled: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("automaticMigrationEnabled: false").length).toBeGreaterThanOrEqual(
      1
    );
    expect(screen.getAllByText("send_message").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("start_stream_message").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("ordinaryEntryPath: legacy_stream").length).toBeGreaterThanOrEqual(
      2
    );
    expect(screen.getAllByText("sideEffectLockEngaged: true").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("ordinaryEntryPreflight: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("checks default chat adapter narrow implementation discussion gate without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_narrow_implementation_discussion_gate") {
        return Promise.resolve({
          eligible: true,
          defaultChatUnchanged: true,
          cutoverPlanApprovalReady: true,
          ordinaryEntryPreflightStatusReady: true,
          sendPreflightReady: true,
          streamPreflightReady: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [],
          metadataSafeSummary: {
            narrowImplementationDiscussionGate: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            eligible: true,
            defaultChatUnchanged: true,
            cutoverPlanApprovalReady: true,
            ordinaryEntryPreflightStatusReady: true,
            sendPreflightReady: true,
            streamPreflightReady: true,
            notAutomaticMigration: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Narrow Implementation Discussion Gate")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Narrow Implementation Gate" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "check_default_chat_adapter_narrow_implementation_discussion_gate",
        {
          input: {
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
          },
        }
      );
    });
    expect(
      await screen.findByText("Narrow implementation discussion eligible")
    ).toBeInTheDocument();
    expect(screen.getAllByText("eligible: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("defaultChatUnchanged: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("cutoverPlanApprovalReady: true").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("ordinaryEntryPreflightStatusReady: true").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("sendPreflightReady: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("streamPreflightReady: true").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getByText("narrowImplementationDiscussionGate: default_chat_adapter")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("drafts default chat adapter narrow implementation plan without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_default_chat_adapter_narrow_implementation_plan") {
        return Promise.resolve({
          draftReady: true,
          discussionGate: {
            eligible: true,
            defaultChatUnchanged: true,
            cutoverPlanApprovalReady: true,
            ordinaryEntryPreflightStatusReady: true,
            sendPreflightReady: true,
            streamPreflightReady: true,
            controlledAdapterEnabled: false,
            automaticMigrationEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            blockingReasons: [],
            metadataSafeSummary: {
              narrowImplementationDiscussionGate: "default_chat_adapter",
              metadataSafe: true,
              readOnly: true,
            },
          },
          manualReviewRequired: true,
          notAutomaticMigration: true,
          requiresSeparateImplementation: true,
          requiresSeparateCutoverReview: true,
          sourceSessionId: "settings-dry-run",
          inputMessageLength: 31,
          inputMessageHash: "sha256:message123",
          stablePlanDigest: "sha256:narrow-plan-123",
          planSections: [
            {
              sectionKey: "implementationScope",
              title: "Implementation Scope",
              items: ["Keep default Chat unchanged."],
            },
            {
              sectionKey: "explicitNonGoals",
              title: "Explicit Non Goals",
              items: ["Do not migrate default Chat."],
            },
          ],
          blockingReasons: [],
          metadataSafeSummary: {
            narrowImplementationPlan: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Narrow Implementation Plan")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Draft Narrow Implementation Plan" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("draft_default_chat_adapter_narrow_implementation_plan", {
        input: {
          sourceSessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
        },
      });
    });
    expect(
      await screen.findByText("Narrow implementation plan ready for human review")
    ).toBeInTheDocument();
    expect(screen.getByText("stablePlanDigest: sha256:narrow-plan-123")).toBeInTheDocument();
    expect(screen.getByText("narrowImplementationPlan: default_chat_adapter")).toBeInTheDocument();
    expect(screen.getByText("Implementation Scope")).toBeInTheDocument();
    expect(screen.getByText("Keep default Chat unchanged.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("records default chat adapter narrow implementation plan review without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "record_default_chat_adapter_narrow_implementation_plan_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "evidence-narrow-plan-review",
          decisionKind: args?.input?.decisionKind ?? "approve",
          sourceSessionId: "settings-dry-run",
          draftReady: true,
          narrowPlanDigest: "sha256:narrow-plan-review-123",
          planSectionCount: 8,
          createdAt: "2026-06-02T00:00:00Z",
          blockingReasons: [],
        });
      }
      if (cmd === "get_default_chat_adapter_narrow_implementation_plan_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "evidence-narrow-plan-review",
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            draftReady: true,
            narrowPlanDigest: "sha256:narrow-plan-review-123",
            planSectionCount: 8,
            w57Eligible: true,
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 19,
            reviewerNoteCategory: "brief",
            createdAt: "2026-06-02T00:00:00Z",
          },
          approvedCount: 1,
          rejectedCount: 0,
          requestReworkCount: 0,
          latestApprovedPlanDigest: "sha256:narrow-plan-review-123",
          latestTimestamp: "2026-06-02T00:00:00Z",
          blockingReasons: [],
          metadataSafeSummary: {
            narrowImplementationPlanReview: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Narrow Implementation Plan Review")
    ).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Narrow implementation reviewer note"), {
      target: { value: "Looks safe to review" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve Narrow Plan" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_default_chat_adapter_narrow_implementation_plan_review_decision",
        {
          input: {
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
            optionalReviewerNote: "Looks safe to review",
          },
        }
      );
    });
    expect(
      await screen.findByText("Narrow implementation plan review recorded")
    ).toBeInTheDocument();
    expect(screen.getByText("narrowPlanDigest: sha256:narrow-plan-review-123")).toBeInTheDocument();
    expect(
      screen.getByText("narrowImplementationPlanReview: default_chat_adapter")
    ).toBeInTheDocument();
    expect(screen.getByText("approvedCount: 1")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("checks default chat adapter narrow implementation plan approval readiness without migration controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_narrow_implementation_plan_approval_readiness") {
        return Promise.resolve({
          ready: true,
          draftReady: true,
          discussionGateEligible: true,
          narrowPlanReviewApproved: true,
          narrowPlanDigestMatched: true,
          currentPlanDigest: "sha256:narrow-plan-approval-current",
          latestApprovedPlanDigest: "sha256:narrow-plan-approval-current",
          latestDecision: {
            evidenceId: "evidence-narrow-plan-review",
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            draftReady: true,
            narrowPlanDigest: "sha256:narrow-plan-approval-current",
            planSectionCount: 8,
            w57Eligible: true,
            reviewerNoteChecksum: "sha256:reviewer-note",
            reviewerNoteLength: 19,
            reviewerNoteCategory: "brief",
            createdAt: "2026-06-02T00:00:00Z",
          },
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [],
          metadataSafeSummary: {
            narrowImplementationPlanApprovalReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Narrow Implementation Plan Approval Readiness")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Narrow Plan Approval Readiness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "check_default_chat_adapter_narrow_implementation_plan_approval_readiness",
        {
          input: {
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
          },
        }
      );
    });
    expect(await screen.findByText("Narrow plan approval readiness passed")).toBeInTheDocument();
    expect(screen.getByText("ready: true")).toBeInTheDocument();
    expect(screen.getByText("narrowPlanDigestMatched: true")).toBeInTheDocument();
    expect(
      screen.getByText("currentPlanDigest: sha256:narrow-plan-approval-current")
    ).toBeInTheDocument();
    expect(
      screen.getByText("narrowImplementationPlanApprovalReadiness: default_chat_adapter")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("checks default chat adapter contract harness without routing controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_contract_harness") {
        return Promise.resolve({
          contractHarnessReady: true,
          contractShape: "disabled_adapter_legacy_stream_contract",
          adapterDisabled: true,
          activationImplementationGateEligible: true,
          routingStatus: {
            currentMode: "legacy_stream",
            adapterScaffoldPresent: true,
            controlledAdapterEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            activationImplementationGateEligible: true,
            requiresSeparateCutoverImplementation: true,
            blockingReasons: [],
            metadataSafeSummary: {
              metadataSafe: true,
              readOnly: true,
            },
          },
          sendMessageContract: {
            name: "send_message",
            ready: true,
            expectedPath: "legacy_stream",
            actualPath: "legacy_stream",
            blockingReasons: [],
          },
          streamMessageContract: {
            name: "start_stream_message",
            ready: true,
            expectedPath: "legacy_stream",
            actualPath: "legacy_stream",
            blockingReasons: [],
          },
          blockingReasons: [],
          metadataSafeSummary: {
            contractHarness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            contractHarnessReady: true,
            contractShape: "disabled_adapter_legacy_stream_contract",
            adapterDisabled: true,
            activationImplementationGateEligible: true,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            controlledAdapterEnabled: false,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Contract Harness")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Adapter Contract Harness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_default_chat_adapter_contract_harness", {
        input: { requiredApprovedCandidates: 1 },
      });
    });
    expect(await screen.findByText("Adapter contract harness ready")).toBeInTheDocument();
    expect(
      screen.getAllByText("contractShape: disabled_adapter_legacy_stream_contract").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("adapterDisabled: true").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("activationImplementationGateEligible: true").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("controlledAdapterEnabled: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("defaultSendPath: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("startStreamPath: legacy_stream").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("send_message")).toBeInTheDocument();
    expect(screen.getByText("start_stream_message")).toBeInTheDocument();
    expect(screen.getAllByText("actualPath: legacy_stream").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("contractHarness: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter contract harness blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_contract_harness") {
        return Promise.resolve({
          contractHarnessReady: false,
          contractShape: "disabled_adapter_legacy_stream_contract",
          adapterDisabled: true,
          activationImplementationGateEligible: false,
          routingStatus: {
            currentMode: "legacy_stream",
            adapterScaffoldPresent: true,
            controlledAdapterEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            activationImplementationGateEligible: false,
            requiresSeparateCutoverImplementation: true,
            blockingReasons: ["activation_implementation_gate_not_eligible"],
            metadataSafeSummary: {},
          },
          sendMessageContract: {
            name: "send_message",
            ready: true,
            expectedPath: "legacy_stream",
            actualPath: "legacy_stream",
            blockingReasons: [],
          },
          streamMessageContract: {
            name: "start_stream_message",
            ready: true,
            expectedPath: "legacy_stream",
            actualPath: "legacy_stream",
            blockingReasons: [],
          },
          blockingReasons: ["activation_implementation_gate_not_eligible"],
          metadataSafeSummary: {
            contractHarness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            contractHarnessReady: false,
            activationImplementationGateEligible: false,
            blockingReasonCount: 1,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Check Adapter Contract Harness" }));

    expect(await screen.findByText("Adapter contract harness blocked")).toBeInTheDocument();
    expect(screen.getByText("activation_implementation_gate_not_eligible")).toBeInTheDocument();
    expect(screen.getAllByText("contractHarnessReady: false").length).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("activationImplementationGateEligible: false").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("runs default chat adapter dry run without enabling routing controls", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_dry_run") {
        return Promise.resolve({
          dryRunReady: true,
          blocked: false,
          contractShape: "default_chat_adapter_dry_run_contract",
          sourceSessionId: args?.input?.sessionId ?? "settings-dry-run",
          adapterPath: "controlled_adapter_dry_run",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: false,
          contractHarnessReady: true,
          inputMessageLength: 31,
          inputMessageHash: "abc123",
          blockingReasons: [],
          metadataSafeSummary: {
            adapterDryRun: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            dryRunReady: true,
            contractShape: "default_chat_adapter_dry_run_contract",
            adapterPath: "controlled_adapter_dry_run",
            allowWrites: false,
            maxToolCalls: 0,
            defaultChatPathUnchanged: true,
            chatMessageSaved: false,
            agentRunRecorded: false,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Dry Run")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Run Adapter Dry Run" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_default_chat_adapter_dry_run", {
        input: {
          sessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedCandidates: 1,
        },
      });
    });
    expect(await screen.findByText("Adapter dry run ready")).toBeInTheDocument();
    expect(
      screen.getAllByText("contractShape: default_chat_adapter_dry_run_contract").length
    ).toBeGreaterThanOrEqual(1);
    expect(
      screen.getAllByText("adapterPath: controlled_adapter_dry_run").length
    ).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("allowWrites: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("maxToolCalls: 0").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("defaultChatPathUnchanged: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("chatMessageSaved: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("agentRunRecorded: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("adapterDryRun: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter dry run blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_dry_run") {
        return Promise.resolve({
          dryRunReady: false,
          blocked: true,
          contractShape: "default_chat_adapter_dry_run_contract",
          sourceSessionId: "settings-dry-run",
          adapterPath: "blocked",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: false,
          contractHarnessReady: false,
          inputMessageLength: 31,
          inputMessageHash: "abc123",
          blockingReasons: [
            "contract_harness_not_ready",
            "activation_implementation_gate_not_eligible",
          ],
          metadataSafeSummary: {
            adapterDryRun: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            dryRunReady: false,
            blocked: true,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Adapter Dry Run" }));

    expect(await screen.findByText("Adapter dry run blocked")).toBeInTheDocument();
    expect(screen.getByText("contract_harness_not_ready")).toBeInTheDocument();
    expect(screen.getByText("activation_implementation_gate_not_eligible")).toBeInTheDocument();
    expect(screen.getAllByText("dryRunReady: false").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("blocked: true").length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("records default chat adapter dry-run review decision from Settings", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_dry_run") {
        return Promise.resolve({
          dryRunReady: true,
          blocked: false,
          contractShape: "default_chat_adapter_dry_run_contract",
          sourceSessionId: "settings-dry-run",
          adapterPath: "controlled_adapter_dry_run",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: false,
          contractHarnessReady: true,
          inputMessageLength: 31,
          inputMessageHash: "abc123",
          blockingReasons: [],
          metadataSafeSummary: {
            adapterDryRun: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            dryRunReady: true,
          },
        });
      }
      if (cmd === "record_default_chat_adapter_dry_run_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_dry_run_review_1",
          decisionKind: args?.input?.decisionKind ?? "approve",
          sourceSessionId: args?.input?.sourceSessionId ?? "settings-dry-run",
          contractShape: "default_chat_adapter_dry_run_contract",
          dryRunReady: true,
          dryRunSummaryDigest: "sha256:abc123",
          createdAt: "2026-05-31T00:00:00Z",
          blockingReasons: [],
        });
      }
      if (cmd === "get_default_chat_adapter_dry_run_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_dry_run_review_1",
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            contractShape: "default_chat_adapter_dry_run_contract",
            dryRunReady: true,
            dryRunSummaryDigest: "sha256:abc123",
            reviewerNoteChecksum: "sha256:def456",
            reviewerNoteLength: 0,
            reviewerNoteCategory: "none",
            createdAt: "2026-05-31T00:00:00Z",
          },
          approvedCount: 1,
          rejectOrReworkCount: 0,
          latestTimestamp: "2026-05-31T00:00:00Z",
          blockingReasons: [],
          metadataSafeSummary: {
            dryRunReview: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            approvedCount: 1,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Adapter Dry Run" }));
    expect(await screen.findByText("Adapter dry run ready")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Approve Dry Run Review" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("record_default_chat_adapter_dry_run_review_decision", {
        input: {
          decisionKind: "approve",
          sourceSessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedCandidates: 1,
        },
      });
    });
    expect(await screen.findByText("Dry-run review evidence recorded")).toBeInTheDocument();
    expect(screen.getByText("decisionKind: approve")).toBeInTheDocument();
    expect(screen.getByText("dryRunReady: true")).toBeInTheDocument();
    expect(screen.getByText("dryRunReview: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("blocks default chat adapter dry-run approval evidence when dry run is blocked", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_dry_run") {
        return Promise.resolve({
          dryRunReady: false,
          blocked: true,
          contractShape: "default_chat_adapter_dry_run_contract",
          sourceSessionId: "settings-dry-run",
          adapterPath: "blocked",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: false,
          contractHarnessReady: false,
          inputMessageLength: 31,
          inputMessageHash: "abc123",
          blockingReasons: ["contract_harness_not_ready"],
          metadataSafeSummary: {
            adapterDryRun: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            dryRunReady: false,
          },
        });
      }
      if (cmd === "record_default_chat_adapter_dry_run_review_decision") {
        return Promise.resolve({
          recorded: false,
          decisionKind: args?.input?.decisionKind ?? "approve",
          sourceSessionId: "settings-dry-run",
          contractShape: "default_chat_adapter_dry_run_contract",
          dryRunReady: false,
          dryRunSummaryDigest: "sha256:blocked",
          createdAt: "2026-05-31T00:00:00Z",
          blockingReasons: ["contract_harness_not_ready", "dry_run_not_ready_for_approval"],
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Adapter Dry Run" }));
    expect(await screen.findByText("Adapter dry run blocked")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Approve Dry Run Review" }));

    expect(await screen.findByText("Dry-run review not recorded")).toBeInTheDocument();
    expect(screen.getByText("dry_run_not_ready_for_approval")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter implementation readiness as a read-only gate", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_implementation_readiness") {
        return Promise.resolve({
          implementationReady: true,
          latestDryRunReviewDecision: {
            evidenceId: "ev_dry_run_review_1",
            decisionKind: "approve",
            sourceSessionId: args?.input?.sourceSessionId ?? "settings-dry-run",
            contractShape: "default_chat_adapter_dry_run_contract",
            dryRunReady: true,
            dryRunSummaryDigest: "sha256:abc123",
            reviewerNoteChecksum: "sha256:def456",
            reviewerNoteLength: 0,
            reviewerNoteCategory: "none",
            createdAt: "2026-05-31T00:00:00Z",
          },
          activationImplementationGateEligible: true,
          contractHarnessReady: true,
          dryRunReady: true,
          dryRunReviewApproved: true,
          dryRunDigestMatched: true,
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          blockingReasons: [],
          metadataSafeSummary: {
            implementationReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            implementationReady: true,
            dryRunReviewApproved: true,
            controlledAdapterEnabled: false,
            automaticMigrationEnabled: false,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Implementation Readiness")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Adapter Implementation Readiness" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_default_chat_adapter_implementation_readiness", {
        input: {
          sourceSessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedCandidates: 1,
        },
      });
    });
    expect(await screen.findByText("Implementation readiness ready")).toBeInTheDocument();
    expect(screen.getByText("implementationReady: true")).toBeInTheDocument();
    expect(screen.getByText("dryRunReviewApproved: true")).toBeInTheDocument();
    expect(screen.getByText("dryRunDigestMatched: true")).toBeInTheDocument();
    expect(screen.getByText("implementationReadiness: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter implementation readiness blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "check_default_chat_adapter_implementation_readiness") {
        return Promise.resolve({
          implementationReady: false,
          latestDryRunReviewDecision: null,
          activationImplementationGateEligible: false,
          contractHarnessReady: false,
          dryRunReady: false,
          dryRunReviewApproved: false,
          dryRunDigestMatched: false,
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          blockingReasons: [
            "activation_implementation_gate_not_eligible",
            "dry_run_review_approval_missing",
          ],
          metadataSafeSummary: {
            implementationReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            implementationReady: false,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(
      await screen.findByRole("button", { name: "Check Adapter Implementation Readiness" })
    );

    expect(await screen.findByText("Implementation readiness blocked")).toBeInTheDocument();
    expect(screen.getByText("activation_implementation_gate_not_eligible")).toBeInTheDocument();
    expect(screen.getByText("dry_run_review_approval_missing")).toBeInTheDocument();
    expect(screen.getByText("implementationReady: false")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("runs default chat adapter controlled preview only from the explicit Settings panel", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_controlled_preview") {
        return Promise.resolve({
          previewReady: true,
          blocked: false,
          contractShape: "send_message_compatible",
          sourceSessionId: args?.input?.sourceSessionId ?? "settings-dry-run",
          adapterPath: "controlled_adapter_preview",
          reply: "Controlled adapter preview reply",
          reasoningTrace: {
            strategyResult: {
              adapterPreview: "default_chat_adapter_controlled_preview",
              metadataSafe: true,
            },
          },
          toolCalls: [],
          runId: "run-adapter-preview-1",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: true,
          implementationReady: true,
          warnings: ["controlled adapter preview forced allowWrites=false"],
          blockingReasons: [],
          metadataSafeSummary: {
            adapterPreview: "default_chat_adapter_controlled_preview",
            metadataSafe: true,
            allowWrites: false,
            maxToolCalls: 0,
            chatHistoryStorage: "none",
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Controlled Preview")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Run Adapter Controlled Preview" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_default_chat_adapter_controlled_preview", {
        input: {
          sourceSessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedCandidates: 1,
        },
      });
    });
    expect(await screen.findByText("Controlled preview ready")).toBeInTheDocument();
    expect(screen.getByText("Controlled adapter preview reply")).toBeInTheDocument();
    expect(screen.getByText("previewReady: true")).toBeInTheDocument();
    expect(screen.getAllByText("allowWrites: false").length).toBeGreaterThan(0);
    expect(screen.getAllByText("maxToolCalls: 0").length).toBeGreaterThan(0);
    expect(
      screen.getByText("adapterPreview: default_chat_adapter_controlled_preview")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter controlled preview blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_controlled_preview") {
        return Promise.resolve({
          previewReady: false,
          blocked: true,
          contractShape: "blocked",
          sourceSessionId: args?.input?.sourceSessionId ?? "settings-dry-run",
          adapterPath: "blocked",
          reply: null,
          reasoningTrace: {
            strategyResult: {
              adapterPreview: "default_chat_adapter_controlled_preview",
              blockedBeforeRuntime: true,
            },
          },
          toolCalls: [],
          runId: null,
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: false,
          implementationReady: false,
          warnings: [],
          blockingReasons: [
            "implementation_readiness_not_ready",
            "dry_run_review_approval_missing",
          ],
          metadataSafeSummary: {
            adapterPreview: "default_chat_adapter_controlled_preview",
            metadataSafe: true,
            blockedBeforeRuntime: true,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Adapter Controlled Preview" }));

    expect(await screen.findByText("Controlled preview blocked")).toBeInTheDocument();
    expect(screen.getByText("implementation_readiness_not_ready")).toBeInTheDocument();
    expect(screen.getByText("dry_run_review_approval_missing")).toBeInTheDocument();
    expect(screen.getByText("blockedBeforeRuntime: true")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("records default chat adapter controlled preview review evidence explicitly", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_default_chat_adapter_controlled_preview") {
        return Promise.resolve({
          previewReady: true,
          blocked: false,
          contractShape: "send_message_compatible",
          sourceSessionId: args?.input?.sourceSessionId ?? "settings-dry-run",
          adapterPath: "controlled_adapter_preview",
          reply: "Controlled adapter preview reply",
          reasoningTrace: {
            strategyResult: {
              adapterPreview: "default_chat_adapter_controlled_preview",
              metadataSafe: true,
            },
          },
          toolCalls: [],
          runId: "run-adapter-preview-review-1",
          allowWrites: false,
          maxToolCalls: 0,
          defaultChatPathUnchanged: true,
          chatMessageSaved: false,
          agentRunRecorded: true,
          implementationReady: true,
          warnings: [],
          blockingReasons: [],
          metadataSafeSummary: {
            adapterPreview: "default_chat_adapter_controlled_preview",
            metadataSafe: true,
            allowWrites: false,
            maxToolCalls: 0,
          },
        });
      }
      if (cmd === "record_default_chat_adapter_controlled_preview_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_adapter_preview_review_1",
          previewRunId: args?.input?.previewRunId,
          decisionKind: args?.input?.decisionKind,
          contractShape: "send_message_compatible",
          previewSummaryDigest: "sha256:preview123",
          createdAt: "2026-05-31T00:00:00Z",
          blockingReasons: [],
        });
      }
      if (cmd === "get_default_chat_adapter_controlled_preview_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_adapter_preview_review_1",
            previewRunId: "run-adapter-preview-review-1",
            decisionKind: "approve",
            contractShape: "send_message_compatible",
            previewSummaryDigest: "sha256:preview123",
            reviewerNoteChecksum: "sha256:note123",
            reviewerNoteLength: 11,
            reviewerNoteCategory: "brief",
            createdAt: "2026-05-31T00:00:00Z",
          },
          approvedCount: 1,
          rejectOrReworkCount: 0,
          latestTimestamp: "2026-05-31T00:00:00Z",
          blockingReasons: [],
          metadataSafeSummary: {
            controlledPreviewReview: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            reviewerNoteStorage: "length_checksum_category_only",
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Adapter Controlled Preview" }));
    expect(await screen.findByText("Controlled preview ready")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Optional preview review note"), {
      target: { value: "Looks safe." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve Controlled Preview" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_default_chat_adapter_controlled_preview_review_decision",
        {
          input: {
            previewRunId: "run-adapter-preview-review-1",
            decisionKind: "approve",
            optionalReviewerNote: "Looks safe.",
          },
        }
      );
    });
    expect(await screen.findAllByText(/ev_adapter_preview_review_1/)).toHaveLength(2);
    expect(screen.getAllByText("previewRunId: run-adapter-preview-review-1")).toHaveLength(2);
    expect(screen.getByText("approvedCount: 1")).toBeInTheDocument();
    expect(screen.getByText("controlledPreviewReview: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter controlled preview approval readiness", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_controlled_preview_approval_readiness") {
        return Promise.resolve({
          ready: true,
          requiredApprovedPreviews: args?.input?.requiredApprovedPreviews ?? 1,
          approvedPreviewCount: 1,
          latestDecision: {
            evidenceId: "ev_adapter_preview_review_1",
            previewRunId: "run-adapter-preview-review-1",
            decisionKind: "approve",
            contractShape: "send_message_compatible",
            previewSummaryDigest: "sha256:preview123",
            reviewerNoteChecksum: "sha256:note123",
            reviewerNoteLength: 11,
            reviewerNoteCategory: "brief",
            createdAt: "2026-05-31T00:00:00Z",
          },
          verifiedPreviewRunIds: ["run-adapter-preview-review-1"],
          implementationReadinessReady: true,
          previewReviewApproved: true,
          previewDigestMatched: true,
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [],
          metadataSafeSummary: {
            controlledPreviewApprovalReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Controlled Preview Approval Readiness")
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Check Controlled Preview Approval Readiness" })
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "check_default_chat_adapter_controlled_preview_approval_readiness",
        {
          input: {
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
          },
        }
      );
    });
    expect(
      await screen.findByText("Controlled preview approval readiness ready")
    ).toBeInTheDocument();
    expect(screen.getAllByText("ready: true").length).toBeGreaterThan(0);
    expect(screen.getAllByText("previewReviewApproved: true").length).toBeGreaterThan(0);
    expect(screen.getAllByText("previewDigestMatched: true").length).toBeGreaterThan(0);
    expect(
      screen.getByText("verifiedPreviewRunIds: run-adapter-preview-review-1")
    ).toBeInTheDocument();
    expect(
      screen.getByText("controlledPreviewApprovalReadiness: default_chat_adapter")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter controlled preview approval readiness blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_default_chat_adapter_controlled_preview_approval_readiness") {
        return Promise.resolve({
          ready: false,
          requiredApprovedPreviews: args?.input?.requiredApprovedPreviews ?? 1,
          approvedPreviewCount: 0,
          latestDecision: null,
          verifiedPreviewRunIds: [],
          implementationReadinessReady: true,
          previewReviewApproved: false,
          previewDigestMatched: false,
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [
            "controlled_preview_review_decision_missing",
            "controlled_preview_review_approval_missing",
          ],
          metadataSafeSummary: {
            controlledPreviewApprovalReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            ready: false,
            blockingReasonCount: 2,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Check Controlled Preview Approval Readiness",
      })
    );

    expect(
      await screen.findByText("Controlled preview approval readiness blocked")
    ).toBeInTheDocument();
    expect(screen.getByText("controlled_preview_review_decision_missing")).toBeInTheDocument();
    expect(screen.getByText("controlled_preview_review_approval_missing")).toBeInTheDocument();
    expect(screen.getAllByText("ready: false").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter cutover implementation plan draft", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_default_chat_adapter_cutover_implementation_plan") {
        return Promise.resolve({
          draftReady: true,
          controlledPreviewApprovalReadiness: {
            ready: true,
            requiredApprovedPreviews: args?.input?.requiredApprovedPreviews ?? 1,
            approvedPreviewCount: 1,
            latestDecision: {
              evidenceId: "ev_adapter_preview_review_1",
              previewRunId: "run-adapter-preview-review-1",
              decisionKind: "approve",
              contractShape: "send_message_compatible",
              previewSummaryDigest: "sha256:preview123",
              reviewerNoteChecksum: "sha256:note123",
              reviewerNoteLength: 11,
              reviewerNoteCategory: "brief",
              createdAt: "2026-05-31T00:00:00Z",
            },
            verifiedPreviewRunIds: ["run-adapter-preview-review-1"],
            implementationReadinessReady: true,
            previewReviewApproved: true,
            previewDigestMatched: true,
            defaultChatUnchanged: true,
            controlledAdapterEnabled: false,
            automaticMigrationEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            blockingReasons: [],
            metadataSafeSummary: {
              controlledPreviewApprovalReadiness: "default_chat_adapter",
              metadataSafe: true,
              readOnly: true,
            },
          },
          manualReviewRequired: true,
          notAutomaticMigration: true,
          requiresSeparateImplementation: true,
          requiresSeparateCutoverReview: true,
          sourceSessionId: "settings-dry-run",
          inputMessageLength: 31,
          inputMessageHash: "sha256:message123",
          stablePlanDigest: "sha256:cutover-plan-123",
          planSections: [
            {
              sectionKey: "implementationScope",
              title: "Implementation Scope",
              items: ["Keep default Chat unchanged."],
            },
            {
              sectionKey: "explicitNonGoals",
              title: "Explicit Non Goals",
              items: ["Do not migrate default Chat."],
            },
          ],
          blockingReasons: [],
          metadataSafeSummary: {
            cutoverImplementationPlan: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            notAutomaticMigration: true,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Cutover Implementation Plan")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Draft Cutover Implementation Plan" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "draft_default_chat_adapter_cutover_implementation_plan",
        {
          input: {
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
          },
        }
      );
    });
    expect(await screen.findByText("Cutover implementation plan ready")).toBeInTheDocument();
    expect(screen.getByText("stablePlanDigest: sha256:cutover-plan-123")).toBeInTheDocument();
    expect(screen.getByText("implementationScope")).toBeInTheDocument();
    expect(screen.getByText("Explicit Non Goals")).toBeInTheDocument();
    expect(screen.getByText("cutoverImplementationPlan: default_chat_adapter")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter cutover implementation plan blockers without sections", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "draft_default_chat_adapter_cutover_implementation_plan") {
        return Promise.resolve({
          draftReady: false,
          controlledPreviewApprovalReadiness: {
            ready: false,
            requiredApprovedPreviews: args?.input?.requiredApprovedPreviews ?? 1,
            approvedPreviewCount: 0,
            latestDecision: null,
            verifiedPreviewRunIds: [],
            implementationReadinessReady: true,
            previewReviewApproved: false,
            previewDigestMatched: false,
            defaultChatUnchanged: true,
            controlledAdapterEnabled: false,
            automaticMigrationEnabled: false,
            defaultSendPath: "legacy_stream",
            startStreamPath: "legacy_stream",
            blockingReasons: ["controlled_preview_review_approval_missing"],
            metadataSafeSummary: {
              controlledPreviewApprovalReadiness: "default_chat_adapter",
              metadataSafe: true,
              readOnly: true,
            },
          },
          manualReviewRequired: true,
          notAutomaticMigration: true,
          requiresSeparateImplementation: true,
          requiresSeparateCutoverReview: true,
          sourceSessionId: "settings-dry-run",
          inputMessageLength: 31,
          inputMessageHash: "sha256:message123",
          stablePlanDigest: null,
          planSections: [],
          blockingReasons: [
            "controlled_preview_approval_readiness_not_ready",
            "controlled_preview_review_approval_missing",
          ],
          metadataSafeSummary: {
            cutoverImplementationPlan: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
            draftReady: false,
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(
      await screen.findByRole("button", { name: "Draft Cutover Implementation Plan" })
    );

    expect(await screen.findByText("Cutover implementation plan blocked")).toBeInTheDocument();
    expect(screen.getByText("controlled_preview_approval_readiness_not_ready")).toBeInTheDocument();
    expect(screen.getByText("controlled_preview_review_approval_missing")).toBeInTheDocument();
    expect(screen.queryByText("Implementation Scope")).not.toBeInTheDocument();
    expect(screen.getAllByText("draftReady: false").length).toBeGreaterThan(0);
  });

  it("renders default chat adapter cutover plan review summary and records approval", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_default_chat_adapter_cutover_plan_review_summary") {
        return Promise.resolve({
          latestDecision: {
            evidenceId: "ev_cutover_plan_review_1",
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            draftReady: true,
            cutoverPlanDigest: "sha256:cutover-plan-review-123",
            planSectionCount: 9,
            reviewerNoteChecksum: "sha256:note123",
            reviewerNoteLength: 10,
            reviewerNoteCategory: "brief",
            createdAt: "2026-06-01T00:00:00Z",
          },
          approvedCount: 1,
          rejectedCount: 0,
          requestReworkCount: 0,
          latestApprovedPlanDigest: "sha256:cutover-plan-review-123",
          latestTimestamp: "2026-06-01T00:00:00Z",
          blockingReasons: [],
          metadataSafeSummary: {
            cutoverPlanReview: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
          },
        });
      }
      if (cmd === "record_default_chat_adapter_cutover_plan_review_decision") {
        return Promise.resolve({
          recorded: true,
          evidenceId: "ev_cutover_plan_review_2",
          decisionKind: "approve",
          sourceSessionId: "settings-dry-run",
          draftReady: true,
          cutoverPlanDigest: "sha256:cutover-plan-review-456",
          planSectionCount: 9,
          createdAt: "2026-06-01T00:01:00Z",
          blockingReasons: [],
        });
      }
      return mockInvoke(cmd);
    });

    renderSettings();

    await clickTab("实验");
    expect(await screen.findByText("Default Chat Adapter Cutover Plan Review")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Refresh Cutover Plan Review" }));
    expect(
      await screen.findByText("latestApprovedPlanDigest: sha256:cutover-plan-review-123")
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Cutover plan reviewer note"), {
      target: { value: "Approve reviewed plan" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve Cutover Plan" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "record_default_chat_adapter_cutover_plan_review_decision",
        {
          input: {
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            message: "Settings adapter dry-run probe.",
            requiredApprovedPreviews: 1,
            requiredApprovedCandidates: 1,
            optionalReviewerNote: "Approve reviewed plan",
          },
        }
      );
    });
    expect(await screen.findByText("Cutover plan review recorded")).toBeInTheDocument();
    expect(
      screen.getByText("cutoverPlanDigest: sha256:cutover-plan-review-456")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders blocked default chat adapter cutover plan review approval without evidence", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "record_default_chat_adapter_cutover_plan_review_decision") {
        return Promise.resolve({
          recorded: false,
          evidenceId: null,
          decisionKind: "approve",
          sourceSessionId: "settings-dry-run",
          draftReady: false,
          cutoverPlanDigest: null,
          planSectionCount: 0,
          createdAt: "2026-06-01T00:01:00Z",
          blockingReasons: ["cutover_implementation_plan_not_ready"],
        });
      }
      return mockInvoke(cmd);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Approve Cutover Plan" }));

    expect(await screen.findByText("Cutover plan review blocked")).toBeInTheDocument();
    expect(screen.getByText("cutover_implementation_plan_not_ready")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter cutover plan approval readiness as a read-only gate", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "check_default_chat_adapter_cutover_plan_approval_readiness") {
        return Promise.resolve({
          ready: true,
          draftReady: true,
          w45Ready: true,
          cutoverPlanReviewApproved: true,
          cutoverPlanDigestMatched: true,
          currentPlanDigest: "sha256:cutover-plan-approval-123",
          latestApprovedPlanDigest: "sha256:cutover-plan-approval-123",
          latestDecision: {
            evidenceId: "ev_cutover_plan_review_1",
            decisionKind: "approve",
            sourceSessionId: "settings-dry-run",
            draftReady: true,
            cutoverPlanDigest: "sha256:cutover-plan-approval-123",
            planSectionCount: 9,
            w45Ready: true,
            reviewerNoteChecksum: "sha256:note123",
            reviewerNoteLength: 10,
            reviewerNoteCategory: "brief",
            createdAt: "2026-06-01T00:00:00Z",
          },
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [],
          metadataSafeSummary: {
            cutoverPlanApprovalReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
          },
        });
      }
      return mockInvoke(cmd);
    });

    renderSettings();

    await clickTab("实验");
    expect(
      await screen.findByText("Default Chat Adapter Cutover Plan Approval Readiness")
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check Cutover Plan Approval" }));

    expect(await screen.findByText("Cutover plan approval ready")).toBeInTheDocument();
    expect(
      screen.getByText("currentPlanDigest: sha256:cutover-plan-approval-123")
    ).toBeInTheDocument();
    expect(screen.getByText("cutoverPlanDigestMatched: true")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith(
      "check_default_chat_adapter_cutover_plan_approval_readiness",
      {
        input: {
          sourceSessionId: "settings-dry-run",
          message: "Settings adapter dry-run probe.",
          requiredApprovedPreviews: 1,
          requiredApprovedCandidates: 1,
        },
      }
    );
    expect(screen.queryByRole("button", { name: /save to chat/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /promote/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /switch/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /activate/i })).not.toBeInTheDocument();
  });

  it("renders default chat adapter cutover plan approval readiness blockers", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "check_default_chat_adapter_cutover_plan_approval_readiness") {
        return Promise.resolve({
          ready: false,
          draftReady: true,
          w45Ready: true,
          cutoverPlanReviewApproved: false,
          cutoverPlanDigestMatched: false,
          currentPlanDigest: "sha256:cutover-plan-current",
          latestApprovedPlanDigest: null,
          latestDecision: null,
          defaultChatUnchanged: true,
          controlledAdapterEnabled: false,
          automaticMigrationEnabled: false,
          defaultSendPath: "legacy_stream",
          startStreamPath: "legacy_stream",
          blockingReasons: [
            "cutover_plan_review_decision_missing",
            "cutover_plan_review_approval_missing",
          ],
          metadataSafeSummary: {
            cutoverPlanApprovalReadiness: "default_chat_adapter",
            metadataSafe: true,
            readOnly: true,
          },
        });
      }
      return mockInvoke(cmd);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Check Cutover Plan Approval" }));

    expect(await screen.findByText("Cutover plan approval blocked")).toBeInTheDocument();
    expect(screen.getByText("cutover_plan_review_decision_missing")).toBeInTheDocument();
    expect(screen.getByText("cutover_plan_review_approval_missing")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /migrate/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /enable/i })).not.toBeInTheDocument();
  });

  it("renders shadow run gate blockers without controlled runtime success", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_controlled_chat_migration_shadow_run") {
        return Promise.resolve({
          shadowRunReady: false,
          implementationGateReport: {
            implementationEligible: false,
            latestDecision: null,
            readinessReport: {
              ready: false,
              requiredPromotions: 3,
              promotedCount: 1,
              recentPromotedPilotRunIds: ["run-controlled-pilot-1"],
              latestPromotionTimestamp: "2026-05-30T01:02:03Z",
              sourceTargetMismatchBlockCount: 0,
              metadataSafeEvidenceReady: true,
              defaultChatUnchanged: true,
              blockingReasons: ["insufficient_promotion_evidence: required 3 promotions, found 1"],
            },
            draftHashMatched: false,
            approvedAfterLatestDraft: false,
            blockingReasons: ["metadata_safe_approve_decision_missing"],
          },
          strategyKind: "notRun",
          payloadKind: "notRun",
          metadataSafeSummary: {
            blockedBeforeRuntime: true,
            metadataSafe: true,
          },
          warnings: [],
          blockingReasons: [
            "implementation_gate_blocked",
            "metadata_safe_approve_decision_missing",
          ],
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.reject(new Error("preview must not run from shadow panel"));
      }
      return mockInvoke(cmd, args);
    });

    renderSettings();

    await clickTab("实验");
    fireEvent.click(await screen.findByRole("button", { name: "Run Shadow Run" }));

    expect(await screen.findByText("Shadow blocked")).toBeInTheDocument();
    expect(screen.getByText("implementation_gate_blocked")).toBeInTheDocument();
    expect(screen.getByText("metadata_safe_approve_decision_missing")).toBeInTheDocument();
    expect(screen.getByText("Strategy: notRun")).toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "run_multi_strategy_agent_preview")
    ).toBe(false);
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
