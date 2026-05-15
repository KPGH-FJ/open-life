# OpenLife Codex-Level Agent Runtime Upgrade Plan

Date: 2026-05-15

Status: draft-for-execution

Owner: OpenLife engineering / AI coding agents

## 0. Purpose

This document defines the upgrade path for OpenLife to reach a Codex / Claude Code level
Agent Runtime standard.

The goal is not to add more isolated features. The goal is to make OpenLife a trustworthy,
governed, observable, replayable, local-first personal Agent OS.

## 1. North Star

OpenLife must converge on this execution spine:

```text
User Intent
  -> AgentTask
  -> AgentRun
  -> ContextAssembler
  -> PromptStack
  -> ModelRouter
  -> ReAct Loop
  -> ToolRuntime
  -> AgentSpec / Sandbox / Permission / Privacy / NetworkPolicy
  -> Proposal / User Decision
  -> Continue / Replay / Recovery
  -> AgentRunEvent Audit
  -> MemoryEvidence
  -> LifeModel Proposal
```

Every formal Agent behavior must eventually enter this spine:

- Chat
- Streaming chat
- Builder
- Calibration
- Plan execution
- Tool execution
- Proposal apply
- Replay / continue
- Scheduled / proactive runs
- SubAgent execution

## 2. Non-Goals

These are explicitly out of scope for this upgrade:

- No project rewrite.
- No new page sprawl as a substitute for runtime convergence.
- No fake success paths.
- No production fallback that turns MCP tools into builtin tools.
- No direct LifeModel mutation from model output.
- No broad UI redesign before runtime invariants are stable.
- No weakening of tests to make CI green.

## 3. Hard Invariants

The following invariants are release blockers.

### 3.1 Governance Invariants

- Replay must restore the original governance context.
- Replay must have the same or lower authority than the original action.
- `AgentSpec`, `ExecutionSandbox`, `ToolPermissionStore`, `NetworkPolicy`, and `PrivacyPolicy`
  must not be bypassed by wrappers, fallbacks, or replay.
- Missing governance context in a formal path must fail closed.
- `agent_spec: None` is allowed only in tests or explicitly documented non-Agent utility paths.

### 3.2 Tool Invariants

- A tool exposed as executable must have a real executor.
- Declarative-only tools must not appear as model-callable executable tools.
- Stub tools must be disabled, hidden, or proposal-only.
- `ToolSource::Mcp` must never execute through a builtin fallback in production code.
- `mcp.call_tool` must govern the real target tool, not only the wrapper.
- Same-name MCP tools across servers must require server disambiguation.

### 3.3 Proposal / Replay Invariants

- High-risk changes must become proposals before state mutation.
- Tool permission proposals must grant permission to the exact real target scope:
  `tool_name + source + risk_level + action_type + capabilities`.
- Accepting a proposal must not silently execute work unless the user explicitly continues.
- Proposal continuation must be typed, not smuggled through error strings.
- Rejected or expired proposals must not be replayable.

### 3.4 Prompt / Memory Invariants

- Formal model calls must use `PromptStack`.
- Cloud-bound prompts must be privacy filtered or summarized.
- Memory evidence may generate LifeModel proposals, but may not directly mutate LifeModel.
- LifeModel updates must retain evidence, before/after, reason, and user decision trace.

### 3.5 Observability Invariants

- AgentRunEvent is append-only.
- Tool calls, blocks, proposals, accepts/rejects, replay, fallback, and model route decisions
  must be traceable.
- A run inspector must be able to answer: what context, what model, what tool, what policy,
  what user decision, and what result.

## 4. Current Critical Findings

These are the current P0/P1 blockers that motivated this upgrade.

### P0: Replay loses AgentSpec

`src-tauri/src/commands/agent.rs` rebuilds replay `ActionContext` with `agent_spec: None`.

Impact:

- A previously governed action can replay without the original AgentSpec.
- Permission accept can accidentally become an authority escalation.

Required outcome:

- Replay restores original AgentSpec from run / plan / task metadata.
- Missing AgentSpec fails closed for formal Agent actions.

### P0: MCP wrapper can hide real target governance

`mcp.call_tool` is checked as a wrapper, while the real target is resolved later.

Impact:

- Allowing `mcp.call_tool` can permit target tools denied by AgentSpec.

Required outcome:

- Target-level AgentSpec check after MCP target resolution.
- Wrapper allow does not imply target allow.

### P0: MCP source can fallback to builtin closure

`ToolSource::Mcp` currently has a fallback to `get_builtin_fn` if no MCP client exists.

Impact:

- Real MCP configuration errors can become fake successes.
- Tests can pass without exercising real MCP semantics.

Required outcome:

- Remove production fallback.
- Use a test-only MCP mock seam for integration tests.

### P1: MCP target resolver is duplicated / partial

Successful `mcp.call_tool` trace can resolve target scope by name only and ignore `server`.

Impact:

- Same-name tools on different servers can produce wrong trace / permission scope.

Required outcome:

- One target resolver used by execution, proposal, success trace, and replay tests.

### P1: Proposal continuation uses string protocol

`__blocked_action__:` is embedded in an error field and parsed later.

Impact:

- Internal continuation data is fragile and not type-safe.

Required outcome:

- Typed continuation result with `run_id`, `action_id`, and optional blocked action metadata.

### P1: Stub tools may be exposed as executable

Some tools return `"Beta MVP stub"` while being registered as enabled / executable.

Impact:

- Model and user may believe real work happened when it did not.

Required outcome:

- Stub capability audit.
- Fake executors become disabled, declarative-only, proposal-only, or real executors.

## 5. Execution Strategy

This upgrade must be delivered in small PR-sized batches. Do not let one Agent attempt the
entire upgrade as one giant patch.

### Batch 1: Governed Replay

Goal:

- Replay restores original governance context and fails closed when it cannot.

Scope:

- `src-tauri/src/commands/agent.rs`
- `openlife-core/src/agent/store.rs`
- `openlife-core/src/agent/types/mod.rs`
- related run / plan metadata if needed

Required tests:

- `replay_restores_original_agent_spec`
- `replay_missing_agent_spec_fails_closed`
- `replay_does_not_escalate_tool_scope`
- `accepted_tool_permission_replay_still_checks_agent_spec`

Exit criteria:

- No formal replay path uses `agent_spec: None`.
- Existing network ask -> accept -> replay still passes.
- Missing governance context is a controlled error.

### Batch 2: MCP Target Governance

Goal:

- `mcp.call_tool` governs the real target tool.

Scope:

- `openlife-core/src/agent/action_executor/execution_tools.rs`
- `openlife-core/src/agent/action_executor/tool_executor.rs`
- `openlife-core/src/mcp.rs` if resolver belongs there

Required tests:

- `mcp_call_tool_denied_target_is_blocked`
- `mcp_call_tool_allowed_wrapper_denied_target_is_blocked`
- `mcp_call_tool_allowed_target_succeeds`
- `mcp_call_tool_same_name_requires_server`

Exit criteria:

- Wrapper governance and target governance are both enforced.
- Target block emits event / observation with real target source.

### Batch 3: No Fake MCP Execution

Goal:

- MCP execution cannot silently fallback to builtin.

Scope:

- `openlife-core/src/agent/action_executor/tool_executor.rs`
- `openlife-core/src/mcp.rs`
- test utilities for MCP mock client

Required tests:

- `mcp_source_never_falls_back_to_builtin`
- `mcp_missing_server_fails`
- `network_ask_accept_replay_uses_real_mcp_client`

Exit criteria:

- `ToolSource::Mcp` with no registered client fails.
- Tests use explicit mock MCP client / transport, not builtin closure fallback.

### Batch 4: Unified MCP Resolver

Goal:

- One resolver determines the real MCP target everywhere.

Scope:

- New resolver module or method, likely near `McpRegistry` or `action_executor`.
- Replace duplicated name-only lookups.

Required tests:

- `mcp_resolver_uses_server`
- `mcp_resolver_rejects_ambiguous_same_name`
- `mcp_success_tool_scope_matches_resolved_server`
- `mcp_network_ask_proposal_scope_matches_resolved_server`

Exit criteria:

- Execution, proposal creation, success trace, and permission grant agree on source.

### Batch 5: Typed Proposal Continuation

Goal:

- Remove string-smuggled continuation data.

Scope:

- `src-tauri/src/commands/proposal.rs`
- `openlife-core/src/life_model/patch.rs` or a new proposal apply result type
- frontend Tauri types / `ProposalReviewPage`

Required tests:

- `proposal_accept_returns_typed_continuation`
- `proposal_accept_no_string_blocked_action_protocol`
- `frontend_shows_continue_from_typed_response`

Exit criteria:

- `__blocked_action__:` is removed from production code.
- Frontend still supports continue / replay.

### Batch 6: Tool Capability Audit

Goal:

- Model-visible executable tools represent real executable capability.

Scope:

- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- frontend tool capability display if needed

Required tests:

- `model_visible_tools_exclude_stubs`
- `declarative_only_tools_not_executable`
- `stub_tools_are_disabled_or_proposal_only`

Exit criteria:

- No `"Beta MVP stub"` executable tool is model-callable.
- Tool inventory documents status for each tool.

### Batch 7: Trace and Release Gate

Goal:

- Codex-level behavior is visible and release-gated.

Scope:

- AgentRunEvent coverage.
- Run detail UI / diagnostics if needed.
- Release gate docs.

Required tests:

- `agent_run_event_records_agent_spec_selected`
- `agent_run_event_records_tool_block`
- `agent_run_event_records_proposal_accept`
- `agent_run_event_records_action_replay`

Exit criteria:

- Release gate matrix passes.
- `make ci` passes.

## 6. Development Rules for Agents

All implementation Agents must follow these rules.

1. Write or update failing tests before implementation for the targeted behavior.
2. Keep changes scoped to the batch.
3. Do not weaken governance to make tests pass.
4. Do not add production fallbacks that hide runtime errors.
5. Do not hand-build proposals when the test is meant to cover real proposal generation.
6. Do not manually pre-authorize target permissions unless the test explicitly covers post-accept state.
7. Do not rely only on `"not blocked"` assertions; assert final status, output, source, and events.
8. Do not introduce ad hoc prompt strings for formal Agent calls.
9. Do not make fake executors look executable.
10. Do not remove old compatibility without a migration or documented fail-closed behavior.

## 7. Verification Commands

Every batch must run at least:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
make ci
```

Targeted tests must be listed explicitly in the final report.

## 8. Batch Completion Report Template

Every Agent must report:

```text
Batch:
Files changed:
Runtime behavior changed:
Tests added:
Targeted tests run:
Full validation:
Known residual risk:
Any governance boundary touched:
Any compatibility behavior changed:
```

## 9. Definition of Done

The upgrade is complete only when:

- The acceptance matrix passes.
- No P0/P1 item remains open.
- `make ci` passes.
- Governance regression tests fail when intentionally reverting the fix.
- Tool inventory is truthful.
- Release gate is documented and green.

