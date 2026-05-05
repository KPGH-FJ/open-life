# OpenLife vNext Execution Entry Points Map

Date: 2026-05-06  
Status: P0-3 deliverable (spec only, no code changes)  
Related: `plans/openlife_vnext_p0_p1_task_specs.md`, `plans/current_agent_runtime_audit.md`

---

## 1. Current Agent Execution Entry Points

The following table inventories every current formal/near-formal agent execution entrypoint. "Formal" means the path creates or attaches to an `AgentRun`, uses `AgentLoop` or `AgentRuntime`, or produces agent-observable side effects.

| # | Entry Point | Function/Command | Current Behavior | Creates AgentRun? | Streaming? | Fallback? | Generates Proposals? | Calls ToolRuntime/ActionExecutor? | Current Risk | vNext Runtime Mode |
|---|------------|-----------------|------------------|-------------------|------------|-----------|----------------------|----------------------------------|-------------|-------------------|
| 1 | **Chat (non-stream, AgentLoop)** | `lib.rs::send_message` → `send_message_with_agent_loop` (line 1178) | Full chat: preprocess → intent/layer → AgentLoop.run() → finalize. L1 reflex bypasses AgentLoop. | Yes (AgentRun::new_chat_run) | No | Yes (AgentLoop failure → `handle_agent_loop_fallback` calls `generate_non_stream_fallback`) | Yes (`generate_and_persist_chat_proposals` at end) | Yes (via AgentLoop → ActionExecutor) | Fallback path creates a **new** AgentRun instead of continuing the same run; L1 bypass creates AgentRun with `ModelRouteTrace.provider == "direct"` | `chat` |
| 2 | **Chat (non-stream, legacy)** | `lib.rs::send_message` (line 1024) → L2/L3 path via legacy direct generation | When `use_agent_loop` is false, bypasses AgentLoop entirely; calls `generate_non_stream` directly | Partial (AgentRun created but not wired through AgentLoop) | No | N/A (this **is** the fallback path) | Yes (same as AgentLoop path) | No (direct `scheduler.generate()` — no ActionExecutor) | Legacy code divergence; no tool execution ability; no action/observation trace | `chat` (to be removed after facade) |
| 3 | **Chat (stream, AgentLoop)** | `lib.rs::start_stream_message` → `start_stream_message_with_agent_loop` (line 1616) | Streaming chat: preprocess → AgentLoop.run_streaming() → TauriStreamingCallback emits chunks | Yes (via AgentLoop internal AgentRun) | Yes (token-level via `scheduler.generate_stream()`) | Yes (AgentLoop failure → `handle_agent_loop_fallback` → emit as single chunk) | Yes (via `generate_and_persist_chat_proposals`) | Yes (via AgentLoop → ActionExecutor) | Same fallback re-run issue as non-stream; placeholder run_id before actual execution | `stream_chat` |
| 4 | **Chat (stream, L1 reflex)** | `lib.rs::start_stream_message` (line 1872) L1 branch | Direct reflex: no model call, emits reply + AgentRun as single chunk | Yes (AgentRun::new_chat_run with ModelRouteTrace::direct) | No (single chunk emit) | No | Yes (via `finalize_chat_agent_run` → `generate_and_persist_chat_proposals`) | No | L1 reflex is a non-AgentLoop shortcut; creates an AgentRun but semantically different from full execution | `chat` (keep as explicit shortcut) |
| 5 | **Chat (stream, legacy fallback)** | `lib.rs::start_stream_message` (line 1872) → legacy streaming path | When not L1 and not AgentLoop, goes through legacy streaming path | Yes (pre-created AgentRun) | Yes (legacy streaming) | No (this is the legacy path itself) | Yes | No (legacy direct generation) | Legacy path divergence; no tool trace | `stream_chat` (to be removed) |
| 6 | **Scheduled Task** | `scheduler_runner.rs::execute_scheduled_task` (line 125) | 60s background poll of `scheduled_tasks.json`; AgentLoop.run() with Planner role + restricted toolset_allowlist | Yes (internally via AgentLoop result; persisted only if `agent_run_store` available) | No | No explicit fallback | No (no `generate_and_persist_chat_proposals`) | Yes (via AgentLoop → ActionExecutor) | AgentRun persistence is best-effort (silent drop if store unavailable); no proposal generation; no event trace | `scheduled` |
| 7 | **Proactive Suggestions** | `commands/proactive.rs::get_proactive_suggestions` | Read-only: generates proactive suggestion entries based on LifeModel/goals/state/proposal state. Does NOT execute AgentLoop. | No | N/A | N/A | No (displays suggestions, user clicks to trigger) | No | Not a formal agent execution — it's a suggestion generator. P0-3 should classify this as a read-only trigger source, not an execution path. | `proactive` (trigger only) |
| 8 | **Builder** | `commands/builder.rs::builder_step` (line 171) | Interactive LifeModel construction: LLM-driven multi-step builder. Generates LifeModel proposals for user review. | Yes (AgentRun::new_builder_run implicitly via proposal generation) | No | No | Yes (builder_create_proposals at review phase) | No (uses direct LLM calls, not AgentLoop) | Builder uses its own LLM call path, not AgentLoop/ActionExecutor; creates AgentRun for tracking but executes outside the unified runtime | `builder` |
| 9 | **Calibration** | `commands/calibration.rs::apply_calibration` (line 186), `generate_calibration_report` (line ~60) | LifeModel calibration: generates calibration report → micro-evolution changes → creates proposals | Yes (AgentRun::new_calibration_run) | No | No | Yes (calibration_create_proposals) | No (uses direct LLM calls) | Same issue as Builder: creates AgentRun but executes outside unified runtime; calibration has a direct-apply mode that bypasses proposal | `calibration` |
| 10 | **Replay** | `commands/agent.rs::replay_agent_action` (line 92) | Re-executes a blocked tool action after user grants permission | No (modifies existing run) | No | No | Yes (regenerates proposals after replay) | Yes (creates fresh ActionExecutor + ActionExecutionContext) | Replay does not create a new AgentRun or event; mutates existing run in-place; `consume_allow_once: false` prevents double-consumption | `replay` |
| 11 | **Tool Execution (direct)** | `commands/execution.rs::run_skill`, `check_tool_permission`, `grant_tool_permission` | Direct tool/skill execution or permission management outside AgentLoop | Partial (some commands create AgentRun::new_tool_execution_run) | No | No | Depends on tool | Yes (ActionExecutor) | Multiple ad-hoc paths into ActionExecutor; inconsistent AgentRun creation | `replay` / `tool_execution` |
| 12 | **Feedback Evolution** | `commands/feedback.rs::apply_feedback_evolution` | Applies feedback-derived micro-evolution to LifeModel | No | No | No | Yes (creates proposals) | No | Feedback evolution modifies LifeModel via proposals but does not create AgentRun | `calibration` (to be unified) |

---

## 2. Entry Point Count and Path Divergence

- **Total formal entrypoints**: 12
- **Paths using AgentLoop**: 3 (chat non-stream, chat stream, scheduled)
- **Paths using ActionExecutor but NOT AgentLoop**: 2 (replay, direct tool execution)
- **Paths using direct LLM calls (no AgentLoop, no ActionExecutor)**: 3 (legacy chat, builder, calibration)
- **Paths that are triggers/read-only**: 1 (proactive suggestions)
- **Paths creating AgentRun**: 8+ (chat X 3, scheduled, builder, calibration, legacy chat, L1 reflex)
- **Paths with fallback**: 2 (chat non-stream AgentLoop → legacy, chat stream AgentLoop → legacy)
- **Paths generating proposals**: 8 (all chat variants, builder, calibration, replay, feedback evolution)

Key divergence: There are **4 distinct execution topologies**:
1. AgentLoop-based (chat non-stream, chat stream, scheduled)
2. Legacy direct generation (fallback, old chat path)
3. Builder/Calibration (custom LLM + proposal path)
4. Direct tool execution (ActionExecutor only, no AgentLoop)

---

## 3. Proposed `run_agent_task` Facade Signature

The facade must unify the 4 execution topologies without forcing all callers to understand AgentLoop internals.

### 3.1 Core Facade

```rust
/// Execution mode governs which runtime strategy is used.
pub enum AgentExecutionMode {
    /// Standard conversation with optional tool use
    Chat,
    /// Streaming chat (token-level chunks via callback)
    StreamChat { callback: Arc<dyn StreamingCallback> },
    /// Scheduled background execution (limited tools, no user interaction)
    Scheduled,
    /// Proactive agent run (read-only, generates suggestions)
    Proactive,
    /// Interactive LifeModel builder
    Builder,
    /// LifeModel calibration run
    Calibration,
    /// Replay a previously blocked action
    Replay {
        original_run_id: String,
        action_id: String,
    },
    /// Direct tool execution (no model planning)
    ToolExecution,
}

/// Input to the unified facade.
pub struct AgentExecutionInput {
    pub task: AgentTask,
    pub mode: AgentExecutionMode,
    /// Optional dependencies. Some modes require stores that others don't.
    pub deps: AgentExecutionDeps,
    /// Optional streaming callback (only used by StreamChat mode)
    pub stream: Option<Arc<dyn StreamingCallback>>,
}

/// Dependencies assembled by the Tauri layer.
pub struct AgentExecutionDeps {
    pub life_model: LifeModel,
    pub tools_prompt: String,
    pub privacy_engine: PrivacyEngine,
    pub scheduler: InferenceScheduler,
    pub agent_loop_config: AgentLoopConfig,
    pub action_ctx: ActionExecutionContext<'static>,
    pub agent_run_store: Option<Arc<tokio::sync::Mutex<AgentRunStore>>>,
    pub proposal_store: Option<Arc<tokio::sync::Mutex<ProposalStore>>>,
    pub proposal_engine: Option<Arc<tokio::sync::Mutex<ProposalEngine>>>,
}

/// Unified outcome of any agent execution.
pub struct AgentExecutionOutcome {
    /// The final text reply
    pub reply: String,
    /// The created or updated AgentRun
    pub run: AgentRun,
    /// Runtime mode that was actually used
    pub actual_mode: AgentExecutionModeKind,
    /// Whether fallback was triggered
    pub fallback_used: bool,
    /// Fallback reason if applicable
    pub fallback_reason: Option<String>,
    /// Generated proposal IDs
    pub generated_proposal_ids: Vec<String>,
    /// Trace summary for audit
    pub trace_summary: ExecutionTraceSummary,
    /// Any warnings
    pub warnings: Vec<String>,
}

pub struct ExecutionTraceSummary {
    pub run_id: String,
    pub event_count: u32,
    pub model_calls: u32,
    pub tool_calls: u32,
    pub tool_blocks: u32,
    pub repairs: u32,
    pub total_latency_ms: u64,
}
```

### 3.2 Facade Implementation Sketch

```rust
async fn run_agent_task(
    input: AgentExecutionInput,
) -> Result<AgentExecutionOutcome> {
    match input.mode {
        AgentExecutionMode::Chat => run_chat_mode(input).await,
        AgentExecutionMode::StreamChat { callback } => run_stream_chat_mode(input, callback).await,
        AgentExecutionMode::Scheduled => run_scheduled_mode(input).await,
        AgentExecutionMode::Proactive => run_proactive_mode(input).await,
        AgentExecutionMode::Builder => run_builder_mode(input).await,
        AgentExecutionMode::Calibration => run_calibration_mode(input).await,
        AgentExecutionMode::Replay { .. } => run_replay_mode(input).await,
        AgentExecutionMode::ToolExecution => run_tool_execution_mode(input).await,
    }
}
```

### 3.3 Fallback Semantics

The facade should handle fallback transparently:

1. **Primary path**: AgentLoop (for Chat/StreamChat/Scheduled modes).
2. **Fallback path**: Direct `scheduler.generate()` call, logged as fallback event.
3. **Continuation**: Fallback replies are stored in the **same** AgentRun (not a new one).
4. **Trace**: `AgentExecutionOutcome.fallback_used = true` + `AgentRunEvent::fallback.started/completed`.
5. **Proposals**: Fallback path still triggers `generate_and_persist_chat_proposals` for the same run.

---

## 4. Mapping Current Entry Points to Facade Modes

| Current Entry Point | Facade Mode | Migration Notes |
|--------------------|-------------|-----------------|
| `send_message` (AgentLoop branch) | `AgentExecutionMode::Chat` | Direct migration: wrap AgentLoop.run() call |
| `send_message` (legacy branch) | `AgentExecutionMode::Chat` | Remove; fallback inside facade |
| `send_message` (L1 reflex) | `AgentExecutionMode::Chat` with special `task.layer == L1` handling | Keep as explicit shortcut before facade |
| `start_stream_message` (AgentLoop stream) | `AgentExecutionMode::StreamChat { callback }` | Direct migration |
| `start_stream_message` (legacy stream) | `AgentExecutionMode::StreamChat { callback }` | Remove; handled by fallback inside facade |
| `start_stream_message` (L1 stream) | `AgentExecutionMode::StreamChat { callback }` with L1 shortcut | Keep before facade |
| `execute_scheduled_task` (scheduler_runner) | `AgentExecutionMode::Scheduled` | Migrate to facade call; add event tracing |
| `get_proactive_suggestions` | `AgentExecutionMode::Proactive` | Currently read-only; future: execute AgentLoop with Proactive role |
| `builder_step` | `AgentExecutionMode::Builder` | Builder needs its own rendering loop; share AgentRun/Proposal path |
| `apply_calibration` / `generate_calibration_report` | `AgentExecutionMode::Calibration` | Unify with facade; remove direct-apply mode |
| `replay_agent_action` | `AgentExecutionMode::Replay { .. }` | Currently modifies run in-place; facade should create child run |
| Direct tool/skill execution | `AgentExecutionMode::ToolExecution` | Unify permission check through facade |

---

## 5. Entry Point Convergence Sequence

### 5.1 Recommended P1-7 Implementation Order

Based on risk analysis and dependency graph:

| Step | What | Why First |
|------|------|-----------|
| **Step 0** | AgentRunEvent store (P0-1, P0-2) | Trace substrate must exist before routing |
| **Step 1** | ToolRuntime enforcement (P1-1) | Hardens action execution before facade routes more calls through it |
| **Step 2** | PromptStack skeleton (P1-2, P1-3) | Prevents prompt drift when facade redirects chat → builder → calibration |
| **Step 3** | LifeModel risk matrix (P1-4) | Protects LifeModel before calibration/builder/evolution paths get facade'd |
| **Step 4** | MemoryEvidence schema (P1-5, P1-6) | Prepares evidence layer before facade routes memory ops |
| **Step 5** | Execution facade (P1-7) | After all above are stable, implement facade and migrate paths |
| **Step 6+** | Legacy path removal | Only after facade tests pass for all modes |

### 5.2 Pre-P1-7 Implementation Risks

1. **AgentRunEvent store must exist first** (P0-1, P0-2): Without events, the facade can't record trace. Fallback would remain invisible.

2. **ToolRuntime enforcement blocks must be in place**: If facade routes more callers through ActionExecutor before declarative-only filtering is enforced, stubs could become accidentally executable.

3. **Legacy direct-apply calibration paths**: `apply_calibration` has a direct-apply mode that bypasses Proposal. Facade must either disable this or wrap it.

4. **Builder has its own LLM call path**: `builder_step` uses `scheduler.generate()` directly, not AgentLoop. Facade should wrap this, not replace it wholesale.

5. **Scheduler runner has no fallback**: If scheduled AgentLoop fails, the task is marked `failed` with no alternative. Facade should add fallback for scheduled mode.

6. **Replay mutates existing run**: Currently modifies run in-place. Facade should create a child AgentRun for replay to preserve audit trail.

7. **L1 reflex is intentional shortcut**: Do not force L1 through AgentLoop. Keep as a pre-facade fast path that explicitly creates a non-AgentLoop AgentRun.

### 5.3 Non-Goals for P1-7

These should NOT be part of P1-7 facade implementation:

- Removing all legacy paths (do in follow-up PR after tests)
- Rewriting ChatPage state management
- Adding PlanMode
- Adding SubAgentRuntime
- Adding Bash/Shell
- Changing builder/calibration UX

### 5.4 Acceptance Criteria for P1-7

1. `send_message` and `start_stream_message` share the same core runtime semantics via the facade.
2. AgentLoop failure → fallback is traceable (events recorded, same AgentRun used).
3. Fallback still generates proposals for the same run.
4. Scheduled execution passes through the facade (even if implementation remains thin).
5. The facade returns `AgentExecutionOutcome` with consistent fields for all modes.
6. Existing chat tests pass without modification.
7. Event store tests pass (P0-1/P0-2).

---

## 6. Open Questions

1. Should `AgentExecutionDeps` use `Arc`-wrapped stores or `&'a` references? (Lifetime complexity vs. clone overhead)
2. Should the facade live in `openlife-core/src/agent/` or `src-tauri/src/`? (Core vs. shell concern)
3. Should L1 reflex be folded into the facade as a special mode or remain a pre-facade shortcut?
4. Should `handle_agent_loop_fallback` be inlined into the facade or remain a separate function?
5. How should `AgentExecutionOutcome.trace_summary` be derived — from AgentRunEvent store or from in-memory counts?

---

*End of P0-3 deliverable.*
