# OpenLife Architecture

## Status

Stable source map and accepted target direction for OpenLife. Sections marked
"current" describe the production tree; sections marked "target" describe an
accepted migration that is not yet product credit. Current source and accepted
ADRs remain authoritative.

## Current product path

```text
React Workbench
  -> frontend/src/tauri.ts
  -> Tauri commands in src-tauri/src/lib.rs
  -> Rust runtimes and governed command gateways
  -> openlife-core contracts and canonical stores
  -> SQLite, local files, Keychain, providers, and governed external tools
```

Main Chat send and stream have separate transport entrypoints and converge on
`OpenLifeTurnRuntime`:

```text
main_chat_send.rs | main_chat_streaming.rs
  -> main_chat_turn_runtime.rs
  -> main_chat_kernel.rs
  -> openlife-core/src/agent/main_chat_agent_v1.rs
```

Other product commands, including Review acceptance, Settings persistence, and
direct Memory controls, use their command-specific gateways rather than passing
through `OpenLifeTurnRuntime`.

The current runtime already owns admission, durable user-message recording,
bounded context, provider execution, cancellation, recovery, and terminal
settlement. Its current persistence still splits lifecycle responsibility among
task sessions, Agent runs, action queues, event streams, and a separate
PlanExecute session. A current Main Chat operation is also effectively one task
slice. These are migration constraints, not the target product contract.

The first S2 vertical slice adds `CanonicalTaskRuntimeStore` on the
provider-generated report path. It owns stable report Task identity, Run
membership, typed instruction/plan/tool/observation/provider-generation/
artifact/review/materialization/verification/final-result Items, and independent
ArtifactVersion metadata in `task_runtime.db`. A report Task and Run begin
before governed reads or provider work with the Policy-authorized instruction
and deterministic plan digest. Completed tool and provider receipts append
later in the same Run before ArtifactDraft. The canonical store records their
identities and bounded digests, not prompt or response bodies. The report Artifact exists before its
Proposal; Review is a checkpoint relation, and confirmed materialization updates
the same ArtifactVersion. The Task becomes delivered only after each current
ArtifactVersion has an exact expected/observed digest match and the store writes
the completing Run's canonical FinalResult. This is current product code for
that path, not yet a
claim that every Main Chat route has migrated.

S3 extends that same report Run rather than adding another execution owner.
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
model output before a report ArtifactDraft or ReviewCheckpoint can exist.
Missing/failed reads or a failed one-shot citation repair therefore stop before
provider-backed completion and durable effects. For document reads, durable
replay metadata contains only the selected chunk count and stable selection
digest. Restart replay reselects the exact task-bound ResourceStore content and
must reproduce that digest before provider synthesis; it does not redispatch the
ToolGateway read or persist a document-body preview.

S4 extends the same report Run with typed Steering Items and a monotonic plan
revision. Workspace submits authenticated steering while work is active.
Conversation remains the only body owner; `CanonicalTaskRuntimeStore` keeps the
exact message reference and digests. One pending in-scope Steering Item is
consumed exactly once before the next report provider generation and survives
restart. Steering that asks for a new workspace, provider, network route, tool,
destructive action, or other scope is recorded blocked and grants nothing. The
process also bounds independent Main Chat executions before any message or task
persistence, while the cancellation registry retains one execution owner per
task.

Review approval can use one inline approve-and-continue command. The backend
first proves acceptance and, when required, materialization; it then reloads
task truth and resumes only when the existing control owner says the same task
is resumable. Approval, effect confirmation, and continuation remain separate
facts even though the product presents one action.

The existing backend-owned `TasksViewModel` and `WorkspaceViewModel` now read a
consistent canonical report snapshot and project its Run memberships, typed
Items, Artifact versions, Review wait, rejection, verified delivery, and
effect-unknown states. Exact Run membership overlays the migrated report onto
its compatibility execution session instead of showing two tasks. Canonical
report lifecycle wins when compatibility TaskSession state disagrees; the
compatibility session remains only the current control target until that control
path is migrated.

S5 adds backend-owned Result, Change, Preview, and Verification projections to
each report ArtifactVersion. A pending preview is admitted only from the exact
proposal whose Artifact id, version, target digest, content digest, and body
digest all match. A materialized preview is reread only from a regular file
inside the configured safe paths and is shown only when its current byte digest,
the stored observed digest, and the canonical Verification Item agree. File
drift, disappearance, symlinks, oversize content, or non-UTF-8 bytes remove the
preview and prevent the Task from retaining delivered product credit. React
renders these typed projections; it does not read files, proposals, or
`task_runtime.db` itself.

## Canonical task runtime target

ADR 0017 accepts one product lifecycle:

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

The shipped Workbench routes are:

```text
/today
/workspace
/tasks
/review
/life-model
/settings
```

Product read state comes from `LifeStateProjection`, backend ViewModels, and
strict frontend adapters that compose named backend owner outputs. Governed
writes pass through proposal, permission, persistence, and target-domain owners
rather than page-local state.

## Domain ownership

The current boundary is defined by ADR 0016:

- Agent Runtime owns turn and action execution;
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

During the current vertical migration, `AgentRunStore` remains the execution
and receipt owner while `CanonicalTaskRuntimeStore` owns stable report Task,
Run membership, typed execution-fact Item, and Artifact metadata. It
deliberately does not copy AgentRun status, provider payloads, or TaskSession
bodies. Compatibility owners are retired only after each migrated path has a
replacement read model and recovery proof.

## Personal intelligence boundary

Agent Memory and LifeModel participate through narrow context, learning, and
materialization ports. They do not own Task state, permission, execution
strategy, artifacts, or completion. A later LifeModel implementation change,
including possible AI-assisted maintenance, must not require rewriting the
Agent harness.

## Source maps

- [Agent runtime](architecture/agent-runtime.md)
- [LifeModel](architecture/life-model.md)
- [Governance](architecture/governance.md)
- [Agent Memory](architecture/memory.md)
- [Testing](development/testing.md)
- [Accepted decisions](../plans/adr/)

These documents explain source. They do not override runtime code or accepted
ADRs.
