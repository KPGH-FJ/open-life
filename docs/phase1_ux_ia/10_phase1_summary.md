# Phase 1 UX / IA Summary

Status: Phase 1 documentation summary.
Scope: Summarizes decisions, constraints, open questions, and Phase 2 entry checklist. No Frontend V2 implementation started.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Input Completeness

`VERIFIED_FACT`: Required bootstrap files were present after handoff extraction:

- `OpenLife_Phase1_UX_IA_Codex_Goal_v1.2.md`
- `docs/_templates/phase1_ux_ia/01_v2_decision_record.md`
- `docs/openlife-phase0-audit/13_audit_summary.md`
- `docs/phase0_5/07_phase0_5_summary.md`

`VERIFIED_FACT`: Phase 0 audit inputs and Phase 0.5 inputs were available under `docs/openlife-phase0-audit/` and `docs/phase0_5/`.

## Decisions Made

1. `DESIGN_DECISION`: V2 is a bounded product-experience and state-contract rewrite, not a blank rebuild.
2. `DESIGN_DECISION`: Companion + Chat merge into `工作区`.
3. `DESIGN_DECISION`: Mailbox becomes `审核中心`.
4. `DESIGN_DECISION`: Runs becomes `任务`.
5. `DESIGN_DECISION`: Diagnostics are hidden by default but preserved through expandable, advanced, and developer layers.
6. `DESIGN_DECISION`: Backend-owned ViewModels / ReadModels must be defined before UI implementation.
7. `DESIGN_DECISION`: Normal product language should be Chinese-first, with implementation terms hidden by default.
8. `DESIGN_DECISION`: Review actions, product actions, and debug actions must remain separate.

## Accepted With Constraints

1. `CANDIDATE`: Memory becomes top-level `记忆` only if Phase 2 validates clear boundaries from LifeModel, Review Center, Workspace evidence, and Settings/Data Management.
2. `DESIGN_DECISION`: `LifeModel` remains English-branded only with clear Chinese explanatory copy such as `OpenLife 对你的长期理解`.
3. `CANDIDATE`: Advanced evidence inspection is preserved, but raw traces, kernel events, provider/router internals, and dev/test surfaces are hidden by default.
4. `CANDIDATE`: Planning, memory, tool, Web/MCP, provider, and external-write capabilities are preserved as product concepts where appropriate, but unverified capabilities remain `PHASE_2_REQUIRED` or `UNKNOWN`.

## Open Questions

1. `UNKNOWN`: Should Memory remain top-level after backend lane/status/provenance read-model validation?
2. `UNKNOWN`: Should `任务` remain top-level or become part of `工作区` history/detail?
3. `UNKNOWN`: Which advanced routes, especially MCP/A2A, calibration, metrics, and versions, are product trust surfaces versus developer-only tools?
4. `UNKNOWN`: Which memory lanes may materialize locally without Review Center approval?
5. `UNKNOWN`: How should manual LifeModel editing be exposed, if retained?
6. `UNKNOWN`: Which provider/privacy summary should be default-visible?
7. `UNKNOWN`: What support/debug mode exposes PolicyRouter/ModelRouter internals?

## Implementation Blockers

1. `PHASE_2_REQUIRED`: Define backend-owned ViewModel scope before Frontend V2 implementation.
2. `PHASE_2_REQUIRED`: Validate or implement `WorkspaceViewModel`, `TasksViewModel`, `ReviewCenterViewModel`, `LifeModelViewModel`, `MemoryViewModel`, and `SettingsViewModel`.
3. `PHASE_2_REQUIRED`: Decide whether to expand `LifeStateProjection` or add adjacent read models.
4. `PHASE_2_REQUIRED`: Define unified ReviewItem item types, statuses, allowed actions, expiration behavior, and durable materialization state.
5. `PHASE_2_REQUIRED`: Define memory lane/lifecycle/provenance read model.
6. `PHASE_2_REQUIRED`: Define default/advanced/developer diagnostics visibility gates.
7. `UNKNOWN`: Desktop/Tauri product trial remains unverified / Phase7 `red-until-trial-green`.
8. `UNKNOWN`: External live-provider generation, Web AgentLoop, and MCP AgentLoop readiness remain unverified.
9. `UNKNOWN`: Browser smoke E2E remains blocked/partial by Phase 0.5 evidence.
10. `PHASE_2_REQUIRED`: Resolve whether MCP/A2A, calibration, versions, and metrics are product, advanced, or developer-only before route or navigation work.
11. `PHASE_2_REQUIRED`: Resolve final Chinese labels for review actions and Safe Mode before shipping V2 copy.

## Required Human Approvals

1. Approve V2 IA and final route names.
2. Approve whether Memory is top-level or reduced-risk sub-surface.
3. Approve Review Center scope beyond proposals.
4. Approve LifeModel English brand plus Chinese explanatory copy.
5. Approve canonical Chinese status and action vocabulary.
6. Approve diagnostics visibility policy.
7. Approve ViewModel/read-model ownership before implementation.
8. Approve whether calibration, versions, MCP/A2A, and metrics are product, advanced, or developer-only.

## Capability Preservation Notes

1. `DESIGN_DECISION`: Do not delete Memory as a product concept because the read model is incomplete; mark missing support `PHASE_2_REQUIRED`.
2. `DESIGN_DECISION`: Do not delete LifeModel provenance/change concepts; make them understandable and mark unverified projection fields `PHASE_2_REQUIRED`.
3. `DESIGN_DECISION`: Do not hide evidence permanently; hide raw diagnostics by default while preserving advanced access.
4. `DESIGN_DECISION`: Do not collapse proposals, blockers, waiting permission, and failures into completed states.
5. `DESIGN_DECISION`: Do not restore old Phase7 Stage/Beta/migration/cutover routes or command surfaces.

## Core User Scenarios

Scenario validation status: `DESIGN_DECISION` / `CANDIDATE` only. These scenarios are design validation inputs, not browser E2E, desktop/Tauri trial, live-provider, Web AgentLoop, MCP AgentLoop, or execution-readiness evidence.

## Scenario S1: User asks OpenLife to plan today's priorities

User goal:
Help me decide what to focus on today using my current context.

Entry surface:
`今日` or `工作区`.

Surfaces involved:
`今日`, `工作区`, `任务`, optionally `审核中心`.

Default UI:
Show a concise goal/understanding summary, staged planning timeline, blockers, and final priority recommendation.

System understanding:
OpenLife identifies this as a planning task, states what context it used, and flags missing context or uncertainty.

Execution timeline:
Understand request, gather local context, draft priorities, identify blockers, return plan and next action.

Review Center trigger:
Only if the plan proposes durable Memory/LifeModel changes, permissions, or external writes.

Task state:
running, blocked, failed, completed, or completed_with_pending_items.

LifeModel / Memory impact:
May use existing context; new durable memory or LifeModel update requires lane policy and review threshold.

Diagnostics visibility:
Default timeline and evidence links; raw trace/provider/router details in advanced inspector.

Required ViewModel fields:
user goal, agent understanding, lifecycle status, timeline stages, blockers, result, evidence refs, related review refs.

Failure / empty state:
If context is unavailable, show empty/error/stale state and ask whether to plan without it.

Success criteria:
User can see the plan, understand assumptions, and know whether anything changed or awaits review.

Evidence classification:
`CANDIDATE`: backend planning/task primitives exist, but full desktop planning journey is not verified.

Open questions:
Which daily signals belong in the default planning context?

## Scenario S2: User asks OpenLife to execute a task requiring external write

User goal:
Ask OpenLife to perform work that would write to a file, external system, email, calendar, provider, plugin, or other sensitive target.

Entry surface:
`工作区`.

Surfaces involved:
`工作区`, `审核中心`, `任务`, `设置`.

Default UI:
Workspace shows action intent, target, risk summary, and waiting-for-confirmation or blocked state.

System understanding:
OpenLife identifies the action, target, external/sensitive boundary, permission need, and what will change.

Execution timeline:
Understand request, prepare action, stop before write, create ReviewItem or preflight confirmation, resume only after approval.

Review Center trigger:
external_write, permission_request, dangerous_action, or policy_change.

Task state:
waiting_permission or blocked until approved; completed only after action actually succeeds.

LifeModel / Memory impact:
None unless separately proposed and approved.

Diagnostics visibility:
Default shows target/risk/reason; raw payloads, trace, and manifests are advanced/debug-only.

Required ViewModel fields:
risk level, impact scope, target, review item ref, allowed review actions, resume relation, evidence refs, safe-mode state.

Failure / empty state:
If validation fails, show blocker/failure and do not imply write completion.

Success criteria:
No external or sensitive write happens without explicit review/confirmation.

Evidence classification:
`VERIFIED_FACT` for safe-path/danger/tool governance primitives; unified ReviewItem is `PHASE_2_REQUIRED`.

Open questions:
Which external write categories are in first V2 scope?

## Scenario S3: OpenLife detects a candidate memory requiring confirmation

User goal:
Let OpenLife remember useful information while keeping control over durable memory.

Entry surface:
`工作区`, `记忆`, or `审核中心`.

Surfaces involved:
`工作区`, `审核中心`, `记忆`, `LifeModel`.

Default UI:
Show a candidate memory preview, why it matters, source, lane, risk, and review action.

System understanding:
OpenLife distinguishes context-only information from candidate memory and from canonical LifeModel truth.

Execution timeline:
Detect candidate, classify lane, create or link ReviewItem, wait for decision, update memory state only after approval or documented low-risk lane.

Review Center trigger:
memory_update.

Task state:
completed_with_pending_items or waiting_permission until reviewed.

LifeModel / Memory impact:
Candidate memory stays pending; confirmed memory may later affect LifeModel only through defined governance.

Diagnostics visibility:
Default shows source/lane/status; raw memory row, vector/index details, and lifecycle internals are advanced/debug-only.

Required ViewModel fields:
candidate text, lane, source refs, confidence, risk, review status, resulting memory lifecycle state.

Failure / empty state:
If lane or source is unknown, mark unknown and ask for confirmation instead of silently remembering.

Success criteria:
User controls whether the memory is confirmed, edited, postponed, or rejected.

Evidence classification:
`VERIFIED_FACT` for MemoryGateway/lifecycle primitives; memory lane UI read model is `PHASE_2_REQUIRED`.

Open questions:
Which memory lanes may bypass review?

## Scenario S4: OpenLife proposes updating a long-term LifeModel preference

User goal:
Update OpenLife's long-term understanding of a preference, rule, goal, or relationship.

Entry surface:
`工作区`, `LifeModel`, or `审核中心`.

Surfaces involved:
`工作区`, `审核中心`, `LifeModel`, `记忆`.

Default UI:
Show proposed LifeModel change, before/after summary, evidence, risk, and review actions.

System understanding:
OpenLife identifies that this change affects long-term structured understanding, not just a chat answer.

Execution timeline:
Capture source, draft proposed change, create ReviewItem, record approval/rejection/edit, apply/materialize only after approval.

Review Center trigger:
lifemodel_change.

Task state:
completed_with_pending_items until review; completed only when durable materialization is actually done.

LifeModel / Memory impact:
May create memory/evidence refs and canonical LifeModel update after approval/materialization.

Diagnostics visibility:
Default shows source and impact; raw patch/provenance/debug details are advanced.

Required ViewModel fields:
before/after summary, affected dimension, source refs, review status, materialization status, audit refs.

Failure / empty state:
If source/provenance is missing, mark open item and avoid claiming canonical update.

Success criteria:
User can distinguish proposed, approved, applied, and historical LifeModel states.

Evidence classification:
`VERIFIED_FACT` for LifeModel/provenance primitives; LifeModelViewModel is `PHASE_2_REQUIRED`.

Open questions:
How should manual LifeModel editing be governed and described?

## Scenario S5: A tool call fails; user needs to understand what happened without reading raw trace

User goal:
Understand a failed tool/action and know what to do next.

Entry surface:
`工作区` or `任务`.

Surfaces involved:
`工作区`, `任务`, `高级检查`, possibly `设置` or `审核中心`.

Default UI:
Show failed or blocked state, plain-language reason, next action, and optional "查看依据".

System understanding:
OpenLife identifies whether the failure came from permission, risk policy, missing/incomplete manifest, safe path, provider/model, network, runtime, or unknown cause.

Execution timeline:
Show attempted action, failure category, blocker/failure result, and retry/review/settings path if available.

Review Center trigger:
Only when permission/review can resolve the failure.

Task state:
failed or blocked, never completed.

LifeModel / Memory impact:
No durable update unless a separate review item exists.

Diagnostics visibility:
Default shows summary and next action; raw trace/tool metadata/provider health behind advanced inspector.

Required ViewModel fields:
failure category, user-readable reason, retry eligibility, related permission/review state, evidence/debug refs.

Failure / empty state:
If cause is unknown, say unknown and expose advanced evidence; do not invent a cause.

Success criteria:
User understands what happened, whether anything changed, and what can be done next.

Evidence classification:
`VERIFIED_FACT` for ToolGateway/fail-closed primitives; concise failure taxonomy is `PHASE_2_REQUIRED`.

Open questions:
What failure taxonomy should be canonical in backend read models?

## Phase 2 Entry Checklist

Before Phase 2 starts, humans must approve:

- V2 decision record.
- Memory top-level vs LifeModel subnav / Settings sub-surface.
- ReviewItem type/status/action model.
- Workspace responsibility migration.
- Chinese product vocabulary.
- Diagnostics visibility policy.
- ViewModel ownership strategy: expand `LifeStateProjection` or add dedicated read models.
- Backend validation list for all `PHASE_2_REQUIRED` fields.
- Product trial/E2E expectations before claiming readiness.
- Stop-rule review for IA, Review Center, ViewModel, language, diagnostics, and scenario evidence boundaries.

## Non-Implementation Confirmation

`VERIFIED_FACT`: This Phase 1 package is documentation-only. It does not authorize React components, routes, CSS, backend contracts, command changes, bridge changes, ProductShell refactors, ChatPage refactors, MailboxPage refactors, or Frontend V2 implementation.
