import type { TaskControl, TaskViewModelItem } from "@/tauri";

export type TaskControlDispatchState =
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

export type TaskControlDispatchEvent =
  | { type: "request"; control: TaskControl; expectedTaskId: string }
  | { type: "confirm" }
  | { type: "cancel_confirmation" }
  | { type: "dispatch_succeeded" }
  | { type: "dispatch_failed"; errorCode: string }
  | { type: "refresh_succeeded"; task: TaskViewModelItem | null }
  | { type: "refresh_failed"; errorCode: string }
  | { type: "reset" };

export const initialTaskControlDispatchState: TaskControlDispatchState = { phase: "idle" };

const effectByKind = {
  resume: "task_resume_request",
  retry: "task_retry_request",
  cancel: "task_cancel_request",
  refresh_context: "task_refresh_request",
} as const;

export type ExecutableTaskControlKind = keyof typeof effectByKind;

export function isExecutableTaskControl(
  control: TaskControl
): control is TaskControl & { kind: ExecutableTaskControlKind } {
  return control.kind in effectByKind;
}

function contractBlocker(control: TaskControl, expectedTaskId: string): string | null {
  if (
    ![control.id, control.label, control.targetTaskId, expectedTaskId].every(value => value.trim())
  ) {
    return "task_control_required_field_missing";
  }
  if (!isExecutableTaskControl(control)) return "task_control_not_executable_on_tasks_surface";
  if (control.effect !== effectByKind[control.kind]) return "task_control_kind_effect_mismatch";
  if (control.targetTaskId !== expectedTaskId) return "task_control_target_mismatch";
  if (control.kind === "retry" && !control.targetActionId?.trim()) {
    return "task_retry_target_action_missing";
  }
  if (control.kind === "cancel" && !control.requiresConfirmation) {
    return "task_cancel_requires_confirmation";
  }
  if (control.completionProofAfterDispatch) return "task_control_claims_completion_after_dispatch";
  if (control.enabled && control.disabledReason !== undefined) {
    return "enabled_task_control_has_disabled_reason";
  }
  return null;
}

function identityMatches(control: TaskControl, task: TaskViewModelItem): boolean {
  return task.taskSessionId === control.targetTaskId;
}

function refreshedStateConfirmsRequest(control: TaskControl, task: TaskViewModelItem): boolean {
  if (control.kind === "cancel") return task.lifecycleStatus === "cancelled";
  if (control.kind === "resume") {
    return !["waiting_review", "waiting_permission", "blocked", "unknown"].includes(
      task.lifecycleStatus
    );
  }
  if (control.kind === "retry") {
    return !["failed", "remote_unknown", "unknown"].includes(task.lifecycleStatus);
  }
  return control.kind === "refresh_context";
}

export function taskControlDispatchReducer(
  state: TaskControlDispatchState,
  event: TaskControlDispatchEvent
): TaskControlDispatchState {
  if (event.type === "reset") return initialTaskControlDispatchState;
  if (event.type === "request") {
    if (!event.control.enabled) {
      return {
        phase: "blocked",
        control: event.control,
        reason: event.control.disabledReason?.trim() || "backend_task_control_disabled",
      };
    }
    const blocker = contractBlocker(event.control, event.expectedTaskId);
    if (blocker) return { phase: "blocked", control: event.control, reason: blocker };
    return event.control.requiresConfirmation
      ? { phase: "confirming", control: event.control }
      : { phase: "dispatching", control: event.control };
  }
  if (state.phase === "confirming") {
    if (event.type === "confirm") return { phase: "dispatching", control: state.control };
    if (event.type === "cancel_confirmation") return initialTaskControlDispatchState;
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
      if (!identityMatches(state.control, event.task)) {
        return {
          phase: "failed",
          control: state.control,
          stage: "refresh",
          errorCode: "task_refresh_target_mismatch",
        };
      }
      if (!refreshedStateConfirmsRequest(state.control, event.task)) {
        return {
          phase: "awaiting_projection",
          control: state.control,
          reason: `refreshed_task_does_not_confirm_${state.control.kind}_yet`,
          refreshedTask: event.task,
        };
      }
      return { phase: "resolved", control: state.control, refreshedTask: event.task };
    }
  }
  return state;
}
