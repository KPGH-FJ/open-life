# ADR 0003: ToolRuntime Metadata and Declarative-Only Enforcement

Date: 2026-05-06
Status: accepted

## Context

OpenLife uses tools as agent execution capability. Some tools are real executors, while others are declarative-only stubs or proposal-generating tools. vNext must ensure models only see tools that are genuinely callable under policy.

## Decision

Formalize tool metadata and make declarative-only enforcement a ToolRuntime guarantee.

## Required Tool Metadata

Every tool must declare:

- `name`
- `description`
- `source`
- `executable`
- `declarative_only`
- `risk_level`
- `permission_policy`
- `executor_kind`
- `input_schema`
- `output_schema`
- `side_effect_type`

Recommended additional fields:

- `cloud_safe_description`
- `requires_safe_path`
- `requires_network`
- `writes_external_state`
- `proposal_type`
- `audit_category`

## Enforcement Rules

1. `declarative_only == true` means the tool cannot appear in the model-callable ToolPrompt.
2. `executable == false` means the tool cannot be executed by ToolRuntime.
3. Proposal-generating tools can be executable even when they do not apply side effects directly.
4. Write tools should create proposals by default.
5. Replay must re-check permission and current policy.
6. Blocked tools must create an observation/event explaining why they were blocked.

## Options Considered

### Option A: Enforce at prompt-generation only

Pros:

- Simple.

Cons:

- A malformed or legacy path could still attempt execution.

### Option B: Enforce at ActionExecutor only

Pros:

- Central execution guard.

Cons:

- Model may still be invited to call unavailable tools.

### Option C: Enforce at both ToolPrompt and ToolRuntime

Pros:

- Defense in depth.
- Better user trust.

Cons:

- Requires metadata consistency.

## Recommendation

Use Option C.

## Consequences

Positive:

- Models stop planning around fake tools.
- Runtime blocks impossible/unsafe calls.
- Tool availability becomes inspectable.

Tradeoffs:

- Tool registry needs stricter validation.
- Tests must cover prompt and execution enforcement.

## Implementation Guardrails

- No new tool can be registered without required metadata.
- ToolPrompt must filter declarative-only and non-executable tools.
- ToolRuntime must reject declarative-only/non-executable calls even if they arrive.
- Tool permission proposals must include enough blocked action payload to allow safe replay.

## Verification

Tests should prove:

- Declarative-only tools are omitted from ToolPrompt.
- Declarative-only tools are blocked by ToolRuntime.
- Proposal-generating tools remain callable when allowed.
- Permission-required tools create ToolPermission proposals when blocked.
- Replay re-checks policy.

## Open Questions

1. Should P2 tools be visible in UI as unavailable capabilities?
2. Should tool metadata live in Rust structs, manifests, or both?
3. Should MCP tools be normalized into this metadata model before prompt injection?
