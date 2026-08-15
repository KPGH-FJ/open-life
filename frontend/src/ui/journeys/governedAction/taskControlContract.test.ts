import { describe, expect, it } from "vitest";
import type { TaskControl, TaskViewModelItem } from "@/tauri";
import { initialTaskControlDispatchState, taskControlDispatchReducer } from "./taskControlContract";

function control(overrides: Partial<TaskControl> = {}): TaskControl {
  return {
    id: "task-1:retry",
    label: "重试失败步骤",
    kind: "retry",
    effect: "task_retry_request",
    enabled: true,
    requiresConfirmation: false,
    targetTaskId: "task-1",
    targetActionId: "action-3",
    completionProofAfterDispatch: false,
    ...overrides,
  };
}

function task(overrides: Partial<TaskViewModelItem> = {}): TaskViewModelItem {
  return {
    canonicalTaskId: "task-1",
    relatedRunIds: [],
    title: "整理访谈",
    lifecycleStatus: "running",
    terminalDeliveryStatus: "not_terminal",
    finalDeliveryEvidencePresent: false,
    items: [],
    artifacts: [],
    pendingBlockers: [],
    pendingReviewItemRefs: [],
    allowedControls: [],
    nextRecommendedControl: "open_trace",
    evidenceRefs: [],
    ...overrides,
  };
}

describe("task control dispatch contract", () => {
  it("blocks retry without an exact action target", () => {
    const state = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: control({ targetActionId: undefined }),
      expectedTaskId: "task-1",
    });
    expect(state).toMatchObject({
      phase: "blocked",
      reason: "task_retry_target_action_missing",
    });
  });

  it("requires confirmation for cancellation even if the backend omitted it", () => {
    const state = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: control({
        id: "task-1:cancel",
        label: "取消任务",
        kind: "cancel",
        effect: "task_cancel_request",
        targetActionId: undefined,
        requiresConfirmation: false,
      }),
      expectedTaskId: "task-1",
    });
    expect(state).toMatchObject({ phase: "blocked", reason: "task_cancel_requires_confirmation" });
  });

  it("does not resolve a command until the exact refreshed task confirms it", () => {
    const retry = control();
    const dispatching = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: retry,
      expectedTaskId: "task-1",
    });
    const refreshing = taskControlDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const unresolved = taskControlDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      task: task({ lifecycleStatus: "failed" }),
    });
    expect(unresolved.phase).toBe("awaiting_projection");

    const resolved = taskControlDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      task: task({ lifecycleStatus: "running" }),
    });
    expect(resolved).toMatchObject({
      phase: "resolved",
      refreshedTask: { canonicalTaskId: "task-1", lifecycleStatus: "running" },
    });
  });

  it("blocks controls that claim completion from dispatch", () => {
    const state = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: control({ completionProofAfterDispatch: true }),
      expectedTaskId: "task-1",
    });
    expect(state).toMatchObject({
      phase: "blocked",
      reason: "task_control_claims_completion_after_dispatch",
    });
  });
});
