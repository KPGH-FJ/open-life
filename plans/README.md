# OpenLife Plans Document Governance

> Last updated: 2026-06-18
> Status: authoritative document index for Agents; Main Chat Agent Beta v1 deterministic readiness is the completed foundation; Stage 1 Real End-to-End Dogfood is the current Goal-mode entry

This file prevents old planning documents from steering new Agent work. If two
documents disagree, use the precedence below and treat lower-priority stale text
as reference only.

## 1. Precedence

1. `AGENTS.md`
   - Project-wide Agent instructions, current constraints, and Tool Taxonomy.
2. `plans/README.md`
   - This authority map and current entry point.
3. `plans/main_chat_stage1_preparation_index.md` and
   `plans/main_chat_agent_stage1_dogfood_goal_spec.md`
   - Current Goal-mode entry for Stage 1 Real End-to-End Dogfood. This Goal
     must build on the existing Beta v1 task/event/memory/plan/proposal/
     skill/tool foundations, not create parallel runtime systems. It focuses
     on deterministic seed data, product dogfood scenarios, self-contained
     browser E2E evidence, final-delivery proof, visible blockers, and a
     fail-closed Stage 1 readiness report. External live-provider proof remains
     opt-in and separate from default readiness.
4. `plans/main_chat_agent_v1_stabilization_goal_spec.md`
   - Previous Goal-mode entry for the stabilization / acceptance-blocker
     remediation pass after checkpoint `d8e415f`. This Goal does not restart
     the previous broad migration Goal and does not expand the product roadmap.
     It exists to fix known blockers: real auditable final gate, external
     live-provider evidence/preflight, test-only harness leakage, ReAct
     tool-selection boundary, safe workspace file read, controlled
     knowledge-format context surfaces, and `src-tauri/src/lib.rs` module
     cleanup. It must fail closed and must not claim completion without real
     evidence.
5. `plans/main_chat_agent_migration_v1_goal_spec.md`
   - Main Chat Agent Execution v1 remediation spec / audit trail and capability
     target.
     Ordinary Main Chat now enters AgentIngress and a governed task session,
     ReActToolExecution now attempts the governed plan-guided AgentLoop before
     single-step ActionExecutor-backed read fallback, AgentLoop results that do
     not observe the planned action fall back instead of being treated as tool
     completion, direct read parser input is aligned with the memory/session
     executors, and a 100-case runtime harness now exercises the control plane.
     Chat now renders an execution
     task panel for goal/current plan/action queue/observations/blockers/
     fallback/task controls, Review Center affordances, and a route-level
     proposal accept / explicit task resume handoff. Accepted ToolPermission
     proposal + explicit resume now has a narrow command-surface proof that
     replays a pending read action through the governed executor. Command-surface tests cover
     DirectAnswer send/stream AgentRun/task-session completion, L2 DirectAnswer scheduler/provider generation trace, send/stream governed file-read, send/stream PlanExecute draft, proposal-path send/stream, send/stream registered-MCP AgentLoop success, send/stream registered-MCP ToolPermission proposal, send/stream web AgentLoop blocker, send/stream fixture-backed web AgentLoop success, plus
     send/stream web-policy and missing-MCP blocker
     preservation. A 24-case send/stream command-surface eval gate now exercises
     those paths through real Tauri mock IPC across DirectAnswer, scripted
     provider generation, file read, PlanExecute draft, proposal, web blocker, web AgentLoop blocker, fixture-backed web AgentLoop success, missing MCP blocker,
     registered MCP AgentLoop success, and registered MCP ToolPermission proposal with legacy fallback and silent write
     counts at zero, and the runtime eval report now includes explicit
     webPolicyBlocker/mcpMissingReadTarget blocker-state coverage,
     webSuccessfulReadCoverage fixture-backed success coverage,
     mcpRegisteredReadSuccess and mcpToolPermissionProposal coverage,
     providerRoute/localOnlyProviderGuard
     coverage, evalProviderGeneration/evalSchedulerGeneration coverage, and
     webAgentLoop/mcpAgentLoop coverage. The core runtime eval report and
     command-surface report also keep live-provider generation, combined
     provider-backed web/MCP AgentLoop, split provider-backed web AgentLoop,
     split provider-backed MCP AgentLoop, and provider/live
     proposal-permission coverage at zero in normal CI, with
     `finalCompletionReady=false` and named live-provider blockers including
     the split web/MCP blocker names.
     Core now also exposes a fail-closed Main Chat Agent Execution v1
     acceptance gate that aggregates runtime, send/stream command-surface, and
     live-provider evidence and rechecks key coverage so a spoofed ready flag
     cannot satisfy final completion. Tauri focused coverage now runs the
     real 24-case send/stream command-surface gate and feeds its output into
     the core final acceptance gate, and live-provider harness reports now
     aggregate into separate Direct generation, web AgentLoop, MCP AgentLoop,
     and proposal-permission evidence, with harness scenario identity checked
     before evidence is credited. Credited live-provider scenarios must also
     have completed status, no blockers, raw metadata-safe run/task ids with
     no wrapping whitespace/control characters, and a bounded non-empty
     harness-normalized single-line response trace field with no
     leading/trailing whitespace, repeated whitespace runs, or control
     characters. Runtime and
     command-surface reports now
     expose split zero live-provider web/MCP AgentLoop coverage rather than
     only a combined web-MCP field. Tauri now also has a single final
     acceptance runner that runs the core 100-case runtime gate and 24-case
     command-surface gate, then fails closed without live-provider opt-in.
     The scripted AgentLoop eval hook is no longer core-test-only, so Tauri
     sees the same memory/session/web/MCP AgentLoop proof when it invokes the
     core runtime gate. Complete clean live harness evidence is explicitly
     merged into runtime live coverage and command-surface final evidence; the
     runner returns runtime/command-surface case counts, live-provider
     attempted/report/auditable-ready/main-chat-invoked/model-invoked counts,
     where ready counts only reports credited by the matching scenario rules,
     metadata-safe live-provider blockers, including missing live-evidence
     blockers even when no live harness reports exist, a direct-write flag, and the nested
     core acceptance report; post-invocation live failures now also derive
     scenario-specific blockers, and ready/completed reports missing
     live-provider invocation allowance, Main Chat invocation, or model
     invocation proof derive explicit blockers instead of silently losing
     credit.
     The explicit non-default `run_main_chat_agent_execution_v1_eval_gate`
     Tauri command exposes the core 100-case runtime eval gate as a
     metadata-safe, no-external-provider, no-app-store-write report with
     `migrationPermission=false`; it includes a typed `liveProviderPreflight`
     report plus current-config live-provider preflight blockers without
     serializing keys or invoking the provider, lists split web and MCP live
     evidence requirements, and remains blocked without command-surface and
     live-provider evidence.
     A metadata-safe live-provider eval preflight now fails closed unless an
     operator explicitly opts into live eval with a cloud provider key, network
     enabled, no scripted scheduler response, and no LocalOnly policy; this
     preflight records blockers without invoking a model, can derive key
     presence from AppConfig without serializing the key, has Tauri
     command-state no-invocation blocker coverage, and is paired with ignored
     opt-in Tauri harness paths that invoke ordinary `send_message` only when
     the external-provider preflight is ready. A non-ignored local HTTP
     OpenAI-compatible provider-client harness now permits the `local_test_http`
     endpoint kind and proves ordinary `send_message` can run DirectAnswer
     through the real scheduler/HTTP client path with normalized single-line
     response trace and no
     silent writes; the acceptance evidence intentionally does not credit this
     as external live-provider generation, derives
     `live_provider_external_provider_missing` when such local proof or local,
     localhost, mock, fixture-like, or loopback/private-network alias provider
     identity, including alphanumeric-embedded IPv4 aliases and embedded
     local/mock/fixture/synthetic/scripted/ollama labels, is audited as live evidence, and normal command-surface
     live coverage remains zero.
     The ignored external paths cover DirectAnswer,
     provider-backed ReAct web AgentLoop, bounded multi-candidate registered
     MCP AgentLoop, and MCP ToolPermission proposal evidence, including
     `liveProviderInvoked`,
     metadata-safe provider identity with no local/private network alias,
     including alphanumeric-embedded loopback/private IPv4 aliases or embedded
     local/mock/fixture/synthetic/scripted/ollama labels,
     raw metadata-safe provider model identity with no wrapping whitespace/control characters,
     exact metadata-safe required-evidence manifest,
     AgentLoop action status, no single-step fallback, MCP target resolution /
     ToolPermission proposal checks, model-selected candidate
     rank/raw metadata-safe source with no wrapping whitespace/control characters/metadata-safe capability digest/bounded safe capability labels with a discrete read label and write-like labels rejected/raw metadata-safe match reason with no wrapping whitespace/control characters,
     selected candidate id/target/action type, selected candidate rank matching
     the selected candidate's 1-based position in the bounded candidate list,
     bounded candidate ids, target allowlist, exact action-target allowlist /
     ExecutionPolicy / metadata-safe governed-arguments digest trace, and no
     silent writes; candidate ids, target allowlist, and action-target
     allowlist must share the same distinct bounded target set and candidate
     cardinality, every allowed action must use the model-selected
     governed action type, every action-target allowlist entry must be an exact
     two-field `{actionType,target}` metadata-safe object with no extra JSON
     fields and no trim-normalized raw labels, registered MCP provider-ranked live credit must prove
     AgentLoop candidate id order exactly matches the provider-ranked candidate
     id order, and web AgentLoop live credit specifically requires
     the selected governed `web.*` action type to be `mcp_tool`. DirectAnswer live credit
     must prove direct provider generation with raw metadata-safe provider model identity with no wrapping whitespace/control characters and no AgentLoop, single-step fallback, MCP/proposal,
     or tool-selection metadata. ReAct live credit must not use single-step fallback. Web AgentLoop live credit
     must prove selected candidate id/target identity and action evidence are scoped to a governed `web.*` tool,
     with no overlapping registered MCP read-success or ToolPermission proposal trace.
     Registered MCP live credit
     must also prove at least two complete, duplicate-free bounded
     model-selectable MCP candidates / targets / action-target pairs plus
     provider-ranked selection metadata
     (`toolSelectionModelRanked`, `provider_model` ranking source, cloud/provider-backed
     ranking route with metadata-safe non-local ranking provider identity with no wrapping whitespace/control characters raw-exact matching the metadata-safe live report provider,
     metadata-safe ranking model identity with no wrapping whitespace/control characters raw-exact matching the metadata-safe live report model, exact
     one-field provider-ranking JSON response containing only `ranked_candidate_ids`, with Markdown fenced
     JSON, extra explanatory text, and extra response fields rejected fail-soft, complete
     duplicate-free bounded raw exact candidate-id permutation, contract-unsafe returned
     candidate ids, including ids that only match after trimming whitespace,
     rejected fail-soft, extra provider response fields rejected fail-soft, source
     candidate sets with contract-unsafe candidate/action/target/source/match labels or
     duplicate candidate ids rejected before provider invocation, contract-unsafe candidate/target/action
     labels rejected, candidate ids matching the exact target/action-target
     allowlists, selected candidate id matching
     the selected MCP target, selected rank matching the provider-ranked order,
     action type `mcp_tool`, metadata-safe response digest with
     `bytes:<positive-n> hash:sha256:<64-hex>` shape, canonical decimal
     byte count with no leading zeros, with zero-byte placeholders,
     leading-zero byte counts, and free-form hash suffixes rejected by harness
     ready checks and final credit, and no overlapping ToolPermission
     proposal outcome). MCP ToolPermission proposal live credit must also prove selected
     candidate id/target identity matches the pending ToolPermission proposal target, uses
     `mcp_tool`, and has no overlapping registered MCP read-success outcome. Live
     ReAct reports missing that governance trace are not credited as
     web/MCP/proposal live evidence.
     ReAct generic MCP
     candidate selection now records deterministic capability/name/tag ranking
     evidence that ignores raw manifest ids/descriptions in the metadata-safe
     candidate contract, including rank, source, capability digest, bounded safe
     capability labels, and sanitized match
     reason, redacts unsafe candidate contract labels, deduplicates by model-selectable target before applying the
     bounded limit, while continuing to reject
     high-risk/critical/confirmation-required/write-like read-shaped manifests,
     including embedded write-like terms in manifest id/name/action/capability/tag surfaces, and
     contract-unsafe or oversized model-facing manifest names/source labels;
     explicit named MCP read target resolution uses a permission-preserving
     governed-read target predicate that keeps safe read ToolPermission proposal
     flow available while still rejecting high/critical, write-like, and
     contract-unsafe read-shaped manifests before candidate exposure.
     AgentLoop now blocks model-selected exact-target allowlist misses, wrong action-target
     pairs, write-like or unsupported action types, and unknown non-candidate
     calls as explicit `model_selected_disallowed_tool` blockers without
     single-step fallback or writes, and records metadata-safe
     model-selected ExecutionPolicy validation with a
     `model_selected_tool_policy_blocked` path for policy-denied candidates.
     Exact candidate-pair allowlist entries now carry governed executor input,
     and command-surface boundary coverage verifies model-supplied `arguments`
     are replaced by the candidate contract before execution.
     Main Chat legacy fallback route-plan and non-stream generation fallback
     helpers now live in `src-tauri/src/main_chat_legacy_fallback.rs`, keeping
     fallback orchestration outside `src-tauri/src/lib.rs` while preserving
     explicit fallback visibility. Deprecated/non-default Main Chat AgentLoop
     send/stream helpers now live in `src-tauri/src/main_chat_legacy_agent_loop.rs`.
     Main Chat preprocessing and memory-hit merge helpers now live in
     `src-tauri/src/main_chat_preprocess.rs`.
     Main Chat auto-checkin, reasoning-trace prompt, and conversation-signal
     helpers now live in `src-tauri/src/main_chat_conversation_updates.rs`.
     Final-acceptance helper/runner/evidence tests now live in
     `src-tauri/src/main_chat_final_acceptance_tests.rs`, and the local
     FileReadSuccess command-surface eval case explicitly scopes isolated
     `safe_paths` to the canonical workspace root.
     Live-provider command-surface harness tests now live in
     `src-tauri/src/main_chat_live_provider_tests.rs`, including no-invocation
     preflight blockers, local HTTP provider proof, and ignored external-provider
     opt-in proof paths.
     Command-surface proposal-path IPC tests now live in
     `src-tauri/src/main_chat_command_surface_tests.rs`, keeping proposal-first
     send/stream proofs out of `src-tauri/src/lib.rs`; the same focused module
     now also owns DirectAnswer send/stream run-completion,
     scheduler/provider trace, web-policy blocker, missing-MCP blocker, and
     registered-MCP read-success plus registered-MCP / web-policy AgentLoop
     no-fallback, registered-MCP multi-candidate AgentLoop IPC tests, and the
     24-case command-surface eval gate coverage test.
     Main Chat HS runtime behavior and extraction-guard tests now live in
     `src-tauri/src/main_chat_hs_runtime_tests.rs`, covering HS helper module
     extraction, sanitized HS packet construction, tools-prompt read-only/write
     requirement separation, LocalOnly no-cloud fallback, sensitive-topic
     LocalOnly policy selection, and no `src-tauri/src/lib.rs` root re-export
     for HS runtime helpers.
     Main Chat task-control behavior tests now live in
     `src-tauri/src/main_chat_task_control_tests.rs`, covering retry manual
     blocker / automatic replay, permission-preserving resume / accepted
     ToolPermission replay, and cancel queued-action stop behavior outside
     `src-tauri/src/lib.rs`.
     Main Chat context-loader and workspace-file resolver behavior tests now
     live in `src-tauri/src/main_chat_context_loader_tests.rs`, covering
     bounded knowledge-format surfaces, selected `SKILL.md`
     loading/sanitization, selectedSkillId send/stream plumbing, and explicit
     workspace path/traversal read boundaries outside `src-tauri/src/lib.rs`.
     Main Chat runtime-module extraction guard tests now live in
     `src-tauri/src/main_chat_runtime_module_tests.rs`, covering runtime /
     generation / proposal / final-gate / command-surface / live-provider
     helper module boundaries, focused module helper import direction,
     send/stream state-executor guards, ordinary send/stream deprecated-helper
     isolation, and Chat page migration-command isolation outside
     `src-tauri/src/lib.rs`.
     Main Chat now also has a fail-soft provider/model-ranked preselection
     path for multi-candidate MCP read plans: it sends only the metadata-safe
     candidate contract, including capability digest and bounded safe capability labels, plus
     privacy-masked bounded context to the provider without injecting
     the full LifeModel system prompt, requires the previewed ranking route
     provider/model to match the actual request provider/model before provider-ranked evidence is credited, accepts only
     known candidate ids as a reorder signal, ignores invalid or
     contract-unsafe ids, preserves
     governed executor arguments, and records ranking source/digest plus
     accepted candidate-order evidence only when the provider returns a
     complete bounded candidate-id permutation; ignored provider orders keep
     only the ignored flag and metadata-safe response digest. This path is covered with ordinary `send_message` through a
     local HTTP OpenAI-compatible provider. The ignored external live runs were
     not executed in this environment and do not count as live-provider
     completion.
     Final completion still requires live-provider-backed
     generation eval coverage, broader provider-backed web/MCP AgentLoop and
     manifest/permission coverage, and broader provider/live proposal-permission
     proof.
5. `plans/openlife_lifemodel_governed_agent_runtime.md`
   - Current implementation program and next development order.
6. `plans/lifemodel_governed_backend_completion_goal_spec.md`
   - Completed Goal-mode master spec for the pre-UI LifeModel-governed
     backend kernel through W149.
7. `plans/lifemodel_governed_runtime_progress.md`
   - Compact W1-W158 completion/status index. This is not a second roadmap.
8. `plans/skill_runtime_goal_spec.md`
   - Completed CLI Goal-mode spec and audit trail for W150-W158 Skill Runtime
     Beta Maturity. This is not default Chat migration permission.
9. `plans/react_beta_execution_hardening_goal_spec.md`
   - Completed CLI Goal-mode spec and audit trail for ReAct Beta Execution
     Hardening W114-W123.
10. `plans/runtime_strategy_maturity_goal_spec.md`
   - Completed CLI Goal-mode spec and audit trail for RuntimeStrategy /
     Multi-Strategy Runtime Maturity W106-W113.
11. `plans/plan_execute_product_vertical_goal_spec.md`
   - Completed CLI Goal-mode spec and audit trail for the Plan-Execute Product
     Vertical W98-W105.
12. `plans/legacy_direct_write_convergence_goal_spec.md`
   - Completed CLI Goal-mode spec and audit trail for Legacy Direct-Write
     Convergence W90-W97.
13. `plans/lifemodel_maturation_goal_plan.md`
   - Completed Goal-mode preparation/spec and audit trail for the W73-W78
     LifeModel Maturation proof slice after W72.
14. Hard governance baselines:
   - `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
   - `plans/openlife_react_beta_roadmap.md`
   - `plans/lifemodel_hs_mvp_task_specs.md`
   - `plans/lifemodel_hs_legacy_write_path_audit.md`
15. Scoped architecture/product baselines:
   - `plans/openlife_agent_framework_architecture.md`
   - `OpenLife_PRD_v2_Agent_Framework.md`
16. Current execution helpers:
   - `plans/openlife_development_plan.md`
   - `plans/openlife_codex_execution_playbook.md`
17. Current product-surface rewrite entry:
   - `plans/frontend_product_shell_rewrite_goal_spec.md`
   - `plans/frontend_product_shell_rewrite_plan.md`
18. Historical/reference documents.
   - Useful for context, but never authoritative for current task order.

## 2. Current Position

Current latest status is **Main Chat Agent Beta v1 deterministic readiness is
the completed foundation, and Stage 1 Real End-to-End Dogfood is the current
Goal-mode entry**. The next Goal should use
`plans/main_chat_stage1_preparation_index.md` and
`plans/main_chat_agent_stage1_dogfood_goal_spec.md` to implement deterministic
product dogfood, UI-visible evidence, final-delivery proof, self-contained
browser E2E, and a fail-closed Stage 1 readiness gate before broader capability
expansion.

Main Chat Agent Execution v1 remediation remains in progress after W150-W158
Skill Runtime Beta Maturity. Ordinary `send_message` and
`start_stream_message` now enter `AgentIngress`, create/resume durable
`AgentTaskSession` records, and can render `ExecutionTranscript` / action queue
state in Chat through an execution task panel. `DirectAnswer` is now on a governed strategy path. Proposal /
blocker, PlanExecute draft, ActionExecutor-backed memory/session/file read-only
observation, retry/cancel/resume foundations, safe read automatic retry replay,
permission-preserving resume, cancel queued-action stop proof, and
non-replayable retry manual blockers exist.
ReActToolExecution now attempts the governed plan-guided AgentLoop first, then
fail-softs to a single-step ActionExecutor-backed read path for
memory/session/file and web/MCP wrapper cases; AgentLoop results that do not
observe the planned action are rejected for completion and use the fallback
path, direct read parser input is aligned with the memory/session executors,
there is eval-gated memory/session multi-step AgentLoop read/observe/follow-up
proof plus web network-policy blocker, fixture-backed successful web read, and
registered MCP AgentLoop proof, named
registered read-only MCP tools resolve through manifest/permission checks while
missing or non-read MCP targets block clearly, and web network-policy denial
returns a governed blocker rather than a generic tool failure.
Local-only, network, manifest, and permission blockers are preserved.
Successful observations feed a governed follow-up synthesis
step instead of being returned as raw observation echoes. A 100-case runtime eval
harness now drives AgentIngress, session/transcript/action queue, follow-up,
proposal/blocker, automatic retry replay, task controls, and separate
memory/session/file/web/MCP/PlanExecute coverage metrics, plus formal
ActionExecutor-backed observation coverage for deterministic read/blocker
paths, explicit webPolicyBlocker/mcpMissingReadTarget blocker-state coverage,
registered read-only MCP success coverage, mcpToolPermissionProposalCoverage
for generic registered-MCP ToolPermission proposal creation,
webAgentLoop/mcpAgentLoop coverage
for scripted AgentLoop plus ActionExecutor observations,
webSuccessfulReadCoverage for context-scoped fixture-backed web success, and
multi-step AgentLoop coverage for memory/session and registered MCP read tasks.
Tauri
mock IPC command-surface tests now cover `send_message` and
`start_stream_message` proposal-path execution, including waiting governed task
state, a completed governed `proposal.create` queue action, and a pending Review
Center proposal; they also cover send/stream registered-MCP AgentLoop success,
send/stream registered-MCP ToolPermission proposal,
send/stream web AgentLoop blocker, send/stream fixture-backed web AgentLoop
success, and send/stream web network-policy blocker / missing MCP read-target
blocker preservation as blocked governed task sessions.
The 24-case `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix`
gate aggregates send/stream DirectAnswer, scripted provider generation,
file read, PlanExecute draft, proposal, web blocker, web AgentLoop blocker, fixture-backed web AgentLoop
success, missing MCP blocker, registered MCP AgentLoop success, and registered
MCP ToolPermission proposal through real
Tauri mock IPC, with no legacy fallback and no silent write observed. The core
100-case runtime eval report and this command-surface gate both report
`finalCompletionReady=false` with zero live-provider generation /
web-MCP / proposal-permission coverage and named blockers until the ignored
live-provider harnesses are actually executed.
The core acceptance gate aggregates runtime, command-surface, and live-provider
evidence, and rechecks coverage thresholds before it can report ready.
Tauri focused coverage now proves the 24-case command-surface report is
converted into that core acceptance evidence rather than remaining a parallel
test-only surface.
Live-provider harness report aggregation now keeps Direct generation, web
AgentLoop, MCP AgentLoop, and proposal-permission evidence separate, so the
final gate cannot satisfy provider-backed web/MCP coverage with only one of
the two AgentLoop families.
The runtime and command-surface reports also expose separate
liveProviderWebAgentLoopCoverage and liveProviderMcpAgentLoopCoverage fields,
both zero until the live harnesses actually run.
The Tauri final acceptance runner now combines the core runtime gate,
command-surface gate, and optional live harness evidence into one report; the
default no-live path runs local gates and remains blocked. When
`OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1` is set, the runner uses isolated eval
AppState instances to execute the four ordinary Main Chat live harness scenarios
and returns exact blocked reports if credential/network/provider preflight fails;
the current environment only proves the no-invocation blocker path. Complete
clean live harness evidence is explicitly merged into runtime live coverage and
command-surface final evidence before the core final acceptance gate is
evaluated.
ActionExecutor now maps missing MCP read targets to a governed blocked action
instead of a generic failed tool call, can execute a registered read-only MCP
target as a successful read observation, and can create a generic
ToolPermission proposal for a registered MCP read target when policy requires
review.
The older deterministic 100-case suite is legacy scaffold coverage. This is
still not final completion: the fixture-backed web success proof is not live
provider-backed web completion, and the eval gate must expand to
live-provider-backed generation, provider-backed web/MCP AgentLoop/manifest
cases, and broader provider/live proposal-permission proof beyond the
fail-closed live-provider preflight, local runtime/command-surface proposal
gates, route-level UI handoff, and narrow accepted ToolPermission resume replay
case.

Stabilization progress after checkpoint `d8e415f`: final-gate aggregation,
live evidence normalization, command-surface live overlay, blocker derivation,
and live-provider required-evidence plus blocked/completed report construction
now live in `src-tauri/src/main_chat_final_gate.rs` and are used by the final
acceptance runner. A real non-default
`run_main_chat_agent_execution_v1_final_acceptance_gate` Tauri command now uses
the same aggregation, runs core runtime eval, attaches current state/scheduler
metadata-safe live-provider preflight, avoids external provider invocation by
default, avoids app-store writes, and fails closed with
`migrationPermission=false` when live evidence is absent; it also runs all 24
local send/stream command-surface cases on an isolated eval AppState, using
`main_chat_send::send_message_with_state` for send cases and
`main_chat_streaming::start_stream_message_with_state` for stream cases. With explicit
live opt-in, the same runner now executes DirectAnswer, web AgentLoop,
registered MCP AgentLoop, and MCP ToolPermission proposal harness scenarios on
isolated eval AppState instances; missing credentials produce four blocked
reports with no Main Chat/provider invocation.
Command-surface eval
case matrix, scenario state setup, prompt/session-id mapping,
case assertion/no-silent-write interpretation, report shape, coverage math, and
acceptance evidence normalization now live in
`src-tauri/src/main_chat_command_surface_eval.rs`, and isolated eval
AppState construction now lives in `src-tauri/src/main_chat_eval_state.rs` so
the command-surface and live harness state setup no longer depends on a
`#[cfg(test)]` state factory. Live-provider harness opt-in, suite execution,
ordinary `send_message` invocation, and report extraction now live in
`src-tauri/src/main_chat_live_provider_harness.rs` instead of `src-tauri/src/lib.rs`.
Main Chat task-control command state plus resume/cancel/retry/replay helpers
now live in `src-tauri/src/main_chat_task_controls.rs`, with Tauri command
registration still in `src-tauri/src/lib.rs`.
Main Chat generation support helpers for chat persistence, vector persistence,
AgentRun finalization, non-stream fallback generation, provider endpoint
classification, and metadata-safe preview text now live in
`src-tauri/src/main_chat_generation_support.rs`.
ReAct tool-selection plan/candidate helpers now live in
`src-tauri/src/main_chat_react_tool_selection.rs`.
ReAct AgentLoop attempt execution, runtime helper types, follow-up synthesis,
action-to-tool-call conversion, and tool-call/blocker metadata helpers now live in
`src-tauri/src/main_chat_react_runtime.rs`, and ReAct
ActionExecutor-backed fallback execution now lives in
`src-tauri/src/main_chat_react_execution.rs`.
Main Chat proposal and ToolPermission proposal support helpers now live in
`src-tauri/src/main_chat_proposal_support.rs`.
Main Chat HS runtime packet/topic/tool-requirement and LifeModel section
helpers now live in `src-tauri/src/main_chat_hs_runtime.rs`, with consumers
importing them directly instead of through `src-tauri/src/lib.rs` root
re-exports.
Main Chat task-session, transcript, and action-queue runtime support helpers now
live in `src-tauri/src/main_chat_runtime_support.rs`.
Main Chat send command state executor now lives in
`src-tauri/src/main_chat_send.rs`, leaving the Tauri send command in
`src-tauri/src/lib.rs` as command-surface wiring.
Main Chat strategy dispatch now lives in `src-tauri/src/main_chat_strategy.rs`,
leaving `src-tauri/src/lib.rs` focused on command-surface wiring and fallback
orchestration.
Main Chat stream command state executor and stream timeout policy now live in
`src-tauri/src/main_chat_streaming.rs`, leaving the Tauri stream command in
`src-tauri/src/lib.rs` as command-surface wiring.
Main Chat context compilation and selected-skill sanitization now live beside
bounded knowledge-format loading in `src-tauri/src/main_chat_context_loader.rs`.
The Main Chat workspace file read
resolver now accepts explicit workspace-relative paths, resolves from the
project workspace root even when the process CWD is `src-tauri`, canonicalizes
read targets, and blocks traversal/outside-workspace reads before execution. The
ReAct AgentLoop prompt now carries a metadata-safe tool-candidate contract,
generic MCP read requests can expose a bounded registered read-only manifest
candidate set, deterministic capability/name/tag ranking that ignores raw
manifest ids/descriptions is recorded as candidate rank/source/capability
digest/sanitized match reason evidence, unsafe candidate contract labels are redacted,
candidates are deduplicated by model-selectable target before the bounded limit is applied, high-risk /
critical / confirmation-required / write-like read-shaped manifests, including
embedded write-like terms in manifest id/name/action/capability/tag surfaces, and
contract-unsafe or oversized model-facing manifest names/source labels, are excluded from
generic candidate sets, explicit named MCP read target resolution uses a
permission-preserving governed-read target predicate that keeps safe read
ToolPermission proposal flow available while still rejecting high/critical and
write-like/contract-unsafe read-shaped manifests, and selected candidate targets plus exact action-target
pairs are enforced through exact `toolset_allowlist` target checks / exact candidate-pair checks;
allowlist misses, wrong action-target pairs, write-like or unsupported action
types, and unknown non-candidate model calls now block explicitly instead of
falling back, while model-selected candidates now record metadata-safe
ExecutionPolicy validation and policy-denied selected candidates block as
`model_selected_tool_policy_blocked`. Exact candidate-pair allowlist entries
now carry governed executor input, with boundary coverage proving model
`arguments` do not reach the executor for allowed selected candidates. Main
Chat context assembly now uses a controlled loader for bounded
workspace/configured `AGENTS.md`, `SOUL.md`, root / `memories/` `USER.md` /
`MEMORY.md`, and selected `SKILL.md` surfaces, with optional sanitized
`selectedSkillId` plumbing through ordinary send/stream command surfaces and
frontend Tauri wrappers plus an explicit manual Chat composer `SKILL.md`
context field; the async context compiler and selected-skill sanitizer are now
owned by the same focused context module. This improves the current narrow paths, but it does
not change the final completion status or satisfy real live-provider evidence,
remaining external live-provider harness evidence, or further module cleanup of
other Main Chat runtime/strategy code.

`plans/legacy_direct_write_convergence_goal_spec.md` is retained as the
completed W90-W97 Goal-mode spec and audit trail. W90-W92 retire the
Builder/Calibration/Feedback legacy direct-write override paths. W93 converts
Snapshot restore and Data import into explicit governed restore/import
operations with pre-change snapshots, payload/request validation, and
metadata-safe audit results. W94 converts the manual LifeModel editor save into
a governed manual override requiring explicit user intent, risk acknowledgement,
and a pre-change snapshot. W95 closes ProposalSource -> PatchSource mapping with
dedicated source variants and no Manual fallback blocker. W96 preserves the
State/Daily Goal source-data compatibility boundary. W97 updates the inventory
and materializer matrix so `overall_converged=true`,
`all_direct_writes_converged=true`, `high_risk_legacy_direct_write_count=0`,
and `proposal_first_convergence_complete=true`, while default Chat remains
`legacy_stream` and ordinary `send_message` / `start_stream_message` do not call
W79-W97 helpers.

`plans/plan_execute_product_vertical_goal_spec.md` is retained as the completed
W98-W105 Goal-mode spec and audit trail. W98-W105 implement a narrow
Plan-Execute Product Vertical: a non-default weekly planning workflow with a
typed product contract, durable plan sessions, review/edit/finalize lifecycle,
step-by-step execution, proposal-first write-like steps, AgentRun/trace
linkage, and Workspace/Runs frontend surfaces. It is not default Chat migration
or external provider write execution. W98-W105 alone did not complete full
RuntimeStrategy maturity; W106-W113 now complete the RuntimeStrategy maturity
layer as a separate non-default boundary.
Ordinary `send_message` / `start_stream_message` do not call the W98-W105
Plan-Execute product commands or helpers.

`plans/runtime_strategy_maturity_goal_spec.md` is retained as the completed
W106-W113 Goal-mode spec and audit trail. W106-W113 mature RuntimeStrategy /
Multi-Strategy Runtime with metadata-safe strategy capability descriptors,
registry readiness, StrategySelector candidate matrix/explanation,
MultiStrategy execution report envelope, an explicit non-default read-only
`get_runtime_strategy_registry_status` command, preview/product trace
vocabulary convergence, declarative-only future strategy boundaries, and
default Chat isolation hardening. ReAct and PlanExecute are executable
descriptor/registry-ready strategies. Direct, Layered, Workflow, Proactive, and
Reflective are future/declarative-only descriptors unless separately
implemented with full governance. W106-W113 is not default Chat migration and
not ReAct Beta execution hardening; readiness/status/maturity reports are not
migration permission. Ordinary `send_message` / `start_stream_message` do not
call W106-W113 helpers or commands.

W114-W123 ReAct Beta Execution Hardening is complete. It adds a metadata-safe
ReAct readiness/status contract, stable action schema and fail-soft parser,
Tool Registry Beta taxonomy/readiness, manifest-authoritative ActionExecutor
blocking, AgentRun action/observation `react_trace` envelopes, canonical
permission/replay scope, proposal-first write hardening, an explicit
non-default `get_react_beta_execution_status` command, Runs/Trace lifecycle UI
hardening, and this docs/progress sync. W114-W123 is not default Chat migration
and not a full Beta declaration; ordinary `send_message` /
`start_stream_message` remain unchanged on `legacy_stream` unless a later
separate reviewed route task explicitly changes them. Full Beta may still
require product surface work and any separately reviewed default Chat route
migration.

W124-W127 Backend Completion Goal 1 is complete. It adds the pure backend
LifeModel-Governed Backend Completion readiness/contract report, typed
LifeEvent schema and metadata-safe store skeleton, typed Signal schema and
deterministic low-risk extractor, and a safe LifeEvent/Signal/Evidence bridge
that writes EvidenceStore candidate records only for metadata-safe,
low-risk, sufficiently confident signals with lineage. W124-W127 add no Tauri
command, no frontend surface, no runtime/model/tool execution, no business
writes outside the explicit safe EvidenceStore bridge, no durable LifeModel or
Memory truth writes, and no default Chat route change.

W128-W130 Backend Completion Goal 2 is complete. It adds a pure backend
Evidence Graph v1 with support/opposition links, dedupe clusters, source
weights, cluster summaries, deterministic conflict/decay/cooldown state from
an injected timestamp, rejected-similar cooldown metadata, and a metadata-safe
Evidence Timeline read model. W128-W130 add no Tauri command, no frontend
surface, no runtime/model/tool execution, no LifeModel/Memory/Heuristic/Chat/
AgentRun/MCP audit/external writes, no durable truth materialization, and no
default Chat route change.

W131-W133 Backend Completion Goal 3 is complete. It adds a pure backend
Maturation Engine v1 with low-risk Evidence Graph cluster candidate generation
for planning preference, energy pattern, work style, and communication
preference; proposal outcome evidence convergence into positive/corrective/
negative metadata; and deterministic candidate suppression/correction using
opposition, conflict, decay, cooldown, and rejected-similar history. W131-W133
add no Tauri command, no frontend surface, no runtime/model/tool execution, no
LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external writes, no durable
truth materialization, and no default Chat route change.

W134-W136 Backend Completion Goal 4 is complete. It adds a pure backend
accepted guidance lifecycle that turns accepted maturation candidate proposals
into Trial HeuristicStore guidance with source proposal/evidence/run lineage,
privacy/model/tool constraints, usage metadata, and rollback/archive paths;
extends the LifeModel compatibility materialized YAML view with proposal/
evidence/patch/heuristic source digests and explicit compatibility provenance;
and adds a metadata-safe version diff / rollback read model for accepted
guidance and materialized view provenance. W134-W136 add no Tauri command, no
frontend surface, no runtime/model/tool execution, no ordinary Chat routing
change, no Memory/Chat/AgentRun/MCP audit/external writes, and no silent
durable LifeModel-HS truth materialization.

W137-W140 Backend Completion Goal 5 is complete. It adds RuntimeHSPacket v2
metadata-safe accepted/trial guidance refs, ReAct guidance consumption through
metadata-safe prompt summaries plus config/action-boundary constraints only
when `RuntimeGuidanceConsumptionMode::ExplicitRuntime` is enabled, explicit
Plan-Execute weekly planning guidance consumption, and a metadata-safe Guidance
Impact read model / trace linkage. W137-W140 add no ordinary Chat routing
change, no ordinary Chat accepted-guidance consumption, no default Chat
migration permission, no durable LifeModel-HS truth write, no Memory or external
write, and no raw prompt/user text/assistant output/memory/LifeModel/tool
payload leakage in read models.

W141-W143 Backend Completion Goal 6 is complete. It hardens ModelRouter/Privacy
HS policy enforcement so High/Critical privacy and HS LocalOnly can only select
local `ollama` and fail closed without cloud fallback; hardens ActionExecutor
HS tool governance so unsupported Plugin/A2A sources remain blocked before
permission replay/execution and write-like HS paths remain proposal-first; and
adds a shared metadata-safe Governor decision report for maturation, model
route, tool action, memory write, and external write decisions. W141-W143 add
no ordinary Chat routing change, no `send_message` / `start_stream_message`
replacement, no migration permission, no durable LifeModel-HS truth write, no
Memory/file/calendar/email/external/provider/plugin state write beyond existing
proposal-first paths, and no raw prompt/user text/assistant output/memory/
LifeModel/tool payload leakage.

W144-W146 Backend Completion Goal 7 is complete. It proves three pure
backend/core Backend Golden Paths: W144 Weekly Planning golden path, W145
Low-Energy Support golden path, and W146 Preference Correction golden path.
Goal 7 adds no ordinary Chat routing change, no `send_message` /
`start_stream_message` replacement, no Tauri command, no UI, no durable
LifeModel/Memory/external provider state write, and no migration permission.
Ordinary Chat must not call W144-W146 golden path helpers and must not treat
golden path ready as migration permission.

W147-W149 Backend Completion Goal 8 is complete. It freezes pure backend/core
metadata-safe read-model contracts for Learning Inbox, Evidence Timeline,
Proposal Review, Runtime Trace, Guidance Impact, Privacy Controls, and
LifeModel Overview; adds a final read-only backend completion gate report; and
syncs authority docs/progress/verification. Goal 8 adds no ordinary Chat
routing change, no `send_message` / `start_stream_message` replacement, no
Tauri command, no UI, no runtime/model/tool execution, no durable
LifeModel/Memory/external provider state write, and no migration permission.
Ordinary Chat must not call W147-W149 contract/final-gate helpers and must not
treat contract frozen or final gate ready as migration permission.

Backend Completion Goal 8 is no longer a future entry. W150-W158 Skill Runtime
Beta Maturity is complete: built-in skill readiness, bounded Skill context
assembly, LifeModel-HS privacy/model-route governance, output envelope and trace
stability, proposal candidate governance, plugin declarative-only boundaries,
non-default read-only Skill Runtime status, Runs/Review trace integration, and
docs sync are in place. This does not grant default Chat migration permission.
Other future options, such as pre-UI product surface design or a separately
reviewed default Chat route migration Goal, must be started through separate
specs. Old docs that still name Goal 8 as next or Skill Runtime as prepared
defer to this file, the backend completion spec, the Skill Runtime spec, and
`plans/lifemodel_governed_runtime_progress.md`.
W64 validated the compressed W1-W63 authority/index entry. W65 adds a pure Rust
descriptor mapper in `src-tauri/src/default_chat_adapter.rs` for a future
controlled adapter candidate contract. W66 adds a pure Rust controlled adapter
contract report/evaluator/ensure over that descriptor. W67 adds a pure Rust
backend-only non-default invocation harness that reads/reuses only the W66
contract report and proves the future controlled adapter candidate invocation
shape is metadata-safe, zero-side-effect, and executor-disabled/unattached.
W68 adds a pure Rust backend-only send-compatible proof/evaluator/ensure that
reads/reuses only W65 descriptor, W66 contract, and W67 harness metadata to
prove the controlled adapter candidate can map to a SendMessageResult-compatible
metadata-safe shape. It allows only the SendMessage callsite to become proof
ready; stream callsites fail closed. W69 adds a pure Rust backend-only
stream-compatible boundary proof/evaluator/ensure that reads/reuses only W65
descriptor, W66 contract, and W67 harness metadata to prove the controlled
adapter candidate can form a `start_stream_message`-compatible metadata
boundary. It allows only the StartStreamMessage callsite to become proof ready;
SendMessage fails closed with `callsite_not_start_stream_message`. W69 does not
emit a real stream, open an event channel, attach an executor, or authorize a
route cutover. W70 adds a pure Rust backend-only executor attachment gate
report/evaluator/ensure that simultaneously reuses W65-W67 metadata-safe
descriptor/contract/harness results, the W68 send-compatible proof, and the W69
stream-compatible boundary proof. W70 can report that the proof stack is
metadata-ready for the next executor skeleton discussion, but it keeps
executor_attachment_allowed=false, executor_attached=false,
executor_enabled=false, route_cutover_permission=false, and
migrationPermission=false. Executor implementation missing, human review
missing, and route cutover not authorized remain explicit blockers. W65-W70 add
no command, no frontend change, no Settings surface, no runtime/model/tool call,
no store write, no executor attachment, and no routing change.
W71 adds a pure Rust backend-only disabled controlled executor skeleton
contract/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W70 gate report and stores only metadata-safe callsite, route metadata,
input length/hash, and requested shape. Known send/stream shapes return
metadata-only placeholders; unknown shapes fail closed. W71 fixes
executor_skeleton_present=true, executor_enabled=false, executor_attached=false,
executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, and migrationPermission=false. W71 adds no
command, no frontend change, no Settings surface, no runtime/model/tool call,
no stream emission, no event channel, no business write, no executor
attachment, no route cutover, and no migration permission.
W72 adds a pure Rust backend-only disabled skeleton binding integrity
report/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W71 disabled skeleton, W71 skeleton input, and W70 gate report to verify
that input length/hash, route metadata, requested shape/callsite, skeleton
output shape, legacy route metadata, gate metadata, and disabled/no-run/no-write/no-stream
constraints are bound consistently. W72 keeps executor_enabled=false,
executor_attached=false, executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, migrationPermission=false, and
selected_adapter_path=legacy_stream. W72 is not executor implementation, not
executor attachment, not route cutover, and not migration permission.
W73 adds a pure core LifeModel maturation readiness report/evaluator/ensure in
`openlife-core/src/agent/maturation.rs`. It validates only the narrow
low-energy / low-pressure planning preference domain, checks that candidate
metadata is safe, proposal-first, source-lineage-ready, and does not require
direct LifeModel/Memory/Heuristic writes, keeps a zero side-effect budget, and
returns `nextAllowedStep=non_default_maturation_invocation` only when clean.
W73 adds no command, no frontend surface, no runtime/model/tool call, no
Evidence/Proposal/LifeModel/Memory/Heuristic/Chat/MCP audit/external write, no
ordinary Chat auto-maturation, and no default Chat route change.
W74 adds a pure core explicit non-default LifeModel maturation invocation
harness/report in `openlife-core/src/agent/maturation.rs`. It must call W73
readiness first; when readiness is blocked it writes no stores, and when ready
it only writes governed candidate EvidenceStore records and pending
ProposalStore records. W74 keeps no runtime/model/tool execution, no
LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external write, no Tauri
command, no frontend surface, no ordinary Chat auto-maturation, and no default
Chat route change.
W75 adds `openlife-core/src/agent/proposal_outcome.rs` with
`MaturationProposalOutcome`, `MaturationProposalOutcomeEvidenceReport`,
`evaluate_maturation_proposal_outcome_evidence`, and
`record_maturation_proposal_outcome_evidence`. It minimally wires
`src-tauri/src/commands/proposal.rs` after successful proposal accept/reject/edit
state updates. Only maturation lineage proposals record metadata-safe
`ProposalOutcome` evidence; rejected proposals record negative/opposing outcome
evidence, edited proposals do not persist raw edited payload in the outcome
report/evidence, and non-maturation proposals no-op. W75 does not add a
command/frontend surface, does not run runtime/model/tool, does not change
default Chat, and is not a maturation runtime migration.
W76 adds pure core low-energy collaboration rule candidate aggregation in
`openlife-core/src/agent/maturation.rs` with
`LowEnergyCollaborationRuleCandidateInput`,
`LowEnergyCollaborationRuleCandidateReport`,
`evaluate_low_energy_collaboration_rule_candidate`, and
`propose_low_energy_collaboration_rule_candidate`. It aggregates only
metadata-safe accepted/edited/rejected maturation ProposalOutcome evidence,
preserves accepted/rejected/edited outcome evidence ids, source evidence ids,
linked proposal ids, and linked AgentRun ids, and opposing/negative evidence
blocks or weakens repeated similar candidate rules. When ready, W76 may write
only a pending ProposalStore candidate proposal; it does not activate a
Heuristic, does not write active rules, adds no command/frontend surface, runs
no runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, and does not
affect default Chat.
W77 adds pure core accepted low-energy rule selection proof in
`openlife-core/src/agent/maturation.rs` with
`AcceptedLowEnergyRuleSelectionInput`,
`AcceptedLowEnergyRuleSelectionReport`,
`AcceptedLowEnergyRuleSelectionHSPacketAuditProof`,
`evaluate_accepted_low_energy_rule_selection`, and
`ensure_accepted_low_energy_rule_selection`. It selects only user-accepted W76
candidate proposals into a future RuntimeHSPacket metadata-safe planning
guidance proof, preserves outcome evidence / proposal / AgentRun lineage, and
fails closed for pending/rejected/non-W76 proposals, non-planning tasks, and
non-low-energy domains. If privacy policy or an existing packet requires
LocalOnly, W77 keeps or strengthens that route; the rule cannot override or
relax privacy/model route policy. W77 adds no command/frontend surface, runs no
runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, does not
activate a Heuristic, and does not affect default Chat.
W78 adds pure core metadata-safe trace visibility proof in
`openlife-core/src/agent/maturation.rs` with
`LowEnergyRuleTraceVisibilityInput`,
`LowEnergyRuleTraceVisibilityReport`, `LowEnergyRuleTraceMetadata`,
`evaluate_low_energy_rule_trace_visibility`, and
`ensure_low_energy_rule_trace_visibility`. It proves that W77 selected
guidance can be shown or recorded by a future runtime/run trace using only
metadata-safe fields: selected guidance summary/hash, candidate proposal
id/hash, rule digest, evidence/proposal/AgentRun lineage id/hash/count/status/type,
selected policy ids, route policy proof, and stable report/payload hashes. W78
fails closed for blocked or non-selected W77 reports, non-planning or
non-low-energy selections, raw trace payloads, privacy/model route relaxation,
default Chat route cutover, runtime/model/tool execution, AgentRun writes, or
Heuristic activation. W78 adds no command/frontend surface, runs no
runtime/model/tool, writes no AgentRun/LifeModel/Memory/Heuristic truth, does
not activate a Heuristic, and does not affect default Chat.
W79 adds a backend-only/internal Rust legacy direct-write convergence inventory
guard in `src-tauri/src/legacy_write_convergence.rs` with
`LegacyWriteRiskClass`, `LegacyWriteConvergenceStatus`,
`LegacyWritePathKind`, `LegacyWriteInventoryEntry`,
`LegacyWriteConvergenceReport`, `legacy_write_convergence_inventory`,
`evaluate_legacy_write_convergence_inventory`, and
`ensure_legacy_write_convergence_inventory_guard`. It turns
`plans/lifemodel_hs_legacy_write_path_audit.md` into a machine-readable,
metadata-safe development entry that covers LifeModel save/manual editor,
Builder, Calibration, Feedback, restore/import, state/daily goal, raw
chat/memory/vector, proposal application, and external proposal paths. W79
reports known high-risk direct-write blockers and keeps
`overall_converged=false` / `all_direct_writes_converged=false`; it does not
converge any direct-write path, add commands/frontend, run runtime/model/tool,
write stores, change product behavior, or affect default Chat.
W80 adds a backend-only/internal manual LifeModel editor explicit override
audit guard in `src-tauri/src/commands/life_model.rs` with
`ManualLifeModelOverrideAuditReport`,
`evaluate_manual_lifemodel_override_audit`, and
`record_manual_lifemodel_override_audit_with_state`. `save_life_model_with_state`
preserves existing editor save behavior, but after a successful save it records
a metadata-safe `manual_lifemodel_override_audit` analytics event with only
source, before/after hashes, rough changed section names/count, risk class,
timestamp, command/function name, and
manualOverride/proposalFirst/stillLegacyDirectWrite flags. It does not record
raw LifeModel JSON, identity values, goals, relationships, health/finance/privacy
text, prompts, outputs, tool payloads, or full before/after payloads. It does
not create Proposal/AgentRun/Heuristic/Patch records, run runtime/model/tool, or
affect default Chat. W79 inventory now marks the manual editor guard present
while keeping `manual_lifemodel_editor` as a high-risk legacy direct-write
blocker and keeping convergence false.
W81 adds a backend-only Builder legacy direct apply dev/migration gate in
`src-tauri/src/commands/builder.rs`. `builder_apply_signals` now fails closed
by default and only enters the legacy direct write path when an explicit
dev/migration override is supplied. The old direct-apply response no longer
returns raw model payloads or run ids and exposes only metadata-safe applied
path summaries/counts and warnings. The no-signal completion branch in
`builder_step_with_state` performs session-only cleanup and does not persist
durable LifeModel truth. The normal Builder product path remains
`builder_create_proposals`; the Builder legacy path remains a high-risk
direct-write blocker and is not fully converged.
W82 adds a backend Calibration legacy direct apply dev/migration gate in
`src-tauri/src/commands/calibration.rs`. `apply_calibration(mode="direct")`
and `run_micro_evolution` now fail closed by default and only enter legacy
direct persistence when an explicit `CalibrationLegacyDirectApplyDevMigrationOverride`
is supplied. Legacy responses are metadata-safe and do not return raw
LifeModel, raw calibration, or raw evolution payloads. Normal Calibration and
Dashboard product flow uses `calibration_create_proposals` / proposal mode and
writes ProposalStore entries; Calibration direct/evolution capability remains a
high-risk direct-write blocker and is not fully converged.
W83 adds a backend Feedback evolution legacy direct apply dev/migration gate in
`src-tauri/src/commands/feedback.rs`. `apply_feedback_evolution` now fails
closed by default and only enters the legacy direct write path when an explicit
`FeedbackEvolutionLegacyDirectApplyOverride` is supplied. Legacy responses are
metadata-safe and do not return raw feedback text, raw conversation inference,
raw LifeModel, or raw evolution rule payloads. `generate_evolution_report` is
now read-only and returns metadata-safe counts/status only; it does not write
LifeModel or `evolution_rules` truth. The settings UI presents the result as a
read-only candidate report. The W79 inventory separates Feedback signals as
low-risk source data and the read-only report from the Feedback evolution
direct-apply blocker; the remaining override capability means Feedback
evolution direct apply is still not fully converged.
W84 adds backend Snapshot restore and Data import legacy direct write gates in
`src-tauri/src/commands/version.rs` and `src-tauri/src/commands/settings.rs`.
`restore_snapshot` and `import_all_data` now fail closed by default and only
enter the legacy direct write path when explicit dev/migration/manual restore
overrides are supplied. Legacy responses return metadata-safe snapshot
ids/counts/status only and do not return raw LifeModel, raw memory/vector
content, raw imported payloads, or snapshot YAML. Export and read-only snapshot
inspection paths remain available; restore/import override capability means
these paths are still not fully converged.
W85 adds a backend-only/internal State / Daily Goal source-data boundary proof
in `src-tauri/src/legacy_write_convergence.rs` with
`StateSourceDataBoundaryReport`, `evaluate_state_source_data_boundary`, and
`ensure_state_source_data_boundary`. It proves only that
`state_daily_goal_direct_writes` is classified as low-risk transient/source-data
compatibility materialized write rather than accepted durable LifeModel-HS
truth. The inventory must explicitly list `persist_life_model`, because State /
Daily Goal currently writes the current LifeModel compatibility view / YAML.
The report is metadata-safe and contains only path ids, fixed classification
flags, compatibility_lifemodel_materialized_write=true,
writes_current_lifemodel_compatibility_view=true,
accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false,
proposal_required_for_hs_truth_promotion=true, ordinary/default Chat unchanged
booleans, and blocker codes. W85 is not a proposal-first conversion, does not
change default Chat, does not change `record_state` or daily-goal product
behavior, and does not mark State/Daily Goal as fully converged. Future
promotion into durable LifeModel-HS truth must be a separate proposal-first
slice.
W86 adds a backend-only/internal LifeModel compatibility materializer caller
matrix in `src-tauri/src/legacy_write_convergence.rs` with
`LifeModelMaterializerCallerKind`,
`LifeModelMaterializerCallerRisk`,
`LifeModelMaterializerCallerGovernanceState`,
`LifeModelMaterializerCallerMatrixEntry`,
`LifeModelMaterializerCallerMatrixReport`,
`lifemodel_materializer_caller_matrix`,
`evaluate_lifemodel_materializer_caller_matrix`, and
`ensure_lifemodel_materializer_caller_matrix`. It classifies every current
production materializer/save entry found for this slice: 16
`persist_life_model` callsites plus 3 production `LifeModelManager::save`
related entries. The matrix distinguishes the materializer root, ordinary Chat
daily-goal auto-checkin source-data compatibility writes, State/Daily Goal
source-data compatibility materialization, accepted proposal apply, audited
manual override, Builder/Calibration/Feedback guarded legacy dev-migration
override paths, and Snapshot restore/Data import gated override paths. W86 is
metadata-safe and does not include raw LifeModel, memory, chat, or daily-goal
payloads. It does not add a command/frontend surface, does not change default
Chat, does not change the `persist_life_model` signature, does not retire any
legacy path, and keeps migration_permission=false,
runtime_authority_granted=false, and proposal_first_convergence_complete=false.
W86 is not convergence complete; it is the preparation layer for W87 caller
restriction.
W87 adds the backend-only/internal caller restriction layer on top of the W86
matrix. It introduces `LifeModelMaterializerCallerPurpose`,
`LifeModelMaterializerCallerContext`,
`LifeModelMaterializerCallerRestrictionReport`,
`evaluate_lifemodel_materializer_caller_restriction`,
`ensure_lifemodel_materializer_caller_allowed`, and
`ensure_lifemodel_materializer_caller_restriction` in
`src-tauri/src/legacy_write_convergence.rs`. `persist_life_model` now requires
an explicit typed caller context, and every production `persist_life_model`
callsite passes its W86 stable id, kind, and governance purpose. Snapshot
restore's direct `LifeModelManager::save(&restored_model)` has an explicit W87
restriction guard after the existing W84 restore override and before the save.
Unknown stable ids, kind/purpose mismatches, metadata-unsafe entries,
source-data callers marked as accepted durable LifeModel-HS truth,
migration/runtime authority grants, and legacy override callers marked fully
converged fail closed. W87 changes no default Chat routing, adds no
command/frontend/Settings surface, does not run runtime/model/tool, does not
write Chat/AgentRun/Evidence/Proposal/Memory/MCP audit/external records, does
not retire legacy paths, and does not complete proposal-first source-specific
patch mapping.
W88 adds a backend-only/internal source-specific PatchSource mapper for accepted
LifeModel proposal apply in `src-tauri/src/commands/proposal.rs`.
`apply_proposal_to_state` no longer hardcodes `PatchSource::BuilderReview` when
creating a `LifeModelPatch`. BuilderReview maps to BuilderReview,
CalibrationRun maps to Calibration, FeedbackEvolution maps to Evolution, and
Manual maps to Manual. ChatConversation, ProactiveAgent, SkillRuntime, Plugin,
and MemoryGovernance use an explicit metadata-safe Manual fallback with W89
follow-up/blocking metadata because PatchSource has no dedicated variants for
those proposal sources. W88 adds no command/frontend/Settings surface, runs no
runtime/model/tool, changes no default Chat routing, retires no legacy path, and
keeps `proposal_first_convergence_complete=false` for W89 audit/readiness.
W89 adds a backend-only/internal readiness entry/report/evaluator/ensure in
`src-tauri/src/commands/proposal.rs` for the accepted LifeModel proposal
PatchSource mapping. It proves exact mappings for BuilderReview, CalibrationRun,
FeedbackEvolution, and Manual; metadata-safe Manual fallback mappings for
ChatConversation, ProactiveAgent, SkillRuntime, Plugin, and MemoryGovernance;
`apply_proposal_to_state` still calls the W88 mapping ensure and resolver before
`LifeModelPatch::from_proposal`; the apply path does not hardcode
BuilderReview; and ordinary default Chat entrypoints do not call the W88/W89
helpers. The report stores no raw proposal payload, raw LifeModel patch value,
memory text, chat text, or tool payload. W89 adds no command/frontend/Settings
surface, runs no runtime/model/tool, changes no product behavior or default
Chat routing, retires no legacy path, and keeps fallback blockers plus
`proposal_first_convergence_complete=false`.

Any next controlled adapter work must arrive through a separate task that
explicitly asks for it and preserves default Chat `legacy_stream` until a
reviewed route change is implemented and verified.

`plans/skill_runtime_goal_spec.md` is the completed W150-W158 Skill Runtime Beta
Maturity spec / audit trail on top of the W149 backend kernel. Skill Runtime
does not migrate default Chat, replace ordinary `send_message` /
`start_stream_message`, call Skill Runtime readiness/status from ordinary Chat,
directly write LifeModel/Memory/file/calendar/email/external/plugin state from
skills, or bypass proposal-first governance. Skill Runtime readiness/status
remains non-default, read-only, metadata-safe, and not migration permission.
`plans/lifemodel_maturation_goal_plan.md` is retained as the completed W73-W78
LifeModel maturation proof-slice spec/audit trail.
W79 completes only the inventory guard for the next Legacy Direct-Write
Convergence phase. W80 adds a metadata-safe manual override audit guard to the
highest-risk manual editor save path; actual proposal-first convergence remains
future work. W81 reduces Builder legacy direct-apply risk with a default
fail-closed dev gate and no-signal no-write proof, but the remaining override
capability still means Builder legacy direct apply is not fully converged.
W82 reduces Calibration direct/evolution risk with a default fail-closed
dev/migration gate and keeps normal Calibration/Dashboard flow proposal-first,
but the remaining override capability still means Calibration direct/evolution
is not fully converged.
W83 reduces Feedback evolution direct-apply risk with a default fail-closed
dev/migration gate and makes `generate_evolution_report` read-only, but the
remaining override capability still means Feedback evolution direct apply is
not fully converged.
W84 reduces Snapshot restore and Data import risk with default fail-closed
dev/migration/manual-restore gates and metadata-safe legacy responses, but the
remaining override capability still means restore/import are not fully
converged.
W85 proves the State/Daily Goal source-data boundary only. State history and
Daily Goal entries currently write the LifeModel compatibility view / YAML, but
that materialized compatibility write is source data / low-risk transient
state, not accepted durable LifeModel-HS truth, not an active HS LifeModel
patch, and not automatically promoted. W85 does not convert the path to
proposal-first and does not mark it fully converged; durable truth promotion
remains proposal-first future work.
W86 proves the materializer caller matrix only. It confirms ordinary Chat
auto-checkin is source-data compatibility, proposal apply is accepted proposal
apply, manual editor is audited manual override and still a high-risk blocker,
restore/import are gated overrides and still high-risk blockers, and no
unclassified production caller is known. W87 now restricts materializer callers
with typed context and fail-closed checks. W88 fixes accepted LifeModel proposal
PatchStore source mapping, and W89 adds the metadata-safe source-specific
application audit/readiness proof. Fallback proposal sources still require a
separate policy decision before convergence can be marked complete.

Current Main Chat Agent Execution v1 constraints:

- ordinary Main Chat must enter `AgentIngress` before strategy execution.
- legacy generation remains available only as a visible fallback.
- do not claim Main Chat Agent v1 is complete until the real eval gate and all
  required runtime/tool/UI checks pass.
- no strategy may silently write durable LifeModel-HS truth, long-term Memory,
  file/calendar/email/external/provider/plugin state, or dangerous shell state.
- memory and LifeModel updates from Chat must create Review Center proposals.
- external/sensitive writes must create blockers/confirmation requests.
- bounded context may include workspace/materialized files only as task context;
  those files cannot override privacy, model-route, or tool policy.
- W19-W60 readiness/review/preview/gate outputs are not migration permission.
- W61-W63 are整理阶段, not default Chat migration.
- Ordinary `send_message` / `start_stream_message` must not call W19-W60
  command surfaces.
- Ordinary `send_message` / `start_stream_message` must not call the W67
  non-default invocation harness.
- Ordinary `send_message` / `start_stream_message` must not call the W68
  send-compatible proof.
- Ordinary `send_message` / `start_stream_message` must not call the W69
  stream-compatible boundary proof.
- Ordinary `send_message` / `start_stream_message` must not call the W70
  executor attachment gate.
- Ordinary `send_message` / `start_stream_message` must not call the W71
  disabled executor skeleton.
- Ordinary `send_message` / `start_stream_message` must not call the W72
  skeleton binding integrity report.
- Ordinary `send_message` / `start_stream_message` must not call the W73
  LifeModel maturation readiness report.
- Ordinary `send_message` / `start_stream_message` must not call the W74
  non-default LifeModel maturation invocation.
- Ordinary `send_message` / `start_stream_message` must not call the W75
  proposal outcome evidence helper.
- Ordinary `send_message` / `start_stream_message` must not call the W76
  low-energy collaboration rule candidate helper.
- Ordinary `send_message` / `start_stream_message` must not call the W77
  accepted low-energy rule selection helper.
- Ordinary `send_message` / `start_stream_message` must not call the W78
  low-energy rule trace visibility helper.
- Ordinary `send_message` / `start_stream_message` must not call the W79
  legacy write convergence inventory guard.
- Ordinary `send_message` / `start_stream_message` must not call the W80
  manual LifeModel override audit helper.
- Ordinary `send_message` / `start_stream_message` must not call the W81
  Builder legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W82
  Calibration legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W83
  Feedback evolution legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W84
  Snapshot restore / Data import legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W85 State
  / Daily Goal source-data boundary helper.
- Ordinary `send_message` / `start_stream_message` must not call the W86
  LifeModel materializer caller matrix helper.
- Ordinary `send_message` / `start_stream_message` must not call the W87
  LifeModel materializer caller restriction evaluator/ensure helpers.
- Ordinary `send_message` / `start_stream_message` must not call the W88
  proposal PatchSource mapping helper.
- Ordinary `send_message` / `start_stream_message` must not call the W89
  proposal PatchSource readiness helper.
- W49-W55 pure ordinary-entry guards / preflight are historical regression
  context after Main Chat Agent v1; they do not authorize bypassing
  AgentIngress or disabling the v1 policy/proposal/fallback gates.
- W65-W72 backend-only descriptor/contract/harness/proof/gate/skeleton/binding work is metadata only
  and is not migration permission. W67 `harness_ready` only means the
  non-default invocation shape proof is safe; W68 `proof_ready` only means the
  SendMessageResult-compatible metadata shape proof is safe; W69 `proof_ready`
  only means the stream-compatible metadata boundary proof is safe; W70
  `gate_report_metadata_ready` only means the attachment gate report metadata is
  ready for executor skeleton discussion; W71 `skeleton_contract_ready` only
  means the disabled skeleton contract metadata is safe and still no-run; W72
  `binding_integrity_ready` only means the disabled skeleton binding metadata is
  internally consistent and still no-run.

## 3. W1-W158 Compression Map

For the row-level structured index, use
`plans/lifemodel_governed_runtime_progress.md`. It lists every stage with:
stage id, name, status, command/surface type, read-only/write-disabled/
metadata-safe safety, default Chat impact, and next dependency.

| Range | Compressed meaning | Default Chat authority |
| --- | --- | --- |
| W1-W8 | Runtime, LifeModel, Strategy, and MultiStrategy foundations | No migration authority |
| W9-W18 | Non-default preview, preview audit, and migration gate evidence surfaces | No migration authority |
| W19-W23 | Controlled pilot eligibility, explicit pilot, reviewed promotion, source binding, promotion evidence | Explicit pilot/promotion only; ordinary Send unchanged |
| W24-W27 | Promotion readiness, migration plan draft, review evidence, implementation gate | Readiness/approval is discussion only, not migration permission |
| W28-W33 | Shadow run/review, cutover readiness, candidate adapter/review, candidate promotion readiness | Non-default write-disabled validation only |
| W34-W42 | Default Chat boundary, activation plan/review/gate, disabled routing, contract harness, dry run/review, implementation readiness | Read-only or non-default evidence only |
| W43-W48 | Controlled preview/review/readiness and cutover implementation plan/review/readiness | Non-default preview and planning only |
| W49-W55 | Route guard, invocation harness/plan/boundary, typed callsite contract, ordinary-entry preflight | Pure fail-closed guard only; route stays `legacy_stream` |
| W56-W60 | Ordinary-entry status, narrow discussion gate, narrow plan draft/review/approval readiness | Settings/status/planning only; ordinary entries must not call commands |
| W61-W64 | Docs/index整理, W1-W63 compression freeze, and authority validation | Docs only; no default Chat effect |
| W65 | Backend-only controlled adapter descriptor skeleton | Internal metadata-safe mapper only; no default Chat effect |
| W66 | Backend-only controlled adapter contract report | Internal metadata-safe contract evaluator only; no default Chat effect |
| W67 | Backend-only non-default controlled invocation harness | Internal metadata-safe shape proof only; no command, executor, runtime, write, routing, or default Chat effect |
| W68 | Backend-only send-compatible contract proof | Internal SendMessageResult-compatible metadata proof only; stream fails closed; no command, executor, runtime, write, routing, or default Chat effect |
| W69 | Backend-only stream-compatible boundary proof | Internal `start_stream_message`-compatible metadata boundary proof only; SendMessage fails closed; no real stream, event channel, command, executor, runtime, write, routing, or default Chat effect |
| W70 | Backend-only executor attachment gate report | Internal metadata-ready gate report only; executor attachment/cutover/migration permission all false; no command, executor, runtime, write, routing, or default Chat effect |
| W71 | Backend-only disabled executor skeleton contract | Internal metadata-only placeholder contract only; executor disabled/unattached/not runnable, invocation disallowed, no stream/event channel, no command, runtime, write, routing, or default Chat effect |
| W72 | Backend-only disabled skeleton binding integrity report | Internal metadata binding report only; verifies W71 input/skeleton and W70 gate consistency, no executor implementation/attachment/cutover/migration permission, no command, runtime, write, routing, or default Chat effect |
| W73 | LifeModel maturation readiness report | Pure core metadata-safe readiness report only; low-energy planning domain, proposal-first, no writes, no command, no ordinary Chat effect |
| W74 | LifeModel non-default maturation invocation | Pure core explicit invocation only; calls W73 first, blocked writes no stores, ready writes EvidenceStore + ProposalStore only, no command, no ordinary Chat effect |
| W75 | Proposal outcome evidence link | Core helper plus minimal proposal accept/reject/edit internal wiring; writes metadata-safe ProposalOutcome evidence only for maturation lineage proposals; no command/frontend/runtime/default Chat effect |
| W76 | Low-energy collaboration rule candidate | Pure core evaluator/proposer only; aggregates metadata-safe ProposalOutcome evidence into a pending candidate proposal, blocks/weakens on opposing evidence, no active Heuristic/rule, no command/frontend/runtime/default Chat effect |
| W77 | Accepted rule to RuntimeHSPacket selection proof | Pure core evaluator/report/ensure only; accepted W76 candidate proposal, planning task, low-energy domain, metadata-safe guidance, lineage retained, privacy/model route policy not relaxed, no command/frontend/runtime/default Chat effect |
| W78 | Run trace visibility proof | Pure core evaluator/report/ensure only; W77 selected guidance and lineage can be shown as trace metadata using summary/hash/id/count/status/type fields, privacy/local-only proof preserved, raw payload/policy relaxation/execution/cutover hints fail closed, no command/frontend/runtime/AgentRun write/default Chat effect |
| W79 | Legacy direct-write convergence inventory guard | Internal Rust inventory/report/ensure only; machine-readable metadata-safe audit over known direct-write/proposal-first/source-data paths, reports blockers and keeps overall convergence false, no command/frontend/runtime/write/default Chat effect |
| W80 | Manual LifeModel editor explicit override audit guard | Historical guard slice; superseded by W94 governed manual override |
| W81 | Builder legacy direct apply dev-gate / no-signal completion guard | Historical guard slice; superseded by W90 retirement |
| W82 | Calibration direct apply legacy gate / proposal-first default | Historical guard slice; superseded by W91 retirement |
| W83 | Feedback evolution legacy direct apply gate / read-only report | Historical guard slice; superseded by W92 retirement |
| W84 | Snapshot restore / data import legacy direct write gate | Historical guard slice; superseded by W93 governed restore/import operations |
| W85 | State / Daily Goal source-data boundary proof | Backend-only internal proof over `state_daily_goal_direct_writes`; inventory explicitly lists `persist_life_model`; report is metadata-safe, default Chat unchanged, ordinary Chat unchanged, compatibility_lifemodel_materialized_write=true, writes_current_lifemodel_compatibility_view=true, accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false, proposal_required_for_hs_truth_promotion=true; not proposal-first conversion and not fully converged |
| W86 | LifeModel compatibility materializer caller matrix | Historical matrix slice; superseded by W97 final materializer matrix |
| W87 | LifeModel materializer caller restriction | Typed caller context/restriction remains active; W97 matrix now admits only classified source-data compatibility, governed manual override, governed restore/import, accepted proposal apply, and materializer root callers |
| W88 | Proposal application source-specific PatchSource mapping | Historical mapper slice; superseded by W95 exact ProposalSource -> PatchSource mapping |
| W89 | Proposal application source-specific PatchSource audit/readiness | Historical readiness slice; superseded by W95 exact mapping and W97 `proposal_first_convergence_complete=true` |
| W90 | Builder legacy direct apply retirement | `builder_apply_signals` is retired/fail-closed and writes no LifeModel; normal product flow remains `builder_create_proposals`; no dev/migration direct-apply override remains |
| W91 | Calibration direct/evolution legacy write retirement | `apply_calibration(mode="direct")` and `run_micro_evolution` are retired/fail-closed for durable LifeModel writes; normal flow remains `calibration_create_proposals` / proposal mode |
| W92 | Feedback evolution legacy write retirement | `apply_feedback_evolution` is retired/fail-closed for LifeModel and `evolution_rules` writes; reports are metadata-safe/read-only |
| W93 | Governed Snapshot restore / Data import | `restore_snapshot` and `import_all_data` require explicit governed requests, pre-change snapshots, validation, and metadata-safe audit/count/hash results; no legacy restore/import override remains |
| W94 | Governed manual LifeModel editor override | `save_life_model` requires explicit user intent, risk acknowledgement, pre-change snapshot, typed materializer context, and metadata-safe audit |
| W95 | Proposal PatchSource mapping closure | `PatchSource` has dedicated variants for all ProposalSource values; accepted proposal apply uses exact source-specific mapping, no Manual fallback blocker |
| W96 | State / Daily Goal source-data boundary preserved | State and daily-goal compatibility materialization remains source-data / low-risk transient compatibility view, not accepted durable LifeModel-HS truth |
| W97 | Final legacy direct-write convergence inventory | `overall_converged=true`, `all_direct_writes_converged=true`, `high_risk_legacy_direct_write_count=0`, `proposal_first_convergence_complete=true`, metadata-safe reports, default Chat unchanged |
| W98 | Plan-Execute product contract and weekly scenario | Typed weekly planning contract, max step/risk/action bounds, metadata-safe authority report, no direct writes |
| W99 | Plan-Execute session store and non-default commands | Durable `PlanExecuteSession` store plus explicit create/get/list/update/finalize/cancel/execute command surface; ordinary Chat does not call it |
| W100 | Review/edit/finalize lifecycle | Draft plans can be edited and finalized; execution fails closed before finalize and after cancel |
| W101 | Proposal-first step execution | Read-only steps produce metadata-safe observations; write-like steps create Review Center proposals only and are idempotently linked |
| W102 | AgentRun trace/proposal linkage | Product sessions create/update metadata-safe `plan_execute_product` AgentRun traces with session/proposal/status counts and no raw content |
| W103 | Frontend weekly planning surface | Workspace weekly planning panel supports create, edit, finalize, execute, observation display, proposal links, and source run link |
| W104 | Safety/isolation hardening | Default Chat entrypoint guard list includes product commands; proposal mapping has `PlanningSession`; regression tests cover metadata and isolation |
| W105 | Docs and verification sync | Authority docs, progress index, trace/Runs UI, and verification matrix synced; default Chat remains `legacy_stream` |
| W106 | RuntimeStrategy descriptor and registry readiness | ReAct/PlanExecute executable descriptors are metadata-safe; readiness fails closed for missing/duplicate/mismatched/migration-granting descriptors |
| W107 | Strategy selection candidate matrix | StrategySelector emits metadata-safe candidate matrix/explanation while preserving ReAct/PlanExecute selection behavior and local-only blocking |
| W108 | MultiStrategy execution report envelope | Runtime output includes selector/registry/descriptor/payload/governance/side-effect/default-Chat report; blocked paths still report without adapter execution |
| W109 | Non-default registry status command | `get_runtime_strategy_registry_status` returns read-only maturity status, executable descriptors, future descriptors, and zero execution/write proof |
| W110 | Preview/product trace convergence | MultiStrategy preview and Plan-Execute product traces share runtime strategy trace vocabulary without raw prompt/output/plan/tool/proposal payloads |
| W111 | Future strategy boundary descriptors | Direct, Layered, Workflow, Proactive, and Reflective are disabled/declarative-only future descriptors, not executable capabilities |
| W112 | Default Chat isolation hardening | Ordinary `send_message` / `start_stream_message` forbidden-call tests include W106-W113 command/helpers; readiness/status is not migration authority |
| W113 | Docs and verification sync | Historical RuntimeStrategy maturity handoff; superseded by W114-W123 ReAct Beta execution hardening |
| W114-W123 | ReAct Beta Execution Hardening | ReAct readiness/status, AgentLoop action schema/parser, Tool Registry Beta taxonomy/readiness, manifest authority, `react_trace` envelopes, permission/replay scope, proposal-first writes, non-default status command, Runs/Trace hardening, and docs sync are complete; not default Chat migration and not full Beta declaration |
| W124-W127 | Backend Completion Goal 1: Master Contract And Schemas | Pure backend readiness/contract report, typed LifeEvent store skeleton, deterministic low-risk Signal extractor, and safe Signal -> EvidenceStore candidate bridge are complete; no command/frontend/runtime/model/tool/default Chat impact |
| W128-W130 | Backend Completion Goal 2: Evidence Graph v1 | Pure backend evidence graph/timeline read model, support/opposition links, dedupe clusters, source weights, cluster summaries, conflict/decay/cooldown, and rejected-similar cooldown metadata are complete; no command/frontend/runtime/model/tool/default Chat impact |
| W131-W133 | Backend Completion Goal 3: Maturation Engine v1 | Pure backend Maturation Engine v1 candidate generation, proposal outcome evidence convergence, and deterministic suppression/correction are complete for low-risk planning/energy/work-style/communication domains; no command/frontend/runtime/model/tool/default Chat impact |
| W134-W136 | Backend Completion Goal 4: Accepted Guidance And Materialization | Pure backend accepted guidance lifecycle, governed LifeModel compatibility materialized view provenance, and version diff/rollback read model are complete; no command/frontend/runtime/model/tool/default Chat impact |
| W137-W140 | Backend Completion Goal 5: Runtime Guidance Integration | RuntimeHSPacket v2 accepted/trial guidance metadata, non-default ReAct guidance consumption gated by `RuntimeGuidanceConsumptionMode::ExplicitRuntime`, explicit Plan-Execute weekly planning guidance consumption, and Guidance Impact trace/read model are complete; ordinary Chat keeps guidance consumption disabled and has no routing change or migration permission |
| W141-W143 | Backend Completion Goal 6: Policy / Privacy / Tool Governance Hardening | ModelRouter/Privacy HS LocalOnly hard enforcement, ActionExecutor HS tool governance, and Governor unified metadata-safe decision reports are complete; High/Critical privacy cannot select cloud providers, HS LocalOnly has no cloud fallback, unsupported Plugin/A2A tools remain disabled/declarative-only, and ordinary Chat does not consume these results as migration permission |
| W144-W146 | Backend Completion Goal 7: Backend Golden Paths | Pure backend/core Weekly Planning, Low-Energy Support, and Preference Correction golden paths are complete; no default Chat migration, no ordinary send/stream replacement, no Tauri command, no UI, no durable LifeModel/Memory/external provider state write, and ordinary Chat does not call golden path helpers or treat golden path ready as migration permission |
| W147-W149 | Backend Completion Goal 8: Pre-UI Backend Contract Freeze | Pure backend/core read-model contracts for Learning Inbox, Evidence Timeline, Proposal Review, Runtime Trace, Guidance Impact, Privacy Controls, and LifeModel Overview are frozen; final backend completion gate report and docs/progress/verification sync are complete; no command/UI/store write/runtime/model/tool/default Chat impact |
| W150-W158 | Skill Runtime Beta Maturity | Built-in skill readiness, bounded metadata-safe context, HS privacy/model-route governance, fail-soft output envelopes, proposal candidate governance, plugin declarative-only boundary, non-default read-only status command, Runs/Review trace integration, and docs sync are complete; no ordinary Chat routing change and no migration permission |
| Main Chat Agent Execution v1 | Main Chat Agent remediation | Ordinary `send_message` / `start_stream_message` enter AgentIngress and governed task sessions with transcript/action queue foundations; DirectAnswer is on a real strategy path with send/stream AgentRun, prompt/context transcript, task-session completion proof, and L2 scheduler/provider generation trace proof; ReActToolExecution attempts the governed plan-guided AgentLoop first with a metadata-safe tool-candidate contract, generic MCP read bounded read-only manifest candidate set, deterministic capability/name/tag ranking evidence, provider/model-ranked preselection local HTTP proof, candidate rank/source/capability digest/bounded safe capability labels/sanitized match reason metadata, model-selected ExecutionPolicy metadata, governed candidate arguments source/digest metadata, high-risk/confirmation/write-like candidate exclusion, exact `toolset_allowlist` target enforcement, and exact action-target candidate enforcement, with ReAct tool-selection plan/candidate helpers extracted to `src-tauri/src/main_chat_react_tool_selection.rs`, ReAct AgentLoop attempt execution/runtime helper types/follow-up synthesis/action-to-tool-call conversion/tool-call metadata helpers extracted to `src-tauri/src/main_chat_react_runtime.rs`, ReAct ActionExecutor-backed fallback execution extracted to `src-tauri/src/main_chat_react_execution.rs`, proposal/ToolPermission proposal support helpers extracted to `src-tauri/src/main_chat_proposal_support.rs`, HS runtime packet/topic/tool-requirement helpers extracted to `src-tauri/src/main_chat_hs_runtime.rs`, task-session/transcript/action-queue runtime support helpers extracted to `src-tauri/src/main_chat_runtime_support.rs`, send command state executor extracted to `src-tauri/src/main_chat_send.rs`, strategy dispatch extracted to `src-tauri/src/main_chat_strategy.rs`, and stream command state executor extracted to `src-tauri/src/main_chat_streaming.rs`; rejects no-planned-action AgentLoop results as incomplete tool execution, blocks model-selected exact-target allowlist misses / wrong action-target pairs / write-like or unsupported action types / unknown non-candidate calls as explicit `model_selected_disallowed_tool` blockers without single-step fallback, replaces model-supplied arguments with exact allowlist governed executor input before execution, and blocks policy-denied selected candidates as `model_selected_tool_policy_blocked`, otherwise falling back to a single-step ActionExecutor-backed read path with direct read parser/executor input alignment, eval-gated memory/session multi-step read/observe/follow-up proof, web AgentLoop blocker proof, fixture-backed successful web read AgentLoop proof, registered MCP AgentLoop success proof, registered MCP ToolPermission proposal proof, and governed follow-up synthesis; Main Chat context assembly now uses a controlled knowledge-format loader for bounded workspace/configured `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, and selected `SKILL.md` surfaces, with context compilation / selected-skill sanitization extracted to `src-tauri/src/main_chat_context_loader.rs`, optional sanitized selected-skill id plumbing through send/stream command surfaces, frontend Tauri wrappers, and an explicit manual Chat composer source; `proposal.create`, safe retry/replay, permission-preserving resume, accepted ToolPermission resume replay, cancel, execution task panel, and Review Center accept/resume handoff are covered; a 100-case runtime harness covers per-capability execution plus provider/local-only/eval-generation/webAgentLoop/mcpAgentLoop/mcpToolPermissionProposal metrics; a fail-closed live-provider eval preflight reports missing opt-in/key/network/non-scripted/local-only blockers without invoking a model; Tauri mock IPC covers send/stream DirectAnswer, L2 scheduler/provider generation trace, governed file-read, PlanExecute draft, proposal-path, registered-MCP AgentLoop success, registered-MCP ToolPermission proposal, web AgentLoop blocker, fixture-backed web AgentLoop success, web-policy blocker, and missing-MCP blocker; a 24-case send/stream command-surface eval gate keeps legacy fallback=0 and silent write=0; live-provider-backed generation eval, broader live/provider-backed web/MCP manifest coverage, and broader provider/live proposal-permission proof remain required before completion |

## 4. Current Authoritative Entry Points

| Document | Use for |
| --- | --- |
| `AGENTS.md` | Agent instructions, project context, Tool Taxonomy, and current hard constraints. |
| `plans/main_chat_stage1_preparation_index.md` | Current Stage 1 preparation index, required reading, phase map, readiness boundaries, and short CLI Goal prompt. |
| `plans/main_chat_agent_stage1_dogfood_goal_spec.md` | Current CLI Goal-mode entry for Main Chat Agent Stage 1 Real End-to-End Dogfood. Build on Beta v1 foundations; do not create parallel runtime systems. |
| `plans/main_chat_agent_productization_v1_goal_spec.md` | Next development goal spec for Main Chat Agent Control Plane, product eval, runtime-backed UI state, L0-L2 product completion, and narrow L3/L4/L5 continuity. |
| `plans/openlife_agent_product_capability_matrix_v1.md` | Product capability matrix for the next Agent phase: capability levels, current/target state, UI/backend dependencies, acceptance gates, and Codex/Hermes/OpenClaw gaps. |
| `plans/main_chat_agent_product_eval_scenarios_v1.md` | Product-level Main Chat Agent scenario set covering DirectAnswer, read tools, ReAct, PlanExecute, memory, permission, skill, recovery, and final delivery. |
| `plans/main_chat_agent_control_plane_ui_contract_v1.md` | Runtime-backed Agent Control Plane UI objects, state machine, controls, streaming rules, and anti-fake UI constraints. |
| `plans/main_chat_runtime_to_ui_evidence_mapping_v1.md` | Contract mapping runtime evidence to UI task, action, observation, blocker, proposal, and final delivery objects. |
| `plans/main_chat_permission_proposal_memory_ux_contract_v1.md` | Permission, ToolPermission proposal, memory proposal, conflict, rollback, and Review Center UX contract. |
| `plans/main_chat_final_delivery_contract_v1.md` | Final delivery object, completion statuses, section requirements, and negative assertions for task completion. |
| `plans/main_chat_agent_v1_stabilization_goal_spec.md` | Previous stabilization / acceptance-blocker remediation entry. Keep as audit trail, not the current Goal-mode entry. |
| `plans/main_chat_agent_migration_v1_goal_spec.md` | Main Chat Agent Execution v1 capability target and audit trail. Do not directly restart it as the next broad Goal unless explicitly requested. |
| `plans/openlife_lifemodel_governed_agent_runtime.md` | Current LifeModel-Governed Runtime program and post-W149 implementation options. |
| `plans/lifemodel_governed_backend_completion_goal_spec.md` | Completed Backend Completion master spec through Goal 8 / W147-W149. |
| `plans/lifemodel_governed_runtime_progress.md` | W1-W158 structured status index and compressed guardrail map. |
| `plans/skill_runtime_goal_spec.md` | Completed W150-W158 Skill Runtime Beta Maturity CLI spec/audit trail. |
| `plans/react_beta_execution_hardening_goal_spec.md` | Completed W114-W123 ReAct Beta Execution Hardening CLI spec/audit trail. |
| `plans/runtime_strategy_maturity_goal_spec.md` | Completed W106-W113 RuntimeStrategy / Multi-Strategy Runtime Maturity spec/audit trail. |
| `plans/plan_execute_product_vertical_goal_spec.md` | Completed W98-W105 Plan-Execute Product Vertical spec/audit trail. |
| `plans/lifemodel_maturation_goal_plan.md` | Completed W73-W78 LifeModel Maturation proof-slice preparation/spec and audit trail. |
| `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` | LifeModel-HS source-of-truth, proposal-first, privacy, materialized-view hard rules. |
| `plans/openlife_react_beta_roadmap.md` | ReAct execution seriousness, Beta gates, tool/action/audit baseline. |
| `plans/lifemodel_hs_mvp_task_specs.md` | Coding-ready LifeModel-HS MVP task specs. |
| `plans/lifemodel_hs_legacy_write_path_audit.md` | Direct-write convergence backlog and safety map. |
| `plans/openlife_development_plan.md` | Current execution route, already aligned to the LifeModel-Governed program. |
| `plans/openlife_codex_execution_playbook.md` | How to slice and verify individual Codex tasks. |

## 5. Historical Or Scoped Reference Documents

These files are useful context, but they are not current execution authority:

| Document | Status |
| --- | --- |
| `OpenLife_Final_PRD.md` | Historical long-form PRD. Do not use for current task order. |
| `OpenLife_PRD_v2_Agent_Framework.md` | Product definition baseline only; implementation order is governed by current Agent/LifeModel runtime docs. |
| `UI_BETA_SHELL_CONTRACT.md` | Historical Beta shell contract; do not use for current Beta status, navigation, or Tool Taxonomy. |
| `plans/openlife_alpha_beta_plan.md` | Historical Alpha to Beta productization plan. |
| `plans/openlife_remaining_tasks_plan.md` | Historical sprint debt plan. Re-check code before using any item. |
| `plans/openlife_stabilization_and_spine_consolidation_plan.md` | Historical stabilization plan. |
| `plans/builder_life_model_design.md` | Builder UX/domain reference only; LifeModel-HS governance overrides direct-write assumptions. |
| `plans/frontend_experience_rebuild_plan.md` | Frontend UX reference only; current IA is governed by Agent/LifeModel-HS docs. |
| `plans/engineering_structure_notes.md` | Engineering history/reference only. |
| `architecture_diagram.md` | Snapshot diagram; verify against code and current program. |
| `BETA_CHECKLIST.md` | Historical checklist; current Beta/tool status is in AGENTS and roadmap. |
| `docs/ARCHITECTURE.md` | Quick architecture explainer; defer to current program for implementation order. |
| `docs/BETA_USER_GUIDE.md` | Historical/draft user guide; current project has not declared full Beta. |
| `docs/DEV_HANDOVER.md` | General handover; defer to this index and AGENTS for current Agent work. |
| `docs/decisions/0001-lifemodel-patch.md` | Historical ADR; direct-write compatibility assumptions are superseded by ADR 0013 and W90-W97 convergence. |
| `docs/decisions/0002-proposal-unified.md` | Historical ADR for Proposal intent; current Proposal semantics are governed by W90-W123 docs. |
| `docs/decisions/0003-agent-run-tracking.md` | Historical ADR for AgentRun intent; current trace semantics include W10, W98-W105, W106-W113, and W114-W123. |

## 6. Tool Status Guardrail

`calendar.propose_event` and `email.propose_draft` are P1 proposal-only
governed executors. They create `ScheduledTask` / `DataExport` proposals and
must not perform real calendar writes, email sends, or `ExternalWriteAction`
fallback unless a future governed provider executor and tests are added.

`ExternalWriteAction` proposal creation must enforce pre-insert size limits and
payload minimization. This is a hard acceptance gate.

`run_multi_strategy_agent_preview` is a preview/beta command. Its W10 AgentRun
audit is a metadata-safe outer run; any ReAct inner run id is child metadata and
must not become the product trace's primary query id. Do not replace
`send_message` or the default Chat path just because the preview path works.

`check_runtime_migration_gate`, W19 pilot eligibility, W24/W27/W30/W33/W37/
W42/W45/W48/W57/W60 readiness gates, W25/W35/W46/W58 plan drafts, W26/W29/W32/
W36/W41/W44/W47/W59 review evidence, W28/W31/W40/W43 non-default run/preview
commands, and W56 status commands are not migration permission. They are
readiness, review, preview, draft, evidence, or status surfaces only.

For the historical W19-W72 pre-migration slices, default `Send`, ordinary
`send_message`, and ordinary `start_stream_message` were required to remain on
`legacy_stream` and not call W19-W60 command surfaces. In the active Main Chat
Agent Execution v1 remediation, ordinary Chat now enters AgentIngress /
governed task session scaffolding with visible legacy fallback; W19-W60
readiness/status/preview surfaces still are not migration permission.

W67 is backend-only non-default harness code, W68 is backend-only
send-compatible proof code, W69 is backend-only stream-compatible boundary
proof code, W70 is backend-only executor attachment gate report code, W71 is
backend-only disabled executor skeleton contract code, and W72 is backend-only
disabled skeleton binding integrity report code. They do not add a
Tauri command, frontend surface, Settings surface, runtime/model/tool execution,
business write, controlled executor attachment, real stream emission, event
channel, route cutover, or migration permission.
Ordinary default Chat entries must not call any of them. W68 only proves a
SendMessageResult-compatible metadata shape for a controlled adapter candidate;
W69 only proves a `start_stream_message`-compatible metadata boundary with
streamStarted/eventChannelOpened/streamEventsEmitted=false; W70 only reports
metadata readiness for an executor skeleton discussion while keeping
executor_attachment_allowed=false, route_cutover_permission=false, and
migrationPermission=false; W71 only defines disabled/unattached/no-run
send/stream metadata-only placeholders while keeping executor_runnable=false
and invocation_allowed=false; W72 only verifies W71 input/skeleton and W70 gate
binding integrity while keeping executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, and migrationPermission=false; and default Chat
remains `legacy_stream`.
W73/W74/W75/W76/W77/W78 are LifeModel maturation slices only: readiness, non-default
invocation, proposal outcome evidence link, and low-energy collaboration rule
candidate aggregation plus accepted-rule selection proof and trace visibility
proof. They do not add default Chat routing authority or ordinary Chat
auto-maturation.
W79 is a legacy direct-write convergence inventory guard only; it makes the
legacy audit map testable but does not complete direct-write convergence. W80
adds metadata-safe audit to the manual editor direct save path, but it does not
make that path proposal-first or fully converged.
W81 adds a default fail-closed dev/migration gate to Builder legacy direct
apply and proves no-signal completion does not write durable LifeModel truth,
but Builder legacy direct apply remains a high-risk blocker until removed or
converted to proposal-first.
W82 adds a default fail-closed dev/migration gate to Calibration direct apply
and micro-evolution persistence, keeps legacy responses metadata-safe, and
keeps normal product flow on `calibration_create_proposals`; Calibration
direct/evolution remains a high-risk blocker until removed or converted to
proposal-first.
W83 adds a default fail-closed dev/migration gate to Feedback evolution direct
apply, keeps legacy responses metadata-safe, and makes
`generate_evolution_report` read-only/no active `evolution_rules` write;
Feedback evolution direct apply remains a high-risk blocker until removed or
converted to proposal-first.
W84 adds default fail-closed dev/migration/manual-restore gates to Snapshot
restore and Data import, keeps legacy responses metadata-safe, and leaves
export/read-only paths unchanged; restore/import remain high-risk blockers until
removed or converted to governed rollback/migration flows.
W85 adds only a backend/internal source-data boundary proof for State / Daily
Goal. It explicitly acknowledges the current `persist_life_model` compatibility
view / YAML write while classifying it as source-data compatibility materialized
state, not accepted durable LifeModel-HS truth. It does not add a command,
frontend surface, runtime/model/tool call, ProposalStore/EvidenceStore/AgentRun
write, ordinary Chat integration, default Chat routing change, proposal-first
conversion, or fully-converged marker.
W86 adds only a backend/internal caller matrix for the LifeModel compatibility
materializer. It classifies all current production `persist_life_model` and
`LifeModelManager::save` related entries without changing routing, product
behavior, the `persist_life_model` signature, or legacy path availability. It
is metadata-safe, not migration permission, not runtime authority, and not
convergence completion; W87 is the caller restriction slice.
W87 adds that caller restriction slice, requiring typed caller context at every
production `persist_life_model` callsite and adding a direct-save guard to
snapshot restore. W88 fixes accepted LifeModel proposal PatchStore source
mapping, and W89 proves that mapping/readiness remains metadata-safe and
apply-path bound. This is still not full convergence: W89 does not retire
legacy paths, alter default Chat routing, grant migration/runtime authority, or
resolve the fallback source policy blocker.

## 7. Agent Rules

- Always read `AGENTS.md`, this file, and
  `plans/openlife_lifemodel_governed_agent_runtime.md` before starting a new
  architecture/runtime/LifeModel/tool task.
- Use `plans/lifemodel_governed_runtime_progress.md` for W1-W105 status, not as
  an implementation roadmap.
- Do not use historical plans to override current ordering, current Tool
  Taxonomy, or the default Chat `legacy_stream` boundary.
- If implementation changes tool status, proposal semantics, runtime authority,
  model routing, LifeModel source-of-truth, privacy boundaries, or default Chat
  routing, update the relevant docs in the same task and run the implementation
  verification gate.

## 8. Next Recommended Sequence

```text
W63 complete -> W64 authority compression validated -> W65 backend-only
descriptor skeleton complete -> W66 controlled adapter contract report complete
-> W67 non-default invocation harness complete -> W68 send-compatible proof
complete -> W69 stream-compatible boundary proof complete -> W70 executor
attachment gate report complete -> W71 disabled executor skeleton contract
complete -> W72 disabled skeleton binding integrity report complete -> W73
LifeModel maturation readiness report complete -> W74 non-default maturation
invocation complete -> W75 proposal outcome evidence link complete -> W76
low-energy collaboration rule candidate complete -> W77 accepted rule to
RuntimeHSPacket selection proof complete -> W78 run trace visibility proof
complete -> W79 legacy direct-write convergence inventory guard complete ->
W80 manual LifeModel override audit guard complete -> W81 Builder legacy direct
apply dev-gate complete -> W82 Calibration direct apply legacy gate complete
-> W83 Feedback evolution legacy direct apply gate complete -> W84 Snapshot
restore / data import legacy direct write gate complete -> W85 State / Daily
Goal source-data boundary proof complete -> W86 LifeModel materializer caller
matrix complete -> W87 LifeModel materializer caller restriction complete ->
W88 Proposal Application Source-Specific Patch Mapping complete -> W89
Proposal Application Source-Specific Patch Audit / Readiness complete -> W90
Builder legacy direct apply retirement complete -> W91 Calibration
direct/evolution retirement complete -> W92 Feedback evolution retirement
complete -> W93 governed Snapshot restore / Data import complete -> W94
governed manual LifeModel editor override complete -> W95 Proposal PatchSource
mapping closure complete -> W96 State / Daily Goal boundary reconciliation
complete -> W97 final Legacy Direct-Write Convergence inventory complete ->
W98 product contract complete -> W99 durable session store and commands complete
-> W100 review/edit/finalize lifecycle complete -> W101 proposal-first step
execution complete -> W102 AgentRun trace/proposal linkage complete -> W103
frontend weekly planning surface complete -> W104 safety/isolation hardening
complete -> W105 docs/progress/verification sync complete -> W106
RuntimeStrategy descriptor/readiness complete -> W107 selection candidate
matrix complete -> W108 execution report envelope complete -> W109 non-default
registry status command complete -> W110 preview/product trace convergence
complete -> W111 future strategy declarative boundary complete -> W112 default
Chat isolation hardening complete -> W113 docs/progress/verification sync
complete -> W114 ReAct Beta readiness contract complete -> W115 AgentLoop
action schema/parser hardening complete -> W116 Tool Registry Beta
taxonomy/readiness complete -> W117 ActionExecutor manifest authority complete
-> W118 AgentRun action/observation trace envelope complete -> W119 permission
proposal/replay hardening complete -> W120 proposal-first write hardening
complete -> W121 non-default ReAct Beta status harness complete -> W122
Runs/Trace lifecycle UI hardening complete -> W123 docs/progress/verification
sync complete -> W124 backend completion readiness/contract report complete ->
W125 LifeEvent schema/store contract complete -> W126 Signal
schema/deterministic extractor complete -> W127 safe Signal -> EvidenceStore
candidate bridge complete -> W128 Evidence support/opposition/dedupe graph
complete -> W129 conflict/decay/cooldown complete -> W130 Evidence Timeline
read model complete -> W131 low-risk multi-domain maturation candidate
generation complete -> W132 proposal outcome to evidence convergence complete
-> W133 candidate suppression/correction complete -> W134 accepted guidance
lifecycle complete -> W135 governed materialized LifeModel view provenance
complete -> W136 version diff and rollback read model complete -> W137
RuntimeHSPacket v2 guidance contract complete -> W138 ReAct guidance
consumption complete -> W139 Plan-Execute guidance consumption complete ->
W140 Guidance Impact read model complete -> W141 ModelRouter/Privacy HS
hardening complete -> W142 ActionExecutor HS tool governance complete -> W143
Governor unified decision report complete -> W144 Weekly Planning golden path
complete -> W145 Low-Energy Support golden path complete -> W146 Preference
Correction golden path complete -> W147 UI read model contract freeze complete
-> W148 final backend completion gate complete -> W149 docs/progress/
verification sync complete -> W150 Skill Runtime readiness complete -> W151
bounded context assembly complete -> W152 HS privacy/model-route governance
complete -> W153 output envelope/trace complete -> W154 proposal candidate
governance complete -> W155 plugin skill boundary complete -> W156 read-only
status command complete -> W157 Runs/Review trace integration complete -> W158
docs/progress/verification sync complete.
Backend Completion Goal 8 and Skill Runtime Beta Maturity are complete. Full
Beta still needs separately scoped product surface work. Any future
default Chat executor implementation or route cutover remains a separate
reviewed task that preserves default Chat legacy_stream until a route change is
explicitly implemented, reviewed, verified, and authorized.
```

For docs-only index整理, `git diff --check` plus targeted `rg` validation is
enough. Run `make ci` when code, tests, package configuration, or runtime
behavior changes.
