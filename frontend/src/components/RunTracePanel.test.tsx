import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import RunTracePanel from "./RunTracePanel";
import type { AgentRun } from "../tauri";

const baseRun: AgentRun = {
  id: "run-1",
  taskId: "task-1",
  status: "completed",
  kind: "planning",
  generatedProposals: [],
  actions: [],
  observations: [],
  startedAt: new Date().toISOString(),
};

describe("RunTracePanel", () => {
  it("renders selected collaboration rules, styles, and behavior checks without raw HS terms", () => {
    render(
      <RunTracePanel
        run={{
          ...baseRun,
          hsSelectionAudit: {
            selectedPolicyIds: ["policy.external_writes.proposal_first"],
            selectedHeuristicIds: ["heuristic.low_energy_planning"],
            estimatedTokens: 42,
            tokenBudget: 256,
          },
          behaviorChecks: [
            {
              id: "regression.external_write_proposal_first",
              label: "External writes stay reviewable",
              passed: true,
              summary: "Direct writes become proposals.",
            },
          ],
          outputPreview: "raw-sensitive-payload-should-not-drive-trace",
        }}
      />
    );

    expect(screen.getByText("AI collaboration rules used")).toBeInTheDocument();
    expect(screen.getByText("Review before external writes")).toBeInTheDocument();
    expect(screen.getByText("AI collaboration style")).toBeInTheDocument();
    expect(screen.getByText("Low-energy planning style")).toBeInTheDocument();
    expect(screen.getByText("behavior check")).toBeInTheDocument();
    expect(screen.getByText("External writes stay reviewable")).toBeInTheDocument();
    expect(screen.queryByText(/heuristic/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText("raw-sensitive-payload-should-not-drive-trace")
    ).not.toBeInTheDocument();
  });

  it("renders an empty state when no HS assets affect a run", () => {
    render(<RunTracePanel run={baseRun} />);

    expect(screen.getByText("No collaboration rules affected this run.")).toBeInTheDocument();
    expect(screen.queryByText("AI collaboration style")).not.toBeInTheDocument();
  });
});
