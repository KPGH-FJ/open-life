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

  it("previews array proposal values with the actual reviewable items", () => {
    const model = buildProposalDisplayModel(
      proposal({
        proposalType: "capability_update",
        affectedPath: "capabilities.skills",
        before: [],
        after: [
          {
            name: "analyze messy problems",
            proficiency: 5,
            description: "I can analyze messy problems",
          },
          {
            name: "write clearly",
            proficiency: 5,
            description: "I can write clearly",
          },
        ],
      })
    );

    expect(model.afterSummary).toContain("analyze messy problems");
    expect(model.afterSummary).toContain("write clearly");
    expect(model.afterSummary).not.toBe("数组 2 项");
    expect(model.diffRows[0].after).toContain("analyze messy problems");
    expect(model.diffRows[0].after).not.toBe("数组 2 项");
  });

  it("normalizes communication style aliases into a path-specific Review display", () => {
    const model = buildProposalDisplayModel(
      proposal({
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
        whyOpenLifeThinksThis: "用户在复盘中明确接受了更直接的沟通偏好。",
      })
    );

    expect(model.title).toBe("更新沟通偏好");
    expect(model.domain).toBe("沟通偏好");
    expect(model.diffRows).toEqual([
      {
        field: "沟通偏好",
        before: "「建议太绕」",
        after: "「直接给结论，再解释原因」",
        redacted: false,
      },
    ]);
    expect(model.evidenceSummary).toBe("来源摘录：用户在复盘中明确接受了更直接的沟通偏好。");
    expect(model.technicalRows).toEqual(
      expect.arrayContaining([
        { label: "位置", value: "preferences.communication_style" },
        { label: "规范位置", value: "preferences.communication_style" },
        { label: "Proposal", value: "proposal-communication-1" },
        { label: "置信度", value: "91%" },
        { label: "风险", value: "low" },
        {
          label: "来源摘录",
          value: "用户在复盘中明确接受了更直接的沟通偏好。",
        },
      ])
    );
    expect(model.technicalRows).toContainEqual({
      label: "Run",
      value: "run-communication-1",
      href: "#/runs/run-communication-1",
    });
  });

  it("shows a typed unavailable source reason for communication style proposals without source text", () => {
    const model = buildProposalDisplayModel(
      proposal({
        proposalType: "preference_update",
        affectedPath: "preferences.communication_style",
        before: "",
        after: "先给步骤",
        reason: "",
        whyOpenLifeThinksThis: "",
        evidenceSummaries: [],
      })
    );

    expect(model.evidenceSummary).toBe("source_excerpt_unavailable");
    expect(model.technicalRows).toContainEqual({
      label: "来源不可用",
      value: "source_excerpt_unavailable",
    });
  });

  it("labels memory governance proposals from typed candidate metadata", () => {
    const memoryModel = buildProposalDisplayModel(
      proposal({
        proposalType: "memory_write",
        source: "memory_governance",
        affectedPath: "memory.pending.chat_conversation",
        after: {
          content: "空腹喝咖啡会心慌",
          candidateKind: "semantic_user_fact",
          sourceEvidence: "帮我记下来：空腹喝咖啡会心慌",
          impactPreview: "确认后会影响 Memory 检索。",
        },
      })
    );

    expect(memoryModel.domain).toBe("用户事实/经验");
    expect(memoryModel.evidenceSummary).toBe("Source evidence：帮我记下来：空腹喝咖啡会心慌");
    expect(memoryModel.plainImpact).toBe("确认后会影响 Memory 检索。");
    expect(memoryModel.technicalRows).toEqual(
      expect.arrayContaining([
        { label: "Candidate kind", value: "semantic_user_fact" },
        { label: "Source evidence", value: "帮我记下来：空腹喝咖啡会心慌" },
        { label: "Impact preview", value: "确认后会影响 Memory 检索。" },
      ])
    );

    const ruleModel = buildProposalDisplayModel(
      proposal({
        proposalType: "life_model_update",
        source: "memory_governance",
        affectedPath: "lifemodel.pending.chat_conversation",
        after: {
          requestedChange: "以后早上安排工作前先确认我有没有吃东西",
          candidateKind: "procedural_rule",
          sourceEvidence: "以后早上安排工作前先确认我有没有吃东西",
          impactPreview: "确认后会影响 LifeModel 规划和未来建议。",
        },
        riskLevel: "high",
      })
    );

    expect(ruleModel.domain).toBe("未来行为规则/偏好");
    expect(ruleModel.plainImpact).toBe("确认后会影响 LifeModel 规划和未来建议。");
  });
});
