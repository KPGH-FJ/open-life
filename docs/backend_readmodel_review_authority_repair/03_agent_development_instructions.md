# Agent Development Instructions

Status: handoff instructions for the future Goal-mode development run.

Use these instructions after the user starts the Goal mode for:

```text
Backend ReadModel & Review Authority Repair Phase
```

## Operating Rules

1. Start by reading the authority stack in
   `00_goal_mode_preparation.md`.
2. Keep Phase7 single-system constraints active throughout the work.
3. Work one slice at a time. Do not combine slices unless the user explicitly
   asks and the same tests cover both changes.
4. Before editing a slice, produce a source map for the exact owner shift.
5. Do not implement official frontend replacement before the backend read model
   for that concern exists.
6. Do not restore deleted legacy objects, old commands, old wrappers, or old
   product routes.
7. Preserve dirty worktree boundaries. Stage only files in the slice.

## Core Engineering Standard

Solve root authority problems, not symptoms.

- A new frontend adapter is not a backend read-model owner.
- A proposal status is not a materialization status.
- A successful command dispatch is not proof that a task resumed or a durable
  write applied.
- A route/provider label is not external transmission evidence.
- A memory tier count is not Memory product readiness.
- A raw diagnostics field is not product readiness if a backend read model
  exists.

## Required Implementation Order

Preferred order:

1. R0 inventory and guards.
2. R1 backend shared ViewModel contract.
3. R2 ReviewItem and ReviewCenterViewModel.
4. R3 LifeModelViewModel backend owner.
5. R4 TasksViewModel and Workspace baseline.
6. R5 MemoryViewModel and ProviderPrivacyBoundarySummary.
7. R6 frontend convergence and anti-hallucination guards.

Do not start R4 Workspace UI work before R2 and the task identity part of R4
exist. Do not start top-level Memory product work before R5 exists.

## Slice Definition Of Done

Each slice must close with:

- source map updated or confirmed;
- code changes implemented;
- focused Rust tests added or updated when backend behavior changes;
- focused frontend tests added or updated when frontend rendering changes;
- guard tests added or updated when authority moves;
- validation commands run and recorded;
- self-review/hallucination check in the final response or docs;
- no readiness/completion claim beyond evidence.

## Anti-Hallucination Checklist

For every new claim, answer these before finalizing:

- Which file owns this behavior now?
- Is the owner backend, frontend display-only, test-only, doc-only, or
  historical?
- Does a Tauri command exist, or is this only a TypeScript adapter?
- Does this read model compute from real stores, or from fixture/mock data?
- Does an accepted proposal prove materialization, or only decision state?
- Does a resume/apply button mean the backend action completed, or only that a
  request can be dispatched?
- Does the UI say ready/completed when unknown/stale/error evidence exists?
- Did any old Phase7 expected-absent object get recreated?

## Review Criteria Before Passing A Slice

Reject the slice if any of these are true:

- It adds a second authority instead of moving authority.
- It keeps old page-local inference and merely renames it to ViewModel.
- It treats `accepted`, `completed`, or command success as durable state without
  refreshed backend read-model evidence.
- It hides `unknown`, `stale`, `blocked`, or `PHASE_2_REQUIRED` states behind
  optimistic copy.
- It broadens product-visible debug/developer surfaces without support policy.
- It changes frontend IA or ProductShell while the backend owner remains
  missing.
- It passes tests only by weakening mocks or removing guards.

## Suggested Commit Strategy

Use one commit per completed slice when possible:

```text
Add backend read-model envelope contract
Add ReviewItem read model authority
Add LifeModel backend ViewModel owner
Add backend task read model authority
Add Memory and provider privacy read models
Converge product pages on backend read models
```

If a slice is too large, split by backend owner first, frontend consumption
second, guard/tests third. Do not split in a way that leaves a product page
overclaiming in between commits.

## Expected Final Report Shape

When reporting a completed slice, use this shape:

```text
Status: passed / not passed
Scope: files changed
Authority moved: old owner -> new owner
Behavior: what changed
Non-goals preserved: what was intentionally not touched
Validation: commands and results
Residual risks: what remains blocked
Next slice: recommended next step
```
