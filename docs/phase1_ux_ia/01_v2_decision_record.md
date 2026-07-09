# OpenLife Phase 1 V2 Decision Record

Status: Phase 1 documentation decision record.
Scope: UX, IA, product language, and read-model direction only. No implementation authorization.

## Classification Legend

- `VERIFIED_FACT`: verified by Phase 0 / 0.5 evidence or active Phase7 authority.
- `DESIGN_DECISION`: accepted product/architecture direction for V2 planning.
- `DESIGN_ASSUMPTION`: plausible UX or engineering assumption requiring later validation.
- `CANDIDATE`: preserved product capability whose implementation shape is not approved.
- `UNKNOWN`: not verified by Phase 0 / 0.5.
- `PHASE_2_REQUIRED`: must be validated or implemented before Frontend V2 work.

## D1-bounded-rewrite

Decision ID: D1-bounded-rewrite
Title: V2 uses bounded product-experience + state-contract rewrite
Status: Accepted

Decision: `DESIGN_DECISION` - Frontend V2 should be a bounded rewrite of product experience, information architecture, and backend-owned state contracts. It must not discard current backend/domain primitives or restore old routes.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/openlife-phase0-audit/13_audit_summary.md`, `docs/openlife-phase0-audit/10_rewrite_strategy.md`
  Claim: Backend/runtime/domain primitives are substantial, while frontend problems are mainly UX and page-local state-boundary debt.
  Confidence: High
  Limitation: Phase 0 / 0.5 did not prove full desktop product readiness.
- Evidence type: Existing codebase fact
  Source: `AGENTS.md`, `plans/openlife_single_system_deletion_manifest.md`
  Claim: Phase7 requires old runtime, command, bridge, product UI route, and active route-authorizing docs to remain absent after deletion.
  Confidence: High
  Limitation: This decision record does not re-run Phase7 guards.

Product rationale: OpenLife already has real local-first agent, LifeModel, memory, review, tool, and audit primitives. The V2 opportunity is to make those understandable and coherent, not to rebuild a generic chat/dashboard frontend.

Engineering impact: Phase 2 must define backend-owned ViewModels / ReadModels before UI implementation. UI pages must not reconstruct readiness, pending review, task state, or durable-write truth from raw fragments when a read model exists.

Risk: If treated as a blank rewrite, V2 may delete useful product capability or revive parallel old systems. If too narrow, it may keep ChatPage-style state overload.

Reversal cost: High. Reversing after routes/components exist would likely require route, state, and copy churn.

Phase 2 implication: Create or validate ViewModel contracts first, then migrate surfaces incrementally.

Human approval needed: Yes, for IA, naming, ViewModel scope, and implementation sequencing.

## D2-workspace

Decision ID: D2-workspace
Title: Companion + Chat merge into `工作区`
Status: Accepted

Decision: `DESIGN_DECISION` - The V2 primary agent work surface is `工作区`. It absorbs current Companion and Chat responsibilities as a workspace for intent, understanding, plan, execution, review links, and result.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/phase0_5/03_chat_companion_workspace_mapping.md`
  Claim: `CompanionPage` wraps `ChatPage` in companion mode and is not a separate backend workflow.
  Confidence: High
  Limitation: The emotional/ambient value of a separate companion mode was not usability-tested.
- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/openlife-phase0-audit/09_agent_experience_gap_analysis.md`
  Claim: Chat currently owns input, task lifecycle, trace, proposals, tool calls, and final delivery, but not as a coherent workspace ViewModel.
  Confidence: High
  Limitation: The exact Phase 2 backend read-model shape is not yet verified.

Product rationale: A workspace frames OpenLife as an operating partner that understands, plans, acts, and asks for review. Chat becomes one input mode rather than the whole product model.

Engineering impact: Phase 2 needs a `WorkspaceViewModel` or adjacent read model. Direct ChatPage refactor is explicitly out of scope for this Phase 1 documentation pass.

Risk: Users who expect a lightweight chat/companion mode may perceive `工作区` as too task-heavy unless composer states and empty states are calm.

Reversal cost: Medium. The merged IA can still expose a companion/composer mode inside `工作区`.

Phase 2 implication: Validate workspace state, timeline, task controls, and review-link contracts before moving any component.

Human approval needed: Yes, for final route naming and whether companion remains a sub-mode.

## D3-review-center

Decision ID: D3-review-center
Title: Mailbox becomes `审核中心`
Status: Accepted

Decision: `DESIGN_DECISION` - The V2 decision surface should be `审核中心`, not Mailbox. It owns consequential review decisions across proposals, permissions, external writes, memory updates, LifeModel changes, policy changes, and dangerous actions.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/phase0_5/02_current_route_map.md`, `docs/phase0_5/03_chat_companion_workspace_mapping.md`
  Claim: Current `/mailbox` already supports proposal accept, reject, postpone, edit, safe-mode blocking, safe-path checks, and task resume after review.
  Confidence: High
  Limitation: Tool permissions and external writes are not yet verified as a single unified Review Center item model.
- Evidence type: Existing codebase fact
  Source: `docs/openlife-phase0-audit/02_backend_capability_map.md`, `docs/openlife-phase0-audit/06_security_governance_audit.md`
  Claim: ProposalStore, ReviewWorkflow, ToolPermissionStore, danger preflight, and safe-path file write approval primitives exist.
  Confidence: High
  Limitation: ReviewWorkflow is partial because direct proposal callsites still exist by inventory.

Product rationale: "Mailbox" implies messages. `审核中心` makes user agency and durable-change review explicit.

Engineering impact: Phase 2 must define a `ReviewItem` model and allowed review actions. Other pages should preview or link to review items, not own final decision state.

Risk: Over-broad Review Center scope could become a second dashboard unless item types, status, and actions are constrained.

Reversal cost: Medium. Naming can be changed, but the approval workflow concept should remain.

Phase 2 implication: Server-owned grouping, allowed actions, risk, evidence, and related task-resume fields are required.

Human approval needed: Yes, for exact Review Center scope and item taxonomy.

## D4-tasks

Decision ID: D4-tasks
Title: Runs becomes `任务`
Status: Accepted

Decision: `DESIGN_DECISION` - Current Runs should become `任务`, covering active and historical task/run lifecycle, controls, evidence, and details.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/phase0_5/02_current_route_map.md`, `docs/phase0_5/06_view_model_gap_inventory.md`
  Claim: `/runs` and `/runs/:runId` already combine AgentRun history with Main Chat task summaries, controls, deletion preflight, and evidence.
  Confidence: High
  Limitation: The current merge is page-local; there is no verified server-owned merged `TasksViewModel`.
- Evidence type: Product design rationale
  Source: `docs/phase0_5/05_ui_terminology_inventory.md`
  Claim: `任务` is clearer for ordinary users than "Runs" or "AgentRun".
  Confidence: Medium
  Limitation: Chinese label preference needs human approval.

Product rationale: OpenLife work is not just chat messages; it has resumable, cancellable, failed, blocked, completed, and review-waiting tasks.

Engineering impact: Phase 2 needs a task list/detail read model that does not require pages to locally merge run metadata, task summaries, final delivery, and danger preflight.

Risk: If `任务` overemphasizes task management, OpenLife may feel like a todo app. The surface must show agent work lifecycle, not generic todos.

Reversal cost: Low to medium. Label and placement can change if task history is later nested under `工作区`.

Phase 2 implication: Validate canonical relationship between AgentRun and Main Chat task session.

Human approval needed: Yes, for whether `任务` remains top-level or becomes a workspace subview.

## D5-memory-nav

Decision ID: D5-memory-nav
Title: Memory becomes top-level `记忆`
Status: Accepted with constraints

Decision: `DESIGN_DECISION` - Preserve Memory as a first-class product capability and propose top-level `记忆`, but only if Phase 2 can clearly distinguish it from LifeModel, Review Center, workspace evidence, and data-management settings.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`, `docs/openlife-phase0-audit/09_agent_experience_gap_analysis.md`
  Claim: Memory has real backend semantics across MemoryStore, MemoryGateway, lifecycle states, evidence, proposals, and LifeModel impacts.
  Confidence: High
  Limitation: Product read-model support for lane-level user explanation is incomplete.
- Evidence type: Product design rationale
  Source: `OpenLife_Phase1_UX_IA_Codex_Goal_v1.2.md`
  Claim: Guardrails should not reduce OpenLife into generic chat or delete important incomplete capabilities.
  Confidence: High
  Limitation: Human review may still choose reduced-risk IA.

Product rationale: Memory is central to a personal AI operating system, but it must not become a raw database browser or duplicate LifeModel.

Engineering impact: `MemoryViewModel` requires lane counts/status, candidate/confirmed/materialized/withdrawn states, provenance, and review links. Missing support is `PHASE_2_REQUIRED`.

Risk: Top-level Memory can confuse users if it overlaps with LifeModel, Review Center, or evidence drawers.

Reversal cost: Low to medium if decided before implementation; high if routes/components are built first.

Phase 2 implication: Validate memory lane read models before implementing top-level navigation.

Human approval needed: Yes.

Constraints:

- Memory must have a clear boundary from LifeModel canonical understanding.
- Memory decisions must route through `审核中心` where consequential.
- Workspace evidence drawer may preview memory impact but must not become memory authority.
- Settings/Data Management may own privacy/export/recovery controls, not everyday memory meaning.

Fallback: If Phase 2 cannot clearly distinguish Memory from LifeModel, Review Center, and Workspace Evidence, Memory may be downgraded to a LifeModel sub-surface or Settings/Data Management sub-surface.

## D6-lifemodel-name

Decision ID: D6-lifemodel-name
Title: LifeModel remains English-branded
Status: Accepted with constraints

Decision: `DESIGN_DECISION` - Keep `LifeModel` as the top-level brand/domain term, with Chinese explanatory copy such as `OpenLife 对你的长期理解`.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/openlife-phase0-audit/04_domain_model_analysis.md`, `docs/phase0_5/05_ui_terminology_inventory.md`
  Claim: LifeModel is a real structured domain model, while current UI mixes `Life Model`, `LifeModel`, and Chinese copy.
  Confidence: High
  Limitation: No human preference test has validated English branding in Chinese-first navigation.
- Evidence type: Product design rationale
  Source: `docs/openlife-phase0-audit/12_rebirth_strategy.md`
  Claim: A private LifeModel is a core part of OpenLife's product narrative.
  Confidence: High
  Limitation: The exact explanatory subtitle and onboarding copy remain open.

Product rationale: Keeping `LifeModel` preserves a distinctive domain concept while Chinese copy makes it understandable.

Engineering impact: UI copy and docs must distinguish canonical LifeModel truth, compatibility/current views, candidate changes, evidence, and manual override states.

Risk: English branding can feel opaque to Chinese-first users.

Reversal cost: Low before implementation; medium after navigation and docs are shipped.

Phase 2 implication: Validate route label and Chinese explanatory text before UI changes.

Human approval needed: Yes.

Constraint: Navigation may use `LifeModel`, but page subtitle/onboarding must explain it in ordinary Chinese.

## D7-diagnostics

Decision ID: D7-diagnostics
Title: Diagnostics hidden by default and available through advanced inspection
Status: Accepted

Decision: `DESIGN_DECISION` - Default product UI should show concise trust, task, blocker, permission, and review states. Raw traces, kernel events, provider/router internals, dev/test wrappers, and historical surfaces should move to advanced/developer inspection layers.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/phase0_5/04_diagnostics_visibility_inventory.md`
  Claim: Current diagnostics are useful but mixed across ProductShell, Chat, Runs, Settings, and advanced routes, creating cognitive load.
  Confidence: High
  Limitation: No human usability test was run.
- Evidence type: Existing codebase fact
  Source: `docs/openlife-phase0-audit/08_frontend_current_state_audit.md`
  Claim: Existing components expose reasoning trace, run trace, tool cards, kernel events, durable events, and advanced technical navigation.
  Confidence: High
  Limitation: Phase 1 does not decide which advanced surfaces become support-mode versus developer-only.

Product rationale: Evidence must remain available, but OpenLife should not feel like a raw developer console in normal use.

Engineering impact: ViewModel actions must split `ProductAction`, `ReviewAction`, and `DebugAction`; debug actions must not appear as default product actions.

Risk: Hiding too much can weaken trust; showing too much can overwhelm users.

Reversal cost: Medium. Visibility policy can be tuned if the underlying evidence references are preserved.

Phase 2 implication: Implement visibility policy and support/debug mode gating before removing default trace panels.

Human approval needed: Yes, for calibration/versions/MCP/A2A visibility.

## D8-viewmodel-first

Decision ID: D8-viewmodel-first
Title: Backend-owned ViewModels / ReadModels before UI implementation
Status: Accepted

Decision: `DESIGN_DECISION` - Frontend V2 implementation must not start until page-level ViewModel contracts are defined and backend ownership is verified or marked `PHASE_2_REQUIRED`.

Evidence:

- Evidence type: Verified Fact from Phase 0 / 0.5
  Source: `docs/phase0_5/06_view_model_gap_inventory.md`
  Claim: `LifeStateProjection` exists and should be preferred for shared product state, but current per-surface rows are not yet rich enough and pages still combine raw domain reads.
  Confidence: High
  Limitation: The inventory proposes candidate ViewModels; it does not implement them.
- Evidence type: Existing codebase fact
  Source: `docs/openlife-phase0-audit/05_backend_frontend_contract.md`
  Claim: `frontend/src/tauri.ts` is the product bridge, `frontend/src/tauriDev.ts` is dev/test compatibility, and structured chat results exist alongside a deprecated reply-only wrapper.
  Confidence: High
  Limitation: Phase 1 does not inspect every current command signature.

Product rationale: Users should see backend-owned truth for readiness, review, task state, memory/LifeModel state, and external-write results, not page-local guesses.

Engineering impact: Phase 2 must decide whether to expand `LifeStateProjection` or add adjacent dedicated read models. Existing helpers may remain display-only formatters, not truth owners.

Risk: If UI starts first, V2 will likely recreate current page-local state debt.

Reversal cost: High after components/routes are created.

Phase 2 implication: Define `TodayViewModel`, `WorkspaceViewModel`, `TasksViewModel`, `ReviewCenterViewModel`, `LifeModelViewModel`, `MemoryViewModel`, and `SettingsViewModel` with explicit backend owner status.

Human approval needed: Yes, for read-model scope and implementation order.
