# RuntimeStrategy / Multi-Strategy Runtime Maturity Goal Spec

> Last updated: 2026-06-03
> Status: completed CLI Goal-mode implementation spec / audit trail for W106-W113

This document is the CLI Goal-mode handoff for the next architecture block:
mature the RuntimeStrategy and MultiStrategy Runtime layer from a lightweight
adapter/orchestrator into a governed, inspectable, metadata-safe strategy
protocol.

The intended use is direct: start Codex CLI from the repository root, point it
to this file, and ask it to implement the full Goal. The Agent may complete the
whole block in one sustained run, but must internally keep the W106-W113 order,
prove each slice with tests, and stop only after final verification. The Agent
must not commit or push unless the user asks after review.

## 1. Current Baseline

The authoritative baseline is **W105 Plan-Execute Product Vertical complete**.

The Agent must read these files before editing code:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/lifemodel_governed_runtime_progress.md`
4. `plans/openlife_lifemodel_governed_agent_runtime.md`
5. `plans/plan_execute_product_vertical_goal_spec.md`
6. `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
7. `plans/openlife_react_beta_roadmap.md`
8. `openlife-core/src/agent/strategy.rs`
9. `openlife-core/src/agent/strategy_runtime.rs`
10. `openlife-core/src/agent/multi_strategy_runtime.rs`
11. `openlife-core/src/agent/tests/strategy.rs`
12. `openlife-core/src/agent/tests/multi_strategy_runtime.rs`
13. `src-tauri/src/commands/agent_runtime/mod.rs`
14. `frontend/src/utils/previewAudit.ts`
15. `frontend/src/utils/planExecuteProduct.ts`
16. `frontend/src/components/RunTracePanel.tsx`
17. `frontend/src/pages/RunsPage.tsx`

Current completed preparation:

- `RuntimeStrategy` trait exists in `strategy_runtime.rs`.
- Fixed ReAct and PlanExecute adapters exist.
- `RuntimeStrategyRegistry` can register and retrieve strategy adapters.
- `StrategySelector` chooses ReAct or PlanExecute from metadata-safe intent,
  governor decision, planning allowance, and local model availability.
- `MultiStrategyRuntime` orchestrates StrategySelector plus selected adapter.
- `run_multi_strategy_agent_preview` exposes MultiStrategy Runtime as an
  explicit non-default preview command and persists metadata-safe AgentRun audit.
- W98-W105 added a real non-default Plan-Execute weekly planning product
  vertical with durable session lifecycle, proposal-first step execution, and
  Plan-Execute product trace visibility.

Known limitations that remain real:

- Runtime strategies do not have first-class capability descriptors.
- Registry readiness is implicit; there is no report proving required
  strategies are present, unique, metadata-safe, and policy-compatible.
- Selection output has a small summary, but not a full candidate matrix or
  human-reviewable explanation.
- `RuntimeStrategyOutput.metadata_safe_summary` is not preserved as a stable
  top-level MultiStrategy runtime execution report.
- Preview trace and Plan-Execute product trace use related but separate shapes.
- There is no non-default read-only status command that reports runtime strategy
  registry/selection maturity without running a preview.
- Future strategies such as Workflow, Proactive, Reflective, Direct, or Layered
  have no declarative boundary in the runtime strategy taxonomy.
- Default Chat remains `legacy_stream` and must remain unchanged.

## 2. Goal Objective

Complete W106-W113: RuntimeStrategy / Multi-Strategy Runtime Maturity.

The final state must satisfy all of the following:

- Runtime strategies have metadata-safe capability descriptors.
- Registry readiness can be evaluated without running model/runtime/tool calls.
- Strategy selection has a metadata-safe candidate matrix and blocking/fallback
  explanation.
- MultiStrategy runtime output preserves a stable execution report that includes
  selector, registry, strategy descriptor, payload kind, governance state, and
  side-effect budget.
- Preview and product trace metadata share a common strategy trace contract
  vocabulary.
- Future strategy kinds are represented as non-executable/declarative
  placeholders unless implemented and registered with full governance.
- A non-default read-only Tauri status command can report strategy runtime
  maturity.
- Default Chat remains `legacy_stream` and ordinary `send_message` /
  `start_stream_message` do not call W106-W113 helpers or commands.
- Docs and progress index are synchronized to W113.

This Goal is not a default Chat migration and not ReAct Beta execution
hardening. It makes the strategy layer safer and clearer before those later
blocks.

## 3. Non-Negotiable Invariants

Do not change these invariants:

- Do not migrate default Chat.
- Do not replace ordinary `send_message` or `start_stream_message`.
- Do not call W106-W113 registry/status/readiness/maturity helpers from ordinary
  Chat entrypoints.
- Do not add any automatic runtime strategy switch for default Chat.
- Do not execute model/tool/runtime calls from readiness/status commands.
- Do not create AgentRuns, Proposals, Evidence, Memory, LifeModel patches, MCP
  audit rows, external writes, or Chat messages from readiness/status commands.
- Do not treat strategy readiness, registry readiness, candidate matrix ready,
  or status command pass as migration permission.
- Do not implement Workflow/Proactive/Reflective execution in this block. They
  may be represented only as disabled/declarative future strategy descriptors.
- Do not store raw prompt, raw assistant output, raw LifeModel text, raw memory
  content, raw tools prompt, raw tool payload, raw plan prose, raw proposal
  payload, or user PII in strategy reports, debug dumps, registry status, or
  trace metadata.
- Do not loosen Proposal-first or W97 governed write constraints.
- Do not commit or push unless the user explicitly asks after review.

## 4. Target Architecture Shape

Prefer small, typed additions to existing modules rather than a broad rewrite.

Recommended core additions:

- `RuntimeStrategyCapability`
- `RuntimeStrategySideEffectBudget`
- `RuntimeStrategyDescriptor`
- `RuntimeStrategyRegistryReadinessReport`
- `StrategyCandidateEvaluation`
- `StrategySelectionReport`
- `RuntimeStrategyExecutionReport`
- `MultiStrategyRuntimeMaturityReport`

Recommended command addition:

- `get_runtime_strategy_registry_status`
  or `check_multi_strategy_runtime_maturity`

Recommended trace vocabulary:

- `runtimeStrategyTraceKind`
- `selectedStrategyKind`
- `payloadKind`
- `strategyDescriptorId`
- `strategyCapabilityIds`
- `selectionReasonCode`
- `governanceDecisionKind`
- `sideEffectBudget`
- `registryReady`
- `metadataSafe`
- `defaultChatUnchanged`

Names may differ if the existing code style suggests a cleaner local pattern,
but the semantics above must exist and be tested.

## 5. Strategy Taxonomy Boundary

At W113, the taxonomy should be explicit:

| Strategy | Execution status in this Goal | Required behavior |
| --- | --- | --- |
| ReAct | Implemented adapter | Current default strategy for open-ended/tool tasks inside MultiStrategy preview, not default Chat route replacement |
| PlanExecute | Implemented adapter + W105 product vertical | Planning/write-like governed strategy; write-like steps remain proposal-first |
| Direct | Declarative/future or existing reasoning strategy only | Do not register as executable RuntimeStrategy unless full contract exists |
| Layered | Declarative/future or existing reasoning strategy only | Do not register as executable RuntimeStrategy unless full contract exists |
| Workflow | Disabled/declarative future descriptor only | No execution in W106-W113 |
| Proactive | Disabled/declarative future descriptor only | No execution in W106-W113 |
| Reflective | Disabled/declarative future descriptor only | No execution in W106-W113 |

Disabled/declarative descriptors must not appear as executable capabilities in
UI or command output.

## 6. Implementation Strategy

The Agent should complete W106-W113 in one Goal run, but implement in this exact
internal order:

1. W106 RuntimeStrategy capability descriptor and registry readiness
2. W107 Strategy selection candidate matrix and explanation
3. W108 MultiStrategy runtime execution report envelope
4. W109 Non-default strategy registry/maturity status command
5. W110 Preview/product strategy trace convergence
6. W111 Future strategy boundary descriptors
7. W112 Default Chat isolation and side-effect regression hardening
8. W113 Docs, progress index, and final verification sync

Run targeted tests after each major code area when practical. Run the full
verification matrix at the end.

If a slice becomes too large, keep the maturity layer backend-only and
metadata-safe rather than adding a broad frontend surface. A small read-only
status surface is acceptable; runtime execution scope must stay unchanged.

## 7. W106 Spec: Capability Descriptor And Registry Readiness

### Scope

Primary files:

- `openlife-core/src/agent/strategy_runtime.rs`
- `openlife-core/src/agent/mod.rs`
- `openlife-core/src/agent/tests/multi_strategy_runtime.rs`
- optionally new focused tests under `openlife-core/src/agent/tests/`

### Required Behavior

- Add metadata-safe descriptors for executable RuntimeStrategy adapters.
- Each executable strategy descriptor must include:
  - strategy kind
  - metadata-safe id/name
  - payload kind
  - capability ids
  - supported task categories
  - write policy
  - tool/model/runtime side-effect budget declaration
  - proposal-first requirement
  - metadata-safe trace support
  - default Chat migration permission fixed false
- Add registry readiness evaluator/report.
- Readiness must fail closed when:
  - required ReAct adapter is missing
  - required PlanExecute adapter is missing
  - duplicate strategy kind or descriptor id exists
  - descriptor/payload kind mismatch exists
  - a strategy claims writes/external side effects without proposal-first
  - a strategy claims default Chat migration permission
  - descriptors contain raw content
- Registry readiness must not execute any strategy.

### Required Tests

- ReAct and PlanExecute descriptors are present and metadata-safe.
- Registry readiness passes for the fixed ReAct/PlanExecute registry.
- Registry readiness fails for missing adapter.
- Registry readiness fails for duplicate descriptor/kind.
- Registry readiness fails if descriptor grants default Chat migration.
- Registry readiness/debug output excludes raw prompt, tools prompt, LifeModel,
  memory, assistant output, and tool payload.
- Readiness evaluation does not execute adapters or mutate stores.

### W106 Done Criteria

- RuntimeStrategy registry can prove its executable strategy set is ready and
  metadata-safe without running runtime/model/tool logic.

## 8. W107 Spec: Selection Candidate Matrix And Explanation

### Scope

Primary files:

- `openlife-core/src/agent/strategy.rs`
- `openlife-core/src/agent/tests/strategy.rs`
- `openlife-core/src/agent/multi_strategy_runtime.rs`

### Required Behavior

- Extend strategy selection with a metadata-safe candidate matrix/report.
- Candidate evaluations should include:
  - strategy kind
  - candidate supported/not supported
  - reason code
  - governance decision kind
  - risk level
  - planning allowed
  - local model available
  - has HS packet
  - blocked/fallback status
- Preserve existing selection behavior unless tests prove an existing behavior
  was unsafe.
- Keep broad tools prompt from implying write/external side-effect intent.
- Keep raw user text out of reports.
- Keep blocked local-only behavior: selection may identify a candidate kind, but
  runtime execution must remain blocked.

### Required Tests

- Simple chat candidate matrix selects ReAct with reason `default_react`.
- Planning intent selects PlanExecute when planning is allowed.
- Planning intent falls back to ReAct when planning is disabled.
- Write-like intent selects PlanExecute for governed planning but does not
  execute writes.
- Sensitive local-only without local model returns blocked explanation.
- Candidate matrix is metadata-safe and excludes raw prompt, PII, tools prompt,
  LifeModel, memory context.
- Candidate matrix does not mutate RuntimeInput or stores.

### W107 Done Criteria

- StrategySelector produces a reviewable metadata-safe explanation, not only a
  final enum selection.

## 9. W108 Spec: MultiStrategy Runtime Execution Report Envelope

### Scope

Primary files:

- `openlife-core/src/agent/multi_strategy_runtime.rs`
- `openlife-core/src/agent/strategy_runtime.rs`
- `openlife-core/src/agent/tests/multi_strategy_runtime.rs`

### Required Behavior

- Preserve `RuntimeStrategyOutput.metadata_safe_summary` in a stable
  MultiStrategy runtime execution report.
- The report must include:
  - runtime kind / report kind
  - selected strategy kind
  - payload kind
  - strategy descriptor id/name
  - registry readiness result
  - selection reason code
  - governance decision kind
  - blocked state
  - warning count
  - side-effect budget summary
  - default Chat unchanged
  - migration permission false
  - metadata safe true
- Blocked outputs must include an execution report even when no adapter runs.
- Missing strategy must fail closed with metadata-safe error.
- Existing `MultiStrategyRuntimePayload` behavior must remain compatible.

### Required Tests

- ReAct path includes execution report and selected descriptor metadata.
- PlanExecute path includes execution report and selected descriptor metadata.
- Blocked local-only path includes report and executes no adapter.
- Missing strategy fails closed with sanitized error and no raw input.
- Execution report is metadata-safe and excludes raw content.
- Adapter output summaries are preserved without leaking raw prompt/tools.

### W108 Done Criteria

- MultiStrategy Runtime output has a stable audit envelope suitable for preview,
  product trace, and future status surfaces.

## 10. W109 Spec: Non-Default Strategy Registry / Maturity Status Command

### Scope

Primary files:

- `src-tauri/src/commands/agent_runtime/mod.rs`
- possibly a new `runtime_strategy_status.rs` submodule
- `src-tauri/src/lib.rs`
- `frontend/src/tauri.ts` and `frontend/src/types.ts` only if a frontend wrapper
  is useful
- Settings UI only if a small read-only panel is straightforward

### Required Behavior

- Add an explicit non-default read-only command, recommended:
  - `get_runtime_strategy_registry_status`
  - or `check_multi_strategy_runtime_maturity`
- The command must:
  - evaluate registry readiness
  - report executable strategies
  - report disabled/declarative future strategies if W111 lands before the
    command is finalized
  - report default Chat unchanged
  - report migration permission false
  - report no runtime/model/tool execution
  - report no business writes
  - return metadata-safe blocking reasons
- The command must not:
  - run MultiStrategy preview
  - create AgentRun
  - create Proposal/Evidence/Memory/LifeModel patch
  - inspect raw current chat input
  - be called from ordinary Chat entrypoints

### Required Tests

- Command returns registry/maturity ready for current ReAct/PlanExecute setup.
- Command is read-only by side-effect counts.
- Command output is metadata-safe and raw-content-free.
- Command fails closed or reports blockers if registry readiness is invalid.
- Ordinary `send_message` / `start_stream_message` do not call the command.

### W109 Done Criteria

- There is a safe non-default status surface for runtime strategy maturity.

## 11. W110 Spec: Preview/Product Strategy Trace Convergence

### Scope

Primary files:

- `src-tauri/src/commands/agent_runtime/mod.rs`
- `src-tauri/src/commands/agent_runtime/plan_execute_product.rs`
- `frontend/src/utils/previewAudit.ts`
- `frontend/src/utils/planExecuteProduct.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/RunsPage.tsx`
- related frontend tests/mocks

### Required Behavior

- Converge preview and product trace metadata around a shared strategy trace
  vocabulary.
- Do not remove existing fields that current UI/tests depend on.
- Add shared fields where useful:
  - `runtimeStrategyTraceKind`
  - `selectedStrategyKind`
  - `payloadKind`
  - `strategyDescriptorId`
  - `selectionReasonCode`
  - `registryReady`
  - `metadataSafe`
  - `defaultChatUnchanged`
- Preview AgentRun remains the outer run for preview. Any inner ReAct run id
  remains child metadata only and must not replace the queryable preview run id.
- Plan-Execute product trace remains tied to explicit plan sessions.
- Runs/Trace UI should render both preview and product traces without raw
  content and without implying Chat migration.

### Required Tests

- MultiStrategy preview trace includes shared strategy trace vocabulary.
- Plan-Execute product trace includes shared strategy trace vocabulary.
- UI renders shared metadata for both shapes.
- Search text includes strategy trace metadata but not raw output.
- Raw prompt, raw plan prose, raw assistant output, raw LifeModel, raw memory,
  raw tool payload, and raw proposal payload do not render.
- Existing preview/product trace tests remain compatible.

### W110 Done Criteria

- Strategy trace visibility is coherent across preview and the W105 product
  vertical.

## 12. W111 Spec: Future Strategy Boundary Descriptors

### Scope

Primary files:

- `openlife-core/src/agent/strategy_runtime.rs`
- `openlife-core/src/agent/strategy.rs`
- `openlife-core/src/agent/tests/multi_strategy_runtime.rs`
- docs as needed

### Required Behavior

- Add declarative descriptors for future strategy kinds only if the enum/type
  design can represent them without accidentally making them executable.
- Acceptable future descriptors:
  - Direct
  - Layered
  - Workflow
  - Proactive
  - Reflective
- Future descriptors must be:
  - disabled or declarative-only
  - not registered as executable adapters
  - excluded from runtime selection unless explicitly implemented later
  - excluded from default Chat authority
  - represented in readiness/status as future/non-executable, not missing
    blockers
- If adding enum variants would create too much churn, use a separate
  declarative strategy descriptor id type instead of changing executable
  `RuntimeStrategyKind`.

### Required Tests

- Future descriptors appear in status/readiness as declarative-only.
- Future descriptors are not executable and cannot be selected by current
  selector.
- Disabled/declarative strategies cannot grant write/model/tool/default Chat
  authority.
- Registry readiness distinguishes required executable strategies from future
  declarative descriptors.

### W111 Done Criteria

- The project has an explicit future strategy taxonomy without fake executable
  capabilities.

## 13. W112 Spec: Default Chat Isolation And Side-Effect Hardening

### Scope

Primary files:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/agent_runtime/mod.rs`
- `openlife-core/src/agent/tests/strategy.rs`
- `openlife-core/src/agent/tests/multi_strategy_runtime.rs`

### Required Behavior

- Update default Chat forbidden-call tests to include W106-W113 commands and
  helpers.
- Add or strengthen tests proving:
  - status/readiness commands create no AgentRun/Proposal/Evidence/Memory/
    LifeModel/MCP audit/Chat/external write records
  - registry readiness does not run adapters
  - selection explanation does not run adapters
  - strategy readiness is not migration permission
  - broad tools prompt catalog does not imply writes
  - `allowWrites=false` remains enforced for preview paths
- Ensure any new command is non-default and explicit.

### Required Tests

- Default Chat ordinary entrypoint isolation passes.
- Side-effect counts remain unchanged for status/readiness commands.
- Preview command behavior remains unchanged and write-disabled.
- Plan-Execute product command behavior remains unchanged and proposal-first.

### W112 Done Criteria

- The maturity layer is provably non-default, side-effect-free where read-only,
  and not migration authority.

## 14. W113 Spec: Docs, Progress Index, And Final Verification Sync

### Scope

Primary files:

- `AGENTS.md`
- `plans/README.md`
- `plans/lifemodel_governed_runtime_progress.md`
- `plans/openlife_lifemodel_governed_agent_runtime.md`
- `plans/openlife_development_plan.md` if stale PlanExecute status remains
  misleading
- this file

### Required Behavior

- Update docs from W105 baseline to W113 RuntimeStrategy / Multi-Strategy
  Runtime Maturity complete.
- Keep `plans/README.md` as the authority map.
- Keep progress index compact and structured.
- State explicitly:
  - default Chat remains `legacy_stream`
  - W106-W113 is not default Chat migration
  - W106-W113 is not ReAct Beta execution hardening
  - ReAct and PlanExecute executable strategies are descriptor/registry ready
  - future strategies are declarative-only unless separately implemented
  - strategy readiness/status is not migration permission
- Remove or qualify stale text that still describes weekly planning as lacking
  a PlanExecute product surface.

### Required Tests

- `rg` checks confirm docs mention W113 consistently.
- `rg` checks confirm old docs do not misleadingly describe the W105
  PlanExecute weekly planning product surface as pending.
- `git diff --check` passes.
- Full CI passes.

### W113 Done Criteria

- RuntimeStrategy / Multi-Strategy Runtime maturity block is complete.
- The next block can move to ReAct Beta Execution Hardening with a more stable
  strategy protocol underneath.

## 15. Final Verification Matrix

Run all applicable targeted tests, then full CI.

Minimum required commands:

```bash
cargo test -p openlife-core strategy -- --nocapture
cargo test -p openlife-core multi_strategy_runtime -- --nocapture
cargo test -p openlife-core runtime_strategy -- --nocapture
cargo test -p openlife-tauri agent_runtime -- --nocapture
cargo test -p openlife-tauri runtime_strategy -- --nocapture
cargo test -p openlife-tauri plan_execute -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
cd frontend && pnpm test -- --run
git diff --check
make ci
```

If no `runtime_strategy` test filter exists at the start of this Goal, create
focused tests whose names include `runtime_strategy` so the command is valid.

Before final handoff, also run focused search checks:

```bash
rg -n "runtime_strategy|RuntimeStrategy|MultiStrategyRuntimeMaturity|W113" AGENTS.md plans/README.md plans/lifemodel_governed_runtime_progress.md openlife-core/src src-tauri/src frontend/src
rg -n "weekly planning.*pending product surface|PlanExecute product surface had not yet shipped" plans AGENTS.md README.md
rg -n "send_message|start_stream_message" src-tauri/src/lib.rs
```

Use the search results to prove W113 docs are synchronized, stale PlanExecute
status is corrected or scoped as historical, and ordinary Chat entrypoint bodies
do not invoke W106-W113 helpers or commands.

## 16. Handoff Output Requirements

When the Agent finishes, it must output:

- change summary by W-slice
- new core interfaces/reports
- new commands or frontend wrappers/surfaces
- tests run and results
- any skipped tests with reason
- residual risks
- whether W113 is complete
- whether the next big block can start

The Agent must not commit or push.

## 17. CLI Goal Prompt

Use this prompt in Codex CLI:

```text
You are implementing the next OpenLife big development block:
RuntimeStrategy / Multi-Strategy Runtime Maturity W106-W113.

Read and follow:
- AGENTS.md
- plans/README.md
- plans/runtime_strategy_maturity_goal_spec.md
- plans/lifemodel_governed_runtime_progress.md
- plans/openlife_lifemodel_governed_agent_runtime.md
- plans/adr/0013-lifemodel-hs-source-of-truth-governance.md
- plans/openlife_react_beta_roadmap.md

Current baseline:
- W105 Plan-Execute Product Vertical is complete.
- ReAct and PlanExecute RuntimeStrategy adapters exist.
- MultiStrategyRuntime exists as explicit non-default preview/core orchestrator.
- Default Chat remains legacy_stream.

Goal:
Implement W106-W113 in one sustained Goal run, keeping the internal order:
1. W106 RuntimeStrategy capability descriptor and registry readiness.
2. W107 Strategy selection candidate matrix and explanation.
3. W108 MultiStrategy runtime execution report envelope.
4. W109 non-default registry/maturity status command.
5. W110 preview/product strategy trace convergence.
6. W111 future strategy boundary descriptors.
7. W112 default Chat isolation and side-effect hardening.
8. W113 docs/progress/authority sync.

Hard constraints:
- Do not migrate default Chat.
- Do not replace send_message or start_stream_message.
- Do not call W106-W113 commands/helpers from ordinary Chat entrypoints.
- Do not run model/tool/runtime calls from readiness/status commands.
- Do not create AgentRun/Proposal/Evidence/Memory/LifeModel/MCP audit/Chat/external write records from readiness/status commands.
- Do not implement Workflow/Proactive/Reflective execution in this block; future strategy descriptors must be disabled/declarative-only.
- Do not store raw prompt, assistant output, LifeModel text, memory content, tools prompt, tool payload, plan prose, proposal payload, or PII in strategy reports/traces/status.
- Do not treat registry readiness, selection explanation, or maturity status as migration permission.
- Do not commit or push.

Expected product:
- RuntimeStrategy descriptors and registry readiness are metadata-safe and testable.
- StrategySelector emits candidate matrix/explanation.
- MultiStrategyRuntime preserves a stable execution report envelope.
- A non-default read-only status command reports strategy maturity without side effects.
- Preview and Plan-Execute product traces share strategy trace vocabulary.
- Future strategies are declared without fake executable authority.
- Docs are synced to W113.

Required verification:
- cargo test -p openlife-core strategy -- --nocapture
- cargo test -p openlife-core multi_strategy_runtime -- --nocapture
- cargo test -p openlife-core runtime_strategy -- --nocapture
- cargo test -p openlife-tauri agent_runtime -- --nocapture
- cargo test -p openlife-tauri runtime_strategy -- --nocapture
- cargo test -p openlife-tauri plan_execute -- --nocapture
- cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
- cd frontend && pnpm test -- --run
- git diff --check
- make ci

Final output only:
- W106-W113 change summary
- new interfaces/commands/surfaces
- tests run and results
- residual risks
- whether W113 is complete
- whether the next big block can start
```
