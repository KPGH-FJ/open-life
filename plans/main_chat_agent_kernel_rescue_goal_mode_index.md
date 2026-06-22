# Main Chat Agent Kernel Rescue Goal Mode Index

> Date: 2026-06-22
> Status: execution index for the eight goal-mode development passes
> Parent: `plans/main_chat_agent_kernel_rescue_preparation.md`

## 1. Feasibility

Using eight goal-mode passes is feasible and preferable for this rescue, as
long as each goal is treated as a bounded delivery slice with its own acceptance
evidence.

The eight goals are development goals. The current preparation branch is not
one of the eight delivery goals; it prepares them.

## 2. Goal Sequence

| Goal | Spec | Outcome |
| --- | --- | --- |
| 1 | `plans/main_chat_agent_kernel_rescue_goal_1_kernel_foundation.md` | Shared direct-answer-only Main Chat kernel. |
| 2 | `plans/main_chat_agent_kernel_rescue_goal_2_send_stream_convergence.md` | `send_message` and `start_stream_message` become kernel adapters. |
| 3 | `plans/main_chat_agent_kernel_rescue_goal_3_read_only_tools.md` | Minimal read-only tool loop through governed execution. |
| 4 | `plans/main_chat_agent_kernel_rescue_goal_4_proposal_only_writes.md` | Memory/LifeModel/file/external writes become proposal or blocker outcomes. |
| 5 | `plans/main_chat_agent_kernel_rescue_goal_5_execution_ux.md` | Chat shows real kernel evidence: thinking, tool, proposal, blocker. |
| 6 | `plans/main_chat_agent_kernel_rescue_goal_6_hs_reintegration.md` | HS returns as bounded context and proposal-reviewed learning. |
| 7 | `plans/main_chat_agent_kernel_rescue_goal_7_web_mcp_provider.md` | Web/MCP/provider capabilities restored on top of stable kernel. |
| 8 | `plans/main_chat_agent_kernel_rescue_goal_8_cleanup_final_gate.md` | Legacy paths reduced and final gates realigned to the new kernel. |

## 3. Cross-Goal Rules

These rules apply to every goal:

- keep `MainChatKernel` as the default runtime spine once it is adopted by
  command surfaces in Goal 2;
- never add a second send/stream implementation path;
- durable writes require proposal, permission, or explicit hard blocker;
- HS can inform context and policy but must not silently materialize truth;
- unsupported capabilities must fail closed with named blockers;
- user-facing UI must not claim execution without runtime evidence;
- final/live/readiness gates must validate the kernel, not replace it.

## 4. Goal Mode Start Protocol

When starting a goal-mode pass:

1. Read this index and the specific goal spec.
2. Confirm `git status --short --branch`.
3. State the exact objective from the goal spec.
4. Work only inside the allowed scope unless a compile/test blocker forces a
   small supporting change.
5. Run the verification commands listed in the goal spec.
6. Fill `plans/main_chat_agent_kernel_rescue_goal_completion_template.md` as the
   completion report for the goal.
7. Mark the goal complete only when the acceptance checklist and matching K-row
   acceptance matrix entries are satisfied.

## 5. Stop Protocol

Stop and ask before continuing if:

- a goal requires rewriting more than its declared allowed scope;
- send/stream parity cannot be preserved;
- a proposed fix needs to weaken no-silent-write governance;
- a goal requires external live-provider credentials not available locally;
- a goal starts recreating large pieces of legacy `main_chat_strategy.rs`.

## 6. Branching

Recommended branches:

```text
rescue/main-chat-kernel-goal-1
rescue/main-chat-kernel-goal-2
rescue/main-chat-kernel-goal-3
rescue/main-chat-kernel-goal-4
rescue/main-chat-kernel-goal-5
rescue/main-chat-kernel-goal-6
rescue/main-chat-kernel-goal-7
rescue/main-chat-kernel-goal-8
```

Each branch should start from the completed previous goal unless a deliberate
rollback decision is made.

## 7. Preparation Inputs

Before starting Goal 1, read:

- `plans/main_chat_agent_kernel_rescue_preparation_scope.md`
- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `plans/main_chat_agent_kernel_rescue_goal_completion_template.md`

These documents define the system lessons, industry practices, and spec-coding
standard used by the eight goal specs.
