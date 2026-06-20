import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import ProposalReviewPage from "./ProposalReviewPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import type { AgentProposal } from "../tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function renderProposalReviewPage() {
  return render(
    <BrowserRouter>
      <ProposalReviewPage />
    </BrowserRouter>
  );
}

describe("ProposalReviewPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        const proposal: AgentProposal = {
          id: "proposal-1",
          runId: "run-1",
          proposalType: "goal_update",
          source: "chat_conversation",
          sourceDetail: "session-123",
          affectedPath: "identity.name",
          before: "",
          after: "Fujing",
          reason: "用户确认的新称呼",
          confidence: 0.9,
          riskLevel: "low",
          status: "pending",
          whyOpenLifeThinksThis: "User approved this name during builder review.",
          evidenceSummaries: [
            {
              id: "ev-1",
              summary: "Builder confirmation supports the candidate.",
              sourceAssetIds: ["run-1"],
              contentDigest: "digest-abc123",
            },
          ],
          behaviorChecks: [
            {
              id: "regression.external_write_proposal_first",
              label: "External writes stay reviewable",
              passed: true,
              summary: "Direct writes remain proposals.",
            },
          ],
          createdAt: new Date().toISOString(),
          expiresAt: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString(),
        };
        return Promise.resolve([proposal]);
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
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders pending proposals and accepts one", async () => {
    renderProposalReviewPage();

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(await screen.findByText("identity.name")).toBeInTheDocument();
    expect(await screen.findByText("Fujing")).toBeInTheDocument();

    fireEvent.click(screen.getByText("应用"));

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

  it("renders concise evidence summaries without raw sensitive payloads", async () => {
    renderProposalReviewPage();

    expect(await screen.findByText("why OpenLife thinks this")).toBeInTheDocument();
    expect(screen.getByText("User approved this name during builder review.")).toBeInTheDocument();
    expect(screen.getByText("Builder confirmation supports the candidate.")).toBeInTheDocument();
    expect(screen.getByText("behavior check")).toBeInTheDocument();
    expect(screen.getByText("External writes stay reviewable")).toBeInTheDocument();
    expect(screen.queryByText("raw-sensitive-payload")).not.toBeInTheDocument();
  });

  it("shows Skill Runtime as a proposal source with run linkage", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        const proposal: AgentProposal = {
          id: "proposal-skill-1",
          runId: "run-skill-123456",
          proposalType: "memory_write",
          source: "skill_runtime",
          sourceDetail: "skill_id=weekly_review;candidate_digest=sha256:abc",
          affectedPath: "memory.candidates",
          after: { digest: "sha256:memory" },
          reason: "Skill generated a reviewable memory candidate.",
          confidence: 0.7,
          riskLevel: "medium",
          status: "pending",
          createdAt: new Date().toISOString(),
          expiresAt: new Date(Date.now() + 14 * 24 * 60 * 60 * 1000).toISOString(),
        };
        return Promise.resolve([proposal]);
      }
      return mockInvoke(cmd, args);
    });

    renderProposalReviewPage();

    expect(await screen.findByText("memory.candidates")).toBeInTheDocument();
    expect(screen.getByText("Skill")).toBeInTheDocument();
    expect(screen.getByTitle("查看来源 Run: run-skill-123456")).toBeInTheDocument();
    expect(screen.queryByText(/raw-sensitive-payload/)).not.toBeInTheDocument();
  });

  it("shows Stage 4 knowledge inventory and managed knowledge write controls", async () => {
    renderProposalReviewPage();

    expect(await screen.findByText("Knowledge assets")).toBeInTheDocument();
    expect(screen.getAllByText("USER.md").length).toBeGreaterThan(0);
    expect(screen.getAllByText("MEMORY.md").length).toBeGreaterThan(0);
    expect(screen.getByText("unselected_skill")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("Draft managed content"), {
      target: { value: "Managed profile summary" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Draft diff/i }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "create_managed_knowledge_write_draft",
        expect.objectContaining({
          targetPath: "USER.md",
          afterContent: "Managed profile summary",
        })
      );
    });
    expect(await screen.findByText(/Managed USER.md draft created/)).toBeInTheDocument();
    expect(screen.getByText(/\+mock/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Confirm write/i }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "confirm_managed_knowledge_write",
        expect.objectContaining({ proposalId: "proposal-managed-knowledge-1" })
      );
    });
  });

  it("uses draft-only edit for pending memory proposals", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        const proposal: AgentProposal = {
          id: "proposal-memory-draft",
          proposalType: "memory_write",
          source: "chat_conversation",
          sourceDetail: "task-session-1",
          affectedPath: "memory.preference.review_style",
          after: { content: "Prefer concise reviews." },
          reason: "User asked OpenLife to remember this.",
          confidence: 0.8,
          riskLevel: "low",
          status: "pending",
          createdAt: new Date().toISOString(),
        };
        return Promise.resolve([proposal]);
      }
      return mockInvoke(cmd, args);
    });

    renderProposalReviewPage();

    expect(await screen.findByText("memory.preference.review_style")).toBeInTheDocument();
    fireEvent.click(screen.getByText("编辑"));
    const textboxes = screen.getAllByRole("textbox");
    const editTextArea = textboxes[textboxes.length - 1];
    expect(editTextArea).toBeTruthy();
    fireEvent.change(editTextArea!, {
      target: { value: JSON.stringify({ content: "Prefer concise and rigorous reviews." }) },
    });
    fireEvent.click(screen.getByRole("button", { name: "保存草稿" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "draft_edit_memory_proposal",
        expect.objectContaining({
          proposalId: "proposal-memory-draft",
          newAfter: { content: "Prefer concise and rigorous reviews." },
        })
      );
    });
    expect(invoke).not.toHaveBeenCalledWith(
      "edit_proposal",
      expect.objectContaining({ proposalId: "proposal-memory-draft" })
    );
  });
});
