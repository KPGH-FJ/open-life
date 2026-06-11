# Main Chat Agent Execution v1 Stabilization Goal Spec

> Date: 2026-06-09
> Status: active stabilization / acceptance-blocker remediation spec; not complete
> Baseline commit: `d8e415f` (`checkpoint: main chat agent execution v1 partial gate`)
> Scope: stabilize and make Main Chat Agent Execution v1 honestly verifiable

> 2026-06-09 progress note: Stage 2, Stage 4, Stage 5, Stage 6, Stage 7, and
> Stage 8 now have focused implementation slices. Final-gate aggregation/evidence
> normalization/blocker derivation plus live-provider required-evidence and
> blocked/completed report construction now lives in
> `src-tauri/src/main_chat_final_gate.rs` and the final acceptance runner calls
> it. Command-surface final evidence now runs through production state-level
> send/stream executors; external live-provider harness evidence remains
> opt-in and unexecuted in the current environment, but the final runner now
> has a real opt-in suite that returns four scenario-level blocker reports
> without invocation when preflight fails.
> A real non-default Tauri command,
> `run_main_chat_agent_execution_v1_final_acceptance_gate`, now calls the same
> aggregation with core runtime eval plus current state/scheduler live-provider preflight,
> does not invoke external providers by default or app-store writes, and fails
> closed with live evidence blockers when those proofs are absent. With explicit
> live opt-in, it uses isolated eval AppState instances to run DirectAnswer, web
> AgentLoop, registered MCP AgentLoop, and MCP ToolPermission proposal harness
> scenarios through the ordinary Main Chat path.
> Command-surface eval case matrix, scenario state setup, prompt/session-id
> mapping, case assertion/no-silent-write interpretation, report shape,
> coverage math, and acceptance evidence normalization now live in
> `src-tauri/src/main_chat_command_surface_eval.rs`; the final command now
> runs all 24 local send/stream command-surface cases on an isolated eval
> AppState, using `send_message_with_state` for send cases and
> `start_stream_message_governed_eval_with_state` for stream cases. Isolated Main Chat eval
> state construction also moved into `src-tauri/src/main_chat_eval_state.rs`, so
> command-surface and live harness state setup no longer depends on a
> `#[cfg(test)]` state factory. Live-provider harness opt-in, suite execution,
> ordinary `send_message` invocation, and report extraction now live in
> `src-tauri/src/main_chat_live_provider_harness.rs` instead of `src-tauri/src/lib.rs`.
> Main Chat task-control command state plus resume/cancel/retry/replay helpers
> now live in `src-tauri/src/main_chat_task_controls.rs`, with Tauri command
> registration left in `src-tauri/src/lib.rs`.
> Main Chat task session, transcript, and action-queue runtime support helpers
> now live in `src-tauri/src/main_chat_runtime_support.rs`.
> Main Chat generation support helpers for chat persistence, vector persistence,
> AgentRun finalization, non-stream fallback generation, provider endpoint
> classification, and metadata-safe preview text now live in
> `src-tauri/src/main_chat_generation_support.rs`.
> Main Chat ReAct tool-selection plan/candidate helpers now live in
> `src-tauri/src/main_chat_react_tool_selection.rs`.
> Main Chat ReAct AgentLoop attempt execution, runtime helper types, follow-up
> synthesis, action-to-tool-call conversion, and tool-call/blocker metadata helpers now live in
> `src-tauri/src/main_chat_react_runtime.rs`.
> Main Chat ReAct ActionExecutor-backed fallback execution now lives in
> `src-tauri/src/main_chat_react_execution.rs`.
> Main Chat proposal and ToolPermission proposal support helpers now live in
> `src-tauri/src/main_chat_proposal_support.rs`.
> Main Chat HS runtime packet/topic/tool-requirement helpers now live in
> `src-tauri/src/main_chat_hs_runtime.rs`.
> Main Chat context compilation and selected-skill id sanitization now live with
> the bounded knowledge-format loader in
> `src-tauri/src/main_chat_context_loader.rs`, leaving ordinary send/stream
> call sites in `src-tauri/src/lib.rs` to call that focused module.
> Main Chat ReAct AgentLoop guidance now declares a metadata-safe tool-candidate
> contract, configures `AgentLoopConfig` `toolset_allowlist` from the governed
> candidate targets, records candidate count/ids/allowlist/model-selected match
> metadata in transcript evidence, and supports a bounded multi-candidate
> registered read-only manifest set for generic MCP read requests. The generic
> MCP candidate predicate now also excludes high-risk, critical,
> confirmation-required, and write-like read-shaped manifests before exposing
> candidates to the model. Generic MCP candidates now also receive deterministic
> capability/name/tag ranking, and the metadata-safe candidate contract includes
> candidate rank, source, capability digest, and match reason evidence. This is
> local deterministic ranking evidence, not yet the full provider-backed /
> model-ranked manifest/capability path. Main Chat
> workspace file read target resolution also moved from a small hardcoded
> filename list to a workspace-root resolver that accepts explicit relative
> paths, canonicalizes readable targets, blocks traversal/outside-workspace
> reads, and keeps metadata traces to a relative label plus canonical path.
> Main Chat context assembly now uses a controlled knowledge-format loader for
> bounded `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, and selected
> `SKILL.md` surfaces from the workspace/configured root; `SKILL.md` content is
> gated by sanitized selected skill id, and ordinary send/stream command
> surfaces plus the frontend Tauri wrappers can now pass an optional
> `selectedSkillId`; Chat composer also exposes an explicit manual `SKILL.md`
> context field that carries the selected skill id through ordinary stream
> payloads without calling Skill Runtime commands. The existing 24-case
> send/stream command-surface gate remains green. This does not complete the
> Goal: real final/live-provider acceptance, remaining external live-provider
> harness evidence, broader provider-backed/model-ranked manifest/capability
> selection, and further module cleanup of other Main Chat runtime/strategy code
> remain blockers.

## 1. CLI Goal-Mode Short Instruction

Use this short instruction when starting CLI Goal mode. The detailed work order,
constraints, and acceptance criteria are in this document.

```text
Read AGENTS.md, plans/README.md, and
plans/main_chat_agent_v1_stabilization_goal_spec.md.

Execute the stabilization Goal exactly as specified there.
Do not restart the previous broad Main Chat migration Goal.
Fix the current acceptance blockers for Main Chat Agent Execution v1:
real auditable final gate, live-provider evidence/preflight, test-only harness
leakage, ReAct tool-selection boundary, safe workspace file read, controlled
knowledge-format context surfaces, and src-tauri/src/lib.rs module cleanup.

Do not claim complete unless the final gate returns ready=true with real
evidence. If live credentials/network/MCP are unavailable, return blocked with
exact blockers. Do not overclaim. Do not silently write durable user truth,
memory, files, external provider/plugin state, or dangerous shell.
```

## 2. Purpose

This is not a continuation of the previous broad Main Chat migration Goal. That
Goal produced meaningful partial progress and was checkpointed, but review found
that Main Chat Agent Execution v1 is still blocked by acceptance and
architecture issues.

This stabilization Goal exists to make the current implementation honestly
verifiable before more capability expansion continues.

The target is:

- a real final acceptance gate that can be audited outside hidden test-only
  helpers;
- exact ready/blocker reporting;
- no scripted, fixture, or local HTTP proof counted as external live-provider
  completion;
- less accumulation in `src-tauri/src/lib.rs`;
- enough ReAct/file/knowledge-format hardening to avoid building on weak
  scaffolding.

## 3. Non-Goals

Do not use this Goal to expand the product roadmap.

Out of scope:

- broad PlanExecute scenario expansion;
- full Skills Hub;
- full long-term memory product UI;
- proactive agent runtime;
- autonomous self-evolution;
- complete OpenClaw/Codex-level tool ecosystem;
- Beta declaration;
- large frontend redesign;
- broad refactors unrelated to the blockers below.

If a blocker requires follow-up work beyond this scope, document it as a next
Goal instead of absorbing it here.

## 4. Required Context To Read First

Before editing code, read:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/main_chat_agent_v1_stabilization_goal_spec.md`
4. `plans/main_chat_agent_migration_v1_goal_spec.md`
5. `plans/openlife_lifemodel_governed_agent_runtime.md`
6. `plans/openlife_agent_framework_architecture.md`

The previous migration spec remains the v1 capability target and audit trail.
This stabilization spec controls the next Goal-mode run.

## 5. Current Known Blockers

The Goal starts from these known facts:

1. Core runtime eval still reports `finalCompletionReady=false`.
2. Runtime live-provider coverage is zero in normal CI:
   - `liveProviderGenerationCoverage=0`
   - `liveProviderWebMcpAgentLoopCoverage=0`
   - `liveProviderWebAgentLoopCoverage=0`
   - `liveProviderMcpAgentLoopCoverage=0`
   - `liveProviderProposalPermissionCoverage=0`
3. `run_main_chat_agent_execution_v1_eval_gate` is a non-default command, but
   its current command-surface and live evidence inputs are partial/fail-closed.
4. The 24-case command-surface gate and final acceptance runner have strong
   test coverage, but too much reusable acceptance logic is still hidden inside
   `#[cfg(test)]`.
5. External live-provider tests are opt-in/ignored and were not executed in the
   checkpoint environment.
6. Local HTTP provider proof only proves HTTP provider plumbing. It must not be
   credited as external live-provider completion.
7. ReAct execution still starts from heuristic planned-action routing. It now
   exposes a metadata-safe governed candidate contract, enforces the candidate
   targets through `AgentLoopConfig.toolset_allowlist`, and can let AgentLoop
   select one registered read-only manifest from a bounded generic MCP candidate
   set. The candidate filter excludes high-risk, critical,
   confirmation-required, and write-like read-shaped manifests before model
   selection, but still lacks full provider-backed/model-ranked
   manifest/capability selection evidence.
8. Workspace file read target selection is too hardcoded.
9. Knowledge formats (`SOUL.md`, `USER.md`, `MEMORY.md`, `SKILL.md`,
   `AGENTS.md`) now have a controlled bounded loader for workspace/configured
   roots; ordinary send/stream command surfaces can carry a sanitized optional
   selected skill id; and Chat composer has an explicit manual `SKILL.md`
   context field that supplies that id without enumerating Skill Runtime
   commands.
10. `src-tauri/src/lib.rs` is too large and remains the dumping ground for Main
    Chat runtime/harness code.

## 6. Non-Negotiable Constraints

- Do not claim Main Chat Agent Execution v1 is complete unless the final
  acceptance gate returns `ready=true` with real evidence.
- Do not count scripted, fixture, deterministic, or local HTTP provider proof as
  external live-provider completion.
- Do not silently write durable LifeModel-HS truth.
- Do not silently write long-term Memory.
- Do not silently write files, calendar, email, external provider state,
  plugin state, tool permission state, or dangerous shell.
- Memory and LifeModel updates must remain proposal-first.
- External or sensitive writes must require confirmation/proposal.
- Workspace and knowledge-format files are bounded context surfaces only. They
  cannot override privacy, model-route, tool, or write policy.
- Full `SKILL.md` content may load only when that skill is selected.
- Fallback must remain visible and counted.
- Preserve existing passing behavior unless the behavior is explicitly wrong.
- Prefer extraction into focused modules over adding more Main Chat code to
  `src-tauri/src/lib.rs`.
- Do not commit or push unless the user explicitly asks after review.

## 7. Work Order

### Stage 1: Current-State Audit

Confirm the checkpoint state before implementing:

- current branch and latest commit;
- current `git status`;
- core runtime final readiness and blockers;
- command-surface coverage location;
- live-provider ignored/opt-in paths;
- file read resolver limitations;
- knowledge-format runtime support gap;
- `src-tauri/src/lib.rs` size and Main Chat concentration.

Do not change code until the blockers are concretely mapped.

### Stage 2: Real Final Acceptance Gate

Create a real non-default auditable final gate entry that aggregates:

- core 100-case runtime eval;
- send/stream command-surface eval;
- live-provider preflight and evidence;
- legacy fallback count;
- silent write count;
- required evidence;
- exact blockers.

The gate must:

- return structured `ready`, `status`, `blockers`, `requiredEvidence`, and
  coverage fields;
- fail closed;
- avoid provider invocation unless explicitly opted into live eval;
- avoid app-store writes;
- avoid serializing provider keys or raw private payloads.

Tests may call the same implementation, but the implementation must not depend
on hidden test-only helpers.

Progress so far: `run_main_chat_agent_execution_v1_final_acceptance_gate` is now
registered as a non-default Tauri command and uses the production
`main_chat_final_gate` aggregation. In the current no-live-opt-in path it runs
the core runtime eval, attaches metadata-safe live-provider preflight from the
current state/scheduler, executes all 24 local send/stream command-surface cases on isolated eval
state, avoids external provider invocation and app-store writes, reports
`migrationPermission=false`, and fails closed because full live-provider
evidence is absent. Isolated eval state construction is
production-safe through `main_chat_eval_state`, so this evidence does not mutate
real app stores. This is auditable Stage 2 progress, not final acceptance
readiness.

Additional progress: the same final acceptance runner now reads explicit live
opt-in from `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL` and has a state-level helper
for tests to pass that opt-in directly. When opted in, it runs the four
live-provider harness scenarios on isolated eval AppState instances through
ordinary `send_message`; if provider credentials or other preflight requirements
are missing, it returns four blocked scenario reports with
`mainChatInvoked=0`, `modelInvoked=0`, and no source app-store writes. This
proves the fail-closed opt-in path, not completed external live evidence.

### Stage 3: External Live-Provider Acceptance Path

Wire an opt-in live-provider acceptance path for:

- DirectAnswer generation;
- provider-backed web AgentLoop;
- provider-backed registered MCP AgentLoop;
- provider-backed ToolPermission proposal.

Rules:

- Missing key/network/provider/MCP must produce exact blockers.
- External provider runs must use ordinary `send_message` or the same Main Chat
  execution path, not a detached synthetic path.
- The final acceptance runner's explicit opt-in path must run all four live
  scenarios on isolated eval state and must not mutate the source app stores.
- Credited scenarios must have non-empty run id, task session id, response
  preview, completed status, no blockers, no silent writes, and no legacy
  fallback.
- Local HTTP provider proof remains useful plumbing evidence, but not external
  live-provider completion credit.

### Stage 4: Reduce Test-Only Acceptance Leakage

Move reusable harness logic out of `#[cfg(test)]` where it is needed by the real
non-default final gate. Keep test-only mock app construction in tests if needed,
but the acceptance aggregation, evidence normalization, and blocker derivation
must be reusable production/test code.

Avoid broad rewrites. Extract the smallest modules that make the gate auditable.

Progress so far: final-gate aggregation, live evidence normalization, command
surface live overlay, live-provider blocker derivation, required-evidence
normalization, and blocked/completed report construction moved into
`src-tauri/src/main_chat_final_gate.rs`, and the Tauri final acceptance runner
uses that module. Command-surface eval case matrix, report shape, coverage math,
and acceptance evidence normalization moved into
`src-tauri/src/main_chat_command_surface_eval.rs`. Isolated eval AppState
construction moved into `src-tauri/src/main_chat_eval_state.rs`, and command
surface/live harness tests now use it directly. The final acceptance runner now
executes all 24 local send/stream command-surface cases through production
state-level executors instead of Tauri mock IPC, and the live harness suite now
has a production state-level opt-in path instead of only blocked/completed report
synthesis. Live-provider harness execution moved into
`src-tauri/src/main_chat_live_provider_harness.rs`, so the test module no longer
keeps a duplicate mock-IPC harness implementation. Remaining Stage 4 work is
limited to external live-provider harness evidence that still requires explicit
opt-in and real provider/MCP/network proof.

### Stage 5: ReAct Tool-Selection Boundary

Improve the current heuristic planned-action path into a governed tool-selection
boundary:

- assemble allowed tool/capability candidates;
- expose those candidates to the model in a bounded tool-selection prompt or
  contract;
- allow the model to select only from allowed candidates;
- apply `ExecutionPolicy` after selection;
- reject invalid, unknown, write-like, or unauthorized tool calls;
- keep fallback visible when model output is invalid or unsupported;
- preserve ActionQueue and transcript evidence.

Do not remove the governed fallback path until the new path is proven.

Progress so far: the current runtime declares metadata-safe allowed candidates
in the AgentLoop system message, enforces selected targets through
`toolset_allowlist`, records candidate evidence in transcripts, supports a
bounded generic MCP read manifest candidate set, ranks generic MCP candidates by
deterministic query/manifest capability, name, description, and tag matches, and
exposes candidate rank/source/capability digest/match reason evidence without
raw manifest payloads. It also excludes high-risk / critical /
confirmation-required / write-like manifests from that model-selectable set.
Remaining Stage 5 work is real provider-backed/model-ranked manifest and
capability selection before execution policy application.

### Stage 6: Safe Workspace File Read

Replace the hardcoded file target list with a workspace-scoped safe resolver:

- accept explicit relative paths inside the workspace;
- canonicalize and block path traversal/outside-workspace reads;
- preserve metadata-safe traces;
- keep read-only execution automatic only when policy allows;
- require proposal/confirmation for writes.

### Stage 7: Controlled Knowledge-Format Context Surfaces

Implement read-only, bounded context support for:

- workspace `AGENTS.md`;
- global or configured `SOUL.md`;
- global or configured `memories/USER.md`;
- global or configured `memories/MEMORY.md`;
- global and workspace `skills/<skill>/SKILL.md`.

Rules:

- These files are context surfaces, not canonical truth.
- User edits may become evidence/proposal candidates only through governed
  lifecycle.
- Raw memory snippets and full LifeModel YAML must not be trusted by default.
- Full `SKILL.md` instructions load only for the selected skill.
- Context selection must be token bounded and traceable by digest/source id, not
  by raw private content in metadata reports.

Progress so far: Main Chat context assembly now calls a controlled
knowledge-format loader that reads bounded workspace/configured `AGENTS.md`,
`SOUL.md`, root and `memories/` `USER.md` / `MEMORY.md`, and
`skills/<selected>/SKILL.md` only for a sanitized selected skill id. Ordinary
send/stream command surfaces and frontend Tauri wrappers now accept and pass an
optional selected skill id when one exists. Chat composer now exposes an
explicit manual `SKILL.md` context field for that selected skill id and carries
it through the ordinary stream command payload. The async Main Chat context
compiler and selected-skill sanitizer now live in
`src-tauri/src/main_chat_context_loader.rs` beside the bounded file loader. A
fuller Skills Hub/discovery selector remains future productization, not a Main
Chat v1 completion blocker.

### Stage 8: Module Cleanup

Reduce new Main Chat centralization in `src-tauri/src/lib.rs`.

Extract focused modules where practical, such as:

- Main Chat final gate / acceptance report;
- command-surface eval support;
- live-provider eval harness support;
- Main Chat task controls;
- workspace file resolver;
- knowledge-format context loader.

This stage is stabilization, not cosmetic cleanup. Do not rewrite unrelated
Tauri commands.

Progress so far: focused modules now exist for final-gate aggregation,
command-surface eval/report support, isolated eval state, live-provider harness
execution, Main Chat task-control commands, ReAct tool-selection plan/candidate
helpers, generation support helpers, ReAct AgentLoop attempt execution / runtime helper types / follow-up
synthesis / action-to-tool-call conversion / tool-call metadata helpers, ReAct ActionExecutor-backed fallback execution, HS runtime
packet/topic/tool-requirement helpers, workspace file resolution,
task-session/transcript/action-queue runtime support helpers,
proposal/ToolPermission proposal support helpers, and knowledge-format context
loading/compilation.
Remaining Stage 8 work is further extraction of other Main Chat runtime/strategy
code without broad Tauri rewrites.

### Stage 9: Documentation Sync

Update:

- `AGENTS.md`
- `plans/README.md`
- `plans/main_chat_agent_v1_stabilization_goal_spec.md`
- `plans/main_chat_agent_migration_v1_goal_spec.md` only if the audit trail
  needs a new status note
- `README.md` if user-facing status changes

Docs must state either:

- v1 complete with the exact final gate evidence; or
- v1 blocked with exact remaining blockers.

No ambiguous "ready" language.

## 8. Completion Criteria

This Goal is complete only if all are true:

1. A real auditable non-default final gate exists.
2. The final gate aggregates runtime, command-surface, and live-provider
   evidence.
3. The final gate fails closed without live opt-in/evidence.
4. Fixture/scripted/local HTTP evidence cannot satisfy external live-provider
   requirements.
5. Command-surface evidence is included and not only test-only.
6. No silent writes are detected.
7. Legacy fallback is visible and counted.
8. External live-provider evidence is either credited from real completed runs
   or exact blockers are reported.
9. ReAct tool selection is materially less heuristic and remains governed by
   manifest/capability candidates plus `ExecutionPolicy`.
10. Workspace file read is safe and no longer hardcoded to a few filenames.
11. Controlled knowledge-format surfaces are implemented as bounded context.
12. New Main Chat logic is materially less concentrated in
    `src-tauri/src/lib.rs`.
13. Docs accurately state complete or blocked based on actual gate output.

If any of these are not true, the final response must mark the Goal blocked and
list remaining blockers.

## 9. Required Verification

Run and report:

```bash
git diff --check
cargo check
cargo test -p openlife-core agent::tests::main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
cargo test -p openlife-tauri final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_live_provider_eval_harness_executes_local_http_provider_without_external_live_credit -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend test -- --run ChatPage.test.tsx ProposalReviewPage.test.tsx tauri.test.ts AgentStage.test.tsx
```

Also run the new real final acceptance command/runner and report:

- `ready`
- `status`
- blockers
- required evidence
- runtime case count
- command-surface case count
- live-provider attempted count
- live-provider credited count
- silent write count
- legacy fallback count

If external live-provider opt-in is available, also run the real external live
provider scenarios and report the evidence. If it is unavailable, report the
preflight blockers and do not claim completion.

## 10. Final Response Requirements

The final response must lead with one of:

- `Ready`: final gate returned `ready=true` with real evidence.
- `Blocked`: stabilization improved the system but final completion is blocked.

Then include:

- remaining blockers;
- tests and commands run;
- files changed;
- final gate summary;
- whether Main Chat Agent Execution v1 can be considered complete;
- whether follow-up capability expansion can start.

Do not overclaim. Do not hide skipped or ignored live-provider runs.
