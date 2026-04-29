import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import WorkspaceOverview from "./WorkspaceOverview";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("WorkspaceOverview contract", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-1",
            taskId: "task-1",
            status: "completed",
            kind: "conversation",
            userInput: "hello",
            outputPreview: "world",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_proposals") {
        return Promise.resolve([
          {
            id: "proposal-1",
            status: "pending",
            proposalType: "memory_write",
            source: "memory_governance",
            affectedPath: "memory.candidates",
            after: { content: "prefers concise replies" },
            reason: "candidate",
            confidence: 0.7,
            riskLevel: "medium",
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("loads runs through the formal listAgentRuns wrapper and reads camelCase dates", async () => {
    render(
      <MemoryRouter>
        <WorkspaceOverview />
      </MemoryRouter>
    );

    expect(await screen.findByText("今日 Agent Run")).toBeInTheDocument();
    expect(screen.getAllByText("1").length).toBeGreaterThan(0);
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("list_agent_runs", {
      limit: 100,
      offset: 0,
    });
  });
});
