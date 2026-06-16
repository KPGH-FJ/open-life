# Main Chat Permission, Proposal, And Memory UX Contract v1

> Date: 2026-06-16
> Status: required preparation artifact before Main Chat Agent Productization v1
> Parent: `plans/openlife_agent_product_capability_matrix_v1.md`

## 1. Purpose

OpenLife's differentiator is governed execution plus long-term personal memory.
That only works if permission, proposal, and memory flows are clear to users.

This document defines the first UX contract for:

- execution permission
- ToolPermission proposal
- write/proposal-first behavior
- memory proposal and confirmation
- rejection, deferral, and rollback

## 2. Risk Policy UX

| Risk class | Runtime behavior | User-facing behavior |
| --- | --- | --- |
| Safe read | May auto-execute if policy allows. | Show action and observation; no modal required. |
| Local low-risk write | Proposal-first unless explicitly whitelisted. | Show proposal or scoped confirmation. |
| Memory/LifeModel update | Proposal-first. | Show memory proposal card with evidence. |
| External write | Requires explicit confirmation. | Show permission request with target, risk, and consequence. |
| Dangerous action | Blocked. | Show blocker with reason and no approve button. |
| Unknown/unsafe tool | Blocked or proposal-first. | Show safety blocker or request tool review. |

The UI must help execution proceed where safe, but it must not hide risk.

## 3. Permission Request Card

Required fields:

| Field | Meaning |
| --- | --- |
| `permissionId` | Stable id tied to pending action or proposal. |
| `actionId` | Exact pending action that permission applies to. |
| `toolName` | Metadata-safe tool label. |
| `target` | Exact target or bounded target label. |
| `riskClass` | Safe read, local low-risk, external write, dangerous, unknown. |
| `scope` | Once, task, session, or user-configured duration. |
| `consequence` | What will happen if approved. |
| `denyEffect` | What will happen if denied. |
| `deferEffect` | Whether the task can resume later. |

Required controls:

- approve once
- deny
- defer
- cancel task
- inspect trace

Rules:

- Approval applies only to the exact pending action unless the user explicitly
  chooses broader scope.
- If target/action changes after approval, permission must be requested again.
- Deny must prevent execution and preserve audit history.
- Defer must keep task resumable when runtime supports it.

## 4. Proposal Types

| Proposal type | Purpose | Durable effect |
| --- | --- | --- |
| Memory proposal | Add/update/remove user memory or preference. | Applies only after acceptance. |
| LifeModel proposal | Update structured LifeModel/guidance. | Applies only after acceptance. |
| ToolPermission proposal | Allow a specific tool/target/action. | Applies only to scoped pending action or configured scope. |
| Write request proposal | Ask user to approve local/external write. | Applies only after confirmation. |
| Task follow-up proposal | Create a follow-up task or plan. | Applies only after user action. |

Proposal is not completion. A final delivery must say "proposal created" rather
than "change completed" until accepted.

## 5. Canonical Proposal Status

All proposal UI surfaces use this status enum:

```ts
type ProposalStatus =
  | "draft"
  | "pending_review"
  | "accepted"
  | "rejected"
  | "deferred"
  | "rolled_back"
  | "stale";
```

Rules:

- `draft` is not user-reviewable yet.
- `pending_review` is visible to the user and can be accepted/rejected/edited.
- `accepted` means the proposal outcome was accepted, not necessarily that every
  downstream materialized view has refreshed.
- `rejected` must not affect runtime memory.
- `deferred` may remain in Review Center but must not be runtime memory.
- `rolled_back` records that a previously accepted/materialized effect was
  undone or superseded.
- `stale` applies when the underlying action/target/evidence changed and the
  proposal can no longer be safely applied.

Memory-specific lifecycle events such as `edited`, `materialized`, and
`rollback_available` should be modeled as events or derived flags, not as
separate proposal statuses.

## 6. Memory Proposal Card

Required fields:

| Field | Meaning |
| --- | --- |
| `proposalId` | ProposalStore id. |
| `candidateText` | Proposed memory/guidance text. |
| `memoryType` | Preference, fact, goal, work style, communication style, energy pattern, or task-specific note. |
| `scope` | Global, project, task, temporary, or custom scope. |
| `evidence` | Source messages/events/observations supporting the candidate. |
| `confidence` | Low, medium, high, or explicit numeric band. |
| `conflicts` | Existing memory/guidance that may conflict. |
| `reason` | Why the Agent believes this should be remembered. |
| `impact` | Where this memory would affect future behavior. |
| `status` | Canonical `ProposalStatus`. |
| `events` | Memory-specific lifecycle events such as edited, materialized, or rollback_available. |

Required controls:

- accept
- reject
- edit
- defer
- change scope
- view evidence
- rollback if already accepted

## 7. Memory Proposal Lifecycle

```text
candidate_detected
  -> pending_review(status)
  -> accepted(status) -> materialized(event) -> rollback_available(derived flag)
  -> rejected(status)
  -> edited(event) -> pending_review(status)
  -> deferred(status)
  -> stale(status)
```

Rules:

- Assistant-generated text cannot become user fact.
- Raw transcript cannot become long-term memory directly.
- Rejected memory must not be used as accepted memory.
- Deferred memory may appear in Review Center but not runtime memory context.
- Edited memory must preserve original evidence and edit history as events.
- Accepted memory must show provenance and rollback availability.
- Rollback must remove or supersede materialized memory, set status or linked
  successor appropriately, and record why.

## 8. Conflict UX

When candidate memory conflicts with existing memory:

User should see:

- new candidate
- existing conflicting memory
- evidence for both
- confidence for both
- suggested resolution
- accept new / keep old / edit / reject controls

Runtime requirements:

- Conflict must be backed by evidence graph or memory search.
- If conflict evidence is weak, show "possible conflict" rather than a hard
  conflict.
- Do not silently overwrite accepted memory.

## 9. ToolPermission Proposal UX

ToolPermission proposal is different from memory proposal.

Required fields:

| Field | Meaning |
| --- | --- |
| `proposalId` | Proposal id. |
| `actionId` | Pending action id. |
| `tool` | Tool/MCP/Skill label. |
| `target` | Exact target. |
| `actionType` | Exact action type. |
| `scope` | Once/task/session/custom. |
| `risk` | Risk class. |
| `resumeBehavior` | What resumes after acceptance. |

Rules:

- ToolPermission proposal acceptance must resume the exact pending action.
- It must not be credited as successful tool execution before action runs.
- It must not overlap with MCP read success evidence.
- If the pending target changes, proposal becomes stale.

## 10. Permission And Proposal In Chat

Chat rendering rules:

- Permission cards appear inline at the point of blockage.
- Proposal cards appear inline and are also linked to Review Center.
- Final delivery lists proposals as pending/accepted/rejected, not as hidden
  side effects.
- If user accepts/rejects from Review Center, Chat task state must update.

## 11. Review Center Integration

Review Center must support:

- pending proposal list
- accepted/rejected history
- evidence viewer
- conflict viewer
- scope editing
- rollback
- link back to originating task/session/message

Chat must not become the only place where a proposal can be reviewed.

## 12. Negative Assertions

The product must never:

- silently write durable memory from a normal answer
- treat assistant claims as user facts
- hide a permission request in prose
- approve a different target than the pending action
- mark a proposal as completed execution
- use rejected memory as accepted memory
- let `USER.md`, `MEMORY.md`, `SOUL.md`, or `SKILL.md` override privacy/tool
  policy
- auto-load unselected `SKILL.md` content

## 13. Acceptance

This contract is satisfied when product tests prove:

- "remember this" creates a memory proposal, not a direct write
- accept applies candidate with provenance
- reject prevents runtime memory use
- edit preserves evidence and history
- rollback removes or supersedes accepted materialization
- permission approve/deny/defer controls affect exact pending action
- final delivery separates executed actions from proposals and pending review
