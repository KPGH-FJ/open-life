import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import OverviewTab from "./OverviewTab";
import { vi } from "vitest";

const baseProps = {
  diagnostics: {
    router: {
      onnx_available: true,
      onnx_disabled: false,
      active_backend: "regex",
      latency_threshold_us: 50000,
    },
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
    beta_ready: true,
    beta_readiness_issues: [],
    data_dir: "/tmp/openlife-test",
    active_data_dir: "/tmp/openlife-test",
    legacy_data_dir: null,
    database_status: "ok",
    startup_warnings: [],
    snapshot_count: 2,
    life_model_ready: true,
    app_version: "0.1.0",
    model_empty: false,
    chat_session_count: 3,
    onboarding_completed: true,
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
  tierResult: null,
  setTierResult: vi.fn(),
};

describe("P11 Smoke: OverviewTab readiness states", () => {
  it("shows ready state when all checks pass", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText("Beta Readiness 状态")).toBeInTheDocument();
    expect(screen.getAllByText("就绪").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/所有核心检查项通过/)).toBeInTheDocument();
  });

  it("shows blocked state when chat is not ready", () => {
    const blockedDiagnostics = {
      ...baseProps.diagnostics,
      chat_ready: false,
      beta_ready: false,
      beta_readiness_issues: ["核心聊天链路未就绪"],
      readiness_issues: ["聊天不可用：未配置模型"],
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: blockedDiagnostics }} />
      </MemoryRouter>
    );
    expect(screen.getByText("阻塞")).toBeInTheDocument();
    expect(screen.getByText(/存在阻塞项/)).toBeInTheDocument();
  });

  it("shows safe-mode state when safeMode is true", () => {
    const safeModeDiagnostics = {
      ...baseProps.diagnostics,
      startup_warnings: ["数据库降级启动"],
      beta_ready: false,
      beta_readiness_issues: ["数据存储曾在启动时降级"],
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: safeModeDiagnostics, safeMode: true }} />
      </MemoryRouter>
    );
    expect(screen.getAllByText("Safe Mode").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/请先导出备份并修复/)).toBeInTheDocument();
    expect(screen.getByText(/恢复控制台/)).toBeInTheDocument();
  });

  it("shows partial state when chat_ready but not beta_ready", () => {
    const partialDiagnostics = {
      ...baseProps.diagnostics,
      chat_ready: true,
      beta_ready: false,
      beta_readiness_issues: ["尚未开始任何对话"],
    };
    render(
      <MemoryRouter>
        <OverviewTab {...{ ...baseProps, diagnostics: partialDiagnostics }} />
      </MemoryRouter>
    );
    expect(screen.getByText("部分就绪")).toBeInTheDocument();
  });

  it("renders all readiness checklist items", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText("模型/Provider 就绪")).toBeInTheDocument();
    expect(screen.getByText("LifeModel 状态")).toBeInTheDocument();
    expect(screen.getByText("数据健康")).toBeInTheDocument();
    expect(screen.getByText("待处理提案")).toBeInTheDocument();
    expect(screen.getByText("AgentRun 记录")).toBeInTheDocument();
    expect(screen.getByText("备份/快照可用性")).toBeInTheDocument();
    expect(screen.getByText("诊断导出")).toBeInTheDocument();
  });
});
