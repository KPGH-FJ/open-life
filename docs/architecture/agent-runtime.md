# Agent Runtime

## Status

Source-backed description of the current release runtime. It is an architecture
map, not a readiness claim. Controlled tests, exact-native evidence, and
external-live evidence remain separate.

## Product spine

OpenLife has two composer modes over one canonical Conversation owner:

```text
frontend/src/ipc/conversation.ts
  -> main_chat_send.rs | main_chat_streaming.rs
  -> canonical_chat_runtime.rs | canonical_work_runtime.rs
  -> provider_runtime.rs | provider_client.rs
  -> ToolGateway | ReviewWorkflow | personal-intelligence ports
  -> ConversationStore
  -> CanonicalTaskRuntimeStore (Work only)
```

Chat owns `Conversation -> Turn -> Item`. It records the authenticated user
Item, provider attempt, assistant Item, and terminal Turn state. Chat never
creates a Task.

Work owns `Task -> Run -> Item -> ItemAttempt -> FinalResult` in addition to
the same Conversation history. One Conversation may retain multiple completed
Tasks, while the Workbench presents the currently relevant outcome. Retry
creates a new Run and Turn under the same Task; it never creates another
lifecycle owner.

No parallel execution store, plan lifecycle, action queue, or compatibility
IPC participates in the release graph.

## Source map

- `src-tauri/src/canonical_chat_runtime.rs`
- `src-tauri/src/canonical_work_runtime.rs`
- `src-tauri/src/provider_runtime.rs`
- `src-tauri/src/provider_client.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/main_chat_tool_selection.rs`
- `src-tauri/src/main_chat_steering.rs`
- `src-tauri/src/personal_intelligence_ports.rs`
- `src-tauri/src/read_models/tasks.rs`
- `openlife-core/src/conversation.rs`
- `openlife-core/src/task_runtime.rs`
- `openlife-core/src/work_orchestration.rs`
- `openlife-core/src/agent/tool_gateway.rs`

## Planning and capability execution

Planning is a schema-validated Plan Item inside the current Run. The model may
propose the structure, but Policy defines eligible capability kinds and the
runtime validates dependencies, targets, budgets, and completion requirements.
There is no separate plan session, plan store, or plan-specific task lifecycle.
Work performs one bounded second-pass goal-coverage audit before accepting a
schema-valid plan. The audit compares the draft with the authenticated user
request and must preserve requested sources, current-information requirements,
deliverable formats, verification, and explicit stop-before-write conditions.
An invalid or unaudited plan blocks; it is never weakened into an answer-only
fallback.

Running Work accepts one authenticated Steering item against the exact current
Run revision. Steering text remains in ConversationStore; TaskRuntime stores
only its reference and digests. At each safe checkpoint, the selected provider
revises the same typed plan contract. The runtime intersects the candidate with
the existing capability and MCP-target envelope and requires already completed
steps to remain unchanged. Applying the plan and resolving Steering is one
TaskRuntime transaction. Provider or contract failure is persisted rejected;
typed scope expansion is persisted blocked. TasksViewModel projects the exact
status and applied revision instead of inferring success from the submit
command.

The Work Item scheduler handles imported documents, workspace files, Web
Search, Web Fetch, selected Skills, and exact registered read-only MCP tools.
`main_chat_tool_selection.rs` builds bounded governed candidates; it is not a
second execution runtime. Every real tool or provider dispatch becomes a
canonical ItemAttempt with a typed receipt. Successful reads add digest-only
Observation Items. Tool bodies and provider prompts are not stored as task
metadata.

Web Fetch may use an exact URL present in the authenticated current user
message or bind to a validated URL in a successful Web Search Observation from
the same Run. A plan that proposes direct fetch without either a user URL or a
Search dependency is rejected before execution. Named official-source
requirements are carried as lowercase domain constraints; non-matching search
or fetch observations are removed before they can become citable evidence. A
fetch URL therefore cannot be invented from model memory, reused from another
Run, or credited when its source domain does not satisfy the task.

Web and selected-file citations are request-scoped allowlists. A draft missing
an exact runtime-issued citation may receive one same-provider, same-context
repair before display or write. A second invalid draft blocks; unsupported
text is never shown or materialized as a completed result.

One bounded evidence-driven plan revision may continue the same Run. It cannot
expand Policy scope, repeat a completed capability, reset budget, or erase
earlier attempts. Failed, blocked, cancelled, and effect-unknown attempts stay
terminal facts.

Reusable tool-permission lifecycle is backend-owned. `ToolPermissionStore`
persists the exact tool, source, risk, action, policy, expiry, and usage facts;
`ToolPermissionViewModel` projects that metadata to Settings. Revocation
requires canonical write admission and deletes only the selected active,
reusable record. `allow_once` remains bound to its reviewed execution and is
never converted into a Settings-managed reusable grant. The frontend does not
reconstruct permission policy from proposals or configuration.

## Context, Memory, and LifeModel

`main_chat_context_loader.rs` compiles bounded Conversation history,
workspace/configured instruction files, selected Skill instructions, accepted
Agent Memory, and confirmed LifeModel v2 hints. Context never grants a tool,
permission, durable write, or completion claim.

Agent Memory and LifeModel are optional typed collaborators through
`personal_intelligence_ports.rs`. Their unavailable state degrades
personalization without replacing Conversation or Task truth. Markdown files
remain ordinary selected documents or Project artifacts; they are not an
implicit Memory source and never gain authority merely from their filename.

The not-yet-created Conversation exposes its Memory mode before the first send.
ConversationStore admits the selected mode and optional active Project in the
same insert. Existing Conversations may still change Memory mode through the
dedicated command and canonical ViewModel refresh; automatic learning reads the
persisted Conversation mode and never a composer-only value.

## Policy, provider, and network boundary

Policy owns risk, capability, scope, and data route. The user-selected provider
profile, model, and admitted reasoning effort remain fixed for the Turn/Run;
there is no silent provider, model, or reasoning-budget substitution. Network policy is resolved at the exact endpoint and
capability boundary. An Ask decision creates one scoped Review checkpoint;
approval authorizes only the matching retry. Canonical Work persists that
checkpoint against the exact Task, Run, ToolCall Item, model step, executor
Action, Proposal, and metadata-safe scope digest. Generic action permission and
NetworkPolicy consent use separate one-shot grant types; neither becomes a
reusable Settings permission.

The Work Run separately persists a user-selected execution ceiling. The
default `scoped_agent` ceiling leaves the ordinary capability set available but
does not grant any tool, path, network target, disclosure, or durable effect.
`observe_only` removes Artifact drafting and personal-intelligence mutation
from provider tool schemas, plan eligibility, and direct-step execution. Retry
inherits the prior Run's ceiling. ToolPermissionStore and Review remain the
only owners of per-action grants, so an existing grant cannot widen an
observe-only Run and the standard mode cannot bypass a required decision.

The composer reads a backend provider-profile registry. The registry preserves
the configured default even when it is unverified or offline, adds models
actually discovered from the local Ollama deployment, and distinguishes ready,
unverified, stale, offline, degraded, and incomplete configuration states.
Cloud readiness requires a fresh authenticated provider-validation receipt;
the presence of a model label and credential is not readiness. The selected
profile id travels with the exact Chat or Work request, and the runtime resolves
it before creating the Turn. Missing or unavailable profiles therefore cannot
leave a partial Turn, and selecting another installed local model derives a
request-scoped scheduler without mutating Settings or silently falling back.

Each registry item separates adapter contract from observed model evidence. It
reports the exact wire protocol, the structured-output request plus local
validation contract, and the adapter's reasoning control; none of those fields
claims that an arbitrary model actually followed the contract. Chat validation
comes only from a completed canonical Conversation Turn that is not the
execution-session Turn of a Work Run. Work compatibility comes only from recent
canonical Work Task/Run and Conversation Turn facts: a completed FinalResult
validates that exact profile, while an explicit model-authored plan or AgentStep
schema failure is projected as an observed Work contract failure. Provider
validation proves route readiness, not Chat or Work behavior. Permission,
Project, network, and policy failures never become model compatibility
evidence. A model with an observed Work contract failure remains available for
Chat and an explicit user-authored Work retry, but is never presented as proven
Work-ready. A future explicit compatibility probe may add a stronger admission
gate only together with a complete revalidation path.

The composer consumes those facts from the complete backend
`ConversationViewModel` after every terminal Turn and Conversation switch. It
must not refresh only transcript messages and leave provider compatibility,
selected profile, reasoning binding, or Work availability stale until an app
restart.

Adapter structured-output behavior and user-selectable reasoning are separate
facts. Reasoning is a typed provider/model capability containing the accepted
levels, provider default, mandatory state, discovery source, and wire protocol.
Verified official contracts cover OpenAI GPT families, Google Gemini,
DeepSeek V4, and Ollama GPT-OSS. An official OpenRouter profile may instead
discover the exact selected model's capability from bounded `/models` metadata;
dynamic router entries that omit reasoning metadata expose no selector.

The user may keep the model default, which omits an override, or select one
admitted level. The selection is persisted on the Conversation Turn and
canonical provider ItemAttempts and preserved on retry. Only the adapter maps
it to OpenAI/Gemini `reasoning_effort`, OpenRouter's unified `reasoning` object,
DeepSeek `thinking` plus effort, or Ollama `think`. Unknown models and custom
gateways retain their provider default and cannot acquire a control from a
model-name substring or frontend guess. Capability discovery failure never
erases the configured model or invents support.

The Agent loop is provider-agnostic. Goal interpretation, planning, capability
selection, observation handling, completion requirements, and Artifact
verification are shared runtime contracts. A provider adapter is limited to
wire concerns such as endpoint and credential binding, compatible request and
stream formats, native function/JSON support, and optional reasoning transport
controls. These adapter profiles are selected by protocol capability, never by
task meaning, and no model identifier may activate a different Agent path.
Unknown OpenAI-compatible endpoints use the standard protocol profile and must
fail explicitly when an advertised capability is unsupported; they do not gain
a vendor-specific fallback Agent.

Provider and tool receipts bind the canonical Task, Run, Item, and Attempt
identities. Settings route evidence reports configuration readiness separately
from proof that a provider request was actually sent.

## Artifact and Review lifecycle

Artifact identity is independent of Proposal identity. Each ArtifactVersion
owns its Task/Run/Item provenance, managed draft, target precondition, content
digest, Review checkpoint, materializer attempt, verification, and optional
Undo. When a version replaces an existing target, the runtime captures the
exact prior bytes into app-owned pre-change storage before binding that target
precondition. Snapshot metadata and version-source metadata commit in one
transaction; absent, oversize, or digest-drifting bytes fail closed.

The Tasks read model resolves each current ArtifactVersion's source Item back
to its exact Run and provider-bound Conversation Turn. It also projects local
resources bound to that source Turn and the immediate previous version when one
exists. Results therefore never relabel an old Artifact with the currently
selected model, Project, or attachments. Missing historical provenance remains
absent rather than being inferred.

Review approval is a checkpoint transition, not task completion. Confirmed
materialization projects into the same ArtifactVersion and only then permits a
FinalResult. Recovery distinguishes prepared, staged, confirmed,
failed-before-effect, and effect-unknown states and never blindly redispatches
an ambiguous effect.

Open, Export, and Undo-request actions are not completion signals. Open and
Export re-read and digest-check the exact current ArtifactVersion immediately
before their effect; action-local failures are retained on the Result card.
Export additionally verifies the bytes at the chosen destination. Undo remains
available only when a verified create can be moved to the governed trash target
or a verified replacement has its exact digest-bound pre-change snapshot. A
replacement restore is a separate reviewed external write with both the
restore digest and current-target digest precondition. It shares the Artifact
effect journal and receipt path, and becomes `undone` only after the restored
bytes are observed. Older replacement versions without retained bytes remain
explicitly unavailable rather than simulated.

Focused post-completion revision is neither Retry nor Undo. It admits a new Run
against one exact verified current ArtifactVersion, persists the base version,
base digest, and new instruction digest, and retains prior FinalResults. The new
Run preserves the source Run's provider/model, reasoning, selected Skill,
resource scope, execution mode, and immutable Project scope. The verified base
content is bounded untrusted comparison data for generation and independent
semantic verification, never source credit or executable instruction. Delivery
is limited to one changed Artifact with the same identity target and media type;
a replacement still crosses the ordinary Review and materialization boundary.

The bounded text artifact contract currently supports Markdown, plain text,
self-contained HTML, JSON objects/arrays, and CSV tables. Provider output is
schema-validated, serialized by the backend where appropriate, checked against
the requested extension, and staged under an allowed output root before Review.
HTML rejects active or remotely loaded content; CSV rejects formula-leading
cells. These formats use the same ArtifactVersion, Review, materialization and
verification lifecycle rather than format-specific task runtimes.

Review does not own a generic task-resume action. Canonical Work owns immediate
Stop for the current Run and separate Retry/Continue commands that create a new
Run from the exact latest failed, blocked, cancelled, or interrupted Run.
Approval continues only the exact waiting Item checkpoint by starting another
Attempt on the same ToolCall Item and Run. Rejection blocks that Run, and Stop
may cancel it while the checkpoint is waiting. The in-process waiter is
ephemeral: if approval happens after restart, the durable approval remains but
the old Run becomes `interrupted`, because the raw tool input and live adapter
context are not reconstructed from Review metadata. Product surfaces do not
expose Pause or outcome-level Cancel Task until those actions have distinct
durable owners.

## Workbench projection

`WorkbenchViewModel` captures Conversation, scoped Workspace, Tasks, Review,
and Provider Boundary lanes once. Its Tasks and Workspace lanes read canonical
Task snapshots and project plans, Items, attempts, Needs Attention, inline
Review, Results, Changes, Preview, Verification, and Undo. React does not
reconstruct lifecycle truth from messages, Proposal payloads, diagnostics, or
local files.

Each Task projection resolves the latest Run's immutable Project revision and
scope digest plus the exact provider profile/model attempt back to the matching
Conversation Turn. The resulting metadata-safe provenance includes provider,
model, local/cloud class, Turn status/error, and Project name/revision without
projecting a current Settings value as historical truth or exposing the folder
path.

Project lifecycle controls follow the same projection rule. ConversationStore
owns Project status, revision, selection, and Conversation references;
CanonicalTaskRuntimeStore owns Task/Run references. The Conversation ViewModel
combines those facts into allowed update, archive, restore, and delete controls.
Permanent deletion is fail-closed when Task history is unavailable, requires a
native system confirmation, and removes only the Project record—never the
selected folder or user files.

Conversation lifecycle uses the same two-owner read. ConversationStore owns
active/archived status and exact Turn/message counts; CanonicalTaskRuntimeStore
owns total and active Task references. Archive refuses an active Chat Turn or
Work Task, restore retains all original bindings, and permanent deletion is
available only for an archived record whose Turn, message, and Task counts are
all zero. The final deletion rechecks both stores after native confirmation, so
ordinary Conversation history never disappears through a cascading shortcut.

Filesystem scopes remain typed at their owners. The Run-bound Project scope has
one optional primary root and revisioned secondary read roots. Each
`read_workspace_file` action binds one exact root id plus a relative path and
authorizes only that root; omitting the id defaults to the primary root. A
configured
`artifact_output_directory` is write authority only for reviewed non-Project
exports; `additional_read_roots` is generic read authority only. The native
Artifact picker cannot grant reads, editable Settings payloads cannot inject
filesystem authority, Project secondary roots cannot authorize writes, and
canonical Work Artifact delivery reconstructs its own primary-Project-or-managed
scope.

## Evidence boundary

Repository tests prove controlled source and product contracts. Browser-shell
tests prove the React/Tauri contract under controlled data. Exact signed QA
runs prove native process, persistence, and UI paths for the exact artifact.
External-live credit additionally requires the selected real provider or Web
route and its receipts. No evidence level substitutes for another.
