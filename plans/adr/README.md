# OpenLife Architecture Decision Records

Date: 2026-05-06

This directory contains Architecture Decision Records for the OpenLife vNext Agent Framework upgrade.

## ADR Workflow

High-risk architecture changes use:

```text
AI propose
-> Human review
-> Human decide
-> AI implement
-> Tests verify
-> Docs update
```

ADR statuses:

- `proposed`: drafted, not approved.
- `accepted`: approved and can guide implementation.
- `rejected`: explicitly not chosen.
- `superseded`: replaced by a newer ADR.

Implementation should not begin for high-risk boundaries until the relevant ADR is `accepted`.

## Required ADRs for vNext

| ADR | Title | Status | Blocks |
|---|---|---|---|
| 0001 | AgentRunEvent Append-Only Trace Model | accepted | Phase 3 |
| 0002 | PromptStack and Base System Prompt Constitution | accepted | Phase 4, PlanMode, SubAgent |
| 0003 | ToolRuntime Metadata and Declarative-Only Enforcement | accepted | Phase 3, all tools |
| 0004 | LifeModel Field Risk Classification | accepted | Memory evolution, calibration, proposal apply |
| 0005 | MemoryEvidence and LifeModel Evolution Proposal Policy | accepted | Phase 5 |
| 0006 | Cloud Privacy Policy and ModelRouter Disclosure Rules | accepted | PromptStack, MemoryEvidence, ModelRouter |
| 0007 | PlanMode Confirmation Policy | accepted | Phase 6, P4 Plan Execution |
| 0008 | SubAgent Default Permissions and Delegation Modes | accepted | Phase 7, P4 Review Gate |
| 0009 | ExecutionSandbox and Bash/Shell Boundary | accepted | Phase 9 |
| 0010 | ChatPage State Model Migration Policy | accepted | Phase 10, P4 Trace UI |
| 0011 | Plan Recovery and Rollback Policy | proposed | P5 retry/cancel/rollback |
| 0012 | AgentSpec Store And Runtime Selection | proposed | P7 AgentSpecStore, runtime selection |
| 0013 | LifeModel-HS Source Of Truth And Governance | accepted | Post-Beta LifeModel-HS design and MVP |

## Recommended Acceptance Sequence

P0 can begin with documentation-only mapping and store skeleton work while ADRs are still proposed. P1 implementation should not begin until the relevant ADRs are accepted.

Suggested order:

1. Accept ADR 0001 and ADR 0003 first.
   - Unlocks AgentRunEvent trace work and ToolRuntime declarative-only enforcement.
2. Accept ADR 0002 and ADR 0004 next.
   - Unlocks PromptStack skeleton, AgentRole prompt migration, and LifeModel risk matrix.
3. Accept ADR 0005 and ADR 0006 after that.
   - Unlocks MemoryEvidence and cloud privacy/model routing work.
4. Draft and accept ADR 0007-0010 before Phase 6-10 implementation.
   - PlanMode, SubAgent permissions, ExecutionSandbox/Bash, and ChatPage migration remain blocked until then.

ADR 0007-0010 are accepted and have guided P2/P3 plus P3 hardening. Further high-risk changes should add new ADRs or supersede these records explicitly.

ADR 0011 is proposed for P5. Implement cancellation and whole-plan retry only within the conservative policy in `plans/openlife_vnext_p5_task_specs.md`; rollback implementation should wait until ADR 0011 is accepted.

ADR 0012 is proposed for P7. Implement AgentSpecStore and runtime selection conservatively: bootstrap a stored default main AgentSpec, resolve specs deterministically, and do not let AgentSpec grant authority beyond ToolRuntime, ActionExecutor, Proposal, PromptStack, ContextPolicy, AgentRunEvent, ExecutionSandbox, or PlanExecutor.

ADR 0013 is accepted for Post-Beta LifeModel-HS work. Implement the next LifeModel phase as an additive Personal Heuristic System layer: keep current YAML as a compatibility materialized view during migration, introduce canonical accepted HS assets incrementally, enforce Proposal-first governance for risky changes, treat privacy as hard Policy rather than a soft Heuristic, and limit automatic updates to low-risk transient state plus low-risk maintenance metadata. Coding work should follow `plans/lifemodel_hs_mvp_task_specs.md` one task at a time.

## Decision Backlog

### P0 Decisions

These must be resolved before implementation beyond trace/path preparation:

1. What is the persisted shape of `AgentRunEvent`?
2. Is `AgentRunEvent` stored in the existing agent runs database or a new event store?
3. Which event payloads are redacted, summarized, or stored raw?
4. What is the minimum required event set for P0?
5. What makes a tool model-callable?
6. What is the canonical declarative-only enforcement point?

### P1 Decisions

These must be resolved before PromptStack and MemoryEvidence implementation:

1. What are OpenLife's base system prompt principles?
2. Which prompt blocks are cloud disallowed by default?
3. Which LifeModel fields are high/medium/low risk?
4. Which LifeModel fields are never allowed in cloud prompts?
5. Can low-risk state updates auto-apply under any policy?
6. How should rejected proposals affect future memory evidence?

### P2 Decisions

These must be resolved before PlanMode/SubAgent/Bash implementation:

1. Is PlanMode explicit, automatic, or both?
2. Which task risk classes require plan confirmation?
3. Which sub-agent roles ship first?
4. Can sub-agents call tools directly or only request the parent agent to call them?
5. Is Bash available to user-initiated tasks, scheduled tasks, or neither by default?
6. What is the default filesystem deny-read list?

### P5 Decisions

These must be resolved before rollback implementation and advanced recovery:

1. Does retry create a new `AgentRun` or reuse the parent run with attempt markers?
2. Which local side effects have enough metadata for rollback?
3. Which external side effects are explicitly irreversible?
4. Should failed review ever allow a user override?
5. Should rollback proposals live in Review Center or a dedicated plan recovery surface?

### P7 Decisions

These must be resolved before advanced specialist agents or AgentSpec editing:

1. What is the stable id of the default main AgentSpec?
2. Does AgentSpecStore live in the existing agent database or a dedicated SQLite file?
3. Should missing explicit AgentSpec ids fail or fall back to the stored default main spec?
4. Which AgentSpec changes eventually require Proposal review?
5. Should inactive AgentSpecs be usable for historical replay only?

### Post-Beta LifeModel-HS Decisions

These are resolved by ADR 0013 and should guide LifeModel-HS MVP planning:

1. LifeModel-HS canonical truth should move toward accepted HS assets; YAML remains a compatibility materialized view during migration.
2. Low-risk auto-accept is limited to transient StateAsset updates with TTL.
3. Privacy is a hard Policy boundary, not merely a Heuristic.
4. Raw data deletion must weaken, archive, or tombstone linked evidence according to user intent.
5. Active heuristics should have per-domain caps and compression pressure.
6. Maintenance auto-actions are limited to low-risk metadata, expiration, cache, diagnostics, and materialized-view rebuilds.

## ADR Template

```markdown
# ADR 0000: Title

Date:
Status: proposed

## Context

## Decision

## Options Considered

## Consequences

## Implementation Guardrails

## Verification

## Open Questions
```
