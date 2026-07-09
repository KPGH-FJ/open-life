# LifeModel / Memory / Evidence / Change Model

Status: Phase 1 product model proposal.
Scope: User-understandable model and Phase 2 read-model requirements only.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## User-readable Definitions

| Concept | User explanation | Classification |
| --- | --- | --- |
| LifeModel | OpenLife 对“你是谁、你在乎什么、你当前状态如何、你长期目标是什么”的结构化理解。 | `DESIGN_DECISION` |
| 记忆 | OpenLife 记住过的事实、事件、偏好、证据和候选更新。 | `DESIGN_DECISION` |
| 依据 | 某个理解或记忆来自哪里。 | `DESIGN_DECISION` |
| 变更 | OpenLife 准备如何更新它对你的理解。 | `DESIGN_DECISION` |

`VERIFIED_FACT`: The codebase has LifeModel, LifeModel patch/provenance structures, MemoryStore, MemoryGateway, EvidenceStore, LifeEventStore, and MemoryLifecycleStore primitives. Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`.

## Memory States

| State | User explanation |
| --- | --- |
| 候选记忆 | OpenLife 认为可能值得记住，但还没确认。 |
| 已确认记忆 | 用户确认过或系统可信写入。 |
| 已用于 LifeModel | 已影响长期理解。 |
| 已撤回 / 已过期 | 不再使用或被用户移除。 |

Classification: `DESIGN_DECISION`. These states are the first user-facing vocabulary for memory lifecycle.

## LifeModel Visible Concepts

| Concept | Default explanation | Status |
| --- | --- | --- |
| 当前理解 | OpenLife 当前展示给你的长期理解。 | `CANDIDATE` |
| 依据 | 当前理解的来源、记录或确认项。 | `CANDIDATE` |
| 待确认变更 | OpenLife 建议改变的内容，但还没有成为长期状态。 | `DESIGN_DECISION` |
| 已应用变更 | 已经过确认并写入长期状态的变更。 | `PHASE_2_REQUIRED` to validate materialization state. |
| 兼容/历史视图 | 为兼容旧数据或历史路径生成的视图。 | `ADVANCED_INSPECTOR` wording needed; not normal default copy. |

## Memory Lane Model

`CANDIDATE`: V2 should present memory lanes in ordinary language:

| Lane | User meaning | Review default |
| --- | --- | --- |
| 上下文 | 只用于当前任务或近期对话。 | Usually no review; not durable long-term truth. |
| 事件 | 发生过的具体事情。 | Review if sensitive/consequential. |
| 偏好 | 用户长期偏好、习惯、选择。 | Usually review before long-term use. |
| 规则 | 用户希望 OpenLife 以后遵守的做事方式。 | Review before durable use. |
| 依据 | 支撑某个记忆或 LifeModel 理解的记录。 | Preserve source refs; visibility depends on sensitivity. |
| LifeModel 真相 | 影响 OpenLife 长期理解的结构化状态。 | Review/materialization required unless a documented low-risk lane exists. |

`VERIFIED_FACT`: MemoryGateway separates turn context, episodic life event, semantic preference, procedural rule, evidence record, and canonical LifeModel truth. Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`.

`PHASE_2_REQUIRED`: The UI needs backend lane counts/status/provenance. Current MemorySearch is not enough as a product memory model.

## Evidence / Provenance Model

`DESIGN_DECISION`: Evidence must be visible as `依据` in normal product copy and deeper `来源与记录` in advanced inspection.

Required evidence fields for Phase 2:

- source surface/task;
- source time;
- source refs;
- user confirmation state;
- privacy/sensitivity summary;
- relation to memory item, proposal, or LifeModel change;
- audit refs.

`PHASE_2_REQUIRED`: Do not claim these fields are present in a single existing backend projection. Phase 2 must validate or implement them.

## Change Model

`DESIGN_DECISION`: Product copy must separate:

- observed context;
- candidate memory;
- pending review item;
- approved proposal;
- applied/materialized durable change;
- withdrawn/expired item;
- rollback/restore event.

`VERIFIED_FACT`: Proposal-first and materialization primitives exist, but governance is not fully unified across every code path. Source: `docs/openlife-phase0-audit/06_security_governance_audit.md`.

## Boundary With Review Center

`DESIGN_DECISION`: Review Center owns decisions about consequential changes. Memory and LifeModel surfaces show state, history, provenance, and links to review.

Examples:

- Candidate memory requiring confirmation: Review Center owns approve/reject/edit/later.
- LifeModel preference update: Review Center owns decision; LifeModel shows resulting/pending state.
- Memory rollback/restore: Review Center or Settings/Data Management may own the dangerous action if consequential.

## Boundary With Workspace Evidence Drawer

`DESIGN_DECISION`: Workspace can show "this task produced a memory candidate" or "this result used these memories," but it must not become the source of durable memory truth.

`PHASE_2_REQUIRED`: Workspace evidence refs must point to backend-owned memory/LifeModel/review records.

## Boundary With LifeModel Page

`DESIGN_DECISION`: LifeModel explains the structured long-term understanding. Memory explains remembered facts/events/preferences/evidence and their lifecycle.

Constraint: Memory may feed LifeModel, but not every memory is LifeModel truth.

## Boundary With Settings / Data Management

`DESIGN_DECISION`: Settings/Data Management should own export, import, retention, deletion, backups, safe paths, and support/debug access.

`DESIGN_DECISION`: Everyday memory meaning belongs in `记忆` or LifeModel, not in a settings table.

## First-version Lightweight Memory Scope

`CANDIDATE`: If Memory remains top-level, first V2 can start with:

- memory status summary;
- candidate vs confirmed vs used-in-LifeModel vs withdrawn/expired filters;
- source/evidence preview;
- review links;
- archive/restore where governed;
- clear "not a raw database" copy.

`PHASE_2_REQUIRED`: Backend must provide lane/status/provenance summaries before top-level Memory implementation.

## Fallback If Memory Is Not Top-level

If Phase 2 cannot validate distinct Memory UX/read-model support, use one of:

- LifeModel sub-surface: `LifeModel > 记忆`;
- Settings/Data Management sub-surface for export/archive/restore;
- Workspace evidence preview only, with decisions still in Review Center.

This fallback preserves Memory as a product capability; it only changes navigation placement.

## Capability Preservation Note

`DESIGN_DECISION`: Do not delete Memory as a product concept solely because current implementation is incomplete. Mark missing read-model support as `PHASE_2_REQUIRED`.

## Human Decisions Needed

1. Which memory lanes require explicit Review Center confirmation?
2. Which low-risk lanes may be written directly, if any?
3. Whether `记忆` remains top-level after Phase 2 read-model validation.
4. What Chinese wording best distinguishes `依据`, `来源`, and `证据`.
5. Whether manual LifeModel editing remains, and how it is labeled as a governed/manual override.
