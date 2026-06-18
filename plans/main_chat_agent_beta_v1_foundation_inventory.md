# Main Chat Agent Beta v1 Foundation Inventory

> Date: 2026-06-18
> Phase: 0 foundation inventory
> Goal entrypoint: `plans/main_chat_agent_beta_v1_goal_spec.md`

## Scope And Method

This inventory classifies the foundations required by Main Chat Agent Beta v1
from current repository evidence. It does not treat plans or contract documents
as implementation evidence.

Status meanings:

- `verified`: implemented and backed by runtime, command-surface, UI, or gate
  evidence strong enough to reuse.
- `partial`: useful implementation exists, but Beta v1 still lacks required
  product, UI, eval, live, or readiness proof.
- `missing`: the required Beta v1 artifact or behavior is not implemented in the
  repo.

Evidence commands run for this inventory:

- `cargo test -p openlife-core main_chat_agent -- --nocapture`
  - result: 36 passed, 0 failed.
- `cargo test -p openlife-tauri main_chat_agent_productization -- --nocapture`
  - result after Phase 2/5 Beta evidence updates: 37 passed, 0 failed, 1 ignored external-live test.
- `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture`
  - result after expanding the command-surface matrix to 38 cases: 23 passed, 0 failed.
- `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture`
  - result after expanding the command-surface matrix to 38 cases: 86 passed, 0 failed, 1 ignored external-live test.
- `cargo test -p openlife-tauri main_chat_agent_execution_v1 -- --nocapture`
  - result after expanding the command-surface matrix to 38 cases: 3 passed, 0 failed.
- `cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture`
  - result after adding Phase 5 Beta readiness command/report and live opt-in audit: 3 passed, 0 failed.
- `cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture`
  - result: 9 passed, 0 failed.
- `corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts`
  - result after adding Beta readiness frontend wrapper/mock: 117 passed, 0 failed.
- `corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts`
  - result: 116 passed, 0 failed.
- `corepack pnpm --dir frontend typecheck`
  - result: passed.

## Foundation Dependencies

| Component | Status | Runtime evidence | Command-surface evidence | UI evidence | Tests / gates checked | Blockers or gaps | Development decision |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Governed Main Chat ingress and strategy routing | verified | `src-tauri/src/main_chat_runtime_support.rs` creates task turns and transcript route decisions; `src-tauri/src/main_chat_strategy.rs` dispatches governed strategies. | `src-tauri/src/main_chat_send.rs` and `src-tauri/src/main_chat_streaming.rs` both call `try_run_main_chat_agent_strategy`; Beta readiness command now reports command-surface totals and zero legacy fallback. | `frontend/src/pages/ChatPage.tsx` renders agent state through `AgentControlPlane`. | Core `main_chat_agent`, Tauri command-surface/productization filters, and focused Beta readiness tests passed. | External live remains opt-in and was not run. | Reuse. |
| `AgentTaskSession`, `ActionQueue`, execution transcript, task controls | verified | `openlife-core/src/agent/main_chat_agent_v1.rs` defines `AgentTaskSessionStore`, `ActionQueueStore`, and `ExecutionTranscriptEntry`; `src-tauri/src/main_chat_task_controls.rs` implements list/detail/resume/retry/cancel. | Existing task-control Tauri commands and command-surface eval exercise governed send/stream paths; Beta readiness aggregates recovery/permission dimensions from Product Maturity v2 and command-surface evidence. | `AgentControlPlane` exposes resume, retry, cancel, permission, proposal, and rollback controls when runtime evidence supports them. | Core `main_chat_agent`; Tauri productization; focused Beta readiness; frontend ChatPage/AgentControlPlane tests passed. | External live task continuation evidence remains opt-in. | Reuse. |
| DirectAnswer path | verified | DirectAnswer is represented as governed strategy/session/final transcript and provider trace in Main Chat Agent v1 runtime. | `send_message` and `start_stream_message` command-surface eval covers DirectAnswer and scripted provider generation. | Direct answers render compactly without fake action timelines in the control plane tests. | Core `main_chat_agent`; Tauri productization; frontend focused tests passed. | Beta B1 fixture is encoded and currently covered by existing command-surface/productization evidence. | Reuse. |
| ReAct / governed read / blocker paths | verified | `src-tauri/src/main_chat_react_runtime.rs`, `main_chat_react_execution.rs`, and `main_chat_react_tool_selection.rs` implement AgentLoop, allowlists, policy blockers, observations, and follow-up synthesis. | `src-tauri/src/main_chat_command_surface_eval.rs` and `src-tauri/src/main_chat_command_surface_tests.rs` cover direct answer, accepted memory context, memory conflict comparison, file, session search, selected skill context, knowledge asset inspection, knowledge asset edit proposal, multi-read AgentLoop, web, MCP, proposal, and blocker send/stream cases in a 38-case matrix. | `AgentControlPlane` renders actions, observations, blockers, retry/cancel controls, and final delivery sections. | Core `main_chat_agent`; Tauri productization; command-surface; focused Beta readiness; frontend focused tests passed. | Full external provider ReAct proof remains opt-in. | Reuse. |
| Plan-Execute draft/edit/confirm/skip/execute/review objects | verified | `openlife-core/src/agent/plan_execute.rs` and `src-tauri/src/main_chat_plan_interaction_eval.rs` cover revisioned plan sessions, stale revision blockers, skip, execute, cancel, and review. | `src-tauri/src/commands/agent_runtime/plan_execute_product.rs` exposes plan product commands; B8 now maps to the existing `PlanExecuteDraft` ordinary send/stream command-surface proof. | `AgentControlPlane` has plan controls; ChatPage wires plan controls into the task panel. | `cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture`, focused B8 Beta proof, productization, command-surface, and final-acceptance filters passed. | None for deterministic default scope. | Reuse. |
| Proposal and permission flows | verified | ProposalStore/ToolPermission proposal paths are linked to pending actions and exact replay evidence. | Command-surface eval covers governed proposal path and ToolPermission proposal cases. | `AgentControlPlane` exposes approve/deny/defer/review-center style controls from runtime proposal evidence. | Core and Tauri productization filters passed. | Beta fixture outcome semantics still missing. | Reuse. |
| Memory lifecycle and rollback | verified | `openlife-core/src/agent/memory_lifecycle.rs` plus `src-tauri/src/main_chat_memory_lifecycle_eval.rs` cover pending proposal, accept, reject, scoped memory, ambiguity, rollback, inactive memory, provenance, and MR-09 memory conflict state through `EvidenceStore`/`evaluate_evidence_graph` plus lifecycle `conflict_ids`. | Memory commands exist in `src-tauri/src/commands/memory.rs`; Product Maturity v2 memory gate is command-returnable through final readiness. B21 also runs ordinary send/stream with `memory_conflict_compare_success`; Beta readiness includes the Memory dimension. | `AgentControlPlane` renders rollback affordance for memory lifecycle evidence; ChatPage wires `onRollbackMemory`. | Tauri Product Maturity v2 memory tests passed 9-scenario MR matrix; B21 focused Beta proof and Beta readiness tests passed. | None for deterministic default scope. | Reuse. |
| Durable/replayable task delta events | verified | `src-tauri/src/main_chat_event_stream.rs` stores monotonic per-task events and replay state; events include route/action/observation/proposal/blocker/memory/final delivery. | `list_main_chat_agent_events` is exposed as a Tauri command; event gate command returns auditable report; Beta readiness includes the Events dimension. | ChatPage applies events by sequence, replays gaps, and falls back to snapshot recovery; AgentControlPlane displays event stream status and sequence. | Tauri Product Maturity v2 event tests, focused Beta readiness tests, and frontend focused tests passed. | None for deterministic default scope. | Reuse. |
| Long task continuity list/detail/resume safety | verified | `src-tauri/src/main_chat_task_continuity_eval.rs` and task controls validate summaries, blocked detail, exact permission resume, changed-target blocker, retry, stale, terminal no-resume, and persistence-style detail. | Task control commands are registered and exercised by productization tests; Beta real-task fixtures carry required runtime/UI/final-delivery evidence for continuity and recovery scenarios. | AgentControlPlane and ChatPage expose resume/retry/cancel and continuity states from runtime data. | Tauri Product Maturity v2 continuity tests, productization, focused Beta readiness, and frontend focused tests passed. | None for deterministic default scope. | Reuse. |
| Skills/tool product surface and selected `SKILL.md` plumbing | verified | `src-tauri/src/main_chat_context_loader.rs` loads only sanitized selected skills; `src-tauri/src/main_chat_skills_tools.rs` covers selected, cleared, unsafe, write-like, and retry scenarios. | Frontend Tauri wrappers pass selected skill aliases into send/stream commands; B6 now has ordinary send/stream selected-skill context evidence for `phase_e_review` with unselected skill exclusion; Beta readiness includes Tools/Permissions dimensions. | ChatPage exposes selected skill evidence; AgentControlPlane/trace surfaces show skill/tool state. | Tauri Product Maturity v2 skills tests, focused Beta readiness tests, and frontend Tauri tests passed. | None for deterministic default scope. | Reuse selected-skill/tool surface. |
| Knowledge assets and context inventory | partial | Controlled loader supports bounded `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, selected `SKILL.md`, config-backed knowledge roots, digests, and skipped/unselected skill behavior. | Selected skill id is plumbed through ordinary send/stream wrappers; B27 has `knowledge_asset_context_success` ordinary send/stream evidence with four scoped configured assets and policy-override blocking; B28 has `knowledge_asset_edit_proposal` evidence with Review Center proposal creation and no direct file write. | ChatPage shows selected skill evidence, but there is no complete knowledge asset manager for all asset types. | Context loader tests, command-surface/productization filters, focused B27 test, and focused B28 test passed. | Minimum Beta inspection/edit-proposal evidence is covered; broader user-facing rollback/conflict/edit management for all knowledge asset types is still missing. | Reuse the minimum Beta slice and defer broader knowledge manager work. |
| Execution-first UI events and task panel foundations | verified | Productization state snapshots are assembled from runtime evidence, not assistant text. | Send result includes runtime-backed agent state in productization tests; event replay command exists; Beta readiness includes `defaultExperienceRequiredStateCount=11` and `defaultExperienceVerifiedStateCount=11`. | `frontend/src/components/AgentControlPlane.tsx` and ChatPage render task, actions, observations, blockers, proposals, events, controls, and final delivery from typed payloads; frontend Tauri wrapper/mock can invoke the Beta readiness command. | Focused frontend task-panel/Tauri bundle passed 117 tests; Beta default-experience and readiness reports map required states. | Full per-scenario visual UI rendering gate remains a future hardening improvement, not a deterministic default blocker in the current report. | Reuse. |
| External live product evidence gate | partial | `src-tauri/src/main_chat_live_productization_eval.rs` defines six opt-in live product scenarios and rejects local/mock provider credit. | Existing live-provider harness and final gates fail closed without opt-in/credentials. | Product-level live UI mapping is required by the live productization gate but was not externally executed in this inventory run. | Tauri productization filter passed fail-closed/local-provider rejection tests; one external test ignored by design. | No real external provider evidence was run; external live remains opt-in and separate. | Keep separate; do not count toward deterministic Beta readiness. |
| Final readiness aggregation | verified for deterministic default, partial for external live | `src-tauri/src/main_chat_agent_beta_v1_readiness.rs` builds `MainChatAgentBetaV1ReadinessReport` from isolated eval state, Beta default-experience report, Beta real-task report, Product Maturity v2 final readiness, command-surface counts, no-silent-write count, and no-legacy-fallback count. | `run_main_chat_agent_beta_v1_readiness_gate` is registered as a Tauri command and returns the report without using real app-store state for the default eval run. | `frontend/src/tauri.ts` exposes `runMainChatAgentBetaV1ReadinessGate`; `frontend/src/test/mocks/tauri.ts` includes a metadata-safe mock. | Focused Beta readiness Rust tests passed 3 cases; frontend Tauri wrapper test passed. | External live remains opt-in and was not run; report returns opt-in live blocked by default. | Reuse as Phase 5 aggregation; keep external live separate. |
| Beta real task vertical fixture and harness | verified for deterministic default, partial for opt-in live | `src-tauri/src/main_chat_agent_beta_v1_real_tasks.rs` defines B1-B30 fixtures with `expected_outcome`, `command_surface`, runtime event/action/observation/UI/final-delivery requirements, and fail-closed proof rows. | B1/B2/B3/B4/B5/B6/B7/B10/B16/B17/B18/B21/B22/B23/B24/B27/B28 now map to concrete command-surface cases; B3 has real `session.search` evidence, B4 has accepted memory lifecycle context evidence, B6 has selected `phase_e_review` `SKILL.md` context evidence with unselected skill exclusion, B21 has `memory_conflict:evidence_graph_conflict_count=2` plus lifecycle `conflict_ids=2`, B22 has `multi_read_agent_loop:tool_calls=2:observations=2`, B27 has `knowledge_assets:loaded=4:scope_digest_loaded=true`, and B28 has `knowledge_asset_edit:proposal_created=true:proposed_diff=true:direct_write=false`. | The harness records expected UI states per scenario; Beta readiness aggregates 28/28 default scenario pass count. | `main_chat_agent_beta_v1_real_task_harness_defines_b1_b30_and_marks_phase2_ready`, B3/B4/B6/B21/B22/B27/B28 focused real-task tests, command-surface/final-acceptance filters, and focused Beta readiness tests passed. | B25/B26 remain opt-in live only and were not run. | Reuse as deterministic default real-task evidence. |
| Beta v1 readiness command/report | verified for deterministic default, partial for external live | `MainChatAgentBetaV1ReadinessReport` exists in `src-tauri/src/main_chat_agent_beta_v1_readiness.rs`, builds from isolated eval state, and reports default readiness plus opt-in live blockers separately. | `run_main_chat_agent_beta_v1_readiness_gate` is registered in `src-tauri/src/commands/agent_runtime/mod.rs` and included in the app command handler. | `frontend/src/tauri.ts` exposes the wrapper and `frontend/src/test/mocks/tauri.ts` mocks the report. | `cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture` passed 3 tests; `corepack pnpm --dir frontend test -- src/tauri.test.ts` passed. | External live evidence remains opt-in and not executed in this environment. | Reuse as Phase 5 default-readiness aggregation. |
| Beta v1 release notes | verified | `plans/main_chat_agent_beta_v1_release_notes.md` states default capabilities, proposal-first behavior, unsupported/blocked behavior, task evidence inspection, knowledge asset inspection, readiness commands, and live evidence status. | Not command-surface behavior; release notes reference the readiness and eval commands used to inspect command-surface evidence. | User-facing documentation artifact; no runtime UI claim. | Reviewed alongside the foundation inventory and final readiness report. | External live evidence was not run and is documented as not included in deterministic readiness. | Ship with the Beta v1 evidence bundle. |

## Product Maturity v2 Phase Evidence

| Phase | Status | Evidence | Beta implication |
| --- | --- | --- | --- |
| Phase A: Memory lifecycle gate | verified | `main_chat_product_maturity_v2_memory_lifecycle_eval_covers_mr_matrix` passed; MR gate covers proposal, accept, reject, rollback, ambiguity, scope, provenance, and memory conflict state. | Reuse for Beta memory scenarios and knowledge assets. |
| Phase B: Event delta gate | verified | `main_chat_product_maturity_v2_event_eval_covers_ev_matrix` passed; durable event store/replay command exists. | Reuse for Beta UI/event replay readiness. |
| Phase C: Plan interaction gate | verified | `main_chat_product_maturity_v2_plan_gate_covers_phase_c_pi_matrix` passed; plan command report is auditable. | Reuse for Beta plan/edit/skip/execute/review scenarios. |
| Phase D: Long task continuity gate | verified | `main_chat_product_maturity_v2_task_continuity_eval_covers_lt2_matrix` passed. | Reuse for Beta continuation/retry/cancel/stale scenarios. |
| Phase E: Skills/tool surface gate | verified | `main_chat_product_maturity_v2_skills_tool_eval_covers_sk2_matrix` passed. | Reuse for Beta selected skill and tool surface scenarios. |
| Phase F: External live product evidence | partial | Six opt-in rows exist and fail closed without opt-in; external provider test was ignored in this run. | Keep opt-in and separate; no deterministic Beta credit. |
| Phase G: Final readiness | verified for deterministic default, partial for external live | Product Maturity v2 final readiness gate aggregates deterministic and opt-in live sections; Beta readiness now consumes it and keeps external live separate. | Useful foundation for Beta readiness; external live remains opt-in. |

## Hardening Readiness Dimensions

| Dimension | Status | Evidence | Gaps / blockers | Decision |
| --- | --- | --- | --- | --- |
| Routing | verified | Ordinary send/stream enter task/session and strategy routing; command-surface gates passed; Beta readiness Routing dimension is ready. | External live routing remains opt-in. | Reuse. |
| UI | verified for mapped states | AgentControlPlane and ChatPage tests passed; runtime-backed state payload fail-closed tests passed; Beta readiness reports 11/11 default-experience state mappings verified. | Full per-scenario visual rendering gate remains future hardening. | Reuse. |
| Events | verified | Durable event store, replay command, sequence gap recovery, EV gate, and Beta readiness Events dimension passed. | None for deterministic default scope. | Reuse. |
| Memory | verified | MR gate, rollback/provenance lifecycle, B21 conflict proof, and Beta readiness Memory dimension passed. | None for deterministic default scope. | Reuse. |
| Plan | verified | PI gate passed for draft/edit/confirm/execute/skip/review and stale blockers; Beta readiness Plan dimension is ready. | None for deterministic default scope. | Reuse. |
| Tools | verified | File/session/memory/web/MCP/skill read/blocker paths have runtime and command-surface coverage, including B3 session search, B4 accepted memory context, B6 selected-skill context, B21 memory conflict comparison, B22 multi-read AgentLoop, B27 knowledge asset inspection, and B28 knowledge asset edit proposal through ordinary send/stream; Beta readiness Tools dimension is ready. | External live tool proof remains opt-in. | Reuse. |
| Permissions | verified | ToolPermission proposal/replay, exact scope tests, and Beta readiness Permissions dimension passed. | External live permission proof remains opt-in. | Reuse. |
| Recovery | verified | Retry/cancel/resume/stale/terminal checks passed in task-control and continuity tests; Beta readiness Recovery dimension passed. | None for deterministic default scope. | Reuse. |
| Final delivery | verified for deterministic default | AgentControlPlane renders final delivery sections; productization state payload includes final delivery evidence; B19/B30 are represented in real-task fixture/harness and Beta readiness Final delivery dimension is ready. | Broader visual final-delivery QA remains future hardening. | Reuse. |
| Live provider | partial | Existing gates fail closed and reject local/mock provider credit. | No external live provider evidence was run; do not count as deterministic readiness. | Keep opt-in separate. |
| No silent writes | verified | Existing gates check no silent durable writes and proposal-first memory/write behavior. | Beta-specific harness must continue enforcing this. | Reuse invariant. |
| No legacy bypass | verified | Command-surface eval counts legacy fallback and current tested paths passed; Beta readiness reports `legacyFallbackCount=0`. | None for deterministic default scope. | Reuse checks in Beta gate. |

## Phase 0 Development Decision

Phase 0 is complete enough to build on existing runtime foundations. The current
repo already verifies most Product Maturity v2 deterministic foundations, so
Beta v1 should not create parallel task, event, memory, plan, skill, or tool
systems.

Beta v1 deterministic default readiness is now represented by a structured
Beta readiness command/report and release evidence bundle. The final development
decision for this inventory is:

1. Phase 1: use the Beta default-experience evidence layer that maps each
   required UI state to existing runtime records and ordinary send/stream paths.
2. Phase 2: use the Beta real-task fixture/harness as the default scenario
   source. B1-B30 ids, `expected_outcome`, `command_surface`, required
   runtime/UI/final-delivery evidence, deterministic default evidence, and
   opt-in-live separation now exist; B25/B26 remain opt-in live only.
3. Phase 3: keep planner/executor quality checks on existing
   PlanExecute, AgentLoop, ActionQueue, transcript, blocker, retry, and final
   delivery objects.
4. Phase 4: treat the minimum knowledge asset inspection/edit-proposal surface
   as covered for Beta, while keeping broader knowledge manager work outside
   the Beta readiness claim.
5. Phase 5: use `run_main_chat_agent_beta_v1_readiness_gate` and
   `plans/main_chat_agent_beta_v1_release_notes.md` as the default readiness
   and release-evidence surfaces.

Required default Beta capabilities are covered by deterministic local evidence.
External live remains opt-in and was not run in this environment.
