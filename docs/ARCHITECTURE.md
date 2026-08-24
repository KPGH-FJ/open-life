# OpenLife Architecture

## Status

Stable source map for the current capable-Agent baseline. Current source,
accepted ADRs, and the one active implementation plan remain authoritative.
The path below describes production reachability today; it does not promote
every reached module to target architecture.

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
  -> provider_runtime.rs | provider_client.rs
  -> ToolGateway | ReviewWorkflow | personal-intelligence ports
  -> ConversationStore (Conversation -> Turn -> Item)
  -> CanonicalTaskRuntimeStore (Task -> Run -> Item -> ItemAttempt -> FinalResult)
```

The model authors schema-validated Chat or Work steps. Runtime code enforces
eligible capability kinds, exact arguments, Project/resource scope, privacy,
permissions, budgets, receipts, completion evidence, and durable effects. The
retired keyword router and Main Chat Kernel are absent from the source tree and
cannot participate through a compatibility fallback.

`mode=chat` enters `CanonicalChatRuntime`. `mode=work` requires caller-owned
Task, Run, Turn, and Conversation UUIDs and enters `CanonicalWorkRuntime`.
Neither release branch can fall back to `OpenLifeTurnRuntime`. Historical
capability fixtures use a `cfg(test)`-only route and provide no product credit.
The Workbench enters through one backend-composed `WorkbenchViewModel` snapshot
that binds Conversation, scoped Workspace, Tasks, Review, and Provider Boundary
lanes at one capture point.

Other product commands, including Review acceptance, Settings persistence, and
direct Memory controls, use their command-specific gateways rather than passing
through `OpenLifeTurnRuntime`.

Canonical Chat owns exact UUID idempotency, atomic user/assistant Items,
terminal transitions, cancellation, failed-provider state, restart
interruption, and immutable provider profile/model/reasoning effort/configuration generation for
each Turn. It never creates a Task. Canonical Work begins the Conversation Turn
and Work Task before provider execution, records the provider Item and exact
provider/model/reasoning-bound ItemAttempt, and completes with one FinalResult bound to the exact assistant
Item. Stopping, interruption, failure, retry, and replay stay with the same
Task/Run owner. Stop terminalizes the current Run as cancelled. Retry after a
failed or blocked Run and Continue after a cancelled or interrupted Run both
create a new immutable Run and Turn for the same Task; neither resurrects the
provider request or tool call from the prior Run.
Each Run additionally persists an immutable execution ceiling. Scoped-agent
mode and observe-only mode restrict the shared Work capability set; they are
not ToolPermissionStore grants. Observe-only removes Artifact and
personal-intelligence writes at provider schema, plan validation, and direct
step execution boundaries, and retry preserves that ceiling.
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
before ToolGateway dispatch. The selected model proposes executable arguments
just in time from the authenticated request and prior bounded observations;
the runtime validates their schema and binds them to Task, Project, resource,
network, and tool scope before dispatch.

A citation-shape failure after successful reads does not replace or revise the
plan. The same Run retains the completed ToolCall/Observation Items and asks
for one observation-bound terminal `AgentStep`; only the requested answer or
Artifact kind is accepted. Failed, cancelled, blocked, or effect-unknown tool
attempts remain terminal and cannot be hidden by another plan. Release Work
does not compile a strategy-owned execution branch.
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

It also contains typed Steering Items and a monotonic plan revision. Workspace
submits authenticated steering while work is active. Conversation remains the
only body owner; `CanonicalTaskRuntimeStore` keeps the exact message reference
and digests plus pending/applied/rejected/blocked resolution facts. Canonical
Work consumes one pending item at safe checkpoints after planning, between
governed tool steps, inside the observation-bound Web loop, and before terminal
generation. The selected provider authors a revised typed plan, while the host
restricts it to the Run's already admitted capability kinds and exact MCP
targets. Adding a new capability or target is therefore a typed scope-diff
failure, not a keyword judgment. Applying a revision atomically advances the
Run and plan history and records the applied revision; rejected or blocked
Steering leaves the current plan authoritative and grants nothing. Completed
steps cannot be rewritten. The process bounds independent Work executions
before any message or Task persistence, while the cancellation registry retains
one execution owner per Task. Once terminal provider generation has started,
late steering is rejected instead of being displayed as if it could still
change that Run.

Projects are canonical Conversation groupings with an optional primary workspace
root, up to eight explicitly selected secondary read roots, active/archived
lifecycle state, monotonic revision, and exact scope digest. Adding or removing
a secondary root advances the Project revision and changes that digest. A Work
Run snapshots Project id, revision, and digest at admission. Moving a
Conversation or changing that scope does not rewrite historical Runs; retry
fails closed with a `scope_stale` attention fact until the user starts work
under the current scope. An archived Project cannot admit new Conversations or
Work. Archive is reversible and requires no active linked Conversation. Restore
creates a new revision. Existing active Projects can be selected durably as the
scope of a not-yet-created Conversation; first send binds that selected scope
and the selected Conversation Memory mode in one atomic Conversation admission
before Turn/Run admission. An archived or unknown Project rejects the whole
creation instead of leaving a partially scoped Conversation. Permanent
deletion is metadata-only, requires native confirmation, and fails closed
unless the Project is archived and has no
Conversation, Task, or Run reference. The backend lifecycle ViewModel merges
those two store-owned reference facts and exposes exact allowed controls and
blockers; React does not infer deletion safety. Multiple Work Runs can continue
in the background up to the bounded global admission limit while the user opens
another Conversation.

The native folder picker canonicalizes every Project root at the platform edge.
Production workspace-file resolution accepts only the admitted Run's exact
Project read-root set and never falls back to process cwd. The model receives
bounded root ids and names, selects at most one root per `file.read`, and supplies
a root-relative path; the runtime resolves and authorizes only that selected
root. The primary root remains the default when `rootId` is omitted. Project
secondary roots are read-only and never become Artifact destinations.
Filesystem configuration has separate typed fields:
`artifact_output_directory` authorizes reviewed non-Project exports, while the
global `additional_read_roots` can widen only generic read tools. Neither field
creates or changes Project scope, and editable Settings JSON cannot mutate
either authority. Legacy `safe_paths` is read once as the former UI's Artifact
destination and is not retained as generic read permission. Canonical Work
Artifacts use only the exact Run's primary Project root or app-managed storage,
never a Project secondary root or either global setting. The composer reads the backend
provider-profile registry, can select a discovered local model, and passes the
exact profile id into Turn/Run admission. A configured cloud route remains
unverified, stale, offline, or degraded until its authenticated validation
receipt proves readiness; credential presence alone is not enough.

Review approval addresses the exact Proposal and effect. For canonical Work
Artifacts, confirmed materialization projects back into the same Task, Run,
ArtifactVersion, and FinalResult. There is no proposal-owned continuation or
compatibility resume command. Canonical Continue is an explicit new-Run
command owned by Work itself. Tool permission Review is a narrower same-Run
checkpoint: TaskRuntime schema v23 binds the Proposal to the exact ToolCall
Item, step, executor Action, and scope digest. A live approval creates the next
Attempt on that same Item and Run; rejection blocks it. If the process-level
wait owner is gone after restart, approval preserves the permission fact but
marks the Run interrupted so a lost adapter context is never presented as
resumed.
Approval, effect confirmation, and canonical completion remain separate facts
even though the product can present one decision.

The backend-owned `WorkbenchViewModel` composes Conversation, scoped Workspace,
Tasks, Review, and Provider Boundary lanes without creating another store. Its
Tasks and Workspace projections read canonical Task snapshots directly. They
project Work and report Run membership, typed
Items, attempts, FinalResult, Artifact versions, Review wait, rejection,
verified delivery, and effect-unknown states. They do not overlay another
execution store.

The Workbench global Activity surface reads the unscoped canonical Tasks lane;
selecting an entry navigates to its exact Conversation and Task rather than
copying lifecycle state into frontend storage. Conversation search and
active/archived presentation remain ConversationStore projections, while Task
reference blockers come from CanonicalTaskRuntimeStore.

Current Work controls use three status-specific canonical
IPCs: Stop current Run while running, Retry into a new Run after failure or
blocking, and Continue into a new Run after cancellation or interruption. Each
control carries the exact latest Run id, and the backend rejects historical Run
targets. Pause and outcome-level Cancel Task are not advertised: the current
runtime has no durable safe-checkpoint pause owner or separate task-abandonment
disposition, so those labels would overstate the lifecycle contract.
Unresolved review, blocked, failed, effect-unknown, and stale-scope facts are
projected as backend-owned Needs Attention state rather than inferred by React.
The scoped Workspace lane accepts the selected Conversation identity and
returns only that Conversation's Tasks and related Review checkpoints. Each Task projection
also carries the current structured Work plan, completion requirement, and
immutable budget policy from `canonical_work_plans`; React does not reconstruct
them from messages or Item labels. It also carries metadata-safe provenance for
the latest Run: the exact provider/model-bound Conversation Turn and its status,
plus the immutable Project id, name, revision, and scope digest. Historical
Tasks therefore do not inherit current Settings or a later Project revision.
The Workbench Provider Boundary uses the exact active Run Turn when one exists,
otherwise the same selected Conversation identity, when resolving durable
route/transmission evidence; the standalone Settings summary intentionally
remains global. An absent or unknown selected Conversation never falls back to
another Conversation's latest Turn, and a Run Turn cannot be rebound across
Conversations.

The canonical Artifact path adds backend-owned Result, Change, Preview,
Verification, and Undo projections to each Work ArtifactVersion. A pending
preview is admitted only from that version's managed draft after regular-file,
size, UTF-8, store-root, and content-digest checks; Proposal payload is not a
content or preview owner. A materialized preview is reread only from a regular
file inside the ArtifactVersion's reconstructed delivery scope and is shown only when its current byte
digest, stored observed digest, and canonical Verification Item agree. File
drift, disappearance, symlinks, oversize content, or non-UTF-8 bytes remove the
preview and prevent the Task from retaining delivered product credit. React
renders these typed projections; it does not read files, proposals, or
`task_runtime.db` itself.

Each Artifact result additionally resolves its source Item to the exact
Run/provider/model-bound Conversation Turn, projects local resources bound to
that Turn, and exposes the immediate predecessor version when present. Opening
or exporting performs a fresh current-version/digest check; exporting verifies
the destination copy. Open, Export, and Undo-request failures remain scoped to
the Artifact card and do not become success announcements. The read model does
not infer missing provenance from current Settings or current attachments.
FinalResult separately persists structured completion limitations as exact
requirement ids, descriptions, and evidence references; the projection labels
them as disclosed limits, never as supporting sources.

The canonical Artifact effect journal binds the exact ArtifactVersion,
materializer ItemAttempt, Review dispatch claim, target digest, content digest,
and physical effect state. `prepared`, `staged`, `confirmed`,
`failed_before_effect`, and `effect_unknown` survive restart. Recovery inspects
the exact filesystem facts, never blindly redispatches an ambiguous effect, and
repairs a confirmed-but-unprojected Review decision without creating a
ProposalStore Artifact record. A confirmed governed Undo is version-bound and
independently receipted. Create Undo moves the verified file to governed trash;
replacement Undo restores a pre-change snapshot only when its retained bytes,
restore digest, current-target digest, ArtifactVersion, Run, and Project scope
all still match. Both preserve the original verified Task history and are
presented as a later reversal instead of missing delivery.

Focused revision creates a new canonical Run rather than rewriting a completed
Run or reusing Retry. Its admission binds the exact current verified
ArtifactVersion and instruction digest; the Task store retains all prior
FinalResults and creates the next version only through the regular Artifact
draft, semantic verification, target-precondition, Review, materialization, and
digest-verification path. The base content is bounded untrusted model context,
and the runtime rejects target/media changes, multiple outputs, and no-op
revisions.

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

Product read state comes from `LifeStateProjection` and backend ViewModels. The
Workbench uses one aggregate backend snapshot rather than composing overlapping
frontend reads. Governed
writes pass through proposal, permission, persistence, and target-domain owners
rather than page-local state.

The shipped top-level surfaces are Workbench (`/workspace`), Personal
Intelligence (`/life-model`), and Settings. Task and approval status are
presented in their Conversation and a Needs Attention filter rather than
duplicate top-level Task and Review products.

## Domain ownership

The current boundary is defined by ADR 0016:

- Conversation and the canonical Work runtime own Chat and Work execution;
- Agent Memory owns working, project, episodic, semantic, procedural, and
  Reflection context;
- LifeModel owns confirmed long-term understanding of the user;
- domain stores own task, transient state, calendar, email, and other business
  facts;
- safety and governance own permissions, privacy, review, and write admission.

Reusable tool permissions remain durable `ToolPermissionStore` facts. Settings
reads them through `ToolPermissionViewModel` and may revoke only an exact,
active, reusable record through the canonical store. A consumed, expired, or
one-time Review authorization is not presented as a reusable grant. This
lifecycle is separate from the Run execution ceiling: revocation can remove
authority, while neither Settings nor the Run mode can create authority that
Policy, scope, and Review did not grant.
Action-bound Work permission and exact NetworkPolicy consent instead use
separate reviewed one-shot records. ToolGateway consumes them only when the
retry matches the reviewed action/input or network-decision binding; they do
not appear as reusable grants.

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
adapters retain bounded receipts, but cannot declare Task completion. No
historical report runtime remains as a fallback. Capabilities execute through
the general coordinator or are unavailable.

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
