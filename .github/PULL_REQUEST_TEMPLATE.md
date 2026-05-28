## Summary

-

## Related Issue

- Closes #

## PR Type

- [ ] LifeModel-HS task
- [ ] Engineering task
- [ ] Bug fix
- [ ] Documentation only
- [ ] Infrastructure / CI

## Scope

- Selected LMHS task:
- First-pass plan reviewed: yes / no / not applicable
- Files changed:
- Out-of-scope work intentionally avoided:

## ADR 0013 / Governance Check

For non-LMHS work, mark items as not applicable only when they truly do not touch LifeModel, memory, privacy, model routing, tools, proposals, audit, or runtime authority.

- [ ] Change is additive and does not switch LifeModel source of truth in one step.
- [ ] Current YAML LifeModel remains a compatibility materialized view.
- [ ] Risky HS mutation remains Proposal-first.
- [ ] Privacy remains hard Policy; heuristics do not relax Policy.
- [ ] No identity, values, mission, long-term goal, sensitive relationship, or privacy-boundary update auto-applies.
- [ ] Runtime/audit/regression records are metadata-safe and avoid raw sensitive payloads.
- [ ] PromptStack, ModelRouter, ToolRuntime, ExecutionFacade, Proposal, and AgentRunEvent governance are preserved.
- [ ] This PR does not implement adjacent LMHS tasks without explicit issue scope.

## Security / Privacy Check

- [ ] No API keys, secrets, raw LifeModel, raw memory, private file contents, raw sensitive chat, or complete private prompts are included.
- [ ] Logs, fixtures, screenshots, diagnostics, and audit payloads are redacted or metadata-only where needed.

## Tests

- [ ] Task-specific verification:
- [ ] Focused module tests:
- [ ] `make ci`:

## Screenshots / Logs

- Screenshots:
- Logs or trace excerpts:
- Redaction note:

## Documentation

- [ ] Docs updated, or not needed because:

## Review Notes

- Risky assumptions:
- Remaining risks:
- Follow-ups:
