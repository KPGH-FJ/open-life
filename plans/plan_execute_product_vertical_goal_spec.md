# Plan-Execute Product Vertical Goal Spec

> Last updated: 2026-06-03
> Status: completed CLI Goal-mode implementation spec / audit trail for W98-W105

This document is the CLI Goal-mode handoff and audit trail for the W98-W105
Plan-Execute Product Vertical: turning the existing PlanExecute runtime V1
slice into one narrow, user-visible product vertical.

The intended use is direct: start Codex CLI from the repository root, point it
to this file, and ask it to implement the full Goal. The Agent may complete the
whole block in one sustained run, but must internally keep the W98-W105 order,
prove each slice with tests, and stop only after final verification. The Agent
must not commit or push unless the user asks after review.

## 1. Current Baseline

The authoritative baseline is **W97 Legacy Direct-Write Convergence complete**.

The Agent must read these files before editing code:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/lifemodel_governed_runtime_progress.md`
4. `plans/openlife_lifemodel_governed_agent_runtime.md`
5. `plans/openlife_agent_framework_architecture.md`
6. `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
7. `openlife-core/src/agent/plan_execute.rs`
8. `openlife-core/src/agent/strategy.rs`
9. `openlife-core/src/agent/strategy_runtime.rs`
10. `openlife-core/src/agent/multi_strategy_runtime.rs`
11. `src-tauri/src/commands/agent_runtime/mod.rs`
12. `frontend/src/tauri.ts`

Current completed preparation:

- W15 completed the core PlanExecute governed runtime V1 slice.
- `PlanExecuteService` can draft and execute metadata-safe read-only/write-like
  steps.
- `StrategySelector` can select `RuntimeStrategyKind::PlanExecute` for planning
  or write-like intent when planning is allowed.
- `PlanExecuteRuntimeStrategy` is registered in `MultiStrategyRuntime`.
- The existing `run_multi_strategy_agent_preview` command can produce a
  metadata-safe multi-strategy preview AgentRun.
- W90-W97 converged legacy direct writes, so high-risk durable LifeModel-HS
  mutation must now go through proposal/governed operation boundaries.

Known limitations that remain real:

- PlanExecute is not productized for weekly planning.
- There is no dedicated Plan-Execute session/store/lifecycle.
- There is no review/edit/freeze step before execution.
- There is no user-facing product surface for a planned week or plan step list.
- Write-like plan steps are not bridged into concrete Review Center proposals
  for this product vertical.
- Existing MultiStrategy preview is an experimental/runtime surface, not the
  product workflow.
- Default Chat still uses `legacy_stream` and must stay unchanged.

## 2. Goal Objective

Build the first Plan-Execute product vertical around one narrow scenario:

```text
Use my LifeModel to plan this week.
```

The final W105 state must support this governed workflow:

```text
User starts a weekly planning request
  -> backend creates a Plan-Execute session and metadata-safe AgentRun trace
  -> PlanExecute drafts a reviewable plan using RuntimeHSPacket-compatible context
  -> user reviews, edits, and confirms/finalizes the plan
  -> user executes one step at a time
  -> read-only/internal steps produce metadata-safe observations
  -> write-like steps create Review Center proposals instead of direct writes
  -> session status, step status, proposal links, and trace metadata stay queryable
```

This Goal is successful only when the user can see and drive that vertical from
the frontend while the backend preserves Proposal-first, metadata safety, and
default Chat isolation.

## 3. Non-Negotiable Invariants

Do not change these invariants:

- Do not migrate default Chat.
- Do not replace ordinary `send_message` or `start_stream_message`.
- Do not route ordinary Chat through PlanExecute, MultiStrategy preview, W19-W60
  command surfaces, W65-W72 adapter proof stack, W73-W78 maturation helpers, or
  W79-W97 convergence helpers.
- Do not use Plan-Execute readiness, review, or execution as migration
  permission for default Chat.
- Do not directly write durable LifeModel-HS truth from Plan-Execute steps.
- Do not directly write memory, external provider state, calendar, email, file,
  or plugin state from this vertical.
- Do not execute real external tools unless an existing governed executor is
  already present and explicitly proposal-first. For W98-W105, prefer
  proposal-only.
- Do not store raw prompt, raw assistant output, raw LifeModel text, raw memory
  content, raw tool payload, raw weekly plan prose, or raw proposal payload in
  metadata reports, debug dumps, trace summaries, or audit summaries.
- Do not claim Plan-Execute is the default runtime strategy for Chat.
- Do not mark RuntimeStrategy / Multi-Strategy Runtime maturity complete. This
  Goal productizes one vertical only.
- Do not commit or push unless the user explicitly asks after review.

## 4. Product Scope

The product vertical is intentionally narrow.

### In Scope

- Weekly planning or low-energy weekly task planning.
- One Plan-Execute session lifecycle.
- Reviewable plan with bounded step count.
- User edit/finalize gate before execution.
- Step-by-step execution.
- Metadata-safe read-only/internal observations.
- Proposal creation for write-like steps.
- AgentRun trace and Review Center links.
- Frontend surface integrated into an existing product area.
- Tests proving default Chat isolation and proposal-first behavior.

### Out Of Scope

- Default Chat migration.
- Full planner/calendar/task manager.
- Full workflow engine.
- Plan-Execute as global runtime default.
- Multi-agent planning.
- External provider calendar/email writes.
- Automatic LifeModel maturation from plan completion.
- Unbounded generated plans.
- Background/proactive plan execution.
- Full PlanExecute AI planner quality improvements beyond the minimum needed
  to support the product contract.

## 5. Preferred UX Shape

Prefer adding a focused Plan-Execute panel to an existing product area instead
of creating a broad new isolated page. Acceptable placements:

- Dashboard / Workspace overview as a weekly planning panel.
- A small dedicated route only if the existing layout makes the workflow
  materially clearer.

The UI must support:

- start weekly plan
- display plan status
- display steps with status
- edit step title/intent before finalizing
- finalize plan
- execute next step or a selected step
- show proposal links for write-like steps
- show metadata-safe observations and warnings
- link to the AgentRun trace and Review Center

The UI must not describe internal architecture or migration mechanics to normal
users. Settings can keep technical preview controls, but the product vertical
should feel like a usable planning workflow, not a debug panel.

## 6. Data And Authority Model

Introduce the smallest durable model needed for the product vertical.

Recommended backend concepts:

- `PlanExecuteSession`
- `PlanExecuteSessionStatus`
- `PlanExecuteStepRecord`
- `PlanExecuteStepStatus`
- `PlanExecuteSessionStore`
- `CreatePlanExecuteSessionInput`
- `ReviewPlanExecuteSessionInput`
- `ExecutePlanExecuteStepInput`
- `PlanExecuteSessionOutput`

Required session metadata:

- `session_id`
- `source_agent_run_id`
- `source_chat_session_id` or product surface id
- `scenario` fixed to a typed value such as `weekly_planning`
- `status`
- `created_at`
- `updated_at`
- `finalized_at`
- `metadata_safe_objective`
- `step_count`
- `completed_step_count`
- `proposal_required_step_count`
- `linked_proposal_ids`
- `warnings`

Required step metadata:

- `step_id`
- `order`
- `title`
- `intent`
- `tool_name`
- `action_kind`
- `risk_level`
- `declared_write`
- `status`
- `linked_proposal_id`
- `observation_summary`
- `policy_reason_code`
- `metadata_safe_summary`

Raw user input and raw generated plan prose must not be duplicated into audit or
metadata reports. If product UX needs visible text such as step titles, store
only the bounded user-visible plan fields required for the session, and keep all
separate audit/debug/report fields metadata-safe.

## 7. Implementation Strategy

The Agent should complete W98-W105 in one Goal run, but implement in this exact
internal order:

1. W98 Plan-Execute product contract and scenario scope
2. W99 Plan-Execute session store and non-default command surface
3. W100 Review/edit/finalize lifecycle
4. W101 Step execution and proposal-first bridge
5. W102 AgentRun, trace, and Review Center linkage
6. W103 Frontend weekly planning surface
7. W104 Safety, isolation, metadata, and regression hardening
8. W105 Docs, progress index, and final verification sync

Run targeted tests after each major code area when practical. Run the full
verification matrix at the end.

If a slice is too large, keep the product vertical narrow instead of expanding
scope. A smaller correct weekly planning workflow is preferred over a broad
planner with weak governance.

## 8. W98 Spec: Product Contract And Scenario Scope

### Scope

Primary files:

- `openlife-core/src/agent/plan_execute.rs`
- `openlife-core/src/agent/tests/plan_execute.rs`
- possibly a new focused module under `openlife-core/src/agent/`

### Required Behavior

- Add a typed product scenario for weekly planning.
- Define a bounded product contract around PlanExecute output:
  - scenario id
  - max step count
  - allowed action kinds
  - allowed risk levels
  - proposal-first write boundary
  - metadata-safe summary
  - lifecycle expectations
- Ensure the scenario can be evaluated without model/tool/runtime side effects.
- Ensure broad `tools_prompt` content does not imply write permission or
  external side effects.
- Ensure LifeModel-HS influence is represented as metadata-safe guidance only:
  goal priority, energy/current state, planning intensity, privacy/model route,
  and proposal boundaries.

### Required Tests

- Weekly planning scenario contract is ready for a clean PlanExecute draft.
- Contract rejects unsupported scenario ids.
- Contract rejects excessive step count.
- Contract rejects high/critical risk direct write steps.
- Broad tools prompt does not grant write or external side-effect authority.
- Contract report/debug output excludes raw prompt, raw LifeModel text, raw
  memory, raw tool payload, raw assistant output.

### W98 Done Criteria

- Product contract exists and is testable in core.
- It does not create stores, commands, UI, or Chat route changes.

## 9. W99 Spec: Session Store And Non-Default Command Surface

### Scope

Primary files:

- `openlife-core/src/agent/plan_execute.rs`
- `openlife-core/src/agent/store.rs` or a new dedicated store module
- `src-tauri/src/commands/agent_runtime/` or a new command module if cleaner
- `src-tauri/src/lib.rs`
- `frontend/src/tauri.ts`

### Required Behavior

- Add a durable Plan-Execute session store.
- Add explicit non-default Tauri commands. Recommended command names:
  - `create_plan_execute_session`
  - `get_plan_execute_session`
  - `list_plan_execute_sessions`
- The create command must:
  - build or reuse RuntimeHSPacket-compatible context
  - draft the plan through PlanExecute service/contract
  - create an AgentRun or link to a source AgentRun
  - store a session in `draft` status
  - return a bounded product output
- The commands must not be called by ordinary `send_message` or
  `start_stream_message`.
- Command outputs must keep audit/report fields metadata-safe.

### Required Tests

- Creating a weekly planning session stores a draft session.
- Getting/listing sessions returns bounded metadata and visible product fields.
- Invalid scenario or oversized max steps fails closed.
- No proposal is created during draft creation.
- No durable LifeModel/Memory/External write occurs during draft creation.
- Ordinary Chat entrypoints do not call the new command/helpers.

### W99 Done Criteria

- A non-default product command surface exists.
- Sessions are durable and queryable.
- Default Chat remains isolated.

## 10. W100 Spec: Review, Edit, And Finalize Lifecycle

### Scope

Primary files:

- Plan-Execute session store/module
- Tauri command module
- `frontend/src/tauri.ts`

### Required Behavior

- Add explicit lifecycle commands. Recommended command names:
  - `update_plan_execute_session_draft`
  - `finalize_plan_execute_session`
  - optionally `cancel_plan_execute_session`
- Draft sessions may be edited before finalization.
- Finalized sessions become executable.
- Executing a non-finalized session must fail closed.
- Editing a finalized, completed, or cancelled session must fail closed.
- Step edits must remain bounded:
  - title length limit
  - intent/action kind allowlist
  - no high/critical direct write
  - no raw hidden payload fields
- Finalization must preserve metadata-safe lineage:
  - session id
  - source run id
  - step ids
  - step counts
  - risk summary

### Required Tests

- Draft session can be edited with valid bounded fields.
- Invalid step edit fails closed.
- Finalized session cannot be edited.
- Non-finalized session cannot execute.
- Finalization report is metadata-safe and raw-content-free.

### W100 Done Criteria

- User review/edit/finalize gate exists.
- Execution cannot bypass review.

## 11. W101 Spec: Step Execution And Proposal-First Bridge

### Scope

Primary files:

- Plan-Execute session store/module
- `openlife-core/src/agent/proposal_store.rs`
- `src-tauri/src/commands/proposal.rs` only if existing validation helpers need
  reuse
- Tauri command module

### Required Behavior

- Add explicit step execution command. Recommended command name:
  - `execute_plan_execute_step`
- Read-only/internal steps may execute as metadata-safe observations.
- Write-like steps must create Review Center proposals and must not execute.
- Proposal types should reuse existing proposal categories:
  - `ScheduledTask` for schedule/task/check-in style steps
  - `ExternalWriteAction` only as proposal-only, with payload minimization and
    size limits
  - `MemoryWrite` only if the user explicitly creates a memory proposal
  - `GoalUpdate` or `LifeModelUpdate` only for reviewable LifeModel changes
- Any `ExternalWriteAction` proposal must satisfy W97-level hard requirements:
  - payload minimization before store insertion
  - size limit before store insertion
  - no raw provider payload or raw body leakage in audit/report
  - no fallback external write execution
- Proposal source must be accurate, preferably a dedicated or already supported
  source such as `PlanningSession` if available. If a new `ProposalSource` is
  needed, update PatchSource/readiness mappings and tests.
- Link generated proposal ids to the Plan-Execute session and AgentRun.
- Re-executing an already completed/proposal-created step must be idempotent or
  fail closed without duplicate proposal creation.

### Required Tests

- Read-only step execution records a metadata-safe observation and no proposal.
- Write-like step creates a proposal and does not execute an external write.
- Duplicate execution does not create duplicate proposals.
- Proposal payload is minimized and size-limited before insertion.
- Generated proposal source/mapping is source-specific and not mislabeled as
  BuilderReview or misleading Manual.
- Step execution does not write durable LifeModel-HS truth directly.
- Step execution does not write memory directly unless the output is only a
  reviewable proposal.

### W101 Done Criteria

- Step execution is useful but proposal-first.
- Review Center becomes the authority for write-like steps.

## 12. W102 Spec: AgentRun, Trace, And Review Center Linkage

### Scope

Primary files:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/store.rs`
- Tauri command module
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/utils/previewAudit.ts` if applicable

### Required Behavior

- Persist a metadata-safe AgentRun trace for Plan-Execute product sessions.
- Trace should include:
  - `planExecuteProductVertical=true`
  - scenario id
  - plan session id
  - strategy kind `plan_execute`
  - step count
  - step status counts
  - generated proposal ids
  - governance decision counts
  - source run/session lineage
  - warning count
- Trace must not include raw prompt, raw weekly plan prose, raw LifeModel text,
  raw memory content, raw tool payload, or raw proposal payload.
- Runs UI should identify Plan-Execute product traces and link back to the plan
  session or show session metadata if no route exists.
- Review Center proposal cards should still work through existing proposal
  surfaces. Add only minimal linking metadata if needed.

### Required Tests

- Plan-Execute product AgentRun is created and updated through lifecycle.
- Trace metadata includes plan id/session id/proposal ids/counts.
- Trace metadata excludes raw content.
- Runs/trace frontend renders the Plan-Execute product metadata.
- Proposal ids generated by steps are visible from session output and AgentRun.

### W102 Done Criteria

- The workflow is auditable and traceable.
- Trace remains metadata-safe.

## 13. W103 Spec: Frontend Weekly Planning Surface

### Scope

Primary files:

- `frontend/src/tauri.ts`
- likely `frontend/src/components/WorkspaceOverview.tsx` or a new focused
  component under `frontend/src/pages/` / `frontend/src/components/`
- related frontend tests and mocks

### Required Behavior

- Add a user-facing weekly planning surface.
- The surface must call explicit Plan-Execute commands only.
- The surface must support:
  - create session
  - load current/recent sessions
  - edit draft steps
  - finalize plan
  - execute a selected/next step
  - show statuses, observations, warnings, proposal links, and run link
- Keep the UI operational, not architecture-explanatory.
- Use existing design conventions and lucide icons where useful.
- Avoid a large new isolated page unless it is clearly cleaner than integrating
  into Dashboard/Workspace.
- Ensure loading, error, empty, draft, finalized, executing, proposal-required,
  completed, and cancelled states are represented.

### Required Tests

- Frontend can create a weekly planning session.
- Frontend can edit and finalize a draft.
- Frontend cannot execute before finalization.
- Frontend can execute a read-only step and display observation.
- Frontend can execute a write-like step and show proposal link.
- Frontend mocks include all new commands.
- Text does not claim default Chat migration or direct external execution.

### W103 Done Criteria

- A user can drive the weekly planning vertical from the app.
- The surface uses the governed command path.

## 14. W104 Spec: Safety, Isolation, And Regression Hardening

### Scope

Primary files:

- all Plan-Execute product modules/tests
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/life_model.rs` default Chat isolation tests if
  currently hosted there
- frontend tests/mocks

### Required Behavior

- Add regression tests for default Chat isolation.
- Add raw-content leak tests for every report/debug/trace surface introduced.
- Add idempotency tests for step execution.
- Add failure tests for missing store, missing session, bad status transition,
  unsupported scenario, unsafe step, oversized payload, and proposal store
  errors.
- Add tests that prove no direct LifeModel/Memory/External writes occur.
- Preserve existing MultiStrategy preview behavior.

### Required Tests

Minimum targeted commands:

```bash
cargo test -p openlife-core plan_execute -- --nocapture
cargo test -p openlife-core strategy -- --nocapture
cargo test -p openlife-core multi_strategy_runtime -- --nocapture
cargo test -p openlife-tauri plan_execute -- --nocapture
cargo test -p openlife-tauri agent_runtime -- --nocapture
cargo test -p openlife-tauri proposal -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
```

Frontend targeted tests should include any new or changed files, for example:

```bash
cd frontend && pnpm test -- --run
```

Use the exact frontend test command already standard in the repo if different.

### W104 Done Criteria

- New workflow has meaningful backend and frontend regression coverage.
- Existing runtime preview and default Chat isolation still pass.

## 15. W105 Spec: Docs, Progress Index, And Final Verification Sync

### Scope

Primary files:

- `AGENTS.md`
- `plans/README.md`
- `plans/lifemodel_governed_runtime_progress.md`
- `plans/openlife_lifemodel_governed_agent_runtime.md` if status references
  need update
- this file

### Required Behavior

- Update docs from W97 baseline to W105 Plan-Execute Product Vertical complete.
- Keep `plans/README.md` as the authority map.
- Keep progress index compact and machine-readable enough for future Agents.
- State explicitly:
  - default Chat remains `legacy_stream`
  - Plan-Execute product vertical is non-default
  - write-like steps create proposals
  - no external writes execute directly
  - no direct LifeModel-HS truth writes occur
  - RuntimeStrategy / Multi-Strategy Runtime maturity remains future work
- Do not let stale text imply W105 completed default Chat migration or full
  runtime strategy maturity.

### Required Tests

- `rg` checks should confirm new docs mention W105 status consistently.
- `rg` checks should confirm ordinary Chat entrypoints do not call new
  Plan-Execute product commands/helpers.
- `git diff --check` must pass.
- Full CI must pass.

### W105 Done Criteria

- The Plan-Execute Product Vertical block is complete.
- The next block can move to RuntimeStrategy / Multi-Strategy Runtime maturity
  or ReAct Beta execution hardening with a real Plan-Execute product vertical
  already in place.

## 16. Final Verification Matrix

Run all applicable targeted tests, then full CI.

Minimum required commands:

```bash
cargo test -p openlife-core plan_execute -- --nocapture
cargo test -p openlife-core strategy -- --nocapture
cargo test -p openlife-core multi_strategy_runtime -- --nocapture
cargo test -p openlife-tauri plan_execute -- --nocapture
cargo test -p openlife-tauri agent_runtime -- --nocapture
cargo test -p openlife-tauri proposal -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
cd frontend && pnpm test -- --run
git diff --check
make ci
```

If the repo has no `openlife-tauri plan_execute` test target at the start of
this Goal, create focused tests that can be selected by `plan_execute`.

Before final handoff, also run focused search checks:

```bash
rg -n "create_plan_execute_session|execute_plan_execute_step|PlanExecuteSession" src-tauri/src/lib.rs src-tauri/src/commands frontend/src
rg -n "send_message|start_stream_message" src-tauri/src/lib.rs
rg -n "plan_execute_product|planExecuteProduct|Plan-Execute Product" AGENTS.md plans/README.md plans/lifemodel_governed_runtime_progress.md
```

Use the search results to prove command registration exists, frontend wrappers
exist, and ordinary Chat entrypoint bodies do not invoke the new product
vertical.

## 17. Handoff Output Requirements

When the Agent finishes, it must output:

- change summary by W-slice
- new backend interfaces and commands
- new frontend surfaces
- tests run and results
- any skipped tests with reason
- risk notes
- whether W105 is complete
- whether the next big block can start

The Agent must not commit or push.

## 18. CLI Goal Prompt

Use this prompt in Codex CLI:

```text
You are implementing the next OpenLife big development block:
Plan-Execute Product Vertical W98-W105.

Read and follow:
- AGENTS.md
- plans/README.md
- plans/plan_execute_product_vertical_goal_spec.md
- plans/lifemodel_governed_runtime_progress.md
- plans/openlife_lifemodel_governed_agent_runtime.md
- plans/adr/0013-lifemodel-hs-source-of-truth-governance.md

Current baseline:
- W97 Legacy Direct-Write Convergence is complete.
- Existing PlanExecute core/runtime V1 exists, but product weekly planning is still future work.
- Default Chat remains legacy_stream.

Goal:
Implement W98-W105 in one sustained Goal run, keeping the internal order:
1. W98 Plan-Execute product contract and weekly planning scenario scope.
2. W99 durable Plan-Execute session store and explicit non-default commands.
3. W100 review/edit/finalize lifecycle.
4. W101 step execution with proposal-first bridge.
5. W102 AgentRun, trace, and Review Center linkage.
6. W103 frontend weekly planning surface.
7. W104 safety, isolation, metadata, and regression hardening.
8. W105 docs/progress/authority sync.

Hard constraints:
- Do not migrate default Chat.
- Do not replace send_message or start_stream_message.
- Do not call the new Plan-Execute product commands/helpers from ordinary Chat entrypoints.
- Do not directly write durable LifeModel-HS truth from Plan-Execute.
- Do not directly write Memory, external provider state, calendar, email, file, or plugin state.
- Write-like steps must create Review Center proposals and must not execute.
- ExternalWriteAction proposals require payload minimization and size limit before store insertion.
- Reports, traces, debug dumps, and audits must be metadata-safe and raw-content-free.
- Do not claim RuntimeStrategy / Multi-Strategy Runtime maturity is complete.
- Do not commit or push.

Expected product:
- A user-visible weekly planning workflow.
- User can create a plan, review/edit/finalize it, execute one step at a time, see observations/proposal links, and inspect run trace.
- Backend session state is durable and queryable.
- AgentRun trace and proposal linkage are metadata-safe.

Required verification:
- cargo test -p openlife-core plan_execute -- --nocapture
- cargo test -p openlife-core strategy -- --nocapture
- cargo test -p openlife-core multi_strategy_runtime -- --nocapture
- cargo test -p openlife-tauri plan_execute -- --nocapture
- cargo test -p openlife-tauri agent_runtime -- --nocapture
- cargo test -p openlife-tauri proposal -- --nocapture
- cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
- cd frontend && pnpm test -- --run
- git diff --check
- make ci

Final output only:
- W98-W105 change summary
- new interfaces/commands/surfaces
- tests run and results
- residual risks
- whether W105 is complete
- whether the next big block can start
```
