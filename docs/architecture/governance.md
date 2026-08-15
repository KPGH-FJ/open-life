# Governance

## Status

Source-backed description of current privacy, routing, tool, proposal, and task
control boundaries.

## Authority

Authority remains with `PRODUCT.md`, `AGENTS.md`, accepted ADRs, and current
source.

`docs/repository_document_governance.md` governs which repository documents are
public, historical, active, or local-only. This page follows that public
document rule set.

## Last verified

2026-07-31 during repository cleanup source tracing.

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
- `src-tauri/src/canonical_work_runtime.rs`
- `src-tauri/src/provider_network_consent.rs`
- `src-tauri/src/read_models/tasks.rs`
- `src-tauri/src/commands/mcp.rs`
- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/commands/proposal.rs`

## Evidence Boundary

This document explains guardrails. It does not prove a provider invocation,
durable write, or product-readiness state.

## Document Governance

`docs/repository_document_governance.md` separates public entry points, stable
architecture docs, current execution plans, historical plans, and local/private
planning. It explicitly requires status labeling for scoped or historical docs
and excludes raw LifeModel, raw memory, sensitive chat, credentials, private
provider endpoints, and unpublished private strategy from public docs by
default.

These architecture pages are explanatory documents, not execution plans or
proof artifacts.

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
execution. Plugin sources are rejected as executor-unavailable, disabled or
declarative-only manifests are blocked, and manifest contracts must include
known permission, risk, action type, capability, and parameter fields. Gateway
execution attaches contract evidence to successful observations. Only
registered built-in and external MCP sources can enter the executor graph.

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
does not by itself mean a canonical Work scenario has live provider credit.

## Proposal Governance

Main Chat proposal orchestration lives in `src-tauri/src/main_chat_kernel.rs`;
tool-governance proposals enter the typed ReviewWorkflow gateway owned by
ActionExecutor. Provider network-consent staging uses the same canonical Work
execution epoch and never grants completion credit by itself.

`src-tauri/src/commands/proposal.rs` validates proposal payloads before
application. Accepting ToolPermission proposals records a permission policy.
Accepting Memory and LifeModel proposals delegates to the relevant gateway.
Editing Memory proposals is draft-only and preserves provenance.

## Task Control Governance

`src-tauri/src/canonical_work_runtime.rs` owns cancellation, retry, recovery,
scope revalidation, and Work continuation. `src-tauri/src/read_models/tasks.rs`
projects the resulting controls and Needs Attention facts. Retry creates a new
Run for the same Task and is blocked by stale Project scope or non-recoverable
effect uncertainty; React does not infer controls from messages.
