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
});
