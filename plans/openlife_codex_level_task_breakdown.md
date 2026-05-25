# OpenLife Codex-Level Task Breakdown

Date: 2026-05-15

Status: ready-for-agent-assignment

This document breaks the Codex-level upgrade into Agent-sized tasks. Each task should be
implemented and reviewed independently.

## 1. Operating Rules

Every Agent must:

1. Read:
   - `AGENTS.md`
   - `plans/openlife_codex_level_upgrade_plan.md`
   - `plans/openlife_codex_level_acceptance_matrix.md`
   - this file
2. Work on one batch only.
3. Write or update targeted tests before implementation.
4. Avoid broad refactors.
5. Preserve user changes and unrelated local edits.
6. Run targeted tests and `make ci`.
7. Report residual risks.

Every reviewer must:

1. Review behavior first, not only code shape.
2. Verify tests would fail on the old bug.
3. Check for fake success or silent fallback.
4. Check governance cannot be bypassed by replay or wrappers.

## 2. Priority Map

| Priority | Batch | Why |
|----------|-------|-----|
| P0 | Governed Replay | Replay can become authority escalation if governance context is lost |
| P0 | MCP Target Governance | Wrapper allow can hide denied real target |
| P0 | No Fake MCP Execution | Production fallback can fake MCP success |
| P1 | Unified MCP Resolver | Same-name tools can corrupt permission/trace source |
| P1 | Typed Continuation | String protocol is fragile and non-production quality |
| P1 | Tool Capability Audit | Model-visible fake tools damage trust |
| P2 | Trace / Release Gate | Required for product confidence and future releases |

## 3. Batch 1: Governed Replay

### Goal

Ensure replay restores original governance context and never escalates authority.

### Files to inspect

- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/plan.rs`
- `openlife-core/src/agent/types/mod.rs`
- `openlife-core/src/agent/store.rs`
- `openlife-core/src/agent/plan_store.rs`
- `openlife-core/src/agent/agent_spec_store.rs`

### Required failing tests first

- `replay_restores_original_agent_spec`
- `replay_missing_agent_spec_fails_closed`
- `accepted_tool_permission_replay_still_checks_agent_spec`
- `replay_does_not_escalate_tool_scope`

### Implementation notes

- Do not pass `agent_spec: None` in formal replay paths.
- Resolve the original AgentSpec from `AgentRun.agent_spec_id`, plan-bound spec, or task metadata.
- If the run is old and has no AgentSpec, fail closed unless an explicit compatibility rule says it is a non-Agent utility run.
- Record replay governance metadata in AgentRunEvent if event store exists.

### Done when

- Replaying a previously blocked action uses the same AgentSpec as the original run.
- A deny spec still blocks after permission accept.
- Missing governance context returns a controlled permission/config error.

## 4. Batch 2: MCP Target Governance

### Goal

`mcp.call_tool` must enforce governance on the real target tool.

### Files to inspect

- `openlife-core/src/agent/action_executor/execution_tools.rs`
- `openlife-core/src/agent/action_executor/tool_executor.rs`
- `openlife-core/src/agent/types/mod.rs`
- `openlife-core/src/mcp.rs`

### Required failing tests first

- `mcp_call_tool_denied_target_is_blocked`
- `mcp_call_tool_allowed_wrapper_denied_target_is_blocked`
- `mcp_call_tool_allowed_target_succeeds`
- `mcp_call_tool_target_block_records_real_source`

### Implementation notes

- Wrapper check is not enough.
- After resolving MCP target manifest, check `AgentSpec::is_tool_allowed` against the real target.
- The block result must identify the target tool and source, not only `mcp.call_tool`.
- Ensure NetworkPolicy and permission checks still use the real target scope.

### Done when

- Allowing `mcp.call_tool` does not allow denied targets.
- Denied MCP target does not execute.
- Trace / observation identifies the real denied target.

## 5. Batch 3: No Fake MCP Execution

### Goal

Remove production fallback from MCP source to builtin closure.

### Files to inspect

- `openlife-core/src/agent/action_executor/tool_executor.rs`
- `openlife-core/src/mcp.rs`
- existing MCP tests in `tool_executor.rs`
- Tauri proposal replay tests

### Required failing tests first

- `mcp_source_never_falls_back_to_builtin`
- `mcp_missing_server_fails`
- `network_ask_accept_replay_uses_real_mcp_client`

### Implementation notes

- Remove `ToolSource::Mcp` -> `get_builtin_fn` production fallback.
- Introduce a test-only MCP mock path if needed.
- Tests must verify a missing MCP client fails even if a same-name builtin closure exists.
- Existing replay tests must be updated to exercise real MCP semantics through the mock seam.

### Done when

- MCP missing server fails.
- MCP test success is not produced by builtin closure fallback.
- Network ask -> accept -> replay still succeeds through MCP mock client.

## 6. Batch 4: Unified MCP Resolver

### Goal

Use one resolver for all MCP target decisions.

### Files to inspect

- `openlife-core/src/mcp.rs`
- `openlife-core/src/agent/action_executor/execution_tools.rs`
- `openlife-core/src/agent/action_executor/tool_executor.rs`

### Required failing tests first

- `mcp_resolver_uses_server`
- `mcp_resolver_rejects_ambiguous_same_name`
- `mcp_success_tool_scope_matches_resolved_server`
- `mcp_network_ask_proposal_scope_matches_resolved_server`

### Implementation notes

- Resolver input: `tool_name`, optional `server`.
- Resolver output: exact `ToolManifest` or typed error.
- If multiple matching MCP tools exist and no server is provided, return disambiguation error.
- Replace name-only target lookups.

### Done when

- Execution, proposal, success trace, and replay agree on the same source.
- Same-name target ambiguity is impossible to silently ignore.

## 7. Batch 5: Typed Proposal Continuation

### Goal

Remove `__blocked_action__:` string protocol.

### Files to inspect

- `src-tauri/src/commands/proposal.rs`
- `frontend/src/tauri.ts`
- `frontend/src/pages/ProposalReviewPage.tsx`
- `openlife-core/src/life_model/patch.rs`
- possible proposal result types in `openlife-core/src/agent/types/mod.rs`

### Required failing tests first

- `proposal_accept_returns_typed_continuation`
- `proposal_accept_no_string_blocked_action_protocol`
- `frontend_shows_continue_from_typed_response`
- `frontend_replay_failure_is_visible`

### Implementation notes

- Add a typed result:

```text
ProposalApplyContinuation {
  run_id: String,
  action_id: String,
  blocked_action: Option<Value>
}
```

- Return it from proposal apply / accept as structured data.
- Preserve frontend aliases temporarily if needed, but do not use error string smuggling internally.

### Done when

- `rg "__blocked_action__" src-tauri openlife-core frontend` returns no production hits.
- Continue button behavior still works.

## 8. Batch 6: Tool Capability Audit

### Goal

Make tool inventory truthful.

### Files to inspect

- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/agent/action_executor/*`
- `plans/openlife_vnext_tool_inventory.md`

### Required failing tests first

- `model_visible_tools_exclude_stubs`
- `declarative_only_tools_not_executable`
- `stub_tools_are_disabled_or_proposal_only`

### Implementation notes

- Find all `"Beta MVP stub"` executors.
- If no real executor exists, mark the tool as disabled, declarative-only, or proposal-only.
- Update tool inventory document.
- Do not remove useful future manifests; make their runtime status truthful.

### Done when

- Model tools prompt contains no fake executable capability.
- Tool inventory matches runtime behavior.

## 9. Batch 7: Trace and Release Gate

### Goal

Make behavior observable enough for release decisions.

### Files to inspect

- `openlife-core/src/agent/event_store.rs`
- `openlife-core/src/agent/types/mod.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/proposal.rs`
- frontend run detail / trace panels

### Required failing tests first

- `agent_run_event_records_agent_spec_selected`
- `agent_run_event_records_tool_block`
- `agent_run_event_records_proposal_created`
- `agent_run_event_records_proposal_accept`
- `agent_run_event_records_action_replay`

### Implementation notes

- Add missing events only where they are useful for reconstructing decisions.
- Avoid noisy event spam.
- Link proposal / action / run ids.
- Create a final release gate document after tests pass.

### Done when

- A reviewer can reconstruct a permission/replay flow from events.
- Release gate references evidence for P0/P1 rows.

## 10. Phase 2: Execution Path Convergence

After the P0/P1 governance batches, the next Codex-level phase is Tauri-side execution
path convergence.

Read before assigning implementation:

- `plans/openlife_vnext_execution_entrypoints.md`
- `plans/openlife_codex_level_phase2_execution_facade_prep.md`

First implementation batch:

- Add `src-tauri/src/execution_facade.rs`.
- Add typed facade mode/input/outcome structures.
- Centralize AgentLoop and ActionContext assembly helpers.
- Keep behavior unchanged while preparing migration.
- Do not migrate Builder, Calibration, Replay, or frontend UX in the first batch.

Required targeted tests:

- `execution_facade_builds_action_context_with_agent_spec`
- `execution_facade_builds_action_context_with_sandbox_from_config`
- `execution_facade_non_stream_chat_matches_existing_agentloop_config`
- `execution_facade_scheduled_mode_uses_restricted_toolset`

Validation:

```bash
cargo fmt --check
cargo clippy -p openlife-tauri -- -D warnings
cargo test -p openlife-tauri execution_facade -- --nocapture
cargo test -p openlife-tauri scheduler -- --nocapture
cargo test -p openlife-tauri replay -- --nocapture
make ci
```

## 11. Agent Prompt Template

Use this template for each batch.

```text
You are working in /Users/fujing/Desktop/偶来福.

Read first:
- AGENTS.md
- plans/openlife_codex_level_upgrade_plan.md
- plans/openlife_codex_level_acceptance_matrix.md
- plans/openlife_codex_level_task_breakdown.md

You are assigned Batch <N>: <Batch Name>.

Do not work on other batches.
Do not rewrite the project.
Do not weaken governance.
Do not introduce fake success paths.
Write failing tests first for the required scenarios in the task breakdown.
Then implement the smallest correct change.

After implementation, run:
- targeted tests for this batch
- cargo fmt --check
- cargo clippy --workspace --all-targets -- -D warnings
- make ci

Final report must include:
- Files changed
- Tests added
- Targeted tests run
- make ci result
- Governance boundaries touched
- Residual risks
```

## 12. Reviewer Checklist

Before accepting any batch:

- Does the test fail on the old bug?
- Does the implementation preserve or strengthen governance?
- Did the Agent avoid production fake fallback?
- Did replay preserve original context?
- Did wrapper tools govern real targets?
- Are errors explicit rather than silently converted to success?
- Are traces/audit sufficient to explain the behavior?
- Does `make ci` pass?
