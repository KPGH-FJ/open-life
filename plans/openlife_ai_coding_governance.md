# OpenLife AI Coding Governance

Date: 2026-05-06

This document defines how AI coding should participate in the Agent Framework upgrade without taking over high-risk architecture decisions.

## Core Workflow

High-risk architecture work must use:

```text
AI propose
-> Human review
-> Human decide
-> AI implement
-> Tests verify
-> Docs update
```

AI can analyze, compare options, draft ADRs, write tests, and implement approved specs. Human reviewers decide the values, defaults, safety boundaries, permission model, and user-sovereignty tradeoffs.

## ADR-First Rule

The following topics require an ADR before implementation:

- LifeModel governance.
- Bash/Shell safety boundary.
- Sub-agent default permissions.
- Cloud privacy policy.
- ChatPage state model rewrite.
- PromptStack/system prompt architecture.
- Memory to LifeModel evolution.
- PlanMode confirmation policy.
- ExecutionSandbox defaults.

No high-risk boundary enters implementation without an accepted ADR.

## ADR Template

Use this template:

```markdown
# ADR: <title>

Date:
Status: proposed | accepted | rejected | superseded

## Context

What problem are we solving?

## Options

1. Option A
2. Option B
3. Option C

## AI Recommendation

What does the AI recommend and why?

## Human Decision

What did the human reviewer decide?

## Consequences

What gets easier, harder, safer, or riskier?

## Implementation Guardrails

What must implementation not violate?

## Verification

What tests or reviews prove the decision was followed?
```

## Topic-Specific Governance

### LifeModel Governance

AI can:

- propose field risk classification
- identify mutation paths
- draft proposal-first policies
- design rollback and evidence rules

Human must decide:

- which fields are high risk
- whether any low-risk field can auto-apply
- how user sovereignty is expressed
- what rejection means for future inference

Default stance:

- identity, values, mission, role definition, and long-term goals never auto-apply
- preferences, relationships, and capabilities are proposal-first
- low-risk state may be eligible for lightweight policy only after ADR approval

### Bash/Shell Safety Boundary

AI can:

- compare sandbox designs
- draft deny-read and command policies
- implement approved allowlists and tests

Human must decide:

- whether Bash exists in product
- who can trigger it
- what commands are permanently forbidden
- whether scheduled/proactive runs may use it

Default stance:

- Bash default-off
- no shell without ExecutionSandbox
- write effects proposal-first
- no inherited full environment

### Sub-Agent Default Permissions

AI can:

- propose sub-agent role matrix
- design context isolation
- implement approved call-as-tool flow

Human must decide:

- which sub-agents ship first
- which tools each role can use
- whether any sub-agent can access LifeModel or memory evidence
- whether handoff/parallel modes are allowed

Default stance:

- minimal permissions
- read-only planner
- no direct writes
- child AgentRun links to parent AgentRun

### Cloud Privacy Policy

AI can:

- draft privacy levels
- propose redaction and summary strategies
- implement ModelRouter policy tests

Human must decide:

- which LifeModel fields never leave device
- when cloud models can see memory evidence
- whether user confirmation is needed for sensitive cloud calls

Default stance:

- cloud gets summaries by default
- raw LifeModel and sensitive memories stay local unless explicitly allowed
- route decisions must be traceable

### ChatPage State Model

AI can:

- map current component state
- propose decomposition
- migrate one subcomponent at a time
- write regression tests

Human must decide:

- UX priorities
- interaction behavior that cannot regress
- acceptable temporary UI changes

Default stance:

- no wholesale rewrite
- split incrementally
- backend runtime migration and frontend state rewrite should not happen in the same task

### PromptStack / System Prompt

AI can:

- draft prompt blocks
- compare prompt architecture options
- write prompt assembly code
- test cloud filtering and version trace

Human must decide:

- base system principles
- tone and product philosophy
- privacy red lines
- output schema strictness

Default stance:

- PromptStack is required before PlanMode/SubAgent expansion
- prompt blocks are versioned
- cloud disallowed blocks are filtered or summarized

### Memory to LifeModel Evolution

AI can:

- propose evidence scoring
- detect patterns and contradictions
- draft evolution proposals
- implement approved MemoryEvidence schema

Human must decide:

- evidence thresholds
- risk classification
- whether rejected proposals become negative evidence
- what kinds of inference are too intimate or speculative

Default stance:

- memory-driven evolution creates proposals only
- high-risk LifeModel fields need explicit review
- evidence links are required

## AI Coding Task Classes

Good AI coding tasks:

- add tests
- extract types
- split functions
- migrate one call path
- implement a schema from spec
- add event logging
- implement policy checks
- update docs after implementation

Risky AI coding tasks:

- inventing LifeModel governance rules
- deciding default shell permissions
- rewriting ChatPage state wholesale
- designing sub-agent permissions from scratch
- changing prompt philosophy without review
- removing fallback paths without tests

## Implementation Rules

1. Every architecture task must name the primitive it affects.
2. Every task must list files it is allowed to edit.
3. Every task must have verification steps.
4. New execution behavior must have tests.
5. New tool behavior must go through ToolRuntime.
6. New prompt behavior must go through PromptStack after PromptStack exists.
7. New LifeModel mutation behavior must go through Proposal.
8. New memory evolution behavior must include evidence links.
9. AI must not combine unrelated migrations in one patch.
10. AI must not delete legacy paths until tests prove replacement behavior.

## Review Checklist

Before accepting AI-generated architecture work:

- Does it preserve user sovereignty?
- Does it create or update AgentRun trace?
- Does it bypass ToolRuntime?
- Does it bypass Proposal/Permission/Audit?
- Does it add prompt text outside PromptStack?
- Does it expose raw LifeModel or memory to cloud unexpectedly?
- Does it weaken declarative-only tool filtering?
- Does it make sub-agent or shell behavior less bounded?
- Does it include tests or a clear reason tests were not run?
- Does it update the relevant architecture document?

## Recommended First ADRs

1. ADR: PromptStack and Base System Prompt Constitution
2. ADR: AgentRunEvent Append-Only Trace Model
3. ADR: LifeModel Field Risk Classification
4. ADR: MemoryEvidence and LifeModel Evolution Proposal Policy
5. ADR: ToolRuntime Metadata and Declarative-Only Enforcement
6. ADR: PlanMode Confirmation Policy

## Active vNext Planning Artifacts

Use these documents together:

- `plans/current_agent_runtime_audit.md`: current runtime facts.
- `plans/openlife_vnext_architecture_principles.md`: framework principles.
- `plans/openlife_vnext_architecture_diagrams.md`: architecture and sequence diagrams.
- `plans/openlife_vnext_core_primitives_and_boundaries.md`: primitive definitions and boundaries.
- `plans/openlife_vnext_migration_plan.md`: phase order.
- `plans/openlife_vnext_p0_p1_task_specs.md`: AI-coding-ready task specs.
- `plans/openlife_vnext_p2_p3_task_specs.md`: next-stage PlanMode/SubAgent/Sandbox task specs.
- `plans/openlife_vnext_agent_coding_prompts.md`: reusable prompts for driving later Agent coding.
- `plans/openlife_vnext_test_and_acceptance_matrix.md`: phase acceptance tests.
- `plans/adr/README.md`: ADR backlog and workflow.

Before implementation, every task should cite:

1. the affected primitive,
2. the phase,
3. the relevant ADR if required,
4. allowed edit areas,
5. verification steps.
