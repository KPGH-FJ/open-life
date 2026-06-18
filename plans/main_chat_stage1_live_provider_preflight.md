# Main Chat Stage 1 Live Provider Preflight

> Date: 2026-06-18
> Scope: opt-in external live dogfood requirements
> Status: preparation artifact

## 1. Purpose

Stage 1 default readiness remains deterministic. External live dogfood is
valuable but must be explicit, auditable, and separate.

## 2. Environment Variables

The harness may use the existing live eval environment names:

```bash
export OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1
export OPENLIFE_LIVE_EVAL_PROVIDER=deepseek
export OPENLIFE_LIVE_EVAL_BASE=https://api.deepseek.com
export OPENLIFE_LIVE_EVAL_MODEL=deepseek-v4-flash
export OPENLIFE_LIVE_EVAL_API_KEY="<from local shell or non-repo secret file>"
```

Rules:

- never commit the API key;
- never serialize the key in reports;
- report key presence as boolean only;
- no `.env` file should be added to git;
- local/mock/fixture provider cannot count as external live credit.
- test commands may omit the key from inline examples, but the implementation
  must fail closed unless `OPENLIFE_LIVE_EVAL_API_KEY` is already exported or
  provided by a non-repo secret mechanism.

## 3. Live Scenarios

Minimum opt-in live scenarios:

- DirectAnswer external provider generation;
- provider-backed web ReAct read;
- provider-backed MCP candidate selection/read;
- provider-backed ToolPermission proposal path.

## 4. Required Live Evidence

Each credited live scenario must prove:

- explicit live opt-in;
- external provider identity;
- provider model identity;
- main Chat ordinary path invoked;
- model/provider invoked;
- task session id and run id;
- route strategy;
- no legacy fallback;
- no silent durable write;
- no local/mock/fixture provider credit;
- bounded response preview;
- scenario-specific action/proposal/tool evidence.

## 5. Fail-Closed Conditions

Live dogfood must fail closed when:

- opt-in is missing;
- API key is missing;
- network is disabled;
- provider identity is local/mock/fixture/synthetic;
- model response cannot be audited;
- tool/action evidence overlaps incorrectly;
- provider-ranked candidate output is partial, malformed, or not an exact
  candidate permutation;
- a live result is used to mark default deterministic readiness.

## 6. Stage 1 Reporting

The Stage 1 report must include live status separately:

- `externalLiveAttempted`;
- `externalLiveScenarioCount`;
- `externalLivePassedCount`;
- `externalLiveBlockedCount`;
- `externalLiveBlockers`;
- `defaultReadinessUnaffectedByLive`.
