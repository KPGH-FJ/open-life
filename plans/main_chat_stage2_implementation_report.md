# Main Chat Agent Stage 2 Implementation Report

> Date: 2026-06-20
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: implementation mechanism verified; limited internal trial readiness not granted

## 1. Recommendation

Current recommendation:

```text
not_ready_for_limited_internal_trial
```

Current implementation status:

```text
implementation_complete_for_stage2_mechanism
```

This report does not grant limited internal trial readiness. Stage 2 now has
credited DeepSeek external live-provider P0 scenario evidence with known
artifact build provenance, but still requires real two-reviewer manual P0
dogfood evidence and a final readiness gate run with known build provenance
before the readiness recommendation can become
`ready_for_limited_internal_trial`.

Internal trial readiness is not public beta readiness.

## 2. Completed Phases

| Phase | Status | Evidence |
| --- | --- | --- |
| Phase 0: readiness report skeleton | Complete for mechanism | `run_main_chat_agent_stage2_readiness_gate` exists and aggregates Stage 1/Beta, manual, live, control-plane, memory, recovery, final-delivery, safety, and artifact evidence. |
| Phase 1: manual dogfood evidence flow | Complete for mechanism; real dogfood missing | Reviewer-facing report, non-evidence worksheet, non-evidence JSON artifact template, focused artifact validator command, and machine-readable artifact contract exist. Gate validates S2-D01 through S2-D24, reports `missingScenarioIds` for required P0 rows without real attempts, requires two known reviewers, trace ids, build commits, provider mode, result/severity, reviewer notes, and blocker labels, and rejects placeholder/fake reviewer ids, trace ids, build provenance, and template rows. |
| Phase 2: live provider P0 eval integration | Complete for mechanism; DeepSeek external run credited 10/10 with build provenance | L2-L01 through L2-L10 scenario plans, artifact schema, preflight blockers, final-gate harness adaptation, scenario evidence validation, external-provider identity checks, required-evidence manifests, placeholder/fake response rejection, generated/read-back artifact commit validation, and local/mock/loopback/private-network/placeholder-alias rejection are wired. The current generated artifact proves 10/10 scenario credit for `deepseek` / `deepseek-v4-flash` at commit `092805564d98baec69375f9daffddfd17c01772b`. |
| Phase 3: AgentControlPlane P0 states | Complete for deterministic mechanism | Stage 2 aggregates ten P0 control-plane states from existing typed runtime/UI evidence. `AgentControlPlane` now also renders and copies a reviewer trace strip with full task id, run id, task status, route, and blocker reason-code evidence from typed runtime state for manual dogfood reporting. |
| Phase 4: memory proposal trial flow | Complete for deterministic mechanism | M2-01 through M2-08 aggregate existing memory lifecycle, conflict, rollback, accepted-context, and proposal-first knowledge edit evidence. |
| Phase 5: failure recovery coverage | Complete for deterministic mechanism | R2-01 through R2-10 aggregate missing-source, web policy, MCP missing target, disallowed tool, permission denial/acceptance, retry, cancel, stale resume, and plan-step failure evidence. R2-04 now requires an ordinary Main Chat disallowed-tool probe with explicit blocker, no single-step fallback, and no direct-write evidence. |
| Phase 6: final gate | Complete for mechanism | Final readiness remains fail-closed unless all P0 automated, manual, live, safety, and final-delivery evidence is present and credited. |

## 3. Requirement Audit

| Requirement | Current result | Evidence surface |
| --- | --- | --- |
| Reuse existing Main Chat runtime systems; do not create parallel task/event/memory/plan/proposal/tool systems. | Satisfied for implementation mechanism. | Stage 2 code aggregates existing Stage 1/Beta gates, Main Chat task/session/runtime evidence, proposal/memory coverage, and live harness outputs instead of defining replacement runtime systems. |
| Provide `run_main_chat_agent_stage2_readiness_gate` with typed fail-closed recommendation and blockers. | Satisfied for implementation mechanism. | Tauri command, Rust report model, frontend wrapper, and mock IPC coverage are wired for `stage2-readiness-v1`. |
| Require known report build provenance. | Satisfied for mechanism; final gate runs must set `GITHUB_SHA` or `OPENLIFE_BUILD_COMMIT`. | Top-level `commit: "unknown"`, `commit: "none"`, or fake labels such as `commit: "mock-build"` now add `stage2_readiness_commit_missing` and cannot receive `ready_for_limited_internal_trial`. The live artifact was regenerated with `OPENLIFE_BUILD_COMMIT=092805564d98baec69375f9daffddfd17c01772b`. |
| Aggregate deterministic Stage 1/Beta readiness. | Satisfied for deterministic mechanism. | Stage 2 readiness includes Stage 1 dogfood, Beta v1, product maturity, command-surface, and final-acceptance evidence; existing gates are not lowered. |
| Load and validate manual dogfood evidence for S2-D01 through S2-D24. | Mechanism satisfied; real evidence missing. | Reviewer-facing report path, non-evidence reviewer worksheet, non-evidence JSON template, focused `validate_main_chat_agent_stage2_manual_dogfood_artifact` command, frontend wrapper/mock, and machine-readable artifact contract exist; validator reports `missingScenarioIds` for absent P0 attempts and gate rejects missing rows, optional P1 substitution, placeholder or fake identity labels, weak trace ids, weak reviewer ids, missing text, fake or bad commits, template rows, and failing P0 results. |
| Require at least two real reviewers on required P0 manual rows. | Mechanism satisfied; current credited reviewers are 0. | Gate reports `stage2_manual_reviewer_count_below_2` and `stage2_manual_p0_reviewer_count_below_2` until real artifact rows are present. |
| Run or wire L2-L01 through L2-L10 live-provider P0 scenarios. | Mechanism satisfied; current external run credited 10/10 with artifact commit. | Scenario plans, artifact schema, preflight blockers, harness adaptation, required-evidence manifest validation, model/Main Chat invocation checks, trace/preview checks, fake response-preview rejection, and local/mock/scripted/fixture/loopback/private-network rejection are wired. The latest explicit live run generated `frontend/test-results/main-chat-stage2-live-provider-report.json` with 10 credited scenarios, 10 Main Chat invocations, 10 model invocations, and artifact commit `092805564d98baec69375f9daffddfd17c01772b`. |
| Require scenario-scoped live policy setup where scenarios conflict. | Satisfied for mechanism. | Live runner paths preserve per-scenario setup/blockers instead of relaxing the gate when web-disabled and web-enabled scenarios need different runtime state. |
| Productize AgentControlPlane P0 task states with typed runtime evidence. | Satisfied for deterministic mechanism. | Ten P0 states are credited only through runtime payload evidence: direct answer, planning, executing, observed, blocked, waiting for permission, proposal pending, retry available, cancelled, completed. The task panel exposes and copies full task/run ids, task status, route, and blocker reason codes in a reviewer trace strip so manual reviewers can report trace-backed evidence without relying on assistant prose. |
| Close M2 memory proposal trial flow without silent durable truth writes. | Satisfied for deterministic mechanism. | M2-01 through M2-08 evidence covers candidate/source evidence, conflict, accept/reject/edit/defer, materialization provenance, rollback visibility, accepted context, and proposal-first knowledge edits. |
| Close R2 failure recovery coverage with user-facing next action or terminal explanation. | Satisfied for deterministic mechanism. | R2-01 through R2-10 evidence covers missing source, web policy, missing MCP target, disallowed tool, permission denial/acceptance, retry, cancel, stale resume, and plan-step failure; R2-04 specifically proves `model_selected_disallowed_tool`, no single-step fallback, no direct write, blocked state, and next-action/terminal explanation. |
| Keep final delivery honest for completed/proposed/blocked/skipped/unexecuted work. | Satisfied for deterministic mechanism. | Final-delivery summary credits 28 default task proofs and fails on overclaim markers. |
| Reject fake browser/live evidence and local/mock/scripted providers as external live credit. | Satisfied for mechanism. | Artifact refs use blocked status for blocker-bearing evidence; external-live credit rejects local, mock, scripted, fixture, loopback, private-network, and synthetic provider identities, plus fake/scripted/local-style response previews. |
| Return readiness only when all P0 automated, manual, live, safety, and final-delivery evidence is present. | Satisfied for mechanism; current recommendation is not ready. | Current report remains `not_ready_for_limited_internal_trial` because real manual dogfood evidence is still missing. A final gate run must also provide known top-level build provenance. |

## 4. Evidence Counts

| Area | Required | Current credited evidence |
| --- | ---: | ---: |
| Manual dogfood P0 scenarios | 24 | 0 real manual records in repository artifact |
| Manual reviewers on P0 rows | 2 | 0 real reviewers in repository artifact |
| Live provider P0 scenarios | 10 | 10 credited real external live scenarios; artifact commit is `092805564d98baec69375f9daffddfd17c01772b` |
| AgentControlPlane P0 states | 10 | 10 deterministic mechanism states covered |
| Memory proposal scenarios | 8 | 8 deterministic mechanism scenarios covered |
| Failure recovery scenarios | 10 | 10 deterministic mechanism scenarios covered |
| Final-delivery default task proofs | 28 | 28 deterministic mechanism proofs covered |

## 5. Verification Run

Latest Codex preflight on 2026-06-20 after fixing configured-provider router
initialization:

```text
cargo test -p openlife-core model_router_ -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_manual_dogfood -- --nocapture
cargo test -p openlife-tauri validate_stage2_manual_dogfood_artifact_command_returns_focused_summary -- --nocapture
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 OPENLIFE_BUILD_COMMIT=092805564d98baec69375f9daffddfd17c01772b cargo test -p openlife-tauri main_chat_stage2_live_provider_summary_invokes_external_provider_when_opted_in -- --ignored --nocapture
OPENLIFE_BUILD_COMMIT=092805564d98baec69375f9daffddfd17c01772b cargo test -p openlife-tauri run_stage2_readiness_gate_command_returns_auditable_report -- --nocapture
corepack pnpm --dir frontend test -- src/tauri.test.ts src/components/AgentControlPlane.test.tsx src/pages/ChatPage.test.tsx
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
cargo fmt --check
git diff --check
```

Observed Codex preflight results:

| Command | Result |
| --- | --- |
| `openlife-core model_router_` | 5 passed |
| `main_chat_agent_stage2_readiness` | 56 passed, 1 ignored external-live test |
| `main_chat_stage2_manual_dogfood` | 9 passed |
| `validate_stage2_manual_dogfood_artifact_command_returns_focused_summary` | 1 passed |
| `main_chat_stage2_live_provider_summary_invokes_external_provider_when_opted_in` | 1 passed with explicit external live-provider opt-in; refreshed 10/10 credited DeepSeek L2 scenario artifact |
| `run_stage2_readiness_gate_command_returns_auditable_report` | 1 passed with credited live artifact present and manual dogfood still fail-closed |
| Frontend focused bundle | 124 passed |
| Frontend typecheck | passed |
| Frontend format check | passed |
| Rust format check | passed |
| `git diff --check` | passed |

Latest local verification:

```text
git diff --check
cargo fmt --check
cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_manual_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_live_provider -- --nocapture
cargo test -p openlife-tauri main_chat_react_unit_tests -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_failure_recovery -- --nocapture
OPENLIFE_BUILD_COMMIT=092805564d98baec69375f9daffddfd17c01772b cargo test -p openlife-tauri run_stage2_readiness_gate_command_returns_auditable_report -- --nocapture
cargo test -p openlife-tauri validate_stage2_manual_dogfood_artifact_command_returns_focused_summary -- --nocapture
cargo test -p openlife-tauri main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in -- --ignored --nocapture
cargo test -p openlife-tauri main_chat_live_provider_eval_harness_invokes_external_react_web_and_mcp_when_opted_in -- --ignored --nocapture
OPENLIFE_BUILD_COMMIT=092805564d98baec69375f9daffddfd17c01772b cargo test -p openlife-tauri main_chat_stage2_live_provider_summary_invokes_external_provider_when_opted_in -- --ignored --nocapture
corepack pnpm --dir frontend test -- src/tauri.test.ts
corepack pnpm --dir frontend test -- src/components/AgentControlPlane.test.tsx
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
corepack pnpm --dir frontend test:e2e -- main-chat-stage1-dogfood.spec.ts --reporter=line
```

Observed results:

| Command | Result |
| --- | --- |
| `main_chat_agent_stage1_dogfood` | 22 passed |
| `main_chat_agent_beta_v1_readiness` | 3 passed |
| `main_chat_product_maturity_v2` | 9 passed |
| `main_chat_command_surface` | 24 passed |
| `main_chat_final_acceptance` | 86 passed, 1 ignored external-live test |
| `main_chat_agent_stage2_readiness` | 56 passed, 1 ignored external-live test |
| `main_chat_stage2_manual_dogfood` | 9 passed |
| `main_chat_stage2_live_provider` | 25 passed, 1 ignored external-live test |
| `main_chat_react_unit_tests` | 30 passed |
| `main_chat_stage2_failure_recovery` | 3 passed |
| `run_stage2_readiness_gate_command_returns_auditable_report` | 1 passed with credited live artifact present and manual dogfood still fail-closed |
| `validate_stage2_manual_dogfood_artifact_command_returns_focused_summary` | 1 passed |
| `main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in` | 1 passed with explicit external live-provider opt-in |
| `main_chat_live_provider_eval_harness_invokes_external_react_web_and_mcp_when_opted_in` | 1 passed with explicit external live-provider opt-in |
| `main_chat_stage2_live_provider_summary_invokes_external_provider_when_opted_in` | 1 passed with explicit external live-provider opt-in; generated 10/10 credited L2 scenario artifact at commit `092805564d98baec69375f9daffddfd17c01772b` |
| `frontend/src/tauri.test.ts` | 64 passed |
| `frontend/src/components/AgentControlPlane.test.tsx` | 11 passed |
| Frontend focused bundle | 124 passed |
| Stage 1 browser dogfood e2e | 1 passed |

Known non-failing warnings:

- React Router future-flag warnings in frontend tests.
- Prettier `jsxBracketSameLine` deprecation warning.
- Local Perl locale warning during explicit trailing-whitespace scan.
- Non-failing vector persistence timeout logs can appear during live-provider
  runs while the Main Chat response path still completes.

## 6. Changed Files

Stage 2 implementation and evidence files:

- `src-tauri/src/main_chat_agent_stage2_readiness.rs`
- `src-tauri/src/main_chat_agent_stage2_readiness_tests.rs`
- `plans/main_chat_stage2_manual_dogfood_report.md`
- `plans/main_chat_stage2_manual_dogfood_artifact_template.json`
- `plans/main_chat_stage2_manual_dogfood_reviewer_worksheet.md`
- `plans/main_chat_stage2_readiness_gate_contract.md`
- `plans/main_chat_stage2_implementation_report.md`

Command/runtime/frontend integration files:

- `openlife-core/src/agent/model_router.rs`
- `openlife-core/src/scheduler.rs`
- `src-tauri/src/commands/agent_runtime/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/main_chat_agent_productization_tests.rs`
- `src-tauri/src/main_chat_react_tool_selection.rs`
- `src-tauri/src/main_chat_react_unit_tests.rs`
- `src-tauri/src/main_chat_strategy.rs`
- `src-tauri/src/main_chat_task_control_tests.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- `frontend/src/tauri.test.ts`
- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/components/AgentControlPlane.test.tsx`

## 7. Unresolved Blockers

- `stage2_manual_dogfood_evidence_missing`
- `stage2_manual_reviewer_count_below_2`
- `stage2_manual_p0_reviewer_count_below_2`
- `stage2_readiness_commit_missing` when neither `GITHUB_SHA` nor
  `OPENLIFE_BUILD_COMMIT` is configured for the gate run

These blockers are expected in the current environment because no real manual
dogfood artifact has been provided. The external live-provider P0 scenarios now
have credited evidence with known artifact build provenance. The final readiness
gate still needs a known top-level build commit in the command environment.

## 8. Residual Risks

- Future provider/model changes may regress one or more L2 scenarios; keep
  failures trace-backed and regenerate the artifact after provider/API changes.
- Manual reviewers may find UI clarity or recovery issues not visible in the
  deterministic gates.
- Public beta polish, broad Skills Hub behavior, background autonomy, and
  unrestricted external writes remain out of scope.
- Stage 2 must continue rejecting local, mock, scripted, fixture, loopback, or
  synthetic provider evidence as external-live credit.

## 9. Evidence State

The remaining readiness evidence and current artifact state are:

| Evidence | Required condition |
| --- | --- |
| Final gate run provenance | `GITHUB_SHA` or `OPENLIFE_BUILD_COMMIT` must provide a known metadata-safe commit for the readiness gate run; `unknown`, `none`, local/mock/scripted/fixture/synthetic aliases, and private-network-looking labels are placeholders, not build provenance. |
| `frontend/test-results/main-chat-stage2-manual-dogfood-report.json` | Real S2-D01 through S2-D24 P0 rows from at least two known reviewers, matching build commit, valid task/run ids, reviewer-entered prompts/notes/problem fields, and no P0 failures. |
| `frontend/test-results/main-chat-stage2-live-provider-report.json` | Present in the current workspace with 10/10 credited `deepseek` / `deepseek-v4-flash` L2 rows, exact required-evidence manifests, ordinary Main Chat invocation proof, model invocation proof, trace ids, provider/model identity, response previews, no local/mock/scripted/fixture/loopback/private-network credit, and artifact commit `092805564d98baec69375f9daffddfd17c01772b`. Regenerate it after provider/model changes or after certifying a different build commit. |

## 10. Next Required Evidence

Before readiness can move to `ready_for_limited_internal_trial`:

1. Run real manual dogfood for S2-D01 through S2-D24 with at least two known
   reviewers and write
   `frontend/test-results/main-chat-stage2-manual-dogfood-report.json`.
2. Run the readiness gate with `GITHUB_SHA` or `OPENLIFE_BUILD_COMMIT` set to
   the known metadata-safe build commit being certified.
3. Re-run L2-L01 through L2-L10 only if certifying a different build commit,
   provider, model, or code path than the current generated artifact.
4. Re-run `run_main_chat_agent_stage2_readiness_gate` and confirm it has no P0
   blockers before declaring limited internal trial readiness.
