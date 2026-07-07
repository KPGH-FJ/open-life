import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import OverviewTab from "./OverviewTab";
import { vi } from "vitest";

const baseProps = {
  diagnostics: {
    policy_router: { activeAuthority: "IntentFrame + PolicyRouter", authorityChain: ["user_input", "IntentFrame", "PolicyRouter", "AgentIngressDecision", "OpenLifeTurnRuntime", "MainChatKernel"], routeOutputs: ["direct_answer", "read_only_tool", "proposal_only_write", "plan_draft", "ask_clarification", "governed_blocker", "confirmation_request"], appStateOldRoutersPresent: false, diagnosticsSurface: "policy_router_status" },
    mcp_server_count: 1,
    mcp_tool_count: 2,
    mcp_recent_audit_count: 0,
    mcp_recent_pii_count: 0,
    memory_chunk_count: 42,
    vector_corrupt_embedding_count: 0,
    unfinished_builder_sessions: 0,
    pending_builder_review_sessions: 0,
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
    snapshot_count: 2,
    life_model_ready: true,
    app_version: "0.1.0",
    model_empty: false,
    chat_session_count: 3,
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
    config_source: "env+default",
    agent_run_count: 5,
    agent_run_store_status: "ok",
    pending_proposal_count: 0,
    high_risk_pending_proposal_count: 0,
    proposal_store_status: "ok",
  } as any,
  safeMode: false,
  exportLoading: false,
  handleExport: vi.fn(),
  refreshAllDiagnostics: vi.fn(),
  tierLoading: false,
  setTierLoading: vi.fn(),
  rebuildLoading: false,
  setRebuildLoading: vi.fn(),
  rebuildResult: null,
  setRebuildResult: vi.fn(),
  handleVectorRebuild: vi.fn(),
  tierResult: null,
  setTierResult: vi.fn(),
  onNavigateTab: vi.fn(),
};

describe("OverviewTab readiness smoke", () => {
  it("shows ready state when all checks pass", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText("启动检查清单")).toBeInTheDocument();
    expect(screen.getByText("可开始使用")).toBeInTheDocument();
    expect(screen.getByText(/核心链路已就绪/)).toBeInTheDocument();
  });

  it("shows blocked state when chat is not ready", () => {
    const blockedDiagnostics = {
      ...baseProps.diagnostics,
      chat_ready: false,
      readiness_issues: ["聊天不可用：未配置模型"],
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: blockedDiagnostics }} />
      </MemoryRouter>
    );
    expect(screen.getByText("还有阻塞")).toBeInTheDocument();
    expect(screen.getByText(/按这些项逐个修复/)).toBeInTheDocument();
    expect(screen.getByText("聊天不可用：未配置模型")).toBeInTheDocument();
  });

  it("shows safe-mode state when safeMode is true", () => {
    const safeModeDiagnostics = {
      ...baseProps.diagnostics,
      startup_warnings: ["数据库降级启动"],
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: safeModeDiagnostics, safeMode: true }} />
      </MemoryRouter>
    );
    expect(screen.getAllByText("Safe Mode").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/建议先备份，再继续使用/)).toBeInTheDocument();
    expect(screen.getByText(/恢复控制台/)).toBeInTheDocument();
  });

  it("shows partial state when chat_ready but not usage ready", () => {
    const partialDiagnostics = {
      ...baseProps.diagnostics,
      chat_ready: true,
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: partialDiagnostics }} />
      </MemoryRouter>
    );
    expect(screen.getByText("闭环中")).toBeInTheDocument();
  });

  it("renders all readiness checklist items", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText("云端模型")).toBeInTheDocument();
    expect(screen.getByText("本地模型")).toBeInTheDocument();
    expect(screen.getAllByText("人生模型").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("数据文件")).toBeInTheDocument();
    expect(screen.getByText("对话验证")).toBeInTheDocument();
  });
});
