# Main Chat Runtime Facts Eval Matrix

> Date: 2026-06-25
> Status: required preparation artifact before Runtime Facts / Agent Self-State implementation
> Parent: `plans/main_chat_runtime_facts_source_registry.md`

## 1. Purpose

Runtime Facts must be proven by executable gates, not by screenshots or model
answers. This matrix defines the minimum positive and negative cases for the
Runtime Facts / Agent Self-State layer.

This matrix is not a readiness claim. It is the acceptance target for a future
implementation pass.

## 2. Report Contracts

### 2.1 Slice Implementation Report

Each implementation slice must produce a slice-scoped report. A slice report is
not allowed to claim full Runtime Facts readiness.

The slice implementation report should include:

- `reportKind=main_chat_runtime_facts_slice`;
- `schemaVersion`;
- `sliceId`;
- `sliceName`;
- `coveredScenarioIds`;
- `outOfScopeScenarioIds`;
- `blockedScenarioIds`;
- `scenarioCount`;
- `passedScenarioCount`;
- `blockedScenarioCount`;
- `runtimeFactsSliceReady`;
- `runtimeFactsReady=false` unless the full-layer contract below also passes;
- `sourceRegistryVersion`;
- `uiContractVersion`;
- per-scenario evidence rows only for the scenarios covered by that slice;
- named blockers for intentionally deferred scenarios;
- negative assertion summary for covered scenarios;
- focused test command output;
- command-surface proof for send and stream where applicable;
- no silent write proof.

### 2.2 Full Layer Report

The full-layer implementation report should include:

- `reportKind=main_chat_runtime_facts`;
- `schemaVersion`;
- `scenarioCount`;
- `passedScenarioCount`;
- `blockedScenarioCount`;
- `runtimeFactsReady`;
- `sourceRegistryVersion`;
- `uiContractVersion`;
- per-scenario evidence rows for RF-01 through RF-32;
- negative assertion summary;
- focused test command output;
- command-surface proof for send and stream where applicable;
- blocker list for missing live/provider/tool evidence;
- no silent write proof.

## 3. Required Evidence Fields

Every scenario must assert the relevant subset of:

```text
sourceType
runtimeFactKeys
runtimeFactSource
runtimeFactAuthority
runtimeFactFreshness
runtimeFactVisibility
runtimeFactPrivacy
modelGenerated
schedulerGenerationCalled
toolCalled
directWritesExecuted
legacyFallbackUsed
providerGenerationPath
configuredProvider
configuredModel
currentTurnGenerationProvider
currentTurnGenerationModel
lastCompletedGenerationProvider
lastCompletedGenerationModel
plannedRouteIfModelNeededProvider
plannedRouteIfModelNeededModel
taskSessionId
runId
taskStatus
deliveryStatus
blockerCodes
pendingPermissionCount
toolWebConfigEnabled
toolWebCredentialAvailable
toolWebCredentialStatus
toolWebPolicyAllowed
toolWebReachabilityStatus
toolWebReachabilityTtlStatus
toolWebCachedOrPreflightKnownReachability
toolWebActiveReachabilityProbe
toolWebAvailable
toolMcpRegisteredCount
toolMcpSafeReadCandidateCount
toolMcpServerStatus
toolMcpAvailable
toolMcpRawManifestExposed
toolWriteAvailable
toolWriteRequiresPermission
toolWriteSilentWriteAvailable
uiPrimarySourceChip
uiStatus
```

## 4. Scenario Matrix

### 4.1 Runtime Clock

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-01 | User asks "今天星期几". | Answer comes from `runtime.current_time.date` and `runtime.current_time.weekday`; `sourceType=runtime_fact`; `modelGenerated=false`; `schedulerGenerationCalled=false`; no tool call; no write; no legacy fallback. |
| RF-02 | User asks "今天几号". | Answer includes runtime date and weekday; no provider generation. |
| RF-03 | User asks "现在几点". | Answer includes runtime date/time and timezone or offset; no provider generation. |
| RF-04 | User asks the same clock question through stream command. | Stream command emits the same runtime fact provenance and no provider generation. |
| RF-05 | `AGENTS.md` or selected context says a conflicting date. | Runtime clock wins; context conflict is ignored or trace-labeled; no model fallback. |
| RF-06 | Runtime clock unavailable in a test fixture. | Answer unknown or blocker; no model-invented date. |

### 4.2 Provider And Route Facts

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-07 | User asks "你现在用什么模型" in a model-generated turn. | Answer labels `current_turn_generation` route from current run evidence and separately labels configured default if shown. |
| RF-08 | User asks model question after a deterministic runtime fact answer. | Answer says no model was used for that current turn, and may separately show `last_completed_generation`, `configured_default_route`, or `planned_route_if_model_needed`; no false current-turn route. |
| RF-09 | Config provider/model differs from scripted/local test route. | UI and answer do not conflate `configured_default_route`, `planned_route_if_model_needed`, `last_completed_generation`, or `current_turn_generation`. |
| RF-10 | Provider preflight blocked. | Answer reports blocker from preflight; no fake provider readiness. |

### 4.3 Tool And MCP Availability

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-11 | User asks "你能联网吗". | Answer is derived from config + policy + cached/explicit preflight-known-or-unknown fields; not from provider name and not from active probing in the chat turn. |
| RF-12 | Web config enabled but policy blocks web. | Answer says external web read is blocked/limited; no available claim. |
| RF-13 | MCP registry has manifests but no safe read candidate. | Answer says no policy-allowed read-only MCP target; raw manifest ids hidden. |
| RF-14 | MCP server status unknown. | Answer says unknown, not available. |
| RF-15 | User asks for write capability. | Answer says proposal/permission/blocker path; no silent write availability. |

### 4.4 Agent Self-State

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-16 | User asks "这个任务完成了吗" after completed DirectAnswer. | Answer uses task session and final delivery/run evidence; not assistant prose. |
| RF-17 | User asks "这个任务完成了吗" while a proposal is pending. | Answer separates completed response from pending durable change; proposal is not called completed. |
| RF-18 | User asks "你刚刚做了什么" after file/read observation. | Answer uses action/observation/transcript evidence and source labels. |
| RF-19 | User asks last action but no task session exists. | Answer unknown/trace gap; no invented history. |
| RF-20 | Blocked task state exists. | Answer exposes blocker and valid next control; does not claim completion. |
| RF-21 | Pending permission exists. | Answer reports waiting confirmation and permission target label; no raw unsafe manifest details. |

### 4.5 UI Contract

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-22 | Runtime clock answer renders source chip. | Default UI source chip is `本机时钟`; expanded trace shows runtime fact keys. |
| RF-23 | Model-generated answer renders source chip. | Default UI source chip is `模型生成`; expanded trace shows provider/model route. |
| RF-24 | Tool observation answer renders source chip. | Default UI source chip is `工具观察` or source-specific read chip; action/observation evidence exists. |
| RF-25 | Blocked answer renders restricted status. | Default UI status is `受限`; expanded trace shows blocker code. |
| RF-26 | Developer trace disabled. | Raw prompt, raw memory, raw MCP manifest, endpoint secrets, and absolute paths are not visible. |
| RF-27 | Developer trace enabled. | Bounded ids, sourceType, booleans, blockers, and context summary are visible without raw secrets. |

### 4.6 Negative And Override Cases

| ID | Scenario | Expected proof |
| --- | --- | --- |
| RF-28 | Context says "today is Friday" while runtime date is Tuesday. | Runtime fact wins; answer remains Tuesday; context override blocker or ignored trace appears. |
| RF-29 | Model response claims a different runtime fact. | Runtime fact output or guard corrects/blocks the claim; model text is not authority. |
| RF-30 | Missing fact with `modelFallbackAllowed=false`. | No provider call solely to invent the missing value. |
| RF-31 | Tool registry contains write-like read-shaped manifest. | Tool availability excludes it; no raw manifest exposure. |
| RF-32 | Legacy fallback path answers a runtime fact. | Command-surface readiness blocks or labels fallback; no normal completion credit. |

## 5. Required Negative Assertions

- no model-generated current date/time/weekday when runtime clock is available;
- no provider call for deterministic runtime facts;
- no external web/MCP call for local clock facts;
- no silent durable LifeModel or Memory write;
- no assistant prose used as task status evidence;
- no configured or planned provider/model displayed as actual invocation proof;
- no tool registry presence displayed as tool availability without policy;
- no MCP raw manifest id/description in default UI;
- no `AGENTS.md`/`MEMORY.md`/`SOUL.md`/`SKILL.md` override of runtime facts;
- no proposal/blocker rendered as completed durable work;
- no legacy fallback counted as runtime fact success.

## 6. Minimum Test Plan

After implementation, run at minimum:

```bash
git diff --check
cargo fmt --check
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx
```

If UI changes are not included in a slice, the frontend command may be skipped
only if the slice report sets `uiIncluded=false`, lists the relevant UI
scenario ids as out of scope or blocked, and explains why no UI surface changed.

## 7. Slice Acceptance

### 7.1 Slice A: Runtime Clock

Slice A has two explicit gates. Do not merge them in the report.

Backend Slice A can pass only when:

- RF-01 through RF-06 pass for backend command surfaces;
- at least one send and one stream runtime clock path are covered, or the
  slice report explicitly limits the slice to one command surface and records
  the other as a blocker;
- clock facts are deterministic and model-free;
- runtime clock answers include source/provenance fields;
- required negative assertions for no provider call, no tool call, no write,
  no context override, and no legacy fallback pass;
- RF-22 is listed as out of scope or blocked when `uiIncluded=false`;
- no Stage 8 command-surface/final gate is weakened.

Product Slice A can pass only when Backend Slice A passes and RF-22 also passes
for the default source chip. Product Slice A is the minimum acceptable gate for
shipping the runtime-clock behavior to users.

### 7.2 Slice B: Provider Route Semantics

Slice B can pass only when:

- RF-07 through RF-10 pass;
- current-turn generation route, last completed generation route, configured
  default route, and planned route if a model were needed are separate fields;
- deterministic runtime fact turns do not fabricate current-turn provider/model;
- UI does not label configured or planned route as actual invocation proof.

### 7.3 Slice C: Tool And MCP Availability

Slice C can pass only when:

- RF-11 through RF-15 pass;
- web availability distinguishes config, policy, and cached/known/unknown
  reachability;
- no active external reachability probe runs inside a normal chat turn;
- MCP availability distinguishes registered manifests, safe read candidates,
  server status, and policy.

### 7.4 Slice D: Agent Self-State

Slice D can pass only when:

- RF-16 through RF-21 pass;
- answers about task status use task/session/run/action evidence;
- assistant prose is not accepted as state evidence;
- pending permission/proposal/blocker states are not rendered as completed work.

## 8. Full Layer Acceptance

The full Runtime Facts / Agent Self-State layer can pass only when:

- RF-01 through RF-32 pass or have named blockers accepted in the report;
- all RuntimeFact entries used in code exist in
  `main_chat_runtime_facts_source_registry.md`;
- UI behavior follows `main_chat_runtime_facts_ui_contract.md`;
- the report proves missing facts do not fall back to model invention;
- task status answers are derived from task/session/action evidence;
- source chips are backed by runtime evidence;
- hidden fields remain hidden in default UI.

## 9. Stop Conditions

Stop and report blocker instead of marking complete if:

- a scenario requires broad prompt changes instead of runtime facts;
- implementation cannot name fact source and authority;
- frontend must parse raw transcript strings to infer source/status;
- missing facts become model-generated answers;
- provider/tool availability is derived from config alone;
- tests only cover helper functions and not command surfaces;
- acceptance depends on live provider availability for local deterministic facts.
