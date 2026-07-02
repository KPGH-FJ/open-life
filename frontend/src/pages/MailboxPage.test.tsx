import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
import { MemoryRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
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
      summary:
        "用户在构建复盘里反复提到睡眠节律影响第二天专注度，并确认下周先把作息恢复作为短期目标。",
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

const readableGoalProposal: AgentProposal = {
  id: "proposal-readable-goal-1",
  runId: "run-readable-goal-1",
  proposalType: "goal_update",
  source: "chat_conversation",
  sourceDetail: "chat-session-1",
  affectedPath: "goals.short_term[0].name",
  before: "睡前刷手机",
  after: "23 点前睡觉",
  reason: "用户明确表示近期想先修复睡眠节律。",
  confidence: 0.78,
  riskLevel: "low",
  status: "pending",
  whyOpenLifeThinksThis: "用户在对话中直接确认了新的短期目标。",
  createdAt: "2026-06-03T09:00:00.000Z",
  expiresAt: "2026-07-03T09:00:00.000Z",
};

const communicationStyleProposal: AgentProposal = {
  id: "proposal-communication-1",
  runId: "run-communication-1",
  proposalType: "preference_update",
  source: "feedback_evolution",
  sourceDetail: "maturation:preference.communication",
  affectedPath: "/preferences/communication",
  before: "建议太绕",
  after: "直接给结论，再解释原因",
  reason: "用户确认希望 OpenLife 更直接。",
  confidence: 0.91,
  riskLevel: "low",
  status: "pending",
  whyOpenLifeThinksThis: "用户在复盘中明确接受了更直接的沟通偏好。",
  evidenceSummaries: [
    {
      id: "ev-communication-1",
      summary: "对话证据支持更直接的沟通偏好。",
      sourceAssetIds: ["run-communication-1"],
      contentDigest: "sha256:communication",
    },
  ],
  behaviorChecks: [],
  createdAt: "2026-06-04T09:00:00.000Z",
  expiresAt: "2026-07-04T09:00:00.000Z",
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
        database_status: safeMode ? "degraded" : "ok",
        startup_warnings: safeMode ? ["memory.db 初始化失败，正在使用临时数据库"] : [],
        snapshot_count: 1,
        life_model_ready: true,
        app_version: "0.1.0",
        model_empty: false,
        chat_session_count: 1,
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

function renderMailboxPage(
  initialEntries: Array<string | { pathname: string; state?: unknown }> = ["/mailbox"]
) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <MailboxPage />
    </MemoryRouter>
  );
}

function ReviewRedirect() {
  const location = useLocation();
  return (
    <Navigate
      to={{ pathname: "/mailbox", search: location.search, hash: location.hash }}
      state={location.state}
      replace
    />
  );
}

function renderMailboxRoutes(
  initialEntries: Array<string | { pathname: string; search?: string; state?: unknown }> = [
    "/mailbox",
  ]
) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <Routes>
        <Route path="/review" element={<ReviewRedirect />} />
        <Route path="/mailbox" element={<MailboxPage />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("MailboxPage", () => {
  beforeEach(() => {
    mockProposals([lowRiskProposal, unsupportedProposal]);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the mailbox layout with proposal rows", async () => {
    renderMailboxPage();

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Mailbox" })).toBeInTheDocument();
    expect(screen.getByText("2 个待确认")).toBeInTheDocument();
    expect(screen.getAllByText("待确认").length).toBeGreaterThan(0);
    expect(screen.getByText("已同意")).toBeInTheDocument();
    expect(screen.getByText("已处理")).toBeInTheDocument();
    expect(screen.getByText("草稿修改")).toBeInTheDocument();
    expect(screen.getAllByText("OpenLife").length).toBeGreaterThan(0);
    expect(screen.getAllByText("新增目标").length).toBeGreaterThan(0);
    expect(screen.getAllByText("确认外部能力").length).toBeGreaterThan(0);
  });

  it("selects the matching proposal from /mailbox deep links", async () => {
    mockProposals([
      lowRiskProposal,
      { ...communicationStyleProposal, status: "accepted" },
      unsupportedProposal,
    ]);

    renderMailboxRoutes(["/mailbox?proposal=proposal-communication-1"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("更新沟通偏好");
    });
    expect(screen.getByTestId("mail-reader")).toHaveTextContent("直接给结论，再解释原因");
    expect(screen.getByRole("button", { name: /已同意 1/ })).toHaveClass("bg-stone-900");
  });

  it("keeps proposal selection after /review deep links redirect to Mailbox", async () => {
    mockProposals([lowRiskProposal, communicationStyleProposal, unsupportedProposal]);

    renderMailboxRoutes(["/review?proposal=proposal-communication-1#trace"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("更新沟通偏好");
    });
    expect(screen.queryByText("Review Center")).not.toBeInTheDocument();
  });

  it("shows a non-blocking not-found notice when a proposal deep link is unavailable", async () => {
    renderMailboxRoutes(["/mailbox?proposal=proposal-missing-1"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    expect(await screen.findByText("确认项不存在、已处理或不可见。")).toBeInTheDocument();
    expect(screen.getByText(/proposal-missing-1/)).toBeInTheDocument();
    expect(screen.getAllByText("新增目标").length).toBeGreaterThan(0);
  });

  it("selects rows and renders the selected proposal reader", async () => {
    renderMailboxPage();

    expect((await screen.findAllByText("新增目标")).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /确认外部能力/ }));

    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("插件请求获得写权限。");
    });
    expect(screen.getByTestId("mail-reader")).toHaveTextContent("OpenLife");
    expect(screen.getAllByText("确认外部能力").length).toBeGreaterThan(0);
    expect(screen.getByText(/影响与风险/)).toBeInTheDocument();
    expect(screen.getByText("技术详情")).toBeInTheDocument();
    expect(screen.getByText(/plugins\.demo\.write/)).toBeInTheDocument();
  });

  it("shows human reader sections with impact, confidence, and evidence summary", async () => {
    renderMailboxPage();

    expect(await screen.findByText("变化对比")).toBeInTheDocument();
    expect(screen.getByText("为什么问你")).toBeInTheDocument();
    expect(screen.getByText("依据")).toBeInTheDocument();
    expect(screen.getByText("来源摘要")).toBeInTheDocument();
    expect(screen.getByText("影响与风险")).toBeInTheDocument();
    expect(screen.getByText("你的回复")).toBeInTheDocument();
    expect(screen.getAllByText(/影响：低/).length).toBeGreaterThan(0);
    expect(screen.getByText(/把握：86%/)).toBeInTheDocument();
    const primarySurface = await screen.findByTestId("review-primary-surface");
    expect(primarySurface).toHaveTextContent("用户确认下周需要聚焦睡眠节律。");
    expect(primarySurface).toHaveTextContent("构建形成了这条候选更新");
    expect(
      within(primarySurface).queryByText(
        "Builder review produced a confirmed low-risk goal candidate."
      )
    ).not.toBeInTheDocument();
    expect(within(primarySurface).queryByText(/睡眠节律影响第二天专注度/)).not.toBeInTheDocument();
    expect(screen.getByText(/为什么问你/)).toBeInTheDocument();
    expect(screen.queryByText("raw-sensitive-payload-should-not-render")).not.toBeInTheDocument();
    expect(screen.queryByText(/sha256:abcdef1234567890/)).not.toBeInTheDocument();
  });

  it("shows readable before and after diff rows for ordinary low-risk updates", async () => {
    mockProposals([readableGoalProposal]);

    renderMailboxPage();

    expect(await screen.findByText("变化对比")).toBeInTheDocument();
    expect(screen.getAllByText("字段").length).toBeGreaterThan(0);
    expect(screen.getAllByText("当前值").length).toBeGreaterThan(0);
    expect(screen.getAllByText("将变为").length).toBeGreaterThan(0);
    expect(screen.getByText("name")).toBeInTheDocument();
    expect(screen.getByText("「睡前刷手机」")).toBeInTheDocument();
    expect(screen.getByText("「23 点前睡觉」")).toBeInTheDocument();
  });

  it("shows communication style proposals with path-specific trace details", async () => {
    mockProposals([communicationStyleProposal]);

    renderMailboxPage();

    expect((await screen.findAllByText("更新沟通偏好")).length).toBeGreaterThan(0);
    expect(screen.getByText("沟通偏好")).toBeInTheDocument();
    expect(screen.getByText("「建议太绕」")).toBeInTheDocument();
    expect(screen.getByText("「直接给结论，再解释原因」")).toBeInTheDocument();
    expect(screen.getByText("来源摘录：")).toBeInTheDocument();
    expect(screen.getAllByText("用户在复盘中明确接受了更直接的沟通偏好。").length).toBeGreaterThan(
      0
    );
    expect(screen.getByText("proposal-communication-1")).toBeInTheDocument();
    expect(screen.getAllByText("preferences.communication_style").length).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "run-communication-1" })).toHaveAttribute(
      "href",
      "#/runs/run-communication-1"
    );
    expect(screen.getByText("91%")).toBeInTheDocument();
    expect(screen.getByText("low")).toBeInTheDocument();
  });

  it("keeps long source text collapsed until expanded while main actions remain available", async () => {
    renderMailboxPage();

    expect(await screen.findByText("变化对比")).toBeInTheDocument();
    const primarySurface = await screen.findByTestId("review-primary-surface");
    expect(within(primarySurface).queryByText(/睡眠节律影响第二天专注度/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "展开" }));

    expect(await screen.findByTestId("review-expanded-source-details")).toHaveTextContent(
      "睡眠节律影响第二天专注度"
    );
    expect(screen.getByRole("button", { name: "同意" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "不同意" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "稍后再说" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "改一下" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "收起" }));

    await waitFor(() => {
      expect(screen.queryByTestId("review-expanded-source-details")).not.toBeInTheDocument();
    });
  });

  it("redacts sensitive payload-like values from the main diff panel", async () => {
    renderMailboxPage();

    expect(await screen.findByText("变化对比")).toBeInTheDocument();
    expect(
      screen.getAllByText("该字段可能包含敏感或原始内容，主面板只显示摘要。").length
    ).toBeGreaterThan(0);
    expect(screen.queryByText("raw-sensitive-payload-should-not-render")).not.toBeInTheDocument();
  });

  it("accepts a low-risk proposal through the existing acceptProposal command", async () => {
    renderMailboxPage();

    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

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

  it("resumes a Main Chat task after accepting the matching proposal route state", async () => {
    const taskId = "mainchat-task-resume-1";
    mockProposals([{ ...lowRiskProposal, sourceDetail: taskId }]);

    renderMailboxPage([
      {
        pathname: "/mailbox",
        state: { mainChatTaskSessionId: taskId, returnTo: "/companion" },
      },
    ]);

    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(await screen.findByText("Main Chat task ready to resume")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Resume Main Chat task" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "resume_main_chat_agent_task",
        expect.objectContaining({
          taskSessionId: taskId,
          task_session_id: taskId,
        })
      );
    });
  });

  it("does not allow unsupported proposal types to be accepted", async () => {
    renderMailboxPage();

    fireEvent.click(await screen.findByRole("button", { name: /确认外部能力/ }));

    const unsupportedAccept = await screen.findByRole("button", { name: "同意" });
    expect(unsupportedAccept).toBeDisabled();
    fireEvent.click(unsupportedAccept);

    await waitFor(() => {
      expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")).toBe(false);
    });
  });

  it("rejects through an existing quick reply command", async () => {
    renderMailboxPage();

    fireEvent.click(await screen.findByRole("button", { name: "不同意" }));

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

  it("postpones and starts edits through existing quick reply commands", async () => {
    renderMailboxPage();

    fireEvent.click(await screen.findByRole("button", { name: "稍后再说" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "postpone_proposal",
        expect.objectContaining({
          proposalId: "proposal-low-1",
          proposal_id: "proposal-low-1",
        })
      );
    });

    fireEvent.click(await screen.findByRole("button", { name: "改一下" }));
    const editField = await screen.findByLabelText("你想改成什么");
    expect(editField).toBeInTheDocument();
    expect((editField as HTMLTextAreaElement).value).toContain(
      "raw-sensitive-payload-should-not-render"
    );
  });

  it("keeps Safe Mode protection on accept and edit quick replies", async () => {
    mockProposals([lowRiskProposal], true);

    renderMailboxPage();

    expect(await screen.findByText("系统处于 Safe Mode")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "同意" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "改一下" })).toBeDisabled();

    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")).toBe(false);
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "edit_proposal")).toBe(false);
  });
});
