# OpenLife Capable Agent Harness Plan

Status: complete

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

## Completed: H3 - Result And Effect Loop

Work Artifact identity is now stable across versions and excludes Proposal id.
Each ArtifactVersion owns exact Task/Run/Item provenance, a durable managed
draft, target precondition, content digest, version-bound Review checkpoint,
materializer ItemAttempt, effect journal, verification, and version-bound Undo.
Changes and pending Preview read canonical version truth rather than Proposal
payload. The old `canonical_artifacts.proposal_id` column and Work Artifact
dual-write into ProposalStore were removed. Recovery now distinguishes
prepared, staged, confirmed, failed-before-effect, effect-unknown, and
confirmed-before-Review-projection without blind redispatch. V1 and V2
materialization, rejection, Undo, exact replay, schema migration, restart
recovery, backend projections, and full Rust/frontend repository gates pass.

The exact signed macOS bundle also passed identity and resource-seal checks. An
isolated launch remained stable and opened no network socket, but the newly
signed binary did not obtain the canonical internal credential needed to open
`task_runtime.db`; this is unknown native product evidence and is intentionally
deferred to H5 rather than credited as a native golden path.

## Completed: H4 - Canonical Workbench Product Surface

The shipped Workbench is now one Conversation-scoped surface. It combines Chat
and Work history with the exact canonical Tasks for that Conversation, their
structured plan, completion contract, bounded execution policy, ordered Items,
backend controls, Needs Attention facts, inline Review checkpoints, Results,
Changes, Preview, Verification, and Undo. A mismatched Conversation projection
is hidden rather than shown under the wrong chat.

Top-level Tasks and Review navigation and the duplicate frontend Resume
lifecycle were deleted. Task controls and refreshed-state confirmation bind
only to canonical Task identity. LifeModel and Settings checkpoints can open as
one exact inline decision without exposing an unrelated global queue, and Back
returns to the originating product context. Wide/narrow component tests, full
frontend tests, browser-shell checks, production absence guards, and the full
Rust repository gates passed before advancing.

## Completed: H5 - Behavior And Native Evidence Matrix

The bilingual controlled matrix now enters through the shipped Chat/Work
coordinator and covers direct answers, documents, Web, mixed-source reports,
Skills, read-only MCP, planning, steering, Review checkpoints, cancellation,
retry, Artifact verification and Undo, blocked scope, provider failure,
effect-unknown, restart recovery, and Workbench states. All controlled rows and
the full Rust/frontend/browser-shell/absence gates pass.

An exact signed QA bundle with an isolated profile proved visible Conversation,
Chat, canonical Work progress, document plus real Web search, inline Review,
single file materialization, digest verification, cancellation, retry, and
cross-build restart recovery. The explicitly selected Provider/model and real
Web route completed without silent substitution. QA and dev use separate
atomic `0600` profile secret files; release remains on Keychain. Rebuilding the
QA executable changed its CDHash without another credential initialization or
recovery prompt.

The matrix exposed and closed defects in canonical retry identity, exact Web
subject extraction, action-instruction Memory classification, Workbench refresh
after controls, Artifact safe-path admission, and bounded Artifact schema
repair. Controlled, exact-native, and external-live evidence remain separate in
`docs/development/testing.md`.

## Completed: H6 - Clean Release Baseline

Release Chat and Work now have one canonical persistence, control, recovery,
and projection spine. Retired execution stores, strategy runtimes, task-control
owners, event stores, queues, packages, commands, fixtures, credential purposes,
routes, frontend branches, and stable documentation claims were removed with
their consumers. The remaining historical scheduler column is read only by its
bounded database migration and cannot become a runtime owner.

The kernel now receives structured Work-plan decisions and current capability
contracts directly. Keyword ReAct planning, replay-derived tool graphs,
provider-lifecycle re-projection, and duplicate artifact/proposal identities no
longer participate in release execution. Workbench reads canonical backend
ViewModels and ships no compatibility route, old page, or dev harness.

All Rust and frontend gates, browser-shell tests, production absence checks,
release and QA exact-signature checks, and an isolated no-network QA startup
passed. The isolated profile opened current canonical stores with a private
`0600` profile-secret file. No external-live rerun was needed because H6 did not
change the H5 provider or Web adapter behavior.
