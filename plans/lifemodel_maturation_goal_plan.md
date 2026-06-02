# LifeModel Maturation Loop End-to-End Goal Plan

> Last updated: 2026-06-02
> Status: W73 readiness report complete; W74 non-default maturation invocation next

This document is the entry point for the next Goal-mode development block after
W72. It does not authorize default Chat route migration, controlled adapter
executor attachment, or direct LifeModel writes.

## 1. Goal Boundary

Build one narrow, reversible LifeModel maturation loop end-to-end:

```text
Runtime / Chat / Feedback / Calibration source
  -> LifeEventDraft
  -> Signal / maturation candidate
  -> Evidence
  -> Governor
  -> Proposal
  -> user accept / reject / edit
  -> future RuntimeHSPacket-visible collaboration guidance
```

The first product domain is:

```text
low-energy / low-pressure planning preference
```

This is intentionally narrow. It is lower risk than identity, values,
relationships, health, finance, or long-term goal rewriting, and it can improve
future planning behavior without making LifeModel-HS truth mutable by raw model
output.

## 2. Current Baseline

Current completed default Chat adapter state:

- W65-W72 backend-only controlled adapter descriptor / contract / invocation /
  send proof / stream proof / attachment gate / disabled skeleton / binding
  integrity proof stack is complete.
- The proof stack is metadata-safe, no-run, no-write, no-stream, no-command,
  no-frontend, no-executor-attachment, no-route-cutover.
- Default Chat remains `legacy_stream`.
- Ordinary `send_message` / `start_stream_message` may only use the W49-W55
  pure ordinary-entry guard/preflight and must not call W67-W72 proof/skeleton
  code.

Current LifeModel maturation baseline:

- `openlife-core/src/agent/runtime_contract.rs`
  - `RuntimeOutput` can carry `life_event_candidates`.
  - `LifeEventDraft` exists as a draft-only candidate shape.
  - `RuntimeOutput::from_agent_loop_result` currently emits no candidates.
- `openlife-core/src/agent/maturation.rs`
  - `LifeModelMaturationService` converts LifeEventDrafts into proposal
    candidates.
  - `MaturationService::mature_runtime_output` creates governed Evidence and
    Proposal records from RuntimeOutput candidates.
  - High-risk LifeModel candidates remain proposal-first.
  - Raw user input / assistant output is not copied into evidence/report audit.
- `openlife-core/src/agent/evidence_store.rs`
  - EvidenceStore supports metadata-safe candidate evidence, source refs,
    proposal links, AgentRun links, weakening/archive/contradiction/tombstone
    lifecycle.
- `openlife-core/src/agent/governor.rs`
  - LifeModelGovernor already gates maturation candidates and blocks
    `proposal_only=false`.
- Existing maturation tests pass:
  - `cargo test -p openlife-core maturation_loop -- --nocapture`
  - `cargo test -p openlife-core runtime_output_life_event_candidates_do_not_persist_to_lifemodel_or_hs_stores -- --nocapture`

## 3. Non-Goals

Do not do any of the following in this Goal block unless a later explicit stage
changes the boundary:

- Do not migrate default Chat away from `legacy_stream`.
- Do not attach or run the controlled default Chat adapter executor.
- Do not call W67-W72 proof/skeleton functions from ordinary Chat entries.
- Do not make raw model output accepted LifeModel truth.
- Do not directly write LifeModel, MemoryStore, HeuristicStore active rules, or
  materialized YAML from a LifeEventDraft.
- Do not add broad identity/values/relationship/health/finance maturation in
  the first slice.
- Do not add a large UI/editor before the backend loop is proven.
- Do not use cloud extraction over raw sensitive content as an MVP dependency.

## 4. Hard Acceptance Rules

Every slice in this Goal block must preserve:

- Proposal-first: LifeModel and memory changes must be reviewable proposals
  before apply.
- Metadata safety: evidence, reports, run metadata, and debug dumps must not
  contain raw prompt, raw assistant output, raw memory context, tool payloads,
  secrets, emails, phone numbers, or file body content.
- Reversibility: accepted collaboration guidance must have lineage and an
  obvious path to weaken/archive/reject.
- Negative evidence: rejection should become useful evidence against repeated
  similar suggestions.
- Narrow behavior: the first behavior change may only affect low-energy /
  low-pressure planning guidance.
- Runtime visibility: when a collaboration rule affects future behavior, the
  RuntimeHSPacket or run trace must show metadata-safe selected guidance and
  evidence lineage.
- Default Chat isolation: no work in this block may change default Chat routing.

## 5. Recommended W-Slice Plan

### W73: Maturation End-to-End Readiness Report

Status: Done.

Goal:

Added a pure/read-only backend report that evaluates whether the current
MaturationService, EvidenceStore, ProposalStore, Governor, RuntimeOutput
candidate shape, and default Chat isolation are ready for a non-default
end-to-end maturation invocation.

Expected shape:

- Internal Rust report/evaluator first.
- Optional explicit read-only Tauri command only if it is useful for future
  Settings visibility; if added, it must be read-only and metadata-safe.
- No runtime/model/tool execution.
- No Evidence/Proposal/LifeModel/Memory/Heuristic writes.
- No default Chat route change.

Acceptance:

- Reports existing maturation primitives and blockers.
- Confirms default Chat remains isolated on `legacy_stream`.
- Confirms ordinary Chat entries do not call maturation readiness code.
- Fails closed when a synthetic candidate would be raw-content-bearing,
  unsupported, low confidence, proposal_only=false, or outside the low-energy
  / planning domain.

### W74: Non-Default Maturation Invocation Command

Status: Next.

Goal:

Add an explicit non-default command or backend harness that takes a
metadata-safe RuntimeOutput candidate and runs `MaturationService` into
EvidenceStore + ProposalStore.

Acceptance:

- Creates Evidence and pending Proposal only.
- Does not write LifeModel, MemoryStore, HeuristicStore active records, Chat
  messages, MCP audit, external write actions, or default Chat adapter records.
- Stores only candidate digest, source refs, proposal id, AgentRun id, risk,
  confidence, reason code, and metadata-safe summary.
- Rejects or redacts raw-content-bearing metadata.

### W75: Proposal Outcome Evidence Link

Goal:

Make proposal accept/reject/edit outcomes produce metadata-safe outcome
evidence for maturation candidates.

Acceptance:

- Accepted proposal outcome links back to evidence/proposal/source run lineage.
- Rejected proposal outcome creates negative evidence or opposing refs.
- Edited proposal outcome records edit metadata without storing raw reviewer
  text outside existing proposal semantics.
- Existing proposal apply semantics remain unchanged.

### W76: Low-Energy Collaboration Rule Candidate

Goal:

Aggregate repeated low-energy / low-pressure planning signals into a
reviewable collaboration rule proposal.

Acceptance:

- Requires repeated evidence or a clear threshold.
- Produces a proposal for a trial collaboration rule, not an active rule.
- The rule is narrow, reversible, and metadata-safe.
- Rejection weakens or blocks repeated similar rule suggestions.

### W77: Accepted Rule To RuntimeHSPacket Selection Proof

Goal:

After user acceptance, prove a narrow collaboration rule can be selected into a
future RuntimeHSPacket for planning tasks.

Acceptance:

- Only the low-energy planning domain is affected.
- Privacy policy cannot be relaxed by the rule.
- The selected guidance appears in metadata-safe HS packet audit fields.
- Non-planning tasks remain unaffected.

### W78: Run Trace Visibility

Goal:

Expose the selected collaboration rule and evidence lineage in run trace or a
read-only diagnostics surface.

Acceptance:

- User can see why OpenLife selected the guidance.
- Evidence/proposal/run lineage is visible by ids/digests/summaries only.
- No raw prompt/output/tool payload leakage.

## 6. First Agent Development Prompt

```text
你现在开发 W73: LifeModel Maturation End-to-End Readiness Report。

当前基线：
- W72 Default Chat Adapter Disabled Executor Skeleton Binding Integrity Report 已完成。
- default Chat 仍是 legacy_stream。
- 普通 send_message / start_stream_message 只能调用 W49-W55 ordinary-entry guard/preflight，不得调用 W67-W72，也不得调用本次 W73 readiness report。
- openlife-core 已有 RuntimeOutput.life_event_candidates、LifeEventDraft、MaturationService、LifeModelMaturationService、EvidenceStore、LifeModelGovernor、ProposalStore。
- 现有 maturation_loop tests 通过，但当前还没有端到端产品闭环，也没有普通 Chat 自动成熟化。

开发目标：
- 新增一个 read-only / metadata-safe 的 LifeModel maturation readiness report。
- 该 report 用于判断是否可以进入下一步 non-default maturation invocation slice。
- 不新增 default Chat route，不接普通 Chat，不运行 runtime/model/tool，不写 Evidence/Proposal/LifeModel/Memory/Heuristic/Chat/MCP audit/external write。
- 首选在 openlife-core 中实现 pure evaluator/report；如必须暴露 Tauri command，只能是显式 read-only diagnostics command，并补测试证明无副作用。

建议命名：
- LifeModelMaturationReadinessReport
- evaluate_lifemodel_maturation_readiness(...)
- ensure_lifemodel_maturation_readiness(...)

Report 至少包含：
- readinessReady / ready
- defaultChatUnchanged true
- ordinaryChatEntrypointUnchanged true
- runtimeOutputCandidateShapePresent
- maturationServicePresent
- evidenceStorePresent
- proposalStorePresent
- governorPresent
- proposalFirstRequired true
- directLifeModelWriteAllowed false
- directMemoryWriteAllowed false
- heuristicActivationAllowed false
- lowEnergyPlanningDomainOnly true
- metadataSafe true
- containsRawContent false
- sourceLineageRequired true
- negativeEvidenceRequiredForRejection true
- acceptedRuleRuntimePacketFutureOnly true
- blockingReasons
- nextAllowedStep = non_default_maturation_invocation

必须 fail closed 的场景：
- candidate metadata contains raw prompt / raw assistant output / raw memory context / tool payload / secret-like content。
- candidate type is outside low-energy / low-pressure planning domain。
- candidate confidence too low。
- candidate proposal_only=false。
- report would require direct LifeModel/Memory/Heuristic write。
- report assumes default Chat route migration or ordinary Chat auto-maturation。

测试要求：
- 新增 focused Rust tests。
- 覆盖 clean readiness。
- 覆盖 raw content fail closed。
- 覆盖 unsupported domain fail closed。
- 覆盖 low confidence fail closed。
- 覆盖 proposal_only=false fail closed。
- 覆盖 no direct LifeModel/Memory/Heuristic writes。
- 覆盖 ordinary send_message / start_stream_message 不调用 W73 readiness report。
- 覆盖 debug/report serialization 不包含 raw prompt、assistant output、memory context、tool payload、secret/email。

至少运行：
- cargo test -p openlife-core lifemodel_maturation_readiness -- --nocapture
- cargo test -p openlife-core maturation_loop -- --nocapture
- cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
- git diff --check

完成后不要提交、不要推送。只输出：
- 变更摘要
- 新增接口说明
- 测试结果
- 风险与后续建议
```

## 7. Goal-Mode Operating Rules

- Use one W-slice per Agent iteration.
- 验收通过后再提交推送。
- If a slice touches runtime behavior, run `make ci` before commit.
- If a slice is docs-only or pure internal report-only, run `git diff --check`
  plus targeted tests/rg checks.
- Update `AGENTS.md`, `plans/README.md`, and
  `plans/lifemodel_governed_runtime_progress.md` whenever runtime authority,
  LifeModel source-of-truth, proposal semantics, privacy boundaries, or default
  Chat routing assumptions change.
