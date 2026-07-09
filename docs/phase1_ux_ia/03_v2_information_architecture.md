# OpenLife V2 Information Architecture

Status: Phase 1 IA decision document.
Scope: Navigation and surface ownership proposal only.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Primary IA

`DESIGN_DECISION`: Primary proposed top-level navigation:

```text
今日
工作区
任务
审核中心
LifeModel
记忆
设置
```

Memory status: `Accepted with constraints`.

## Reduced-risk Alternative IA

`CANDIDATE`: If Phase 2 cannot validate a distinct Memory product model, use:

```text
今日
工作区
任务
审核中心
LifeModel
设置
```

LifeModel subnav:

- 概览
- 目标
- 偏好
- 关系
- 记忆
- 依据与变更

## Surface Definitions

### 今日

Purpose: `DESIGN_DECISION` - Daily landing page for current state, review pressure, blockers, safe mode, and next recommended action.
User question answered: "今天我该关注什么？"
Current route mapping: `VERIFIED_FACT` - `/today` maps to `TodayPage`.
Default visible data: safe mode, pending review count, current daily goal, blockers, suggestions, next action.
Hidden/advanced data: raw diagnostics and low-level readiness causes unless needed.
Source of truth: `PARTIAL` - `LifeStateProjection` plus daily-goal reads exist; classified daily cards and next action need Phase 2 validation.
Must not do: Reconstruct global readiness or pending review counts locally when projection fields exist.
Key scenarios: planning today's priorities, reviewing blockers, jumping to `审核中心`.

### 工作区

Purpose: `DESIGN_DECISION` - Current agent work surface for goal entry, understanding, plan, timeline, controls, review links, and result.
User question answered: "OpenLife 理解了什么，正在做什么，需要我确认什么？"
Current route mapping: `VERIFIED_FACT` - `/companion` wraps `ChatPage`; old `/chat` redirects to Companion.
Default visible data: intent summary, task lifecycle, execution timeline, blockers, review-needed links, final result.
Hidden/advanced data: raw reasoning trace, kernel events, durable event stream, provider/router internals.
Source of truth: `PHASE_2_REQUIRED` - backend primitives exist, but no verified `WorkspaceViewModel` exists.
Must not do: Become a renamed ChatPage with the same local-state responsibility overload.
Key scenarios: current work, external write request, failed tool explanation.

### 任务

Purpose: `DESIGN_DECISION` - Active and historical agent work, including lifecycle, controls, evidence, and detail.
User question answered: "哪些任务正在进行、卡住、失败或已完成？"
Current route mapping: `VERIFIED_FACT` - `/runs` and `/runs/:runId`.
Default visible data: task list, status, next control, blocker/review counts, latest result preview.
Hidden/advanced data: full transcript, raw run JSON, kernel trace.
Source of truth: `PHASE_2_REQUIRED` - current pages locally merge AgentRun and task summaries.
Must not do: Present agent task lifecycle as a generic todo list.
Key scenarios: resume, retry, cancel, inspect completed/failed work.

### 审核中心

Purpose: `DESIGN_DECISION` - Central review surface for consequential changes and permissions.
User question answered: "OpenLife 想改变什么，需要我同意什么？"
Current route mapping: `VERIFIED_FACT` - `/mailbox` already handles proposal review actions.
Default visible data: grouped review items, risk, impact, evidence summary, recommendation, available review actions.
Hidden/advanced data: raw proposal payloads and debug-only trace.
Source of truth: `PHASE_2_REQUIRED` - current proposal actions exist, but unified review item grouping and allowed actions need backend validation.
Must not do: Treat proposal creation as durable completion.
Key scenarios: approve/reject/edit/postpone memory, LifeModel, permission, external write, policy, and dangerous action items.

### LifeModel

Purpose: `DESIGN_DECISION` - User-understandable view of OpenLife's structured long-term understanding.
User question answered: "OpenLife 现在怎样理解我？依据是什么？哪些变更待确认？"
Current route mapping: `VERIFIED_FACT` - `/life-model` and `/life-model/build`.
Default visible data: current model summary, dimensions, pending update count, provenance highlights, quality/trust state.
Hidden/advanced data: raw diagnostics, compatibility internals, low-level patch data.
Source of truth: `PARTIAL` - LifeModel and current/provenance views exist, but V2 needs a clearer LifeModel ViewModel.
Must not do: Mix canonical truth, compatibility views, candidate updates, and pending proposals without labeling them.
Key scenarios: inspect long-term understanding, review pending LifeModel changes, build/update LifeModel through proposals.

### 记忆

Purpose: `CANDIDATE` - Explain what OpenLife remembers, what is candidate/pending, what became LifeModel, and what was withdrawn or archived.
User question answered: "OpenLife 记住了什么？哪些只是候选？哪些影响了长期理解？"
Current route mapping: `VERIFIED_FACT` - `/memory` exists as a secondary route grouped under Life Model.
Default visible data: lane/status summaries, active/archived memory, pending memory proposals, provenance, review links.
Hidden/advanced data: raw memory rows, vector internals, diagnostic indexing details.
Source of truth: `PHASE_2_REQUIRED` - Memory primitives exist, but lane-level user read model is missing.
Must not do: Become a raw memory database browser.
Key scenarios: inspect candidate/confirmed/materialized/withdrawn memory and linked evidence.

### 设置

Purpose: `DESIGN_DECISION` - Product-safe setup, privacy, model/provider, tools, data, and advanced inspection entry.
User question answered: "OpenLife 的权限、隐私、模型和高级设置是什么状态？"
Current route mapping: `VERIFIED_FACT` - `/settings`.
Default visible data: setup readiness, safe mode, provider/privacy summary, tool permissions, data control, advanced mode entry.
Hidden/advanced data: PolicyRouter, ModelRouter internals, MCP/A2A details, metrics, debug toggles.
Source of truth: `PARTIAL` - settings data exists but is currently assembled from many diagnostics/admin sources.
Must not do: Become a diagnostic junk drawer or a second product truth source.
Key scenarios: manage permissions, understand external transmission, inspect advanced support data.

## Current Route To V2 Surface Migration Matrix

| Current page / route | V2 destination | Preserve | Migrate | Remove / hide | Risk |
| --- | --- | --- | --- | --- | --- |
| Today / `/today` | 今日 | daily entry, safe mode, review count | daily goal card classification to backend-backed ViewModel | raw diagnostics as default UI | Page-local goal/suggestion classification may overclaim product truth. |
| ChatPage | 工作区 | user input, intent, task lifecycle, controls, final result | timeline, review links, result, evidence refs into `WorkspaceViewModel` | default raw trace/kernel/provider labels | Recreating ChatPage responsibility overload. |
| CompanionPage / `/companion` | 工作区 | companion entry and stage feel | companion mode into workspace composer/state | independent companion shell unless human-approved | Losing emotional/ambient value if workspace is too task-heavy. |
| Mailbox / `/mailbox` | 审核中心 | accept/reject/postpone/edit, safe mode checks, proposal folders | unified ReviewItem model and actions | mailbox naming and message-inbox framing | Review decisions remain proposal-only and miss permissions/external writes. |
| Runs / `/runs`, `/runs/:runId` | 任务 | task/run history, resume/cancel/retry/delete preflight, evidence | merged task/run read model, concise lifecycle copy | raw run/debug detail by default | AgentRun/task relationship remains page-local and confusing. |
| LifeModel / `/life-model` | LifeModel | structured model, current view, evidence, builder entry | canonical/candidate/provenance/trust ViewModel | raw diagnostics mixed into default model understanding | Users confuse current compatibility view with canonical truth. |
| Memory / `/memory` | 记忆 or LifeModel subnav | search, archive/restore, tier stats, manual index capability | lane/status/provenance/read-model explanation | raw memory database feel and indexing internals | Top-level Memory overlaps LifeModel/Review/Evidence unless constrained. |
| Settings / `/settings` | 设置 | setup, privacy, provider, tool permissions, data controls | trust summary plus advanced/developer layers | diagnostic junk drawer as default | Settings becomes second readiness authority. |
| `/mcp`, `/a2a` | 开发者 / 高级检查 (NEEDS_HUMAN_DECISION; not primary product nav) | external connection management and audit | visibility decided by human review | default product navigation | External capability feels product-ready without journey evidence. |
| `/metrics` | 开发者 | operational metrics | support/debug mode if needed | normal product route | Product feels like developer console. |
| `/calibration`, `/versions` | 高级检查 / 设置 (NEEDS_HUMAN_DECISION; not primary product nav) | trust/safety and maintenance value | either user-facing trust subflows or advanced tools | default technical copy | Hiding too much weakens trust; showing too much overwhelms. |

## Navigation Rationale

- `VERIFIED_FACT`: Current navigation already separates primary, secondary, and advanced route groups. Source: `docs/phase0_5/02_current_route_map.md`.
- `DESIGN_DECISION`: V2 should keep stable left navigation but rename surfaces around user work and review, not implementation terms.
- `DESIGN_DECISION`: `高级检查` is a visibility layer, not necessarily a top-level primary route.
- `PHASE_2_REQUIRED`: Route implementation must wait until ViewModel ownership and human IA approval are complete.

## First-version Scope

1. `DESIGN_DECISION`: Plan for the primary IA above, with Memory marked constrained.
2. `PHASE_2_REQUIRED`: Define backend-owned ViewModels before component/route work.
3. `DESIGN_DECISION`: Preserve evidence access through expandable/advanced layers.
4. `UNKNOWN`: Do not claim browser E2E, desktop trial, live provider, Web AgentLoop, or MCP AgentLoop readiness from IA docs.

## Phase 2 IA Stop Rules

1. `PHASE_2_REQUIRED`: Do not create or rename routes until human review approves final IA and ViewModel ownership.
2. `PHASE_2_REQUIRED`: Do not promote `/mcp`, `/a2a`, `/calibration`, `/versions`, or `/metrics` into primary product navigation from this document alone.
3. `PHASE_2_REQUIRED`: Do not implement top-level `记忆` unless the Memory lane/status/provenance read model is validated.
4. `PHASE_2_REQUIRED`: Do not treat reduced-risk alternative IA as rejected; it remains the fallback if Memory boundaries are unclear.

## Open Questions

1. Should Memory remain top-level after Phase 2 validates lane/status/provenance support?
2. Should `任务` remain top-level or move under `工作区` after task read-model design?
3. Which advanced routes are support tools versus product trust controls?
4. Should `Safe Mode` be displayed as `安全模式` or `Safe Mode（安全模式）`?
5. What default route should open on first launch after onboarding?
