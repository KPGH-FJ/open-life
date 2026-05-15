# OpenLife Codex-Level Acceptance Matrix

Date: 2026-05-15

Status: draft-for-execution

This matrix defines the required behavior for OpenLife to claim Codex / Claude Code level
Agent Runtime quality.

Passing `make ci` is necessary but not sufficient. The behavioral rows below are release gates.

## 1. Global Gates

| Gate | Required Result | Evidence |
|------|-----------------|----------|
| Formatting | `cargo fmt --check` passes | CI log |
| Rust lint | `cargo clippy --workspace --all-targets -- -D warnings` passes | CI log |
| Rust tests | `cargo test --workspace` or documented equivalent passes | CI log |
| Frontend tests | `pnpm test` passes | CI log |
| Frontend build | `pnpm build` passes | CI log |
| No fake success | No production path returns success for unimplemented tool execution | Code audit + tests |
| No silent governance bypass | Replay / wrapper / fallback cannot bypass AgentSpec, Sandbox, Permission, Privacy, NetworkPolicy | Governance tests |
| Traceability | Formal runs emit enough AgentRunEvent data to reconstruct model/tool/proposal/replay decisions | Event tests |

## 2. Runtime Governance Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| GOV-001 | Replay of governed action | Restores original AgentSpec | `replay_restores_original_agent_spec` |
| GOV-002 | Replay with missing original AgentSpec | Fails closed | `replay_missing_agent_spec_fails_closed` |
| GOV-003 | Replay after accepted permission | Still checks AgentSpec | `accepted_tool_permission_replay_still_checks_agent_spec` |
| GOV-004 | Replay action scope differs from granted scope | Fails permission check | `replay_does_not_escalate_tool_scope` |
| GOV-005 | Plan retry after permission accept | Uses plan-bound AgentSpec | `plan_retry_restores_plan_agent_spec` |
| GOV-006 | SubAgent tool call | Uses child AgentSpec, not parent authority | `sub_agent_tool_uses_child_spec` |

## 3. MCP Target Governance Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| MCP-001 | `mcp.call_tool` wrapper allowed, target denied | Blocked | `mcp_call_tool_allowed_wrapper_denied_target_is_blocked` |
| MCP-002 | `mcp.call_tool` wrapper allowed, target allowed | Succeeds | `mcp_call_tool_allowed_target_succeeds` |
| MCP-003 | `mcp.call_tool` target denied by AgentSpec | Blocked before execution | `mcp_call_tool_denied_target_is_blocked` |
| MCP-004 | Same target name on two servers, no server provided | Fails with disambiguation error | `mcp_call_tool_same_name_requires_server` |
| MCP-005 | Same target name on two servers, server provided | Uses that exact server | `mcp_resolver_uses_server` |
| MCP-006 | Success trace for MCP target | `tool_scope.source` matches resolved server | `mcp_success_tool_scope_matches_resolved_server` |
| MCP-007 | Network ask proposal for MCP target | Proposal source/risk/action/capabilities match target | `mcp_network_ask_proposal_scope_matches_resolved_server` |

## 4. MCP Execution Truthfulness Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| EXEC-001 | `ToolSource::Mcp` with missing server | Failed, no builtin fallback | `mcp_missing_server_fails` |
| EXEC-002 | MCP manifest and same-name builtin closure exist | MCP does not execute builtin closure | `mcp_source_never_falls_back_to_builtin` |
| EXEC-003 | MCP mock client configured | Executes through MCP client seam | `network_ask_accept_replay_uses_real_mcp_client` |
| EXEC-004 | MCP client returns tool error | Action failed with surfaced error | `mcp_client_error_surfaces` |
| EXEC-005 | MCP client timeout | Action failed or timed out with trace | `mcp_client_timeout_records_event` |

## 5. NetworkPolicy / Proposal / Replay Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| NPR-001 | Network target with `default_decision=ask` | Creates ToolPermission proposal | `network_policy_ask_creates_tool_permission_proposal` |
| NPR-002 | Accept network ask proposal | Grants exact real target scope | `network_policy_accept_grants_exact_target_scope` |
| NPR-003 | Replay after accept | Replays original wrapper action and succeeds | `network_ask_accept_replay_succeeds` |
| NPR-004 | Proposal rejected | Replay not available | `network_ask_reject_disables_replay` |
| NPR-005 | Proposal expired | Replay not available | `network_ask_expired_disables_replay` |
| NPR-006 | Hard deny | Does not create accept/replay proposal | `network_policy_deny_hard_blocks` |

## 6. Typed Continuation Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| CONT-001 | Accept replayable ToolPermission proposal | Returns typed continuation | `proposal_accept_returns_typed_continuation` |
| CONT-002 | Accept non-replayable proposal | No continuation | `proposal_accept_without_replay_has_no_continuation` |
| CONT-003 | Internal apply result | Does not use `__blocked_action__:` string protocol | `proposal_accept_no_string_blocked_action_protocol` |
| CONT-004 | Frontend receives continuation | Shows continue action | `frontend_shows_continue_from_typed_response` |
| CONT-005 | Replay fails | Frontend shows failure without hiding error | `frontend_replay_failure_is_visible` |

## 7. Tool Capability Truth Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| TOOL-001 | Model-visible tools prompt | Excludes stub executors | `model_visible_tools_exclude_stubs` |
| TOOL-002 | Declarative-only tool execution attempt | Blocked / proposal-only | `declarative_only_tools_not_executable` |
| TOOL-003 | Stub tool registration | Disabled, hidden, or proposal-only | `stub_tools_are_disabled_or_proposal_only` |
| TOOL-004 | Real file read | Safe path enforced | `file_read_safe_path_required` |
| TOOL-005 | Real file write | Proposal-first | `file_write_requires_external_write_proposal` |
| TOOL-006 | Shell run | Default off and sandboxed | `shell_run_default_off_and_sandboxed` |

## 8. PromptStack Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| PRM-001 | Formal chat model call | Uses PromptStack | `chat_model_call_uses_prompt_stack` |
| PRM-002 | Fallback model call | Uses PromptStack or records explicit legacy exception | `fallback_model_call_prompt_stack_trace` |
| PRM-003 | Tool repair prompt | PromptStack block recorded | `tool_repair_prompt_stack_recorded` |
| PRM-004 | Cloud summary-only route | Raw LifeModel not leaked | `summary_only_cloud_route_excludes_raw_lifemodel` |
| PRM-005 | Tool prompt | Excludes unavailable / declarative-only tools | `tool_prompt_excludes_unavailable_tools` |

## 9. Memory / LifeModel Evolution Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| MEM-001 | Repeated preference detected | Creates evidence-backed proposal | `repeated_preference_generates_evidence_proposal` |
| MEM-002 | High-risk identity update | Requires explicit proposal accept | `high_risk_lifemodel_update_requires_accept` |
| MEM-003 | Rejected proposal | Reduces near-term repeat proposal likelihood | `rejected_proposal_affects_evidence_scoring` |
| MEM-004 | Contradictory evidence | Creates conflict / asks user, no confident patch | `contradiction_does_not_auto_patch` |
| MEM-005 | Accepted LifeModel update | Links before/after/evidence/user decision | `lifemodel_update_has_evidence_trace` |

## 10. AgentRunEvent Trace Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| EVT-001 | AgentSpec selected | Event recorded | `agent_run_event_records_agent_spec_selected` |
| EVT-002 | PromptStack built | Event records block metadata | `agent_run_event_records_prompt_stack` |
| EVT-003 | Tool call requested | Event recorded | `agent_run_event_records_tool_call_requested` |
| EVT-004 | Tool call blocked | Event records policy reason | `agent_run_event_records_tool_block` |
| EVT-005 | Proposal created | Event linked to run/action | `agent_run_event_records_proposal_created` |
| EVT-006 | Proposal accepted | Event linked to proposal and continuation | `agent_run_event_records_proposal_accept` |
| EVT-007 | Action replayed | Event links original and replayed action | `agent_run_event_records_action_replay` |
| EVT-008 | Fallback used | Event records original error and fallback route | `agent_run_event_records_fallback` |

## 11. Frontend Acceptance Matrix

| ID | Scenario | Expected Behavior | Required Test |
|----|----------|-------------------|---------------|
| UI-001 | ToolPermission accept returns continuation | User sees continue button | `proposal_review_continue_button_visible` |
| UI-002 | Continue clicked | Calls replay command with run/action id | `proposal_review_calls_replay_agent_action` |
| UI-003 | Replay success | Shows success state | `proposal_review_replay_success_visible` |
| UI-004 | Replay failure | Shows exact failure reason | `proposal_review_replay_failure_visible` |
| UI-005 | Tool source is MCP | UI displays `mcp:server` source | `proposal_review_displays_mcp_source` |
| UI-006 | High-risk proposal | Batch accept disabled | existing or new high-risk test |

## 12. Release Decision

OpenLife cannot claim Codex-level Agent Runtime readiness until:

- All P0 rows pass.
- All P1 rows either pass or have an accepted ADR with a release-safe mitigation.
- `make ci` passes.
- Tool inventory reflects true executable status.
- Release gate document links to evidence for each row.

