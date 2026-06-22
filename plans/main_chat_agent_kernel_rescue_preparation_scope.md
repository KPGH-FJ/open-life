# Main Chat Kernel Rescue Preparation Scope

> Date: 2026-06-22
> Status: preparation scope for the eight goal-mode rescue passes
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## 1. What Must Be Prepared Before Coding

Before Goal 1 starts, the preparation layer must make five things explicit:

1. The system-level lessons from current OpenLife.
2. The external agent-engineering practices that shape the rescue.
3. The shared spec-coding contract used by all eight goals.
4. The eight bounded goal specs, each with scope, non-goals, acceptance,
   verification, and stop conditions.
5. The cross-goal sequencing rules that prevent later product loops from
   entering the kernel too early.

This preparation branch is successful only if a future goal-mode run can start
from a single goal spec and know what to build, what not to build, what evidence
to produce, and when to stop.

## 2. OpenLife Lessons To Preserve

The rescue must absorb these lessons from the current repository:

- Main Chat cannot keep two runtime implementations for send and stream.
- A readiness gate cannot substitute for a usable agent loop.
- HS is valuable, but it should not make ordinary chat responsible for answer,
  learning, materialization, proposal, and governance at the same time.
- Tool execution foundations are useful only when surfaced through a simple
  model-action-observation-answer loop.
- Proposal-first governance is a product advantage, but only after the agent
  can reliably identify the action and produce inspectable evidence.
- Product UI must show what happened, not expose internal readiness machinery as
  the main experience.
- Live-provider and provider-ranked MCP proof should validate a stable kernel,
  not become prerequisites for basic local behavior.

## 3. Architecture Boundary For The Rescue

The rescue architecture is:

```text
Transport command
  -> MainChatKernel adapter
  -> bounded context
  -> model route/generation
  -> optional governed action
  -> observation
  -> final answer, proposal, permission interruption, or blocker
  -> event/result surfaces for UI and tests
```

The kernel must not own every OpenLife feature. It owns the turn loop. HS,
proposal stores, permissions, tools, MCP, web, and UI are attached to the kernel
in later goals through narrow contracts.

## 4. Preparation Artifacts

The preparation set is:

- `plans/main_chat_agent_kernel_rescue_preparation.md`
- `plans/main_chat_agent_kernel_rescue_preparation_scope.md`
- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `plans/main_chat_agent_kernel_rescue_goal_completion_template.md`
- `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`
- `plans/main_chat_agent_kernel_rescue_goal_1_kernel_foundation.md`
- `plans/main_chat_agent_kernel_rescue_goal_2_send_stream_convergence.md`
- `plans/main_chat_agent_kernel_rescue_goal_3_read_only_tools.md`
- `plans/main_chat_agent_kernel_rescue_goal_4_proposal_only_writes.md`
- `plans/main_chat_agent_kernel_rescue_goal_5_execution_ux.md`
- `plans/main_chat_agent_kernel_rescue_goal_6_hs_reintegration.md`
- `plans/main_chat_agent_kernel_rescue_goal_7_web_mcp_provider.md`
- `plans/main_chat_agent_kernel_rescue_goal_8_cleanup_final_gate.md`

## 5. Development Readiness Bar

Do not start Goal 1 until:

- this preparation set exists and is linked from `plans/README.md`;
- each goal has an objective that can be marked pass/fail;
- each goal has allowed scope and out-of-scope sections;
- each goal names verification commands;
- each goal names stop conditions;
- the full K0-K8 acceptance matrix exists;
- the completion report template exists;
- the industry-practice digest exists and is explicitly mapped to the rescue;
- `git diff --check` passes.
