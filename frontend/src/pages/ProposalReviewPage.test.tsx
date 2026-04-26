import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
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
            proposalType: "life_model_update",
            affectedPath: "identity.name",
            before: "",
            after: "Fujing",
            reason: "用户确认的新称呼",
            confidence: 0.9,
            riskLevel: "low",
            status: "pending",
            source: "builder:test:sig_name",
            createdAt: new Date().toISOString(),
          },
        ] as any);
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
});
