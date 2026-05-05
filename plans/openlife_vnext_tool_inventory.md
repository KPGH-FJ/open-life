# OpenLife vNext Tool Inventory and Enforcement Spec

Date: 2026-05-06  
Status: P0-4 deliverable (spec only, no code changes)  
Related: `plans/adr/0003-toolruntime-metadata.md`, `plans/openlife_vnext_p0_p1_task_specs.md`

---

## 1. Complete Tool Inventory

All tools registered in `openlife-core/src/mcp.rs::register_builtins()`. Source code handler files: `action_executor/core_os_tools.rs`, `action_executor/execution_tools.rs`, `action_executor/tool_executor.rs`.

### 1.1 Core OS Tools (tag: `core_os`)

| # | Tool Name | Source | Executable | Declarative-Only | Risk Level | Permission Behavior | Executor Source | Proposal Behavior | Model-Callable | Metadata Gaps | vNext Enforcement |
|---|-----------|--------|------------|------------------|-----------|---------------------|------------------|-------------------|---------------|---------------|-------------------|
| 1 | `life_model.read` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | No `goal.read` alias for scoped access | OK as-is |
| 2 | `tool.list_available` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | Exposes all tool metadata to model | OK as-is |
| 3 | `goal.read` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 4 | `state.read` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 5 | `memory.search` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 6 | `proposal.list` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 7 | `agent_run.lookup` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 8 | `permission.check` | BuiltIn | ✅ P1 | ❌ | low | allow (read) | `core_os_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 9 | `permission.request` | BuiltIn | ✅ P1 | ❌ | medium | allow (proposal-generating) | `core_os_tools.rs` | Creates `ToolPermission` Proposal | ✅ yes | — | OK as-is |
| 10 | `permission.replay_action` | BuiltIn | ✅ P1 | ❌ | medium | allow (write) | `core_os_tools.rs` (self-executes via `self.execute()`) | N/A (executes action) | ✅ yes | Re-entrant execution through ActionExecutor | OK as-is |
| 11 | `life_model.propose_patch` | BuiltIn | ✅ P1 | ❌ | **high** | proposal-generating (always allowed) | `core_os_tools.rs` | Creates `LifeModelUpdate` Proposal | ✅ yes | None | **High-risk: consider requiring user confirmation in tools prompt** |
| 12 | `memory.propose_write` | BuiltIn | ✅ P1 | ❌ | medium | proposal-generating (always allowed) | `core_os_tools.rs` | Creates `MemoryWrite` Proposal | ✅ yes | — | OK as-is |
| 13 | `memory.propose_archive` | BuiltIn | ✅ P1 | ❌ | medium | proposal-generating (always allowed) | `core_os_tools.rs` | Creates `MemoryArchive` Proposal | ✅ yes | — | OK as-is |

### 1.2 Execution Tools (tag: `execution`)

| # | Tool Name | Source | Executable | Declarative-Only | Risk Level | Permission Behavior | Executor Source | Proposal Behavior | Model-Callable | Metadata Gaps | vNext Enforcement |
|---|-----------|--------|------------|------------------|-----------|---------------------|------------------|-------------------|---------------|---------------|-------------------|
| 14 | `file.read` | BuiltIn | ✅ P1 | ❌ | low | allow (read, safe_paths enforced) | `execution_tools.rs` | N/A (read-only) | ✅ yes | — | OK as-is |
| 15 | `file.write_proposal` | BuiltIn | ✅ P1 | ❌ | **high** | proposal-generating (always allowed) | `execution_tools.rs` | Auto-creates `ExternalWriteAction` Proposal | ✅ yes | None | **High-risk: requires safe_paths + 100KB limit** |
| 16 | `web.fetch` | BuiltIn | ✅ P1 | ❌ | medium | allow (network, domain policy checked) | `execution_tools.rs` | N/A (read-only) | ✅ yes | Requires `NetworkPolicy` context | Policy enforces domain allow/denylist |
| 17 | `web.search` | BuiltIn | ✅ P1 | ❌ | medium | allow (network, rate-limited) | `execution_tools.rs` | N/A (read-only) | ✅ yes | Tag: `web`; uses DuckDuckGo/Brave/SearXNG | Policy enforces network toggle |
| 18 | `mcp.call_tool` | BuiltIn | ✅ P1 | ❌ | medium | allow (wrapper; perm falls on target tool) | `execution_tools.rs` + `tool_executor.rs` | N/A (delegates to target) | ✅ yes | Tag: `mcp_wrapper` | OK as-is |
| 19 | `calendar.read` | BuiltIn | ✅ P1 | ❌ | low | allow (read, ics_paths safe) | `execution_tools.rs` | N/A (read-only) | ✅ yes | Requires `calendar_ics_paths` context | OK as-is |
| 20 | `calendar.propose_event` | BuiltIn | ✅ P1 | ❌ | medium | proposal-generating (always allowed) | `execution_tools.rs` → `declarative_stubs.rs` | Creates `ScheduledTask` Proposal | ✅ yes | Executed via `create_declarative_stub_proposal` because `declarative_only=false` | **INCONSISTENCY:** This tool is registered as executable (`declarative_only: false`) but its only handler is in `declarative_stubs.rs`. P1-1 should clarify. |
| 21 | `email.read` | BuiltIn | ❌ P2 | ✅ declarative-only | low | blocked (declarative-only) | `declarative_stubs.rs` | N/A | ❌ **SHOULD NOT** be callable | Tag: `stub`; `declarative_only: true` | **P1-1 enforce: must not be model-callable** |
| 22 | `email.propose_draft` | BuiltIn | ✅ P1 | ❌ | medium | proposal-generating (always allowed) | `execution_tools.rs` | Creates `DataExport` Proposal + mailto: link | ✅ yes | — | OK as-is |
| 23 | `task.create_proposal` | BuiltIn | ✅ P1 | ❌ | medium | proposal-generating (always allowed) | `execution_tools.rs` | Creates `ScheduledTask` Proposal | ✅ yes | — | OK as-is |
| 24 | `a2a.call_agent` | BuiltIn | ✅ P1 | ❌ | medium | allow (network, 30s timeout, private IP block) | `execution_tools.rs` | N/A | ✅ yes | — | OK as-is |

### 1.3 Declarative-Only Stub

| # | Tool Name | Source | Executable | Declarative-Only | Risk Level | Permission Behavior | Executor Source | Proposal Behavior | Model-Callable | Metadata Gaps | vNext Enforcement |
|---|-----------|--------|------------|------------------|-----------|---------------------|------------------|-------------------|---------------|---------------|-------------------|
| 25 | `snapshot.create` | BuiltIn | ❌ P2 | ✅ declarative-only | low | **blocked** (declarative-only) | `declarative_stubs.rs` | N/A | ❌ **SHOULD NOT** be callable | `declarative_only: true` but may still appear in tools prompt | **P1-1: must filter from ToolPrompt + block at runtime** |

---

## 2. Summary Statistics

| Category | Count | Model-Callable | Not Model-Callable |
|----------|-------|----------------|---------------------|
| Core OS Tools | 13 | 13 | 0 |
| Execution Tools | 11 | 10 | 1 (`email.read`) |
| Declarative Stubs | 1 | 0 | 1 (`snapshot.create`) |
| **Total** | **25** | **23** | **2** |

### 2.1 Risk Distribution

| Risk Level | Count | Tools |
|-----------|-------|-------|
| **high** | 2 | `life_model.propose_patch`, `file.write_proposal` |
| **medium** | 11 | `permission.request`, `permission.replay_action`, `memory.propose_write`, `memory.propose_archive`, `web.fetch`, `web.search`, `mcp.call_tool`, `calendar.propose_event`, `email.propose_draft`, `task.create_proposal`, `a2a.call_agent` |
| **low** | 12 | All read-only core OS tools + `calendar.read`, `file.read`, `snapshot.create` (stub) |

### 2.2 Tool Registration Methods

| Registration Method | Tags Added | Count |
|--------------------|------------|-------|
| `register_core_os_tool` | `["core_os"]` | 13 |
| `register_execution_tool` | `["execution"]` | 10 |
| `register_declarative_stub` | `["execution", "stub"]` | 1 |
| `register_builtin` (raw) | `["execution", "web"]` / `["execution", "mcp_wrapper"]` | 2 |

Routing in `tool_executor.rs`:
- `tags.contains("core_os")` → `execute_core_os_tool()`
- `tags.contains("execution")` → `execute_execution_tool()` (includes stubs if not filtered first)
- Otherwise → `call_tool_internal()`

---

## 3. Metadata Gaps Identified

### 3.1 Missing `input_schema` on Many Tools

Core OS tools registered via `register_core_os_tool` use an empty `parameters: {"type": "object", "properties": {}}`. The actual input is hardcoded in the handler. This is **acceptable for Beta** but should be filled in before P2.

**Affected tools (13):** All `core_os` tools.

### 3.2 Missing `output_schema` on All Tools

No tool currently declares an output schema. The output format is implicit in the handler code. This is **not blocking for P0/P1**.

### 3.3 `calendar.propose_event` Uses `declarative_stubs.rs`

This tool is registered with `declarative_only: false` and `executable: true`, but its actual execution handler (`create_declarative_stub_proposal`) is in `declarative_stubs.rs`. The tool does create a real Proposal, so it is genuinely executable — but its handler location is semantically misleading.

**Recommendation:** Move the `calendar.propose_event` and `email.propose_draft` (which follows the same pattern) logic into `execution_tools.rs` for consistency, or rename `declarative_stubs.rs` to `proposal_stubs.rs`.

### 3.4 `email.read` Is P2 but NOT Filtered from ToolPrompt

ADR 0003 mandates: "declarative-only tools must never enter the model-callable tools prompt." `email.read` has `declarative_only: true` but the `tools_prompt()` method in `McpRegistry` does not filter by this flag. This is a **P1-1 enforcement gap**.

### 3.5 `snapshot.create` Is Declarative-Only but Retained

Same issue as `email.read`. `declarative_only: true` but no prompt filtering. This tool should be invisible to models.

### 3.6 No `permission_policy` Field in ToolManifest

ADR 0003 requires a `permission_policy` field. The current `ToolManifest` has `permission_level` (string "low"/"medium"/"high") and `requires_confirmation` (bool), but no structured permission policy. ADR 0003 calls for:
- `executable`
- `declarative_only`
- `risk_level`
- `permission_policy`
- `executor_kind`
- `input_schema`
- `output_schema`
- `side_effect_type`

Of these, `permission_policy` and `side_effect_type` are **not yet in the ToolManifest struct**.

### 3.7 `proposal.create` (AGENTS.md Table) Not Registered

The AGENTS.md tool taxonomy lists `proposal.create` as a P1 Core OS Tool, but it is **not registered** in `mcp.rs::register_builtins()`. This is either a doc bug or an unimplemented tool.

---

## 4. Enforcement Recommendations for P1-1

### 4.1 Prompt-Level Filtering (ToolPrompt)

```rust
// In mcp.rs::tools_prompt()
fn tools_prompt(&self) -> String {
    let manifests: Vec<_> = self.list_manifests()
        .into_iter()
        .filter(|m| m.enabled && !m.declarative_only) // ← ADD THIS
        .collect();
    // ... format as tool prompt
}
```

### 4.2 Runtime Enforcement (ActionExecutor)

Already partially implemented in `tool_executor.rs`:
```rust
if !manifest.enabled || manifest.declarative_only {
    // blocked
}
```

But the check should be more explicit:

```rust
if manifest.declarative_only {
    return Err(anyhow::anyhow!("Tool '{}' is declarative-only and cannot be executed", tool_name));
}
```

### 4.3 Minimum Metadata Validation

On tool registration, validate:
1. `declarative_only` and `executable` are mutually consistent (declarative ⇒ not executable)
2. `risk_level` is one of: low, medium, high, critical
3. `source` is populated
4. `capabilities` list is not empty

### 4.4 Recommended P1-1 Action Items

1. **Add `prompt-level declarative-only filtering`** to `McpRegistry::tools_prompt()`.
2. **Add runtime declarative-only hard block** in `ActionExecutor::execute_tool()`.
3. **Add `ToolManifest::permission_policy` field** (e.g., `"always_allow" | "confirm" | "deny"`).
4. **Move `calendar.propose_event` handler** out of `declarative_stubs.rs` or rename the file.
5. **Add test:** declarative-only tool filtered from tools prompt.
6. **Add test:** declarative-only tool blocked at runtime.

---

## 5. Open Questions

1. Should `life_model.propose_patch` (high risk) remain model-callable, or be restricted to Builder/Calibration modes only?
2. Should `permission.replay_action` have its own risk classification separate from the replayed tool?
3. Should MCP tools be normalized into the same metadata model before prompt injection?
4. Should `tool.list_available` expose `declarative_only` status to the model?

---

*End of P0-4 deliverable.*
