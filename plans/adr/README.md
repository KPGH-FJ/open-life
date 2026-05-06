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
| 0007 | PlanMode Confirmation Policy | proposed | Phase 6 |
| 0008 | SubAgent Default Permissions and Delegation Modes | proposed | Phase 7 |
| 0009 | ExecutionSandbox and Bash/Shell Boundary | proposed | Phase 9 |
| 0010 | ChatPage State Model Migration Policy | proposed | Phase 10 |

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

ADR 0007-0010 drafts now exist and should be reviewed after P0/P1 code is committed and before P2/P3 implementation.

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
