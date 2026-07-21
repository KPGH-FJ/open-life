import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { BuilderTurnResponse } from "@/tauri";
import type { LifeModelBuilderDataSource } from "./lifeModelBuilderDataSource";
import { useLifeModelBuilder } from "./useLifeModelBuilder";

const askingTurn: BuilderTurnResponse = {
  prompt: "接下来三个月，你最希望推进什么？",
  finished: false,
  progress: {
    progress: 25,
    current_step_label: "当前目标",
    step_index: 0,
    total_steps: 2,
  },
  review: null,
  waiting_for_review: false,
  durable_lifemodel_write: false,
};

const reviewTurn: BuilderTurnResponse = {
  prompt: "请逐项核对这些候选理解。",
  finished: true,
  progress: {
    progress: 100,
    current_step_label: "候选核对",
    step_index: 1,
    total_steps: 2,
  },
  waiting_for_review: true,
  durable_lifemodel_write: false,
  review: {
    session_id: "builder-session",
    finished: true,
    signals: [
      {
        id: "signal-goal",
        source_step: 1,
        source_question_id: "current-goal",
        dimension: "Goals",
        affected_path: "goals.short_term",
        proposed_value: "完成客户研究",
        confidence: 0.82,
        reason: "来自当前目标回答。",
        risk_level: "medium",
        user_status: "Pending",
      },
      {
        id: "signal-focus",
        source_step: 1,
        source_question_id: "current-goal",
        dimension: "State",
        affected_path: "state.current_focus",
        proposed_value: { priority: "research" },
        confidence: 0.76,
        reason: "来自当前工作顺序。",
        risk_level: "low",
        user_status: "Pending",
      },
    ],
    summary: {
      identity_summary: "",
      goals_summary: "当前重点是完成客户研究。",
      capabilities_summary: "",
      state_summary: "当前处于研究阶段。",
      assumptions: [],
      unresolved_questions: [],
      recommended_next_steps: ["逐项核对候选"],
    },
  },
};

function source(overrides: Partial<LifeModelBuilderDataSource> = {}) {
  return {
    listUnfinished: vi.fn().mockResolvedValue([]),
    startQuick: vi.fn().mockResolvedValue(askingTurn),
    resume: vi.fn().mockResolvedValue(askingTurn),
    answer: vi.fn().mockResolvedValue(reviewTurn),
    createProposals: vi.fn().mockResolvedValue({
      success: true,
      created_count: 1,
      reused_count: 0,
      updated_count: 0,
      rejected_count: 1,
      proposal_ids: ["proposal-goal"],
      run_id: "builder-run",
      warnings: [],
    }),
    ...overrides,
  } satisfies LifeModelBuilderDataSource;
}

async function reachReview(dataSource: LifeModelBuilderDataSource, announce = vi.fn()) {
  const hook = renderHook(() => useLifeModelBuilder(dataSource, announce));
  await act(async () => hook.result.current.start());
  act(() => hook.result.current.setAnswerDraft("先完成三次访谈分析"));
  await act(async () => hook.result.current.answer());
  return { ...hook, announce };
}

describe("LifeModel Builder journey", () => {
  it("starts an exact quick session and waits for a user answer", async () => {
    const dataSource = source();
    const announce = vi.fn();
    const { result } = renderHook(() => useLifeModelBuilder(dataSource, announce));

    await act(async () => result.current.start());

    expect(dataSource.startQuick).toHaveBeenCalledWith(expect.any(String));
    expect(result.current.phase).toBe("asking");
    expect(result.current.prompt).toBe(askingTurn.prompt);
    expect(result.current.answerAction().enabled).toBe(false);
  });

  it("keeps every generated candidate undecided until the user chooses", async () => {
    const dataSource = source();
    const { result } = await reachReview(dataSource);

    expect(dataSource.answer).toHaveBeenCalledWith(expect.any(String), "先完成三次访谈分析");
    expect(result.current.phase).toBe("reviewing");
    expect(result.current.candidates.map(candidate => candidate.decision)).toEqual([
      "undecided",
      "undecided",
    ]);
    expect(result.current.proposalAction()).toMatchObject({
      enabled: false,
      disabledReason: "还有 2 个候选未决定。",
    });
  });

  it("creates review proposals only after explicit decisions and does not claim an applied state", async () => {
    const dataSource = source();
    const { result, announce } = await reachReview(dataSource);

    act(() => {
      result.current.setCandidateDecision("signal-goal", "accepted");
      result.current.setCandidateDecision("signal-focus", "rejected");
    });
    await act(async () => result.current.createProposals());

    expect(dataSource.createProposals).toHaveBeenCalledWith(expect.any(String), [
      { id: "signal-goal", status: "accepted" },
      { id: "signal-focus", status: "rejected" },
    ]);
    expect(result.current.phase).toBe("created");
    expect(result.current.receipt?.proposal_ids).toEqual(["proposal-goal"]);
    expect(announce).toHaveBeenLastCalledWith(
      "审核建议已创建；它们尚未批准，也尚未写入 LifeModel。"
    );
  });

  it("fails closed on malformed edited values without sending a proposal command", async () => {
    const dataSource = source();
    const { result } = await reachReview(dataSource);

    act(() => {
      result.current.setCandidateDecision("signal-goal", "accepted");
      result.current.setCandidateDecision("signal-focus", "edited");
      result.current.setCandidateEditedValue("signal-focus", "not-json");
    });
    await act(async () => result.current.createProposals());

    expect(dataSource.createProposals).not.toHaveBeenCalled();
    expect(result.current.phase).toBe("reviewing");
    expect(result.current.error).toBe("builder_edited_value_invalid_json");
  });

  it("keeps retained candidates available when proposal creation fails", async () => {
    const dataSource = source({
      createProposals: vi.fn().mockRejectedValue(new Error("builder_proposal_store_failed")),
    });
    const { result } = await reachReview(dataSource);

    act(() => {
      result.current.setCandidateDecision("signal-goal", "accepted");
      result.current.setCandidateDecision("signal-focus", "rejected");
    });
    await act(async () => result.current.createProposals());

    expect(result.current.phase).toBe("reviewing");
    expect(result.current.candidates).toHaveLength(2);
    expect(result.current.error).toBe("builder_proposal_store_failed");
  });
});
