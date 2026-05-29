import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import ProposalReviewPage from "./ProposalReviewPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import type { AgentProposal } from "../tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

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
    render(<ProposalReviewPage />);

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
    render(<ProposalReviewPage />);

    expect(await screen.findByText("why OpenLife thinks this")).toBeInTheDocument();
    expect(screen.getByText("User approved this name during builder review.")).toBeInTheDocument();
    expect(screen.getByText("Builder confirmation supports the candidate.")).toBeInTheDocument();
    expect(screen.getByText("behavior check")).toBeInTheDocument();
    expect(screen.getByText("External writes stay reviewable")).toBeInTheDocument();
    expect(screen.queryByText("raw-sensitive-payload")).not.toBeInTheDocument();
  });
});
