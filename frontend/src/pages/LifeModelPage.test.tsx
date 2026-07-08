import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
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
  policy_router: {
    activeAuthority: "IntentFrame + PolicyRouter",
    authorityChain: [
      "user_input",
      "IntentFrame",
      "PolicyRouter",
      "AgentIngressDecision",
      "OpenLifeTurnRuntime",
      "MainChatKernel",
    ],
    routeOutputs: [
      "direct_answer",
      "read_only_tool",
      "proposal_only_write",
      "plan_draft",
      "ask_clarification",
      "governed_blocker",
      "confirmation_request",
    ],
    appStateOldRoutersPresent: false,
    diagnosticsSurface: "policy_router_status",
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
    expect(screen.getAllByText("构建状态").length).toBeGreaterThan(0);
    expect(screen.getByText("快速构建")).toBeInTheDocument();
    expect(screen.getByText("对话构建")).toBeInTheDocument();
    expect(screen.getByText("从已有内容整理")).toBeInTheDocument();

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
        "get_life_model_current_view",
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
    const pendingPrimary = await screen.findByTestId(
      "life-model-pending-proposal-primary-proposal-life-model-1"
    );
    expect(
      within(pendingPrimary).queryByText("RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER")
    ).not.toBeInTheDocument();
  });

  it("shows an accepted communication preference in overview with proposal, patch, snapshot, and source trace", async () => {
    const currentView = {
      path: "preferences.communication_style",
      label: "沟通偏好",
      value: "先共情，再给结构化建议",
      unavailableReason: null,
      currentValueSource: "accepted_proposal",
      change: {
        path: "preferences.communication_style",
        proposalId: "proposal-communication-1",
        proposalStatus: "accepted",
        proposalSource: "feedback_evolution",
        proposalSourceDetail: "maturation:preference.communication",
        proposalRunId: "run-communication-1",
        sourceExcerpt: "用户接受低风险沟通偏好。",
        sourceUnavailableReason: null,
        confidence: 0.92,
        riskLevel: "low",
        before: "",
        after: "先共情，再给结构化建议",
        patchId: "patch-communication-1",
        patchStatus: "applied",
        patchPath: "preferences.communication_style",
        patchUnavailableReason: null,
        snapshotVersions: ["before-v1", "after-v1"],
        snapshotUnavailableReason: null,
        currentMatchesAcceptedAfter: true,
      },
    };
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.resolve({
          ...mockLifeModel,
          preferences: {
            ...mockLifeModel.preferences,
            communication_style: "先共情，再给结构化建议",
          },
        });
      }
      if (cmd === "get_life_model_current_view") return Promise.resolve(currentView);
      if (cmd === "get_system_diagnostics") return Promise.resolve(safeDiagnostics);
      if (cmd === "list_proposals") return Promise.resolve([]);
      return mockInvoke(cmd, args);
    });

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByTestId("communication-style-current-view")).toHaveTextContent(
      "先共情，再给结构化建议"
    );
    expect(screen.getByText("沟通偏好")).toBeInTheDocument();
    expect(screen.getByText("preferences.communication_style")).toBeInTheDocument();
    expect(screen.getByText("proposal-communication-1")).toBeInTheDocument();
    expect(screen.getByText("用户接受低风险沟通偏好。")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "run-communication-1" })).toHaveAttribute(
      "href",
      "#/runs/run-communication-1"
    );
    expect(screen.getByText(/patch-communication-1 · applied/)).toBeInTheDocument();
    expect(screen.getByText("before-v1 / after-v1")).toBeInTheDocument();
    expect(screen.getByText("92%")).toBeInTheDocument();
    expect(screen.getByText("low")).toBeInTheDocument();
  });

  it("shows typed unavailable reasons when accepted communication preference lacks patch or snapshot evidence", async () => {
    const currentView = {
      path: "preferences.communication_style",
      label: "沟通偏好",
      value: "直接一点，先给结论",
      unavailableReason: null,
      currentValueSource: "accepted_proposal",
      change: {
        path: "preferences.communication_style",
        proposalId: "proposal-communication-missing",
        proposalStatus: "accepted",
        proposalSource: "manual",
        proposalSourceDetail: null,
        proposalRunId: null,
        sourceExcerpt: null,
        sourceUnavailableReason: "source_excerpt_unavailable",
        confidence: 0.81,
        riskLevel: "low",
        before: "",
        after: "直接一点，先给结论",
        patchId: null,
        patchStatus: null,
        patchPath: null,
        patchUnavailableReason: "patch_missing",
        snapshotVersions: [],
        snapshotUnavailableReason: "snapshot_missing",
        currentMatchesAcceptedAfter: true,
      },
    };
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.resolve({
          ...mockLifeModel,
          preferences: {
            ...mockLifeModel.preferences,
            communication_style: "直接一点，先给结论",
          },
        });
      }
      if (cmd === "get_life_model_current_view") return Promise.resolve(currentView);
      if (cmd === "get_system_diagnostics") return Promise.resolve(safeDiagnostics);
      if (cmd === "list_proposals") return Promise.resolve([]);
      return mockInvoke(cmd, args);
    });

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByText("patch_missing")).toBeInTheDocument();
    expect(screen.getByText("snapshot_missing")).toBeInTheDocument();
    expect(screen.getByText("source_excerpt_unavailable")).toBeInTheDocument();
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
    expect(screen.getByRole("link", { name: "去构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
  });

  it("keeps builder, memory, and mailbox reachable", async () => {
    renderPage();

    expect(await screen.findByRole("link", { name: "开始快速构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
    expect(screen.getByRole("link", { name: "开始对话构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
    expect(screen.getByRole("button", { name: "暂不可用" })).toBeDisabled();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(screen.getByRole("link", { name: "查看记忆" })).toHaveAttribute("href", "/memory");
    expect(screen.getByRole("link", { name: "打开 Mailbox" })).toHaveAttribute("href", "/mailbox");
  });

  it("uses product language instead of engineering readiness/proposal labels in the build tab", async () => {
    renderPage();

    expect((await screen.findAllByText("构建状态")).length).toBeGreaterThan(0);
    expect(
      screen.getByText("构建产生候选，Mailbox 确认后才会更新 Life Model。")
    ).toBeInTheDocument();
    expect(screen.queryByText("Builder readiness")).not.toBeInTheDocument();
    expect(screen.queryByText(/Builder review/)).not.toBeInTheDocument();
    expect(screen.queryByText(/proposal/i)).not.toBeInTheDocument();
  });

  it("keeps pending proposal source text out of the ordinary evidence summary", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("tab", { name: "依据" }));

    const primary = await screen.findByTestId(
      "life-model-pending-proposal-primary-proposal-life-model-1"
    );
    expect(primary).toHaveTextContent("OpenLife 发现一条候选更新");
    expect(within(primary).queryByText(/Builder review produced/i)).not.toBeInTheDocument();

    expect(screen.getByText("来源与技术记录")).toBeInTheDocument();
    expect(
      screen.getByText("Builder review produced a low-risk model update.")
    ).toBeInTheDocument();
    expect(screen.getByText("RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER")).toBeInTheDocument();
  });

  it("shows three local handling actions for Life Model quality issues without writing data", async () => {
    const lowQualityModel = {
      ...mockLifeModel,
      identity: {
        ...mockLifeModel.identity,
        name: "l",
      },
      goals: {
        ...mockLifeModel.goals,
        daily: [{ name: "已记录状态 qapressure = 8 points", done: false }],
      },
      capabilities: {
        ...mockLifeModel.capabilities,
        skills: [{ name: "l", proficiency: 0.2, description: "" }],
      },
      state: {
        ...mockLifeModel.state,
        focus_areas: ["工作", "工作"],
      },
    };
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") return Promise.resolve(lowQualityModel);
      if (cmd === "get_system_diagnostics") return Promise.resolve(safeDiagnostics);
      if (cmd === "list_proposals") return Promise.resolve([pendingProposal]);
      return mockInvoke(cmd, args);
    });

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByText("发现可能影响画像可信度的字段")).toBeInTheDocument();
    expect(
      screen.getByText("本次视图处理，不会改写 Life Model；正式更新仍需 Mailbox 确认。")
    ).toBeInTheDocument();
    expect(screen.getByText("身份摘要过短")).toBeInTheDocument();
    expect(screen.getByText("目标里混入了状态或系统回执")).toBeInTheDocument();
    expect(screen.getByText("能力字段像碎片句")).toBeInTheDocument();
    expect(screen.getByText("状态标签重复")).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "修正" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "不采用" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "稍后处理" }).length).toBeGreaterThan(0);

    fireEvent.click(screen.getAllByRole("button", { name: "稍后处理" })[0]);
    expect(screen.getByText("已标记稍后处理")).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "不采用" })[0]);
    expect(screen.queryByText("身份摘要过短")).not.toBeInTheDocument();

    const calledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    for (const forbidden of [
      "accept_proposal",
      "edit_proposal",
      "save_life_model",
      "builder_apply_signals",
      "batch_accept_low_risk_proposals",
    ]) {
      expect(calledCommands).not.toContain(forbidden);
    }
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
    expect(screen.getByRole("link", { name: "开始快速构建" })).toBeInTheDocument();
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
