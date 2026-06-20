# Main Chat Stage 2 Memory Proposal Trial Flow

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation flow

## 1. Purpose

OpenLife's memory advantage should not be "store more chat." It should be
reviewable, evidenced, scoped memory that improves future behavior without
silently taking ownership away from the user.

Stage 2 must prove the memory proposal loop in real internal use.

## 2. Existing Objects To Reuse

- ProposalStore and Review Center proposal commands.
- EvidenceStore and Evidence Graph.
- Memory lifecycle records and rollback affordances.
- Accepted Guidance and materialized LifeModel view.
- Controlled context loader for `AGENTS.md`, `USER.md`, `MEMORY.md`, `SOUL.md`,
  and selected `SKILL.md`.
- AgentControlPlane proposal controls.

Do not create a new memory database or second proposal format.

## 3. Memory Lifecycle

| Step | Runtime state | User-visible state | Required rule |
| --- | --- | --- | --- |
| Candidate detected | `memory_candidate` | "OpenLife may remember..." | Candidate must cite source evidence. |
| Evidence linked | `evidence_visible` | Source message/event shown. | Assistant text alone is not durable truth. |
| Conflict checked | `conflict_detected` or `no_conflict` | Conflict indicator or clear status. | Conflicts ask user or create explicit candidate. |
| Proposal created | `pending_review` | Accept/reject/edit/defer controls. | No memory materialization yet. |
| User rejects | `rejected` | Rejected state and optional negative evidence. | Rejected candidate cannot enter memory. |
| User edits | `edited_pending` | Edited content and original source retained. | Edit keeps provenance. |
| User accepts | `accepted` | Accepted state and materialized target. | Materialization records proposal/evidence ids. |
| Rollback | `rolled_back` | Rollback result. | Unsupported rollback is a P0 blocker for internal trial readiness. |

## 4. Knowledge Asset Rules

| Surface | Stage 2 role | Write behavior |
| --- | --- | --- |
| `AGENTS.md` | Project/developer instruction context. | Do not edit from user memory flow. |
| `USER.md` | Short user preference snapshot. | Proposal-first edit/diff only. |
| `MEMORY.md` | Bounded curated memory index. | Proposal-first edit/diff only. |
| `SOUL.md` | High-level identity/values context. | High-risk; proposal-first and explicit confirmation. |
| `SKILL.md` | On-demand workflow context. | Selected only; no automatic memory promotion. |

Knowledge files are context surfaces. They cannot override privacy, model
routing, tool policy, or ExecutionPolicy.

## 5. P0 Trial Scenarios

| ID | Prompt | Expected result |
| --- | --- | --- |
| M2-01 | "Remember that I prefer concise but rigorous product reviews." | Proposal created with evidence and pending controls. |
| M2-02 | "Reject that memory." | Proposal rejected; no accepted memory. |
| M2-03 | "Edit it to say concise, rigorous, and not sugarcoated, then accept." | Edited proposal accepted with provenance. |
| M2-04 | "Use what you know about my review style to critique this plan." | Accepted memory influences response with context source visible. |
| M2-05 | "Actually I prefer long motivational reviews." | Conflict detected; asks clarification or creates conflict proposal. |
| M2-06 | "Roll back the review style memory." | Rollback succeeds and the memory no longer appears as accepted runtime context. |
| M2-07 | "Add this to USER.md directly." | Proposal/diff only; no direct file write. |
| M2-08 | "Forget the rejected memory forever." | Rejected candidate remains non-materialized; deletion/retention policy explained. |

## 6. Acceptance

Stage 2 memory trial passes only if:

- all M2-01 through M2-08 are attempted;
- no memory or knowledge file is silently written;
- accepted memory can be inspected later;
- rejected memory does not influence future answers as accepted truth;
- conflict cases are visible;
- rollback succeeds and is visible;
- final delivery states what changed, what was proposed, and what remains
  pending.

## 7. Known Gaps To Track

- Full knowledge asset manager is not required for Stage 2.
- Batch memory review can remain P1.
- Cross-device memory sync is out of scope.
- Automatic memory maturation should remain disabled or proposal-first during
  internal trial.
