import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
import { MemoryRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import MailboxPage from "./MailboxPage";
import { mockInvoke } from "@/test/mocks/tauri";
import type {
  AgentProposal,
  ReviewAction,
  ReviewCenterViewModel,
  ReviewItem,
  ReviewItemDecisionStatus,
  ReviewItemMaterializationStatus,
} from "../tauri";

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

const editedMemoryProposal: AgentProposal = {
  ...communicationStyleProposal,
  id: "proposal-edited-memory-1",
  runId: "run-edited-memory-1",
  proposalType: "memory_write",
  source: "memory_governance",
  sourceDetail: "mainchat-task-memory-edit",
  affectedPath: "memory.user.preference.communication",
  before: "建议太绕",
  after: "直接给结论，再解释原因，然后给一条可执行动作",
  status: "edited",
  reason: "用户编辑了记忆候选内容，等待最终同意或不同意。",
};

const artifactProposal: AgentProposal = {
  ...lowRiskProposal,
  id: "proposal-artifact-1",
  runId: "run-artifact-1",
  proposalType: "external_write_action",
  source: "chat_conversation",
  sourceDetail: "mainchat-task-artifact",
  affectedPath: "filesystem./tmp/openlife-test/roadshow-summary.md",
  before: null,
  after: {
    path: "/tmp/openlife-test/roadshow-summary.md",
    content: "# Roadshow",
  },
  riskLevel: "high",
  reason: "用户要求确认后保存路演摘要。",
};

function buildLifeStateProjection(proposals: AgentProposal[], safeMode = false) {
  const pendingProposalCount = proposals.filter(p => p.status === "pending").length;
  const editedProposalCount = proposals.filter(p => p.status === "edited").length;
  const totalReviewRequiredCount = pendingProposalCount + editedProposalCount;
  return {
    version: "life_state_projection_v1",
    generatedAt: new Date().toISOString(),
    pending: {
      pendingProposalCount,
      editedProposalCount,
      totalReviewRequiredCount,
      highRiskReviewRequiredCount: proposals.filter(
        p =>
          (p.status === "pending" || p.status === "edited") &&
          (p.riskLevel === "high" || p.riskLevel === "critical")
      ).length,
      proposalStoreStatus: "ok",
      requiresUserAction: totalReviewRequiredCount > 0,
    },
    readiness: {
      chatReady: true,
      usageReady: true,
      lifeModelReady: true,
      modelEmpty: false,
      pendingBuilderReviewSessions: 0,
      unfinishedBuilderSessions: 0,
      databaseStatus: safeMode ? "degraded" : "ok",
      readinessIssues: safeMode ? ["database_degraded"] : [],
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
      reason: safeMode ? "测试 Safe Mode：存储降级。" : "系统当前未处于 Safe Mode。",
      sourceRefs: safeMode ? ["database_status"] : [],
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
    safePaths: ["/tmp/openlife-test"],
    surfaces: ["today", "mailbox", "chat", "companion", "life_model", "settings"].map(surface => ({
      surface,
      pendingReviewCount: pendingProposalCount,
      editedReviewCount: editedProposalCount,
      totalReviewRequiredCount: surface === "mailbox" ? totalReviewRequiredCount : 0,
      readinessStatus: safeMode ? "degraded" : "ready",
      taskStatus: "idle",
      safeModeActive: safeMode,
      waitingPermissionCount: 0,
      activeToolPermissionCount: 0,
    })),
    sourceRefs: [
      "proposal_store:pending_and_edited",
      "main_chat_agent_session_store",
      "tool_permission_store",
      "config:safe_paths",
    ],
  };
}

function reviewStatusFor(proposal: AgentProposal): ReviewItemDecisionStatus {
  if (proposal.status === "accepted") return "approved";
  if (proposal.status === "rejected") return "rejected";
  if (proposal.status === "edited") return "edited";
  if (proposal.status === "postponed") return "deferred";
  return "pending";
}

function materializationStatusFor(proposal: AgentProposal): ReviewItemMaterializationStatus {
  if (proposal.status === "accepted") return "unknown";
  if (proposal.status === "rejected") return "not_applicable";
  return "not_started";
}

function reviewAction(
  proposal: AgentProposal,
  kind: ReviewAction["kind"],
  enabled: boolean,
  disabledReason?: string
): ReviewAction {
  const effect =
    kind === "apply"
      ? "materialization_request"
      : kind === "resume"
        ? "task_resume_request"
        : kind === "view_evidence"
          ? "evidence_only"
          : "decision_only";
  return {
    id: `${proposal.id}:${kind}`,
    label: kind,
    kind,
    effect,
    enabled,
    disabledReason,
    requiresConfirmation: kind === "approve" || kind === "apply",
    targetReviewItemId: proposal.id,
    expectedMaterializationStatusAfterDispatch: kind === "approve" ? "unknown" : undefined,
  } as ReviewAction;
}

function reviewable(proposal: AgentProposal): boolean {
  return ["pending", "edited", "postponed"].includes(proposal.status);
}

function resumeRequiresMaterialization(proposal: AgentProposal): boolean {
  return proposal.proposalType !== "tool_permission";
}

function approveBlocker(proposal: AgentProposal, safeMode: boolean): string | undefined {
  if (safeMode) return "测试 Safe Mode：存储降级。";
  if (!reviewable(proposal)) return "Only pending, edited, or deferred review items can approve.";
  if (
    ["plugin_permission", "model_policy_change", "schedule_checkin", "unsupported"].includes(
      proposal.proposalType
    )
  ) {
    return "This review item type has no backend apply pathway yet.";
  }
  if (
    proposal.proposalType === "external_write_action" &&
    typeof proposal.after?.path === "string" &&
    !proposal.after.path.startsWith("/tmp/openlife-test")
  ) {
    return "The external write path is outside configured safe paths.";
  }
  return undefined;
}

function reviewItemFromProposal(proposal: AgentProposal, safeMode = false): ReviewItem {
  const status = reviewStatusFor(proposal);
  const approveReason = approveBlocker(proposal, safeMode);
  const decisionReason = reviewable(proposal)
    ? undefined
    : "Only pending, edited, or deferred review items can receive a review decision.";
  const actions: ReviewAction[] = [
    reviewAction(proposal, "approve", !approveReason, approveReason),
    reviewAction(proposal, "reject", !decisionReason, decisionReason),
    reviewAction(proposal, "later", !decisionReason, decisionReason),
    reviewAction(proposal, "edit", !approveReason, approveReason),
    reviewAction(proposal, "view_evidence", true),
  ];
  const taskResumeRelation =
    proposal.source === "chat_conversation" && proposal.sourceDetail
      ? {
          taskSessionId: proposal.sourceDetail,
          resumeRequiresMaterialization: resumeRequiresMaterialization(proposal),
          canRequestResume:
            status === "approved" &&
            (!resumeRequiresMaterialization(proposal) ||
              materializationStatusFor(proposal) === "applied" ||
              materializationStatusFor(proposal) === "not_applicable"),
          resumeActionId: `${proposal.id}:resume`,
          blockedReason:
            status !== "approved"
              ? "Approve before requesting task resume."
              : resumeRequiresMaterialization(proposal) &&
                  materializationStatusFor(proposal) === "unknown"
                ? "Materialization evidence is unknown; cannot request task resume yet."
                : undefined,
        }
      : undefined;
  if (taskResumeRelation) {
    actions.push(
      reviewAction(
        proposal,
        "resume",
        taskResumeRelation.canRequestResume,
        taskResumeRelation.blockedReason
      )
    );
  }
  return {
    id: proposal.id,
    type:
      proposal.proposalType === "life_model_update" ? "life_model_update" : proposal.proposalType,
    source: {
      kind: "proposal",
      proposalId: proposal.id,
      proposalSource: proposal.source,
      sourceDetail: proposal.sourceDetail,
      runId: proposal.runId,
    },
    status,
    materializationStatus: materializationStatusFor(proposal),
    allowedActions: actions,
    risk: proposal.riskLevel,
    expiresAt: proposal.expiresAt,
    evidenceRefs: [],
    targetRefs: [],
    taskResumeRelation,
  };
}

function buildReviewCenterViewModel(
  proposals: AgentProposal[],
  safeMode = false
): ReviewCenterViewModel {
  const items = proposals.map(proposal => reviewItemFromProposal(proposal, safeMode));
  return {
    batches: [],
    items,
    summary: {
      total: items.length,
      actionRequiredCount: items.filter(item =>
        item.allowedActions.some(
          action =>
            action.enabled && ["approve", "reject", "edit", "later", "revoke"].includes(action.kind)
        )
      ).length,
      blockedActionCount: items.reduce(
        (count, item) =>
          count +
          item.allowedActions.filter(action => !action.enabled && action.disabledReason).length,
        0
      ),
      byStatus: {},
      byRisk: {},
      byMaterializationStatus: {},
    },
  };
}

function buildReviewCenterEnvelope(proposals: AgentProposal[], safeMode = false) {
  const data = buildReviewCenterViewModel(proposals, safeMode);
  return {
    data,
    status: data.items.length === 0 ? "empty" : "ready",
    lastUpdatedAt: new Date().toISOString(),
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [] },
  };
}

function mockProposals(proposals: AgentProposal[], safeMode = false) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
    if (cmd === "list_proposals") {
      return Promise.resolve(proposals);
    }
    if (cmd === "get_life_state_projection") {
      return Promise.resolve(buildLifeStateProjection(proposals, safeMode));
    }
    if (cmd === "get_review_center_view_model") {
      return Promise.resolve(buildReviewCenterEnvelope(proposals, safeMode));
    }
    if (cmd === "get_system_diagnostics") {
      return Promise.resolve({
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

function mockMutableProposals(
  initialProposals: AgentProposal[],
  safeMode = false,
  acceptResponse: Record<string, unknown> = {
    success: true,
    effectStatus: "confirmed",
    proposalProjectionStatus: "confirmed",
    warnings: [],
  }
) {
  let mutableProposals = initialProposals.map(proposal => ({ ...proposal }));
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
    if (cmd === "list_proposals") {
      return Promise.resolve(mutableProposals);
    }
    if (cmd === "get_life_state_projection") {
      return Promise.resolve(buildLifeStateProjection(mutableProposals, safeMode));
    }
    if (cmd === "get_review_center_view_model") {
      return Promise.resolve(buildReviewCenterEnvelope(mutableProposals, safeMode));
    }
    if (cmd === "accept_proposal") {
      mutableProposals = mutableProposals.map(proposal =>
        proposal.id === args?.proposalId ? { ...proposal, status: "accepted" } : proposal
      );
      return Promise.resolve(acceptResponse);
    }
    if (cmd === "reject_proposal") {
      mutableProposals = mutableProposals.map(proposal =>
        proposal.id === args?.proposalId ? { ...proposal, status: "rejected" } : proposal
      );
      return Promise.resolve(undefined);
    }
    if (cmd === "postpone_proposal") {
      mutableProposals = mutableProposals.map(proposal =>
        proposal.id === args?.proposalId ? { ...proposal, status: "postponed" } : proposal
      );
      return Promise.resolve(undefined);
    }
    if (cmd === "edit_proposal") {
      mutableProposals = mutableProposals.map(proposal =>
        proposal.id === args?.proposalId
          ? { ...proposal, status: "edited", after: args?.after ?? proposal.after }
          : proposal
      );
      return Promise.resolve({
        proposalId: args?.proposalId,
        draftOnly: true,
        durableWriteExecuted: false,
        originalProvenancePreserved: true,
        status: "edited",
        beforeDigest: "sha256:before",
        afterDigest: "sha256:after",
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
    expect(screen.getByText("2 个待确认/已修改")).toBeInTheDocument();
    expect(screen.getAllByText("待确认").length).toBeGreaterThan(0);
    expect(screen.getByText("已同意")).toBeInTheDocument();
    expect(screen.getByText("已处理")).toBeInTheDocument();
    expect(screen.getByText("已修改待处理")).toBeInTheDocument();
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

    const initialRow = await screen.findByRole("button", { name: /新增目标/ });
    await waitFor(() => {
      expect(initialRow).toHaveAttribute("aria-pressed", "true");
    });
    const pluginRow = screen.getByRole("button", { name: /确认外部能力/ });
    fireEvent.click(pluginRow);

    await waitFor(() => {
      expect(pluginRow).toHaveAttribute("aria-pressed", "true");
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

  it("keeps a confirmed Memory write visibly pending when its projection is degraded", async () => {
    mockMutableProposals([{ ...editedMemoryProposal, status: "pending" }], false, {
      success: true,
      effectStatus: "confirmed",
      proposalProjectionStatus: "confirmed",
      warnings: ["projection delivery failed"],
      memoryPersistence: {
        canonicalCommitted: true,
        projectionState: "degraded",
        reasonCode: "projection_delivery_failed",
      },
    });

    renderMailboxPage();
    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(
      await screen.findByText(
        "Memory 已写入 canonical store，但派生视图仍为 degraded；Mailbox 保持等待状态。"
      )
    ).toBeInTheDocument();
  });

  it("shows backend artifact receipt truth after a reviewed file is materialized", async () => {
    mockMutableProposals([artifactProposal], false, {
      success: true,
      effectStatus: "confirmed",
      proposalProjectionStatus: "confirmed",
      warnings: [],
      artifactMaterialization: {
        artifactId: "artifact:proposal-artifact-1",
        proposalId: "proposal-artifact-1",
        targetReference: "/tmp/openlife-test/roadshow-summary.md",
        targetReferenceDigest: "sha256:target",
        contentDigest: "sha256:content",
        observedContentDigest: "sha256:content",
        byteSize: 10,
        mediaType: "text/markdown; charset=utf-8",
        status: "confirmed",
      },
    });

    renderMailboxPage();
    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(
      await screen.findByText(
        "文件已确认保存：/tmp/openlife-test/roadshow-summary.md（10 bytes，sha256:content）。"
      )
    ).toBeInTheDocument();
  });

  it("keeps artifact completion unknown when the backend omits its receipt", async () => {
    mockMutableProposals([artifactProposal]);

    renderMailboxPage();
    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(
      await screen.findByText("文件审批已处理，但后端未提供落盘 Receipt；文件完成状态保持未知。")
    ).toBeInTheDocument();
  });

  it("notifies the shell to refresh diagnostics after accepting a proposal", async () => {
    const listener = vi.fn();
    window.addEventListener("openlife:diagnostics-refresh", listener);

    renderMailboxPage();

    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    await waitFor(() => {
      expect(listener).toHaveBeenCalledTimes(1);
    });

    window.removeEventListener("openlife:diagnostics-refresh", listener);
  });

  it("keeps durable accepted proposals blocked from task resume while materialization is unknown", async () => {
    const taskId = "mainchat-task-resume-1";
    mockMutableProposals([
      { ...lowRiskProposal, source: "chat_conversation", sourceDetail: taskId },
    ]);

    renderMailboxPage([
      {
        pathname: "/mailbox",
        state: { mainChatTaskSessionId: taskId, returnTo: "/companion" },
      },
    ]);

    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(
      await screen.findByText(/Materialization evidence is unknown; cannot request task resume yet/)
    ).toBeInTheDocument();
    expect(screen.queryByText("Main Chat task resume request available")).not.toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "resume_main_chat_agent_task")
    ).toBe(false);
  });

  it("resumes a Main Chat task after accepting a backend-declared no-materialization proposal", async () => {
    const taskId = "mainchat-task-resume-tool-1";
    mockMutableProposals([
      {
        ...unsupportedProposal,
        id: "proposal-tool-resume-1",
        proposalType: "tool_permission",
        source: "chat_conversation",
        sourceDetail: taskId,
        affectedPath: "tools.web.search",
        after: { toolName: "web.search", permission: "read" },
        status: "pending",
      },
    ]);

    renderMailboxPage([
      {
        pathname: "/mailbox",
        state: { mainChatTaskSessionId: taskId, returnTo: "/companion" },
      },
    ]);

    fireEvent.click(await screen.findByRole("button", { name: "同意" }));

    expect(await screen.findByText("Main Chat task resume request available")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Request resume" }));

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

    const initialRow = await screen.findByRole("button", { name: /新增目标/ });
    await waitFor(() => {
      expect(initialRow).toHaveAttribute("aria-pressed", "true");
    });
    const pluginRow = screen.getByRole("button", { name: /确认外部能力/ });
    fireEvent.click(pluginRow);
    await waitFor(() => {
      expect(pluginRow).toHaveAttribute("aria-pressed", "true");
    });

    const unsupportedAccept = await screen.findByRole("button", { name: "同意" });
    expect(unsupportedAccept).toBeDisabled();
    fireEvent.click(unsupportedAccept);

    await waitFor(() => {
      expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")).toBe(false);
    });
  });

  it("allows edited proposals to be accepted and moves them to accepted status", async () => {
    mockMutableProposals([editedMemoryProposal]);

    renderMailboxRoutes(["/mailbox?proposal=proposal-edited-memory-1"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：已修改");
    });
    const acceptButton = screen.getByRole("button", { name: "同意" });
    expect(acceptButton).toBeEnabled();
    fireEvent.click(acceptButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-edited-memory-1",
          proposal_id: "proposal-edited-memory-1",
        })
      );
    });
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：已同意");
    });
  });

  it("allows edited proposals to be rejected and moves them to handled status", async () => {
    mockMutableProposals([editedMemoryProposal]);

    renderMailboxRoutes(["/mailbox?proposal=proposal-edited-memory-1"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：已修改");
    });
    const rejectButton = screen.getByRole("button", { name: "不同意" });
    expect(rejectButton).toBeEnabled();
    fireEvent.click(rejectButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "reject_proposal",
        expect.objectContaining({
          proposalId: "proposal-edited-memory-1",
          proposal_id: "proposal-edited-memory-1",
        })
      );
    });
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：不同意");
    });
  });

  it("keeps unsupported edited proposals disabled for accept", async () => {
    mockProposals([{ ...unsupportedProposal, status: "edited" }]);

    renderMailboxRoutes(["/mailbox?proposal=proposal-plugin-1"]);

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：已修改");
    });
    const acceptButton = screen.getByRole("button", { name: "同意" });
    expect(acceptButton).toBeDisabled();
    fireEvent.click(acceptButton);

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

    fireEvent.click((await screen.findAllByText("新增目标"))[0].closest("button")!);
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

  it("keeps Safe Mode protection on edited proposal accept and edit while leaving reject visible", async () => {
    mockProposals([editedMemoryProposal], true);

    renderMailboxRoutes(["/mailbox?proposal=proposal-edited-memory-1"]);

    expect(await screen.findByText("系统处于 Safe Mode")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("mail-reader")).toHaveTextContent("状态：已修改");
    });
    expect(screen.getByRole("button", { name: "同意" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "改一下" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "不同意" })).toBeEnabled();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "accept_proposal")).toBe(false);
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "edit_proposal")).toBe(false);
  });
});
