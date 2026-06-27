# Main Chat Kernel Rescue Goal Completion Report Template

> Goal:
> Branch:
> Date:
> Base commit:
> Final commit:
> Author/agent:

## Objective

Copy the exact objective from the goal spec.

## Scope Actually Changed

List changed files and classify each change:

| File | Change type | Why it was needed |
| --- | --- | --- |
|  |  |  |

## Acceptance Checklist

Copy the goal checklist and mark each item:

- [ ]

## Acceptance Matrix Rows

List the K-row IDs satisfied by this goal:

| ID | Evidence |
| --- | --- |
|  |  |

## Verification Commands

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p openlife-core` |  |  |
| `cargo check -p openlife-tauri` |  |  |

If a command was not run, explain why and whether that blocks completion.

## Safety Evidence

| Invariant | Evidence |
| --- | --- |
| No silent durable LifeModel/Memory write |  |
| No unsafe file/calendar/email/provider/plugin/shell side effect |  |
| Unsupported capabilities fail closed |  |
| Send/stream parity preserved where applicable |  |
| UI claims backed by runtime evidence where applicable |  |

## Legacy/Fallback Evidence

Record whether legacy fallback was used:

```text
legacy_fallback_used:
legacy_fallback_count:
why_still_needed:
```

## Direct Write Evidence

Record direct-write status:

```text
direct_writes_executed:
direct_write_count:
proposal_or_permission_records:
```

## Source And Practice Consistency Check

Confirm the implementation does not conflict with:

- `plans/main_chat_agent_kernel_rescue_industry_practices.md`
- `plans/main_chat_agent_kernel_rescue_spec_coding_contract.md`
- `plans/main_chat_agent_kernel_rescue_acceptance_matrix.md`
- `AGENTS.md`

If a conflict exists, record the decision and whether user approval is needed.

## Residual Risk

List unresolved risks and whether they block the next goal:

| Risk | Blocks next goal? | Follow-up |
| --- | --- | --- |
|  |  |  |
