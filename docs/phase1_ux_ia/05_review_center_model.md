# Review Center Model

Status: Phase 1 Review Center model proposal.
Scope: Product model and read-model requirements only. No MailboxPage refactor.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Purpose

`DESIGN_DECISION`: `审核中心` is the central control surface for consequential changes. It is not a mailbox and not a notification feed.

`VERIFIED_FACT`: Current `/mailbox` already handles proposal lists, accept/reject/postpone/edit, safe-mode constraints, safe-path external-write checks, and task resume after proposal review. Source: `docs/phase0_5/02_current_route_map.md`, `docs/phase0_5/03_chat_companion_workspace_mapping.md`.

`PHASE_2_REQUIRED`: A unified server-owned ReviewItem read model is required before V2 implementation can claim Review Center as the authority for all review types.

## Phase 2 Stop Rules

1. `PHASE_2_REQUIRED`: Do not refactor Mailbox or build Review Center UI until a backend-owned ReviewItem read model exists or is explicitly scoped.
2. `PHASE_2_REQUIRED`: Do not let frontend code infer allowed actions, risk level, expiration behavior, or durable apply state from proposal type, tool name, or local page state.
3. `PHASE_2_REQUIRED`: Do not present `approved` as durable completion unless backend materialization/apply state proves it.
4. `PHASE_2_REQUIRED`: If non-proposal item types are not backend-owned yet, keep them `CANDIDATE` and do not render them as live product commitments.

## ReviewItem Type

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

Classification: `DESIGN_DECISION`. These item types preserve important OpenLife capabilities without claiming all are unified today.

## ReviewItem Status

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

Classification: `DESIGN_DECISION`. Status labels must distinguish approval from durable application.

## ReviewItem Required Fields

| Field | Purpose | Classification |
| --- | --- | --- |
| user-readable title | User knows what decision is being requested. | `DESIGN_DECISION` |
| risk level | Low / medium / high or equivalent. | `DESIGN_DECISION` |
| impact scope | What will change and where. | `DESIGN_DECISION` |
| source | Task, workspace, tool, memory, LifeModel, policy, or settings origin. | `DESIGN_DECISION` |
| evidence | Why the request exists, with source refs. | `DESIGN_DECISION` |
| default recommendation | What OpenLife recommends and why. | `CANDIDATE` |
| available actions | Approved action set for this item/status. | `PHASE_2_REQUIRED` |
| expiration behavior | What happens if user does nothing. | `PHASE_2_REQUIRED` |
| audit record | How the decision can be traced later. | `DESIGN_DECISION` |
| related task | Optional task to resume/resolve after decision. | `PHASE_2_REQUIRED` |
| durable apply state | Whether approved change has actually materialized. | `PHASE_2_REQUIRED` |

## Available Review Actions

These are `ReviewAction`, not generic `ProductAction`.

- 批准
- 拒绝
- 稍后
- 修改
- 查看依据

`DESIGN_DECISION`: Review actions must be separated from default product actions. A page may link to a ReviewItem, but the decision itself belongs to Review Center or a Review Center-controlled drawer.

## Relationship To Workspace

- `DESIGN_DECISION`: Workspace may preview review-needed items and show why a task is waiting.
- `DESIGN_DECISION`: Workspace must not present "proposal created" as "durable change completed."
- `PHASE_2_REQUIRED`: WorkspaceViewModel needs review item refs, status, action availability, and resume relationship.

## Relationship To Tasks

- `DESIGN_DECISION`: Tasks show lifecycle and evidence for work that created or waits on review items.
- `PHASE_2_REQUIRED`: TasksViewModel needs a server-owned relation from task/run to ReviewItem and resume state.
- `UNKNOWN`: The canonical relationship between AgentRun and Main Chat task session still needs human/engineering decision.

## Relationship To Memory

- `DESIGN_DECISION`: Candidate memories and memory updates that are consequential should become ReviewItems.
- `VERIFIED_FACT`: MemoryGateway and memory lifecycle primitives exist. Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`.
- `PHASE_2_REQUIRED`: Memory lane/status read model must define which memory updates require review and which low-risk lanes may be direct.

## Relationship To LifeModel

- `DESIGN_DECISION`: LifeModel changes should show before/after, evidence, source, risk, and materialization state.
- `VERIFIED_FACT`: LifeModel, patch, compatibility/provenance views, and write gateway direction exist. Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`, `docs/openlife-phase0-audit/02_backend_capability_map.md`.
- `PHASE_2_REQUIRED`: Review Center must distinguish approved proposal from applied canonical LifeModel update.

## Relationship To Tool Permissions

- `VERIFIED_FACT`: ToolPermissionStore supports allow, deny, ask every time, allow once, and allow until revoked. Source: `docs/openlife-phase0-audit/02_backend_capability_map.md`.
- `DESIGN_DECISION`: Permission prompts should appear as review items when they block or resume consequential work.
- `PHASE_2_REQUIRED`: Tool permission ReviewItems need allowed action metadata from backend authority, not inferred frontend labels.

## Relationship To External Writes And Dangerous Actions

- `VERIFIED_FACT`: Safe-path file writes, danger preflight, typed confirmation, and safe-mode blocking exist. Source: `docs/openlife-phase0-audit/06_security_governance_audit.md`.
- `DESIGN_DECISION`: External writes and dangerous actions belong in Review Center or a Review Center-controlled confirmation drawer.
- `PHASE_2_REQUIRED`: A unified ReviewItem model must carry target, scope digest, risk, confirmation phrase, affected item count, safe mode status, and audit refs where applicable.

## What Should Not Stay In Workspace By Default

| Item | Destination | Reason |
| --- | --- | --- |
| full proposal payload | 审核中心 / 高级检查 | Default workspace should show decision summary only. |
| raw diff/patch internals | 审核中心 details / 高级检查 | Too technical for default task flow. |
| permission policy internals | 审核中心 details / 设置 | User needs action and impact, not internal policy chain. |
| raw trace behind a review item | 高级检查 | Evidence preserved without overwhelming normal review. |
| manual durable apply controls | 审核中心 | Durable writes need governed review action context. |

## Risk Model

`CANDIDATE`: Use a simple visible risk model in V2:

- Low: reversible, local, non-sensitive, no external transmission.
- Medium: affects future behavior, memory, provider/tool permission, or local file state.
- High: external write, sensitive data, dangerous action, irreversible deletion, broad policy change, or canonical LifeModel update.

`PHASE_2_REQUIRED`: Backend authority must validate risk and allowed actions. UI must not infer risk only from item type or tool name.

## Auditability

`DESIGN_DECISION`: Every ReviewItem needs evidence refs and an audit record. Users should be able to answer:

- What is OpenLife asking to change?
- Why did it ask?
- What happens if I approve?
- Did the change actually get applied?
- How can I inspect the evidence later?

## Human Decisions Needed

1. Whether Review Center owns permissions, external writes, policy changes, and dangerous actions in addition to proposals.
2. Which risk labels are user-facing.
3. Whether review expiration is allowed, and default behavior after expiration.
4. How much evidence is default-visible in a review card.
5. Whether approved-but-not-applied should be a separate visible state.
