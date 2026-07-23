import type {
  ReviewAction,
  ReviewItemDecisionStatus,
  ReviewItemMaterializationStatus,
} from "../tauri";

export type RefreshedReviewResolution = {
  reviewItemId: string;
  status: ReviewItemDecisionStatus;
  materializationStatus: ReviewItemMaterializationStatus;
};

export type ReviewDispatchState =
  | { phase: "idle" }
  | { phase: "blocked"; action: ReviewAction; reason: string }
  | { phase: "confirming"; action: ReviewAction }
  | { phase: "dispatching"; action: ReviewAction }
  | { phase: "refreshing"; action: ReviewAction }
  | {
      phase: "awaiting_projection";
      action: ReviewAction;
      refreshed: RefreshedReviewResolution;
      reason: string;
    }
  | { phase: "resolved"; action: ReviewAction; refreshed: RefreshedReviewResolution }
  | {
      phase: "failed";
      action: ReviewAction;
      stage: "dispatch" | "refresh";
      errorCode: string;
    };

export type ReviewDispatchEvent =
  | { type: "request"; action: ReviewAction }
  | { type: "confirm" }
  | { type: "cancel_confirmation" }
  | { type: "dispatch_succeeded" }
  | { type: "dispatch_failed"; errorCode: string }
  | { type: "refresh_succeeded"; item: RefreshedReviewResolution }
  | { type: "refresh_failed"; errorCode: string }
  | { type: "reset" };

export const initialReviewDispatchState: ReviewDispatchState = { phase: "idle" };

const expectedEffects: Record<ReviewAction["kind"], ReviewAction["effect"]> = {
  approve: "decision_only",
  reject: "decision_only",
  edit: "decision_only",
  later: "decision_only",
  revoke: "decision_only",
  apply: "materialization_request",
  resume: "task_resume_request",
  view_evidence: "evidence_only",
};

function actionContractBlocker(action: ReviewAction): string | null {
  if (![action.id, action.label, action.targetReviewItemId].every(value => value?.trim())) {
    return "review_action_required_field_missing";
  }
  if (action.effect !== expectedEffects[action.kind]) {
    return "review_action_kind_effect_mismatch";
  }
  if (action.enabled && action.disabledReason !== undefined) {
    return "enabled_action_has_disabled_reason";
  }
  if ((action.kind === "approve" || action.kind === "apply") && !action.requiresConfirmation) {
    return "review_action_confirmation_required";
  }
  if (action.completionProofAfterDispatch) {
    return "action_contract_claims_completion_after_dispatch";
  }
  if (action.effect === "evidence_only") {
    return "evidence_action_requires_navigation_handler";
  }
  if (action.effect === "task_resume_request") {
    return "resume_action_requires_task_refresh_contract";
  }
  if (action.kind === "revoke") {
    return "revoke_action_requires_backend_status_contract";
  }
  return null;
}

function refreshedItemConfirmsAction(
  action: ReviewAction,
  item: RefreshedReviewResolution
): boolean {
  if (action.kind === "approve") return item.status === "approved";
  if (action.kind === "reject") return item.status === "rejected";
  if (action.kind === "later") return item.status === "deferred";
  if (action.kind === "edit") return item.status === "edited";
  if (action.kind === "apply") {
    return ["applying", "applied", "failed", "rolled_back"].includes(item.materializationStatus);
  }
  return false;
}

export function reviewDispatchReducer(
  state: ReviewDispatchState,
  event: ReviewDispatchEvent
): ReviewDispatchState {
  if (event.type === "reset") return initialReviewDispatchState;

  if (event.type === "request") {
    if (!event.action.enabled) {
      return {
        phase: "blocked",
        action: event.action,
        reason: event.action.disabledReason?.trim() || "backend_action_disabled",
      };
    }
    const contractBlocker = actionContractBlocker(event.action);
    if (contractBlocker) {
      return {
        phase: "blocked",
        action: event.action,
        reason: contractBlocker,
      };
    }
    return event.action.requiresConfirmation
      ? { phase: "confirming", action: event.action }
      : { phase: "dispatching", action: event.action };
  }

  if (state.phase === "confirming") {
    if (event.type === "confirm") return { phase: "dispatching", action: state.action };
    if (event.type === "cancel_confirmation") return initialReviewDispatchState;
  }

  if (state.phase === "dispatching") {
    if (event.type === "dispatch_succeeded") {
      return { phase: "refreshing", action: state.action };
    }
    if (event.type === "dispatch_failed") {
      return {
        phase: "failed",
        action: state.action,
        stage: "dispatch",
        errorCode: event.errorCode,
      };
    }
  }

  if (state.phase === "refreshing") {
    if (event.type === "refresh_succeeded") {
      if (event.item.reviewItemId !== state.action.targetReviewItemId) {
        return {
          phase: "failed",
          action: state.action,
          stage: "refresh",
          errorCode: "review_refresh_target_mismatch",
        };
      }
      if (!refreshedItemConfirmsAction(state.action, event.item)) {
        return {
          phase: "awaiting_projection",
          action: state.action,
          refreshed: event.item,
          reason: "refreshed_read_model_does_not_confirm_action_yet",
        };
      }
      return { phase: "resolved", action: state.action, refreshed: event.item };
    }
    if (event.type === "refresh_failed") {
      return {
        phase: "failed",
        action: state.action,
        stage: "refresh",
        errorCode: event.errorCode,
      };
    }
  }

  return state;
}

export function hasRefreshedMaterializationProof(state: ReviewDispatchState): boolean {
  return state.phase === "resolved" && state.refreshed.materializationStatus === "applied";
}
