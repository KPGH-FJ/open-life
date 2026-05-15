import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ProposalReviewPage from "./ProposalReviewPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("ProposalReviewPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-1",
            runId: "run-1",
            proposalType: "goal_update",
            source: "builder_review",
            sourceDetail: "session-123",
            affectedPath: "identity.name",
            before: "",
            after: "Fujing",
            reason: "用户确认的新称呼",
            confidence: 0.9,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString(),
          },
          {
            id: "proposal-2",
            runId: "run-2",
            proposalType: "memory_write",
            source: "memory_governance",
            affectedPath: "memory.tier1",
            before: null,
            after: { content: "test memory" },
            reason: "检测到重复模式",
            confidence: 0.7,
            riskLevel: "high",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
          {
            id: "proposal-3",
            runId: "run-3",
            proposalType: "external_write_action",
            source: "skill_runtime",
            affectedPath: "/tmp/test.txt",
            before: null,
            after: {
              path: "/tmp/test.txt",
              operation: "create",
              size_bytes: 100,
              content_hash: "abc123",
            },
            reason: "Skill 生成了输出",
            confidence: 0.85,
            riskLevel: "medium",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
          {
            id: "proposal-4",
            runId: "run-4",
            proposalType: "goal_update",
            source: "proactive_agent",
            affectedPath: "goals.short_term[0].priority",
            before: 1,
            after: 2,
            reason: "根据对话分析提升优先级",
            confidence: 0.6,
            riskLevel: "medium",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: {
            patchId: "patch-1",
            success: true,
            path: "/identity/name",
            operation: "replace",
          },
        } as any);
      }
      if (cmd === "reject_proposal") {
        return Promise.resolve(undefined);
      }
      if (cmd === "postpone_proposal") {
        return Promise.resolve(undefined);
      }
      if (cmd === "edit_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-2", success: true, path: "/test", operation: "replace" },
        } as any);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  function renderPage() {
    return render(
      <MemoryRouter>
        <ProposalReviewPage />
      </MemoryRouter>
    );
  }

  it("renders pending proposals and accepts one", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(await screen.findByText("identity.name")).toBeInTheDocument();

    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-1",
          proposal_id: "proposal-1",
        })
      );
    });
  });

  it("renders proposal evidence and source context", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(screen.getAllByText("变更摘要").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Evidence 上下文").length).toBeGreaterThan(0);
    expect(screen.getByText("Builder 构建")).toBeDefined();
  });

  it("shows high-risk proposal context", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const highRiskBadges = screen.getAllByText("高风险 — 需谨慎审查");
    expect(highRiskBadges.length).toBeGreaterThan(0);
  });

  it("rejects proposal works", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const rejectBtns = screen.getAllByText("拒绝");
    expect(rejectBtns.length).toBeGreaterThan(0);
    fireEvent.click(rejectBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "reject_proposal",
        expect.objectContaining({ proposal_id: expect.any(String) })
      );
    });
  });

  it("postpone proposal works", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const postponeBtns = screen.getAllByText("稍后");
    expect(postponeBtns.length).toBeGreaterThan(0);
    fireEvent.click(postponeBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "postpone_proposal",
        expect.objectContaining({ proposal_id: expect.any(String) })
      );
    });
  });

  it("expands before/after comparison", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const summaryBtns = screen.getAllByText("变更摘要");
    expect(summaryBtns.length).toBeGreaterThan(0);
    fireEvent.click(summaryBtns[0]);

    expect(await screen.findByText("变更前")).toBeDefined();
    expect(await screen.findByText("变更后")).toBeDefined();
  });

  it("shows source badges with correct labels", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(screen.getByText("Builder 构建")).toBeDefined();
    // source: memory_governance
    expect(screen.getByText("记忆治理")).toBeDefined();
    // source: skill_runtime
    expect(screen.getByText("Skill 运行")).toBeDefined();
    // source: proactive_agent
    expect(screen.getByText("主动建议")).toBeDefined();
  });

  it("shows run ID link for proposals with runId", async () => {
    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const runLinks = screen.getAllByText(/Run #/);
    expect(runLinks.length).toBeGreaterThan(0);
  });

  it("shows continue action after accepting replayable tool permission", async () => {
    // Override mock for this specific test: include a tool_permission proposal
    // with network_policy_ask, plus acceptProposal returning can_continue,
    // plus replayAgentAction returning success.
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-replay",
            runId: "run-mcp-replay",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.test_mcp_network_tool",
            before: null,
            after: {
              permission_action: "grant",
              tool_name: "test_mcp_network_tool",
              source: "builtin",
              risk_level: "low",
              action_type: "read",
              policy: "allow_until_revoked",
              network_policy_ask: true,
              auto_generated: true,
              reason: "needs_confirmation:network_policy",
              blocked_action: {
                action_type: "builtin_tool",
                target: "mcp.call_tool",
                source_run_id: "run-mcp-replay",
                step_index: 0,
              },
            },
            reason: "[NetworkPolicy ask] 需要网络访问确认",
            confidence: 0.7,
            riskLevel: "medium",
            status: "pending",
            createdAt: new Date().toISOString(),
            expiresAt: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: {
            patchId: "patch-replay",
            success: true,
            path: "/tool_permission/test_mcp_network_tool",
            operation: "replace",
          },
          can_continue: true,
          continue_run_id: "run-mcp-replay",
          continue_action_id: "action-replay",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-replay",
          status: "succeeded",
          output: "mock-ok",
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          overall: "ok",
          items: [],
        } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(screen.getByText("tool_permission.builtin.test_mcp_network_tool")).toBeInTheDocument();

    // Click accept button ("应用")
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-tp-replay",
          proposal_id: "proposal-tp-replay",
        })
      );
    });

    // After accept, the "continue" button should appear
    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();

    // Click the continue button
    const continueBtn = screen.getByText("继续执行已批准的动作");
    fireEvent.click(continueBtn);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "replay_agent_action",
        expect.objectContaining({
          runId: "run-mcp-replay",
          actionId: "action-replay",
        })
      );
    });

    // The replay status should be visible in the notice area
    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    expect(screen.getByText(/succeeded/)).toBeInTheDocument();
  });
});
