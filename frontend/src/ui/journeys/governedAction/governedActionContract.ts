import type { TaskControl, TaskViewModelItem } from "@/tauri";

export type TaskResumeState =
  | { phase: "idle" }
  | { phase: "blocked"; control: TaskControl; reason: string }
  | { phase: "confirming"; control: TaskControl }
  | { phase: "dispatching"; control: TaskControl }
  | { phase: "refreshing"; control: TaskControl }
  | {
      phase: "awaiting_projection";
      control: TaskControl;
      reason: string;
      refreshedTask: TaskViewModelItem | null;
    }
  | { phase: "resolved"; control: TaskControl; refreshedTask: TaskViewModelItem }
  | {
      phase: "failed";
      control: TaskControl;
      stage: "dispatch" | "refresh";
      errorCode: string;
    };

export type TaskResumeEvent =
  | { type: "request"; control: TaskControl; expectedTaskId: string }
  | { type: "confirm" }
  | { type: "cancel_confirmation" }
  | { type: "dispatch_succeeded" }
  | { type: "dispatch_failed"; errorCode: string }
  | { type: "refresh_succeeded"; task: TaskViewModelItem | null }
  | { type: "refresh_failed"; errorCode: string }
  | { type: "reset" };

export const initialTaskResumeState: TaskResumeState = { phase: "idle" };

function taskResumeContractBlocker(control: TaskControl, expectedTaskId: string): string | null {
  if (
    ![control.id, control.label, control.targetTaskId, expectedTaskId].every(value => value.trim())
  ) {
    return "task_control_required_field_missing";
  }
  if (control.kind !== "resume" || control.effect !== "task_resume_request") {
    return "task_control_kind_effect_mismatch";
  }
  if (control.targetTaskId !== expectedTaskId) {
    return "task_control_target_mismatch";
  }
  if (control.completionProofAfterDispatch) {
    return "task_control_claims_completion_after_dispatch";
  }
  if (control.enabled && control.disabledReason !== undefined) {
    return "enabled_task_control_has_disabled_reason";
  }
  return null;
}

function refreshedTaskIdentityMatches(control: TaskControl, task: TaskViewModelItem): boolean {
  return task.taskSessionId === control.targetTaskId;
}

function refreshedTaskConfirmsResume(task: TaskViewModelItem): boolean {
  return !["waiting_review", "waiting_permission", "blocked", "unknown"].includes(
    task.lifecycleStatus
  );
}

export function taskResumeReducer(state: TaskResumeState, event: TaskResumeEvent): TaskResumeState {
  if (event.type === "reset") return initialTaskResumeState;

  if (event.type === "request") {
    if (!event.control.enabled) {
      return {
        phase: "blocked",
        control: event.control,
        reason: event.control.disabledReason?.trim() || "backend_task_control_disabled",
      };
    }
    const blocker = taskResumeContractBlocker(event.control, event.expectedTaskId);
    if (blocker) return { phase: "blocked", control: event.control, reason: blocker };
    return event.control.requiresConfirmation
      ? { phase: "confirming", control: event.control }
      : { phase: "dispatching", control: event.control };
  }

  if (state.phase === "confirming") {
    if (event.type === "confirm") return { phase: "dispatching", control: state.control };
    if (event.type === "cancel_confirmation") return initialTaskResumeState;
  }

  if (state.phase === "dispatching") {
    if (event.type === "dispatch_succeeded") {
      return { phase: "refreshing", control: state.control };
    }
    if (event.type === "dispatch_failed") {
      return {
        phase: "failed",
        control: state.control,
        stage: "dispatch",
        errorCode: event.errorCode,
      };
    }
  }

  if (state.phase === "refreshing") {
    if (event.type === "refresh_failed") {
      return {
        phase: "failed",
        control: state.control,
        stage: "refresh",
        errorCode: event.errorCode,
      };
    }
    if (event.type === "refresh_succeeded") {
      if (!event.task) {
        return {
          phase: "awaiting_projection",
          control: state.control,
          reason: "refreshed_tasks_missing_target",
          refreshedTask: null,
        };
      }
      if (!refreshedTaskIdentityMatches(state.control, event.task)) {
        return {
          phase: "failed",
          control: state.control,
          stage: "refresh",
          errorCode: "task_refresh_target_mismatch",
        };
      }
      if (!refreshedTaskConfirmsResume(event.task)) {
        return {
          phase: "awaiting_projection",
          control: state.control,
          reason: "refreshed_task_does_not_confirm_resume_yet",
          refreshedTask: event.task,
        };
      }
      return { phase: "resolved", control: state.control, refreshedTask: event.task };
    }
  }

  return state;
}

export function hasRefreshedTaskCompletionProof(state: TaskResumeState): boolean {
  return (
    state.phase === "resolved" &&
    state.refreshedTask.lifecycleStatus === "completed" &&
    state.refreshedTask.terminalDeliveryStatus === "delivered" &&
    state.refreshedTask.finalDeliveryEvidencePresent
  );
}
