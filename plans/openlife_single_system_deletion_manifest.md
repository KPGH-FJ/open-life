# OpenLife Single-System Deletion Manifest

> Date: 2026-07-06
> Status: preparation manifest
> Purpose: list the old/new parallel systems that must be resolved during the
> single-system development round.

Disposition values:

- `keep`: remains as the product authority.
- `absorb_then_delete`: useful logic may be moved into the new authority, then
  the old product module/path is removed.
- `delete`: remove from product code and command surface.
- `storage_only`: may remain as a persistence implementation, but cannot be a
  product semantic authority.
- `test_fixture_only`: may remain only under tests/fixtures after product
  references are removed.
- `archive_reference`: documentation may remain only as historical reference,
  not as active authority.

## 1. Product Authority Targets

| Domain | Final product authority | Notes |
| --- | --- | --- |
| Ordinary Main Chat turn | `OpenLifeTurnRuntime` | send and stream are transport wrappers only. |
| Intent/policy routing | `IntentFrame` + `PolicyRouter` | semantic classification and governance routing are separated. |
| Durable writes | `DurableWritePolicy` | decides direct memory, proposal, ask-user, blocker, or no-write. |
| Proposal/review | `ReviewWorkflow` | only creator of product proposals. |
| Memory | `MemoryGateway` | owns product memory lane decisions and reads/writes. |
| LifeModel writes | `LifeModelWriteGateway` | owns canonical LifeModel updates. |
| Tool execution | `ToolGateway` | explicit tool contracts, permission, execution, observation. |
| Final result | `CanonicalFinalDeliveryView` | one terminal object for answer/action/proposal/blocker state. |
| Frontend state | `LifeStateProjection` | common product pages read one projection. |

## 2. Main Chat Runtime And Routing

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `src-tauri/src/main_chat_turn_pipeline.rs` | Dispatches among kernel, tool loop, strategy helper, route preview, and blockers. | shrink to a thin wrapper over `OpenLifeTurnRuntime`; delete routing logic | 2 |
| `src-tauri/src/main_chat_kernel.rs` | Large product runtime with many embedded branches and compatibility terms. | absorb_then_delete into runtime services | 2 |
| `src-tauri/src/main_chat_strategy.rs` | Separate strategy execution path after pipeline routing. | delete | 2 |
| `src-tauri/src/main_chat_tool_loop.rs` | Separate tool-loop adapter with single-step fallback-shaped outcome. | absorb ToolGateway parts, delete fallback/product adapter | 2, 6 |
| `src-tauri/src/main_chat_legacy_agent_loop.rs` | Deprecated/non-default product-shaped loop still compiled. | delete | 2 |
| `src-tauri/src/main_chat_route_preview.rs` | Advisory route preview can still influence runtime/status surface. | delete from product route; any classifier experiment must move outside shipped product runtime | 3 |
| `openlife-core/src/agent/main_chat_agent_v1.rs` `StrategyRouter` | Keyword/rule product router. | absorb useful labels into IntentFrame, then remove route authority | 3 |
| `openlife-core/src/agent/strategy.rs` `StrategySelector` | Older runtime selector using keywords and planning fallback. | remove product dependency, migrate useful assertions, then delete module | 3 |
| `openlife-core/src/agent/multi_strategy_runtime.rs` | Historical multi-strategy runtime. | delete from product path | 7 |
| `openlife-core/src/agent/react_beta.rs` | Beta terminology and helper surface. | absorb metadata-safe helpers into ToolGateway/runtime, then delete beta surface | 6, 7 |

## 3. Router And Intent Systems

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `openlife-core/src/router.rs` `IntentRouter` | Separate router held in `AppState`. | absorb/delete after `PolicyRouter` lands | 3 |
| `openlife-core/src/layer_router.rs` `LayerRouter` | Separate layer route authority held in `AppState`. | absorb/delete after `PolicyRouter` lands | 3 |
| `src-tauri/src/commands/diagnostics.rs` `get_router_status` | Exposes old router status as product diagnostics. | delete from product diagnostics after router removal; developer inspection must use a non-shipped dev harness | 3, 7 |
| `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | UI for old/multi strategy preview/status. | delete | 7 |
| `frontend/src/tauri.ts` multi-strategy/runtime exports | Keeps old routing concepts callable by frontend. | delete from product bridge; dev-only bridge must not be imported by product pages | 7 |

## 4. Proposal And Review

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `openlife-core/src/agent/proposal_engine.rs` | Contains placeholder generators; not proposal authority. | replace with/absorb into `ReviewWorkflow` | 4 |
| `openlife-core/src/agent/proposal_store.rs` | Storage is fine, but direct callers create product proposals. | storage_only | 4 |
| `src-tauri/src/main_chat_proposal_support.rs` | Main Chat creates proposals directly. | absorb_then_delete into ReviewWorkflow adapter | 4 |
| `src-tauri/src/commands/builder.rs` `builder_create_proposals` | Builder creates proposals directly. | route through ReviewWorkflow | 4 |
| `src-tauri/src/commands/calibration.rs` `calibration_create_proposals` | Calibration creates proposals directly. | route through ReviewWorkflow | 4 |
| `src-tauri/src/commands/proactive.rs` | Proactive suggestions create proposals directly. | route through ReviewWorkflow | 4 |
| `src-tauri/src/commands/execution.rs` and action executors | Tool/skill paths create proposals directly. | route through ReviewWorkflow/ToolGateway | 4, 6 |
| `openlife-core/src/agent/maturation.rs` | Maturation writes proposal/evidence directly. | route through ReviewWorkflow and MemoryGateway | 4, 5 |
| `openlife-core/src/agent/plan_execute.rs` | PlanExecute creates proposals directly. | route through ReviewWorkflow | 4 |

## 5. Memory And LifeModel

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `openlife-core/src/memory.rs` `MemoryStore` | Valid storage, but should not be product authority. | storage_only behind MemoryGateway | 5 |
| `openlife-core/src/vectors.rs` `VectorStore` | Valid storage/search, but direct index command exists. | storage_only behind MemoryGateway | 5 |
| `openlife-core/src/agent/memory_lifecycle.rs` | Valid lifecycle storage, but product code reads/writes separately. | storage_only behind MemoryGateway | 5 |
| `openlife-core/src/agent/evidence_store.rs` | Valid evidence storage, but direct evidence writers are scattered. | storage_only behind MemoryGateway/ReviewWorkflow | 5 |
| `openlife-core/src/agent/lifemodel_backend_completion.rs` `LifeEventStore` | Valid event store, but needs lane policy. | storage_only behind MemoryGateway | 5 |
| `src-tauri/src/commands/memory.rs` `index_memory_chunk` | Direct product memory write command. | replace with MemoryGateway command; delete current direct product exposure | 5 |
| `src-tauri/src/commands/life_model.rs` `save_life_model` | Governed manual override exists but still product direct save surface. | move behind LifeModelWriteGateway; only explicit manual override UX may invoke it | 5 |
| `src-tauri/src/commands/state.rs` daily-goal/state writes | Persist LifeModel compatibility/source data. | route through LifeModelWriteGateway with lane classification | 5 |
| `src-tauri/src/commands/settings.rs` import/restore writes | Governed but separate write path. | route through LifeModelWriteGateway/import policy | 5 |
| `src-tauri/src/legacy_write_convergence.rs` | Historical convergence inventory/gate. | delete after gateway guards replace it | 7 |

## 6. Tool Execution

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `openlife-core/src/agent/action_executor/mod.rs` | Very broad context and domain ownership. | shrink behind ToolGateway | 6 |
| `openlife-core/src/agent/action_executor/**` | Mixed tool execution, memory, proposal, permission behavior. | split into ToolGateway adapters, delete direct write behavior | 6 |
| `openlife-core/src/tool_manifest.rs` inference helpers | Infers capability/risk/action type from tool name. | remove execution credit; any migration lint must be non-executable and developer-only | 6 |
| `openlife-core/src/mcp.rs` manifest registration | External MCP may rely on inferred permission/capability. | require explicit executable contract | 6 |
| `src-tauri/src/main_chat_react_*` | Useful candidate/selection logic but tied to old runtime path. | absorb into ToolGateway/runtime | 6 |
| `grant_tool_permission` direct command | Useful user-controlled permission path. | keep only through ToolGateway/ReviewWorkflow policy | 6 |

## 7. Frontend Read Model And Product State

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `frontend/src/pages/TodayPage.tsx` | Builds state from diagnostics, daily goals, proposals. | migrate to LifeStateProjection | 6 |
| `frontend/src/pages/MailboxPage.tsx` | Builds review state from proposals, config, diagnostics. | migrate to LifeStateProjection + ReviewWorkflow views | 6 |
| `frontend/src/pages/ChatPage.tsx` | Large page reads many backend sources and owns status composition. | migrate common state to LifeStateProjection; keep chat-specific transport state local | 6 |
| `frontend/src/pages/LifeModelPage.tsx` | Reads LifeModel/current view/diagnostics/proposals separately. | migrate readiness/pending state to LifeStateProjection | 6 |
| `frontend/src/pages/SettingsPage.tsx` | Mixes config, diagnostics, runtime, router, tool permissions. | split product state from developer diagnostics | 6, 7 |
| `frontend/src/pages/LifeModelEditor.tsx` | Manual save surface. | route through LifeModelWriteGateway; UI must present explicit governed manual override, not automated learning | 5, 6 |
| `frontend/src/tauri.ts` | Monolithic bridge exposing product, dev, migration, eval commands. | split product bridge from dev/test bridge | 7 |

## 8. Stage, Beta, Migration, And Eval Surfaces

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `src-tauri/src/main_chat_agent_beta_v1_*` | Beta readiness systems compiled in product crate. | migrate useful assertions to single-system tests, then delete product modules | 7 |
| `src-tauri/src/main_chat_agent_stage1_dogfood.rs` | Stage dogfood setup compiled and exposed. | remove from product handler; convert remaining value to non-product test fixture or delete | 7 |
| `src-tauri/src/main_chat_agent_stage2_readiness.rs` | Stage readiness compiled and exposed. | archive/test only after final trial plan replaces it | 7 |
| `src-tauri/src/main_chat_stage3_execution_ux.rs` | Stage report system compiled and exposed. | archive/test only | 7 |
| `src-tauri/src/main_chat_stage4_memory_knowledge.rs` | Stage managed knowledge command surface. | absorb valid memory operations into MemoryGateway/ReviewWorkflow | 5, 7 |
| `src-tauri/src/main_chat_stage5_release_debug.rs` | Debug/report product-like surface. | developer-only, not product handler | 7 |
| `src-tauri/src/commands/agent_runtime/migration_ladder.rs` | Controlled pilot/migration/cutover commands exposed. | delete from product handler; archive historical evidence outside product runtime | 7 |
| `frontend/e2e/main-chat-stage1-dogfood.spec.ts` | Old stage-specific e2e. | rewrite as final Computer Use/product trial; old script may only remain as historical archive outside active gates | 7 |
| `frontend/e2e/main-chat-step6-product-acceptance.spec.ts` | Useful concepts but stage/step-specific. | rewrite under single-system acceptance | 7 |

## 9. Active Plan And Documentation Cleanup

| Current object | Current issue | Disposition | Phase |
| --- | --- | --- | --- |
| `plans/README.md` | Still names Goal/Stage/Beta/Migration precedence and fallback visibility. | update to single-system authority after Phase 1 | 1 |
| `plans/main_chat_*stage*` | Historical stage plans can steer new work. | remove from active index; archive_reference only with explicit historical header | 7 |
| `plans/main_chat_agent_beta*` | Historical beta plans can steer new work. | remove from active index; archive_reference only with explicit historical header | 7 |
| `plans/main_chat_agent_migration_v1_goal_spec.md` | Historical migration framing. | archive_reference | 7 |
| `plans/legacy_direct_write_convergence_goal_spec.md` | Historical convergence plan. | archive_reference after gateways replace it | 7 |
| `docs/ARCHITECTURE.md` | Quick architecture still describes old IntentRouter/LayerRouter flow. | update after new authorities land | 7 |

## 10. Guards To Add During Implementation

| Guard | Required after phase |
| --- | --- |
| No ordinary send/stream import or call of retired strategy/fallback modules. | 2 |
| No product route decision from old routers. | 3 |
| No direct product `ProposalStore::create_proposal` outside ReviewWorkflow. | 4 |
| No direct product memory/LifeModel write outside gateways. | 5 |
| No executable tool manifest credit from inferred capability/risk/action type. | 6 |
| Product frontend pages cannot assemble readiness/pending state from raw sources covered by LifeStateProjection. | 6 |
| Shipped Tauri handler contains no migration/cutover/stage/beta/dev dogfood command. | 7 |
| Active docs cannot declare legacy fallback as acceptable product behavior. | 7 |

## 11. Phase 1 Inventory Crosswalk

Machine-readable inventory:

- `plans/openlife_single_system_phase1_inventory.json`

Phase 1 does not delete these systems. It makes them explicit, classified, and
guarded so later phases cannot add unregistered parallel systems.

| Inventory category | Manifest section | Phase rule |
| --- | --- | --- |
| `product_authorities` | Sections 1 and 9 | keep only `AGENTS.md`, `plans/README.md`, and the two single-system docs as active authority |
| `old_runtime_surfaces` | Sections 2, 6, and 8 | absorb/delete/archive according to each entry's phase and disposition |
| `old_router_surfaces` | Section 3 | replace with `IntentFrame` + `PolicyRouter`, then delete old router surfaces |
| `product_old_route_markers` | Sections 2, 3, and 8 | every old marker is counted until the owning phase removes it |
| `direct_proposal_write_surfaces` | Section 4 | route through `ReviewWorkflow`; `ProposalStore` remains storage only |
| `direct_memory_lifemodel_write_surfaces` | Section 5 | route through `MemoryGateway` / `LifeModelWriteGateway`; storage stays storage only |
| `frontend_multi_source_state_surfaces` | Section 7 | migrate product pages to `LifeStateProjection`; split product bridge from dev/test bridge |
| `stage_beta_migration_command_surfaces` | Section 8 and table below | delete from shipped product handler in Phase 7 |

Phase 1 old-route marker inventory:

| Marker | Phase | Disposition |
| --- | --- | --- |
| `legacy_agent_loop` | 2 | delete |
| `main_chat_strategy` | 2 | delete |
| `route_preview` | 3 | delete |
| `single_step_fallback` | 2 | delete |
| `MultiStrategy` | 7 | delete |
| `beta_v1` | 7 | delete |
| `stage1` | 7 | archive_reference |
| `stage2` | 7 | archive_reference |
| `stage3` | 7 | archive_reference |
| `stage4` | 5 | absorb_then_delete |
| `stage5` | 7 | delete |

Phase 1 shipped handler command inventory:

| Command | Phase | Disposition |
| --- | --- | --- |
| `run_main_chat_agent_execution_v1_eval_gate` | 7 | delete |
| `run_main_chat_capability_eval_gate` | 7 | delete |
| `run_main_chat_agent_beta_v1_readiness_gate` | 7 | delete |
| `run_main_chat_agent_stage1_dogfood_gate` | 7 | delete |
| `run_main_chat_agent_stage2_readiness_gate` | 7 | delete |
| `prepare_main_chat_step6_live_provider_eval_state` | 7 | delete |
| `run_main_chat_stage3_execution_ux_report` | 7 | delete |
| `validate_main_chat_agent_stage2_manual_dogfood_artifact` | 7 | delete |
| `prepare_main_chat_agent_stage1_browser_dogfood_state` | 7 | delete |
| `set_main_chat_agent_stage1_browser_network_policy` | 7 | delete |
| `set_main_chat_agent_stage1_browser_scripted_response` | 7 | delete |
| `set_main_chat_agent_stage1_browser_web_fixture_output` | 7 | delete |
| `get_react_beta_execution_status` | 7 | delete |
| `check_runtime_migration_gate` | 7 | delete |
| `draft_controlled_chat_migration_plan` | 7 | delete |
| `record_controlled_chat_migration_review_decision` | 7 | delete |
| `get_controlled_chat_migration_review_decision_summary` | 7 | delete |
| `check_controlled_chat_migration_implementation_gate` | 7 | delete |
| `run_controlled_chat_migration_shadow_run` | 7 | delete |
| `record_controlled_chat_migration_shadow_review_decision` | 7 | delete |
| `get_controlled_chat_migration_shadow_review_summary` | 7 | delete |
| `check_controlled_chat_cutover_readiness` | 7 | delete |
| `run_controlled_chat_cutover_candidate` | 7 | delete |
| `record_controlled_chat_cutover_candidate_review_decision` | 7 | delete |
| `get_controlled_chat_cutover_candidate_review_summary` | 7 | delete |
| `check_controlled_chat_cutover_candidate_promotion_readiness` | 7 | delete |
| `list_stage4_knowledge_asset_inventory` | 7 | delete |
| `run_main_chat_stage4_memory_knowledge_report` | 7 | delete |
| `evaluate_main_chat_stage5_release_debug_preflight` | 7 | delete |
| `run_main_chat_stage5_release_debug_report` | 7 | delete |
