# Backend Capability Map Refresh

Status: `SOURCE_BACKED_REVIEW_CANDIDATE`
Compared baseline: `docs/openlife-phase0-audit/02_backend_capability_map.md` and
`plans/openlife_agent_product_capability_matrix_v1.md`
Source snapshot checked: `e1b43161f78a`
Current bounded backend freeze: tag `backend-freeze-c9e75c8` at
`c9e75c8cc904`

## 1. Method

The refresh did not rely on document claims alone. It checked:

- the shipped `tauri::generate_handler!` list in `src-tauri/src/lib.rs`;
- typed frontend wrappers in `frontend/src/tauri.ts`;
- current product read models in `src-tauri/src/read_models/` and
  `openlife-core/src/agent/*_view_model.rs`;
- ResourceStore, StateStore, tool permission, proposal, memory, LifeModel,
  provider, task, and runtime owners in `openlife-core/src/`;
- the bounded roadshow state and evidence records in
  `plans/openlife_roadshow_core_capability_state.json`;
- the current authority boundary in `AGENTS.md` and
  `docs/openlife_frontend_refactor_readiness_report.md`.

Raw symbol presence was not counted as product readiness. Each capability is
classified by authority, shipped surface, product projection, and outstanding
proof.

The commits between the freeze tag and the source snapshot were also reviewed.
Their backend changes harden Windows/Linux portability, SQLite owner identity,
resource-parser tests, keyring guards, and CI behavior. They do not add a new
product route, read model, or frontend-ready capability, so this map does not
inflate them into product promises.

## 2. What Changed Since The Older Map

| Upgrade | Current evidence | Frontend consequence | Claim boundary |
|---|---|---|---|
| Canonical resource import | `resource.rs`, `resource_gateway.rs`, `resource_parser.rs`, `resource_selection.rs`, `resource_commands.rs` | Workspace composer may show selected/importing/committed/detached/error attachment states using receipts. | No general resource-library ViewModel exists. Native picker product trial is still not global completion evidence. |
| Request-scoped resource citations | `ResourceCitationSet` and final-output validation in Main Chat | Answers and artifacts can expose backend-issued citations and fail closed on missing/forged ids. | UI must not manufacture citation ids or infer “read” from model prose. |
| Governed live Web search/fetch | `web_search.rs`, ActionExecutor, Main Chat runtime and bounded live evidence | Timeline can show one governed Web action, observation, sources, and validation failure. | Bounded DeepSeek/fetch evidence is not a claim that every provider/search route is ready. |
| Canonical StateStore | `state_store.rs`; `commands/state.rs` now reads daily tasks/history from StateStore | Today may consume canonical daily tasks through the compatibility DTO; state mutation must stay governed. | There is still no complete backend TodayViewModel. Do not restore page-local quick-write authority. |
| Reviewed file artifacts | `file.write_proposal`, proposal acceptance/materialization, atomic file write/reconciliation | Workspace and Review may distinguish draft, awaiting review, approved, applying, confirmed, failed, unknown. | No standalone ArtifactViewModel currently owns a complete artifact library. |
| Action-bound allow-once permission | exact `canonical_scope`, `blocked_action`, `grant_action_bound`, `peek_action_bound`, `consume_action_bound` | UX may say “仅允许本次” only when exact scope is projected and verified; resume follows refresh. | Old “scope/time/revoke unavailable” is no longer universally true. Current ReviewItem still does not expose the readable scope. |
| Provider route truth and validation receipts | provider runtime generation, validation record, transmission events, `ProviderPrivacyBoundarySummary` | Settings can test, save, refresh, and show unknown/blocked/sent/not-sent without local inference. | Successful connection test is not saved config and not proof that all future requests stay local. |
| Explicit memory commit/undo and review batching | Memory lifecycle, Review batches, `create_knowledge_note`, `undo_explicit_memory` | Review and LifeModel can show candidate, confirmed/materialized, rollback, conflict, and provenance states. | Review batch is presentation-only and cannot authorize children as a group. |
| Runtime cancellation/restart/replay hardening | task controls, operation identity, receipts, durable events | Tasks can show cancelled, remote unknown, retry, resume, and evidence-required terminal states. | A dispatched control never proves completion; refreshed backend state owns the result. |
| Release quarantine | shipped handler uses feature-gated dev extensions | Product IA must omit MCP/A2A/plugin management from primary surfaces. | Dev tools are not product capabilities simply because code exists. |

## 3. Current Product Capability Map

### 3.1 Conversation And Execution

| Capability | Backend authority | Shipped bridge/read model | Current class | Phase 3F placement |
|---|---|---|---|---|
| Buffered and streaming Main Chat | `OpenLifeTurnRuntime`, Main Chat kernel/runtime | `sendMessageV2`, `startStreamMessage`; task state/snapshot/events | `VERIFIED_BACKEND`, product UX partial | Workspace composer and current-task timeline |
| Governed direct answer | ingress, strategy, provider receipts | structured send/stream result | `VERIFIED_BACKEND` | Compact answer; trace on demand only |
| ReAct/tool execution | AgentLoop, ActionExecutor, ToolGateway, ActionQueue | task detail/events/transcript | `VERIFIED_BACKEND`, projection fragmented | Human-readable Workspace timeline; raw transcript in Inspector |
| Plan-execute-review | plan session store and commands | typed bridge commands | `PRODUCT_BRIDGE`, product loop partial | Future Workspace mode; not a Phase 3F default promise |
| Task resume/retry/cancel | task control owner and refreshed state | task commands plus `TasksViewModel` controls | `PRODUCT_READ_MODEL` | Workspace current control; Tasks continuity/history |
| Skills/tool candidates | skill registry and runtime selection | list/detail/select/clear bridge | `PRODUCT_BRIDGE` | Secondary Workspace capability picker, not top-level nav |

### 3.2 Resources, Web, And Artifacts

| Capability | Backend authority | Shipped bridge/read model | Current class | Phase 3F placement |
|---|---|---|---|---|
| Native file selection/import | ResourceGateway + native picker | pick/cancel/status/detach receipts | `VERIFIED_BACKEND` + `PRODUCT_BRIDGE` | Composer attachment tray with explicit lifecycle |
| PDF/DOCX/CSV/XLSX/text parsing | bounded parser worker and provenance | receipt metadata; selected citations in turn result | `VERIFIED_BACKEND` | File type/status and source details in Inspector |
| Deterministic selection | ResourceSelection | request-bound context/citations | `VERIFIED_BACKEND` | “已使用哪些片段” evidence, never hidden fake reading |
| Web fetch/search | governed network/action path | task/action evidence; no dedicated Web VM | `VERIFIED_BACKEND`, projection partial | Timeline action and source list; no global Web page |
| Generated file proposal | proposal-generation tool + ReviewWorkflow | proposal/review bridge | `VERIFIED_BACKEND`, rich review projection partial | Review Center decision; Workspace shows pending outcome |
| File materialization | accepted proposal effect + atomic write/reconciliation | accept result and proposal state | `VERIFIED_BACKEND`, artifact VM absent | Approved/applied/failed/unknown ledger; no fake completion |

### 3.3 Today, State, Memory, And LifeModel

| Capability | Backend authority | Shipped bridge/read model | Current class | Phase 3F placement |
|---|---|---|---|---|
| Daily tasks | canonical StateStore | `getDailyGoals` compatibility DTO | `VERIFIED_BACKEND`, no complete Today VM | Today focus/schedule; operation ids remain Inspector-only |
| Typed state observations/history | StateStore | history/alert commands | `PRODUCT_BRIDGE` | Today/LifeModel supporting evidence, not a dashboard metric wall |
| Shared product state | `LifeStateProjection` | typed frontend projection | `PRODUCT_READ_MODEL` | cross-surface pending/safe/readiness facts |
| Memory lifecycle | MemoryStore/Gateway/Lifecycle | `MemoryViewModel`, lifecycle and undo commands | `PRODUCT_READ_MODEL` | Review decisions and LifeModel “记忆与来源”; no top-level Memory nav yet |
| LifeModel truth/provenance | manager, write gateway, patches, snapshots | `LifeModelViewModel` | `PRODUCT_READ_MODEL` | LifeModel current/candidate/materialized views |
| Today aggregate | frontend adapter over projection and daily goals | no backend `TodayViewModel` | `TARGET_CONTRACT` | bounded Phase 3F layout only; no independent truth |

### 3.4 Review, Permission, Privacy, And Settings

| Capability | Backend authority | Shipped bridge/read model | Current class | Phase 3F placement |
|---|---|---|---|---|
| Review decision states | ProposalStore/ReviewWorkflow | `ReviewCenterViewModel`, ReviewAction | `PRODUCT_READ_MODEL` | Review Center queue/detail/decision bar |
| Review before/after and impact | `AgentProposal` owns values/reason/source | not projected by `ReviewItem` | `TARGET_CONTRACT`, blocking | Required before React port of rich Review detail |
| Exact action permission | action-bound permission store and acceptance validation | current ReviewItem exposes refs/actions, not readable exact scope | `VERIFIED_BACKEND` + `TARGET_CONTRACT` gap | Enabled only in known-scope fixture; unknown stays disabled |
| Provider/privacy boundary | runtime snapshot, validation and durable transmission events | `ProviderPrivacyBoundarySummary` | `PRODUCT_READ_MODEL` | global boundary control and Inspector conclusion |
| Config read/save | ConfigStore, keychain secret staging, runtime replacement | `getConfig`, `saveConfig` | `PRODUCT_BRIDGE` | Settings form; dirty/saving/saved-awaiting-refresh states |
| Provider connection test | network policy + consent + provider receipt | `testLlmConnection` result | `PRODUCT_BRIDGE` | Explicit external-test confirmation and result ledger |
| Data export/import | governed dangerous-action confirmation | typed settings commands | `PRODUCT_BRIDGE`, outside critical prototype | Settings > Data & Recovery; never a primary nav item |

### 3.5 Development And Quarantined Surfaces

| Surface | Current classification | Phase 3F rule |
|---|---|---|
| MCP server/tool/template management | `DEV_ONLY` under `dev-extensions` | Advanced/support evidence only; absent from product nav |
| A2A sidecar and task bridge | `DEV_ONLY` under `dev-extensions` | Absent from product IA |
| Plugin management | `DEV_ONLY` under `dev-extensions` | Absent from product IA |
| Raw reasoning/kernel/durable event JSON | diagnostic evidence | Inspector advanced disclosure, never default reading flow |
| Scheduler/vector/background maintenance | implementation/support concern | Settings advanced/support only where a shipped user job exists |

## 4. Read-Model Fitness For Frontend V2

| Surface | Fitness | Reason |
|---|---|---|
| Tasks | `USABLE_WITH_LIMITS` | Rich lifecycle, controls, blockers and terminal evidence exist. |
| Workspace | `LIMITED` | References and a compact timeline exist, but it explicitly does not replace a complete V2 execution model. |
| Review Center | `BLOCKED_FOR_RICH_DECISION` | Decisions/actions exist; readable diff, impact, permission scope and transmission context do not. |
| Provider/privacy | `USABLE` | Dedicated backend summary owns route/transmission/risk and fail-closed warnings. |
| LifeModel | `USABLE_WITH_LIMITS` | Current/candidate/materialized/provenance are explicit; compatibility and ownership limits remain visible. |
| Memory | `USABLE_WITH_LIMITS` | Lifecycle and linkage summaries exist; detailed human-readable memory records need deliberate projection. |
| Today | `PARTIAL_FRONTEND_ADAPTER` | StateStore daily tasks and LifeStateProjection exist, but no complete backend Today ViewModel. |
| Settings | `BRIDGE_COMPOSITION_REQUIRED` | Config, test result, privacy summary, permissions, and recovery are separate authorities. |
| Resources/Web/Artifacts | `WORKSPACE_PROJECTION_REQUIRED` | Backend capabilities are real; V2-friendly consolidated timeline/evidence projection is incomplete. |

## 5. Anti-Hallucination Reconciliation

The roadshow state proves a bounded backend freeze with extensive deterministic
and selected external-live evidence. The active repository authority still
forbids these stronger claims:

- Roadshow is globally complete;
- Phase7 is complete;
- all Backend Remediation v4 findings are closed;
- every native product journey has passed;
- every provider, Web search transport, or external route is ready;
- the app is signed/notarized or production keychain behavior is proven;
- a backend primitive automatically has a complete V2 UI contract.

The frontend plan therefore treats the backend freeze as a strong input, not a
license to render global readiness.

## 6. Map Decision

```text
BACKEND_CAPABILITY_FREEZE_INPUT = VERIFIED_BOUNDED
BACKEND_FUNCTION_MAP_REFRESH = REVIEW_CANDIDATE
PRODUCT_READ_MODEL_COVERAGE = PARTIAL
RICH_REVIEW_PROJECTION = BLOCKED
COMPLETE_WORKSPACE_PROJECTION = BLOCKED
GLOBAL_BACKEND_OR_ROADSHOW_COMPLETION = NOT_CLAIMED
```
