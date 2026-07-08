import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import TodayPage from "./TodayPage";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const healthyDiagnostics = {
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
  mcp_recent_audit_count: 0,
  mcp_recent_pii_count: 0,
  memory_chunk_count: 42,
  vector_corrupt_embedding_count: 0,
  unfinished_builder_sessions: 0,
  pending_builder_review_sessions: 0,
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
  pending_proposal_count: 1,
  high_risk_pending_proposal_count: 0,
  proposal_store_status: "ok",
};

function renderPage() {
  render(
    <BrowserRouter>
      <TodayPage />
    </BrowserRouter>
  );
}

function lifeStateProjection({
  pendingCount = 1,
  safeMode = false,
}: {
  pendingCount?: number;
  safeMode?: boolean;
} = {}) {
  return {
    version: "life_state_projection_v1",
    generatedAt: "2026-07-08T00:00:00.000Z",
    pending: {
      pendingProposalCount: pendingCount,
      editedProposalCount: 0,
      totalReviewRequiredCount: pendingCount,
      highRiskReviewRequiredCount: 0,
      proposalStoreStatus: "ok",
      requiresUserAction: pendingCount > 0,
    },
    readiness: {
      chatReady: true,
      usageReady: true,
      lifeModelReady: true,
      modelEmpty: false,
      pendingBuilderReviewSessions: 0,
      unfinishedBuilderSessions: 0,
      databaseStatus: safeMode ? "degraded" : "ok",
      readinessIssues: [],
      usageReadinessIssues: [],
    },
    taskState: {
      taskStoreStatus: "ok",
      latestTaskId: null,
      latestTaskStatus: null,
      runningCount: 0,
      waitingPermissionCount: 0,
      blockedCount: 0,
      failedCount: 0,
      cancelledCount: 0,
      completedCount: 0,
      activeCount: 0,
    },
    safeMode: {
      active: safeMode,
      reason: safeMode ? "memory.db 初始化失败，正在使用临时数据库" : "系统当前未处于 Safe Mode。",
      sourceRefs: safeMode ? ["diagnostics.startup_warnings"] : [],
    },
    toolPermissions: {
      totalCount: 0,
      activeCount: 0,
      consumedCount: 0,
      allowCount: 0,
      denyCount: 0,
      askEveryTimeCount: 0,
      allowOnceCount: 0,
      allowUntilRevokedCount: 0,
    },
    safePaths: [],
    surfaces: ["today", "mailbox", "chat", "companion", "life_model", "settings"].map(surface => ({
      surface,
      pendingReviewCount: pendingCount,
      editedReviewCount: 0,
      totalReviewRequiredCount: pendingCount,
      readinessStatus: safeMode ? "blocked" : "ready",
      taskStatus: "idle",
      safeModeActive: safeMode,
      waitingPermissionCount: 0,
      activeToolPermissionCount: 0,
    })),
    sourceRefs: ["projection:test"],
  };
}

describe("TodayPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_state_projection") return Promise.resolve(lifeStateProjection());
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the Today product page with one daily goal and a next step", async () => {
    renderPage();

    expect(await screen.findByTestId("today-page")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "今日" })).toBeInTheDocument();
    expect(await screen.findByText("早起")).toBeInTheDocument();
    expect(screen.getByText("下一步")).toBeInTheDocument();
    expect(screen.getByText(/从「早起」开始/)).toBeInTheDocument();
  });

  it("calls only the lightweight wrappers needed for daily focus and pending confirmations", async () => {
    renderPage();

    await waitFor(() => {
      const calledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
      for (const command of ["get_life_state_projection", "get_daily_goals"]) {
        expect(calledCommands).toContain(command);
      }
      expect(calledCommands).not.toContain("get_system_diagnostics");
      expect(calledCommands).not.toContain("list_proposals");
      expect(calledCommands).not.toContain("get_pending_proposals");
      expect(calledCommands).not.toContain("count_memory_chunks");
      expect(calledCommands).not.toContain("get_state_alerts");
    });
  });

  it("renders a light empty state when there is no daily goal", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_daily_goals") return Promise.resolve([]);
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ pendingCount: 0 }));
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("今天还没有定下来")).toBeInTheDocument();
    expect(
      screen.getAllByRole("link", { name: "和 OpenLife 说一下现在的状态" })[0]
    ).toHaveAttribute("href", "/companion");
  });

  it("links pending proposals to mailbox without direct accept or reject actions", async () => {
    renderPage();

    expect(await screen.findByText("待确认 1")).toBeInTheDocument();
    expect(screen.getByTestId("today-card-pending-proposal")).toHaveAttribute(
      "data-card-type",
      "pending_proposal"
    );
    expect(screen.getByText("1 个待确认项需要你处理。")).toBeInTheDocument();
    expect(screen.getAllByRole("link", { name: "查看待确认项" })[0]).toHaveAttribute(
      "href",
      "/mailbox"
    );
    for (const label of ["同意", "不同意", "接受", "拒绝", "批量接受", "接受全部"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
    }
  });

  it("does not show direct-write actions in Safe Mode", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ safeMode: true }));
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect((await screen.findAllByText("Safe Mode")).length).toBeGreaterThan(0);
    for (const label of ["完成", "保存", "添加目标", "记录状态", "批量接受", "接受全部"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
    }
    expect(screen.getByRole("link", { name: "和 OpenLife 说一下现在的状态" })).toBeInTheDocument();
  });

  it("renders suspicious metric samples as state_signal, not as a goal, task, or next action", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_daily_goals") {
        return Promise.resolve([{ name: "qapressure = 8 points", done: false }]);
      }
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ pendingCount: 0 }));
      return mockInvoke(cmd, args);
    });

    renderPage();

    const stateSignal = await screen.findByTestId("today-card-state-signal");
    expect(stateSignal).toHaveAttribute("data-card-type", "state_signal");
    expect(stateSignal).toHaveTextContent("qapressure = 8 points");
    expect(screen.getByTestId("today-state-signals")).toHaveTextContent(
      "不会生成目标、任务或下一步行动"
    );
    expect(
      within(screen.getByTestId("today-goal-section")).queryByText(/qapressure/i)
    ).not.toBeInTheDocument();
    expect(
      within(screen.getByTestId("today-next-step")).queryByText(/qapressure/i)
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/从「qapressure = 8 points」开始/)).not.toBeInTheDocument();
    expect(screen.queryByTestId("today-card-task")).not.toBeInTheDocument();
  });

  it("uses LifeStateProjection review counts instead of diagnostics fallback", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ ...healthyDiagnostics, pending_proposal_count: 9 });
      }
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ pendingCount: 2 }));
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("待确认 2")).toBeInTheDocument();
    expect(screen.getByText("2 个待确认项需要你处理。")).toBeInTheDocument();
    expect(screen.queryByText("待确认 9")).not.toBeInTheDocument();
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_life_state_projection", undefined);
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "list_proposals")).toBe(false);
  });

  it("does not render dashboard-style status stats on Today", async () => {
    renderPage();

    expect(await screen.findByTestId("today-page")).toBeInTheDocument();
    for (const hiddenText of ["记忆条数", "记忆", "旧工作台", "本地优先", "Life Model 可用"]) {
      expect(screen.queryByText(hiddenText)).not.toBeInTheDocument();
    }
  });

  it("does not render raw LifeModel, memory, or proposal payloads", async () => {
    renderPage();

    expect(await screen.findByTestId("today-page")).toBeInTheDocument();
    for (const rawText of [
      "RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER",
      "RAW_LIFEMODEL_JSON_SHOULD_NOT_RENDER",
      "RAW_PROPOSAL_PAYLOAD_SHOULD_NOT_RENDER",
    ]) {
      expect(screen.queryByText(rawText)).not.toBeInTheDocument();
    }
  });

  it("does not import write, proposal-apply, migration, governed preview, or Skill Runtime wrappers", () => {
    const sourcePath = join(process.cwd(), "src/pages/TodayPage.tsx");
    const source = readFileSync(sourcePath, "utf8");
    for (const forbidden of [
      "saveLifeModel",
      "builderApplySignals",
      "batchAcceptLowRiskProposals",
      "acceptProposal",
      "rejectProposal",
      "runMultiStrategyAgentPreview",
      "runSkill",
      "getSkillRuntimeStatus",
      "checkRuntimeMigrationGate",
      "run_multi_strategy_agent_preview",
      "run_skill",
      "get_skill_runtime_status",
      "check_runtime_migration_gate",
    ]) {
      expect(source).not.toContain(forbidden);
    }
  });
});
