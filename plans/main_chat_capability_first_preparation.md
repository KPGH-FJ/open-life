# Main Chat Capability-First Preparation Plan

> Date: 2026-06-27
> Status: development preparation artifact
> Baseline: `08dcb8a`
> Scope: prepare the next Main Chat development route for capability-first Agent
> work without reintroducing hidden fallback paths or governance-led product
> drift.

## 1. Purpose

This document prepares the next development phase before runtime code changes
begin. It is the bridge between the earlier kernel-rescue work and the new
product direction:

> Build the Agent capability first. Keep governance as a narrow safety backstop.

The previous privacy-first and governance-first emphasis produced useful
guardrails, but it also made the system heavy and weakened the default Agent
experience. The next phase should be judged by whether a user can ask OpenLife
to do useful work and see that work complete reliably.

This file does not claim that Main Chat Agent Execution v1 is complete. It
defines the preparation, boundaries, sequence, and proof requirements for the
next implementation pass.

## 2. Development Doctrine

The implementation should follow these rules:

1. Capability is the primary product metric.
2. Governance is a minimum viable backstop, not the main architecture.
3. Main Chat must have one turn owner.
4. Tool use should be the normal path for work-like requests.
5. Send and stream must remain behaviorally equivalent.
6. User-visible state must be simple and backed by runtime evidence.
7. Legacy fallback is allowed only as explicit compatibility behavior.

The minimum governance backstop is:

- no silent durable writes to LifeModel, long-term memory, files, external
  systems, providers, plugins, or shell;
- dangerous or external write actions require confirmation or a proposal;
- secrets such as API keys, tokens, and credentials remain redacted;
- every turn keeps enough trace to debug route, tool, proposal, blocker, and
  fallback behavior.

Everything else should be evaluated by whether it improves task completion.

## 3. Industry Cross-Check

The implementation should not migrate to LangGraph, Microsoft Agent Framework,
Temporal, or the OpenAI Agents SDK. The relevant lesson is architectural shape,
not framework adoption.

| Source | Relevant practice | OpenLife implication |
| --- | --- | --- |
| OpenAI Agents SDK guide: https://developers.openai.com/api/docs/guides/agents | Agents plan, call tools, collaborate, and keep enough state for multi-step work. Applications own orchestration, tool execution, approvals, and state when they need advanced runtime behavior. | OpenLife already owns the runtime. It should make the runtime explicit instead of splitting turns across Kernel, strategy, and legacy generation. |
| OpenAI Agents SDK tracing: https://openai.github.io/openai-agents-python/tracing/ | Agent runs trace generations, tool calls, handoffs, guardrails, and custom events. | OpenLife trace should be complete enough for debugging, but trace should not become the user-facing product itself. |
| OpenAI Agents tools: https://openai.github.io/openai-agents-python/tools/ | Tools are action surfaces such as fetching data, running code, calling APIs, and using local/runtime tools. | Work-like Main Chat prompts should route to a first-class tool loop, not to a conversational answer that pretends work happened. |
| LangGraph overview: https://docs.langchain.com/oss/python/langgraph/overview | Durable execution, streaming, human-in-the-loop, persistence, memory, and observability are runtime concerns for stateful agents. | OpenLife's existing task sessions, action queues, proposals, events, and memory stores are the right primitives; they need one pipeline owner. |
| Microsoft Agent Framework workflows: https://learn.microsoft.com/en-us/agent-framework/workflows/ | Workflows have explicit control flow, type safety, external integration, HITL, and checkpointing. | Main Chat should become a typed workflow-like pipeline where dynamic model/tool steps live inside explicit control flow. |
| Microsoft Agent Framework overview: https://learn.microsoft.com/en-us/agent-framework/overview/ | Agents are good for open-ended tool use and planning; workflows are good for well-defined multi-step control. | Use an Agent for route/tool choice, but use a typed pipeline for control, recovery, status, and final delivery. |
| Temporal Workflow Execution: https://docs.temporal.io/workflow-execution | Durable workflow execution is reliable because state persists through failures and resumes from the latest state; event history records workflow progress. | Do not adopt Temporal for this pass, but keep Main Chat turn/task state explicit, replayable, and recoverable instead of hiding progress in assistant prose. |

The cross-check supports one conclusion: the next OpenLife step is not more
readiness machinery. It is a capability-first workflow spine with trace and
approval as guardrails.

## 4. Current Repo Facts To Respect

| Fact | Code evidence | Preparation implication |
| --- | --- | --- |
| `send_message` and `start_stream_message` both decide Kernel vs special ReAct vs strategy/fallback. | `src-tauri/src/main_chat_send.rs:41`, `src-tauri/src/main_chat_streaming.rs:52` | Extract one route decision and one turn pipeline before changing behavior. |
| Kernel is a real turn executor with context, route, read tool, write outcome, blocker, and final answer events. | `src-tauri/src/main_chat_kernel.rs:2171` | Reuse Kernel as the core execution authority rather than adding another runtime. |
| ReAct still runs through `try_run_main_chat_agent_strategy` and old tool-selection helpers. | `src-tauri/src/main_chat_strategy.rs:210`, `src-tauri/src/main_chat_react_tool_selection.rs:571` | Move ReAct under the pipeline as a ToolLoop executor adapter, then reduce the old strategy path. |
| Strategy routing and execution policy still use keyword/string matching. | `openlife-core/src/agent/main_chat_agent_v1.rs:307`, `openlife-core/src/agent/main_chat_agent_v1.rs:455` | Add a structured route contract. Keep keyword routing only as deterministic fallback. |
| Default config is local-first and AgentLoop-off. | `openlife-core/src/config.rs:279` | Add a capability-first beta mode before judging product ability. |
| Diagnostics currently treats configured cloud API as validated. | `src-tauri/src/commands/diagnostics.rs:60` | Fix validation semantics early so readiness cannot overclaim provider capability. |
| Current preprocessing always desensitizes user messages and memory queries. | `src-tauri/src/main_chat_preprocess.rs:246`, `src-tauri/src/main_chat_preprocess.rs:287` | Capability mode must define a concrete privacy mode, not just a general desire to reduce masking. |
| Chat UI has many local state branches and developer surfaces. | `frontend/src/pages/ChatPage.tsx:1120`, `frontend/src/pages/ChatPage.tsx:1690` | Prepare a smaller `MainChatTurnView` user model before UI changes. |
| ActionExecutor already has useful side-effect backstops. | `openlife-core/src/agent/action_executor/mod.rs:168`, `openlife-core/src/agent/action_executor/tool_executor.rs:181` | Reuse existing write blockers/proposal/permission behavior; do not build a new governance layer. |
| `ActionExecutorConfig::default()` allows writes unless callers override it. | `openlife-core/src/agent/action_executor/mod.rs:31` | Every Main Chat Kernel/ToolLoop adapter must explicitly set `allow_writes=false`; do not rely on defaults. |

## 5. Preparation For The Seven Work Items

### 5.1 Capability Mode

Preparation content:

- Define a product/runtime mode named `capability_first_beta`.
- Define mode defaults: cloud-capable route preferred when configured and
  validated, AgentLoop enabled for work-like prompts, read tools enabled, and
  minimum governance backstop active.
- Define provider readiness semantics: `configured`, `validated`, `last_error`,
  `validated_at`, and `validation_source`.
- Define a `CapabilityPrivacyMode` for this mode: keep secret redaction and write
  confirmation, preserve ordinary personal context for model/task ability, and
  switch to stricter redaction or local-only handling only when the topic is
  explicitly sensitive or policy-bound.
- Define a rollout toggle so existing local-first users are not broken.

`CapabilityPrivacyMode` must be explicit before implementation:

| Field | Capability-first value | Required tests |
| --- | --- | --- |
| `model_input_redaction` | `secrets_only` for ordinary personal context; `policy_redacted` for HS-sensitive or LocalOnly topics; `strict_block` for block-level secret/credential findings. | A normal name, preference, project detail, and goal remain visible to the model in capability-first mode; API keys, tokens, private keys, and passwords are masked or blocked. |
| `memory_query_redaction` | `secrets_only` by default so recall quality is not destroyed by broad masking; `policy_redacted` for sensitive topics. | Memory/session retrieval can match ordinary user-provided entities while secret-like strings never leave the redaction boundary. |
| `external_tool_argument_redaction` | Secrets are always redacted; external/MCP/web arguments get policy-specific masking when the selected tool crosses a trust boundary. | A web/MCP call never receives raw credentials; a benign query can still include enough user intent to be useful. |
| `trace_redaction` | No raw secrets, no full raw prompt dump, bounded metadata-safe previews only. | Trace and debug bundles contain route/tool/proposal/blocker evidence without leaking secret payloads. |

The first implementation should route this mode through the existing
`preprocess_chat_input` / `preprocess_chat_input_v2` boundary or a narrow
successor, instead of adding an unrelated preprocessing stack.

Implementation boundary for privacy mode:

```rust
pub enum CapabilityPrivacyMode {
    ExistingDefault,
    CapabilityFirstBeta,
}

pub struct MainChatPreprocessOptions {
    pub capability_privacy_mode: CapabilityPrivacyMode,
}
```

The first PR should add the mode at the preprocessing boundary and keep the
existing call surface backward-compatible. The preferred shape is a small
options object passed into `preprocess_chat_input` / `preprocess_chat_input_v2`
or a narrow successor helper. Do not scatter capability-mode conditionals across
send, stream, Kernel, ReAct, memory retrieval, and UI code.

Provider validation must also be explicit before implementation:

| Field | Required behavior |
| --- | --- |
| Storage | Persist a metadata-safe validation record in an app-owned local store or config-adjacent state, not in assistant text or transient UI state. It must not persist API keys. |
| Identity | Record provider label, base URL hash or normalized endpoint identity, model, key-present boolean, validation source, and validated timestamp. |
| TTL | Treat validation as fresh for a bounded period, initially 24 hours. Expired validation may still be displayed as stale, but must not set `cloud_api_validated=true`. |
| Invalidation | Clear or stale the validation record when provider, base URL, model, key presence, or network policy changes. |
| Failure | Persist metadata-safe `last_error`, `failed_at`, and source for the last validation attempt. Do not serialize request payloads, raw provider responses, or secrets. |
| Diagnostics | `get_system_diagnostics` may return `cloud_api_configured=true` from config alone, but `cloud_api_validated=true` only from a fresh matching validation record. |
| Tests | Cover configured-but-unvalidated, validated-fresh, validated-stale, config-changed-invalidated, failed-validation-with-last-error, and no-key cases. |

Current blockers:

- `prefer_local_model=true` and `use_agent_loop=false` are defaults.
- Cloud API validation is currently inferred from configuration.

First implementation PR:

- Add typed mode config and UI label.
- Fix diagnostics so `cloud_api_validated` is false unless a real validation
  command or fresh stored validation result exists.
- Add the first `CapabilityPrivacyMode` plumbing at the preprocessing boundary.
- Add tests for configured-but-unvalidated vs validated provider state.
- Add tests proving ordinary personal context is preserved in capability mode
  while secrets stay masked or blocked.

Acceptance:

- A beta user can intentionally enter capability-first mode.
- The UI does not claim a provider is validated without evidence.
- Work-like prompts have permission to use ToolLoop paths when tools and a
  provider are available.
- Capability mode improves model/task context quality without leaking secrets or
  bypassing LocalOnly/sensitive-topic policy.

### 5.2 Single Main Chat Turn Pipeline

Preparation content:

- Define `MainChatExecutionPath`:
  - `KernelDirect`
  - `KernelReadTool`
  - `KernelWriteOutcome`
  - `ToolLoop`
  - `PlanExecute`
  - `GovernedBlocker`
  - `LegacyCompatFallback`
- Define `MainChatTurnRouteDecision` with selected strategy, path, reason,
  fallback eligibility, kernel support disposition, and provider/tool-loop
  requirements.
- Make send and stream consume the same decision object.
- Keep output behavior unchanged for the first PR.

Current blockers:

- Route decision is duplicated in send and stream.
- Legacy fallback is still a normal code path after strategy returns `None`.

First implementation PR:

- Add `src-tauri/src/main_chat_turn_pipeline.rs` with route-decision helpers.
- Replace duplicated send/stream route checks with that helper only.
- Add focused tests for send/stream path parity.

Acceptance:

- A route change requires one code edit, not one send edit plus one stream edit.
- Legacy fallback remains visible and countable.
- No product capability is lost in the extraction PR.

Phase 2 readiness supplement:

- Phase 2 is an extraction and parity phase, not a behavior rewrite. It must not
  move ReAct execution, change Kernel support rules, introduce model-backed
  routing, or delete legacy fallback.
- The first code object should be a typed decision record, not another
  strategy-specific branch:

```rust
pub enum MainChatExecutionPath {
    KernelDirect,
    KernelReadTool,
    KernelWriteOutcome,
    ToolLoop,
    PlanExecute,
    GovernedBlocker,
    LegacyCompatFallback,
}

pub struct MainChatTurnRouteDecision {
    pub path: MainChatExecutionPath,
    pub strategy_label: String,
    pub reason_code: String,
    pub kernel_supported: bool,
    pub fallback_allowed: bool,
    pub requires_provider: bool,
    pub requires_tool_loop: bool,
}
```

- `reason_code` must be a bounded enum-like string, not free-form assistant text.
- The helper must be pure over already-available command inputs and runtime
  config. It must not perform tool execution, provider calls, file reads, writes,
  or task-session mutation.
- Send and stream must call the same route-decision helper before dispatch. Any
  divergence must be represented as input fields on the same helper, not copied
  branching logic.
- Add a send/stream parity test table covering at least DirectAnswer,
  read-tool/file prompt, PlanExecute draft, proposal path, web blocker,
  registered MCP success, ToolPermission proposal, and legacy fallback
  eligibility.
- Add negative assertions that extraction keeps `legacyFallbackCount=0` for the
  existing ordinary command-surface matrix and keeps any explicit legacy fallback
  path labelled `LegacyCompatFallback`.
- Add a source guard that `main_chat_send.rs` and `main_chat_streaming.rs` do not
  each re-implement Kernel-vs-strategy-vs-fallback branching after the helper
  lands.
- Verification for Phase 2 must include `cargo check -p openlife-tauri`, the
  focused route-decision tests, and
  `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix`.

Phase 3 readiness supplement:

- Phase 3 is the `MainChatTurnPipeline` wrapper phase. It should use the Phase 2
  `MainChatTurnRouteDecision` object, but it must still preserve existing
  behavior.
- Do not start the ReAct ToolLoop adapter in Phase 3. That remains the following
  phase after the wrapper exists and send/stream parity is proven.
- Add a small orchestration boundary such as:

```rust
pub struct MainChatTurnPipelineInput {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub selected_skill_id: Option<String>,
    pub stream_mode: MainChatTurnStreamMode,
}

pub struct MainChatTurnPipelineOutput {
    pub route_decision: MainChatTurnRouteDecision,
    pub delivery: MainChatTurnDelivery,
}
```

- The first wrapper may still delegate to the existing Kernel, strategy, and
  legacy helpers. Its job is ownership and evidence shape, not a new executor.
- Keep send/stream as thin command adapters around the wrapper. They may retain
  transport-specific event emission, but they must not own route branching or
  fallback selection.
- Pipeline output must carry typed route evidence for Kernel, strategy result,
  and explicit legacy fallback. Do not rely on final answer prose to infer path.
- Preserve task-session, transcript, action queue, proposal, permission, and
  event emission behavior exactly.
- Add parity tests proving the wrapper has the same observable outcomes as the
  current send/stream command-surface matrix for DirectAnswer, file read,
  PlanExecute draft, proposal, web blocker, MCP read, ToolPermission proposal,
  and explicit legacy fallback eligibility.
- Add negative assertions that Phase 3 does not call provider/model during route
  decision, does not introduce writes, and does not make `LegacyCompatFallback`
  the default ordinary path.
- Verification for Phase 3 must include `cargo check -p openlife-tauri`, focused
  pipeline wrapper tests, source guards that send/stream remain thin adapters,
  and `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix`.

### 5.3 ReAct As ToolLoop Executor

Preparation content:

- Define a `MainChatToolLoopExecutor` adapter boundary.
- Let the first adapter call the existing ReAct AgentLoop/runtime helpers.
- Move ReAct under the turn pipeline as `ToolLoop`, not as an external strategy
  detour.
- Preserve ActionExecutor, ActionQueue, transcript, ToolPermission proposal,
  and follow-up synthesis behavior.

Current blockers:

- ReAct is selected by strategy but executed outside the Kernel/Pipeline shape.
- Tool planning still starts from deterministic keyword helpers for many cases.

First implementation PR:

- Create the adapter trait and wrap existing ReAct execution without changing
  tool semantics.
- Route ReAct through the pipeline path label.
- Require all Main Chat ToolLoop adapter calls to construct `ActionExecutor`
  with explicit `allow_writes=false`.
- Add no-hidden-fallback tests for file, web blocker, MCP read, and permission
  proposal cases.

Acceptance:

- ReAct is visible as the same turn lifecycle as DirectAnswer and Kernel read.
- Existing ReAct tests still pass.
- Single-step fallback is explicit, not disguised as successful ToolLoop.
- ToolLoop cannot accidentally inherit `ActionExecutorConfig::default()`
  write allowance.

Phase 4 readiness supplement:

- Phase 4 is the ToolLoop adapter phase. It should move ReAct execution under
  the `MainChatTurnPipeline` `ToolLoop` path label, but it must not introduce
  structured/model-backed routing yet.
- The first adapter should wrap existing ReAct helpers instead of rewriting tool
  planning or candidate selection:

```rust
pub struct MainChatToolLoopInput<'a> {
    pub session_id: &'a str,
    pub user_msg: Option<&'a ChatMessage>,
    pub desensitized_messages: &'a [ChatMessage],
    pub life_model: &'a LifeModel,
    pub context_summary: ContextSummary,
    pub embed_err: Option<String>,
    pub auto_checkin_msg: Option<String>,
    pub main_chat_agent_turn: &'a MainChatAgentTurn,
    pub selected_skill_id: Option<&'a str>,
}

pub enum MainChatToolLoopOutcome {
    Completed(SendMessageResult),
    ExplicitFallbackAvailable { reason_code: String },
    GovernedBlocker(SendMessageResult),
}
```

- The adapter may call the existing ReAct AgentLoop/runtime helpers and existing
  single-step fallback helper, but the pipeline must be able to tell whether the
  result came from AgentLoop, governed blocker, ToolPermission proposal,
  single-step fallback, or no result.
- Do not let Phase 4 change route selection. `MainChatTurnRouteDecision` remains
  the source of whether this turn is `ToolLoop`.
- All ToolLoop adapter construction of `ActionExecutor` must use explicit
  `ActionExecutorConfig { allow_writes: false, ... }`; source guards should fail
  if the adapter relies on `ActionExecutorConfig::default()`.
- Preserve existing candidate contract enforcement, exact target/action
  allowlists, ToolPermission proposal flow, ActionQueue/transcript writes, and
  follow-up synthesis behavior.
- Add no-hidden-fallback tests for file read, web blocker, registered MCP read,
  registered MCP ToolPermission proposal, and model-selected disallowed tool.
- Add negative tests proving a ToolLoop result cannot claim success without
  runtime observation/tool-call/proposal/blocker evidence.
- Keep legacy fallback available only as an explicit compatibility path after the
  adapter returns no result or an explicit fallback outcome; do not silently drop
  into legacy generation.
- Verification for Phase 4 must include existing ReAct focused tests, the new
  ToolLoop adapter tests, source guards for `allow_writes=false`, and
  `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix`.

### 5.4 Structured Routing

Preparation content:

- Define a strict route output schema:

```json
{
  "route": "direct_answer | tool_loop | plan_execute | memory_proposal | permission_request | blocked",
  "confidence": 0.0,
  "requires_tools": false,
  "requires_write": false,
  "reason": "metadata-safe short reason"
}
```

- Treat model route output as advisory unless it validates exactly.
- Keep deterministic keyword routing as fallback and as test oracle for common
  cases.
- Do not let the model provide executor arguments directly.

Current blockers:

- StrategyRouter and ExecutionPolicy are keyword/string based.
- Tool plan inference is still fragile for natural language prompts.

First implementation PR:

- Add route schema and parser tests.
- Add a model-backed route preview only behind capability-first mode.
- Keep fallback deterministic route for no-provider and parse-failure cases.

Acceptance:

- Invalid model route output fails closed to deterministic route.
- The route result is observable in trace.
- Tool executor arguments still come from governed candidates or typed builders.

Phase 5 readiness supplement:

- Phase 5 is an advisory structured-route preview phase. It must not replace
  `MainChatTurnRouteDecision` control flow until deterministic fallback and
  trace evidence prove parity.
- Gate model-backed preview behind all of these conditions:
  - `runtime_mode=capability_first_beta`;
  - provider is configured and freshly validated;
  - route is not HS-sensitive/local-only;
  - network policy allows provider invocation;
  - prompt/context can be rendered as metadata-safe bounded routing context.
- The parser must accept only an exact JSON object with the declared fields:

```json
{
  "route": "direct_answer | tool_loop | plan_execute | memory_proposal | permission_request | blocked",
  "confidence": 0.0,
  "requires_tools": false,
  "requires_write": false,
  "reason": "metadata-safe short reason"
}
```

- Reject markdown fences, arrays, extra fields, missing fields, non-finite
  confidence, confidence outside `0.0..=1.0`, unknown route labels, control
  characters, oversized reason text, unsafe reason text, and inconsistent
  `requires_tools` / `requires_write` combinations.
- The model output is advisory. On parser failure, provider failure, local-only
  policy, unvalidated provider, or low confidence, keep the deterministic route
  decision and record a typed ignored reason.
- Never let route preview include tool target, candidate id, action arguments,
  filesystem paths, raw MCP manifest descriptions, raw memory, raw LifeModel YAML,
  credentials, or executor arguments. Tool executor inputs still come only from
  governed candidates or typed builders.
- Add route preview evidence to the turn trace:
  - attempted/not attempted;
  - provider/model identity when invoked;
  - deterministic route before preview;
  - accepted advisory route or ignored reason;
  - parser status;
  - metadata-safe response digest, not raw invalid model output.
- Add route-preview tests for exact valid JSON, invalid JSON, markdown fenced
  JSON, extra fields, unknown route, unsafe reason, low confidence, local-only
  skip, unvalidated-provider skip, and deterministic fallback parity.
- Add command-surface tests proving Phase 5 does not change existing
  DirectAnswer, file read, PlanExecute, proposal, web blocker, MCP read, and
  ToolPermission proposal outcomes when preview is disabled or ignored.
- Do not introduce a new routing framework or orchestration stack. Keep the
  preview module small and owned by `MainChatTurnPipeline`.

### 5.5 Real Capability Evals

Preparation content:

- Define a small product task set that proves ability, not just governance.
- Reuse existing command-surface gates only as regression checks.
- Add deterministic fixtures for web/MCP where live provider is not available.
- Add optional live-provider runs for final confidence, not as the only proof.

Initial task set:

| Scenario | User job | Must prove |
| --- | --- | --- |
| CF-DIRECT-01 | Ask a normal question. | Direct answer, model/provider trace, no tool claim. |
| CF-FILE-01 | Read a workspace file and summarize it. | File tool executed, source preview, final synthesis. |
| CF-WEB-01 | Fetch or search a real/fixture source. | Network policy decision, observation, no fake web claim. |
| CF-MEMORY-01 | Recall relevant accepted memory. | Memory/session read evidence, no unsupported memory claim. |
| CF-MCP-01 | Use a registered read-only MCP tool. | Candidate/tool selection, observation, no write-like candidate. |
| CF-REACT-01 | Complete a two-step read plus synthesis task. | At least two observations or one tool plus follow-up synthesis. |
| CF-PROPOSAL-01 | Read then create a memory proposal if useful. | Proposal created, no direct memory write. |
| CF-PERM-01 | Hit a tool permission boundary and resume/deny. | Permission proposal or blocker, valid next user action. |

First implementation PR:

- Add scenario contract file or extend `plans/main_chat_agent_product_eval_scenarios_v1.md`.
- Add a focused runner only after the pipeline and ToolLoop labels exist.

Acceptance:

- Completion is measured by runtime objects and final delivery, not assistant
  text alone.
- Hidden fallback, silent write, and fake observation are zero-tolerance
  failures.

### 5.6 Product UI State Simplification

Preparation content:

- Define `MainChatTurnView` as the frontend user model.
- Collapse default UI states to:
  - `working`
  - `needs_permission`
  - `proposal_created`
  - `blocked`
  - `completed`
- Move Kernel/strategy/legacy/live-provider/eval terms into developer diagnostics.
- Keep expanded trace available, but make it secondary.

Current blockers:

- `ChatPage.tsx` owns too many independent pieces of state.
- Runtime evidence is available, but the default product state is too noisy.

First implementation PR:

- Add a view-model builder that maps runtime result/events to the five statuses.
- Do not redesign the full UI yet.
- Add component tests for status mapping.

Acceptance:

- A normal user can understand what the Agent is doing without reading runtime
  labels.
- Developer trace remains available for debugging.
- UI never claims read/write/proposal/permission without matching runtime data.

### 5.7 Governance Backstop And Stack Control

Preparation content:

- Freeze the minimum governance backstop.
- Mark all broader privacy/gate expansion as out of scope for the capability
  pass.
- Reuse ActionExecutor, ToolPermission, ProposalStore, and runtime facts.
- Treat `allow_writes=false` as a hard Main Chat adapter invariant for Kernel and
  ToolLoop paths. Proposal or confirmation paths may create proposal/permission
  records, but ordinary ToolLoop execution must not enable direct writes.
- Do not introduce a new agent framework or orchestration stack.
- Create a removal/deprecation map for old paths after the pipeline lands.

Current blockers:

- Many old gates and productization reports are close to the runtime surface.
- Tauri command surface mixes product commands, eval commands, migration commands,
  and debug commands.
- `ActionExecutorConfig::default()` has `allow_writes=true`, so every Main Chat
  adapter must override it deliberately.

First implementation PR:

- No broad code movement yet.
- Add a command-surface inventory and decide which commands are product,
  developer, eval-only, or deprecated.
- Add a focused invariant test or source guard that Main Chat Kernel/ToolLoop
  adapter constructors explicitly set `allow_writes=false`.
- Add guards only where they prevent user-visible confusion.

Acceptance:

- Capability work is not blocked by new governance expansion.
- Existing safety invariants still pass.
- New runtime paths do not add another parallel stack.
- Direct writes remain impossible from ordinary Main Chat ToolLoop execution even
  though capability-first mode improves model/tool ability.

## 6. Implementation Sequence

The next development phase should use this order:

1. `capability_first_beta` mode and truthful provider validation.
2. Shared `MainChatTurnRouteDecision` and send/stream route parity.
3. `MainChatTurnPipeline` wrapper with unchanged behavior.
4. ToolLoop adapter around existing ReAct execution.
5. Structured route schema and model-backed route preview.
6. Real capability scenario contract and first deterministic runner.
7. `MainChatTurnView` user-state simplification.
8. Command-surface inventory and legacy path deprecation plan.

Do not start with UI redesign. Do not start with deleting old strategy code.
Do not start with another final acceptance gate.

## 7. Definition Of Ready

Implementation may start when:

- this document is indexed from `plans/README.md`;
- the developer has read `AGENTS.md`;
- the developer has inspected the current send/stream, Kernel, ReAct strategy,
  diagnostics, config, ActionExecutor, and ChatPage code;
- the preparation evidence register below has been reviewed and updated if HEAD
  has moved;
- the first PR is scoped to one of the sequence items above;
- the PR has explicit negative assertions for hidden fallback and silent writes
  when it touches execution behavior.

## 8. Stop Conditions

Stop and reassess if:

- a change requires a second runtime stack;
- send and stream behavior diverge;
- ToolLoop behavior starts relying on assistant prose instead of tool/action
  evidence;
- capability-first mode disables the minimum governance backstop;
- structured routing lets the model provide unsafe executor arguments;
- UI simplification hides fallback, blocker, proposal, or permission states;
- a PR tries to make Main Chat Agent Execution v1 complete by changing docs only.

## 9. Preparation Evidence Register

This register records the concrete anchors used for this preparation. If the
baseline commit changes before implementation starts, refresh the affected rows.

| Area | Evidence checked | Why it matters |
| --- | --- | --- |
| Baseline | `git rev-parse --short HEAD` returned `08dcb8a`. | The preparation document names the actual baseline. |
| Worktree | `git status --short --branch` showed only this preparation doc and `plans/README.md` changed. | No runtime code was changed during preparation. |
| Send path | `src-tauri/src/main_chat_send.rs:41` through legacy fallback path at `src-tauri/src/main_chat_send.rs:153`. | Confirms route duplication and fallback path. |
| Stream path | `src-tauri/src/main_chat_streaming.rs:52` through strategy call at `src-tauri/src/main_chat_streaming.rs:237`. | Confirms stream mirrors send decisions with transport differences. |
| Kernel | `src-tauri/src/main_chat_kernel.rs:2171` and read/write branches through `src-tauri/src/main_chat_kernel.rs:2477`. | Confirms Kernel is real execution capability, not just a facade. |
| ReAct | `src-tauri/src/main_chat_strategy.rs:210` and `src-tauri/src/main_chat_react_tool_selection.rs:571`. | Confirms ReAct still lives outside the target pipeline shape. |
| Router/policy | `openlife-core/src/agent/main_chat_agent_v1.rs:307` and `openlife-core/src/agent/main_chat_agent_v1.rs:455`. | Confirms keyword/string routing and policy classification. |
| Config | `openlife-core/src/config.rs:279`. | Confirms local-first and AgentLoop-off defaults. |
| Provider diagnostics | `src-tauri/src/commands/diagnostics.rs:60`. | Confirms configured and validated provider states are conflated. |
| Privacy preprocessing | `src-tauri/src/main_chat_preprocess.rs:246` and `src-tauri/src/main_chat_preprocess.rs:287`. | Confirms capability mode needs a precise privacy boundary. |
| ActionExecutor writes | `openlife-core/src/agent/action_executor/mod.rs:31`, `src-tauri/src/main_chat_react_execution.rs:123`, and `src-tauri/src/main_chat_kernel.rs:592`. | Confirms defaults allow writes, while current Main Chat callers explicitly override to false. |
| Frontend state | `frontend/src/pages/ChatPage.tsx:1120` and `frontend/src/components/MainChatExecutionEvidence.tsx:167`. | Confirms UI state is broad, but runtime-backed evidence components exist. |
| Industry docs | OpenAI Agents, OpenAI tracing/tools, LangGraph, Microsoft Agent Framework, and Temporal sources listed in Section 3. | Confirms the workflow-spine direction is externally cross-checked. |

## 10. Verification Baseline

For documentation-only preparation:

```bash
git diff --check
git diff --no-index --check /dev/null plans/main_chat_capability_first_preparation.md || test $? -eq 1
```

`git diff --check` alone does not inspect untracked files. Use the second
command while this preparation file is still untracked, or stage intent with
`git add -N plans/main_chat_capability_first_preparation.md` before running
`git diff --check`.

For the first runtime PR:

```bash
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
pnpm --dir frontend typecheck
```

For later ToolLoop/UI PRs, add focused tests for the touched module and at least
one capability scenario from Section 5.5.
