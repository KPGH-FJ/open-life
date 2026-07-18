# V2 Decision Record

## Status Values

- Accepted
- Accepted with constraints
- Rejected
- Open
- Needs validation

## Evidence Types

Every Evidence item must include one of:

- Verified Fact from Phase 0 / 0.5
- Existing codebase fact
- Product design rationale
- User experience assumption
- Engineering assumption
- Open item

## Decision Record Template

```text
Decision ID:
Title:
Status:
Decision:
Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:
Product rationale:
Engineering impact:
Risk:
Reversal cost:
Phase 2 implication:
Human approval needed:
```

---

## D1-bounded-rewrite

Decision ID: D1-bounded-rewrite
Title: V2 uses bounded product-experience + state-contract rewrite
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## D2-workspace

Decision ID: D2-workspace
Title: Companion + Chat merge into 工作区
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## D3-review-center

Decision ID: D3-review-center
Title: Mailbox becomes 审核中心
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## D4-tasks

Decision ID: D4-tasks
Title: Runs becomes 任务
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## D5-memory-nav

Decision ID: D5-memory-nav
Title: Memory becomes top-level 记忆
Status: Accepted with constraints

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

### Constraints

Memory may be top-level only if Phase 1 proves it has clear boundaries from:

- LifeModel
- 审核中心
- 工作区 evidence drawer
- 设置 / Data Management

### Fallback

If boundaries are unclear, Phase 2 may downgrade Memory to:

- LifeModel sub-surface, or
- Settings / Data Management sub-surface.

---

## D6-lifemodel-name

Decision ID: D6-lifemodel-name
Title: LifeModel remains English-branded
Status: Accepted with constraints

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

### Constraint

Navigation may use `LifeModel`, but page subtitle/onboarding must explain:

`OpenLife 对你的长期理解`

---

## D7-diagnostics

Decision ID: D7-diagnostics
Title: Diagnostics hidden by default and available through advanced inspection
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## D8-viewmodel-first

Decision ID: D8-viewmodel-first
Title: Backend-owned ViewModels / ReadModels before UI implementation
Status: Accepted

Decision:

Evidence:
  - Type:
    Source:
    Claim:
    Confidence:
    Limitation:

Product rationale:

Engineering impact:

Risk:

Reversal cost:

Phase 2 implication:

Human approval needed:

---

## Rejected Alternatives

## Open Questions

## Decision Summary
