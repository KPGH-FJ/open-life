import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import OverviewTab from "./OverviewTab";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("OverviewTab", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  const defaultDiagnostics = {
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
  };

  const baseProps = {
    diagnostics: defaultDiagnostics as any,
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

  it("renders data file health section", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText(/数据文件健康/)).toBeInTheDocument();
  });

  it("renders recovery console in safe mode", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} safeMode={true} />
      </MemoryRouter>
    );
    expect(screen.getByText(/恢复控制台/)).toBeInTheDocument();
  });

  it("shows core link ready text when diagnostics is ok", () => {
    render(
      <MemoryRouter>
        <OverviewTab {...baseProps} />
      </MemoryRouter>
    );
    expect(screen.getByText(/核心链路已就绪/)).toBeInTheDocument();
  });
});
