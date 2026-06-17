# Main Chat Skills Hub Product Contract v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines the minimal Skills/Tools product surface for Main Chat.

This is not a marketplace. It is a local, policy-bound product surface that lets
the user and agent inspect, select, explain, and execute skills/tools under
OpenLife governance.

## 2. Baseline

OpenLife already has:

- `SKILL.md` selected context plumbing,
- bounded context loader,
- Skill Runtime foundations,
- MCP read candidate selection,
- ExecutionPolicy,
- ToolPermission proposals,
- plugin manifest boundaries.

Missing:

- user-facing skill/tool list,
- skill detail page/panel,
- selected skill explanation,
- tool candidate explanation,
- policy-bound invocation affordance,
- skill/tool result in Agent Control Plane.

## 3. Benchmark Lessons

### Codex-style lesson

Skills are understandable when they are file-backed and scoped. The agent should
load selected skill instructions intentionally, not blindly inject every skill.

### Hermes/OpenClaw-style lesson

Tools should feel like normal execution primitives. Users should see what the
agent chose and why.

### OpenLife constraint

Skills and tools are not authority. Privacy, model route, ExecutionPolicy, and
ToolPermission remain higher priority.

## 4. Product Objects

### 4.1 SkillSummary

Required fields:

- `skillId`
- `name`
- `source`
- `scope`
- `description`
- `riskLevel`
- `available`
- `selected`
- `instructionDigest`
- `sourceKind`: `global`, `workspace`, `project`, or `bundled`
- `lastUsedAt`

### 4.2 SkillDetail

Required fields:

- `skillId`
- `manifest`
- `boundedInstructionsPreview`
- `allowedTools`
- `disallowedTools`
- `policyNotes`
- `requiredPermissions`
- `evidenceDigest`
- `redactionSummary`
- `lastModifiedAt`

### 4.3 ToolCandidate

Required fields:

- `candidateId`
- `toolName`
- `source`
- `capabilityLabels`
- `riskLevel`
- `selectionReason`
- `policyDecision`
- `requiresPermission`
- `candidateDigest`
- `linkedActionId`

## 5. UI Contract

Main Chat should support:

- selected skill field in composer,
- skill/tool panel in Agent Control Plane,
- candidate list for tool-required tasks,
- selected tool explanation,
- permission state,
- result/observation linkage,
- blocker when tool is unsafe or unavailable.

Skill Hub v1 should support:

- list local skills,
- inspect a skill,
- select a skill for current chat/task,
- clear selected skill,
- show why a selected skill was loaded.

Selection scope:

- A selected skill is session/task scoped by default.
- Global default skill selection is out of scope for this contract.
- `skillId` must be stable across runs for the same source path and bounded
  manifest identity.
- The selected skill digest must be recorded in task trace and long-task stale
  diagnostics.
- If a selected skill file changes after task creation, continuation must mark
  the task stale until the user refreshes or confirms.

## 6. Execution Rules

- Unselected `SKILL.md` must not be injected.
- Selected `SKILL.md` must be bounded and sanitized.
- Skill instructions cannot override system/developer policy.
- Skill instructions cannot override privacy/model/tool policy.
- Tool candidates must be allowlisted before model selection.
- Write-like or unsafe tools must create permission/proposal/blocker.
- Raw unsafe manifest details must not be exposed to the model or UI.
- Skill preview must redact or omit secrets, API keys, hidden policy text, and
  oversized instructions.

## 7. Commands

Minimum commands:

- `list_main_chat_skills()`
- `get_main_chat_skill_detail(skillId)`
- `select_main_chat_skill(sessionId, skillId)`
- `clear_main_chat_skill(sessionId)`
- `list_main_chat_tool_candidates(taskSessionId)`

Existing selected skill plumbing may be reused.

Commands must return enough metadata for UI and eval to prove bounded preview,
selection reason, digest, and policy status. A command that only writes a
selected skill id without trace evidence is insufficient.

## 8. Eval Scenarios

Minimum scenarios:

- selected skill loads bounded context,
- unselected skill is not loaded,
- skill detail shows safe preview,
- tool candidate list shows reasons,
- read-only tool executes under policy,
- write-like tool becomes blocker/proposal,
- unsafe manifest is excluded,
- tool failure shows retry/alternative if safe.

## 9. Acceptance

This contract is satisfied when:

- users can inspect and select local skills,
- selected skill is visible in task trace,
- tool choice is explained,
- tool execution remains governed,
- unsafe tools are blocked or proposal-first,
- no skill bypasses policy.

## 10. Stop Conditions

Stop if:

- implementing skill list requires marketplace-scale work,
- selected skill cannot be bounded,
- tool candidates would expose unsafe raw manifest data,
- skill instructions would override privacy or ExecutionPolicy,
- UI would imply a tool is executable before policy allows it.
