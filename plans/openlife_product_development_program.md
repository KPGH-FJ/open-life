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

## Completed: H1 - Unified Work Orchestration

General Work now creates one bounded structured plan inside its existing Run.
The complete plan and immutable budget policy are persisted in
`task_runtime.db`; budget use is reconstructed from canonical Items and
ItemAttempts after restart. Policy limits eligible step kinds, the plan drives
the canonical read adapters, and a mechanical CompletionEvaluator prevents a
FinalResult when a required step, receipt, verification, Artifact, or review
checkpoint is missing. Invalid model plans receive one bounded repair attempt
and then fail closed. Controlled document, Web, selected Skill, builtin MCP,
registered MCP, retry, cancellation, artifact-review, and failure tests remain
green on the same Task/Run owner.

## Completed: H2 - Unified Capability Loop

Work plan schema v2 now selects imported-document, workspace-file, Web Search,
Web Fetch, selected Skill, and exact registered read-only MCP capabilities from
the Policy-bounded set. Fixed capabilities cannot carry model targets; MCP
selection is bound to the current manifest id and execution-contract digest,
and all executable arguments remain runtime-derived. Every adapter executes as
a canonical ItemAttempt with ToolGateway/provider receipt and digest-only
Observation. One bounded evidence-driven plan revision may continue the same
Run without widening scope, repeating completed capabilities, resetting budget,
or hiding failed/unknown work. Release compatibility code no longer compiles
the retired ReAct or PlanExecute execution branches. Focused capability tests
and all repository gates passed before advancing.

## Current stage: H3 - Result And Effect Loop

### In scope

1. Make ArtifactVersion identity, content digest, provenance, draft, Review
   checkpoint, materialization, verification, and Undo one canonical lifecycle
   independent of Proposal identity.
2. Route every supported file effect through one ItemExecutor/materializer
   contract with exact target preconditions, typed receipt, cancellation fence,
   and effect-unknown semantics.
3. Resume the same Task/Run after inline or Review Center decisions; approval
   grants only the exact checkpoint and is never treated as materialization.
4. Recover crashes at prepared, dispatched, confirmed, projection-pending, and
   Undo boundaries without blind redispatch or a second Artifact owner.
5. Project Changes, Preview, Verification, Needs Attention, and Undo solely from
   canonical ArtifactVersion and ItemAttempt truth, then delete replaced
   proposal-derived artifact identity and duplicate result projections.

### Out of scope

- new connectors, Computer Use, arbitrary shell, email/calendar send, or
  scheduling expansion;
- broad new file-edit semantics beyond the existing reviewed artifact effects;
- broader Memory or LifeModel learning; and
- migration of retired task execution/test data.

### Acceptance

- An Artifact has stable identity before Review and every version binds exact
  Task/Run/Item provenance, target precondition, content digest, and status.
- Approve, reject, cancel, retry, materialize, verify, and Undo produce truthful
  same-lifecycle transitions; unknown physical effects never become success.
- Restart recovery is idempotent at every effect boundary and never writes the
  file twice or invents completion from Proposal status.
- Backend ViewModels expose canonical Changes, Preview, Verification, attention,
  and Undo facts without joining a retired TaskSession or AgentRun owner.
- Focused Artifact/effect/recovery tests and full repository gates pass before
  H3 is marked complete.

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
