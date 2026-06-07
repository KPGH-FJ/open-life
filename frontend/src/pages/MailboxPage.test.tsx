import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import MailboxPage from "./MailboxPage";
import { mockInvoke } from "@/test/mocks/tauri";
import type { AgentProposal } from "../tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const lowRiskProposal: AgentProposal = {
  id: "proposal-low-1",
  runId: "run-low-1",
  proposalType: "goal_update",
  source: "builder_review",
  sourceDetail: "builder-session-1",
  affectedPath: "goals.short_term[0].name",
  before: "",
  after: {
    value: "raw-sensitive-payload-should-not-render",
    content_preview: "raw-sensitive-payload-should-not-render",
  },
  reason: "用户确认下周需要聚焦睡眠节律。",
  confidence: 0.86,
  riskLevel: "low",
  status: "pending",
  whyOpenLifeThinksThis: "Builder review produced a confirmed low-risk goal candidate.",
  evidenceSummaries: [
    {
      id: "ev-low-1",
      summary: "Builder confirmation supports the candidate.",
      sourceAssetIds: ["run-low-1"],
      contentDigest: "sha256:abcdef1234567890",
    },
  ],
  behaviorChecks: [
    {
      id: "proposal_first",
      label: "Proposal-first write path",
      passed: true,
      summary: "No durable write happens before user confirmation.",
    },
  ],
  createdAt: "2026-06-01T10:00:00.000Z",
  expiresAt: "2026-07-01T10:00:00.000Z",
};

const unsupportedProposal: AgentProposal = {
  id: "proposal-plugin-1",
  runId: "run-plugin-1",
  proposalType: "plugin_permission",
  source: "plugin",
  sourceDetail: "plugin_id=demo;candidate_digest=sha256:plugin",
  affectedPath: "plugins.demo.write",
  before: null,
  after: { pluginId: "demo", permission: "write" },
  reason: "插件请求获得写权限。",
  confidence: 0.62,
  riskLevel: "medium",
  status: "pending",
  createdAt: "2026-06-02T11:00:00.000Z",
  expiresAt: "2026-07-02T11:00:00.000Z",
};

function mockProposals(proposals: AgentProposal[], safeMode = false) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
    if (cmd === "list_proposals") {
      return Promise.resolve(proposals);
    }
    if (cmd === "get_system_diagnostics") {
      return Promise.resolve({
        router: {
          onnx_available: false,
          onnx_disabled: false,
          active_backend: "regex",
          latency_threshold_us: 50000,
        },
        mcp_server_count: 0,
        mcp_tool_count: 0,
        mcp_recent_audit_count: 0,
        mcp_recent_pii_count: 0,
        memory_chunk_count: 0,
        vector_corrupt_embedding_count: safeMode ? 1 : 0,
        unfinished_builder_sessions: 0,
        ollama_online: true,
        local_model: "llama3",
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
        database_status: safeMode ? "degraded" : "ok",
        startup_warnings: safeMode ? ["memory.db 初始化失败，正在使用临时数据库"] : [],
        snapshot_count: 1,
        life_model_ready: true,
        app_version: "0.1.0",
        model_empty: false,
        chat_session_count: 1,
        onboarding_completed: true,
        beta_ready: true,
        beta_readiness_issues: [],
        builder_completion: {
          identity: 80,
          goals: 70,
          capabilities: 75,
          state: 65,
          overall: 72.5,
          lowest_dimension: "state",
        },
        data_files: {
          messages_db_exists: true,
          messages_db_size_mb: 0.1,
          vectors_db_exists: true,
          vectors_db_size_mb: 0.2,
          mcp_audit_db_exists: true,
          mcp_audit_db_size_mb: 0.1,
          config_yaml_exists: true,
          life_model_yaml_exists: true,
        },
        ollama_models: [],
        config_source: "default",
        agent_run_count: 0,
        agent_run_store_status: "ok",
        pending_proposal_count: proposals.filter(p => p.status === "pending").length,
        high_risk_pending_proposal_count: 0,
        proposal_store_status: "ok",
      });
    }
    if (cmd === "get_config") {
      return Promise.resolve({
        llm: {
          provider: "deepseek",
          openai_base: "https://api.deepseek.com",
          openai_key: "",
          embedding_model: "text-embedding-3-small",
          chat_model: "deepseek-chat",
          embedding_enabled: false,
        },
        prefer_local_model: false,
        local_model: "llama3",
        system: {
          safe_paths: ["/tmp/openlife-test"],
        },
      });
    }
    return mockInvoke(cmd, args);
  });
}

describe("MailboxPage", () => {
  beforeEach(() => {
    mockProposals([lowRiskProposal, unsupportedProposal]);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the mailbox layout with proposal rows", async () => {
    render(<MailboxPage />);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "邮箱" })).toBeInTheDocument();
    expect(screen.getByText("待处理")).toBeInTheDocument();
    expect(screen.getAllByText("Life Model").length).toBeGreaterThan(0);
    expect(screen.getByText("Goal update · goals.short_term[0].name")).toBeInTheDocument();
    expect(screen.getByText("Plugin permission · plugins.demo.write")).toBeInTheDocument();
  });

  it("selects rows and renders the selected proposal reader", async () => {
    render(<MailboxPage />);

    expect(await screen.findByText("Goal update · goals.short_term[0].name")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Plugin permission · plugins.demo.write/ }));

    expect(screen.getByTestId("mail-reader")).toHaveTextContent("Tool");
    expect(screen.getByText("Plugin permission · plugins.demo.write")).toBeInTheDocument();
    expect(screen.getByTestId("mail-reader")).toHaveTextContent("插件请求获得写权限。");
    expect(screen.getByText(/目标：plugins\.demo\.write/)).toBeInTheDocument();
  });

  it("shows reason, impact, and metadata-safe evidence in the reader", async () => {
    render(<MailboxPage />);

    expect(await screen.findByTestId("mail-reader")).toHaveTextContent(
      "Builder review produced a confirmed low-risk goal candidate."
    );
    expect(screen.getByText("Builder confirmation supports the candidate.")).toBeInTheDocument();
    expect(screen.getByText("Proposal-first write path")).toBeInTheDocument();
    expect(screen.queryByText("raw-sensitive-payload-should-not-render")).not.toBeInTheDocument();
  });

  it("accepts a low-risk proposal through the existing acceptProposal command", async () => {
    render(<MailboxPage />);

    fireEvent.click(await screen.findByRole("button", { name: "接受" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-low-1",
          proposal_id: "proposal-low-1",
        })
      );
    });
  });

  it("does not allow unsupported proposal types to be accepted", async () => {
    render(<MailboxPage />);

    fireEvent.click(await screen.findByRole("button", { name: /Plugin permission · plugins.demo.write/ }));

    const unsupportedAccept = await screen.findByRole("button", { name: "暂不支持" });
    expect(unsupportedAccept).toBeDisabled();
    fireEvent.click(unsupportedAccept);

    await waitFor(() => {
      expect(
        vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")
      ).toBe(false);
    });
  });

  it("rejects through an existing quick reply command", async () => {
    render(<MailboxPage />);

    fireEvent.click(await screen.findByRole("button", { name: "拒绝" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "reject_proposal",
        expect.objectContaining({
          proposalId: "proposal-low-1",
          proposal_id: "proposal-low-1",
        })
      );
    });
  });

  it("keeps Safe Mode protection on accept and edit quick replies", async () => {
    mockProposals([lowRiskProposal], true);

    render(<MailboxPage />);

    expect(await screen.findByText("系统处于 Safe Mode")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "接受（Safe Mode）" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "修改（Safe Mode）" })).toBeDisabled();

    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")).toBe(false);
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "edit_proposal")).toBe(false);
  });
});
