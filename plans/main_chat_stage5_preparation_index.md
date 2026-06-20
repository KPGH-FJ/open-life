# Main Chat Agent Stage 5 Preparation Index

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation draft

## 1. Direction

Stage 1 proved real end-to-end dogfood mechanics. Stage 2 built the internal
trial readiness gate but correctly stayed fail-closed without real manual and
external live evidence. Stage 3 made execution visible. Stage 4 made memory and
knowledge assets inspectable, governed, and reversible.

Stage 5 should now make OpenLife usable for serious internal testing:

- each Agent run must be diagnosable after the fact;
- each failure must map to a named failure class and recovery path;
- each internal tester must be able to export a metadata-safe issue bundle;
- provider, tool, context, memory, final delivery, build, and environment
  evidence must be correlated by stable ids;
- the release surface must help testers run real S2-D manual dogfood later
  without fabricating readiness evidence.

Stage 5 is a release/debug operations layer. It is not a new Agent runtime,
memory system, proposal format, or readiness gate replacement.

## 2. Preparation Documents

| Document | Purpose |
| --- | --- |
| `plans/main_chat_stage5_release_debug_best_practices.md` | Source-backed practices from OpenAI Agents tracing, Claude Code, LangSmith, Google ADK/OpenTelemetry, and OpenTelemetry GenAI conventions. |
| `plans/main_chat_stage5_objective_and_scope.md` | Defines Stage 5 target, boundaries, workstreams, non-goals, and exit criteria. |
| `plans/main_chat_stage5_current_gap_inventory.md` | Current OpenLife debug/release assets and gaps after Stage 4. |
| `plans/main_chat_stage5_release_debug_product_contract.md` | Product contract for preflight, debug bundles, issue reports, release evidence, and UI states. |
| `plans/main_chat_stage5_debug_privacy_redaction_contract.md` | Metadata-safe export and redaction rules for prompts, memory, provider data, files, and errors. |
| `plans/main_chat_stage5_internal_tester_workflow.md` | Workflow for internal testers to run tasks, collect evidence, export issues, and stop on invalid evidence. |
| `plans/main_chat_stage5_failure_taxonomy.md` | Failure classes, required evidence, UI copy, and recovery recommendations. |
| `plans/main_chat_stage5_release_debug_eval_matrix.md` | DBG5-01 through DBG5-24 scenarios and minimum test plan. |
| `plans/main_chat_agent_stage5_release_debug_goal_spec.md` | CLI goal-mode implementation entrypoint. |

## 3. Stage 5 Target

Stage 5 must produce an internal-trial release/debug layer where:

- the current build/commit/version is visible and exported;
- provider/network/key/scheduler/workspace/MCP/database preflight is visible;
- any Main Chat task can export a metadata-safe debug bundle;
- debug bundles and issue reports are app-data artifacts with schema version,
  digest, atomic write, list/get after refresh, and retention/delete behavior;
- the bundle links task session, run, transcript, actions, observations,
  blockers, proposals, memory/context inventory, final delivery, and UI state;
- failures are classified into stable categories with recovery guidance;
- issue reports can be attached to future S2-D manual dogfood rows;
- privacy redaction is enforced before export;
- Stage 1/2/3/4/final/live-provider gates remain fail-closed and cannot be
  bypassed by a Stage 5 report.

## 4. Recommended Development Order

1. Add Stage 5 report skeleton and DBG5-01 through DBG5-24 rows.
2. Add a build/environment preflight read model using existing config,
   diagnostics, provider preflight, workspace root, and MCP registry state.
3. Add metadata-safe Agent debug bundle assembly from existing task/run/action/
   transcript/proposal/memory/final delivery objects.
4. Add failure taxonomy mapping and recovery recommendations.
5. Add issue-report/export commands and UI surface in Main Chat / Review Center
   or a focused internal testing panel.
6. Add privacy/redaction tests before enabling export.
7. Add internal tester workflow support: scenario id, reviewer id, build commit,
   task session id, bundle id, pass/fail/blocker, notes, and attachment refs.
8. Add Stage 5 eval/gate and implementation report.
9. Keep Stage 2 readiness blocked unless real S2-D manual dogfood and current
   commit live-provider evidence exist.

Implementation should treat build provenance and UI evidence conservatively:
build metadata must come from deterministic build/app metadata or named
blockers, and UI evidence must be correlated to backend task/snapshot ids before
it can support a debug claim.

## 5. CLI Goal Prompt

Use this short prompt for CLI goal mode after review:

```text
Implement Main Chat Agent Stage 5 Internal Trial Release and Debug Operations.
Read plans/main_chat_agent_stage5_release_debug_goal_spec.md and the Stage 5
preparation docs it lists. Keep scope to release/debug operations. Reuse
existing AgentTaskSession, AgentRun, ExecutionTranscript, ActionQueue,
ProposalStore, MemoryLifecycleStore, Stage 3 execution UX, Stage 4
memory/knowledge inventory, Stage 2 readiness, final acceptance, live-provider
preflight, and frontend Main Chat/Review Center surfaces. Do not create a new
Agent runtime, memory system, proposal format, task control plane, telemetry
backend, or readiness gate. Build metadata-safe preflight, app-data artifact
storage, debug bundle, failure taxonomy, issue export, tester workflow, and
DBG5-01 through DBG5-24 coverage. Use deterministic build/app provenance or
named blockers, and correlate UI evidence with backend task/snapshot ids.
Preserve fail-closed semantics and never claim
ready_for_limited_internal_trial without real S2-D manual dogfood and current
commit external live-provider evidence.
```

## 6. Readiness To Start Stage 5 Development

Stage 5 development can start after:

- these preparation documents are reviewed;
- working tree is clean or intentionally staged;
- Stage 4 implementation commit is present;
- provider keys remain configured outside git;
- the implementer accepts that exported data must be metadata-safe by default;
- the implementer accepts that Stage 5 prepares manual dogfood but does not
  itself create real manual evidence.

## 7. Non-negotiable Invariants

- No silent durable LifeModel, memory, file, external, plugin, or provider
  writes.
- No hidden legacy fallback.
- No fake browser/manual/live-provider evidence.
- No local/mock/scripted provider credited as external live evidence.
- No raw API key, authorization header, full system prompt, raw private memory,
  or unrestricted transcript export.
- No knowledge file can override runtime privacy/tool/model policy.
- No Stage 5 report may replace Stage 2 readiness or final acceptance gates.
- No "debug passed" claim unless the bundle is trace-backed and redacted.
- No "ready_for_limited_internal_trial" claim without real S2-D manual dogfood
  and current-commit external live-provider evidence.
