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
