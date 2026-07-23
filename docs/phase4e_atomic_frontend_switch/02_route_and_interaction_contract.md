# Phase 4E Route And Interaction Contract

Date: 2026-07-21

## Information Priority

Each product work surface follows one order:

1. current goal or current object;
2. blocker, risk, or unknown boundary;
3. next valid action;
4. evidence access;
5. subordinate technical/debug detail.

The top context bar carries only route context and the most important boundary.
It does not duplicate every status from the work surface or inspector.

## Route Behavior

- Primary desktop navigation changes the canonical route and sets one
  `aria-current="page"` item.
- Settings is utility navigation, not a primary product journey.
- Retired and unknown routes remain at their requested path and render an
  unavailable surface with a single return action.
- There is no `/v2` route, compatibility redirect, or production fallback to
  the old shell.

## Action Contract

Every callable product action preserves `id`, `kind`, `enabled`,
`disabledReason`, and `targetRef`. Disabled controls explain why they cannot be
called. No style-only control is presented as an enabled product action.

### Workspace And Conversation

- Workspace actions use the backend-projected exact task/action identity.
- Conversation creates no session before an explicit send.
- Send delegates to governed Main Chat, then reloads exact history and the
  Workspace/Tasks/Review read models.
- Send return, task resume, and refreshed running state are not task-completion
  proof.

### Task Controls

- Resume, retry, cancel, and refresh use the exact `taskId`/`runId`/action
  target required by the command contract.
- Cancel requires explicit confirmation.
- Dispatch is followed by read-model refresh and identity verification.
- Failed or incoherent refresh remains blocked/unknown; callback success does
  not manufacture a terminal task state.

### Review

- Opening a review item never changes its status.
- Decision actions are approval, rejection, modification/later where projected;
  permission decisions retain their scope and confirmation semantics.
- Approval refreshes the exact ReviewItem. It does not resume a task or apply a
  durable change automatically.
- `approved`, `applying`, `applied`, `failed`, `rolled_back`, `rejected`, and
  `unknown` remain distinct.

### LifeModel And First Build

- Durable presentation joins only exact ReviewItem/proposal/materialization
  identities.
- Applied treatment requires refreshed backend materialization proof.
- First build appears only when the read model proves no LifeModel exists.
- Builder submissions create proposals for Review; they never directly write
  LifeModel truth.

### Settings And Privacy

- Draft state is local editing state, not backend truth.
- Provider test, review approval, save, and boundary refresh are separate
  steps.
- A test receipt is not save proof. Save return is not privacy-boundary proof.
- The final local/private conclusion comes only from a refreshed
  `ProviderPrivacyBoundarySummary`; missing or unknown remains non-green.

## Accessible Feedback

- navigation, route status, command status, and inspector changes use semantic
  status/live regions where appropriate;
- focus indicators remain visible;
- opening the evidence inspector moves focus into it and closing restores focus
  to the trigger;
- failure and disabled states retain full text contrast rather than applying
  opacity to an entire region.

`COMMAND_RETURN_EQUALS_COMPLETION=NO`

`APPROVED_EQUALS_APPLIED=NO`

`UNKNOWN_FAILS_CLOSED=YES`
