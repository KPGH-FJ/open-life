# Agent Runtime

## Status

Source-backed description of the current runtime. It is not a product-readiness
claim.

The current ordinary Main Chat entrypoints are implemented through
`send_message_with_state` and `start_stream_message_with_state`, which delegate
to `OpenLifeTurnRuntime`. Local command-surface and runtime evals are evidence
inputs, but they do not count as external live provider completion.

## Authority

Product boundaries come from `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and
current source. Superseded execution plans remain in Git history.

## Last verified

2026-08-10 during Phase 5.5C generic runtime input convergence.

## Source map

- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_turn_pipeline.rs`
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

Ordinary buffered chat enters `src-tauri/src/main_chat_send.rs`. Ordinary
streaming chat enters `src-tauri/src/main_chat_streaming.rs`. Both wrappers
construct `OpenLifeTurnRuntime` and pass `OpenLifeTurnInput` containing the
session id, chat messages, optional selected skill id, and delivery mode.

`src-tauri/src/main_chat_turn_runtime.rs` owns the current runtime boundary. It
starts or resumes an Agent task session through `start_main_chat_agent_turn`,
decides a route, invokes the Main Chat kernel, records route evidence, and
finalizes the task state. Its canonical delivery view separates answer text,
completed actions, observations, proposals, blockers, and pending user actions.

`src-tauri/src/main_chat_turn_pipeline.rs` is a compatibility wrapper around
`OpenLifeTurnRuntime`. It does not make the older route family authoritative.

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
accepted-guidance/HS context in parallel. PlanExecute receives the same
canonical v2 planning hints; its product entrypoint does not enable legacy
runtime-guidance consumption.

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

## ReAct And Tool Execution

ReAct tool selection starts in
`src-tauri/src/main_chat_react_tool_selection.rs`. It builds governed action
plans, candidate contracts, target allowlists, action-target allowlists, and
metadata-safe candidate labels. Generic MCP read candidates are bounded and
filtered to exclude high-risk, confirmation-required, write-like, disabled,
declarative-only, or contract-unsafe manifests.

Provider-ranked candidate preselection is allowed only when the route,
credential, network, and contract checks pass. The model response must be an
exact complete permutation in a one-field JSON object. Invalid responses are
ignored and deterministic ordering is kept.

`src-tauri/src/main_chat_react_runtime.rs` runs the governed AgentLoop with
`allow_writes=false`. It blocks allowlist violations, unsupported action types,
wrong action-target pairs, missing planned actions, and policy-denied selected
candidates as explicit blockers instead of silently executing a fallback.

`src-tauri/src/main_chat_react_execution.rs` executes accepted read actions
through `ToolGateway` and the ActionExecutor with write access disabled. Local
network policy can convert a web/network attempt into a structured blocker.

The first migrated knowledge-work path also exposes production
`document.read`. Policy grants it only for explicit attachment/bound-document
requests, including ordinary phrases such as “这两份文件” or “这两份表格”. The
executor selects only resources bound to the exact task operation, records a
metadata-safe selection digest, and returns untrusted evidence. For document-
only, Web-only, or combined reports, the kernel records ordered read Items,
then invokes the user-selected provider once with exact request-scoped source
contracts. A rejected local-resource or Web citation receives at most one
provider retry; read tools are not redispatched, and a second failure produces
no ArtifactDraft or Proposal.

## Runtime Support And Task Evidence

`src-tauri/src/main_chat_runtime_support.rs` creates task sessions, appends
metadata-safe transcript entries, queues governed actions, classifies execution
policy, and finalizes failures with `directWritesExecuted=false`.

`src-tauri/src/main_chat_task_controls.rs` exposes task-state, list/detail,
refresh, resume, cancel, and retry controls. Resume and retry are evidence
aware: they inspect continuity diagnostics, pending permissions, replay safety,
tool availability, provider availability, and action metadata before replay.

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
