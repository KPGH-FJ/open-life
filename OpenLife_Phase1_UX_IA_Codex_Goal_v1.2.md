# OpenLife Phase 1 UX / IA v1.2
## Codex Goal Mode Specification

## Goal

Generate the Phase 1 UX / IA / Product Language / ViewModel decision documentation package for OpenLife.

This v1.2 goal adds four execution guardrails before Phase 1 is run:

1. ADR evidence must be typed and cannot hide product assumptions inside "evidence".
2. User scenarios must use a fixed output format.
3. ViewModel actions must distinguish product, review, and debug actions.
4. The agent must not invent backend ViewModels/endpoints/projections/stores/workflows.

This is documentation only.

Do not implement Frontend V2.

---

# Role

You are acting as a Principal Product Engineer and AI-native UX Architect.

Your job is to transform Phase 0 and Phase 0.5 audit facts into human-reviewable product and architecture decision documents.

---

# Required Inputs

Read:

```text
docs/openlife-phase0-audit/
docs/phase0_5/
```

Prioritize:

```text
docs/openlife-phase0-audit/02_backend_capability_map.md
docs/openlife-phase0-audit/03_agent_system_analysis.md
docs/openlife-phase0-audit/05_backend_frontend_contract.md
docs/openlife-phase0-audit/08_frontend_current_state_audit.md
docs/openlife-phase0-audit/09_agent_experience_gap_analysis.md
docs/openlife-phase0-audit/10_rewrite_strategy.md
docs/openlife-phase0-audit/11_frontend_v2_requirements.md
docs/openlife-phase0-audit/12_rebirth_strategy.md
docs/openlife-phase0-audit/13_audit_summary.md

docs/phase0_5/02_current_route_map.md
docs/phase0_5/03_chat_companion_workspace_mapping.md
docs/phase0_5/04_diagnostics_visibility_inventory.md
docs/phase0_5/05_ui_terminology_inventory.md
docs/phase0_5/06_view_model_gap_inventory.md
docs/phase0_5/07_phase0_5_summary.md
```

If any input is missing, record it in `10_phase1_summary.md`.

---

# Hard Non-Goals

Phase 1 outputs must not include:

- React component implementation
- CSS implementation
- route creation
- backend schema migration
- mock API pretending to be product truth
- hardcoded frontend-only ViewModel
- ProductShell refactor
- ChatPage refactor
- MailboxPage refactor
- backend command changes
- Tauri bridge changes

Only write documentation under:

```text
docs/phase1_ux_ia/
```

---

# Evidence and Hallucination Rules

Every major statement must be classified as one of:

```text
VERIFIED_FACT
DESIGN_DECISION
DESIGN_ASSUMPTION
CANDIDATE
UNKNOWN
PHASE_2_REQUIRED
```

A design decision can rely on a verified fact, but must not pretend to be a verified fact.

Do not claim:

- desktop/Tauri trial green
- browser E2E green
- live provider ready
- Web AgentLoop ready
- MCP AgentLoop ready

unless already verified in Phase 0 / Phase 0.5.

Treat:

- `frontend/src/tauri.ts` as product bridge.
- `frontend/src/tauriDev.ts` as dev/test compatibility.
- `LifeStateProjection` or adjacent backend read models as preferred product-state authority where available.

---

# Guardrail 1: Typed ADR Evidence

Every ADR evidence item must include:

```text
Evidence type:
Source:
Claim:
Confidence:
Limitation:
```

Allowed evidence types:

```text
Verified Fact from Phase 0 / 0.5
Existing codebase fact
Product design rationale
User experience assumption
Engineering assumption
Open item
```

Evidence must not be only subjective judgment.

If evidence is product reasoning, label it as `Product design rationale` or `User experience assumption`.

If evidence is future implementation feasibility, label it as `Engineering assumption`.

If evidence is not verified, label it as `Open item`.

---

# Guardrail 2: Fixed User Scenario Format

Every required user scenario must use this exact format:

```markdown
## Scenario S#: <title>

User goal:

Entry surface:

Surfaces involved:

Default UI:

System understanding:

Execution timeline:

Review Center trigger:

Task state:

LifeModel / Memory impact:

Diagnostics visibility:

Required ViewModel fields:

Failure / empty state:

Success criteria:

Evidence classification:

Open questions:
```

Required scenarios:

1. User asks OpenLife to plan today's priorities.
2. User asks OpenLife to execute a task requiring external write.
3. OpenLife detects a candidate memory requiring confirmation.
4. OpenLife proposes updating a long-term LifeModel preference.
5. A tool call fails; user needs to understand what happened without reading raw trace.

---

# Guardrail 3: Split ViewModel Actions

Use this envelope:

```ts
type ViewModelEnvelope<T> = {
  data: T | null
  status: 'loading' | 'ready' | 'empty' | 'error' | 'stale'
  lastUpdatedAt: string | null
  source: 'backend-readmodel'
  evidenceRefs?: EvidenceRef[]
  warnings?: ViewModelWarning[]
  actions: {
    primary: ProductAction[]
    review?: ReviewAction[]
    debugOnly?: DebugAction[]
  }
}
```

Definitions:

- `ProductAction`: default user-facing action required to complete the task.
- `ReviewAction`: approval / rejection / edit / later / evidence action for consequential change.
- `DebugAction`: advanced or developer-only action such as raw trace, export JSON, provider health.

Do not mix debug actions into default product actions.

---

# Guardrail 4: Do Not Invent Backend Contracts

Do not invent backend ViewModels, endpoints, projections, stores, or workflows and describe them as existing.

If a required future capability does not exist, mark it as:

```text
CANDIDATE
ENGINEERING_ASSUMPTION
PHASE_2_REQUIRED
UNKNOWN
```

Use this form:

```text
Backend owner:
UNKNOWN or Proposed

Status:
PHASE_2_REQUIRED

Required validation:
<what Phase 2 must verify or implement>
```

Do not claim `EXISTING` unless current code or Phase 0/0.5 evidence verifies it.

---

# Product Capability Preservation Rule

Do not over-constrain the product by deleting important capabilities just because the current implementation is incomplete.

If a capability is important for the OpenLife product vision but not fully verified:

- keep it as `CANDIDATE` or `PHASE_2_REQUIRED`;
- specify required backend/read-model validation;
- specify a fallback UX;
- do not remove it from the design unless humans explicitly reject it.

Examples:

- Memory may remain a top-level candidate with constraints.
- LifeModel provenance should remain a product concept even if the first ViewModel needs Phase 2 work.
- Advanced evidence should be hidden by default but not deleted.
- Review Center should remain broad enough for proposals, permissions, external writes, memory updates, LifeModel changes, and policy changes.

Guardrails prevent hallucination; they must not reduce OpenLife into a generic chat app, todo app, settings panel, or dashboard.

---

# Approved Design Direction

Use these Phase 1 direction decisions:

1. V2 is a bounded product-experience + state-contract rewrite, not a blank rebuild.
2. Companion + Chat should merge into `工作区`.
3. Mailbox should become `审核中心`.
4. Runs should become `任务`.
5. Memory can become top-level `记忆`, but only as `Accepted with constraints`.
6. `LifeModel` remains English-branded, but must have Chinese explanatory copy.
7. Diagnostics are hidden by default but available through advanced inspection.
8. Backend-owned ViewModels / ReadModels must be defined before UI implementation.

---

# Aesthetic Direction

OpenLife V2 should learn from high-quality agent/dev productivity products such as Codex, Cursor, Claude workspace, and Linear.

Do not copy any product.

Extract principles:

- minimal
- calm
- precise
- high information quality
- low decoration
- strong typography hierarchy
- task/workspace-centered
- left navigation for stable structure
- center workspace for current work
- contextual review/inspection only when needed
- evidence available but not overwhelming
- no flashy AI mascot
- no dashboard card overload
- no raw trace as default UI

---

# Required Outputs

Create:

```text
docs/phase1_ux_ia/
```

Generate exactly:

```text
01_v2_decision_record.md
02_product_positioning.md
03_v2_information_architecture.md
04_agent_workspace_model.md
05_review_center_model.md
06_lifemodel_memory_model.md
07_chinese_product_language_v1.md
08_diagnostics_visibility_policy.md
09_view_model_contract_proposal.md
10_phase1_summary.md
```

---

# Required Content by File

## 01_v2_decision_record.md

Use ADR-style records.

Each decision must include:

```text
Decision ID
Title
Status
Decision
Evidence
Product rationale
Engineering impact
Risk
Reversal cost
Phase 2 implication
Human approval needed
```

Every Evidence item must include:

```text
Evidence type
Source
Claim
Confidence
Limitation
```

Required decisions:

| ID | Decision | Required status |
|---|---|---|
| D1-bounded-rewrite | V2 uses bounded product-experience + state-contract rewrite | Accepted |
| D2-workspace | Companion + Chat merge into 工作区 | Accepted |
| D3-review-center | Mailbox becomes 审核中心 | Accepted |
| D4-tasks | Runs becomes 任务 | Accepted |
| D5-memory-nav | Memory becomes top-level 记忆 | Accepted with constraints |
| D6-lifemodel-name | LifeModel remains English-branded | Accepted with constraints |
| D7-diagnostics | Diagnostics hidden by default, available through advanced inspection | Accepted |
| D8-viewmodel-first | Backend-owned ViewModels / ReadModels before UI implementation | Accepted |

For D5-memory-nav, include fallback:

If Phase 1 cannot clearly distinguish Memory from LifeModel, Review Center, and Workspace Evidence, Phase 2 may downgrade Memory to a LifeModel sub-surface or Settings/Data Management sub-surface.

---

## 02_product_positioning.md

Define:

- one-line positioning
- what OpenLife is
- what OpenLife is not
- user promise
- trust promise
- control promise
- local-first privacy framing
- Chinese-first first-version audience
- product capability preservation principle

Must explicitly say OpenLife is not:

- generic chat app
- dashboard
- todo app
- CRM
- knowledge base
- raw database browser
- developer console

---

## 03_v2_information_architecture.md

Define top-level navigation.

Primary proposed IA:

```text
今日
工作区
任务
审核中心
LifeModel
记忆
设置
```

Also include a reduced-risk alternative IA:

```text
今日
工作区
任务
审核中心
LifeModel
设置

LifeModel subnav:
- 概览
- 目标
- 偏好
- 关系
- 记忆
- 依据与变更
```

Required migration matrix:

| Current page / route | V2 destination | Preserve | Migrate | Remove / hide | Risk |
|---|---|---|---|---|---|

Must include at least:

- Today
- ChatPage
- CompanionPage
- Mailbox
- Runs
- LifeModel
- Memory
- Settings

Memory top-level must be treated as `Accepted with constraints`, not unconditional.

---

## 04_agent_workspace_model.md

The workspace is not a renamed ChatPage.

Define four required zones:

```text
Workspace
├── Intent Composer
├── Understanding Panel
├── Execution Timeline
└── Control / Review Drawer
```

Also define:

- user goal object
- agent understanding object
- plan/lifecycle object
- execution timeline
- review links
- result object
- evidence drawer
- advanced inspector
- empty/loading/running/waiting/blocked/failed/completed states
- composer behavior
- scenario coverage using the fixed scenario format

Must include responsibility migration:

| Existing ChatPage responsibility | V2 destination | Reason |
|---|---|---|

Destinations:

- 工作区
- 审核中心
- 任务
- 高级检查
- 删除/隐藏
- Needs human decision

---

## 05_review_center_model.md

Define a unified ReviewItem model.

Required type:

```ts
type ReviewItemType =
  | 'proposal'
  | 'permission_request'
  | 'external_write'
  | 'memory_update'
  | 'lifemodel_change'
  | 'policy_change'
  | 'dangerous_action'
```

Required status:

```ts
type ReviewItemStatus =
  | 'pending'
  | 'approved'
  | 'rejected'
  | 'expired'
  | 'blocked'
  | 'revoked'
  | 'failed'
```

Every ReviewItem must define:

- user-readable title
- risk level
- impact scope
- source
- evidence
- default recommendation
- available actions
- expiration behavior
- audit record
- relation to Workspace
- relation to Task
- relation to Memory
- relation to LifeModel

Available review actions:

```text
批准
拒绝
稍后
修改
查看依据
```

Use `ReviewAction`, not generic `ProductAction`.

---

## 06_lifemodel_memory_model.md

Define user-understandable relationship:

```text
LifeModel:
OpenLife 对“你是谁、你在乎什么、你当前状态如何、你长期目标是什么”的结构化理解。

记忆:
OpenLife 记住过的事实、事件、偏好、证据和候选更新。

依据:
某个理解或记忆来自哪里。

变更:
OpenLife 准备如何更新它对你的理解。
```

Required memory states:

| State | User explanation |
|---|---|
| 候选记忆 | OpenLife 认为可能值得记住，但还没确认 |
| 已确认记忆 | 用户确认过或系统可信写入 |
| 已用于 LifeModel | 已影响长期理解 |
| 已撤回 / 已过期 | 不再使用或被用户移除 |

Must discuss boundary conflicts with:

- Review Center
- Workspace evidence drawer
- LifeModel page
- Settings/Data Management

Do not delete Memory as a product capability solely because current implementation is incomplete. Mark missing read-model support as `PHASE_2_REQUIRED`.

---

## 07_chinese_product_language_v1.md

Must include:

- recommended words
- forbidden/default-hidden words
- ordinary user terms
- review terms
- advanced inspection terms
- developer-only terms
- status labels
- action verbs
- review/proposal terminology
- LifeModel/Memory terminology
- diagnostics terminology
- Agent tone guidelines

Forbidden/default-hidden words for normal users should include:

- run
- trace
- proposal
- kernel
- provider
- policy router
- final delivery
- AgentRun
- raw transcript
- mailbox

They may appear in advanced/developer surfaces only.

---

## 08_diagnostics_visibility_policy.md

Define four main levels:

```text
DEFAULT_PRODUCT
EXPANDABLE_DETAILS
ADVANCED_INSPECTOR
DEVELOPER_ONLY
```

Also allow:

```text
REMOVE_OR_ARCHIVE
NEEDS_HUMAN_DECISION
```

Must classify:

- Safe Mode
- usage readiness
- current task status
- pending review count
- tool permission summary
- runtime disclosure
- tool call details
- reasoning trace
- run trace
- kernel events
- durable events
- raw transcript
- provider health
- PolicyRouter
- ModelRouter
- MCP/A2A
- metrics
- calibration
- versions
- tauriDev/test/historical surfaces

Preserve evidence access. Hide by default does not mean delete.

---

## 09_view_model_contract_proposal.md

Use this envelope:

```ts
type ViewModelEnvelope<T> = {
  data: T | null
  status: 'loading' | 'ready' | 'empty' | 'error' | 'stale'
  lastUpdatedAt: string | null
  source: 'backend-readmodel'
  evidenceRefs?: EvidenceRef[]
  warnings?: ViewModelWarning[]
  actions: {
    primary: ProductAction[]
    review?: ReviewAction[]
    debugOnly?: DebugAction[]
  }
}
```

Hard rules:

1. Pages cannot reconstruct product truth from raw domain reads.
2. Pages can only render backend-owned ViewModels or raw data explicitly marked as debug-only.
3. Do not invent backend owners/endpoints/projections/stores/workflows.
4. Future required backend fields must be marked `PHASE_2_REQUIRED`.

For each ViewModel define:

- Backend owner
- Owner status: EXISTING / PARTIAL / PROPOSED / UNKNOWN / PHASE_2_REQUIRED
- UI cannot infer
- Empty state
- Error state
- Stale state
- Evidence model
- Product actions
- Review actions
- Debug-only actions
- Auditability
- Required fields
- Existing backend support
- Missing backend projection fields
- Phase 2 implication

Required ViewModels:

- TodayViewModel
- WorkspaceViewModel
- TasksViewModel
- ReviewCenterViewModel
- LifeModelViewModel
- MemoryViewModel
- SettingsViewModel

---

## 10_phase1_summary.md

Must include:

- decisions made
- accepted with constraints
- open questions
- implementation blockers
- required human approvals
- Phase 2 entry checklist
- capability preservation notes

Also include at least 5 core user scenarios using the fixed scenario format:

1. User asks OpenLife to plan today's priorities.
2. User asks OpenLife to execute a task requiring external write.
3. OpenLife detects a candidate memory requiring confirmation.
4. OpenLife proposes updating a long-term LifeModel preference.
5. A tool call fails; user needs to understand what happened without reading raw trace.

---

# Completion Criteria

Phase 1 v1.2 is complete only when:

1. 10 documents exist under `docs/phase1_ux_ia/`.
2. Every core decision has status: Accepted / Accepted with constraints / Rejected / Open / Needs validation.
3. Every ADR Evidence item has Evidence type, Source, Claim, Confidence, and Limitation.
4. Memory top-level includes both primary IA and fallback IA.
5. ViewModelEnvelope uses `actions.primary`, `actions.review`, and `actions.debugOnly`.
6. No nonexistent backend owner/endpoint/projection/store/workflow is described as existing.
7. Future important capabilities are preserved as CANDIDATE / PHASE_2_REQUIRED, not deleted.
8. V2 IA includes current route to new surface migration matrix.
9. Workspace model explicitly decomposes current ChatPage responsibilities.
10. Review Center model includes unified ReviewItem type/status machine.
11. LifeModel / Memory / Evidence / Change relationship is understandable to ordinary Chinese users.
12. Diagnostics policy defines default / expandable / advanced / developer-only layers.
13. User scenarios use the fixed output format.
14. No React page, V2 route, backend contract, CSS, or component code is created.
15. Phase 2 entry checklist lists engineering validation open items.

---

# Final Response Format

When complete, respond:

```markdown
Phase 1 UX/IA v1.2 documentation complete.

Created:
- docs/phase1_ux_ia/01_v2_decision_record.md
- docs/phase1_ux_ia/02_product_positioning.md
- docs/phase1_ux_ia/03_v2_information_architecture.md
- docs/phase1_ux_ia/04_agent_workspace_model.md
- docs/phase1_ux_ia/05_review_center_model.md
- docs/phase1_ux_ia/06_lifemodel_memory_model.md
- docs/phase1_ux_ia/07_chinese_product_language_v1.md
- docs/phase1_ux_ia/08_diagnostics_visibility_policy.md
- docs/phase1_ux_ia/09_view_model_contract_proposal.md
- docs/phase1_ux_ia/10_phase1_summary.md

Major decisions:
1.
2.
3.

Accepted with constraints:
1.
2.

Open questions:
1.
2.
3.

Phase 2 blockers:
1.
2.
3.

Capability preservation notes:
1.
2.
3.

No production code was modified.
Frontend V2 implementation was not started.
```
