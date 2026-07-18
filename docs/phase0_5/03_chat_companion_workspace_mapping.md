# Chat / Companion / Mailbox / Runs Capability Mapping

## Existing Capability Inventory

| Capability | Current Surface | Source files | Product meaning | V2 candidate placement |
| --- | --- | --- | --- | --- |
| User input | Chat inside `/companion` | `frontend/src/pages/ChatPage.tsx`; `frontend/src/pages/chat/ChatInputArea.tsx` | User asks OpenLife to think, plan, act, or remember. | `工作区` primary composer. |
| Natural language intent | Chat/backend runtime | `frontend/src/tauri.ts`; `src-tauri/src/main_chat_turn_runtime.rs`; `openlife-core/src/agent/main_chat_agent_v1.rs` | Backend builds intent/route/task evidence, but frontend does not expose an editable intent object. | `工作区` as "OpenLife 理解为..." panel. |
| Skill selection | Chat | `ChatPage.tsx`; `ChatInputArea.tsx`; `listMainChatSkills`; `getMainChatSkillDetail`; `listMainChatToolCandidates` | User can add skill/tool context to a turn. | `工作区` secondary control; advanced details behind inspection. |
| Task session | Chat, Runs, Run detail | `ChatPage.tsx`; `RunsPage.tsx`; `AgentRunDetail.tsx`; `frontend/src/tauri.ts` task commands | Main Chat work is tracked as task sessions, not only messages. | `工作区` current task plus `任务` list/history. |
| Task resume | Chat, Mailbox, Runs, Run detail | `resumeMainChatAgentTask`; `MailboxPage.tsx`; `RunsPage.tsx`; `AgentRunDetail.tsx` | User can continue blocked/waiting tasks after review or context refresh. | `工作区` for current task; `任务` for history; `审核中心` after proposal approval. |
| Task cancel | Chat, Runs, Run detail | `cancelMainChatAgentTask`; `MainChatExecutionEvidence`; `AgentControlPlane` | User can stop active task work. | `工作区` and `任务`. |
| Retry | Chat, Runs, Run detail | `retryMainChatAgentAction`; `ToolCallCard`; task detail controls | User can retry failed action/task states. | `工作区` and `任务`. |
| Reasoning trace | Chat, Run detail | `ReasoningTracePanel`; `RunTracePanel`; `frontend/src/tauri.ts` `ReasoningTrace` | Diagnostic route/strategy/runtime reasoning evidence. | `高级检查` drawer, not default flow. |
| Kernel events | Chat | `MainChatExecutionEvidence`; `main-chat-kernel-event` listener in `ChatPage.tsx`; backend kernel event types | Turn started/context loaded/route/tool/final/blocker events. | Condensed `工作区` timeline; raw events in `高级检查`. |
| Durable agent events | Chat | `listMainChatAgentEvents`; `getMainChatAgentStateSnapshot`; `AgentControlPlane` | Backend durable task/run evidence, including replay gap handling. | `工作区` timeline plus `高级检查`. |
| Tool calls | Chat, Run detail | `ToolCallCard`; `RunTracePanel`; `ToolGateway` evidence via `frontend/src/tauri.ts` | Shows status, risk, permission, blocked/pending/success states. | Summary in `工作区`; details in `高级检查`; permission decisions in `审核中心`. |
| Blockers | Chat, Runs, Run detail | `MainChatExecutionEvidence`; `AgentControlPlane`; `RuntimeDisclosureStrip`; `RunsPage.tsx` | Explicit fail-closed state. | Default visible in `工作区` and `任务`; never flattened into success. |
| Generated proposals | Chat, Mailbox, LifeModel | `AgentControlPlane`; `MailboxPage.tsx`; `LifeModelPage.tsx`; `ProposalDisplay` helpers | Candidate changes require review before durable write. | `审核中心` as decision owner; linked preview in `工作区`. |
| Pending review | Today, Chat, Mailbox, Settings, LifeModel | `LifeStateProjection`; `reviewRequiredCountFromProjection`; `MailboxPage.tsx` | Product-level "needs user action" count. | `审核中心` authority, surfaced globally. |
| Final delivery | Chat, Run detail | `MainChatAgentStateSnapshot.finalDelivery`; `AgentControlPlane`; `MainChatExecutionEvidence`; backend final gate | Separates completed actions, proposals, blockers, pending user actions. | `工作区` result; details in `任务`. |
| Run history | Runs | `listAgentRuns`; `listMainChatAgentTasks`; `RunsPage.tsx` | History of work and evidence. | `任务`. |
| Run detail | `/runs/:runId` | `AgentRunDetail.tsx`; `RunTracePanel` | Deep evidence/timeline/control view. | `任务` detail, with advanced sections collapsible. |
| Execution transcript | Chat, Run detail | `execution_transcript`; task detail transcript; `AgentRunDetail.tsx` timeline builder | Auditable sequence of observations, actions, errors, final result. | Condensed timeline in `工作区`; full transcript in `高级检查` or task detail. |
| Memory impact | Chat, Mailbox, Memory, LifeModel | `memoryGovernanceStatusLabels`; `MailboxPage`; `MemorySearch`; `LifeModelPage` | Distinguishes local life events, pending memory, LifeModel proposals. | `审核中心` for decisions; `记忆`/`LifeModel` for resulting state. |
| LifeModel impact | Mailbox, LifeModel, Builder | `proposalDisplay`; `LifeModelPage`; `BuilderPage` | Proposals and accepted/current LifeModel views. | `审核中心` decisions; `LifeModel` trust/provenance view. |

Finding: `CompanionPage` is not an independent capability surface; it renders `AgentStage` plus `ChatPage companionMode`.
Evidence: `CompanionPage.tsx` imports `ChatPage` and passes `companionMode` / `onCompanionStageChange`.
File location: `frontend/src/pages/CompanionPage.tsx`.
Confidence: High.
Impact: V2 can merge Companion and Chat into one workspace without losing a separate backend route, pending human approval.

Finding: Chat currently owns too many workflow concerns in one page.
Evidence: `ChatPage.tsx` manages sessions, messages, skills, streaming, diagnostics, LifeModel, daily goals, runs, task state/detail, event stream, proposals, plan controls, and task controls.
File location: `frontend/src/pages/ChatPage.tsx`.
Confidence: High.
Impact: V2 should split "current work" ViewModel from raw diagnostics, history, and review decisions.

## Proposed V2 Workspace Model

Candidate only. Do not implement in Phase 0.5.

- User goal: current input plus conversation/task context.
- Agent understanding: visible, correctable summary of intent, privacy boundary, route, and selected skill.
- Plan / task lifecycle: one current task state with explicit `running`, `waiting_permission`, `blocked`, `failed`, `cancelled`, `completed`.
- Execution timeline: human-readable stages from kernel events/final delivery; raw event details hidden by default.
- Review-needed items: proposals, permission requests, and blockers linked to `审核中心`.
- Result: final answer plus completed actions, skipped/blocked work, evidence, and next recommended control.
- Evidence / advanced inspection: reasoning trace, kernel events, durable events, tool call metadata, route/provider proof, transcript.

Finding: Backend/bridge structures already support this shape, but the frontend does not expose it as one coherent ViewModel.
Evidence: `MainChatAgentStateSnapshot`, `MainChatAgentTaskState`, `MainChatTaskSummary`, `RunEvidenceView`, `LifeStateProjection`, `StreamMessageDonePayload`, `ReasoningTrace`, `ToolCallResult`.
File location: `frontend/src/tauri.ts`; `src-tauri/src/life_state_projection.rs`; `src-tauri/src/main_chat_turn_runtime.rs`.
Confidence: High.
Impact: V2 should define the ViewModel before moving any route/component code.

## What Should Move To Review Center

Candidate `审核中心` ownership:

- Proposal accept/reject/postpone/edit decisions.
- Tool permission decisions and permission-linked task resume.
- External write action review.
- Memory write/archive review.
- LifeModel update proposals.
- Model policy changes.
- High-risk data export/import approvals.
- "Proposal created" final delivery sections that are not durable completion.

Finding: Mailbox already implements most review decisions but is named and shaped as an inbox.
Evidence: `MailboxPage` folders `待确认`, `已同意`, `已处理`, `已修改待处理`; proposal actions; task resume after matching proposal approval.
File location: `frontend/src/pages/MailboxPage.tsx`; `frontend/src/utils/reviewDecision.ts`; `frontend/src/utils/proposalDisplay.ts`.
Confidence: High.
Impact: Human review should decide whether Mailbox becomes `审核中心` and which decisions belong there.

## What Should Move To Advanced Inspection

Candidate advanced inspection:

- Raw reasoning trace.
- Raw kernel events.
- Durable event stream replay/gap details.
- Full execution transcript.
- Tool manifest internals and sanitized arguments.
- Provider route proof rows and estimated/probed provider details.
- PolicyRouter authority chain.
- ModelRouter provider health.
- MCP/A2A diagnostic pages.
- Run raw JSON export and delete preflight internals.

Finding: Current UI exposes diagnostic evidence in useful but technical terms.
Evidence: `ReasoningTracePanel`, `RunTracePanel`, `MainChatExecutionEvidence`, `AgentControlPlane`, `RuntimeDisclosureStrip`, Settings `AdvancedTab`.
File location: `frontend/src/components/`; `frontend/src/pages/settings/tabs/AdvancedTab.tsx`.
Confidence: High.
Impact: V2 should preserve evidence while reducing cognitive load in the default workspace.

## Human Decisions Needed

1. Should `Companion` and `Chat` merge into one `工作区`, or should `陪伴` remain an emotional/ambient mode?
2. Should `Runs` become top-level `任务`, or should it be a history tab inside `工作区`?
3. Which task controls belong in `工作区` versus `任务` detail?
4. Should skill selection be visible by default or moved behind an advanced "capability" picker?
5. What exact review categories should `审核中心` own: proposals only, or also permissions, external actions, memory rollback, and plan review?
6. What evidence is required in the default timeline before raw trace panels are hidden?
