# Main Chat Stage 2 Readiness Gate Contract

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation contract

## 1. Purpose

Stage 2 needs one auditable readiness surface. Without a typed gate, manual
dogfood, live-provider evidence, UI coverage, and recovery coverage will drift
into separate claims.

The command should be:

```text
run_main_chat_agent_stage2_readiness_gate
```

The command must return either:

```text
ready_for_limited_internal_trial
```

or:

```text
not_ready_for_limited_internal_trial
```

No intermediate status may be presented as internal-trial readiness.

## 2. Two Different Completion States

Stage 2 implementation can finish before Stage 2 readiness passes.

| State | Meaning |
| --- | --- |
| `implementation_complete_for_stage2_mechanism` | Code/docs/gates exist and fail closed, but required manual dogfood or live provider evidence may still be missing. |
| `ready_for_limited_internal_trial` | All P0 manual, live, UI, memory, recovery, and final-delivery evidence is present and credited. |

Goal-mode Agent work may honestly complete the first state. It must not claim
the second state unless real reviewer and live-provider evidence exists.

## 3. Report Shape

The report should be metadata-safe and include:

| Field | Type | Required meaning |
| --- | --- | --- |
| `schemaVersion` | string | Versioned report shape, e.g. `stage2-readiness-v1`. |
| `runId` | string | Metadata-safe readiness run id. |
| `commit` | string | Git commit used for the report. |
| `recommendation` | enum | `ready_for_limited_internal_trial` or `not_ready_for_limited_internal_trial`. |
| `blockers` | string[] | Metadata-safe blocker ids. Empty only when ready. |
| `deterministicStage1Ready` | bool | Stage 1 automated engineering dogfood remains valid. |
| `betaFoundationReady` | bool | Beta v1 readiness foundations remain valid. |
| `manualDogfood` | object | Manual reviewer evidence summary. |
| `liveProvider` | object | External live-provider evidence summary. |
| `controlPlane` | object | P0 AgentControlPlane state coverage summary. |
| `memoryProposal` | object | M2 memory/proposal scenario summary. |
| `failureRecovery` | object | R2 recovery scenario summary. |
| `finalDelivery` | object | P0 final-delivery honesty summary. |
| `safety` | object | No silent writes, no hidden legacy fallback, no fake evidence. |
| `artifacts` | object[] | Browser/live/manual artifact refs and digests. |

## 4. Manual Dogfood Section

`manualDogfood` must include:

| Field | Required meaning |
| --- | --- |
| `attempted` | Whether manual dogfood evidence was loaded. |
| `reviewerCount` | Distinct reviewer ids. Must be at least 2 for readiness. |
| `requiredScenarioCount` | Count of required P0 manual scenarios. |
| `attemptedScenarioCount` | Count of P0 manual scenarios attempted. |
| `passedScenarioCount` | Count of P0 manual scenarios that passed. |
| `failedScenarioIds` | P0 scenarios with fail/confusing/blocker result. |
| `traceIdsPresent` | Every attempted scenario has trace/run/task ids or explicit missing-trace blocker. |
| `artifactDigest` | Metadata-safe digest of the manual report file. |

The reviewer-facing report path is
`plans/main_chat_stage2_manual_dogfood_report.md`. The machine-readable report
path is `frontend/test-results/main-chat-stage2-manual-dogfood-report.json`.
The readiness gate should load the machine-readable artifact when present and
include its metadata-safe digest in `manualDogfood.artifactDigest`.

Manual reviewer records must not be fabricated by the Agent. If real manual
records are missing, readiness is `not_ready_for_limited_internal_trial`.

## 5. Live Provider Section

`liveProvider` must include:

| Field | Required meaning |
| --- | --- |
| `attempted` | Live P0 eval was attempted with explicit opt-in. |
| `provider` | Metadata-safe non-local provider identity. |
| `model` | Metadata-safe model identity. |
| `requiredScenarioCount` | Count of L2-L01 through L2-L10. |
| `passedScenarioCount` | Count of credited P0 live scenarios. |
| `failedScenarioIds` | Live P0 failures. |
| `modelInvokedCount` | Number of scenarios proving model invocation. |
| `mainChatInvokedCount` | Number of scenarios proving ordinary Main Chat path. |
| `localOrMockCreditRejected` | Count of rejected local/mock/scripted/fixture evidence. |
| `artifactDigest` | Metadata-safe digest of the live report. |

All L2-L01 through L2-L10 must pass for `ready_for_limited_internal_trial`.
Missing credentials/network/provider returns named blockers and not-ready.
Scenarios with contradictory environment needs must use scenario-scoped setup:
for example, L2-L03 must run with web/network policy disabled, while L2-L04
must run with the governed web-read path enabled. A global config that makes one
of these scenarios impossible cannot receive readiness credit.

## 6. Control Plane Section

`controlPlane` must include coverage for these P0 states:

- `direct_answer`;
- `planning`;
- `executing`;
- `observed`;
- `blocked`;
- `waiting_for_permission`;
- `proposal_pending`;
- `retry_available`;
- `cancelled`;
- `completed`.

Each credited state must map to typed runtime payload evidence, not assistant
text.

## 7. Memory Proposal Section

`memoryProposal` must include:

- M2-01 through M2-08 attempted and passed;
- no silent memory write;
- no direct knowledge file write;
- accepted memory inspectable;
- rejected memory not used as accepted truth;
- conflict visible;
- rollback completed and visible.

If rollback remains unsupported after accepting/materializing memory, readiness
must remain `not_ready_for_limited_internal_trial`.

## 8. Failure Recovery Section

`failureRecovery` must include:

- R2-01 through R2-10 attempted and passed;
- every failure has a blocker reason;
- every failure has a user-facing next action or terminal explanation;
- retry/resume/cancel actions are linked to original task/action ids;
- no unsafe replay after denial or stale state.

## 9. Safety Section

`safety` must include:

| Field | Ready requirement |
| --- | --- |
| `silentDurableWriteCount` | `0` |
| `hiddenLegacyFallbackCount` | `0` |
| `fakeBrowserEvidenceCount` | `0` |
| `fakeLiveEvidenceCount` | `0` |
| `localProviderCreditedAsLiveCount` | `0` |
| `unscopedPermissionReplayCount` | `0` |
| `finalDoneOverclaimCount` | `0` |

## 10. Fail-closed Blockers

The gate must return `not_ready_for_limited_internal_trial` when any of these
are true:

- manual dogfood evidence missing;
- fewer than 2 manual reviewers;
- any required P0 manual scenario missing or failing;
- any L2-L01 through L2-L10 live scenario missing or failing;
- any P0 AgentControlPlane state lacks typed runtime evidence;
- M2 memory/proposal P0 flow missing or silently writes;
- R2 recovery P0 flow missing or unsafe;
- final delivery claims proposed/blocked/skipped work is done;
- hidden legacy fallback count is non-zero;
- silent durable write count is non-zero;
- local/mock/scripted/fixture evidence is credited as real browser/live proof.
