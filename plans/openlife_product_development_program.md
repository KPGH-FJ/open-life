# OpenLife Reconstruction Plan

Status: active

## Objective

Reconstruct OpenLife as a capable local-first personal Agent for general
knowledge work. Preserve proven execution and safety assets, replace incorrect
or duplicated lifecycle ownership, and delete each retired production path as
its complete replacement lands.

This plan supersedes the S0-S7 program. Those stages remain historical Git
evidence; they are not current product-completion credit.

## Product contract

- The Workbench contains Projects and Conversations. A Conversation can use
  Chat for a direct response or Work for a durable outcome.
- Chat and Work share one canonical `Conversation -> Turn -> Item` spine.
- Work adds `Task -> Run -> Item -> ItemAttempt`, a canonical `FinalResult`,
  and optional independent `ArtifactVersion` objects.
- Planning, adaptive tool use, approval, recovery, and future subagents are
  phases or capabilities inside that spine, never independent product owners.
- The user-selected provider and model are bound to a Turn or Run and are not
  silently substituted.
- Memory and LifeModel participate only through bounded typed ports. They do
  not own execution, permission, or completion.

## Non-negotiable migration rules

1. A production concern has one canonical writer and recovery owner.
2. Temporary adapters may exist only while the production write owner remains
   unambiguous. There is no legacy runtime fallback.
3. Every migrated capability includes its backend owner, control and recovery
   path, ViewModel, usable frontend, behavior tests, and old-path deletion.
4. A schema, plan, proposal, streaming response, green unit test, or stable
   process launch is not product completion.
5. SQLite owns lifecycle and recovery metadata. Artifact files own their
   content. JSONL is diagnostic or export material only.
6. Missing, stale, failed, or uncertain effects remain unknown or blocked.

## Completed stage: R0 - reconstruction baseline

### Outcome

Create a truthful, reproducible baseline from which the product can be rebuilt
without carrying S7 completion claims or unstable native identity forward.

### In scope

1. Replace S0-S7 as active authority with this plan and the accepted
   reconstruction ADR.
2. Align `PRODUCT.md`, architecture, and testing documentation with the
   accepted Conversation/Task model and reduced product surfaces.
3. Establish the stable `ai.openlife.desktop` macOS bundle identifier and explicit signing identity
   contract for exact-native QA; mechanically verify the signed bundle and
   Keychain round trip.
4. Make a fresh profile initialize required internal integrity credentials
   without an approval ritual. Existing unreadable or missing credentials stay
   typed recovery states and never rotate over protected data.
5. Back up the bounded user-owned configuration and personal-intelligence
   subset, then remove authorized legacy execution/test data without reading or
   exporting secret values.
6. Inventory reusable assets and the production consumers that must be
   migrated or deleted in R1-R7.
7. Establish the reconstruction behavior matrix and cost/performance baseline.

### Out of scope

- R1 Conversation schema or the new general Work runtime;
- new tools, connectors, Computer Use, arbitrary shell, scheduling, cloud
  execution, account sync, or broader LifeModel learning;
- Developer ID distribution, notarization, or public release;
- migrating historical TaskSession, AgentRun, ActionQueue, EventStream,
  PlanExecute, report Task, or test Proposal records.

### Acceptance matrix

| Scenario | Required result | Evidence |
| --- | --- | --- |
| Fresh signed profile | Internal keys initialize, stores open, Workbench reaches a usable empty state | exact signed bundle, fresh reconstruction profile, bounded reset of product-owned internal keys, restart |
| Existing accessible profile | Exact signing identity can read its previously created internal keys after restart | exact-native round trip |
| Existing key unavailable | Only affected capabilities are blocked and recovery is explicit; no key rotation or false provider warning | integration and native recovery test |
| Clean break | Legacy execution/test data is absent from the active profile; retained settings and personal intelligence are enumerated | dry-run manifest, backup, post-clean inspection |
| Authority | Product, ADR, architecture, tests, and this plan agree that reconstruction is active | documentation review and source guards |
| Repository | No secrets, generated bundles, or unrelated changes enter the commit | diff review and common checks |

### Checks

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
OPENLIFE_CODESIGN_IDENTITY="<local signing identity>" \
  scripts/macos-exact-native.zsh
```

### Stop condition

R0 is complete only when the exact signed application passes bundle identity,
resource seal, fresh-profile credential initialization, restart access,
and usable-empty-state checks; the authorized clean break is complete; the
repository is clean; and the current plan points to R1.

### R0 result

Completed on 2026-08-13:

- ADR 0018 and this plan replaced S0-S7 as current product authority;
- macOS bundle/runtime/data identity converged on `ai.openlife.desktop` and the
  exact local-signed bundle passed strict signature and resource-seal checks;
- a provably empty profile now initializes OpenLife-owned internal credentials
  automatically, while existing, invalid, or unavailable authority remains
  fail-closed;
- authorized legacy execution and QA data were removed, while sanitized
  configuration, Memory, and LifeModel were retained through a private backup;
- exact-binary first start and restart opened every protected execution owner
  with no observed network socket; and
- full Rust, frontend, production-build, and browser-shell gates passed.

R0 does not claim Developer ID/notarization or live-provider evidence. Local
self-signed Keychain ACL continuity across a newly rebuilt binary is not stable
product proof and remains part of R8 release identity work.

## Reconstruction sequence

| Stage | Complete vertical outcome |
| --- | --- |
| R0 | Stable native identity, Keychain and clean reconstruction baseline |
| R1 | Canonical Conversation/Turn/Item, Provider Registry, and reliable Chat |
| R2 | General Task/Run/ItemAttempt, Goal, control, recovery, and FinalResult |
| R3 | Production document, Web, citation, Skill, and MCP capability loop |
| R4 | Artifact versions, Changes, Preview, Verification, approval, receipts, Undo, and effect reconciliation |
| R5 | Project scope, restart freshness, controlled concurrency, background work, and notifications |
| R6 | Bounded Memory and LifeModel ports with independent evolution proof |
| R7 | Final Workbench, conversation organization, results, onboarding, diagnostics, i18n, accessibility, and old frontend deletion |
| R8 | Golden behavior, performance/cost, profile migration, exact-native/live evidence, absence guards, and clean release baseline |

## Completed stage: R1 - canonical Conversation and reliable Chat

### Outcome

One canonical Conversation/Turn/Item lifecycle owns ordinary Chat from the
Workbench request through durable history and recovery. Chat binds the exact
user-selected provider/model and never creates Task, Run, Proposal, or an
algorithm-specific product object.

### In scope

1. Add a single SQLite Conversation owner with Conversation, Turn, and typed
   Item tables, exact idempotency, terminal transitions, deletion, and restart
   recovery.
2. Add an explicit Chat/Work request mode. R1 migrates Chat; Work remains an
   explicitly separate current consumer until R2, never a fallback from Chat.
3. Bind every Chat Turn to the selected provider profile, model, endpoint
   class, and configuration generation. Same-provider retries may be bounded;
   model/provider substitution is forbidden.
4. Move ordinary Chat send/stream, history, conversation CRUD, selected Skill,
   cancellation, and failure recovery to the canonical owner.
5. Project one backend Conversation ViewModel for the Workbench and remove its
   dependence on Tasks/Review health.
6. Delete Chat use of MemoryStore message/session tables and Chat use of
   TaskSession, AgentRun, ActionQueue, and MainChatEvent presentation state.
7. Add absence guards, product tests, browser tests, exact-native first Chat,
   restart history, cancellation, and unavailable-provider evidence.

### Out of scope

- durable Work Task/Run/ItemAttempt and report migration (R2-R4);
- multiple simultaneous active Tasks, background work, and notifications;
- new connectors, Computer Use, shell, mail/calendar writes, or broader Memory
  and LifeModel behavior;
- importing the authorized legacy test conversations deleted at R0.

### Acceptance

- A new Chat Conversation survives restart with ordered user/assistant Items
  and no Task, Run, Proposal, or legacy Chat lifecycle row.
- A Turn is exactly-once by caller UUID; payload drift and invalid transitions
  fail closed.
- The terminal assistant Item and Turn completion commit atomically. A crash or
  provider failure leaves a typed recoverable/failed state, never a fake reply.
- Buffered and streaming delivery use the same canonical runtime and the UI
  re-reads backend history after terminal delivery.
- Provider/model shown for the Turn matches the adapter receipt; no silent
  substitution or cross-provider fallback occurs.
- Empty or degraded Task/Review stores cannot hide or disable ordinary Chat.
- Replaced Chat writers, read models, IPC fields, and frontend consumers are
  absent from release source.

### R1 result

Implemented on 2026-08-13:

- `ConversationStore` is the sole ordinary Chat lifecycle owner for exact
  Conversation, Turn, and ordered user/assistant Item persistence;
- buffered and streaming Chat converge on `CanonicalChatRuntime`, which binds
  the Settings-selected provider/model/profile generation, performs bounded
  generation, and commits terminal state without TaskSession, AgentRun,
  ActionQueue, durable Main Chat Event, or Proposal rows;
- exact retries replay the committed assistant Item without another provider
  request, while payload drift, unknown Conversation identity, unavailable
  providers, cancellation, late replies, and restart-interrupted Turns remain
  typed non-success states;
- one backend `ConversationViewModel` owns conversation list, selected history,
  latest Turn, provider availability, exact selected model, and Work
  availability independently of Tasks and Review health;
- the production Workbench no longer calls the retired Chat list/history/Life
  Model influence IPCs or loads Work tools/Markdown Memory for ordinary Chat;
  those retired release IPCs have been deleted;
- Work is shown as reconstructing and cannot silently enter the legacy runtime;
  compatibility-only tests remain explicit migration evidence for R2; and
- system diagnostics count canonical Conversations rather than legacy
  `MemoryStore` chat sessions.

The exact current macOS bundle was rebuilt with bundle identifier
`ai.openlife.desktop`, signed by `OpenLife Local Code Signing`, and passed
strict deep resource-seal verification. Core, Tauri, frontend, production
build, and browser-shell gates are green. Interactive native first-Chat,
restart-history, and cancellation review remains explicitly deferred to the R8
golden native matrix because the current Computer Use host resolves the new
bundle to the retired `ai.openlife.app` accessibility target and cannot attach
to its window. That tooling limitation is not credited as product evidence.

R1 does not claim that Work is complete. R2 must replace the explicit
compatibility Work runtime before the UI marks Work ready.

## R1 entry condition

R1 starts only after R0 is committed and its native evidence is current for the
exact source. R1 must migrate ordinary Chat completely before deleting its old
session/event/presentation consumers; it must not add a parallel Chat runtime.

## Completed stage: R2 - general Work Task runtime

### Outcome

One canonical Task/Run/Item/ItemAttempt/FinalResult lifecycle owns every Work
request. A Conversation may contain many independent Tasks; a Task may span
multiple Runs and Turns. Planning is an Item, execution strategy is internal,
and report/plan are capability outcomes rather than separate lifecycle owners.

### In scope

1. Generalize the existing `CanonicalTaskRuntimeStore`; do not create a second
   task database or runtime owner.
2. Remove the one-Conversation/one-Task and report/plan-only schema constraints.
3. Add typed Task and Run terminal states, Item attempts, exact provider/tool
   bindings, FinalResult references, cancellation, interruption, retry, and
   restart recovery.
4. Add one Work coordinator that starts from the canonical Conversation user
   Item, creates the Task before execution, and commits a truthful terminal
   result without using TaskSession, AgentRun, ActionQueue, or durable Main Chat
   Event as lifecycle owners.
5. Project Tasks and active Work state directly from canonical snapshots and
   expose Work only when this runtime is usable.
6. Delete migrated release Work entrypoints and compatibility state; retained
   report/tool materializers may consume canonical Items but may not own Task
   status.

### Out of scope

- broad document/Web/Skill/MCP tool execution, which is R3;
- Artifact materialization and inline approval migration, which is R4;
- background concurrency and notifications, which are R5; and
- UI polish beyond the minimum Work task/result/control surface, which is R7.

### Acceptance

- two Tasks can exist in one Conversation without identity conflict;
- one Task can own multiple Runs and each Run can own ordered Items and
  attempts without duplicated lifecycle state;
- exact replay returns the same terminal result without another provider call;
- cancel, retry, crash recovery, provider failure, and effect-unknown are typed
  terminal or resumable states and never remain falsely running;
- a completed Task has one canonical FinalResult bound to its Run and exact
  Conversation assistant Item; a failed/blocked/cancelled Task cannot claim one;
- Tasks/Workspace ViewModels do not infer current state from TaskSession,
  AgentRun, ActionQueue, or event-string overlays; and
- release Work send/stream/control paths contain no compatibility fallback to
  the pre-reconstruction runtime.

### R2 result

Completed on 2026-08-13:

- `CanonicalTaskRuntimeStore` schema v7 now owns general `work`, `report`, and
  `plan` Task identity without a Conversation uniqueness constraint, with
  typed Task/Run terminal states, ordered Items, ItemAttempts, and one bounded
  FinalResult reference;
- `CanonicalWorkRuntime` begins the exact Conversation Turn and Work Task/Run
  before provider execution, binds the user-selected provider/model to a
  ProviderGeneration ItemAttempt, and completes only after the assistant Item
  and FinalResult agree;
- exact replay returns the existing assistant Item and FinalResult without a
  second provider dispatch, while provider failure, cancellation, interrupted
  startup recovery, and retry retain typed non-success history and cannot
  claim completion;
- one Conversation can retain multiple Tasks, and retry creates a new Run and
  Turn for the same Task; planning is represented as an Item rather than a
  separate PlanExecute lifecycle;
- release Work send, stream, cancel, and retry IPCs use canonical Task/Run/Turn
  identities. The old TaskSession list/detail/refresh/resume/cancel/retry IPCs
  and frontend consumers are removed;
- `TasksViewModel` and `WorkspaceViewModel` now project canonical snapshots
  directly instead of overlaying TaskSession, AgentRun, ActionQueue, or event
  strings; and
- historical capability fixtures use a `cfg(test)`-only executor route so
  R3/R4 evidence remains runnable without creating a release compatibility
  fallback.

Rust format, Clippy, and the full locked test suite passed: core 1460 passed / 2
ignored, scheduler integration 8 passed, Tauri 1210 passed / 13 ignored,
resource worker 2 passed, and doc tests 8 passed. Frontend formatting,
typecheck, 269 Vitest cases, production build/absence guard, and 8 browser-shell
E2E cases passed. The exact current macOS bundle was rebuilt at
`target/release/bundle/macos/OpenLife.app`, retained bundle id
`ai.openlife.desktop`, was signed by `OpenLife Local Code Signing`, and passed
strict deep resource-seal verification. External-live provider evidence and
interactive native Work behavior remain R8 evidence and are not claimed here.

R2 does not claim document/Web/Skill/MCP execution, Artifact approval or
materialization, controlled concurrency, or final Workbench UX. Those migrate
in R3-R7 through the same canonical runtime.

## Completed stage: R3 - production knowledge-work capability loop

### Outcome

Move the proven document, Web, citation, Skill, and MCP read capabilities into
the general Work coordinator so they execute as canonical Items and Attempts
inside the same Task/Run lifecycle. Delete each migrated capability's use of
the pre-reconstruction Work runtime instead of wrapping or dual-writing it.

### Entry condition

R3 begins only from the committed R2 canonical Task baseline. A capability is
not migrated until its policy grant, execution receipt, failure/replay path,
canonical read model, usable Workbench behavior, and old-path deletion agree.

### R3 result

Completed on 2026-08-13:

- general Work now executes bounded `document.read`, `web.search`/`web.fetch`,
  selected executable Skill context, and registered read-only MCP tools through
  the production kernel and ToolGateway rather than the retired Work runtime;
- every real tool dispatch is a canonical ToolCall Item and ItemAttempt, every
  successful result adds a digest-only Observation Item, and every real model
  invocation—including a one-shot citation repair—is its own
  ProviderGeneration ItemAttempt;
- the canonical Task store issues owner-bound content receipts for observed
  tool bodies without persisting those bodies in Task lifecycle metadata;
- document reads remain bound to the exact initiating Turn. Task retry creates
  a new Run/Turn while reusing only the prior Run's bounded resource scope;
  exact replay never redispatches tools or providers;
- failed/blocked/uncertain tools and providers terminalize their exact Attempt
  and Task truth before a FinalResult can exist; forged, absent, or stale
  citations fail closed;
- Tasks and the Workbench now expose the canonical Item timeline for tool,
  observation, provider, plan, and final-result activity; tool recovery reads
  canonical Attempts rather than ActionQueue state; and
- the old OpenLifeTurnRuntime send/stream/evaluation surfaces are test-only,
  while release read-tool resources no longer require AgentRunStore or durable
  Main Chat Events. The retained historical capability suite remains explicit
  compatibility evidence only and cannot become a release fallback.

Rust formatting, Clippy, the full core/Tauri/integration suite, frontend
formatting, typecheck, 269 Vitest cases, production frontend build, and
browser-shell E2E passed. Controlled document, Web, Skill, builtin MCP,
registered stdio MCP, failure, citation-retry, exact-replay, and document-retry
tests passed. R3 does not claim external-live Web/provider/MCP or interactive
native evidence; those remain R8 evidence.

## Completed stage: R4 - governed Artifact and effect lifecycle

R4 is complete in the controlled product boundary:

- generated Markdown/CSV outcomes enter the same general Work Task and Run;
  each ArtifactDraft exists before Review, and ReviewCheckpoint plus the
  waiting materialization Item are canonical Task Items. Bundles prepare every
  draft before the first Review checkpoint pauses the Run, so Markdown and CSV
  cannot split into competing lifecycles;
- approval claims the exact Proposal, starts a receipt-bound materializer
  ItemAttempt, confirms the file effect, writes Verification, and completes the
  same Run/Task only when every current ArtifactVersion is verified;
- the assistant result identity is persisted while Review is pending, so an
  approval after restart creates the same canonical FinalResult rather than a
  second lifecycle;
- effect failure and effect-unknown states terminalize the exact Artifact Item,
  Attempt, Run, and Task without claiming delivery; startup reconciliation can
  finish a confirmed canonical projection without redispatching the effect;
- created files expose governed Undo. Undo gets its own ReviewCheckpoint and
  materializer Attempt, revalidates exact content and scope, moves the file to
  the safe OpenLife trash location, and records `undone` without rewriting the
  original Artifact history. Replacement Undo stays unavailable until original
  bytes are durably captured;
- TasksViewModel owns Changes, Preview, Verification, and Undo presentation;
  React invokes only the typed Undo command and never reads files or stores.
  Final delivery requires the canonical FinalResult record, its exact
  completed Item, and verified Artifact receipts; a successful governed Undo
  preserves that completed history while presenting the later reversal rather
  than degrading the original Task to missing evidence;
- the compatibility kernel now rejects provider-generated Artifacts and cannot
  call the report-only Artifact owner. An absence guard prevents that consumer
  from returning; and
- the obsolete AgentRun tool-evidence projection used only by the retired
  report Artifact path was deleted.

Focused restart, single- and multi-Artifact generation-to-approval,
document-plus-Web-to-Artifact,
rejection, effect receipt, Undo replay, ViewModel, and frontend rendering tests
passed. Full locked gates passed: core 1464 passed / 2 ignored, scheduler 8
passed, Tauri 1211 passed / 25 ignored, resource worker 2 passed, doc tests 8
passed, frontend 271 passed, production build and absence guard passed, and 8
browser-shell E2E cases passed. The additional ignored Tauri cases are labeled
historical pre-reconstruction report-owner evidence and are not current R4
completion credit. These controlled tests do not claim external-live
provider/Web/MCP or exact-native product evidence; R8 owns those evidence
levels.

## Completed stage: R5 - Project scope, background Work, and task attention

R5 completed the general Work control and continuity boundary:

- canonical Projects group Conversations and carry an optional workspace root,
  revision, and exact scope digest; every admitted Work Run snapshots that
  immutable Project scope;
- retry compares the prior Run scope with the Conversation's current Project
  and revision. Changed or missing scope fails closed and records a canonical
  `scope_stale` attention fact instead of silently widening access;
- authenticated steering bodies are ordered Conversation Items. The Task store
  retains only exact references and digests, consumes an in-scope adjustment at
  the provider checkpoint, and blocks scope expansion without granting access;
- global Work concurrency is bounded before Turn or Task persistence. Running
  Work can continue while the user changes Conversation, and canonical cancel
  remains available on the exact Task;
- ReviewRequired, Blocked, Failed, EffectUnknown, and ScopeStale are durable
  attention facts projected by `TasksViewModel`; React can filter Needs
  Attention without rebuilding lifecycle truth; and
- the release `accept_proposal_and_continue` and compatibility TaskSession
  control consumer were removed. Confirmed Artifact effects complete the same
  canonical Work identity; the legacy control module is test-only migration
  evidence and has no Tauri handler or frontend caller.

Focused migration, Project freshness, admission, steering, cancellation,
attention, command, hook, and React tests pass. Full locked gates passed: core
1470 passed / 2 ignored, scheduler 8 passed, Tauri 1214 passed / 24 ignored,
resource worker 2 passed, doc tests 8 passed, frontend 273 passed, production
build and absence guard passed, and 8 browser-shell E2E cases passed. R5 claims
controlled source/product evidence only; exact-native and external-live
evidence remain R8.

## Current stage: R6 - bounded Memory and LifeModel ports

R6 will place Agent Memory and LifeModel participation behind narrow typed
ports, prove that either system can evolve or be unavailable without changing
Task/Run/Item/Artifact ownership, and preserve proposal-governed durable writes.

R6 is implemented. Canonical Chat and Work now load optional personalization
through `AgentMemoryContextPort` and `LifeModelContextPort`; neither runtime
reads legacy TaskSession conversation-memory ownership. Optional context
failure degrades explicitly without granting permission or changing Task
completion. Canonical Work applies already policy-authorized suggestions only
through `PersonalIntelligenceSuggestionPort`: explicit low-risk facts use the
reversible Memory gateway without provider invocation or Proposal creation,
while stable preferences create only a LifeModel candidate and leave the
canonical LifeModel unchanged. Both paths record a completed canonical
Observation Item. Suggestion failure terminalizes the Work Task as blocked.

Focused source, port, canonical Chat/Work, classification, failure, and absence
guards pass. Full locked gates passed: core 1470 passed / 2 ignored, scheduler
8 passed, Tauri 1220 passed / 24 ignored, resource worker 2 passed, doc tests 8
passed, frontend 273 passed, production build and absence guard passed, and 8
browser-shell E2E cases passed. R6 claims controlled source/product evidence
only; exact-native and external-live evidence remain R8.

## Completed stage: R7 - final Workbench product surface

Completed on 2026-08-14. The shipped frontend now has three product routes:
Workbench (`/workspace`), Personal Intelligence (`/life-model`), and Settings.
Results and needs-attention items are contexts inside the same Workbench and
retain the canonical backend task/review identities; `/today`, `/tasks`, and
`/review` are explicit retired paths without redirects.

The old Today adapter, Today view model, independent Tasks loader, unavailable
compatibility page, daily-goal display guard, and the misleading
`ReadOnlySpineJourney` product owner were deleted. The production composition
owner is now `ProductWorkbenchJourney`, while the independent provider/privacy
boundary has one narrow data source and cannot inherit a Tasks failure.

First-run empty conversation state now explains Projects, Chat/Work, and
results. The same shell adapts at 860px and 560px without a second mobile route
authority. Icon-only navigation and Settings retain accessible names; the skip
link moves focus to the canonical main region. Settings exposes only the two
implemented categories instead of placeholder sections.

Frontend format, typecheck, 245 behavior tests, production build and absence
guard, and 11 browser-shell E2E scenarios pass. R7 claims controlled browser
evidence only; exact-native and external-live evidence remain R8.

## Current stage: R8 - golden evidence and clean release baseline

R8 will run the final golden behavior matrix, measure performance and provider
cost boundaries, verify profile migration and exact native identity, execute
only the external-live checks required by the accepted product contract, and
remove remaining replaced backend/IPC paths before the clean release baseline.
