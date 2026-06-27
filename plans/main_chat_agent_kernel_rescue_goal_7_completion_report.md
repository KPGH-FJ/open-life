# Main Chat Kernel Rescue Goal Completion Report

> Goal: 7 - Web, MCP, And Provider Capability Restoration
> Branch: rescue/main-chat-kernel-goal-7
> Date: 2026-06-22
> Base commit: recorded in branch history before this report is committed.
> Final commit: recorded in branch history after this report is committed.
> Author/agent: Codex

## Objective

Restore governed web read/blocker behavior, registered MCP read, MCP
ToolPermission proposal evidence, permission replay, deterministic MCP
selection, and opt-in live-provider harness coverage on top of
MainChatKernel, without making live provider credentials a dependency of
normal local readiness.

## Scope Actually Changed

| File | Change type | Why it was needed |
| --- | --- | --- |
| `src-tauri/src/main_chat_kernel.rs` | Kernel web/MCP restoration | Added ActionExecutor-backed `web.search` / `web.fetch`, registered MCP read resolution, exact manifest identity selection, deterministic bounded candidate metadata, permission-proposal linking to exact pending action identity, proposal-store attachment, waiting-permission session handling, and consistent read blocker metadata. |
| `src-tauri/src/main_chat_command_surface_tests.rs` | Command-surface evidence | Updated send/stream web and MCP tests to assert kernel-backed tool-loop evidence, strict MCP manifest identity, deterministic candidate order/rank, bounded allowlists, no provider-ranking dependency, and no direct writes. |
| `src-tauri/src/main_chat_command_surface_eval.rs` | Eval acceptance bridge | Updated command-surface eval assertions so web/MCP AgentLoop scenarios accept the kernel-backed read loop only when the required blocker, read, selection, and ToolPermission metadata is present. |
| `src-tauri/src/main_chat_final_gate.rs` | Live-provider audit hardening | Ensures attempted harness reports with raw unsafe provider model identity emit `live_provider_model_identity_missing` even when the unsafe identity prevents model-invocation credit. |
| `plans/main_chat_agent_kernel_rescue_goal_7_completion_report.md` | Added report | Records Goal 7 acceptance, verification evidence, explicit provider-ranking deferral, live-provider opt-in status, and residual risk. |

## Acceptance Checklist

- [x] Web read success or network-policy blocker is explicit on send and stream.
- [x] Registered MCP read uses exact manifest identity and bounded arguments.
- [x] MCP ToolPermission proposal links to the exact pending action.
- [x] Accepted ToolPermission replay uses the original pending action identity.
- [x] Multi-candidate MCP selection is bounded and deterministic before provider ranking.
- [x] Provider-ranked preselection is not required for local completion and is explicitly deferred.
- [x] External live-provider proof remains opt-in and outside normal local readiness.

## Acceptance Matrix Rows

| ID | Evidence |
| --- | --- |
| K7-01 | `main_chat_kernel_goal_3_web_read_unavailable_send_stream_blocks_without_fake_success`, `send_message_web_policy_blocker_completes_through_agent_loop_not_fallback`, `start_stream_message_web_policy_blocker_completes_through_agent_loop_not_fallback`, and the command-surface eval gate passed. The kernel now plans `web.search` through ActionExecutor, records `network_policy_blocked` in `blockerReason`, and the eval gate also covers fixture-backed web success without counting it as real external live/provider evidence. |
| K7-02 | `send_message_command_surface_preserves_registered_mcp_read_success`, `start_stream_message_command_surface_preserves_registered_mcp_read_success`, `send_message_registered_mcp_read_completes_through_agent_loop_not_fallback`, `start_stream_message_registered_mcp_read_completes_through_agent_loop_not_fallback`, and the eval gate passed. MCP reads record exact `manifestId`, `manifestName`, `manifestSource`, `selectedCandidateId`, `selectedCandidateTarget`, `strictManifestIdentity=true`, `fuzzyNameMatchingUsed=false`, bounded governed arguments, and `directWritesExecuted=false`. |
| K7-03 | `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix` passed with both `mcp_tool_permission_proposal_coverage` and `mcp_agent_loop_tool_permission_proposal_coverage` above the two-case threshold. Kernel permission metadata includes `blockedAction`, `pendingActionIdentity`, `proposalId`, `permissionProposalLinkedToPendingAction=true`, and the Review Center proposal is tied to the task session. |
| K7-04 | `cargo test -p openlife-tauri main_chat_task_control -- --nocapture` passed, including `resume_main_chat_task_replays_pending_action_after_tool_permission_acceptance` and `resume_main_chat_task_does_not_replay_tool_permission_when_scope_target_changed`. This proves accepted permission replays the original pending action and preserves the blocker on target mismatch. |
| K7-05 | `send_message_registered_mcp_multi_candidate_agent_loop_selects_allowed_manifest` and the command-surface eval gate passed. The action metadata preserves bounded `boundedCandidateIds`, `targetAllowlist`, exact two-field `actionTargetAllowlist`, selected candidate rank matching the candidate order, and deterministic `toolSelectionRankingSource=deterministic_local`. |
| K7-06 | Deferred by acceptance rule. Deterministic selection is complete and kernel metadata records `toolSelectionModelRanked=false`, `toolSelectionProviderRankingAttempted=false`, `toolSelectionProviderRankingDeferred=true`, `toolSelectionDeterministicFallbackReady=true`, and `toolSelectionProviderRankingRequiredForLocalCompletion=false`. Existing live-provider/final-gate tests still validate provider-ranked evidence shapes, but provider-ranked kernel preselection was not made required for local completion in this goal. |
| K7-07 | `cargo test -p openlife-tauri main_chat_live_provider -- --nocapture` passed with 98 passed and 2 ignored. The ignored external-provider tests remain explicit opt-in and require `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1`, network, and a real provider API key; local HTTP provider proof passes without receiving external live credit. |
| K7-08 | Source audit confirmed MCP selection resolves exact manifest id first and exact manifest name only when unambiguous; write-like/high-risk/critical/contract-unsafe manifests are excluded; ambiguous, missing, or unsafe MCP targets return named blockers instead of fuzzy execution. |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p openlife-core` | Passed | Final output: `Finished dev profile ... target(s) in 0.29s`. |
| `cargo check -p openlife-tauri` | Passed | Final output: `Finished dev profile ... target(s) in 5.31s`. |
| `cargo test -p openlife-tauri main_chat_kernel -- --nocapture` | Passed | Output: 32 passed, 0 failed, 0 ignored, 696 filtered out; finished in 15.87s. |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | Passed | Output: 39 passed, 0 failed, 0 ignored, 689 filtered out; finished in 122.65s. |
| `cargo test -p openlife-tauri main_chat_live_provider -- --nocapture` | Passed | Output: 98 passed, 0 failed, 2 ignored, 628 filtered out; finished in 10.41s. |
| `cargo test -p openlife-tauri main_chat_task_control -- --nocapture` | Passed | Output: 10 passed, 0 failed, 0 ignored, 718 filtered out; finished in 0.08s. |
| `git diff --check` | Passed | No whitespace errors reported. |

If a command was not run: no Goal 7 minimum verification command was skipped.
The task-control run is extra evidence for K7-04.

## Hallucination Check

Every verification result above comes from command output produced in this
turn. Failed intermediate runs were used only to identify assertion and
blocker-metadata gaps; the table records the final passing outputs.

Additional source audit in this turn:

- `rg` confirmed command-surface eval includes registered MCP permission
  proposal scenarios and coverage fields.
- `rg` confirmed kernel MCP metadata records `strictManifestIdentity`,
  `fuzzyNameMatchingUsed=false`, `boundedCandidateIds`, deterministic fallback,
  and provider-ranking deferral.
- `git diff --check` passed after all code and report edits.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No live-provider dependency for basic kernel | All required local checks/tests passed without external provider credentials. External live-provider tests remain ignored unless explicitly opted in. |
| No MCP write-like execution without permission | Kernel read candidates exclude high-risk, critical, external-side-effect, write action/capability, write-like embedded surfaces, and contract-unsafe manifests. Permission-required read targets produce ToolPermission proposal/waiting state. |
| Strict MCP identity | Exact manifest id is preferred, exact manifest name must be unambiguous, and missing/ambiguous/unsafe targets return named blockers. No fuzzy target execution path was added. |
| Deterministic selection before provider ranking | Multi-candidate selection records bounded deterministic order, rank, target allowlist, and action-target allowlist; provider ranking is deferred and not required for local completion. |
| No silent durable writes | Web/MCP read actions and blockers record `directWritesExecuted=false`; ToolPermission proposal creation records the pending action identity instead of executing writes. |
| Send/stream parity preserved | Command-surface suite passed after send and stream web/MCP blocker/read/proposal cases were updated to kernel-backed evidence. |

## Legacy/Fallback Evidence

```text
legacy_fallback_used: false in the web/MCP command-surface tests and eval gate.
legacy_fallback_count: 0 for Goal 7 kernel-backed web/MCP restoration paths.
why_still_needed: Broader final-gate and legacy cleanup remains Goal 8 work; Goal 7 restores web/MCP/provider evidence on the stable kernel path.
```

## Direct Write Evidence

```text
direct_writes_executed: false in web/MCP read observations, blockers, ToolPermission proposal metadata, and live-provider harness reports.
direct_write_count: 0 new direct-write paths.
proposal_or_permission_records: MCP permission-required reads create ToolPermission proposals linked to `blockedAction` and `pendingActionIdentity`; accepted permission replay is covered by task-control tests.
```

## Provider Ranking And Live Provider Status

Provider-ranked kernel preselection was not promoted into the normal local
completion path in this goal. This is an explicit deferral under the K7 exit
rule: deterministic multi-candidate selection passes first, and the kernel
metadata records provider-ranking as deferred and not required for local
completion.

External live-provider proof also remains opt-in. The local harness proves the
ordinary path can invoke a local HTTP OpenAI-compatible provider and preserves
raw provider/model identities for final audit, but it does not count as
external live-provider completion.

## Source And Practice Consistency Check

Confirmed the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`
- `AGENTS.md`

No external source was used for this implementation. The change follows Goal 7
by restoring advanced web/MCP/provider evidence without making those advanced
paths prerequisites for basic chat, read-only tools, or proposal-only writes.

## Residual Risk

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
| Provider-ranked MCP preselection is deferred in the kernel-local path. | No | Goal 8 or a follow-up provider task can promote provider-ranked selection only after preserving the deterministic fallback and metadata-safe contract. |
| Fixture-backed web success is still not external web/provider evidence. | No | Keep it as local command-surface proof; external provider-backed web evidence remains opt-in. |
| MCP natural-language target parsing is intentionally conservative. | No | Add richer target parsing only with strict manifest identity and blocker tests. |
| Final/live acceptance remains broader than Goal 7 local readiness. | No | Goal 8 should realign final gates to kernel evidence while preserving explicit live-provider blockers. |
