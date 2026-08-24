import { describe, expect, it } from "vitest";
import type { TaskControl, TaskViewModelItem } from "@/tauri";
import {
  initialTaskControlDispatchState,
  taskControlDispatchReducer,
} from "@/features/work/taskControlContract";

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
    completionLimitations: [],
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
      reason: "task_retry_target_run_missing",
    });
  });

  it("requires an exact active run target for stop", () => {
    const state = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: control({
        id: "task-1:stop_run",
        label: "停止当前运行",
        kind: "stop_run",
        effect: "task_stop_run_request",
        targetActionId: undefined,
      }),
      expectedTaskId: "task-1",
    });
    expect(state).toMatchObject({ phase: "blocked", reason: "task_stop_run_target_run_missing" });
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
      task: task({ lifecycleStatus: "failed", relatedRunIds: ["action-3"] }),
    });
    expect(unresolved.phase).toBe("awaiting_projection");

    const resolved = taskControlDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      task: task({ lifecycleStatus: "running", relatedRunIds: ["action-3", "run-4"] }),
    });
    expect(resolved).toMatchObject({
      phase: "resolved",
      refreshedTask: { canonicalTaskId: "task-1", lifecycleStatus: "running" },
    });
  });

  it("resolves resume only after a different new run is projected", () => {
    const resume = control({
      id: "task-1:resume",
      label: "继续并创建新运行",
      kind: "resume",
      effect: "task_resume_request",
    });
    const dispatching = taskControlDispatchReducer(initialTaskControlDispatchState, {
      type: "request",
      control: resume,
      expectedTaskId: "task-1",
    });
    const refreshing = taskControlDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const unchanged = taskControlDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      task: task({ lifecycleStatus: "cancelled", relatedRunIds: ["action-3"] }),
    });
    expect(unchanged.phase).toBe("awaiting_projection");

    const resolved = taskControlDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      task: task({ lifecycleStatus: "running", relatedRunIds: ["action-3", "run-4"] }),
    });
    expect(resolved.phase).toBe("resolved");
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
