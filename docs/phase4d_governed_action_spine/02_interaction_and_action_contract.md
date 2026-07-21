# Phase 4D Governed Interaction And Action Contract

Status: `IMPLEMENTED`
Date: 2026-07-20

## Information Priority

The continuous journey uses one fixed hierarchy:

1. current goal/task;
2. current blocker or risk;
3. next exact action;
4. evidence entry;
5. technical identifiers and diagnostics.

Workspace stays deliberately sparse. Review carries the detailed scope,
reason, impact, source, expiry, and decision controls. The same permission fact
is not repeated as a metric, banner, list count, and Inspector headline.

## Review Decision State Machine

```text
idle
  -> request disabled/incoherent: blocked
  -> request requiring confirmation: confirming
  -> confirm: dispatching
  -> command failure: failed(dispatch)
  -> command return: refreshing
  -> refreshed target missing/error/stale: failed(refresh)
  -> same target does not yet confirm decision: awaiting_projection
  -> same target confirms requested decision: resolved
```

- opening Review only selects context;
- approval requires confirmation;
- reject and later use their typed backend actions;
- Evidence opens the Inspector and never enters the dispatch reducer;
- approved ToolPermission reads `已允许一次，尚未继续任务`;
- materialization wording remains reserved for Inspector and durable-truth
  journeys.

## Task Resume State Machine

```text
idle
  -> disabled/missing/mismatched control: blocked
  -> exact control requiring confirmation: confirming
  -> confirm: dispatching
  -> command failure: failed(dispatch)
  -> command return: refreshing
  -> Tasks read error/stale: failed(refresh)
  -> exact task absent or still waiting/blocked/unknown: awaiting_projection
  -> exact task running/nonblocked: resolved, not completed
  -> exact task completed + delivered evidence: resolved completed
```

The controller rejects a resume control when its kind, effect, target task, or
completion claim is incoherent. It never uses the currently selected row as an
implicit target.

## Action Categories

| Category | Visible examples | Dispatch owner |
| --- | --- | --- |
| ProductAction | refresh, product navigation | page/Shell source loader |
| ReviewAction | approve, reject, later, view evidence | review reducer and typed review data source |
| TaskControl | resume exact task | task-resume reducer and task data source |
| DebugAction | none in product work surface | not implemented in this slice |

Review and Task controls expose QA attributes for id, kind, effect, enabled,
disabledReason, targetRef, confirmation, expected materialization status, and
completionProofAfterDispatch. These attributes mirror the typed contract; they
are not hidden replacement commands.

## Component State Matrix

| State | Authority requirement | Visual treatment | Interaction contract |
| --- | --- | --- | --- |
| default | ready envelope plus coherent typed action/control | neutral white surface and standard border | exact target action is available |
| hover | same authority as default | border/background emphasis only; no layout shift | no state transition until activation |
| focus | keyboard focus on a reachable control | visible high-contrast focus ring | follows sidebar -> work surface -> action -> Inspector order |
| disabled | backend disabled action or local fail-closed guard with a reason | readable disabled control plus visible reason; no whole-section opacity | cannot dispatch and remains discoverable to assistive technology |
| loading | command or read-model request is in flight | spinner and stable control dimensions | duplicate dispatch and concurrent refresh are blocked |
| stale | stale backend envelope | amber protection notice and stale status | decisions and task controls disabled; evidence and refresh remain available |
| blocked | coherent blocker, incomplete permission scope, or awaiting projection | amber protection notice tied to the exact target | no implicit retry, approval, resume, or completion |
| unknown | missing, mismatched, incoherent, or unavailable authority | neutral/unknown status without verified green | fail closed; only read, evidence, or refresh paths remain |

Manual refresh resets local interaction feedback and reads the backend owners
again. It never replays the prior ReviewAction or TaskControl. Refresh requests
are rejected while a dispatch-owned refresh is already in flight.

## Fail-Closed Rules

- Workspace stale/error suppresses review and task-control dispatch.
- Review stale disables approve/reject/later while preserving evidence access.
- Permission context `incomplete` disables approval and shows the backend
  disabled reason.
- Missing refreshed ReviewItem cannot become an approved UI result.
- Missing or mismatched refreshed task cannot become running or completed.
- Unknown or stale provider/privacy state cannot render verified local green.
- Error envelopes suppress old/empty payload conclusions.

## Keyboard And Feedback

- current navigation uses `aria-current="page"`;
- product context changes focus the context heading;
- dialogs focus their title first, trap focus, support Escape, and restore the
  opener after cancellation/close;
- confirmation is reachable with visible keyboard focus but is not focused
  automatically for high-impact permission decisions;
- status changes use the Shell live region and visible notices;
- Inspector close restores focus to its trigger;
- disabled actions always show a readable reason.

## Desktop Layout

- target: Tauri desktop only, minimum `1024x720`;
- primary navigation remains in the fixed left sidebar;
- Review uses a 248px queue plus a continuous detail surface;
- the review decision area is sticky so the primary decision remains visible
  at 1024x720 while details scroll;
- Evidence remains available through the fixed Shell Inspector;
- no mobile app bar, drawer, bottom navigation, or mobile acceptance is part of
  this implementation.
