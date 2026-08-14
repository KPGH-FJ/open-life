# OpenLife Capable Agent Harness Plan

Status: active

## Objective

Turn the reconstructed R0-R8 baseline into one capable general knowledge-work
Agent. Complete the canonical runtime rather than adding another vertical
slice, and delete each replaced backend and frontend path in the same stage.

R0-R8 remains completed Git history and reusable evidence. It is not authority
for the current H0-H6 sequence.

## Product contract

- Conversation is the primary workspace. Chat answers directly; Work owns a
  durable outcome and completion contract.
- Work uses one `Task -> Run -> Item -> ItemAttempt` lifecycle. Planning,
  adaptive tool use, approval, steering, recovery, and future subagents are
  phases inside it, never separate product owners.
- The model understands the goal, proposes structured plans, selects eligible
  tools, replans from observations, and proposes completion.
- Policy owns authorization, risk, scope, and data route. The runtime owns
  schema validation, budgets, scheduling, receipts, and completion proof.
- Artifact identity is independent of Proposal. Workspace files own Artifact
  content; SQLite owns identity, versions, digests, provenance, and lifecycle.
- Agent Memory and LifeModel remain optional typed collaborators. They do not
  own execution, permission, or completion.

## Migration rules

1. A production concern has one canonical writer and recovery owner.
2. Each stage includes capability, lifecycle, controls, ViewModel, frontend,
   recovery, tests, and deletion of the path it replaces.
3. No release fallback, dual write, compatibility switch, or second task
   runtime may survive a stage boundary.
4. Direct deletion is preferred because there is no user task history to
   migrate. Retain only verified settings, provider references, Memory,
   LifeModel, Project, and resource configuration.
5. Keep cleanup checks proportional: compile and product tests first, with a
   small source guard only for a high-risk retired entrypoint.
6. Controlled, browser-shell, exact-native, and external-live evidence remain
   distinct.

## Sequence

| Stage | Complete outcome |
| --- | --- |
| H0 | Canonical Chat/Work starts and runs independently of retired execution stores, keys, gates, and frontend health coupling |
| H1 | One structured Planner, ItemScheduler, ItemExecutor, budget owner, and CompletionEvaluator drives general Work |
| H2 | Local document, Web, selected Skill, read-only MCP, and file capabilities use the unified loop; keyword/legacy ReAct owners are deleted |
| H3 | Independent ArtifactVersion, Changes, Preview, Verification, ReviewCheckpoint, Undo, effect-unknown, and recovery complete the result/effect loop |
| H4 | Workbench presents plan, progress, steering, inline decisions, results, and Needs Attention from canonical backend ViewModels; duplicate frontend logic is deleted |
| H5 | Chinese/English behavior matrix, native golden paths, and the minimum necessary live provider/Web evidence prove capability and failure semantics |
| H6 | Remaining retired stores, keys, packages, fixtures, IPCs, routes, and docs are deleted; the release baseline is clean |

## Completed: H0 - Canonical Independence

Canonical Chat and Work now admit only the stores they actually use. Canonical
Task receipts use an independent authority, product readiness no longer
inherits unrelated legacy-store warnings, and a fresh exact release bundle
opened and reopened the canonical Conversation and TaskRuntime stores from an
isolated profile. Controlled repository gates and the exact-native restart
check passed before advancing.

## Current stage: H1 - Unified Work Orchestration

### In scope

1. Introduce one structured Planner that turns a Work goal and constraints into
   typed Items without creating another durable lifecycle.
2. Introduce one ItemScheduler and ItemExecutor over canonical Run state.
3. Make provider and tool attempts consume one explicit Run budget owned by
   the runtime, with bounded retries and visible exhaustion semantics.
4. Introduce one CompletionEvaluator that requires completed required Items,
   valid receipts, and the requested result or review checkpoint.
5. Keep planning, execution, observations, replanning, and completion inside
   the same `Task -> Run -> Item -> ItemAttempt` owner.
6. Delete any strategy projection or special-case execution owner replaced by
   the unified path in this stage.

### Out of scope

- new connectors, Computer Use, arbitrary shell, email/calendar send, or
  scheduling expansion;
- broad capability expansion reserved for H2;
- broader Memory or LifeModel learning; and
- migration of retired task execution/test data.

### Acceptance

- A general Work request creates one canonical plan and executes its required
  Items through the same Run; no parallel task-session or plan-session owner is
  created.
- Planner output is schema-validated, bounded, and recoverably persisted before
  execution; invalid plans fail or replan without inventing completion.
- Provider and tool calls consume the same persisted Run budget and each call
  has a canonical ItemAttempt receipt.
- Completion is impossible while a required Item is pending, failed, unknown,
  or waiting for review.
- Cancellation, retry, and steering preserve Task identity and create only the
  expected Run/Item transitions.
- Focused orchestration tests and full repository gates pass before H1 is
  marked complete.

## Checks

During implementation, run focused Rust and frontend tests. At stage closure:

```sh
git diff --check
cargo fmt --check
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
corepack pnpm --dir frontend verify:release-absence
```

Exact-native and external-live checks follow `docs/development/testing.md`.
External-live runs only when the accepted behavior cannot be proven locally
and require the necessary provider/network authorization.

## Stop condition

Do not advance while the current stage leaves a second production owner or a
false completion/readiness claim. Pause only for a required secret, an
irreversible external action, a product-direction change, or a genuine blocker.
