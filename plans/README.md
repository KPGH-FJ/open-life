# OpenLife Plans Document Governance

> Last updated: 2026-06-02
> Status: authoritative document index for Agents, W65 backend-only descriptor skeleton complete

This file prevents old planning documents from steering new Agent work. If two
documents disagree, use the precedence below and treat lower-priority stale text
as reference only.

## 1. Precedence

1. `AGENTS.md`
   - Project-wide Agent instructions, current constraints, and Tool Taxonomy.
2. `plans/README.md`
   - This authority map and current entry point.
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
   - Current implementation program and next development order.
4. `plans/lifemodel_governed_runtime_progress.md`
   - Compact W1-W65 completion/status index. This is not a second roadmap.
5. Hard governance baselines:
   - `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
   - `plans/openlife_react_beta_roadmap.md`
   - `plans/lifemodel_hs_mvp_task_specs.md`
   - `plans/lifemodel_hs_legacy_write_path_audit.md`
6. Scoped architecture/product baselines:
   - `plans/openlife_agent_framework_architecture.md`
   - `OpenLife_PRD_v2_Agent_Framework.md`
7. Current execution helpers:
   - `plans/openlife_development_plan.md`
   - `plans/openlife_codex_execution_playbook.md`
8. Historical/reference documents.
   - Useful for context, but never authoritative for current task order.

## 2. Current Position

Current latest status is **W65 backend-only descriptor skeleton complete**.
W64 validated the compressed W1-W63 authority/index entry. W65 adds only a pure
Rust mapper in `src-tauri/src/default_chat_adapter.rs` for a future controlled
adapter candidate contract; it adds no command, no frontend change, no Settings
surface, no runtime/model/tool call, no store write, and no routing change.

The next state remains controlled adapter contract work only if a separate task
explicitly asks for it and preserves default Chat `legacy_stream` until a
reviewed route change is implemented and verified.

Hard current constraints:

- default Chat remains `legacy_stream`.
- W19-W60 readiness/review/preview/gate outputs are not migration permission.
- W61-W63 are整理阶段, not default Chat migration.
- Ordinary `send_message` / `start_stream_message` must not call W19-W60
  command surfaces.
- Ordinary default Chat may call only the W49-W55 pure ordinary-entry guards /
  preflight, and those guards may only fail closed while preserving
  `legacy_stream`.
- W65 backend-only descriptor skeleton is metadata only and is not migration
  permission.

## 3. W1-W63 Compression Map

For the row-level structured index, use
`plans/lifemodel_governed_runtime_progress.md`. It lists every stage with:
stage id, name, status, command/surface type, read-only/write-disabled/
metadata-safe safety, default Chat impact, and next dependency.

| Range | Compressed meaning | Default Chat authority |
| --- | --- | --- |
| W1-W8 | Runtime, LifeModel, Strategy, and MultiStrategy foundations | No migration authority |
| W9-W18 | Non-default preview, preview audit, and migration gate evidence surfaces | No migration authority |
| W19-W23 | Controlled pilot eligibility, explicit pilot, reviewed promotion, source binding, promotion evidence | Explicit pilot/promotion only; ordinary Send unchanged |
| W24-W27 | Promotion readiness, migration plan draft, review evidence, implementation gate | Readiness/approval is discussion only, not migration permission |
| W28-W33 | Shadow run/review, cutover readiness, candidate adapter/review, candidate promotion readiness | Non-default write-disabled validation only |
| W34-W42 | Default Chat boundary, activation plan/review/gate, disabled routing, contract harness, dry run/review, implementation readiness | Read-only or non-default evidence only |
| W43-W48 | Controlled preview/review/readiness and cutover implementation plan/review/readiness | Non-default preview and planning only |
| W49-W55 | Route guard, invocation harness/plan/boundary, typed callsite contract, ordinary-entry preflight | Pure fail-closed guard only; route stays `legacy_stream` |
| W56-W60 | Ordinary-entry status, narrow discussion gate, narrow plan draft/review/approval readiness | Settings/status/planning only; ordinary entries must not call commands |
| W61-W64 | Docs/index整理, W1-W63 compression freeze, and authority validation | Docs only; no default Chat effect |
| W65 | Backend-only controlled adapter descriptor skeleton | Internal metadata-safe mapper only; no default Chat effect |

## 4. Current Authoritative Entry Points

| Document | Use for |
| --- | --- |
| `AGENTS.md` | Agent instructions, project context, Tool Taxonomy, and current hard constraints. |
| `plans/openlife_lifemodel_governed_agent_runtime.md` | Next implementation order and LifeModel-Governed Runtime program. |
| `plans/lifemodel_governed_runtime_progress.md` | W1-W63 structured status index and compressed guardrail map. |
| `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` | LifeModel-HS source-of-truth, proposal-first, privacy, materialized-view hard rules. |
| `plans/openlife_react_beta_roadmap.md` | ReAct execution seriousness, Beta gates, tool/action/audit baseline. |
| `plans/lifemodel_hs_mvp_task_specs.md` | Coding-ready LifeModel-HS MVP task specs. |
| `plans/lifemodel_hs_legacy_write_path_audit.md` | Direct-write convergence backlog and safety map. |
| `plans/openlife_development_plan.md` | Current execution route, already aligned to the LifeModel-Governed program. |
| `plans/openlife_codex_execution_playbook.md` | How to slice and verify individual Codex tasks. |

## 5. Historical Or Scoped Reference Documents

These files are useful context, but they are not current execution authority:

| Document | Status |
| --- | --- |
| `OpenLife_Final_PRD.md` | Historical long-form PRD. Do not use for current task order. |
| `plans/openlife_alpha_beta_plan.md` | Historical Alpha to Beta productization plan. |
| `plans/openlife_remaining_tasks_plan.md` | Historical sprint debt plan. Re-check code before using any item. |
| `plans/openlife_stabilization_and_spine_consolidation_plan.md` | Historical stabilization plan. |
| `plans/builder_life_model_design.md` | Builder UX/domain reference only; LifeModel-HS governance overrides direct-write assumptions. |
| `plans/frontend_experience_rebuild_plan.md` | Frontend UX reference only; current IA is governed by Agent/LifeModel-HS docs. |
| `plans/engineering_structure_notes.md` | Engineering history/reference only. |
| `architecture_diagram.md` | Snapshot diagram; verify against code and current program. |
| `BETA_CHECKLIST.md` | Historical checklist; current Beta/tool status is in AGENTS and roadmap. |
| `docs/ARCHITECTURE.md` | Quick architecture explainer; defer to current program for implementation order. |
| `docs/DEV_HANDOVER.md` | General handover; defer to this index and AGENTS for current Agent work. |

## 6. Tool Status Guardrail

`calendar.propose_event` and `email.propose_draft` are P1 proposal-only
governed executors. They create `ScheduledTask` / `DataExport` proposals and
must not perform real calendar writes, email sends, or `ExternalWriteAction`
fallback unless a future governed provider executor and tests are added.

`ExternalWriteAction` proposal creation must enforce pre-insert size limits and
payload minimization. This is a hard acceptance gate.

`run_multi_strategy_agent_preview` is a preview/beta command. Its W10 AgentRun
audit is a metadata-safe outer run; any ReAct inner run id is child metadata and
must not become the product trace's primary query id. Do not replace
`send_message` or the default Chat path just because the preview path works.

`check_runtime_migration_gate`, W19 pilot eligibility, W24/W27/W30/W33/W37/
W42/W45/W48/W57/W60 readiness gates, W25/W35/W46/W58 plan drafts, W26/W29/W32/
W36/W41/W44/W47/W59 review evidence, W28/W31/W40/W43 non-default run/preview
commands, and W56 status commands are not migration permission. They are
readiness, review, preview, draft, evidence, or status surfaces only.

Default `Send`, ordinary `send_message`, and ordinary `start_stream_message`
must remain on `legacy_stream`. They must not call W19-W60 command surfaces.
The only allowed ordinary-entry adapter code is W49-W55 pure guard/preflight
logic, which is read-only/pure, write-disabled, metadata-safe, side-effect-free,
and fail-closed.

## 7. Agent Rules

- Always read `AGENTS.md`, this file, and
  `plans/openlife_lifemodel_governed_agent_runtime.md` before starting a new
  architecture/runtime/LifeModel/tool task.
- Use `plans/lifemodel_governed_runtime_progress.md` for W1-W63 status, not as
  an implementation roadmap.
- Do not use historical plans to override current ordering, current Tool
  Taxonomy, or the default Chat `legacy_stream` boundary.
- If implementation changes tool status, proposal semantics, runtime authority,
  model routing, LifeModel source-of-truth, privacy boundaries, or default Chat
  routing, update the relevant docs in the same task and run the implementation
  verification gate.

## 8. Next Recommended Sequence

```text
W63 complete -> W64 authority compression validated -> W65 backend-only
descriptor skeleton complete -> prepare any future controlled adapter contract
work only if the task explicitly asks for implementation and preserves default
Chat legacy_stream until separately reviewed.
```

For docs-only index整理, `git diff --check` plus targeted `rg` validation is
enough. Run `make ci` when code, tests, package configuration, or runtime
behavior changes.
