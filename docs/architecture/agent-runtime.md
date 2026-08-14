# Agent Runtime

## Status

Source-backed description of the current runtime. It is not a product-readiness
claim.

Release Main Chat has two explicit modes. Chat delegates to
`CanonicalChatRuntime`; Work delegates to `CanonicalWorkRuntime`. Both use the
same Conversation owner, while only Work creates canonical Task/Run state.
Local command-surface and runtime evals are evidence inputs, not external-live
provider completion.

## Authority

Product boundaries come from `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and
current source. Superseded execution plans remain in Git history.

## Last verified

2026-08-13 during R2 general Work runtime reconstruction.

## Source map

- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/canonical_chat_runtime.rs`
- `src-tauri/src/canonical_work_runtime.rs`
- `src-tauri/src/main_chat_steering.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/main_chat_policy_runtime.rs`
- `src-tauri/src/main_chat_react_tool_selection.rs`
- `src-tauri/src/main_chat_react_runtime.rs`
- `src-tauri/src/main_chat_react_execution.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `src-tauri/src/main_chat_command_surface_eval.rs`
- `src-tauri/src/main_chat_final_gate.rs`
- `src-tauri/src/main_chat_live_provider_harness.rs`
- `src-tauri/src/main_chat_runtime_module_tests.rs`
- `src-tauri/src/main_chat_command_surface_tests.rs`
- `src-tauri/src/main_chat_live_provider_tests.rs`
- `openlife-core/src/agent/main_chat_agent_v1.rs`
- `openlife-core/src/agent/model_router.rs`

## Evidence Boundary

Runtime tests and live-provider report builders are local evidence. External
provider behavior remains unproven until an explicitly authorized live run.

## Current Entry Flow

Buffered and streaming transports preserve the same canonical owner for each
mode. Chat receives caller-owned Conversation and Turn IDs. Work additionally
requires caller-owned Task and Run IDs. `CanonicalWorkRuntime` begins the
Conversation Turn and Task Run before provider execution, records a typed
ProviderGeneration Item and ItemAttempt, commits the assistant Item, and then
binds one FinalResult to that exact assistant Item. Exact replay does not call
the provider again; retry creates another Run and Turn for the same Task.

`src-tauri/src/main_chat_turn_runtime.rs` is retained for capability migration
tests and pre-R3/R4 internal consumers. It is not reachable as a release Chat or
Work fallback.

For canonical Work and report Tasks, `src-tauri/src/read_models/tasks.rs` is the product
presentation boundary. It joins the canonical ArtifactVersion to an exact
proposal while waiting for Review, or to a digest-matching regular file after
materialization. It exposes bounded change, preview, and verification fields to
`TasksViewModel`. A stored completion label alone cannot preserve delivery when
the current file is missing or its bytes drift.

The former `main_chat_turn_pipeline.rs` compatibility wrapper is deleted.
Buffered and streaming transports call `OpenLifeTurnRuntime` directly.

## Kernel Responsibilities

`src-tauri/src/main_chat_kernel.rs` is the current turn-level kernel. It builds
bounded context, classifies write and memory intents, handles proposal/blocker
paths, executes read-tool paths, and falls back to DirectAnswer when no governed
tool or proposal path applies.

Current kernel result paths set `legacy_fallback_used=false` and
`direct_writes_executed=false`. DirectAnswer uses a model client and records
provider/scheduler trace evidence, but it does not create tools, proposals, or
durable writes. Proposal paths create Review Center proposals rather than
applying durable truth directly.

`src-tauri/src/main_chat_context_loader.rs` builds bounded knowledge-format
context from workspace/configured files such as `AGENTS.md`, `SOUL.md`,
`USER.md`, `MEMORY.md`, and selected `SKILL.md`. Those surfaces are context,
not policy override and not user truth promotion.

`src-tauri/src/main_chat_policy_runtime.rs` classifies the current task's
policy topic, risk, and write-side-effect requirements. It reads PolicyStore
only: sensitive topics remain LocalOnly and unconfirmed external writes remain
proposal-first. It does not read HeuristicStore or inject personalization.

Generic `AgentRuntime` and `AgentLoop` accept an explicit `RuntimePolicyContext`
containing provider authorization, metadata-safe provenance and the
proposal-first action fact. They do not accept a legacy YAML `LifeModel`, an
HS packet, heuristic guidance or an implicit personalization prompt. Agent
Memory remains an explicit input; canonical LifeModel v2 personalization is
compiled by the owning product adapter before the generic runtime boundary.

Main Chat personalization has one product path: bounded Agent Memory plus the
canonical LifeModel v2 runtime context. The kernel no longer compiles an
accepted-guidance/HS context in parallel. Ordinary planning uses the same
canonical v2 planning hints to draft a bounded Plan Item. Planning has no
standalone release IPC, session store, or product lifecycle; the remaining
PlanExecute-named core code is an internal drafting/evaluation algorithm, and
its former session rules are test-only regression fixtures.

Historical AgentRun rows can still expose minimized HS selection-audit and
behavior-check metadata through the product read model. Those DTOs are
read-only compatibility: current constructors initialize them empty, and no
selector, provider authorization, tool capability, or durable-write path can
be reconstructed from them. They can be removed when the corresponding
historical AgentRun columns are explicitly migrated or retired.

Scheduled tasks consume their durable task claim, typed Policy, canonical
StateStore snapshot and Agent Memory. Planner mode does not advertise the
legacy `life_model.read` or mixed-owner `goal.read` tools. The authenticated
development A2A sidecar exposes only its bounded reasoning bridge and does not
serve legacy personal-profile query skills; release frontend code exposes no
A2A wrapper. The old release Proactive suggestion command and frontend wrapper
had no product caller and are retired. The remaining Proactive core is limited
to proposal-rejection evidence compatibility; it does not own LifeModel,
learning, or the Agent runtime.

## Canonical Capability Execution

Release Work capability selection is part of the schema-validated structured
plan owned by `canonical_work_runtime.rs`; ReAct and PlanExecute are not product
routes or lifecycle owners. The planner receives only Policy-authorized
capability kinds and exact eligible registered read-only MCP manifest ids. The
runtime rejects invented kinds, targets, permissions, manifest digests, and
executable arguments. It binds the selected MCP target to the current manifest
execution-contract digest before persisting the plan.

`main_chat_kernel.rs` converts admitted plan steps into exact bounded adapter
requests. Imported documents remain bound to the source Turn, workspace reads
remain inside the resolved workspace root, Web Search/Fetch remain subject to
network policy and citation validation, selected Skills contribute bounded
context, and registered MCP reads pass the live manifest and permission checks.
Actual reads execute through `ToolGateway`; every dispatch is a canonical
ItemAttempt with a typed receipt and a digest-only Observation.

One bounded observation-driven replacement plan may continue the same Run when
all prior tool attempts succeeded but evidence validation still cannot produce
a deliverable. It cannot repeat completed capabilities, expand registered MCP
targets, reset the Run budget, or erase earlier receipts. Any failed, blocked,
cancelled, or effect-unknown attempt terminates instead. The former ReAct and
PlanExecute execution branches compile only for historical compatibility tests;
release Chat and Work cannot enter them.

The first migrated knowledge-work path also exposes production
`document.read`. Policy grants it only for explicit attachment/bound-document
requests, including ordinary phrases such as “这两份文件” or “这两份表格”. The
executor selects only resources bound to the exact task operation, records a
metadata-safe selection digest, and returns untrusted evidence. For document-
only, Web-only, or combined reports, the kernel records ordered read Items,
then invokes the user-selected provider once with exact request-scoped source
contracts. A rejected local-resource or Web citation receives at most one
provider retry; read tools are not redispatched, and a second failure produces
no ArtifactDraft or Proposal. Durable document-read metadata never stores body
or body-preview text: it keeps only selection digest/count and a safe summary.
Restart synthesis reselects from the canonical task-bound ResourceStore and
fails closed if the selection digest or count has drifted.

For general Work, `CanonicalTaskRuntimeStore` creates Task, Run, Instruction,
and optional Plan before the first governed read or provider call. It also owns
ArtifactDraft, ReviewCheckpoint, materializer ItemAttempt, Verification,
FinalResult, and Undo state. Final delivery requires the canonical FinalResult
record and its exact completed Item; a confirmed Undo preserves that original
completion proof and adds an independently receipted reversal. Active
Workspace steering is an authenticated Conversation message plus a digest-only
Steering Item bound to the exact execution session, canonical Run, and base
plan revision. The kernel consumes one pending in-scope Steering Item at the
safe checkpoint before provider generation. Consumption is transactional and
increments the plan revision; restart cannot consume it twice. A scope-
expanding steering request is recorded blocked and cannot alter policy or mint
a capability.

Independent Main Chat turns share a process-wide bounded execution semaphore.
The limit is claimed immediately after request validation and before canonical
message, task, or run persistence. Per-task cancellation registration remains
the single-owner guard, so identical concurrent work cannot bypass task
ownership.

## Runtime Support And Task Evidence

`src-tauri/src/main_chat_runtime_support.rs` creates task sessions, appends
metadata-safe transcript entries, queues governed actions, classifies execution
policy, and finalizes failures with `directWritesExecuted=false`.

Release Work cancel and retry are owned by `canonical_work_runtime.rs` and
operate on canonical Task/Run/Turn identity. The former TaskSession list,
detail, refresh, resume, cancel, and action-retry IPCs are removed from the
release handler and frontend. `main_chat_task_controls.rs` remains
compatibility/test code only. Canonical Artifact approval and Undo resume
through ReviewWorkflow into the same Work Item lifecycle and never project an
AgentRun.

## Test And Eval Surfaces

`src-tauri/src/main_chat_command_surface_eval.rs` runs local command-surface
coverage across buffered and streaming cases. It proves ordinary send/stream
shape for DirectAnswer, file/session/memory reads, proposal paths, web blockers,
MCP read paths, and ToolPermission proposals under local/scripted conditions.

`src-tauri/src/main_chat_final_gate.rs` aggregates command-surface and live
provider evidence. It requires separate direct generation, web AgentLoop,
registered MCP AgentLoop, and proposal-permission live scenarios before final
readiness can be credited.

`src-tauri/src/main_chat_live_provider_harness.rs` contains fail-closed live
provider preflight and harness logic. Local HTTP compatible proof remains
provider-client path evidence only, not external live provider completion.
