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

Artifact refs must not present blocker-bearing evidence as cleanly loaded. A
browser, manual, or live artifact with a digest but fake-evidence or readiness
blockers should use a blocked status rather than `loaded` or `generated`.
The top-level `commit` must be a known metadata-safe build commit. `unknown`,
`none`, fake labels such as `mock-build`, local/scripted/fixture/synthetic
aliases, private-network-looking labels, or missing commit provenance must fail closed with
`stage2_readiness_commit_missing`; the gate must not return
`ready_for_limited_internal_trial` when it cannot identify the build being
certified.

## 4. Manual Dogfood Section

`manualDogfood` must include:

| Field | Required meaning |
| --- | --- |
| `attempted` | Whether manual dogfood evidence was loaded. |
| `reviewerCount` | Distinct reviewer ids. Must be at least 2 for readiness, and at least 2 distinct reviewers must appear on required P0 rows. |
| `requiredScenarioCount` | Count of required P0 manual scenarios. |
| `attemptedScenarioCount` | Count of P0 manual scenarios actually attempted; rows marked `not attempted` do not count. |
| `passedScenarioCount` | Count of P0 manual scenarios that passed. |
| `missingScenarioIds` | Required P0 scenario ids with no real attempted row yet. |
| `failedScenarioIds` | P0 scenarios with fail/confusing/blocker result. |
| `traceIdsPresent` | Every attempted scenario has trace/run/task ids or explicit missing-trace blocker. |
| `artifactDigest` | Metadata-safe digest of the manual report file. |

Manual row `taskId` and `runId` values must be known metadata-safe trace
labels. `unknown`, `none`, missing-trace placeholders, and labels containing
`mock`, `fixture`, `synthetic`, or `scripted` do not satisfy trace evidence and
must fail closed with `stage2_manual_trace_ids_missing`.
Manual row `reviewerId` values must be known metadata-safe reviewer labels.
`unknown`, `none`, and reviewer labels containing `mock`, `fixture`,
`synthetic`, or `scripted` do not identify a real reviewer and must fail closed with
`stage2_manual_reviewer_id_invalid`.
Manual row `prompt`, `notes`, `userVisibleProblem`, and
`backendRuntimeProblem` values must be real reviewer-entered evidence. Empty
strings and the placeholder `unknown` fail closed with the matching
`stage2_manual_*_missing` blocker; use `none` only where the reviewer has
actually inspected the scenario and has nothing to add.

The reviewer-facing report path is
`plans/main_chat_stage2_manual_dogfood_report.md`. The machine-readable report
path is `frontend/test-results/main-chat-stage2-manual-dogfood-report.json`.
The readiness gate should load the machine-readable artifact when present and
include its metadata-safe digest in `manualDogfood.artifactDigest`.
The artifact commit must be a known metadata-safe build commit; `unknown` and
`none`, fake labels, local/scripted/fixture/synthetic aliases, and
private-network-looking labels are treated as missing build provenance and must
fail closed with `stage2_manual_artifact_commit_missing`.
Each manual row's `buildCommit` must also be a known metadata-safe build
commit; `unknown`, `none`, and fake row commits must fail closed with
`stage2_manual_build_commit_missing`.

Manual reviewer records must not be fabricated by the Agent. If real manual
records are missing, readiness is `not_ready_for_limited_internal_trial`.
If manual evidence is present but not ready, the top-level report must include
`stage2_manual_dogfood_evidence_incomplete` even when an upstream adapter failed
to provide detailed manual blockers.
Machine-readable manual records may include optional P1 manual rows from
`S2-D25` through `S2-D27`, but unknown scenario ids must fail closed with
`stage2_manual_unknown_scenario_id`, and optional rows with a non-P1 severity
must fail closed with `stage2_manual_optional_scenarios_not_p1`. Optional P1
rows cannot satisfy the P0 reviewer-count requirement; required P0 rows with
fewer than two distinct reviewers must fail closed with
`stage2_manual_p0_reviewer_count_below_2`.

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
| `modelInvokedCount` | Number of credited scenarios proving model invocation. |
| `mainChatInvokedCount` | Number of credited scenarios proving ordinary Main Chat path. |
| `localOrMockCreditRejected` | Count of rejected local/mock/scripted/fixture evidence. |
| `artifactDigest` | Metadata-safe digest of the live report. |

Live `provider` and `model` identities must be known metadata-safe external
labels. `unknown` and `none` are placeholder evidence and must fail closed as
missing external provider or model identity.
Labels containing placeholder, local, mock, fixture, synthetic, scripted, or
loopback/private-network aliases, such as `none-provider`, must not receive
external-live credit.
Live scenario `taskSessionId` and `runId` values must be known metadata-safe
trace labels. `unknown` and `none` do not satisfy live trace evidence and must
fail closed with `stage2_live_trace_ids_missing`.
Live scenario `responsePreview` must be a bounded single-line provider response
trace; `unknown` and `none` do not satisfy live response evidence and must fail
closed with `stage2_live_response_preview_missing`.

The machine-readable live report artifact must include a metadata-safe build
commit. When the current build commit is known, the gate must reject stale live
artifacts from a different commit with
`stage2_live_artifact_current_commit_mismatch`; artifacts without a
metadata-safe known commit, including `unknown` or `none`, must produce
`stage2_live_artifact_commit_missing`. This applies both when loading an
existing artifact and when the opt-in live runner generates a fresh artifact in
the same command run.

All L2-L01 through L2-L10 must pass for `ready_for_limited_internal_trial`.
Missing credentials/network/provider returns named blockers and not-ready.
Scenarios with contradictory environment needs must use scenario-scoped setup:
for example, L2-L03 must run with web/network policy disabled, while L2-L04
must run with the governed web-read path enabled. A global config that makes one
of these scenarios impossible cannot receive readiness credit.
Live scenario required-evidence manifests must be exact to the Stage 2 scenario
contract: missing, duplicate, unsafe, or unrelated extra evidence labels fail
closed with `stage2_live_required_evidence_manifest_invalid`.
When Stage 2 adapts an upstream live harness report, it must preserve
`ready=false` as `stage2_live_harness_report_not_ready` and must not upgrade
that scenario into Stage 2 live credit. It must also preserve the upstream
required-evidence manifest contract; a missing, duplicate, unsafe, or extra
harness required-evidence label must fail closed with
`stage2_live_harness_required_evidence_manifest_invalid`. Any blocker produced
by the existing live-provider final-gate harness audit must remain a Stage 2
scenario blocker; Stage 2 must not credit a live harness report that the final
gate rejects.

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
- top-level report build commit is missing, `unknown`, `none`, or fake;
- hidden legacy fallback count is non-zero;
- silent durable write count is non-zero;
- local/mock/scripted/fixture evidence is credited as real browser/live proof.
