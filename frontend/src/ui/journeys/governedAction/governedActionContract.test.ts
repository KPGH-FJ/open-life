import { describe, expect, it } from "vitest";
import type { TaskControl, TaskViewModelItem } from "@/tauri";
import {
  hasRefreshedTaskCompletionProof,
  initialTaskResumeState,
  taskResumeReducer,
} from "./governedActionContract";

function resumeControl(overrides: Partial<TaskControl> = {}): TaskControl {
  return {
    id: "task-1:resume",
    label: "Resume",
    kind: "resume",
    effect: "task_resume_request",
    enabled: true,
    targetTaskId: "task-1",
    completionProofAfterDispatch: false,
    ...overrides,
  };
}

function task(
  lifecycleStatus: TaskViewModelItem["lifecycleStatus"],
  overrides: Partial<TaskViewModelItem> = {}
): TaskViewModelItem {
  return {
    canonicalTaskId: "task-1",
    taskSessionId: "task-1",
    relatedRunIds: [],
    title: "Prepare interview synthesis",
    strategy: "react",
    lifecycleStatus,
    terminalDeliveryStatus: "not_terminal",
    finalDeliveryEvidencePresent: false,
    pendingBlockers: [],
    pendingReviewItemRefs: [],
    allowedControls: [],
    nextRecommendedControl: "open_trace",
    evidenceRefs: [],
    ...overrides,
  };
}

function refreshingState(control: TaskControl = resumeControl()) {
  const dispatching = taskResumeReducer(initialTaskResumeState, {
    type: "request",
    control,
    expectedTaskId: "task-1",
  });
  return taskResumeReducer(dispatching, { type: "dispatch_succeeded" });
}

describe("governed task resume contract", () => {
  it("blocks disabled, mismatched, and completion-claiming controls", () => {
    const disabled = taskResumeReducer(initialTaskResumeState, {
      type: "request",
      control: resumeControl({ enabled: false, disabledReason: "Review is still pending." }),
      expectedTaskId: "task-1",
    });
    const mismatch = taskResumeReducer(initialTaskResumeState, {
      type: "request",
      control: resumeControl({ targetTaskId: "task-2" }),
      expectedTaskId: "task-1",
    });
    const completionClaim = taskResumeReducer(initialTaskResumeState, {
      type: "request",
      control: resumeControl({ completionProofAfterDispatch: true }),
      expectedTaskId: "task-1",
    });

    expect(disabled).toMatchObject({ phase: "blocked", reason: "Review is still pending." });
    expect(mismatch).toMatchObject({ phase: "blocked", reason: "task_control_target_mismatch" });
    expect(completionClaim).toMatchObject({
      phase: "blocked",
      reason: "task_control_claims_completion_after_dispatch",
    });
  });

  it("treats command return as refresh only", () => {
    const refreshing = refreshingState();
    expect(refreshing.phase).toBe("refreshing");
    expect(hasRefreshedTaskCompletionProof(refreshing)).toBe(false);
  });

  it("waits when the exact task is missing or still blocked", () => {
    const missing = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: null,
    });
    const waiting = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: task("waiting_permission"),
    });

    expect(missing).toMatchObject({
      phase: "awaiting_projection",
      reason: "refreshed_tasks_missing_target",
    });
    expect(waiting).toMatchObject({
      phase: "awaiting_projection",
      reason: "refreshed_task_does_not_confirm_resume_yet",
    });
  });

  it("requires exact task identity before accepting a refreshed state", () => {
    const mismatch = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: task("running", { taskSessionId: "task-2" }),
    });

    expect(mismatch).toMatchObject({
      phase: "failed",
      stage: "refresh",
      errorCode: "task_refresh_target_mismatch",
    });
  });

  it("resolves running without calling it completed", () => {
    const resolved = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: task("running"),
    });

    expect(resolved.phase).toBe("resolved");
    expect(hasRefreshedTaskCompletionProof(resolved)).toBe(false);
  });

  it("accepts completion only with refreshed delivered evidence", () => {
    const noEvidence = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: task("completed_needs_evidence", {
        terminalDeliveryStatus: "missing_final_delivery_evidence",
      }),
    });
    const delivered = taskResumeReducer(refreshingState(), {
      type: "refresh_succeeded",
      task: task("completed", {
        terminalDeliveryStatus: "delivered",
        finalDeliveryEvidencePresent: true,
      }),
    });

    expect(hasRefreshedTaskCompletionProof(noEvidence)).toBe(false);
    expect(hasRefreshedTaskCompletionProof(delivered)).toBe(true);
  });
});
