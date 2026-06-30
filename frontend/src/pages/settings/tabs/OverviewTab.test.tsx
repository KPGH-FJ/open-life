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
    handleVectorRebuild: vi.fn(),
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

  it("does not show configured-only provider as cloud ready", () => {
    render(
      <MemoryRouter>
        <OverviewTab
          {...baseProps}
          diagnostics={
            {
              ...defaultDiagnostics,
              cloud_api_validated: false,
              cloud_api_validation_status: "unvalidated",
              runtime_route_evidence: {
                evidence_id: "settings-route-1",
                generated_at: "2026-06-29T00:00:00Z",
                answer_scope: "settings_readiness",
                provider_readiness: {
                  configured: true,
                  credential_present: true,
                  validated: false,
                  validation_status: "unvalidated",
                  preferred: "DeepSeek",
                  actually_used: null,
                  stale: false,
                  failed: false,
                  last_checked_at: null,
                },
                external_transmission: "not_instrumented",
                source_refs: [],
                truth_confidence: "inferred",
              },
            } as any
          }
        />
      </MemoryRouter>
    );

    expect(screen.getByText(/Configured, not validated/)).toBeInTheDocument();
    expect(screen.getByText(/不能当作 cloud-ready/)).toBeInTheDocument();
    expect(screen.queryByText(/云端模型 已配置/)).not.toBeInTheDocument();
  });
});
