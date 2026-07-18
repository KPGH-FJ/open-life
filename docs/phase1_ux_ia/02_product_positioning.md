# OpenLife Product Positioning

Status: Phase 1 product positioning proposal.
Scope: Product framing and guardrails only.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## One-line Positioning

`DESIGN_DECISION`: OpenLife is a local-first personal AI operating partner that helps a Chinese-first user think, plan, act, and remember through a private LifeModel while keeping consequential changes reviewable and auditable.

## What OpenLife Is

- `VERIFIED_FACT`: OpenLife has a Tauri 2 desktop shell, Rust command layer, `openlife-core`, React/Tailwind frontend, SQLite-backed local stores, Main Chat send/stream runtime, proposal/review primitives, memory, LifeModel, tool governance, task state, and audit evidence. Source: `docs/openlife-phase0-audit/13_audit_summary.md`.
- `DESIGN_DECISION`: OpenLife should be presented as a personal AI operating system, not as a chat wrapper or dashboard. It should make intent, plan, execution, review, memory, evidence, and long-term understanding legible to ordinary users.
- `DESIGN_DECISION`: OpenLife's first V2 product experience should center on work with the user's life context: `今日`, `工作区`, `任务`, `审核中心`, `LifeModel`, `记忆`, and `设置`, subject to Memory constraints.

## What OpenLife Is Not

OpenLife is not:

- a generic chat app;
- a dashboard;
- a todo app;
- a CRM;
- a knowledge base;
- a raw database browser;
- a developer console.

`DESIGN_DECISION`: V2 copy and IA must avoid reducing OpenLife to one of these familiar but incorrect categories.

## User Promise

`DESIGN_DECISION`: OpenLife helps the user express a goal, see what the system understood, watch work progress, review consequential changes, and understand what changed or did not change.

`DESIGN_ASSUMPTION`: Chinese-first users will trust the product more if default copy uses ordinary language such as `等待你确认`, `已阻断`, `依据`, and `待确认项` instead of `run`, `trace`, `proposal`, `kernel`, or `provider`.

## Trust Promise

`VERIFIED_FACT`: Governance primitives exist: ProposalStore, partial ReviewWorkflow, ToolGateway, ToolPermissionStore, MemoryGateway, LifeModelWriteGateway, privacy engine, safe mode, danger preflight, safe-path file write validation, and audit stores. Source: `docs/openlife-phase0-audit/02_backend_capability_map.md`, `docs/openlife-phase0-audit/06_security_governance_audit.md`.

`DESIGN_DECISION`: The product promise is not "OpenLife silently does everything." The promise is "OpenLife can help act, but consequential changes are visible, reviewable, and auditable."

`UNKNOWN`: A real desktop/Tauri product trial has not proven the full experience green. Phase7 remains `red-until-trial-green`.

## Control Promise

`DESIGN_DECISION`: Product states must distinguish:

- completed work;
- completed work with pending review items;
- waiting for user confirmation;
- blocked work;
- failed work;
- cancelled work;
- proposals that are not yet durable changes.

`VERIFIED_FACT`: Backend/final-delivery evidence already preserves blocked, failed, pending proposal, and completed distinctions. Source: `docs/openlife-phase0-audit/03_agent_system_analysis.md`.

## Local-first Privacy Framing

`VERIFIED_FACT`: The product has local SQLite stores, privacy masking/blocking primitives, safe paths, tool permissions, and external-transmission/provider evidence surfaces. Source: `docs/openlife-phase0-audit/06_security_governance_audit.md`, `docs/phase0_5/04_diagnostics_visibility_inventory.md`.

`DESIGN_DECISION`: Default product copy should explain where work happens and whether external transmission is involved without exposing provider/router internals by default.

`PHASE_2_REQUIRED`: Define a user-facing provider/privacy trust summary before implementing Settings or Workspace V2 surfaces.

## Chinese-first First-version Audience

`DESIGN_DECISION`: V2 should use Chinese route labels and status/action copy for normal product surfaces.

`DESIGN_DECISION`: Keep `LifeModel` as an English-branded domain term with a Chinese explanation, because Phase 0 evidence treats it as a distinctive structured model rather than a generic "profile."

`DESIGN_ASSUMPTION`: The first version should be calm, precise, and work-focused, learning general principles from agent/dev productivity products without copying them.

## Product Capability Preservation Principle

`DESIGN_DECISION`: Guardrails prevent hallucination; they must not shrink OpenLife into a generic chat app, todo list, settings panel, or dashboard.

Important capabilities that are product-critical but not fully verified must be preserved as `CANDIDATE` or `PHASE_2_REQUIRED`, not deleted:

- Memory top-level navigation and lane model;
- LifeModel provenance and change explanation;
- Review Center beyond proposals;
- tool permission and external-write review;
- advanced evidence inspection;
- provider/privacy boundary summaries;
- task continuity.

## Product Boundaries

`VERIFIED_FACT`: `frontend/src/tauri.ts` is the product bridge. `frontend/src/tauriDev.ts` is dev/test compatibility and must not be treated as product authority. Source: `docs/openlife-phase0-audit/05_backend_frontend_contract.md`, `docs/phase0_5/02_current_route_map.md`.

`DESIGN_DECISION`: Phase 1 does not authorize React components, routes, CSS, backend schemas, command changes, Tauri bridge changes, ProductShell refactors, ChatPage refactors, or MailboxPage refactors.

`PHASE_2_REQUIRED`: Humans must approve IA, language, diagnostics visibility, and ViewModel ownership before Frontend V2 implementation starts.

## Open Questions

1. `UNKNOWN`: Should `记忆` remain top-level or become a LifeModel / Settings sub-surface after read-model validation?
2. `UNKNOWN`: Which advanced trust/safety tools, especially `版本` and `校准`, belong in normal product navigation?
3. `UNKNOWN`: What exact provider/privacy summary should ordinary users see by default?
4. `UNKNOWN`: Should a companion/ambient mode remain inside `工作区`?
5. `UNKNOWN`: Which memory lanes, if any, may materialize without Review Center approval?
