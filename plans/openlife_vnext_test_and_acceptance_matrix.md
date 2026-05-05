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
- Shell uses sandbox.
- Deny-read blocks secrets.
- Dangerous commands blocked.
- Timeout and output limits enforced.
- Shell cannot bypass ToolRuntime.

Tests:

- shell disabled by default
- allowed command success
- denied command block
- secret path read block
- timeout
- output truncation
- env allowlist
- write operation proposal-first

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

## Manual Review Checklist

For each phase:

- Does this increase runtime authority?
- Does this reduce duplicated execution logic?
- Does this preserve user sovereignty?
- Does this improve traceability?
- Does this avoid premature sub-agent/bash complexity?
- Does this keep AI coding inside a clear spec?
