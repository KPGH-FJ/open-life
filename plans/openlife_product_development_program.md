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

## Current stage: H2 - Unified Capability Loop

### In scope

1. Replace keyword-selected Work tools with one model-driven, manifest-bounded
   capability selection contract inside the structured plan/Run.
2. Route local document, workspace file, Web search/fetch, selected Skill, and
   registered read-only MCP through one ItemScheduler and ItemExecutor.
3. Preserve exact task/project/resource/provider scope and ToolGateway receipts
   for every adapter attempt; the model cannot mint a tool, permission, or
   argument outside the manifest and PolicyDecision.
4. Support bounded observation-driven continuation and replanning in the same
   Run without restoring ReAct or PlanExecute as product owners.
5. Delete the replaced keyword selectors, legacy ReAct product path,
   strategy projection, and any release consumer that still treats them as a
   separate execution lifecycle.

### Out of scope

- new connectors, Computer Use, arbitrary shell, email/calendar send, or
  scheduling expansion;
- write/effect lifecycle expansion reserved for H3;
- broader Memory or LifeModel learning; and
- migration of retired task execution/test data.

### Acceptance

- Ordinary natural-language Work selects eligible capabilities without
  requiring product-specific keywords such as `web.search` or `mcp`.
- Every selected capability is present in the policy-bounded manifest and every
  execution is a canonical ItemAttempt with a ToolGateway/provider receipt.
- Document + Web, selected Skill, registered MCP, and workspace-file scenarios
  complete or fail closed through the same adaptive loop.
- A failed/unknown observation cannot be hidden by later successful work;
  bounded replanning never widens scope or resets the Run budget.
- No release Work branch enters the retired ReAct/PlanExecute/task-session
  lifecycle, and replaced selectors and frontend/backend consumers are gone.
- Focused capability-loop tests and full repository gates pass before H2 is
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
