# OpenLife vNext P8 Task Specifications

Date: 2026-05-07

Status: draft

Package:

```text
Compaction, Long-Context Continuity, and Privacy-Governed Summary Trace
```

P8 turns the existing `CompactionSummary` skeleton into a governed runtime capability. The goal is to let long chats and long AgentLoop runs continue without context collapse, while preserving proposals, unresolved observations, decisions, privacy policy, and append-only trace.

P8 does **not** introduce Bash/Shell, SubAgent parallel execution, handoff execution, a ChatPage rewrite, or a model-summarizer requirement. A rule-based compaction path should land first; model-based summarization is optional and must be privacy-governed.

## Baseline

Before P8:

- `CompactionSummary` and `CompactedObservation` exist in `openlife-core/src/agent/types.rs`.
- `AgentRunEvent` exists and is used by runtime/tool/plan/governance paths.
- P7 established durable AgentSpec selection, PromptStack/ContextPolicy binding, and SummaryOnly/LocalOnly model-call governance.
- AgentLoop has a unified effective privacy policy derived from `AgentTask` override or selected `AgentSpec`.
- There is no automatic compaction trigger, no compaction event, and no AgentLoop compaction hook.

## Global Rules

- Execute exactly one P8 task spec at a time.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not bypass AgentSpec, PromptStack, ContextPolicy, AgentRunEvent, PrivacyPolicy, ToolRuntime, ActionExecutor, Proposal, or PlanExecutor.
- Compaction must preserve active proposals, unresolved tool observations, important decisions, and pending user confirmations.
- Compaction summaries and event payloads must not contain raw sensitive user text, raw LifeModel identity fields, or raw memory snippets.
- SummaryOnly cloud paths must receive sanitized messages only.
- New unit tests must not depend on real Ollama/OpenRouter/OpenAI network calls.
- Run the task-specific verification commands.
- Final reports must include changed files, tests run, results, and residual risks.

## P8-0: Documentation And Entry Sync

Goal:

Make P8 discoverable and AI-coding-ready.

Expected behavior:

- `AGENTS.md` references P8 task specs and states the current vNext phase as P8 Compaction.
- Migration plan and test matrix remain aligned with P8 scope.
- Agent coding prompts include P8 global prompt and P8 task prompts.
- P8 explicitly excludes Bash/Shell, SubAgent parallel/handoff, and ChatPage rewrite.

Allowed edit areas:

- `AGENTS.md`
- `plans/openlife_vnext_p8_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`

Constraints:

- Documentation only.
- Do not change Rust or TypeScript code.

Verification:

- `rg -n "openlife_vnext_p8_task_specs|P8-0|P8-1|P8-2|P8-3|P8-4|P8-5|P8-6|CompactionSummary|compaction.created" AGENTS.md plans`
- `git diff --name-only` contains documentation files only.

## P8-1: Compaction Trigger And Policy

Goal:

Define when an AgentLoop context should be compacted, without calling a model.

Expected behavior:

- A compaction policy can decide whether a message/context set should be compacted.
- Decision inputs include message count, estimated tokens, minimum message count, and enabled/disabled config.
- Decision output explains whether compaction should run and why.
- Token estimation is deterministic and lightweight.

Suggested implementation shape:

- Add `openlife-core/src/agent/compaction.rs`.
- Add:
  - `CompactionConfig`
  - `CompactionDecision`
  - `estimate_message_tokens(messages)`
  - `should_compact(messages, config)`

Allowed edit areas:

- `openlife-core/src/agent/compaction.rs`
- `openlife-core/src/agent/mod.rs`
- relevant focused tests

Constraints:

- No LLM calls.
- No AgentLoop behavior changes in this task.
- No persistence changes in this task.

Verification:

- `cargo test -p openlife-core agent::compaction --lib`
- `cargo check -q`

Required tests:

- disabled config never compacts.
- empty messages do not compact.
- below thresholds does not compact.
- token threshold triggers compaction.
- message count threshold triggers compaction.
- `min_messages_before_compaction` prevents premature compaction.

## P8-2: CompactionSummary Builder

Goal:

Build a `CompactionSummary` from runtime context while preserving critical state and redacting sensitive content.

Expected behavior:

- A rule-based builder creates a `CompactionSummary` without calling a model.
- The summary preserves:
  - active proposal ids
  - unresolved tool observations
  - important decisions
  - pending user confirmations/tasks
  - source message count and token estimates
- The summary redacts or summarizes:
  - obvious PII such as email/phone
  - raw LifeModel identity fields
  - raw memory snippets
  - raw sensitive user messages

Suggested implementation shape:

- Add:
  - `CompactionInput`
  - `CompactionSummaryBuilder`
  - `build_rule_based(input) -> CompactionSummary`
- Extend `CompactionSummary` only if needed, for example:
  - `preserved_decisions`
  - `pending_task_summaries`
  - `source_message_count`
  - `privacy_policy`

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/compaction.rs`
- relevant focused tests

Constraints:

- No model summarizer in this task.
- Do not store raw sensitive content in summary fields.
- Preserve serialization compatibility where possible by using serde defaults for new fields.

Verification:

- `cargo test -p openlife-core agent::compaction --lib`
- `cargo test -p openlife-core agent::types::compaction_tests --lib`
- `cargo check -q`

Required tests:

- active proposals are preserved.
- unresolved observations are preserved.
- decisions/pending tasks are preserved.
- PII is redacted.
- raw LifeModel/memory/user sensitive text is absent from cloud-safe summary.
- summary round-trips through serde.

## P8-3: Compaction AgentRunEvent

Goal:

Record compaction as append-only runtime trace.

Expected behavior:

- Add `AgentRunEventType::CompactionCreated` serialized as `compaction.created`.
- Event payload contains metadata and the safe summary, not raw sensitive source text.
- Event store round-trips the new event.
- Frontend event type union and `RunTracePanel` support the event minimally.

Suggested event payload:

- `compaction_id`
- `run_id`
- `reason`
- `original_token_estimate`
- `compacted_token_estimate`
- `source_message_count`
- `active_proposal_count`
- `unresolved_observation_count`
- `redacted_fields`
- `privacy_policy`

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/event_store.rs`
- `frontend/src/types.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/RunTracePanel.test.tsx`
- `frontend/src/test/mocks/tauri.ts`
- relevant focused tests

Constraints:

- No large trace UI rewrite.
- Do not expose raw prompt, memory, or LifeModel text in event payloads.

Verification:

- `cargo test -p openlife-core agent::event_store --lib`
- `pnpm --dir frontend test -- --run RunTracePanel tauri`
- `pnpm --dir frontend typecheck`
- `cargo check -q`

Required tests:

- `compaction.created` serde round-trip.
- event payload excludes raw sensitive content.
- frontend type union includes `compaction.created`.
- `RunTracePanel` renders a compaction event.

## P8-4: AgentLoop Compaction Hook

Goal:

Use compacted context during long AgentLoop runs.

Expected behavior:

- AgentLoop checks the compaction policy before model generation.
- When compaction triggers:
  - builds a `CompactionSummary`
  - records `compaction.created`
  - replaces older messages with one compacted context message
  - preserves the most recent user/assistant/tool context needed for continuity
  - preserves active proposal and unresolved observation metadata
- Future generation uses the compacted context.

Allowed edit areas:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/compaction.rs`
- `openlife-core/src/agent/types.rs`
- relevant focused tests

Constraints:

- Do not lose the latest user message.
- Do not lose unresolved tool observations.
- Do not call a cloud model for compaction in this task.
- Missing event store must not panic.
- Keep changes localized to AgentLoop context preparation.

Verification:

- `cargo test -p openlife-core agent::agent_loop --lib`
- `cargo test -p openlife-core agent::compaction --lib`
- `cargo check -q`

Required tests:

- long message history triggers compaction.
- compacted message count is smaller than original.
- latest user message remains present.
- active proposal ids appear in summary metadata.
- unresolved observations appear in summary metadata.
- `compaction.created` event is recorded.
- no event store path remains safe.
- SummaryOnly compaction summary excludes raw sensitive text.

## P8-5: Optional Privacy-Governed Summarizer

Goal:

Optionally add a model-based compaction summarizer after the rule-based path is safe.

Expected behavior:

- Summarizer is optional and can fall back to rule-based compaction.
- All summarizer calls go through privacy-governed scheduler methods.
- LocalOnly never falls back to cloud.
- SummaryOnly cloud payload is sanitized.
- Summarizer failure records a warning/event and uses rule-based summary.

Allowed edit areas:

- `openlife-core/src/agent/compaction.rs`
- `openlife-core/src/scheduler.rs`
- relevant focused tests

Constraints:

- This task is optional for P8 completion unless explicitly requested.
- Do not require network-backed tests.
- Do not bypass P7 privacy governance.

Verification:

- `cargo test -p openlife-core agent::compaction --lib`
- `cargo test -p openlife-core scheduler --lib`
- `cargo check -q`

Required tests:

- LocalOnly without local model does not call cloud.
- SummaryOnly cloud payload is sanitized.
- summarizer error falls back to rule-based summary.

## P8-6: Minimal Frontend Trace Surface

Goal:

Expose compaction in trace without building a large new UI.

Expected behavior:

- Frontend recognizes `compaction.created`.
- Mock events include a realistic compaction event.
- `RunTracePanel` displays compaction events with a readable icon/summary.
- No ChatPage rewrite.

Allowed edit areas:

- `frontend/src/types.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/RunTracePanel.test.tsx`
- `frontend/src/test/mocks/tauri.ts`

Constraints:

- Minimal trace support only.
- Do not build a compaction editor or timeline redesign.

Verification:

- `pnpm --dir frontend test -- --run RunTracePanel tauri`
- `pnpm --dir frontend typecheck`

Required tests:

- compaction event renders.
- existing trace events still render.

## P8 Exit Criteria

P8 is complete when:

- P8 task specs and coding prompts are discoverable from `AGENTS.md`.
- Compaction trigger policy is deterministic and tested.
- `CompactionSummary` can be built from runtime context.
- Active proposals, unresolved observations, important decisions, and pending tasks are preserved.
- Sensitive raw content is redacted or summarized.
- `compaction.created` is recorded as append-only `AgentRunEvent`.
- AgentLoop can continue generation using compacted context.
- Frontend trace can render compaction events.
- Tests prove compaction does not require real network calls.

Recommended final verification:

- `cargo test -p openlife-core agent::compaction --lib`
- `cargo test -p openlife-core agent::agent_loop --lib`
- `cargo test -p openlife-core agent::event_store --lib`
- `cargo test -p openlife-core agent:: --lib`
- `cargo test -p openlife-tauri agent_spec --lib`
- `cargo test -p openlife-tauri plan --lib`
- `pnpm --dir frontend test -- --run RunTracePanel tauri`
- `pnpm --dir frontend typecheck`
- `cargo check -q`
