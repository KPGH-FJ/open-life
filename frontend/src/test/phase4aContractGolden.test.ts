import { describe, expect, it } from "vitest";

import golden from "./fixtures/phase4a-contract-golden.json";
import type {
  ProviderPrivacyBoundarySummary,
  ReviewAction,
  ReviewItem,
  WorkspaceActivityItem,
} from "../tauri";

function asRecord(value: unknown): Record<string, unknown> {
  expect(value).toBeTypeOf("object");
  expect(value).not.toBeNull();
  expect(Array.isArray(value)).toBe(false);
  return value as Record<string, unknown>;
}

function assertReviewAction(value: unknown): asserts value is ReviewAction {
  const action = asRecord(value);
  expect(action.id).toBeTypeOf("string");
  expect(action.label).toBeTypeOf("string");
  expect(action.kind).toBeTypeOf("string");
  expect(action.effect).toBeTypeOf("string");
  expect(action.enabled).toBeTypeOf("boolean");
  expect(action.targetReviewItemId).toBeTypeOf("string");
  expect(action.completionProofAfterDispatch).toBe(false);

  const expectedEffect: Record<string, ReviewAction["effect"]> = {
    approve: "decision_only",
    reject: "decision_only",
    edit: "decision_only",
    later: "decision_only",
    revoke: "decision_only",
    apply: "materialization_request",
    resume: "task_resume_request",
    view_evidence: "evidence_only",
  };
  expect(action.effect).toBe(expectedEffect[String(action.kind)]);
}

function assertReviewItem(value: unknown): asserts value is ReviewItem {
  const item = asRecord(value);
  expect(item.id).toBeTypeOf("string");
  expect(item.type).toBeTypeOf("string");
  expect(item.status).toBeTypeOf("string");
  expect(item.materializationStatus).toBeTypeOf("string");
  expect(Array.isArray(item.allowedActions)).toBe(true);
  (item.allowedActions as unknown[]).forEach(assertReviewAction);

  const context = asRecord(item.decisionContext);
  expect(context.reviewItemId).toBe(item.id);
  const permission = asRecord(context.permission);
  expect(permission.status).toBe("ready");
  expect(permission.scopeKind).toBe("action_bound");
  expect(permission.policy).toBe("allow_once");
  expect(permission.missingFields).toEqual([]);
  expect(asRecord(permission.transmissionBoundary).externalTransmission).toBe("not_sent");
}

describe("Phase 4A Rust/TypeScript golden contract", () => {
  it("keeps ReviewItem action and exact-permission semantics aligned", () => {
    assertReviewItem(golden.reviewItem);
    const reviewItem: ReviewItem = golden.reviewItem as ReviewItem;
    const approve = reviewItem.allowedActions.find(action => action.kind === "approve");

    expect(approve).toMatchObject({
      enabled: true,
      requiresConfirmation: true,
      expectedMaterializationStatusAfterDispatch: "unknown",
      completionProofAfterDispatch: false,
    });
    expect(reviewItem.materializationStatus).toBe("not_started");
  });

  it("keeps Workspace activity and unknown privacy boundary fail-closed", () => {
    const activity = golden.workspaceActivity as WorkspaceActivityItem;
    const boundary = golden.providerBoundary as ProviderPrivacyBoundarySummary;

    expect(activity.status).toBe("waiting_decision");
    expect(boundary.routeType).toBe("unknown");
    expect(boundary.externalTransmission).toBe("unknown");
    expect(boundary.risk).toBe("unknown");
  });
});
