import { describe, expect, it } from "vitest";

import type { ReviewAction } from "@/tauri";
import {
  hasRefreshedMaterializationProof,
  initialReviewDispatchState,
  reviewDispatchReducer,
} from "./reviewDispatchContract";

function approveAction(overrides: Partial<ReviewAction> = {}): ReviewAction {
  return {
    id: "review-1:approve",
    label: "Approve",
    kind: "approve",
    effect: "decision_only",
    enabled: true,
    requiresConfirmation: true,
    targetReviewItemId: "review-1",
    expectedMaterializationStatusAfterDispatch: "unknown",
    completionProofAfterDispatch: false,
    ...overrides,
  } as ReviewAction;
}

describe("review dispatch contract", () => {
  it("requires confirmation, dispatch, and backend refresh in order", () => {
    const action = approveAction();
    const confirming = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action,
    });
    const dispatching = reviewDispatchReducer(confirming, { type: "confirm" });
    const refreshing = reviewDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const resolved = reviewDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      item: {
        reviewItemId: "review-1",
        status: "approved",
        materializationStatus: "unknown",
      },
    });

    expect(confirming.phase).toBe("confirming");
    expect(dispatching.phase).toBe("dispatching");
    expect(refreshing.phase).toBe("refreshing");
    expect(resolved.phase).toBe("resolved");
    expect(hasRefreshedMaterializationProof(resolved)).toBe(false);
  });

  it("treats an exact backend-approved review button as the single confirmation", () => {
    const dispatching = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction({ requiresConfirmation: false }),
    });

    expect(dispatching.phase).toBe("dispatching");
  });

  it("never treats dispatch success or approved-unknown as applied", () => {
    const confirming = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction(),
    });
    const dispatching = reviewDispatchReducer(confirming, { type: "confirm" });
    const refreshing = reviewDispatchReducer(dispatching, { type: "dispatch_succeeded" });

    expect(refreshing.phase).toBe("refreshing");
    expect(hasRefreshedMaterializationProof(refreshing)).toBe(false);
  });

  it("fails closed for disabled actions, completion claims, and mismatched refresh targets", () => {
    const disabled = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction({ enabled: false, disabledReason: "Exact scope is incomplete." }),
    });
    const invalid = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction({ completionProofAfterDispatch: true }),
    });
    const confirming = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction(),
    });
    const dispatching = reviewDispatchReducer(confirming, { type: "confirm" });
    const refreshing = reviewDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const mismatch = reviewDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      item: {
        reviewItemId: "review-2",
        status: "approved",
        materializationStatus: "applied",
      },
    });

    expect(disabled).toMatchObject({ phase: "blocked", reason: "Exact scope is incomplete." });
    expect(invalid).toMatchObject({
      phase: "blocked",
      reason: "action_contract_claims_completion_after_dispatch",
    });
    expect(mismatch).toMatchObject({
      phase: "failed",
      stage: "refresh",
      errorCode: "review_refresh_target_mismatch",
    });
  });

  it("waits for the refreshed read model to confirm the requested decision", () => {
    const confirming = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction(),
    });
    const dispatching = reviewDispatchReducer(confirming, { type: "confirm" });
    const refreshing = reviewDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const awaiting = reviewDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      item: {
        reviewItemId: "review-1",
        status: "pending",
        materializationStatus: "not_started",
      },
    });

    expect(awaiting).toMatchObject({
      phase: "awaiting_projection",
      reason: "refreshed_read_model_does_not_confirm_action_yet",
    });
    expect(hasRefreshedMaterializationProof(awaiting)).toBe(false);
  });

  it("blocks actions that require a different local or backend refresh handler", () => {
    const evidence = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction({
        kind: "view_evidence",
        effect: "evidence_only",
        requiresConfirmation: false,
      }),
    });
    expect(evidence).toMatchObject({
      phase: "blocked",
      reason: "evidence_action_requires_navigation_handler",
    });
  });

  it("recognizes applied only from the refreshed read model", () => {
    const confirming = reviewDispatchReducer(initialReviewDispatchState, {
      type: "request",
      action: approveAction(),
    });
    const dispatching = reviewDispatchReducer(confirming, { type: "confirm" });
    const refreshing = reviewDispatchReducer(dispatching, { type: "dispatch_succeeded" });
    const resolved = reviewDispatchReducer(refreshing, {
      type: "refresh_succeeded",
      item: {
        reviewItemId: "review-1",
        status: "approved",
        materializationStatus: "applied",
      },
    });

    expect(hasRefreshedMaterializationProof(resolved)).toBe(true);
  });
});
