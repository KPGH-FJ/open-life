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

## Policy, provider, and network boundary

Policy owns risk, capability, scope, and data route. The user-selected provider
profile and model remain fixed for the Turn/Run; there is no silent provider or
model substitution. Network policy is resolved at the exact endpoint and
capability boundary. An Ask decision creates one scoped Review checkpoint;
approval authorizes only the matching retry.

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
Undo.

Review approval is a checkpoint transition, not task completion. Confirmed
materialization projects into the same ArtifactVersion and only then permits a
FinalResult. Recovery distinguishes prepared, staged, confirmed,
failed-before-effect, and effect-unknown states and never blindly redispatches
an ambiguous effect.

The bounded text artifact contract currently supports Markdown, plain text,
self-contained HTML, JSON objects/arrays, and CSV tables. Provider output is
schema-validated, serialized by the backend where appropriate, checked against
the requested extension, and staged under an allowed output root before Review.
HTML rejects active or remotely loaded content; CSV rejects formula-leading
cells. These formats use the same ArtifactVersion, Review, materialization and
verification lifecycle rather than format-specific task runtimes.

Review does not own a generic task-resume action. Canonical Work controls own
cancel and retry; approval resumes only the exact waiting Item checkpoint.

## Workbench projection

`WorkbenchViewModel` captures Conversation, scoped Workspace, Tasks, Review,
and Provider Boundary lanes once. Its Tasks and Workspace lanes read canonical
Task snapshots and project plans, Items, attempts, Needs Attention, inline
Review, Results, Changes, Preview, Verification, and Undo. React does not
reconstruct lifecycle truth from messages, Proposal payloads, diagnostics, or
local files.

## Evidence boundary

Repository tests prove controlled source and product contracts. Browser-shell
tests prove the React/Tauri contract under controlled data. Exact signed QA
runs prove native process, persistence, and UI paths for the exact artifact.
External-live credit additionally requires the selected real provider or Web
route and its receipts. No evidence level substitutes for another.
