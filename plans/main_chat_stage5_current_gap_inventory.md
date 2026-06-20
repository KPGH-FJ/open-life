# Main Chat Stage 5 Current Gap Inventory

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation inventory

## 1. Current Assets To Reuse

| Area | Current state | Stage 5 reuse direction |
| --- | --- | --- |
| Agent task runtime | Main Chat creates governed task sessions, runs, transcript entries, actions, blockers, proposals, and final delivery. | Use as the primary trace source. Do not add a parallel task model. |
| Command-surface eval | 24-case send/stream command-surface gate covers DirectAnswer, file read, proposal, web/MCP blockers/success, and no fallback/silent write rules. | Reuse for deterministic debug bundle proof. |
| Final acceptance | Aggregates runtime, command-surface, live-provider evidence, and fail-closed blockers. | Reuse blockers and provider preflight; do not lower standards. |
| Stage 2 readiness | Has manual dogfood and live-provider artifact contracts, stale build rejection, and fail-closed semantics. | Stage 5 should make artifact creation easier, not mark readiness itself. |
| Stage 3 execution UX | Shows task status, timelines, controls, and execution evidence. | Use as the user-visible anchor for export/debug controls. |
| Stage 4 memory/knowledge | Exposes memory assets, knowledge inventory, managed `USER.md`/`MEMORY.md` writes, rollback, and durable final delivery. | Include memory/context/knowledge summaries in debug bundles. |
| Diagnostics/config | Frontend already calls diagnostics/config and has safe-mode handling. | Extend into a tester-facing preflight/readiness panel. |
| Redaction tests | Frontend and backend already contain secret redaction tests for config/dev invoke logs and live-provider preflight. | Reuse conventions and expand to debug bundle export. |

## 2. Product Gaps

| Gap | Current symptom | Product risk | Stage 5 target |
| --- | --- | --- | --- |
| No unified debug bundle | Evidence exists across many stores/reports but is not one exportable object. | Internal testers cannot file reproducible issues. | Add a metadata-safe bundle keyed by task/session/scenario. |
| Build provenance is not a first-class tester artifact | Stage 2 validates known commits, but ordinary users do not see/export build provenance naturally. | Manual feedback can become stale or unverifiable. | Include commit, branch, version, timestamp, and dirty-state metadata where available. |
| Build provenance source is not productized | Runtime code may not know commit/branch in packaged builds unless injected. | Debug artifacts could contain unknown or fabricated build values. | Use deterministic build/app metadata and emit named blockers for missing fields. |
| Debug artifact storage lifecycle is undefined | There is no app-data artifact store for bundles/reports. | Exports may be written into the workspace, become unbounded, or fail after refresh. | Store schema-versioned artifacts under app data with atomic write, digest, list/get, and retention/delete behavior. |
| Environment readiness is scattered | Provider/live preflight, diagnostics, safe paths, MCP, network, and database state are separate. | Testers cannot tell whether a failure is product behavior or setup. | Add one preflight read model with named blockers. |
| Failure reasons are not normalized enough for users | Backend blockers exist, but UI copy and recovery guidance are uneven. | Testers will report vague failures like "agent bad". | Map failures to stable taxonomy and recovery recommendation. |
| UI export path is missing | AgentControlPlane shows state, but there is no obvious "export this run" flow. | Feedback is hard to collect and correlate. | Add export/report controls near task state/final delivery. |
| UI evidence trust boundary is not explicit | Frontend can show controls, but visible UI state alone does not prove backend execution. | Debug bundles could over-credit UI-only claims. | Correlate UI evidence with task id, backend snapshot id, visible labels, timestamp, and optional digest. |
| Privacy boundaries for debug export are not fully specified | Existing redaction is local to logs/preflight. | Debug bundles can leak private memory or API secrets. | Add explicit export redaction contract and tests. |
| Manual dogfood support is not ergonomic | Stage 2 templates exist but are not integrated into everyday task testing. | Real S2-D dogfood will be slow and inconsistent. | Issue report should include scenario/reviewer/pass-fail/blocker fields. |
| Live-provider failure is hard to explain to testers | Live tests are ignored unless opted in; preflight blockers exist but may not be productized. | Testers may think the Agent failed when env is missing. | Preflight must clearly show opt-in/key/network/provider blockers. |
| No release/debug report | Stage reports exist for earlier phases, but no Stage 5 DBG coverage. | Stage 5 implementation could overclaim readiness. | Add `main_chat_stage5_release_debug` report. |

## 3. High-risk Files

Stage 5 is likely to touch:

- `src-tauri/src/main_chat_final_gate.rs`
- `src-tauri/src/main_chat_eval_state.rs`
- `src-tauri/src/main_chat_agent_stage2_readiness.rs`
- `src-tauri/src/main_chat_stage3_execution_ux.rs`
- `src-tauri/src/main_chat_stage4_memory_knowledge.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `openlife-core/src/agent/main_chat_agent_productization_v1.rs`
- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/ProposalReviewPage.tsx`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- a new focused Stage 5 backend module and tests

Avoid broad edits to planner, memory lifecycle, ReAct tool selection, or final
live-provider acceptance unless the debug bundle needs read-only evidence.

## 4. Out Of Scope For Stage 5

- Public release management.
- Hosted telemetry dashboards.
- Full OpenTelemetry exporter.
- New autonomy or tool capabilities.
- Skills Hub expansion.
- Memory model redesign.
- Running real S2-D manual dogfood.
- Replacing final acceptance or Stage 2 readiness.

## 5. Stage 5 Done Means

Stage 5 is done when an internal tester can:

- see build/environment preflight before testing;
- run a Main Chat task and export a metadata-safe debug bundle;
- see why a task failed and what recovery action is recommended;
- attach a bundle to an issue or future manual dogfood row;
- trust that exported data does not leak keys or raw private memory by default;
- verify that Stage 2 readiness still fails closed until real manual/live
  evidence is present.
