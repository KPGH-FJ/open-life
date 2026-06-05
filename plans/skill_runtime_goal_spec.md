# Skill Runtime Beta Maturity Goal Spec

> Date: 2026-06-05
> Status: completed CLI Goal-mode implementation spec / audit trail for W150-W158
> Baseline: W149 Backend Completion Goal 8 complete; default Chat remains `legacy_stream`
> Scope: mature existing built-in / plugin-declared skills into a governed, metadata-safe Skill Runtime

## 1. Summary

This document is the completed CLI Goal-mode implementation spec / audit trail
for the W150-W158 block after W149.

OpenLife already has a small Skill MVP:

- `openlife-core/src/skills.rs` defines `SkillManifest`, `SkillRegistry`,
  `SkillJsonEnvelope`, proposal candidates, fail-soft JSON parsing, and built-in
  skills: `weekly_review`, `goal_breakdown`, and `memory_consolidation`.
- `src-tauri/src/commands/execution.rs` exposes `list_skills` and `run_skill`.
- `run_skill` currently loads LifeModel, creates a Skill task, runs
  `AgentRuntime`, calls the model, parses a JSON envelope, creates pending
  Review Center proposals, and writes an `AgentRun` with `kind=Skill`.
- Plugin manifests can declare skills, but plugin tools are still
  declarative-only in Beta.

Before this Goal, the Skill MVP was useful but not yet a full governed Skill
Runtime. W150-W158 adds first-class readiness, context contracts,
policy/privacy gates, proposal candidate governance, trace/read-model
stability, plugin boundaries, and product-safe status surfaces.

This Goal is explicitly not a default Chat route migration and not a general
plugin executor rollout.

## 2. Objective

Complete W150-W158: Skill Runtime Beta Maturity.

At the end of this Goal:

- Built-in skills have typed, metadata-safe runtime descriptors and readiness.
- Skill context assembly is bounded, privacy-aware, and traceable.
- Skill model execution respects LifeModel-HS privacy/model-route hard policy.
- Skill output envelopes are validated fail-soft without raw payload leakage in
  status/readiness reports.
- Skill proposal candidates are allowlisted, risk-classified, proposal-first,
  and linked back to the originating `AgentRun`.
- Plugin-declared skills are clearly classified as executable built-in,
  disabled/declarative-only, or blocked until a real safe executor boundary
  exists.
- Skill runs emit stable AgentRun action/observation trace metadata that Runs
  and Review Center can inspect safely.
- A non-default, read-only Skill Runtime status/readiness command can report
  Beta maturity without running a model/tool/store-write path.
- Docs and progress index identify Skill Runtime as implemented
  without granting default Chat migration permission.

## 3. Non-Negotiable Constraints

Do not change these invariants:

- Do not migrate default Chat.
- Do not replace ordinary `send_message` or `start_stream_message`.
- Do not call Skill Runtime readiness/status/final-gate helpers from ordinary
  Chat entrypoints.
- Do not treat Skill Runtime readiness, status, or Beta maturity as default
  Chat migration permission.
- Do not run model/tool/runtime calls from readiness/status commands.
- Do not create AgentRuns, Proposals, Evidence, Memory, LifeModel patches, MCP
  audit rows, external writes, or Chat messages from readiness/status commands.
- Do not directly write durable LifeModel-HS truth, Memory, files, calendar,
  email, external provider state, plugin state, or tool permission state from a
  skill. Skills may create Review Center proposals only through governed paths.
- Do not let accepted proposal review imply permission to send sensitive content
  to cloud model or cloud embedding providers.
- Do not mark plugin tools executable or plugin skills production executable
  unless this Goal adds a real executor boundary, governance report, tests, and
  docs. Otherwise plugin skills must remain disabled/declarative-only or
  explicitly model-only with no external/tool side effects.
- Do not store raw prompt, raw assistant output, raw LifeModel text, raw memory
  content, raw chat history, raw file contents, raw tool payload, raw proposal
  payload, or user PII in readiness/status/debug reports.
- Do not broaden Skill Runtime into proactive/background execution.
- Do not add broad UI/UX productization. Minimal wiring/tests are allowed only
  when command contracts change.
- Do not commit or push from the implementation Agent unless a human explicitly
  asks after review.

## 4. Required Context To Read First

The implementation Agent must read these files before editing code:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
4. `plans/lifemodel_governed_runtime_progress.md`
5. `plans/react_beta_execution_hardening_goal_spec.md`
6. `plans/openlife_react_beta_roadmap.md`
7. `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
8. `openlife-core/src/skills.rs`
9. `src-tauri/src/commands/execution.rs`
10. `openlife-core/src/agent/runtime.rs`
11. `openlife-core/src/agent/runtime_contract.rs`
12. `openlife-core/src/agent/hs_selector.rs`
13. `openlife-core/src/agent/model_router.rs`
14. `openlife-core/src/agent/proposal_store.rs`
15. `openlife-core/src/agent/backend_contract_freeze.rs`
16. `frontend/src/tauri.ts`
17. `frontend/src/test/mocks/tauri.ts`
18. `frontend/src/components/RunTracePanel.tsx`
19. `frontend/src/pages/ProposalReviewPage.tsx`

## 5. Stage Order

Implement W150-W158 in one sustained Goal run, keeping this internal order.

### W150 Skill Runtime Contract And Readiness

Add a pure core Skill Runtime readiness contract over the existing registry.

Required outcomes:

- A typed readiness report proves required built-in skills exist exactly once:
  `weekly_review`, `goal_breakdown`, `memory_consolidation`.
- Each skill descriptor reports only metadata-safe fields:
  id, name, source kind, input schema digest, output schema digest, proposal
  policy, required context ids, allowed tool ids/counts, execution budget, and
  capability flags.
- Readiness fails closed for duplicate ids, missing built-ins, invalid proposal
  policy, unsafe write budget, raw-content fields in descriptors, plugin skills
  marked executable without governance, or skills that imply direct writes.
- The report explicitly states:
  `default_chat_unchanged=true`, `migration_permission=false`,
  `runtime_execution_performed=false`, `model_call_performed=false`,
  `tool_call_performed=false`, and `business_writes_performed=false`.

Suggested files:

- `openlife-core/src/skills.rs`
- optional `openlife-core/src/skill_runtime.rs` if the contract grows large
- `openlife-core/src/lib.rs`

Acceptance tests:

- Missing required built-in blocks readiness.
- Plugin-declared skill without executor governance is disabled/declarative-only
  or blocked in readiness.
- Readiness output omits raw prompts, raw schemas beyond digests, raw LifeModel,
  raw memory, and raw user input.

### W151 Skill Context Assembly Contract

Add a bounded context assembler for Skill Runtime.

Required outcomes:

- Context assembly is typed and budgeted by manifest `required_context`.
- Supported context ids initially include:
  `life_model.summary`, `life_model.goals`, `life_model.state`, `agent_runs`,
  `memory`, and `chat_history`.
- Context outputs contain bounded summaries, counts, ids, digests, timestamps,
  and safe excerpts only when explicitly permitted by the runtime execution
  path. Readiness/status reports must remain raw-content-free.
- Missing context is represented as warnings, not panics.
- Context summary links into `AgentRun.context_summary` or a stable
  Skill-specific metadata-safe context report.
- Context assembly must not write stores.

Suggested files:

- `openlife-core/src/skills.rs`
- `src-tauri/src/commands/execution.rs`

Acceptance tests:

- Skill requiring memory receives bounded memory context and reports hit counts.
- Missing AgentRun store produces a warning and still runs fail-soft.
- Oversized input/context is truncated or summarized with digest/count metadata.

### W152 Skill Privacy, HS Packet, And Model Route Governance

Make Skill Runtime honor LifeModel-HS privacy/model-route boundaries.

Required outcomes:

- Skill task classification uses the same HS privacy topic/risk logic as other
  governed runtime paths.
- High/Critical privacy or HS LocalOnly skill runs select local `ollama` only
  or fail closed when no local model is available.
- Skill model execution attaches a metadata-safe HS audit / route trace to
  `AgentRun`.
- Skill Runtime does not consume accepted guidance by default unless this Goal
  explicitly introduces `RuntimeGuidanceConsumptionMode::ExplicitRuntime` for
  skill runs and tests it. Default mode must remain disabled.
- Skill Runtime must not bypass W141 ModelRouter hardening.

Suggested files:

- `src-tauri/src/commands/execution.rs`
- `src-tauri/src/lib.rs`
- `openlife-core/src/agent/runtime.rs`
- `openlife-core/src/agent/model_router.rs`

Acceptance tests:

- Sensitive skill input with cloud-only config fails closed or routes local,
  never cloud fallback.
- Non-sensitive input can route through existing model routing.
- Skill AgentRun records route/audit metadata without raw input or raw model
  output in status/readiness reports.

### W153 Skill Output Envelope And Trace Stability

Harden skill model output parsing and trace shape.

Required outcomes:

- Skill output must normalize into a stable envelope:
  summary, structured output metadata, proposal candidate metadata, warnings,
  parse status, validation status, and redaction status.
- Parse failure remains recoverable and records an AgentRun error phase such as
  `skill_json_parse`.
- Raw model output must not be stored in metadata-safe reports. If raw output is
  kept in an AgentRun action/observation for product inspection, it must be
  bounded and clearly classified as runtime payload, not status/readiness
  metadata.
- Action/observation trace includes a stable skill trace envelope similar in
  spirit to `react_trace`, using ids/digests/counts/status/type.

Suggested files:

- `openlife-core/src/skills.rs`
- `src-tauri/src/commands/execution.rs`
- `openlife-core/src/agent/types.rs`

Acceptance tests:

- Fenced JSON, direct JSON, and first-object extraction still work.
- Invalid JSON creates no proposals and produces completed-with-warnings
  metadata.
- Skill trace metadata omits raw LifeModel, raw memory, raw chat, and raw model
  output in readiness/status surfaces.

### W154 Proposal Candidate Governance

Make skill proposal creation governed and allowlisted.

Required outcomes:

- Proposal candidates are validated against a strict allowlist per skill.
- Proposal types allowed initially:
  `goal_update`, `state_update`, `memory_write`, `memory_archive`,
  `preference_update`, and `capability_update`.
- Affected paths are allowlisted by proposal type and skill id.
- Risk is classified deterministically. High-risk identity, values,
  relationships, health, finance, privacy, and long-term direction changes are
  either blocked or require explicit high-risk proposal metadata and must never
  be auto-applied.
- Every generated proposal has `ProposalSource::SkillRuntime`, source run id
  linkage, confidence clamping, and metadata-safe reason/summary.
- Unsupported/unsafe candidates are skipped with warnings; safe candidates
  still proceed.

Suggested files:

- `src-tauri/src/commands/execution.rs`
- `src-tauri/src/commands/proposal.rs`
- `openlife-core/src/agent/proposal_store.rs`

Acceptance tests:

- Unsafe proposal type is skipped and no proposal is created.
- Unsafe path is skipped and recorded as warning.
- Accepted skill proposal maps to `PatchSource::SkillRuntime` where supported
  and keeps proposal-first semantics.
- Memory proposals do not directly write Memory during skill run.

### W155 Plugin Skill Boundary And Manifest Authority

Make plugin-declared skills honest.

Required outcomes:

- Built-in skills and plugin-declared skills are classified separately.
- Plugin tools remain declarative-only unless a real safe executor is added in a
  later reviewed Goal.
- Plugin-declared skills must not inherit executable status merely because a
  plugin is enabled. They require an explicit governance classification:
  disabled/declarative-only, model-only/no-tools, or blocked.
- `list_skills` must expose enough metadata for the frontend to avoid implying
  plugin skills are production executable when they are not.
- Reload/enable/disable plugin flows keep SkillRegistry consistent without
  registering unsafe executable capabilities.

Suggested files:

- `openlife-core/src/plugins.rs`
- `openlife-core/src/skills.rs`
- `src-tauri/src/commands/execution.rs`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`

Acceptance tests:

- Enabling a plugin with declared skills does not make external/plugin tool
  execution possible.
- Plugin skill status appears disabled/declarative-only or model-only according
  to manifest/governance.
- `run_skill` blocks disabled/declarative-only plugin skills.

### W156 Skill Runtime Status Command

Add an explicit non-default read-only command.

Recommended command name:

- `get_skill_runtime_status`

Required outcomes:

- The command returns readiness, descriptor summaries, plugin boundary summary,
  proposal governance summary, privacy/model-route boundary summary, trace
  contract summary, and blockers.
- It performs no runtime/model/tool calls and no business writes.
- It is registered in Tauri, wrapped in TypeScript, mocked in frontend tests,
  and covered by Rust tests.
- Naming must not imply default Chat migration permission.

Suggested files:

- `src-tauri/src/commands/execution.rs`
- `src-tauri/src/lib.rs`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`

Acceptance tests:

- Command reports ready when all built-in skill runtime gates pass.
- Command reports blockers for unsafe plugin skills.
- Ordinary `send_message` and `start_stream_message` do not call this command or
  helper.

### W157 Runs / Review Center Skill Trace Integration

Harden existing surfaces enough to inspect Skill Runtime without a broad UI
redesign.

Required outcomes:

- Runs detail can identify `AgentTaskKind::Skill` runs and display skill id,
  status, warnings, generated proposal ids, and metadata-safe trace fields.
- Review Center can show `ProposalSource::SkillRuntime` clearly.
- Frontend wrappers and mocks match the final Tauri response shapes.
- No new large product surface is required in this Goal.

Suggested files:

- `frontend/src/tauri.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/pages/ProposalReviewPage.tsx`
- `frontend/src/test/mocks/tauri.ts`

Acceptance tests:

- Skill run trace renders warnings and generated proposal ids.
- Review Center displays Skill Runtime source without raw payload leakage.

### W158 Docs / Progress / Final Verification Sync

Synchronize authority docs after implementation.

Required outcomes:

- `plans/README.md` names this spec as completed audit trail only after the Goal
  actually passes.
- `plans/lifemodel_governed_runtime_progress.md` adds W150-W158 rows with
  accurate status and default Chat impact.
- `plans/openlife_lifemodel_governed_agent_runtime.md` marks Skill Runtime
  maturity according to the final state.
- `README.md` and `AGENTS.md` no longer describe Skill Runtime as incomplete if
  W150-W158 actually pass.
- Any remaining Beta blockers are precise and do not imply default Chat
  migration permission.

Acceptance tests:

- `rg` checks show no stale text claiming Goal 8 is next or Skill Runtime is
  still prepared-only after this Goal.
- Docs explicitly preserve default Chat `legacy_stream`.

## 6. Suggested Verification Commands

Run all of these before reporting completion:

```bash
cargo test -q -p openlife-core
cargo test -q -p openlife-tauri
cargo clippy -q --workspace -- -D warnings
cd frontend && corepack pnpm test
cd frontend && corepack pnpm run build
cd frontend && corepack pnpm run format:check
git diff --check
git status --short
```

Additional targeted checks:

```bash
rg -n "send_message_with_agent_loop|start_stream_message_with_agent_loop|run_skill|get_skill_runtime_status|SkillRuntime" src-tauri/src/lib.rs
rg -n "raw_output|life_model_json|recent_memory_json|chat_history_json" openlife-core/src src-tauri/src frontend/src
rg -n "skill_runtime_beta_not_complete|Skill Runtime" AGENTS.md README.md plans
```

Use the targeted checks as review aids, not as automatic pass/fail substitutes.

## 7. Completion Definition

The Goal is complete only when all are true:

- `weekly_review`, `goal_breakdown`, and `memory_consolidation` are governed
  Skill Runtime capabilities, not just prompt helpers.
- Skill context assembly is bounded, typed, and privacy-aware.
- Skill model route behavior respects HS LocalOnly / High / Critical privacy
  fail-closed semantics.
- Skill output parsing and validation are fail-soft.
- Skill proposal candidates are proposal-first, allowlisted, and linked to
  AgentRun/proposal records.
- Plugin skills are not misrepresented as executable side-effect capabilities.
- Readiness/status surfaces are metadata-safe and side-effect-free.
- Runs/Review Center can inspect skill runs/proposals safely.
- Docs/progress are synchronized.
- Default Chat remains `legacy_stream`.

## 8. CLI Goal Prompt

Use this prompt to start the implementation Goal:

```text
You are working in /Users/fujing/Desktop/偶来福.

Goal: complete W150-W158 Skill Runtime Beta Maturity according to
plans/skill_runtime_goal_spec.md.

Before editing, read:
- AGENTS.md
- plans/README.md
- plans/openlife_lifemodel_governed_agent_runtime.md
- plans/lifemodel_governed_runtime_progress.md
- plans/skill_runtime_goal_spec.md
- plans/react_beta_execution_hardening_goal_spec.md
- plans/openlife_react_beta_roadmap.md
- plans/adr/0013-lifemodel-hs-source-of-truth-governance.md
- openlife-core/src/skills.rs
- src-tauri/src/commands/execution.rs
- openlife-core/src/agent/runtime.rs
- openlife-core/src/agent/runtime_contract.rs
- openlife-core/src/agent/hs_selector.rs
- openlife-core/src/agent/model_router.rs
- frontend/src/tauri.ts
- frontend/src/test/mocks/tauri.ts

Hard constraints:
- Do not migrate default Chat.
- Do not replace ordinary send_message or start_stream_message.
- Do not call Skill Runtime readiness/status/final-gate helpers from ordinary
  Chat.
- Do not treat readiness/status as migration permission.
- Do not run model/tool/runtime calls or write stores from readiness/status
  commands.
- Do not directly write LifeModel/Memory/file/calendar/email/external/plugin
  state from skills. Skills create Review Center proposals only.
- Do not expose raw prompt, raw assistant output, raw LifeModel, raw memory,
  raw chat history, raw tool payload, raw proposal payload, or PII in
  readiness/status/debug reports.
- Plugin tools remain declarative-only unless a separately governed real
  executor is implemented. Plugin-declared skills must not be advertised as
  executable side-effect capability without governance.

Implementation order:
1. W150 Skill Runtime contract/readiness.
2. W151 bounded Skill context assembly.
3. W152 Skill privacy, HS packet, and model route governance.
4. W153 Skill output envelope and trace stability.
5. W154 proposal candidate governance.
6. W155 plugin skill boundary and manifest authority.
7. W156 non-default read-only Skill Runtime status command.
8. W157 Runs / Review Center skill trace integration.
9. W158 docs/progress/final verification sync.

Verification:
- cargo test -q -p openlife-core
- cargo test -q -p openlife-tauri
- cargo clippy -q --workspace -- -D warnings
- cd frontend && corepack pnpm test
- cd frontend && corepack pnpm run build
- cd frontend && corepack pnpm run format:check
- git diff --check
- git status --short

Do not commit or push. Report completion with changed files, tests run, known
residual blockers, and whether W150-W158 are complete.
```
