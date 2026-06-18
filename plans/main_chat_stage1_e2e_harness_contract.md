# Main Chat Stage 1 E2E Harness Contract

> Date: 2026-06-18
> Scope: end-to-end dogfood harness design
> Status: preparation artifact

## 1. Purpose

Stage 1 needs a harness that proves user-visible agent behavior, not only
backend readiness. The harness must run through ordinary Main Chat entry points
and inspect runtime-backed UI states.

## 2. Harness Layers

### 2.1 Rust/Tauri command E2E

Purpose: prove ordinary app command paths create the right runtime evidence.

Required paths:

- `send_message`;
- `start_stream_message`;
- task controls: resume, retry, cancel;
- proposal controls: accept, reject, defer;
- memory rollback;
- plan controls: confirm, edit, execute, skip, cancel, review;
- event replay;
- state snapshot read.

### 2.2 Frontend integration tests

Purpose: prove typed payloads render correctly in `ChatPage` and
`AgentControlPlane`.

Required assertions:

- task frame appears for work-like prompts;
- DirectAnswer stays compact;
- actions and observations render only when runtime evidence exists;
- blockers render with valid next controls;
- proposals render with Review Center path;
- memory rollback appears only for materialized memory;
- plan controls are enabled only with plan session and revision;
- final delivery sections are separated.

### 2.3 Browser-level Playwright E2E

Purpose: prove the actual user path works.

Environment requirement:

- Stage 1 implementation must make the browser E2E command self-contained by
  adding a Playwright `webServer` entry that starts Vite on the expected port, or
  by adding an equivalent checked-in script that starts and tears down the dev
  server deterministically.
- The readiness report must include `browserE2eEnvironmentReady`.
- If the browser E2E environment cannot start, the Stage 1 report must return a
  browser-specific `not_ready` blocker instead of treating command/component
  tests as a substitute.

Required actions:

- open Chat;
- load dogfood scenario set;
- type user prompt;
- send or stream message;
- wait for task/session state;
- inspect Agent Control Plane;
- click supported controls for seeded task-control scenarios;
- verify final delivery;
- export structured dogfood report.

Playwright should not be the only source of truth. It verifies product flow and
rendering. Runtime assertions must still validate records.

## 3. Report Shape

Target command/report names:

- command: `run_main_chat_agent_stage1_dogfood_gate`;
- report: `MainChatAgentStage1DogfoodReport`.

Required report fields:

- `reportKind`;
- `defaultReady`;
- `optInLiveReady`;
- `scenarioCount`;
- `defaultScenarioCount`;
- `defaultPassedCount`;
- `defaultFailedCount`;
- `taskSessionCreatedCount`;
- `ordinaryChatScenarioCount`;
- `seededTaskControlScenarioCount`;
- `uiVerifiedScenarioCount`;
- `finalDeliveryVerifiedScenarioCount`;
- `legacyFallbackCount`;
- `silentDurableWriteCount`;
- `fakeExecutionDetectedCount`;
- `externalLiveAttempted`;
- `externalLivePassedCount`;
- `browserE2eEnvironmentReady`;
- `browserE2eReportPath`;
- `blockers`;
- per-scenario evidence rows.

## 4. Per-Scenario Evidence Row

Each row must include:

- `scenarioId`;
- `scenarioType`;
- `entryPoint`;
- `scenarioPromptId`;
- `boundedPromptPreview`;
- `userPromptDigest`;
- `taskSessionId`;
- `runId`;
- `routeStrategy`;
- `expectedOutcome`;
- `actualOutcome`;
- `runtimeEvents`;
- `actions`;
- `observations`;
- `proposals`;
- `blockers`;
- `uiStates`;
- `finalDeliverySections`;
- `legacyFallbackUsed`;
- `silentDurableWriteDetected`;
- `fakeExecutionDetected`;
- `seedManifestDigest`;
- `liveProviderEvidence`;
- `passed`;
- `failureReason`.

`boundedPromptPreview` must be short, metadata-safe, and normalized to a single
line. It exists for auditability; `userPromptDigest` remains the canonical
prompt identity in reports.

## 5. Pass/Fail Semantics

A default scenario passes only when:

- expected route is observed;
- required runtime evidence exists;
- required UI states are rendered;
- final delivery sections match expected outcome;
- no forbidden evidence is present;
- no hidden legacy fallback;
- no silent durable write;
- no fake action, fake observation, fake web/MCP source, or assistant text used
  as state evidence.

Expected blockers can pass only if the named blocker is visible and the final
delivery does not claim completion.

## 6. Development Rule

Do not introduce a second task/session/event/memory/plan/proposal runtime for
Stage 1. If existing objects cannot express the required evidence, extend the
existing objects narrowly and update existing gates.
