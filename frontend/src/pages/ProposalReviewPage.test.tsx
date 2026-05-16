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

  it("shows typed replay failure reason when continue fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-fail",
            runId: "run-mcp-fail",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.test_tool",
            before: null,
            after: {
              permission_action: "grant",
              tool_name: "test_tool",
              source: "builtin",
              risk_level: "low",
              action_type: "read",
              policy: "allow_until_revoked",
              reason: "needs_confirmation:network_policy",
            },
            reason: "[NetworkPolicy ask] 需要确认",
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
            patchId: "patch-replay-fail",
            success: true,
            path: "/tool_permission/test_tool",
            operation: "replace",
          },
          can_continue: true,
          continue_run_id: "run-mcp-fail",
          continue_action_id: "action-fail",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-fail",
          status: "blocked",
          error: "IGNORE THIS TEXT: replay_spec_missing in error string",
          output: {
            block_reason: "replay_spec_missing",
            agent_spec_id: "main.default",
          },
          toolScope: {
            toolId: "test_tool",
            toolName: "test_tool",
            source: "builtin",
            riskLevel: "low",
            capabilities: [],
            actionType: "read",
          },
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({ proposalId: "proposal-tp-fail" })
      );
    });

    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();
    fireEvent.click(screen.getByText("继续执行已批准的动作"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "replay_agent_action",
        expect.objectContaining({ runId: "run-mcp-fail" })
      );
    });

    // Replay blocked — should show typed block reason (not error text)
    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    expect(screen.getByText(/状态：blocked/)).toBeInTheDocument();
    expect(screen.getByText(/阻断原因: 缺少重放规格/)).toBeInTheDocument();
    expect(screen.getByText(/AgentSpec: main\.default/)).toBeInTheDocument();
    // error text should NOT appear (it's only for auxiliary use, not typed reason)
    expect(screen.queryByText(/IGNORE THIS TEXT/)).toBeNull();
    // tool_name "test_tool" appears in the notice
    const toolNameElements = screen.getAllByText(/test_tool/);
    expect(toolNameElements.length).toBeGreaterThan(0);
  });

  // ── Batch 5 Fix: NetworkPolicy typed check tests ──────────────────

  it("does not infer network policy ask from reason text", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-nopoly",
            runId: "run-1",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.some_tool",
            before: null,
            after: {
              permission_action: "grant",
              tool_name: "some_tool",
              reason: "needs_confirmation:network_policy",
              // NO network_policy_ask, NO proposal_reason typed fields
            },
            reason: "User confirmed tool permission",
            confidence: 0.9,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-1", success: true, path: "/test", operation: "replace" },
          canContinue: false,
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    // Notice should appear but NOT contain network policy confirmation
    // because no typed fields exist (only reason text)
    await waitFor(() => {
      const notice = document.querySelector(".rounded-2xl.border.border-emerald-100");
      if (notice) {
        expect(notice.textContent).not.toContain("网络策略确认");
      }
    });
  });

  it("shows network policy ask from typed field", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-typed-ask",
            runId: "run-1",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.net_tool",
            before: null,
            after: {
              permission_action: "grant",
              tool_name: "net_tool",
              network_policy_ask: true,
              // reason text is present as noise — should not be used
              reason: "needs_confirmation:network_policy",
            },
            reason: "User confirmed network access",
            confidence: 0.9,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-2", success: true, path: "/test", operation: "replace" },
          canContinue: false,
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    await waitFor(() => {
      expect(screen.getByText(/网络策略确认/)).toBeInTheDocument();
    });
  });

  // ── Batch 5 Fix: Typed replay outcome tests ────────────────────────

  it("shows typed replay block reason when continue returns blocked", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-typed-block",
            runId: "run-typed-block",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.blocked_tool",
            before: null,
            after: {
              permission_action: "grant",
              tool_name: "blocked_tool",
              source: "builtin",
            },
            reason: "Accept to retry blocked action",
            confidence: 0.7,
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
            patchId: "patch-typed",
            success: true,
            path: "/test",
            operation: "replace",
          },
          can_continue: true,
          continue_run_id: "run-typed-block",
          continue_action_id: "action-typed-block",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-typed-block",
          status: "blocked",
          error: "IGNORE: this is noise text with replay_spec_missing keywords",
          output: {
            block_reason: "replay_spec_missing",
            agent_spec_id: "main.default",
          },
          toolScope: {
            toolId: "blocked_tool",
            toolName: "blocked_tool",
            source: "builtin",
            riskLevel: "medium",
            capabilities: ["network"],
            actionType: "read",
          },
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();
    fireEvent.click(screen.getByText("继续执行已批准的动作"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("replay_agent_action", expect.anything());
    });

    // Must show typed reason, not error text
    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    expect(screen.getByText(/阻断原因: 缺少重放规格/)).toBeInTheDocument();
    // Error noise text must NOT appear
    expect(screen.queryByText(/IGNORE:/)).toBeNull();
  });

  it("does not derive typed replay reason from error text", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-no-typed",
            runId: "run-no-typed",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.no_typed",
            before: null,
            after: { permission_action: "grant", tool_name: "no_typed" },
            reason: "Accept",
            confidence: 0.7,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-no", success: true, path: "/test", operation: "replace" },
          can_continue: true,
          continue_run_id: "run-no-typed",
          continue_action_id: "action-no-typed",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-no-typed",
          status: "blocked",
          error: "replay_spec_missing: fallback error message",
          // NO output with typed fields
          toolScope: {
            toolId: "no_typed",
            toolName: "no_typed",
            source: "builtin",
            riskLevel: "low",
            capabilities: [],
            actionType: "read",
          },
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();
    fireEvent.click(screen.getByText("继续执行已批准的动作"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("replay_agent_action", expect.anything());
    });

    // Must show "typed reason unavailable" — NOT derive from error text
    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    expect(screen.getByText(/typed reason unavailable/)).toBeInTheDocument();
    // Must NOT contain the typed reason label (since no typed field exists)
    expect(screen.queryByText(/缺少重放规格/)).toBeNull();
  });

  it("shows typed replay proposal reason for needs_confirmation", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-tp-needsconf",
            runId: "run-needsconf",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.needs_tool",
            before: null,
            after: { permission_action: "grant", tool_name: "needs_tool" },
            reason: "Accept",
            confidence: 0.7,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-nc", success: true, path: "/test", operation: "replace" },
          can_continue: true,
          continue_run_id: "run-needsconf",
          continue_action_id: "action-needsconf",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-needsconf",
          status: "needs_confirmation",
          output: {
            proposal_reason: "tool_permission_ask",
            proposal_id: "proposal-tp-123",
          },
          toolScope: {
            toolId: "needs_tool",
            toolName: "needs_tool",
            source: "builtin",
            riskLevel: "medium",
            capabilities: ["network"],
            actionType: "read",
          },
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();
    fireEvent.click(screen.getByText("继续执行已批准的动作"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("replay_agent_action", expect.anything());
    });

    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    expect(screen.getByText(/状态：needs_confirmation/)).toBeInTheDocument();
    expect(screen.getByText(/需确认: 工具权限询问/)).toBeInTheDocument();
    expect(screen.getByText(/Proposal: proposal-tp-123/)).toBeInTheDocument();
  });

  // ── Hardened: invalid typed reasons are never displayed ──

  it("replay with error text containing replay_spec_missing but output.block_reason invalid → typed reason unavailable", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-invalid-reason",
            runId: "run-invalid-reason",
            proposalType: "tool_permission",
            source: "manual",
            sourceDetail: "",
            affectedPath: "tool_permission.builtin.test_tool",
            before: null,
            after: { permission_action: "grant", tool_name: "test_tool" },
            reason: "Accept",
            confidence: 0.7,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ] as any);
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({
          success: true,
          patchResult: { patchId: "patch-inv", success: true, path: "/test", operation: "replace" },
          can_continue: true,
          continue_run_id: "run-invalid-reason",
          continue_action_id: "action-invalid-reason",
        } as any);
      }
      if (cmd === "replay_agent_action") {
        return Promise.resolve({
          id: "action-invalid-reason",
          status: "blocked",
          error: "replay_spec_missing: fallback error message",
          output: {
            block_reason: "unknown_random_string",
          },
          toolScope: {
            toolId: "test_tool",
            toolName: "test_tool",
            source: "builtin",
            riskLevel: "low",
            capabilities: [],
            actionType: "read",
          },
        } as any);
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({ overall: "ok", items: [] } as any);
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    const acceptBtns = screen.getAllByText("应用");
    fireEvent.click(acceptBtns[0]);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("accept_proposal", expect.anything());
    });

    expect(await screen.findByText("继续执行已批准的动作")).toBeInTheDocument();
    fireEvent.click(screen.getByText("继续执行已批准的动作"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("replay_agent_action", expect.anything());
    });

    expect(await screen.findByText(/已重放动作/)).toBeInTheDocument();
    // invalid block_reason → "typed reason unavailable"
    expect(screen.getByText(/typed reason unavailable/)).toBeInTheDocument();
    // The label "缺少重放规格" must NOT appear since block_reason was invalid
    expect(screen.queryByText(/缺少重放规格/)).toBeNull();
    // The raw "unknown_random_string" must NOT appear
    expect(screen.queryByText(/unknown_random_string/)).toBeNull();
  });
});
