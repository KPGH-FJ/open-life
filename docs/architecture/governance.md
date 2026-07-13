# Governance

## Status

Stage3-A source-backed explainer. This document describes current governance
surfaces for privacy, model routing, tool manifests, MCP, proposals, and task
controls. It does not promote docs, tests, or local proofs into runtime
readiness.

## Authority

Authority remains with `AGENTS.md`, `plans/README.md`,
`plans/openlife_single_system_deletion_manifest.md`,
`plans/openlife_single_system_development_preparation.md`, and
`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`.

`docs/repository_document_governance.md` governs which repository documents are
public, historical, active, or local-only. This page follows that public
document rule set.

## Last verified

2026-07-11 during Phase7 Proposal-authority deletion review.

## Source map

- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `docs/repository_document_governance.md`
- `openlife-core/src/privacy.rs`
- `openlife-core/src/tool_permissions.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/mcp.rs`
- `openlife-core/src/mcp_audit.rs`
- `openlife-core/src/agent/tool_gateway.rs`
- `openlife-core/src/agent/model_router.rs`
- `openlife-core/src/agent/review_workflow.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/provider_network_consent.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_task_control_tests.rs`
- `src-tauri/src/commands/mcp.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/commands/proposal.rs`

## Inherited blocker

Governance docs cannot close the runtime-module blocker, complete external live
provider evidence, or authorize retired routes. They can only explain existing
source-backed guardrails.

## Document Governance

`docs/repository_document_governance.md` separates public entry points, stable
architecture docs, current execution plans, historical plans, and local/private
planning. It explicitly requires status labeling for scoped or historical docs
and excludes raw LifeModel, raw memory, sensitive chat, credentials, private
provider endpoints, and unpublished private strategy from public docs by
default.

For Stage3-A, that means these architecture pages are public explanatory docs.
They are not the current plan authority and do not carry proof artifacts beyond
their source map and validation record.

## Privacy And Model Route Governance

`openlife-core/src/privacy.rs` defines a configurable privacy policy and
privacy engine. Defaults mask phone, email, address, name, and generic sensitive
types when enabled, while ID-card and bank-card style findings default to
blocking. Unknown configured sensitive types fail toward masking.

`openlife-core/src/agent/model_router.rs` routes model usage through provider,
task, and privacy constraints. HS policy packets can force LocalOnly routing,
and high/critical privacy levels hard-filter cloud providers to local routes.
This is a routing guard, not a guarantee that a provider has been invoked.

## Tool Manifest And Permission Governance

`openlife-core/src/tool_manifest.rs` defines the unified tool manifest shape:
id, name, parameters, permission level, risk level, source, capabilities,
confirmation requirement, enabled/declarative state, action type, and tags.
Manifest normalization derives confirmation requirements from risk and
write-like capabilities.

`openlife-core/src/tool_permissions.rs` persists permission policies in SQLite.
Without a stored policy, low-risk read actions can be allowed, while high-risk
or write-capability actions require confirmation. `AllowOnce` permissions are
consumed on check, while replay prechecks can use `peek` without consuming the
permission.

`openlife-core/src/agent/tool_gateway.rs` validates gateway requests before
execution. Plugin and A2A sources are rejected as executor-unavailable, disabled
or declarative-only manifests are blocked, and manifest contracts must include
known permission, risk, action type, capability, and parameter fields. Gateway
execution attaches contract evidence to successful observations.

## MCP Governance

`openlife-core/src/mcp.rs` registers built-in tools, external MCP server tools,
and manifest-only capabilities. MCP arguments are inspected for privacy findings
and can require confirmation based on PII, permission level, and whether the
tool is built in.

`src-tauri/src/commands/mcp.rs` only allows MCP server commands from a small
bare-command allowlist and rejects paths or shell syntax. It exposes server and
manifest listing plus audit log commands.

`openlife-core/src/mcp_audit.rs` stores encrypted MCP audit entries with
key-configuration support, export, cleanup, and key rotation. Audit existence
does not by itself mean a Main Chat AgentLoop scenario has live provider credit.

## Proposal Governance

The dormant `src-tauri/src/main_chat_proposal_support.rs` parallel route is
deleted. Active Main Chat proposal orchestration lives in
`src-tauri/src/main_chat_kernel.rs`; tool-governance proposals enter the typed
ReviewWorkflow gateway owned by ActionExecutor. Provider network-consent
staging is an active separate callsite and remains under execution-epoch
admission remediation; it must not be cited as cancellation-safe until that
gate is complete.

`src-tauri/src/commands/proposal.rs` validates proposal payloads before
application. Accepting ToolPermission proposals records a permission policy.
Accepting Memory and LifeModel proposals delegates to the relevant gateway.
Editing Memory proposals is draft-only and preserves provenance.

## Task Control Governance

`src-tauri/src/main_chat_task_controls.rs` computes task summaries, details,
run evidence views, continuity diagnostics, and allowed controls. Resume and
retry are blocked by terminal state, stale context, missing action evidence,
permission mismatch, unavailable tool/provider state, selected-skill digest
mismatch, and plan revision mismatch.

The task-control tests in `src-tauri/src/main_chat_task_control_tests.rs`
enforce that refresh is evidence-backed, non-replayable retry becomes a manual
blocker, accepted ToolPermission can permit governed replay, and cancel stops
nonterminal queued actions.
