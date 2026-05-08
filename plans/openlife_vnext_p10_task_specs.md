# OpenLife vNext P10 Task Specifications

Date: 2026-05-08

Status: accepted

Acceptance:

- P10 Frontend Agent Workspace passed recommended verification and full
  `make ci` on 2026-05-08.
- Current follow-up phase: `plans/openlife_vnext_p11_task_specs.md`.

Package:

```text
Frontend Agent Workspace
```

P10 turns the vNext runtime spine into a usable workspace surface. P0-P9 made
agent behavior traceable, prompt-governed, proposal-first, plan-aware,
compaction-safe, AgentSpec-selected, and shell/sandbox-governed. P10 must not
reopen runtime architecture questions. Its job is to make the existing runtime
legible and operable from the frontend without destabilizing streaming chat.

The product goal is simple: users should be able to see what the agent
understood, which context it used, what it planned, what tools it called, what
observations came back, what proposals need review, and what can be done next.

## Baseline Review

Before P10:

- Chat, streaming, fallback, scheduled/proactive, plan execution, proposal,
  compaction, AgentSpec, and shell/sandbox governance all have backend trace
  primitives.
- `RunTracePanel` exists and can render basic `AgentRunEvent` rows.
- `RunsPage` exists as a run list / run inspection surface.
- Proposal review exists, but evidence, tool observations, and plan operations
  are not yet presented as a single agent workspace.
- P9 shell remains default-off and excluded from generic model-callable prompts.
- P10 must preserve P9 guarantees. Do not add a shell command box, terminal UI,
  or broad shell-enabled chat mode.

## Global Rules

- Execute exactly one P10 task spec at a time.
- Do not rewrite `ChatPage` wholesale.
- Do not change backend runtime semantics unless the task explicitly requires a
  small read-only query command or DTO normalization.
- Preserve streaming stability; UI panels must not block or reset active
  streams.
- Do not expose `shell.run` in normal chat UI.
- Do not add direct LifeModel or file writes from frontend UI. All mutations stay
  proposal-first or use existing governed plan operations.
- Reuse existing Tauri commands and DTOs where possible.
- Add frontend tests for every UI state added.
- Keep operational UI dense and work-focused. This is a user workspace, not a
  marketing page.

## P10-0: Documentation And Entry Sync

Goal:

Make P10 discoverable and AI-coding-ready.

Expected behavior:

- `AGENTS.md` and `README.md` state that P9 Shell/Sandbox core is closed and
  P10 Frontend Agent Workspace is the current phase.
- P10 task specs, prompts, migration plan, and test matrix are linked from the
  standard vNext entry points.
- P10 non-goals are explicit: no ChatPage rewrite, no terminal UI, no shell
  enablement UX, no backend runtime migration.

Allowed edit areas:

- `AGENTS.md`
- `README.md`
- `plans/openlife_vnext_p10_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`

Constraints:

- Documentation only.
- Do not change Rust or TypeScript code.

Verification:

- `rg -n "openlife_vnext_p10_task_specs|P10-0|P10-1|P10-2|P10-3|P10-4|P10-5|Frontend Agent Workspace" AGENTS.md README.md plans`
- `git diff --name-only` contains documentation files only for this task.

## P10-1: Agent Workspace Information Architecture

Goal:

Define the frontend workspace shell that brings runs, plans, tools, proposals,
and memory evidence into one coherent operational surface.

Expected behavior:

- Add or refine a route/surface for Agent Workspace without disrupting existing
  Chat, Review, Runs, or Settings routes.
- The first viewport is an operational workspace, not a landing page.
- Workspace sections are unframed layouts or compact panels; no nested cards.
- The surface can show:
  - active / recent run summary
  - pending proposal count
  - recent plan status
  - recent tool / observation status
  - next actions
- Empty, loading, error, and stale-data states are explicit.

Allowed edit areas:

- `frontend/src/App.tsx`
- `frontend/src/pages/DashboardPage.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/ChatPage.tsx` only for links/embedding points
- shared frontend components and tests
- `frontend/src/tauri.ts` / mocks only if existing commands need typed wrappers

Constraints:

- No broad ChatPage rewrite.
- No terminal UI or shell command input.
- No backend mutation behavior.

Verification:

- `pnpm --dir frontend test -- --run App Dashboard Runs tauri`
- `pnpm --dir frontend typecheck`

Required tests:

- workspace route/surface renders with mock data.
- loading/empty/error states render.
- navigation to existing Chat/Review/Runs surfaces still works.
- active stream UI is not reset by opening workspace panels.

## P10-2: Run Timeline And Event Detail Surface

Goal:

Upgrade runtime trace from a compact row list into a useful run inspection
experience.

Expected behavior:

- Run timeline groups or visually distinguishes:
  - run lifecycle
  - AgentSpec / PromptStack / context governance
  - model route and model calls
  - tool started / blocked / completed / failed
  - observations
  - proposals
  - compaction
  - plan events
- Event details are inspectable without exposing raw sensitive prompt content.
- Redaction metadata is visible when present.
- Shell events render as ordinary governed tool events; no shell-specific
  command entry surface.

Allowed edit areas:

- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/RunTracePanel.test.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/types.ts`
- `frontend/src/test/mocks/tauri.ts`

Constraints:

- Do not add new backend event types unless a missing type is truly required.
- Do not display unbounded stdout/stderr; use existing truncated metadata.

Verification:

- `pnpm --dir frontend test -- --run RunTracePanel Runs tauri`
- `pnpm --dir frontend typecheck`

Required tests:

- timeline renders model, tool, proposal, plan, compaction, and shell events.
- detail drawer/panel shows payload metadata.
- redaction metadata renders.
- truncated output marker renders.
- unknown event types render without crashing.

## P10-3: Tool Observation Panel

Goal:

Make tool calls and observations explainable to users.

Expected behavior:

- Add a panel or section that lists tool actions and observations for a run.
- Each row shows tool name, source, status, risk/tool scope, timestamp, and
  observation summary.
- Blocked tools clearly show block reason and whether user confirmation is
  available through existing Proposal/Permission flows.
- Large outputs are collapsed and truncated markers are visible.
- Declarative-only tools are labeled as unavailable/stub when relevant.

Allowed edit areas:

- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/components/ToolCallCard.tsx`
- new small frontend components
- `frontend/src/types.ts`
- `frontend/src/test/mocks/tauri.ts`

Constraints:

- No new execution controls.
- No direct retry/replay button unless it calls an existing governed replay
  command and is separately covered by tests.

Verification:

- `pnpm --dir frontend test -- --run Runs ToolCallCard tauri`
- `pnpm --dir frontend typecheck`

Required tests:

- successful tool observation renders.
- blocked tool observation renders reason.
- high-risk tool scope renders.
- truncated output stays collapsed by default.
- declarative-only/unavailable tool state renders.

## P10-4: Proposal Evidence And Review Context

Goal:

Help users review proposals with enough context to accept, reject, edit, or
postpone safely.

Expected behavior:

- Proposal review surfaces show linked run id, source, risk level, affected
  path, before/after summary, and evidence when available.
- MemoryEvidence / proposal evidence links render as summaries, not raw hidden
  transcripts.
- Chat-generated proposals and plan-generated proposals are distinguishable.
- The pending proposal banner can navigate to the relevant review context.

Allowed edit areas:

- Review/Proposal frontend components
- `frontend/src/pages/ChatPage.tsx` for banner link/context only
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/types.ts`
- `frontend/src/tauri.ts` / mocks if existing proposal DTO wrappers need fields

Constraints:

- Do not add direct apply bypasses.
- Do not expose raw sensitive memory evidence.
- Backend changes should be read-only DTO additions only if existing data is
  unavailable.

Verification:

- `pnpm --dir frontend test -- --run Proposal Chat Runs tauri`
- `pnpm --dir frontend typecheck`
- Backend tests only if DTO commands change.

Required tests:

- proposal evidence summary renders.
- missing evidence renders graceful empty state.
- pending proposal banner links to review context.
- accept/reject/edit/postpone existing flows still work.
- high-risk proposals show risk context.

## P10-5: Plan Confirmation And Operations Surface

Goal:

Make confirmed plan execution usable from the frontend without changing plan
runtime semantics.

Expected behavior:

- Plan UI supports viewing a generated plan, confirming, rejecting, and editing
  before execution where existing backend commands allow it.
- Executed plans show step status, deviations, review result, and retry/cancel
  affordances only when legal.
- Blocked action continuation links to existing permission/proposal review
  surfaces.
- Plan events appear in run timeline.

Allowed edit areas:

- plan-related frontend components/pages
- `frontend/src/tauri.ts`
- `frontend/src/types.ts`
- `frontend/src/test/mocks/tauri.ts`
- minimal read-only backend DTO normalization only if required

Constraints:

- Do not implement rollback unless ADR 0011 is accepted and a separate task is
  created.
- Do not add shell plan execution UI.
- Do not make illegal terminal-state operations available.

Verification:

- `pnpm --dir frontend test -- --run Plan Runs Chat tauri`
- `pnpm --dir frontend typecheck`
- `cargo test -p openlife-tauri commands::plan --lib` if command DTOs change.

Required tests:

- plan confirmation interaction renders.
- confirm/reject/edit calls correct Tauri wrappers.
- cancel/retry buttons obey legal state rules.
- blocked action continuation shows existing permission/proposal path.
- plan events render in timeline.

## P10 Exit Criteria

P10 is complete when:

- The current phase and task specs are discoverable from README/AGENTS.
- Agent Workspace gives users one operational surface for recent runs, plans,
  tools, proposals, and next actions.
- Run timeline can explain core vNext events without exposing sensitive payloads.
- Tool observations are visible, bounded, and risk-labeled.
- Proposal review shows source/evidence context.
- Plan confirmation and legal operations are usable from the frontend.
- Chat streaming remains stable.
- P9 shell guarantees remain unchanged: no normal prompt exposure, no terminal UI,
  no scheduled/proactive/sub-agent shell enablement.

Recommended final verification:

- `pnpm --dir frontend test -- --run App Chat Dashboard Runs RunTracePanel ToolCallCard tauri`
- `pnpm --dir frontend typecheck`
- `cargo test -p openlife-core agent::event_store --lib`
- `cargo test -p openlife-core agent::plan_executor --lib`
- `cargo test -p openlife-tauri commands::plan --lib`
- `cargo check -q`
- `make ci`
