import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import LifeModelPage from "./LifeModelPage";
import { createEmptyLifeModel, mockInvoke, mockLifeModel } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const safeDiagnostics = {
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
  memory_chunk_count: 12,
  vector_corrupt_embedding_count: 0,
  unfinished_builder_sessions: 1,
  pending_builder_review_sessions: 1,
  ollama_online: true,
  local_model: "llama3",
  resolved_local_model: "llama3:latest",
  prefer_local_model: true,
  cloud_api_configured: false,
  cloud_provider: "DeepSeek",
  cloud_api_validated: false,
  cloud_api_last_error: null,
  chat_ready: true,
  readiness_issues: [],
  data_dir: "/tmp/openlife-test",
  active_data_dir: "/tmp/openlife-test",
  legacy_data_dir: "/tmp/openlife-legacy",
  database_status: "ok",
  startup_warnings: [],
  snapshot_count: 2,
  life_model_ready: true,
  app_version: "0.1.0",
  model_empty: false,
  chat_session_count: 3,
  onboarding_completed: true,
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
  ollama_models: [],
  config_source: "env+default",
  agent_run_count: 0,
  agent_run_store_status: "ok",
  pending_proposal_count: 2,
  high_risk_pending_proposal_count: 0,
  proposal_store_status: "ok",
};

const pendingProposal = {
  id: "proposal-life-model-1",
  runId: "run-1",
  proposalType: "life_model_update",
  source: "builder_review",
  sourceDetail: "RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER",
  affectedPath: "identity.values",
  before: { raw: "RAW_LIFEMODEL_JSON_SHOULD_NOT_RENDER" },
  after: { raw: "RAW_LIFEMODEL_JSON_SHOULD_NOT_RENDER" },
  reason: "Builder review produced a low-risk model update.",
  confidence: 0.82,
  riskLevel: "low",
  status: "pending",
  createdAt: "2026-06-07T00:00:00.000Z",
};

function renderPage() {
  render(
    <BrowserRouter>
      <LifeModelPage />
    </BrowserRouter>
  );
}

describe("LifeModelPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") return Promise.resolve(safeDiagnostics);
      if (cmd === "list_proposals") return Promise.resolve([pendingProposal]);
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the Life Model product page and switches build, overview, and evidence sections", async () => {
    renderPage();

    expect(await screen.findByTestId("life-model-page")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Life Model" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "构建" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("构建状态")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "概览" }));
    expect(screen.getByRole("tab", { name: "概览" })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("Identity")).toBeInTheDocument();
    expect(screen.getByText("Goals")).toBeInTheDocument();
    expect(screen.getByText("Capabilities")).toBeInTheDocument();
    expect(screen.getByText("State")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(screen.getByRole("tab", { name: "依据" })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("记忆条数")).toBeInTheDocument();
    expect(screen.getByText("待确认更新")).toBeInTheDocument();
  });

  it("calls existing metadata-safe wrappers for model, diagnostics, memory, proposals, and builder state", async () => {
    renderPage();

    await waitFor(() => {
      const calledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
      for (const command of [
        "get_life_model",
        "get_system_diagnostics",
        "get_model_4d_completion",
        "builder_list_unfinished",
        "count_memory_chunks",
        "get_memory_tier_stats",
        "list_proposals",
      ]) {
        expect(calledCommands).toContain(command);
      }
    });
  });

  it("summarizes the Life Model without raw JSON or raw evidence payloads", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));
    expect(await screen.findByText("测试用户")).toBeInTheDocument();
    expect(screen.queryByText(JSON.stringify(mockLifeModel))).not.toBeInTheDocument();
    expect(screen.queryByText("RAW_LIFEMODEL_JSON_SHOULD_NOT_RENDER")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(await screen.findByText("最近依据来源")).toBeInTheDocument();
    expect(screen.queryByText("RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER")).not.toBeInTheDocument();
  });

  it("renders a light empty state when the model is empty", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") return Promise.resolve(createEmptyLifeModel());
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ ...safeDiagnostics, model_empty: true, life_model_ready: false });
      }
      if (cmd === "list_proposals") return Promise.resolve([]);
      return mockInvoke(cmd, args);
    });

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByText("模型还没有形成稳定摘要")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "去构建" })).toHaveAttribute("href", "/builder");
  });

  it("keeps builder, memory, and mailbox reachable", async () => {
    renderPage();

    expect(await screen.findByRole("link", { name: "打开 Builder" })).toHaveAttribute(
      "href",
      "/builder"
    );

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(screen.getByRole("link", { name: "查看记忆" })).toHaveAttribute("href", "/memory");
    expect(screen.getByRole("link", { name: "打开邮箱" })).toHaveAttribute("href", "/mailbox");
  });

  it("does not show direct-write actions in Safe Mode", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          ...safeDiagnostics,
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      if (cmd === "list_proposals") return Promise.resolve([pendingProposal]);
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByTestId("life-model-page")).toBeInTheDocument();
    for (const label of ["保存模型", "应用更改", "直接写入", "批量接受", "接受全部"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
    }
    expect(screen.getByRole("link", { name: "打开 Builder" })).toBeInTheDocument();
  });

  it("does not import write, migration, governed preview, or Skill Runtime wrappers", () => {
    const sourcePath = join(process.cwd(), "src/pages/LifeModelPage.tsx");
    const source = readFileSync(sourcePath, "utf8");
    for (const forbidden of [
      "saveLifeModel",
      "builderApplySignals",
      "batchAcceptLowRiskProposals",
      "runMultiStrategyAgentPreview",
      "run_skill",
      "runSkill",
      "get_skill_runtime_status",
      "getSkillRuntimeStatus",
      "check_runtime_migration_gate",
      "checkRuntimeMigrationGate",
    ]) {
      expect(source).not.toContain(forbidden);
    }
  });
});
