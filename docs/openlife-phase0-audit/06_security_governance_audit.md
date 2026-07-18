# Security and Governance Audit

## Privacy Engine

Finding: The core privacy engine detects and masks or blocks configured
sensitive data types.

Evidence:

- `PrivacyPolicy` defaults include phone, ID card, email, bank card, address,
  name, and generic rules.
- ID card and bank card default to `Block`.
- `desensitize_strict` returns an error for block-level findings.
- `desensitize_secrets_only` redacts credential-like secrets without storing raw
  secret values in the reconstruction map.

File location:

- `openlife-core/src/privacy.rs`

Confidence: High.

Impact: Privacy primitives exist and should be preserved in v2 as user-visible
trust controls.

## Tool Governance

Finding: Tool execution is guarded by both manifest contract checks and
permission policy checks.

Evidence:

- `ToolGateway` blocks missing/inferred/declarative/disabled/incomplete
  manifest contracts.
- `ToolPermissionStore` supports persistent policies including allow-once
  consumption and peek without consumption.
- Kernel read tool execution uses `ActionExecutorConfig { allow_writes: false,
  allow_cloud: false }`.

File location:

- `openlife-core/src/agent/tool_gateway.rs`
- `openlife-core/src/tool_permissions.rs`
- `src-tauri/src/main_chat_kernel.rs`

Confidence: High.

Impact: Frontend should expose why a tool is available, blocked, waiting for
permission, or proposal-only.

## Dangerous Actions

Finding: High-risk settings actions have preflight and typed confirmation
evidence.

Evidence:

- `DangerActionPreflightView` includes risk tier, scope digest, privacy
  sensitivity, external transmission, backup status, confirmation phrase,
  affected item count/digest, safe-mode blocking, and source refs.
- Supported action types include data import overwrite, MCP audit cleanup, MCP
  audit key rotation, agent-run deletion, bulk deletion, and vector rebuild.

File location:

- `src-tauri/src/commands/settings.rs`

Confidence: High.

Impact: V2 should keep danger preflight as a common component pattern.

## File and External Writes

Finding: File write proposals are applied through safe-path validation and
atomic-ish temp write behavior.

Evidence:

- `safe_write_utf8` rejects symlinks, requires canonical parent inside safe
  paths, writes to a temp file, syncs, revalidates, and renames.
- Mailbox disables accepting external write proposals when target path is not
  under safe paths.

File location:

- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/MailboxPage.tsx`

Confidence: High.

Impact: This is a strong safety foundation for user-approved local file writes.

## MCP Audit

Finding: MCP audit logs are encrypted in SQLite with key-epoch support and
export/cleanup operations.

Evidence:

- `McpAuditStore` uses AES-256-GCM, stores encrypted arguments/results, supports
  key config and keyring rotation, and exports decrypted logs by retention
  window.

File location:

- `openlife-core/src/mcp_audit.rs`

Confidence: High.

Impact: Audit data can be productized, but exports are privacy-sensitive and
need clear UI copy.

## Governance Gaps

Finding: Governance is real but not perfectly unified.

Evidence:

- `ReviewWorkflow` exists, but direct `ProposalStore::create_proposal` callsites
  still exist by inventory.
- `MemoryGateway` exists, but low-level stores still expose broad write APIs.
- `LifeModelWriteGateway` exists, but manual and state command paths must stay
  under explicit governed caller contexts.

File location:

- `openlife-core/src/agent/review_workflow.rs`
- `openlife-core/src/memory.rs`
- `openlife-core/src/memory_gateway.rs`
- `openlife-core/src/life_model_write_gateway.rs`
- `src-tauri/src/single_system_authority_tests.rs`

Confidence: High.

Impact: Frontend rewrite should not imply governance is solved. It should make
pending, accepted, applied, blocked, and manual override states visible.
