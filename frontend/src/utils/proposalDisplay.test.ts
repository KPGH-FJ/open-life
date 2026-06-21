import { describe, expect, it } from "vitest";
import type { AgentProposal } from "../tauri";
import { buildProposalDisplayModel } from "./proposalDisplay";

function proposal(overrides: Partial<AgentProposal> = {}): AgentProposal {
  return {
    id: "proposal-test-1",
    runId: "run-test-1",
    proposalType: "goal_update",
    source: "builder_review",
    sourceDetail: "builder-session",
    affectedPath: "goals.short_term[0]",
    before: { name: "旧目标" },
    after: { name: "新目标" },
    reason: "用户确认目标需要更新。",
    confidence: 0.86,
    riskLevel: "low",
    status: "pending",
    whyOpenLifeThinksThis: "用户在 Builder 中确认了这个目标。",
    evidenceSummaries: [],
    behaviorChecks: [],
    createdAt: "2026-06-01T10:00:00.000Z",
    ...overrides,
  };
}

describe("proposalDisplay", () => {
  it("creates a readable metadata-safe diff for normal proposal values", () => {
    const model = buildProposalDisplayModel(proposal());

    expect(model.title).toBe("更新目标");
    expect(model.diffRows).toEqual([
      {
        field: "name",
        before: "「旧目标」",
        after: "「新目标」",
        redacted: false,
      },
    ]);
  });

  it("redacts sensitive-looking keys and values from the main diff", () => {
    const model = buildProposalDisplayModel(
      proposal({
        affectedPath: "memory.private_payload",
        before: { content_preview: "raw-sensitive-payload-should-not-render" },
        after: {
          content_preview: "raw-sensitive-payload-should-not-render",
          api_key: "sk-secret-should-not-render",
        },
      })
    );
    const serializedRows = JSON.stringify(model.diffRows);

    expect(serializedRows).not.toContain("raw-sensitive-payload-should-not-render");
    expect(serializedRows).not.toContain("sk-secret-should-not-render");
    expect(model.diffRows.some(row => row.redacted)).toBe(true);
  });

  it("keeps external write path and hash in technical rows instead of the main diff", () => {
    const model = buildProposalDisplayModel(
      proposal({
        proposalType: "external_write_action",
        affectedPath: "external.write",
        before: null,
        after: {
          path: "/tmp/openlife-test/export.json",
          operation: "write_file",
          size_bytes: 120,
          content_hash: "sha256:external-write-digest",
        },
      })
    );
    const serializedDiffRows = JSON.stringify(model.diffRows);
    const serializedTechnicalRows = JSON.stringify(model.technicalRows);

    expect(serializedDiffRows).not.toContain("/tmp/openlife-test/export.json");
    expect(serializedDiffRows).not.toContain("sha256:external-write-digest");
    expect(serializedTechnicalRows).toContain("/tmp/openlife-test/export.json");
    expect(serializedTechnicalRows).toContain("sha256:external-write-digest");
  });
});
