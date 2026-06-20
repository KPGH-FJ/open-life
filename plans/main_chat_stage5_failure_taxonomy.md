# Main Chat Stage 5 Failure Taxonomy

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation taxonomy

## 1. Purpose

Internal testers need stable failure classes. Without a taxonomy, every problem
becomes a vague "Agent failed" note and cannot become a useful regression.

## 2. Failure Classes

| Code | Class | Meaning | Required evidence | Recovery recommendation |
| --- | --- | --- | --- | --- |
| `routing_failure` | Strategy routing | Request used the wrong strategy or legacy path. | route decision, expected strategy, actual strategy, fallback flag. | Re-run with trace, file routing bug, do not mark tool/memory as failed. |
| `environment_preflight_failure` | Environment preflight | Workspace, safe path, scheduler, network, MCP registry, database, or store readiness is missing or invalid before task execution. | preflight report, workspace/safe-path digest, scheduler/network/MCP/database/store blockers, no unintended provider invocation. | Fix local environment/configuration first; do not mark Agent/provider quality as failed. |
| `provider_failure` | Provider/model call | Model/provider unavailable, rejected, timed out, malformed, or missing key. | provider preflight, provider/model labels, invocation attempt flag, blocker. | Fix provider env/network or retry after provider recovery. |
| `tool_selection_failure` | Tool choice | Candidate set, allowlist, ranking, or model-selected tool was wrong. | candidates, allowlist, selected tool, policy decision, ranking ignored flag. | File tool-selection regression; do not bypass allowlist. |
| `tool_execution_failure` | Tool execution | Tool selected correctly but execution failed. | action id, action type, target, executor status, observation/error. | Retry if safe, otherwise file tool/runtime bug. |
| `policy_blocker` | Governance block | Privacy, permission, network, high-risk write, or external action was blocked. | ExecutionPolicy decision, risk level, confirmation/proposal state. | Ask for confirmation or adjust allowed config; do not mark as model failure. |
| `memory_context_failure` | Memory context | Accepted memory missing, rejected/rolled-back memory used, or wrong memory surfaced. | active/excluded memory ids, lifecycle status, context inventory, answer trace. | File memory/context regression; rollback or rebuild materialized view if offered. |
| `knowledge_asset_failure` | Knowledge file | `USER.md`/`MEMORY.md`/`SKILL.md` load/write/rollback/inventory failed. | asset id, digest, target path, validation, version/audit ids. | Regenerate managed draft, rollback file version, or file asset bug. |
| `final_delivery_failure` | Delivery contract | Final answer hides completed/proposed/blocked/skipped/pending state. | final delivery object, transcript, proposals, durable changes. | File final-delivery product bug. |
| `ui_state_failure` | UI mismatch | Backend state exists but UI does not show it or controls are wrong. | backend snapshot id, UI state, missing/incorrect visible control. | File frontend/state mapping bug. |
| `recovery_failure` | Retry/resume/cancel | Recovery control is missing, unsafe, or behaves incorrectly. | task status, blocker, action queue, retry/resume/cancel result. | File task-control bug; do not manually mutate state. |
| `redaction_failure` | Privacy/export | Bundle or UI leaks secrets/raw private content. | bundle id, redaction report, unsafe field label. | Stop testing; fix redaction before more exports. |
| `release_artifact_failure` | Build/report artifact | Missing/stale build, invalid scenario id, unknown reviewer, missing bundle. | build info, artifact validator output, report blockers. | Re-run on known build and regenerate report. |
| `unknown_failure` | Unclassified | Failure cannot be mapped safely. | raw blocker digest, task id, bundle id. | Mark as unknown and triage; do not convert to pass. |

## 3. Severity

| Severity | Definition |
| --- | --- |
| `p0` | Blocks internal testing, leaks secrets, corrupts durable state, or weakens readiness/final/live gates. |
| `p1` | Breaks a core Stage 1-4 path or makes debug evidence unusable. |
| `p2` | Confusing UI/reporting but workaround exists and evidence remains valid. |
| `p3` | Polish issue or non-blocking copy/layout problem. |

## 4. Recoverability

| Recoverability | Meaning |
| --- | --- |
| `retry_safe` | Same action can be retried without extra permission. |
| `needs_user_confirmation` | User must approve/provide missing input. |
| `needs_environment_fix` | Provider/key/network/workspace setup must change. |
| `needs_developer_fix` | Product/runtime bug; tester should stop or file issue. |
| `terminal_expected` | Correct policy blocker; no bug unless UX is unclear. |

## 5. Mapping Rules

- Prefer specific classes over `unknown_failure`.
- A policy blocker is not a failure if the requested action is correctly
  forbidden; it may still be a UX issue if the explanation is unclear.
- A provider setup blocker is not an Agent quality failure.
- Environment preflight blockers that are not provider/model specific should map
  to `environment_preflight_failure`, not `provider_failure`.
- A redaction failure is always P0.
- A readiness overclaim is always P0.
- A UI state mismatch should reference both backend evidence and expected
  visible control.
