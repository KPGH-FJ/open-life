# ADR 0006: Cloud Privacy Policy and ModelRouter Disclosure Rules

Date: 2026-05-06
Status: accepted

## Context

OpenLife is local-first but supports cloud models. vNext introduces richer PromptStack, MemoryEvidence, PlanMode, and sub-agents. Without explicit disclosure rules, sensitive LifeModel and memory context could leak into cloud prompts accidentally.

## Decision

ModelRouter must enforce cloud disclosure policy. ContextAssembler and PromptStack must mark what is cloud-allowed, summarized, redacted, or local-only.

## Privacy Levels

Suggested levels:

- `public`: safe for cloud and logs.
- `personal`: can be summarized for cloud if user settings allow.
- `sensitive`: local-only by default; cloud requires explicit user permission or strong redaction.
- `secret`: never sent to cloud.

## Default Field Policy

Secret/local-only by default:

- API keys, tokens, credentials
- raw private files
- secrets from `.env`, keys, ssh paths

Sensitive/local-first:

- identity values
- mission statement
- relationships
- health and emotional state
- raw memory records
- high-risk LifeModel fields

Personal/summarizable:

- short-term task context
- non-sensitive goals
- user-approved summaries
- low-risk preferences

## Options Considered

### Option A: Use existing privacy engine only

Pros:

- Minimal change.

Cons:

- Does not reason about LifeModel field classes or prompt blocks.

### Option B: Add PromptStack cloud filtering only

Pros:

- Solves prompt assembly disclosure.

Cons:

- ModelRouter decisions still lack full policy awareness.

### Option C: Combine PrivacyPolicy, PromptStack metadata, and ModelRouter

Pros:

- Defense in depth.
- Traceable route decisions.

Cons:

- More coordination across modules.

## Recommendation

Use Option C.

## Consequences

Positive:

- Cloud usage becomes explainable.
- LifeModel context is protected by default.
- Future sub-agents inherit privacy constraints.

Tradeoffs:

- More metadata required.
- Some cloud tasks may need local summarization first.

## Implementation Guardrails

- Cloud model calls must record route/disclosure summary in AgentRunEvent.
- Raw high-risk LifeModel fields do not go cloud by default.
- Raw accepted memories do not go cloud by default.
- Prompt blocks with `cloud_allowed=false` must be filtered or summarized.
- If tool prompt requires cloud-capable model, sensitive context should be minimized.

## Verification

Tests should prove:

- Cloud route omits cloud-disallowed prompt blocks.
- Sensitive LifeModel fields are summarized or removed.
- Secret fields are never sent.
- Route trace records disclosure behavior.

## Open Questions

1. Should users be able to override field-level cloud policy?
2. Should cloud disclosure require per-run confirmation for sensitive tasks?
3. Should local summarization be mandatory before cloud planning?
