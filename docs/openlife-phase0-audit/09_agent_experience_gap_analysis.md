# Agent Experience Gap Analysis

## Intent Expression

Finding: Users can type natural language, select skills, and use quick/product
routes, but intent capture is not yet a first-class UX object.

Evidence:

- Chat input sends raw message lists and optional selected skill id.
- Backend builds `IntentFrame`, but frontend does not present an editable or
  confirmable intent frame before consequential work.

File location:

- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/chat/ChatInputArea.tsx`
- `openlife-core/src/agent/main_chat_agent_v1.rs`

Confidence: High.

Impact: V2 should make "what OpenLife thinks you want" visible and correctable.

## Understanding Visibility

Finding: Reasoning and trace data exist, but they are closer to diagnostics
than everyday user comprehension.

Evidence:

- `ReasoningTracePanel`, `RunTracePanel`, tool cards, kernel events, and agent
  events exist.
- Chat tracks route decision, execution transcript, agent state, kernel events,
  and durable events.

File location:

- `frontend/src/components/ReasoningTracePanel.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/ToolCallCard.tsx`
- `frontend/src/pages/ChatPage.tsx`

Confidence: High.

Impact: V2 needs a concise agent state timeline for normal use and a deeper
audit drawer for advanced inspection.

## Planning Visibility

Finding: Plan execution primitives exist, but planning is not yet the dominant
visible experience.

Evidence:

- Tauri bridge exposes create/update/finalize/review/execute/skip plan-execute
  commands.
- Agent state snapshot includes optional plan artifact view.
- Chat page integrates task continuity and plan state, but this audit did not
  verify an end-to-end plan journey.

File location:

- `frontend/src/tauri.ts`
- `frontend/src/pages/ChatPage.tsx`
- `src-tauri/src/commands/agent_runtime/plan_execute_product.rs`

Confidence: Medium.

Impact: Planning should become a structured workspace object, not just answer
text.

## Execution Transparency

Finding: Execution transparency primitives are strong but not yet simplified
for product use.

Evidence:

- Tool calls include status, permission level, action id, run id, permission
  decision, and ReAct trace.
- Kernel events include tool decision and tool observation.
- Final delivery includes completed actions, observations, proposals, blockers,
  pending user actions, and durable changes.

File location:

- `frontend/src/tauri.ts`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`

Confidence: High.

Impact: V2 should preserve this evidence but present it as a staged run
timeline with clear user controls.

## User Control

Finding: Review Center, tool permission, safe mode, danger preflight, retry,
resume, cancel, and proposal controls exist.

Evidence:

- Mailbox supports accept, reject, edit, postpone, safe-mode blocking, safe-path
  checks, and task resume after proposal review.
- Chat exposes task resume/cancel/retry controls through Tauri functions.
- Settings exposes danger action preflight surfaces.

File location:

- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `src-tauri/src/commands/settings.rs`

Confidence: High.

Impact: The next UX should make control predictable and central, not scattered.

## Memory Experience

Finding: Memory has strong backend semantics but a fragmented product
experience.

Evidence:

- Memory lanes and proposal thresholds exist in `MemoryGateway`.
- Mailbox reviews proposals.
- LifeModel page and memory search are separate surfaces.
- Chat and Today consume some memory/projection state but do not make memory
  lane status obvious.

File location:

- `openlife-core/src/memory_gateway.rs`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/MemorySearch.tsx`
- `frontend/src/pages/ChatPage.tsx`

Confidence: High.

Impact: V2 needs a memory model the user can inspect: context-only, proposed,
accepted, materialized, rolled back.
