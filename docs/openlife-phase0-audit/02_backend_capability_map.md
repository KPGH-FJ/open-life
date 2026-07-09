# Backend Capability Map

## Capability Table

| Capability | Classification | Finding | Evidence | Impact |
| --- | --- | --- | --- | --- |
| Main Chat send/stream runtime | `PARTIAL` | Send and stream share `OpenLifeTurnRuntime`, but the runtime still delegates into the Main Chat kernel. | `src-tauri/src/main_chat_send.rs`, `src-tauri/src/main_chat_streaming.rs`, `src-tauri/src/main_chat_turn_runtime.rs`, `src-tauri/src/main_chat_kernel.rs` | Runtime convergence exists, but the product spine is still kernel-centered and not finished. |
| Agent ingress and policy route | `PARTIAL` | `IntentFrame`, `PolicyRouter`, `PolicyRouteKind`, and `AgentIngressDecision` exist, but semantic classification is mostly deterministic keyword logic. | `openlife-core/src/agent/main_chat_agent_v1.rs` | Good authority shape, weak AI-native understanding. |
| Planner / plan execution | `PARTIAL` | Plan-execute commands and session types are exposed through the Tauri bridge and runtime, but this audit did not verify end-to-end desktop plan execution. | `src-tauri/src/commands/agent_runtime/plan_execute_product.rs`, `frontend/src/tauri.ts`, `openlife-core/src/agent/plan_execute.rs` | Treat as implemented primitives, not fully proven user journey. |
| Tool execution gateway | `EXISTING` | `ToolGateway` validates manifest identity, risk, permission, action type, capabilities, parameters, and blocks inferred contracts from execution credit. | `openlife-core/src/agent/tool_gateway.rs` | Strong current authority for execution gating. |
| Action executor | `EXISTING` | `ActionExecutor` handles tool, memory search, session search, memory write/archive proposal-required responses, and LifeModel patch proposal-required responses. | `openlife-core/src/agent/action_executor/mod.rs` | Execution primitives exist, with write paths returning proposal-required actions. |
| Tool permissions | `EXISTING` | SQLite-backed `ToolPermissionStore` supports allow, deny, ask every time, allow once, and allow until revoked. | `openlife-core/src/tool_permissions.rs` | Permission governance is real code. |
| Proposal storage | `EXISTING` | `ProposalStore` persists proposal records with status, risk, source, base hash, and resolution fields. | `openlife-core/src/agent/proposal_store.rs` | Review Center has durable backing. |
| Review workflow | `PARTIAL` | `ReviewWorkflow` centralizes pending proposal submission and idempotency, but direct `ProposalStore::create_proposal` callsites still exist by inventory/exception. | `openlife-core/src/agent/review_workflow.rs`, `src-tauri/src/single_system_authority_tests.rs` | Proposal-first direction is real, but single authority is not absolutely closed. |
| Memory store | `EXISTING` | SQLite memory/message/session/state tables and FTS search exist. | `openlife-core/src/memory.rs` | Durable local memory primitives are real. |
| Memory gateway | `PARTIAL` | `MemoryGateway` classifies memory lanes and proposal thresholds; Tauri wrappers use it for many paths, but direct store APIs remain broad. | `openlife-core/src/memory_gateway.rs`, `src-tauri/src/memory_gateway.rs`, `openlife-core/src/memory.rs` | Product-level memory authority is improving but not fully sealed. |
| LifeModel | `EXISTING` | Rich `LifeModel` struct, default model, manager load/save, compatibility/provenance view, and patch application exist. | `openlife-core/src/life_model.rs`, `openlife-core/src/life_model/patch.rs` | Domain model is substantial and should be preserved. |
| LifeModel write gateway | `PARTIAL` | `LifeModelWriteGateway` blocks automatic learning and allows accepted proposal materialization/manual override with governance, but manual/state command paths still need careful classification. | `openlife-core/src/life_model_write_gateway.rs`, `src-tauri/src/life_model_write_gateway.rs`, `src-tauri/src/commands/state.rs` | No silent canonical writes is mostly enforced by gateway checks, but authority review should continue. |
| Audit trail | `EXISTING` | Agent runs, MCP audit logs, evidence stores, memory lifecycle, and task transcripts exist. | `openlife-core/src/agent/store.rs`, `openlife-core/src/mcp_audit.rs`, `openlife-core/src/agent/evidence_store.rs`, `openlife-core/src/agent/memory_lifecycle.rs` | The repo has strong audit primitives. |
| External live provider proof | `PARTIAL` | Provider routing and local/live harnesses exist, but this audit did not verify a live external provider run. Active docs say live-provider evidence is incomplete. | `openlife-core/src/scheduler.rs`, `openlife-core/src/agent/model_router.rs`, `src-tauri/src/main_chat_live_provider_harness.rs`, `AGENTS.md` | Must not claim live provider readiness. |
| Web AgentLoop / MCP AgentLoop | `PARTIAL` | Read-only tool and MCP candidate handling exist; full web/MCP AgentLoop evidence remains incomplete by current status boundary. | `src-tauri/src/main_chat_kernel.rs`, `src-tauri/src/main_chat_react_runtime.rs`, `AGENTS.md` | Treat as partial until live journey evidence exists. |

## Backend Summary

OpenLife has real backend substance: runtime, policy, tool, proposal, memory,
LifeModel, audit, and safety primitives exist. The major gap is not absence of
backend code. The gap is convergence into one product authority per concern and
end-to-end product proof.
