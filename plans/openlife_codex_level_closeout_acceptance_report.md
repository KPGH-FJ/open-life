# OpenLife Codex-Level Closeout Acceptance Report

Date: 2026-05-26

Status: accepted; LifeModel phase entry recommended.

Baseline branch: `dev`

Baseline commit: `03604c9 refactor: harden runtime fallback boundary`

## Current Stage

OpenLife is in Codex-level Final Closeout. P0-P12 vNext primitives are complete, and the Codex-level runtime stabilization boundary has been closed. This report is a fact-sync and acceptance gate for moving into LifeModel Evolution / Evidence / Proposal / Editor / Review.

This stage does not include LifeModel deep feature development, release packaging, code signing, installers, public trial logistics, AgentRuntime redesign, or removal of legacy compatibility code.

## Completed Capability Checklist

| Area | Closeout result |
|---|---|
| AgentRuntime execution convergence | Chat / StreamChat use Tauri `ExecutionFacade` and core `AgentRuntime::execute_task_with_spec`; Scheduled, Direct Tool, Replay, and Plan execution use mode-specific governed wrappers. |
| AgentSpec fail-closed governance | Missing or mismatched AgentSpec blocks governed execution; wrapper paths resolve required specs before mutation or model calls. |
| Proposal-first LifeModel / Memory updates | Builder, Calibration, Chat, MemoryWrite, MemoryArchive, ToolPermission, and external write flows remain proposal-first for high-risk writes. |
| Replay / Plan / Direct Tool wrappers | These are governed no-model action paths and do not emit fake `prompt_stack.assembled` traces. |
| Skill runtime PromptStack boundary | SkillManifest-derived PromptBlocks are appended to an effective AgentSpec and executed through governed generation; Skill remains outside Chat facade by design. |
| Chat Proposal Extraction PromptStack | LLM extraction uses Proposal-specific PromptBlocks and metadata-only audit, with explicit heuristic fallback. |
| Web Summarization PromptStack | Web summarization uses Web-specific PromptBlocks, sanitized source display, and metadata-only audit. |
| LayeredReasoner PromptStack | Meaning / strategy / generation / safety internal prompts use LayeredReasoner PromptBlocks and metadata-only `ReasoningTrace` block traces. |
| Legacy scheduler generation boundary | `InferenceScheduler::generate` / `generate_stream` and `llm::build_system_prompt` are legacy compatibility only; formal AgentRuntime / ExecutionFacade paths and runtime fallback do not call them. |
| Builder / Calibration boundary | Builder model-assisted extraction uses Builder PromptBlocks + `generate_raw_governed(..., LocalOnly)`; Calibration is deterministic / proposal-only / not applicable for PromptStack unless future model generation is added. |
| PromptBlock metadata in AgentRunEvent | Formal governed paths emit typed `prompt_stack.assembled` payloads with PromptBlock id/version/purpose/privacy/cloud/budget/applies_to/tokens only. |
| Runtime fallback boundary | Chat / StreamChat retain a governed legacy compatibility retry for Runtime/model failures only; Governance failures fail closed. |

## Key Governance Boundaries

- `governed`: formal model prompt execution through AgentSpec, PromptStack, PrivacyPolicy, ContextAssembler, ModelRouter, and typed AgentRunEvent/Audit boundaries.
- `legacy compatibility`: explicitly retained compatibility paths that are not formal new runtime contracts. They must remain documented, gated where applicable, and excluded from formal governed entrypoints.
- `not applicable`: no model prompt assembly exists, or emitting PromptStack events would create a fake contract.
- High-risk LifeModel, memory, tool permission, and external write changes must remain Proposal-first and user-confirmed.
- Helper-only PromptStacks must not fabricate `prompt_stack.assembled` AgentRunEvents.
- Runtime fallback must not contain raw prompt, raw user text, raw LifeModel, raw memory, or complete model output in payloads.

## PromptStack Coverage Result

Governed paths:

- Chat and StreamChat formal AgentLoop execution.
- Scheduled execution.
- Skill runtime.
- Chat proposal extraction helper.
- Web content summarization helper.
- LayeredReasoner internal prompts.
- Builder model-assisted extraction helpers.
- PlanMode planning helper.

Legacy compatibility:

- `InferenceScheduler::generate` / `generate_stream`.
- `llm::build_system_prompt`.
- Builder direct apply compatibility gate.
- Calibration direct apply compatibility gate.

Not applicable:

- Replay.
- Direct tool execution.
- Plan action execution.
- Calibration report / evolution / proposal creation and prompt metadata.
- Proactive suggestion generation.

## AgentRunEvent / Audit Result

The AgentRunEvent contract is append-only and typed for the runtime surfaces that need governance. The current event set includes 45 event types, including runtime fallback metadata events. Formal PromptStack events use `build_prompt_stack_assembled_payload`; context governance events use metadata-only payloads; replay and tool-block events use typed reason payloads; Builder and Calibration proposal creation events avoid raw prompt, before/after values, and full LifeModel payloads.

Frontend trace parsing treats governance events as structured contracts and falls back to diagnostic display for malformed known typed events. Raw payloads remain available only in debug views.

## Failure And Fallback Result

Runtime fallback decision: keep compatibility retry, but only as governed legacy compatibility.

Reason: it preserves current Chat / StreamChat UX recovery for Runtime/model failures while avoiding a new first-class fallback mode during final stabilization. The boundary is now explicit: Governance failures do not fallback, fallback calls governed generation with stored AgentSpec / PromptStack / PrivacyPolicy, and fallback event payloads are metadata-only.

Fallback event contract:

- `fallback.started`: metadata only, includes fallback mode, generation path, agent spec, privacy policy, and sanitized original error summary.
- `fallback.completed`: metadata only, includes response length and governed path metadata, not model output.
- `fallback.failed`: metadata only, includes sanitized failure summary.

## Remaining Non-Blocking Items

- `src-tauri/src/lib.rs` remains large and should be reduced as engineering debt, not as a LifeModel entry blocker.
- `ChatPage.tsx` remains large and should be decomposed after the LifeModel entry slice is defined.
- Universal binary, code signing, notarization, Windows/Linux validation, installers, and public trial logistics remain Post-Beta release work, not this phase.
- LifeModel Evolution is intentionally not implemented in this closeout; it is the next phase.
- SubAgent and Bash/Shell expansion should not be expanded inside the LifeModel entry slice unless separately gated.

## LifeModel Phase Entry Conditions

LifeModel-stage work can start when:

- `make ci` remains green.
- New LifeModel model entrypoints are classified before implementation as `governed`, `legacy compatibility`, or `not applicable`.
- Evolution proposals are evidence-backed and Proposal-first.
- High-risk fields such as identity, values, mission, and long-term goals cannot auto-apply.
- New prompt surfaces use PromptStack metadata-only trace or an explicit helper audit boundary.
- New writes preserve snapshot, audit, review, and rollback semantics.

## Test Gate Result

Closeout verification:

- `make ci`: passed on 2026-05-26 with frontend, core, tauri, a2a, clippy, formatting, and production build checks.
- Frontend tests: 431 passed.
- Core tests: 960 passed, 1 ignored.
- Tauri tests: 207 passed.
- A2A tests: 5 passed.
- Frontend production build: passed in 3.60s.
- `git diff --check`: passed.

No code changes are required by this report. The current closeout changes are documentation-only, so no new behavior tests are needed beyond the existing CI and trace/fallback contract tests.

## Next Stage Recommendation

Proceed to LifeModel Evolution / Evidence / Proposal / Editor / Review after final documentation diff checks pass. The first LifeModel slice should be narrow: evidence aggregation to reviewable proposals, with no direct high-risk LifeModel writes and no new bare model path.
