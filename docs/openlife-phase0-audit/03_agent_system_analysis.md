# Agent System Analysis

## Lifecycle

Finding: Ordinary Main Chat starts with `AgentIngress`, creates or resumes a
task session, records transcript entries, routes through `OpenLifeTurnRuntime`,
then delegates to the Main Chat kernel for execution.

Evidence:

- `start_main_chat_agent_turn` constructs `AgentIngress::default()` and calls
  `decide`.
- It creates/resumes `AgentTaskSession` records and appends route-decision
  transcript entries.
- `OpenLifeTurnRuntime::run_with_event_sink` calls
  `run_main_chat_kernel_direct_answer_with_state`.

File location:

- `src-tauri/src/main_chat_runtime_support.rs`
- `openlife-core/src/agent/main_chat_agent_v1.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_kernel.rs`

Confidence: High.

Impact: The lifecycle is explicit and auditable, but kernel delegation remains
central.

## State Transitions

Finding: Agent task state includes running, waiting permission, blocked,
completed, failed, and cancelled states.

Evidence:

- `AgentTaskSessionStatus` defines these states.
- `LifeStateProjection` aggregates running, waiting permission, blocked,
  failed, cancelled, completed, and active counts.
- Chat UI reacts to failed, waiting permission, generated proposals, running,
  and completed run states.

File location:

- `openlife-core/src/agent/main_chat_agent_v1.rs`
- `src-tauri/src/life_state_projection.rs`
- `frontend/src/pages/ChatPage.tsx`

Confidence: High.

Impact: Product UI can present richer task lifecycle than simple chat bubbles.

## Events and Execution Flow

Finding: The kernel emits structured turn events and the frontend consumes both
kernel events and durable agent events.

Evidence:

- `MainChatKernelEvent` includes turn started, context loaded, route selected,
  final answer, tool decision, tool observation, write intent decision, and
  blocker.
- Streaming emits `main-chat-kernel-event`, `stream-message-start`,
  `stream-message-chunk`, `stream-message-done`, and durable
  `main-chat-agent-event`.
- Chat UI tracks event gaps and falls back to snapshot refresh when replay is
  needed.

File location:

- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `frontend/src/pages/ChatPage.tsx`

Confidence: High.

Impact: Execution transparency primitives exist and should be first-class in
the redesign.

## Failure Handling

Finding: Failure handling is increasingly fail-closed.

Evidence:

- `finalize_openlife_turn_result` marks fallback usage as `failed`, blockers
  as `blocked`, and proposals as `completed_with_pending_items`.
- ToolGateway returns blocked results with `toolGatewayAuthority` and
  `directWritesExecuted: false`.
- Main Chat command-surface tests cover blocked network/file paths and no
  legacy fallback success claims.

File location:

- `src-tauri/src/main_chat_turn_runtime.rs`
- `openlife-core/src/agent/tool_gateway.rs`
- `src-tauri/src/main_chat_command_surface_tests.rs`

Confidence: High.

Impact: The frontend should preserve blocked/pending/failed distinctions and
avoid flattening them into "done".

## Feedback Loop

Finding: Feedback, maturation, accepted guidance, evidence stores, and memory
lifecycle components exist, but this audit did not verify a complete user
feedback-to-better-future-behavior loop in the desktop product.

Evidence:

- Core modules include `maturation`, `accepted_guidance`,
  `heuristic_store`, `evidence_store`, and `memory_lifecycle`.
- Frontend includes feedback and review surfaces, but no live journey was run in
  this audit.

File location:

- `openlife-core/src/agent/maturation.rs`
- `openlife-core/src/agent/accepted_guidance.rs`
- `openlife-core/src/agent/evidence_store.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `frontend/src/pages/MailboxPage.tsx`

Confidence: Medium.

Impact: Mark feedback loop as `PARTIAL` until an end-to-end product trial proves
it.
