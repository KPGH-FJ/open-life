# OpenLife Architecture

## Status

Stable source map for the completed R0-R8 and H0-H6 capable-Agent baselines.
Current source and accepted ADRs remain authoritative; there is no active
implementation plan until the next bounded objective is accepted.

## Current product path

```text
React Workbench
  -> frontend/src/tauri.ts
  -> Tauri commands in src-tauri/src/lib.rs
  -> Rust runtimes and governed command gateways
  -> openlife-core contracts and canonical stores
  -> SQLite, local files, Keychain, providers, and governed external tools
```

Chat and Work have separate transport entrypoints and converge on canonical
Conversation and Task owners:

```text
frontend Conversation ViewModel + Chat composer
  -> main_chat_send.rs | main_chat_streaming.rs
  -> canonical_chat_runtime.rs | canonical_work_runtime.rs
  -> main_chat_kernel.rs
  -> openlife-core/src/agent/main_chat_agent_v1.rs
  -> ConversationStore (Conversation -> Turn -> Item)
  -> CanonicalTaskRuntimeStore (Task -> Run -> Item -> ItemAttempt -> FinalResult)
```

`mode=chat` enters `CanonicalChatRuntime`. `mode=work` requires caller-owned
Task, Run, Turn, and Conversation UUIDs and enters `CanonicalWorkRuntime`.
Neither release branch can fall back to `OpenLifeTurnRuntime`. Historical
capability fixtures use a `cfg(test)`-only route and provide no product credit.
The Workbench reads conversation history from `ConversationViewModel` and Work
lifecycle state from backend canonical Task snapshots.

Other product commands, including Review acceptance, Settings persistence, and
direct Memory controls, use their command-specific gateways rather than passing
through `OpenLifeTurnRuntime`.

Canonical Chat owns exact UUID idempotency, atomic user/assistant Items,
terminal transitions, cancellation, failed-provider state, restart
interruption, and immutable provider profile/model/configuration generation for
each Turn. It never creates a Task. Canonical Work begins the Conversation Turn
and Work Task before provider execution, records the provider Item and exact
ItemAttempt, and completes with one FinalResult bound to the exact assistant
Item. Cancellation, interruption, failure, retry, and replay terminalize the
same Task/Run owner; retry creates a new Run and Turn for the same Task.
Production document, Web, selected Skill, and registered read-only MCP work
now executes inside this general coordinator. Each real tool or provider
invocation has its own canonical ItemAttempt; successful tool output adds a
digest-only Observation Item. Citation repair is a second provider Attempt,
not a rewrite of the first attempt. Document retry can reuse only the failed
Run's exact Turn-bound resource scope.

One Conversation may retain multiple Tasks. Planning is a typed Item inside a
Run, never a separate session or strategy-owned lifecycle. Release Work uses
only the canonical Conversation and Task runtime stores.

General Work owns stable Task identity, Run membership, typed
instruction/plan/tool/observation/provider-generation/artifact/review/
materialization/verification/final-result Items, ItemAttempts, and independent
ArtifactVersion metadata in `task_runtime.db`. A Work Task and Run begin before
governed reads or provider work. Completed tool and provider receipts append in
the same Run before ArtifactDraft. The canonical store records identities and
bounded digests, not prompt or response bodies. Draft bytes live in an atomic,
digest-bound file beside `task_runtime.db`; the exact target, draft reference,
target precondition, provenance Item, and content digest belong to the numbered
ArtifactVersion. An Artifact exists before its Proposal. Review is a
version-bound checkpoint relation, approval starts a materializer ItemAttempt,
and confirmed materialization updates that same ArtifactVersion. Proposal id is
not an Artifact field or identity component.

Each Work Run also owns one schema-validated structured plan in
`canonical_work_plans`. The stored plan contains only bounded step ids, typed
capability phases, dependencies, completion requirements, and an immutable Run
budget snapshot; it never stores user text, URLs, file names, secrets, or tool
payloads. Model output may propose this structure, but Policy bounds the
eligible step kinds and the runtime revalidates every adapter invocation. The
same canonical Item/ItemAttempt rows reconstruct consumed budget after restart.
`WorkItemScheduler`, the canonical capability executor, and
`WorkCompletionEvaluator` operate inside this Run rather than creating a plan
session, queue, or second lifecycle owner. A FinalResult cannot be committed
while any canonical non-final Item is non-terminal or while a required plan
step lacks successful evidence.

The current `openlife.work-plan.v2` contract distinguishes imported-document,
workspace-file, Web Search, Web Fetch, selected Skill, and registered read-only
MCP capability phases. Fixed capabilities cannot carry model-authored targets.
An MCP step must select an exact policy-bounded manifest id; the runtime adds
the current execution-contract digest after parsing and rechecks it immediately
before ToolGateway dispatch. Executable arguments remain backend-derived from
the authenticated Task, Project, resource, and user-goal scope.

One observation-driven plan revision is permitted only when every earlier tool
attempt succeeded but its evidence cannot satisfy a bounded citation contract.
The replacement plan stays in the same Run, cannot repeat an already completed
execution capability, cannot widen the registered target set, and inherits the
original budget. Every admitted revision is retained in
`canonical_work_plan_revisions`; failed, cancelled, blocked, or effect-unknown
attempts are terminal and can never be hidden by replanning. Release Work does
not compile a strategy-owned execution branch.
While Review is pending, the exact assistant Conversation Item identity is
stored as a deferred result relation. Approval can therefore complete the same
FinalResult after restart without inventing a second Task owner.
Policy can authorize the bounded production `document.read` capability for
resources already imported and bound to the current message/task operation.
The kernel executes `document.read` before `web.search`/`web.fetch` when both
are requested, and each successful read becomes an ordered canonical ToolCall
and Observation Item before ProviderGeneration. The document ToolCall uses the
real `ResourceStore` and deterministic selector; its durable metadata contains
selection identity, digest, and count, never document bodies. Provider context
is reselected from the same bound resources for the actual provider request,
must match the ToolCall selection digest, and receives newly issued
request-scoped citations. Local-resource and Web citation authorities validate
model output before a Work ArtifactDraft or ReviewCheckpoint can exist.
Missing/failed reads or a failed one-shot citation repair therefore stop before
provider-backed completion and durable effects. For document reads, durable
replay metadata contains only the selected chunk count and stable selection
digest. Restart replay reselects the exact task-bound ResourceStore content and
must reproduce that digest before provider synthesis; it does not redispatch the
ToolGateway read or persist a document-body preview.

It also contains typed Steering Items and a monotonic plan
revision. Workspace submits authenticated steering while work is active.
Conversation remains the only body owner; `CanonicalTaskRuntimeStore` keeps the
exact message reference and digests. One pending in-scope Steering Item is
consumed exactly once before the Work provider-generation checkpoint and survives
restart. Steering that asks for a new workspace, provider, network route, tool,
destructive action, or other scope is recorded blocked and grants nothing. The
process bounds independent Work executions before any message or Task
persistence, while the cancellation registry retains one execution owner per
Task. Once provider generation has started, late steering is rejected instead
of being displayed as if it could still change that Run.

Projects are canonical Conversation groupings with an optional workspace root,
monotonic revision, and exact scope digest. A Work Run snapshots Project id,
revision, and digest at admission. Moving a Conversation or changing that scope
does not rewrite historical Runs; retry fails closed with a `scope_stale`
attention fact until the user starts work under the current scope. Multiple
Work Runs can continue in the background up to the bounded global admission
limit while the user opens another Conversation.

Review approval addresses the exact Proposal and effect. For canonical Work
Artifacts, confirmed materialization projects back into the same Task, Run,
ArtifactVersion, and FinalResult. There is no proposal-owned continuation or
compatibility resume command.
Approval, effect confirmation, and canonical completion remain separate facts
even though the product can present one decision.

The backend-owned `TasksViewModel` and `WorkspaceViewModel` now read canonical
Task snapshots directly. They project Work and report Run membership, typed
Items, attempts, FinalResult, Artifact versions, Review wait, rejection,
verified delivery, and effect-unknown states. They do not overlay another
execution store. Current Work controls use canonical cancel and retry IPCs.
Unresolved review, blocked, failed, effect-unknown, and stale-scope facts are
projected as backend-owned Needs Attention state rather than inferred by React.
`WorkspaceViewModel` accepts the selected Conversation identity and returns only
that Conversation's Tasks and related Review checkpoints. Each Task projection
also carries the current structured Work plan, completion requirement, and
immutable budget policy from `canonical_work_plans`; React does not reconstruct
them from messages or Item labels.

The canonical Artifact path adds backend-owned Result, Change, Preview,
Verification, and Undo projections to each Work ArtifactVersion. A pending
preview is admitted only from that version's managed draft after regular-file,
size, UTF-8, store-root, and content-digest checks; Proposal payload is not a
content or preview owner. A materialized preview is reread only from a regular
file inside the configured safe paths and is shown only when its current byte
digest, stored observed digest, and canonical Verification Item agree. File
drift, disappearance, symlinks, oversize content, or non-UTF-8 bytes remove the
preview and prevent the Task from retaining delivered product credit. React
renders these typed projections; it does not read files, proposals, or
`task_runtime.db` itself.

The canonical Artifact effect journal binds the exact ArtifactVersion,
materializer ItemAttempt, Review dispatch claim, target digest, content digest,
and physical effect state. `prepared`, `staged`, `confirmed`,
`failed_before_effect`, and `effect_unknown` survive restart. Recovery inspects
the exact filesystem facts, never blindly redispatches an ambiguous effect, and
repairs a confirmed-but-unprojected Review decision without creating a
ProposalStore Artifact record. A confirmed governed Undo is a version-bound,
independently receipted move: it preserves the original verified Task history
and is presented as a later reversal instead of missing delivery.

## Capable-Agent target

ADR 0019 extends the reconstructed Conversation spine with one adaptive Agent
loop and clean replacement contract:

```text
Workbench -> optional Project -> Conversation -> Turn -> typed Item
                                           \-> optional Work Task
                                               -> Run -> PlanRevision
                                               -> Item -> ItemAttempt
                                               -> Observation
                                               -> CompletionEvaluator
                                               -> FinalResult
                                               -> ArtifactVersion
```

A direct Chat response does not create a Task. A durable Work outcome does.
Conversation and Task are not aliases, and the target schema must allow a
Conversation to retain multiple historical Tasks without allowing multiple
unrelated active outcomes to blur the user experience.

### Retained assets and migration consumers

The reconstruction keeps proven provider adapters and receipts, ToolGateway
contracts, production document/Web/Skill/MCP reads, ReviewWorkflow,
materializers, effect certainty, cancellation fences, outbox recovery, backend
ViewModels, and the Work Artifact/Changes/Preview/Verification/Undo
implementation.
Memory and LifeModel remain retained stores behind the narrow typed ports in
`src-tauri/src/personal_intelligence_ports.rs`.

The canonical runtime has no compatibility lifecycle owner:

- generated Artifact effects are accepted only when they carry canonical Work
  Task/Run/Item/Artifact identity;
- no parallel execution, plan, action-queue, or event-lifecycle package is
  reachable from release; planning is an Item inside canonical Work; and
- Today, Tasks, and Review are retired product routes. Their backend facts are
  presented only inside the Conversation Workbench or an exact domain
  checkpoint opened from Personal Intelligence or Settings.

Adding another compatibility lifecycle store or restoring a retired route is
forbidden.

The Work lifecycle remains:

```text
Task
  -> Run
    -> typed Item
      -> ItemAttempt / ReviewCheckpoint / ArtifactVersion
```

```text
Frontend -> backend ViewModels
IPC -> TaskRuntime / RunCoordinator
    -> PolicyAuthorization
    -> Planner / ItemScheduler
    -> ItemExecutor
       -> ProviderGateway | ToolGateway | ReviewCheckpoint
       -> ArtifactMaterializer | MemoryPort | LifeModelPort
    -> canonical Item and Run events
    -> projections and ViewModels
```

Planning and adaptive tool use are internal orchestration policies. They do not
own separate task identities, terminal states, or recovery paths. Review pauses
and later resumes the same Item. A production concern has one write owner;
migration must not use dual writes or silently fall back to a retired runtime.

## Product surfaces

The shipped product routes are:

```text
/workspace
/life-model
/settings
```

Product read state comes from `LifeStateProjection`, backend ViewModels, and
strict frontend adapters that compose named backend owner outputs. Governed
writes pass through proposal, permission, persistence, and target-domain owners
rather than page-local state.

R7 reduced the shipped top-level surfaces to Workbench (`/workspace`),
Personal Intelligence (`/life-model`), and Settings. Task and
approval status are presented in their Conversation and a Needs Attention
filter rather than duplicate top-level Task and Review products.

## Domain ownership

The current boundary is defined by ADR 0016:

- Conversation and the canonical Work runtime own Chat and Work execution;
- Agent Memory owns working, project, episodic, semantic, procedural, and
  Reflection context;
- LifeModel owns confirmed long-term understanding of the user;
- domain stores own task, transient state, calendar, email, and other business
  facts;
- safety and governance own permissions, privacy, review, and write admission.

Evidence and proposals connect these domains without becoming another fact
owner. `AppState` is the process composition root, and
`PersistenceCoordinator` owns store health and effect admission; neither is a
single product-truth owner.

Main Chat may read bounded confirmed LifeModel v2 facts and eligible Agent
Memory. Safety, capability, risk, permission, and the current user instruction
remain higher-precedence authorities. Personalization cannot grant a tool,
reveal a credential, approve a durable write, or declare completion.

## Completion boundary

```text
authorization
  != execution
  != canonical persistence
  != product completion
```

Provider text is not write authorization. Proposal acceptance is not by itself
materialization. Streaming tokens are not terminal evidence. Product completion
requires the relevant canonical owner and terminal evidence, followed by a
refreshed product read model where one exists.

## Artifacts and persistence target

- SQLite is the canonical recovery authority for Task, Run, Item, attempt,
  approval, scope, receipt, and artifact metadata.
- Artifact files remain the authority for their actual content. The database
  stores references, digests, versions, and relationships rather than duplicate
  file bodies.
- JSONL may be used for diagnostics or export, but not as a second recovery
  authority.
- Backend ViewModels are rebuildable projections and the only product-facing
  composition surface when a ViewModel exists.

Credential storage follows the build identity rather than sharing a global
development bucket. Release uses the OS credential store. Dev and QA use
separate app identities, data directories, and atomic `0600` local profile
secret files; they never import release Provider/Search credentials
automatically. This avoids treating changing self-signed development CDHashes
as release update behavior. It is not the distributed-release design: a future
Developer ID build must continue to prove stable Keychain access across signed
updates.

For release Work, `CanonicalTaskRuntimeStore` is the Task, Run, Item,
ItemAttempt, terminal-state, recovery, and FinalResult owner. Provider and tool
adapters retain bounded receipts, but cannot declare Task completion. The
historical report slice remains reusable migration evidence for R3/R4, not a
fallback: each capability must move into the general coordinator before its
legacy execution consumer is deleted.

## Personal intelligence boundary

Agent Memory and LifeModel participate through narrow context, learning, and
materialization ports. They do not own Task state, permission, execution
strategy, artifacts, or completion. A later LifeModel implementation change,
including possible AI-assisted maintenance, must not require rewriting the
Agent harness.

Canonical Chat and Work load bounded optional context only through
`AgentMemoryContextPort` and `LifeModelContextPort`. Port failure contributes a
typed degraded context marker rather than taking over Task state. Canonical
Work applies an already policy-authorized explicit suggestion through
`PersonalIntelligenceSuggestionPort`: low-risk explicit facts may use the
existing reversible Memory gateway, while stable LifeModel preferences create
only a reviewable learning candidate. Each successful suggestion is projected
as a completed canonical Observation Item. The port does not select the route,
grant capability, create an Artifact, or terminalize a Task.

## Source maps

- [Agent runtime](architecture/agent-runtime.md)
- [LifeModel](architecture/life-model.md)
- [Governance](architecture/governance.md)
- [Agent Memory](architecture/memory.md)
- [Testing](development/testing.md)
- [Accepted decisions](../plans/adr/)

These documents explain source. They do not override runtime code or accepted
ADRs.
