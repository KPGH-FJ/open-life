# OpenLife Stabilization and Spine Consolidation Plan

> Date: 2026-04-29
> Status: Active stabilization plan
> Scope: Fix review findings, reduce layered complexity, and consolidate the Agent Framework spine.

## 1. Current Stage

OpenLife is in late Agent Framework Alpha. The project already has real product chains:

- Workspace readiness and system diagnostics.
- LifeModel construction through Builder.
- Chat routed through LifeModel, Memory, Privacy, Scheduler, AgentRun, and Proposal generation.
- Review Center for LifeModel/Goal proposals.
- Runs page for AgentRun trace inspection.
- Memory, MCP/A2A, Calibration, VersionControl, and rollout metrics foundations.

The core problem is no longer absence of capability. The main problem is convergence:

- New Agent Framework concepts exist, but some old feature-level paths still remain.
- Frontend/backend contracts can drift because Tauri commands and TS wrappers are manually synced.
- Chat has duplicated stream and non-stream execution paths.
- Proposal currently covers LifeModel/Goal updates better than Memory/Tool governance.
- ModelRouter is structurally present but still partly experimental.

## 2. Stabilization Principles

1. Restore build and CI stability before adding new product capability.
2. Treat `AgentRun`, `AgentProposal`, `AgentAction`, `LifeModelPatch`, `Memory`, and `ModelRouter` as the architecture spine.
3. Keep existing user flows usable while removing old bypass paths gradually.
4. Prefer adapter cleanup and contract tests over broad rewrites.
5. Every high-risk LifeModel update, external write action, and sensitive memory operation should be reviewable, traceable, and reversible.

## 3. Phase Plan

### Phase 1: Contract Stabilization

Goal: Make frontend/backend contracts stable enough for daily development.

Tasks:

- Align `frontend/src/tauri.ts` with Rust serde output for AgentRun, Proposal, and SystemDiagnostics.
- Remove snake_case reads from TS pages when backend returns camelCase.
- Add aliases only where they reduce churn, and document which names are canonical.
- Add or update tests for Workspace, Runs, Review Center, and proposal creation.
- Add frontend production build to normal verification.

Acceptance criteria:

- `npm run build` passes.
- `npm test -- --run` passes.
- `cargo test -q` passes.
- Workspace and Runs no longer log `listRuns is not a function` or field-name errors.

### Phase 2: Developer Command Reliability

Goal: Avoid local workflow failures caused by package manager assumptions.

Tasks:

- Make Makefile frontend commands use pnpm when present and npm fallback otherwise.
- Keep `dev.sh` and `startup.sh` fallback behavior aligned with Makefile.
- Document canonical commands for contributors.

Acceptance criteria:

- `make -n test-front` uses `npm run test` when pnpm is absent.
- `make -n build-front` uses `npm run build` when pnpm is absent.
- The scripts still use pnpm on machines where pnpm is installed.

### Phase 3: Builder Proposal-Only Default

Goal: Remove normal user reliance on direct LifeModel writes from Builder.

Tasks:

- Make `builder_create_proposals` the default and primary Builder completion action.
- Move `builder_apply_signals` behind an explicit legacy/dev-only affordance or remove it from normal UI.
- Update tests that currently assert direct apply as the main path.
- Keep backend direct apply temporarily for migration and tests, but mark it deprecated in code comments and logs.
- Add a regression test that Builder Review creates proposals without mutating LifeModel.

Acceptance criteria:

- Normal Builder review creates Proposal records and does not directly persist LifeModel changes.
- Review Center accept/edit/reject remains the only normal write path.
- Legacy direct apply cannot be triggered accidentally from the primary UI.

### Phase 4: Chat Execution Deduplication

Goal: Reduce divergence between `send_message` and `start_stream_message`.

Tasks:

- Extract a shared `execute_chat_agent_run` or equivalent service in `src-tauri/src/lib.rs` or a new command helper module.
- Shared core should own: intent/layer resolution, preprocessing, AgentRuntime invocation, model generation, tool preparation, privacy reconstruction, persistence, proposal generation, and AgentRun updates.
- Stream and non-stream commands should only differ in transport and chunk emission.
- Add tests for shared failure paths: preprocess failure, model failure, stream fallback, proposal generation failure.

Acceptance criteria:

- Model route, context summary, reasoning trace, tool handling, and proposal generation are consistent across stream and non-stream.
- Future changes to AgentRun/Proposal need one code path change, not two.

### Phase 5: Proposal Coverage Expansion

Goal: Make Review Center the actual governance layer for memory and tool permission changes.

Tasks:

- Implement minimal `MemoryWrite` and `MemoryArchive` proposal application.
- Implement `ToolPermission` proposal state and policy storage, even if execution allowlist remains simple.
- Add clear UI labels for unsupported proposal types until fully implemented.
- Connect memory governance suggestions to ProposalEngine.
- Add tests for unsupported proposal types returning explicit errors instead of silent no-op.

Acceptance criteria:

- LifeModel/Goal, Memory, and Tool Permission proposals have explicit generated/applied/rejected states.
- Unsupported proposal types fail clearly and remain traceable.
- Proposal generated IDs are attached back to AgentRun.

### Phase 6: ModelRouter Reality Check

Goal: Move ModelRouter from structural experiment toward trustworthy routing.

Tasks:

- Rename or label estimated provider health as experimental in UI and diagnostics.
- Replace `available=true` cloud assumptions with real lightweight provider checks where keys are configured.
- Record fallback reason and failure count in ModelRouteTrace.
- Enforce privacy requirement before provider scoring.
- Add diagnostics for provider health freshness and last error.

Acceptance criteria:

- AgentRun route trace can explain provider, model, route type, privacy level, fallback, and errors.
- Cloud provider availability is not reported as healthy without evidence.
- Privacy-high tasks cannot route to cloud unless policy explicitly allows safe summarization.

### Phase 7: Documentation and CI Guardrails

Goal: Prevent the same drift from recurring.

Tasks:

- Update README and AGENTS to remove stale Hermes references and mark experimental features clearly.
- Add a lightweight contract checklist for new Tauri commands.
- Add build/typecheck to CI or the local `make ci` path.
- Consider generating TS types from Rust schema later, but do not block stabilization on it.

Acceptance criteria:

- README, AGENTS, and development plan describe the same current architecture.
- `make ci` catches type drift before runtime.
- New AgentRun/Proposal fields are added in Rust, TS wrapper, mocks, and tests together.

## 4. Immediate Next Work

Recommended next implementation order:

1. Complete Phase 1 and Phase 2.
2. Convert Builder normal completion to proposal-only.
3. Extract shared Chat execution core.
4. Add Memory/Tool proposal application stubs with explicit policy behavior.
5. Harden ModelRouter health and privacy semantics.

## 5. Current Verification Baseline

The stabilization baseline should remain:

```bash
cargo test -q
cd frontend && npm test -- --run
cd frontend && npm run build
```

