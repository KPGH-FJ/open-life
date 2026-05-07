# OpenLife vNext Test and Acceptance Matrix

Date: 2026-05-06

This matrix defines what must be tested before each vNext phase is considered ready.

## Global Gates

Before merging any vNext implementation:

- `cargo test -p openlife-core` passes, unless a task explicitly documents a narrower safe test.
- Relevant Tauri tests pass when Tauri command behavior changes.
- Frontend tests pass when UI/state changes.
- New execution behavior has tests.
- New policy behavior has denial tests, not only success tests.
- New prompt behavior has assembly/filtering tests.
- New LifeModel mutation behavior has proposal and risk tests.

## Phase 2: Execution Path Convergence

Acceptance:

- Each formal execution entrypoint maps to a runtime mode.
- Streaming and non-streaming share core semantics.
- Fallback creates trace events.
- Legacy paths are not removed until replacement tests pass.

Tests:

- normal chat no-tool path
- chat with tool path
- streaming chat no-tool path
- streaming chat with tool path
- AgentLoop failure fallback
- L1 reflex classification behavior
- scheduled task AgentLoop path

## Phase 3: AgentRunEvent + ToolRuntime

Acceptance:

- Events are append-only.
- Events can be listed by run.
- Tool attempts create events.
- Tool blocks are visible as events/observations.
- Declarative-only tools cannot be model-callable.

Tests:

- create/list AgentRunEvent
- model call event sequence
- JSON repair event sequence
- tool success event sequence
- tool blocked event sequence
- permission request event sequence
- fallback event sequence
- declarative-only prompt filtering
- declarative-only execution block
- frontend Tauri mocks updated for any new event types exposed to UI

## Phase 4: PromptStack

Acceptance:

- PromptBlock and PromptStack exist.
- Prompt assembly is deterministic.
- Prompt block versions are traceable.
- Cloud-disallowed blocks are filtered or summarized.
- ToolPrompt excludes unavailable tools.

Tests:

- base stack assembly
- role prompt inclusion
- cloud filtering
- token budget trimming
- output schema inclusion
- prompt block trace metadata
- tool prompt excludes declarative-only tools

## Phase 5: MemoryEvidence + LifeModel Evolution

Acceptance:

- MemoryEvidence links to accepted memories.
- Evolution proposals include evidence IDs.
- High-risk LifeModel changes never auto-apply.
- Contradictions are handled without confident patching.
- Rejected proposals influence future evidence scoring.

Tests:

- repeated preference evidence
- recurring goal evidence
- capability signal evidence
- state trend evidence
- contradiction evidence
- high-risk field review requirement
- rejected proposal negative evidence
- no raw unaccepted transcript as evidence

## Phase 6: AgentSpec + PlanMode

Acceptance:

- AgentSpec governs tools/context/prompt.
- Planner can use only allowed read tools.
- High-risk tasks require plan confirmation.
- Execute mode records plan deviations.

Tests:

- AgentSpec allowed tools
- AgentSpec denied tools
- Plan creation schema
- Plan confirmation required for high-risk task
- Planner cannot write external state
- Execute follows plan
- Execute deviation event

## Phase 7: SubAgentRuntime

Acceptance:

- Sub-agent has isolated context.
- Sub-agent has role-specific tool policy.
- Child AgentRun links to parent.
- Main agent can cite sub-agent result.

Tests:

- call-as-tool sub-agent success
- sub-agent denied tool
- sub-agent context isolation
- parent/child run linkage
- reviewer sub-agent output schema
- parallel mode only after call-as-tool is stable

## Phase 8: Compaction

Acceptance:

- Compaction preserves decisions, proposals, unresolved tasks, and observations.
- Compaction is traceable.
- Sensitive content is summarized under privacy policy.

Tests:

- token threshold trigger
- summary contains active proposals
- summary contains unresolved tool observations
- summary excludes redacted sensitive content
- future response can use compacted context

## Phase 9: Bash / Sandbox

Acceptance:

- Bash default-off.
- Shell uses `ExecutionSandbox`.
- Deny-read blocks secrets.
- Dangerous commands blocked.
- Timeout and output limits enforced.
- Shell cannot bypass ToolRuntime.
- `shell.run` is not model-callable unless sandbox, manifest, permission, and AgentSpec all allow it.
- Initial executor is non-interactive and structured, not raw shell-string based.
- Scheduled/proactive and sub-agent shell access remain disabled by default.

Tests:

- shell disabled by default
- manifest disabled/declarative-only blocks execution
- allowed command success
- denied command block
- secret path read block
- timeout
- output truncation
- env allowlist
- write operation proposal-first
- AgentSpec-denied shell block
- scheduled/proactive disabled sandbox
- trace events for blocked/completed/failed shell attempts

## Phase 10: Frontend Agent Workspace

Acceptance:

- Streaming remains stable.
- Run timeline displays event data.
- Tool panel displays observations.
- Proposal panel displays evidence links.
- Plan UI supports confirm/edit/reject.

Tests:

- ChatPage streaming regression
- pending proposal banner regression
- run timeline rendering
- tool observation rendering
- memory evidence rendering
- plan confirmation interaction

## P5: Governed Plan Operations and Recovery

Acceptance:

- Plan operation command results use a stable frontend/backend contract.
- `cancel_agent_plan` only cancels legal non-terminal states.
- `retry_agent_plan` only retries `failed` and `failed_review` plans.
- Retry appends a new attempt marker and preserves prior events.
- Blocked action continuation uses existing Permission / Proposal / Replay policy.
- Rollback implementation is blocked until ADR 0011 is accepted.
- Real ReviewAgent integration remains read-only and traceable.
- Minimal plan operations UI does not destabilize ChatPage streaming or trace.

Tests:

- plan operation result contract normalization
- cancel legal state success
- cancel illegal terminal state rejected
- retry failed plan appends events
- retry completed/rejected plan rejected
- blocked action continuation records replay events
- ReviewAgent cannot call write tools
- ChatPage plan operation regression

## P6: AgentSpec-Governed Runtime and Context Assembly

Acceptance:

- AgentSpec is a stable runtime identity and policy contract.
- AgentTask separates intent/constraints from AgentRun trace.
- ContextAssembler can produce event-safe context summaries.
- AgentSpec tool policy blocks denied tools without bypassing ToolRuntime or Permission.
- PromptStack consumes AgentSpec prompt references without ad hoc prompt fragments.
- PlanExecutor respects AgentSpec policy for plan tool intents.
- Minimal trace UI can display AgentSpec governance decisions.

Tests:

- AgentSpec serde round-trip
- default/main AgentSpec construction
- AgentTask serde round-trip
- ContextPolicy allow/deny categories
- event-safe context summary redaction
- AgentSpec-allowed tool executes only when ToolRuntime allows it
- AgentSpec-denied tool records block
- PromptStack assembles AgentSpec prompt block ids
- missing prompt block structured error
- PlanExecutor blocks AgentSpec-denied plan tool
- RunTracePanel renders AgentSpec metadata and block summary

## P7: AgentSpec Store, Runtime Selection, and Governed Agent Entry Points

Acceptance:

- AgentSpecStore persists and retrieves AgentSpec records.
- Default main AgentSpec is bootstrapped with a stable id.
- Tauri AppState exposes AgentSpecStore-backed commands.
- AgentRuntime can execute with a resolved AgentSpec.
- PromptStack assembly uses selected AgentSpec prompt block ids.
- ContextPolicy is derived from selected AgentSpec policy fields.
- Plan execution resolves stored AgentSpec instead of hardcoded defaults.
- Frontend/backend contract can read the current default AgentSpec.

Tests:

- default main spec bootstrap
- AgentSpecStore create/get/list/update/activate
- unknown spec id structured error
- Tauri get/list/update/default AgentSpec commands
- frontend wrapper and mock AgentSpec contract
- runtime with AgentSpec prompt block ids
- missing prompt block fails before reasoning
- AgentSpec without memory access excludes memory
- AgentSpec without LifeModel access excludes LifeModel summary
- plan execution uses stored default AgentSpec
- plan-bound AgentSpec-denied tool blocks before execution
- trace payload includes `agentspec_id`

## Manual Review Checklist

For each phase:

- Does this increase runtime authority?
- Does this reduce duplicated execution logic?
- Does this preserve user sovereignty?
- Does this improve traceability?
- Does this avoid premature sub-agent/bash complexity?
- Does this keep AI coding inside a clear spec?
