# Phase 3C Review Decision Projection Gap

Status: `BLOCKING_FOR_REACT_PORT`.
Date: 2026-07-10.
Scope: contract gap and target UI projection only; no backend implementation.

## Decision

The rich approval and permission context shown in the Phase 3C static mockup
must not be treated as fields already owned by `ReviewItem`. React migration is
blocked until a backend-owned review projection can answer the user's decision
questions without reading raw `AgentProposal` JSON in the page.

The static fixtures classify this target as
`PROPOSED_REVIEW_PROJECTION`.

## Verified Current State

`openlife-core/src/agent/review_item.rs` currently projects:

- review item id, type, source, decision status, and materialization status;
- allowed actions and risk;
- expiration, EvidenceRef values, target refs, and optional task-resume relation.

It does not project:

- a one-sentence change summary;
- current and proposed values;
- reason and user-understandable source summary;
- impact summary or affected object label;
- a complete permission scope;
- permission duration, grant mode, or revocation instructions.

`openlife-core/src/agent/types.rs` already stores `AgentProposal.before`,
`after`, `reason`, `affected_path`, `source_detail`, risk, and expiration.
`openlife-core/src/agent/action_executor/tool_executor.rs` also builds a tool
permission proposal containing `canonical_scope`, capabilities, input summary,
blocked target, and policy. Those values are source material, not a product
ReviewItem projection.

## Required Decision Context

The backend-owned review read model needs an equivalent of:

```text
ReviewDecisionContext
  changeSummary
  currentValueSummary
  proposedValueSummary
  reasonSummary
  sourceSummary
  riskSummary
  impactSummary
  affectedObjectLabel
  expiresAt
  evidenceRefs
```

Requirements:

- Values must be bounded summaries, not an unfiltered raw proposal payload.
- Missing current values must be explicit `unknown`, not an empty diff.
- The projection must retain links to source proposal and evidence records.
- Page code must not reconstruct this context from raw proposals.

## Required Permission Context

Tool and external-action review items need an equivalent of:

```text
PermissionDecisionContext
  toolLabel
  capability
  purposeSummary
  targetLabel
  dataScopeSummary
  externalTransmission
  grantMode
  validUntil
  revocationSummary
  evidenceRefs
```

The current tool-permission proposal uses `allow_until_revoked`; it does not
prove that a UI action labeled `仅允许本次` can be honored. That action must
remain disabled until the contract expresses one-time scope, expiry, and
revocation semantics end to end.

## Decision And Application Lifecycle

The required lifecycle is:

```text
pending decision
  -> approved decision
  -> applying only after an acknowledged materialization request
  -> applied only after a refreshed backend read model proves completion
```

Current implementation boundaries:

- `approve` records a decision and currently advertises an expected
  materialization status of `unknown`.
- the generated `apply` action is disabled because no backend materialization
  request command is available;
- therefore the Phase 3C static flow ends at `已批准，尚未应用`;
- it must not optimistically display `正在应用` or `已完成`.

## Product Information Order

For a proposed change:

1. Explain the proposed change in one sentence.
2. Show `当前 -> 建议`.
3. Explain reason, source, risk, impact, and expiry.
4. Offer reject, later, edit, and approve actions.
5. Keep raw ids and field sources in technical details.

For a permission request:

1. Explain the intended action and purpose.
2. Show tool, capability, target, and data scope.
3. Show transmission boundary, duration, and revocation.
4. Offer only actions the backend can honor exactly.

This order follows useful review principles rather than copying another
product's visual design. GitHub documents reviewing a diff before choosing
approve or request-changes, and separately distinguishes deployment approval
from the job that proceeds afterward. Google recommends requesting the smallest
scope in context and supports revoking consent.

References:

- https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request
- https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/review-deployments
- https://developers.google.com/identity/protocols/oauth2/web-server
- https://developers.google.com/identity/oauth2/web/guides/use-token-model
- https://developers.google.com/identity/oauth2/web/guides/how-user-authz-works

## React Port Stop Conditions

Do not port the approval UI as a production surface until all are true:

- the review read model owns bounded decision and permission context;
- action labels match backend semantics exactly;
- one-time permission has enforceable duration and revocation behavior;
- approval, materialization dispatch, applying, and applied are separately
  represented;
- unknown or missing fields disable unsafe decisions;
- focused contract tests cover projection completeness and fail-closed states.
