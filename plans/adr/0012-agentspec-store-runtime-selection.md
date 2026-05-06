# ADR 0012: AgentSpec Store And Runtime Selection

Date: 2026-05-06

Status: accepted

## Context

P6 introduced AgentSpec, AgentTask, ContextPolicy, PromptStack binding helpers, and AgentSpec-aware PlanExecutor policy. Those pieces are now present, but the selected AgentSpec is still mostly defaulted or passed directly by helper APIs. P7 needs a durable source of truth so runtime behavior can answer:

- which AgentSpec governed this run,
- where that AgentSpec came from,
- which PromptBlocks were used,
- which context categories were included or excluded,
- and why a tool was allowed or blocked.

Without a store and deterministic resolution policy, OpenLife risks reintroducing ad hoc prompt fragments and default-policy execution paths.

## Decision

OpenLife will add a durable `AgentSpecStore` and use it as the canonical source for runtime AgentSpec selection.

The default main AgentSpec must be bootstrapped with a stable id, for example:

```text
main.default
```

Runtime selection must follow a deterministic order:

1. explicit task or plan AgentSpec id,
2. run/task association when available,
3. stored default main AgentSpec.

Implicit fallback to an arbitrary in-memory `AgentSpec::default()` is allowed only during bootstrap or tests. Production command paths should resolve the stored default main spec.

## Options Considered

### Option A: Keep using `AgentSpec::default()`

Rejected. It is simple, but it does not create a durable audit trail or allow future specialist AgentSpecs.

### Option B: Store AgentSpec only in frontend settings

Rejected. AgentSpec is a runtime governance object. Backend execution must not depend on frontend UI state.

### Option C: Add a backend AgentSpecStore

Accepted. This matches existing store patterns, keeps execution local-first, and gives AgentRuntime / PlanExecutor a stable policy source.

## Consequences

- App bootstrap must ensure a default main AgentSpec exists.
- Tauri AppState must own or reach an AgentSpecStore.
- AgentRuntime and plan execution commands must resolve AgentSpec before prompt/context/tool decisions.
- AgentRunEvent payloads should include AgentSpec id and prompt block metadata where relevant.
- Missing explicit specs should produce structured errors unless documented fallback to stored default is allowed.

## Implementation Guardrails

- AgentSpec cannot grant authority beyond ToolRuntime, ActionExecutor, Permission, Proposal, ExecutionSandbox, or PlanExecutor.
- PromptStack remains the only supported path for AgentSpec prompt block ids.
- ContextPolicy remains the only supported path for AgentSpec context inclusion/exclusion.
- No raw prompt content, raw memory snippets, or full LifeModel should be written into governance trace events.
- No Bash/Shell work is unlocked by this ADR.
- No SubAgent parallel or handoff execution is unlocked by this ADR.
- A full AgentSpec editor or marketplace is out of scope for P7.

## Verification

- Default main spec bootstraps and round-trips through AgentSpecStore.
- Runtime execution with AgentSpec uses PromptStack prompt block ids.
- Unknown prompt block id fails before model calls.
- AgentSpec-derived ContextPolicy excludes denied memory/LifeModel/tools context.
- Plan execution resolves a stored AgentSpec and records `agentspec_id`.
- AgentSpec-denied tools are blocked before ActionExecutor/ToolRuntime execution.

## Open Questions

- Should AgentSpec updates require Proposal review once a UI editor exists?
- Should inactive AgentSpecs remain selectable for historical replay?
- Should future specialist AgentSpecs be installed by built-in registry, manifest, or user-authored config?
- Which event type should represent AgentSpec selection if existing event payload metadata is not enough?
