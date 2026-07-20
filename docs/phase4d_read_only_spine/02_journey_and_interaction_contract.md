# Phase 4D Read-Only Journey And Interaction Contract

Status: `IMPLEMENTED`
Date: 2026-07-20

## Information Priority

Every migrated surface uses this order:

1. current goal or task question;
2. one active blocker/risk conclusion;
3. next available product action;
4. evidence entry;
5. collapsed technical/debug information.

The same fact is not repeated as a top status, banner, metric card, list row,
button reason, and Inspector sentence. Today and Tasks use continuous sections
and compact rows rather than a metric-card dashboard.

## Today State Machine

```text
loading sources
  -> projection missing/error: error + refresh only
  -> projection ready, supporting source failed: stale + read-only + refresh
  -> projection ready, no goal/task/review: empty
  -> projection ready, Safe Mode active: protected read-only state
  -> projection ready, blocker/review present: ready with one attention section
  -> projection ready, no attention: ready
```

- Safe Mode is amber protection, not red failure.
- unknown provider/privacy remains amber even if Today content loaded.
- stale disables adapter actions that require fresh state and exposes the
  backend/frontend disabled reason.
- opening Review enters an unavailable Review surface in this slice; it never
  changes pending to approved.

## Tasks State Machine

```text
loading TasksViewModel
  -> command/envelope error: error + refresh; suppress payload counts and rows
  -> stale: show existing rows as stale, no controls
  -> empty: explicit no-task state
  -> ready: searchable/filterable list
      -> select row: Inspector context only
      -> completed without final evidence: evidence-required, not completed
```

The read-only Tasks slice does not render Resume, Retry, Cancel, or Refresh
Context controls. Their `allowedControls` values remain visible only as
technical contract metadata until the governed-action journey implements
dispatch -> refresh -> identity/state verification.

An error payload is never treated as a normal empty payload. In particular,
the page does not show `0 items`, an empty-list conclusion, or normal list
controls while the envelope status is `error`.

## Unavailable Navigation

Workspace, Review Center, LifeModel, and Settings categories remain reachable
for IA and keyboard testing, but each click must:

1. update the current navigation state;
2. move focus to the new context heading;
3. render a reason and an available alternative;
4. announce the change;
5. perform no redirect or backend mutation.

No enabled navigation entry is allowed to do nothing.

## Inspector

Inspector order is fixed:

1. what happened;
2. risk;
3. next step;
4. EvidenceRef-like metadata rows;
5. collapsed technical details.

Selecting evidence reports its exact id/source/sensitivity and does not claim
that a metadata ref contains a readable evidence body. Closing restores focus
to the Inspector trigger.

## Keyboard And Feedback

- current primary navigation uses `aria-current="page"`;
- route/context changes focus the top context heading;
- Inspector open/close restores focus through the Shell contract;
- refresh, search, filter, selection, and unavailable navigation use one polite
  live region message;
- concrete load failure uses visible error semantics;
- all icon-only buttons retain an accessible label and tooltip;
- desktop Tab order remains sidebar -> utilities -> context -> page ->
  Inspector after opening.

## Fixture Boundary

The Phase 4D QA toolbar is outside `OpenLifeWorkbenchShell`. It may switch
between real Tauri and named layout fixtures for deterministic visual/state
testing. The toolbar must always expose whether the current data source is:

- real Tauri backend;
- browser layout fixture; or
- unavailable desktop connection.

Fixture values do not count as backend readiness, task completion, local-only
proof, or Tauri dogfood evidence.
