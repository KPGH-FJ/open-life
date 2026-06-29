# OpenLife Repair Industry Benchmark Guardrails

Date: 2026-06-29

Status: product benchmark guardrail for repair planning. These references are product-quality comparators, not proof that OpenLife has implemented the same behavior.

## Why This Exists

The repair plan uses industry examples to raise product quality, but OpenLife cannot claim an industry-standard behavior until its own runtime evidence proves it. This file separates:

1. The product pattern worth borrowing.
2. The OpenLife behavior contract.
3. The anti-hallucination check that prevents a borrowed pattern from becoming a false implementation claim.

## Benchmarks

| Reference | Public source | Product pattern | OpenLife contract | Anti-hallucination check |
|---|---|---|---|---|
| ChatGPT Memory | https://help.openai.com/articles/8590148-memory-faq | Saved memory is visible and user-manageable. | LifeModel/Memory changes must be visible, editable/rejectable, and traceable to source/proposal/patch. | Do not claim a memory/LifeModel fact is committed until proposal, patch/snapshot/current view, and UI agree. |
| ChatGPT Projects | https://help.openai.com/en/articles/10169521-projects-in-chatgpt | Project context groups instructions/files/chats around a user goal. | OpenLife task context must be scoped and auditable, not accidental carryover from unrelated history. | Verify task/session/context refs rather than relying on assistant prose saying it used context. |
| Claude Artifacts | https://support.anthropic.com/en/articles/9487310-what-are-artifacts-and-how-do-i-use-them | Substantial generated output has a visible reusable surface separate from chat. | PlanExecute must return a plan body or artifact card with plan id, controls, and trace. | Do not count "created governed draft" as task success without visible user-facing body/artifact. |
| Notion AI | https://www.notion.com/help/notion-ai-faqs and https://www.notion.com/help/notion-ai-connectors | Workspace AI needs clear knowledge boundaries and source feel. | File/session/web/MCP answers must identify the source boundary or show a blocker. | Do not treat a cloud LLM answer as web/current-fact evidence without a tool/source record. |
| Granola sharing/privacy pattern | https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes | Sharing and visibility are explicit user-facing product controls. | OpenLife privacy UI must make local/private/external/provider transmission states visible. | Granola is only a product-pattern analogy here; it is not evidence for OpenLife provider telemetry or provider retention behavior. |
| Codex cloud / CLI | https://developers.openai.com/codex/cloud and https://developers.openai.com/codex/cli/features | Background agent work needs status, logs, resumability, approval/sandbox boundaries, and deliverable evidence. | Runs must show lifecycle, route, blocker/proposal/artifact refs, controls, and safe trace evidence. | Do not mark a run complete from UI status alone; cross-check AgentRun, task session, transcript, and final delivery. |
| Cursor cloud agents | https://cursor.com/docs/cloud-agent | Background work should expose task state, environment, reviewable changes, and recovery path. | Long OpenLife tasks need cancel/retry/resume and evidence, not indefinite running states. | Do not infer recoverability unless the UI and durable task state both expose the control. |

## Cross-Reference To Repair Phases

| Phase | Minimum industry-standard behavior to borrow | Must be proven by |
|---|---|---|
| Sprint 1 Trust Foundation | Route/readiness truth cannot be model self-report. | Runtime metadata, Settings diagnostics, Runs route, DB/trace. |
| Sprint 2 Runs / Trace / Recovery | Background task status and evidence are first-class product UI. | AgentRun + task session + transcript + Runs UI replay. |
| Sprint 3 LifeModel Closed Loop | Memory/user model changes are user-visible and reversible. | Proposal + patch/snapshot + current view + Review/LifeModel UI. |
| Sprint 4 Agent Task Productization | Substantial outputs become artifacts or explicit deliverables. | Plan body/card, plan id, controls, replay screenshots/tests. |
| Sprint 5 Privacy / Provider Governance | External transmission and dangerous actions have explicit boundaries. | ProviderTransmissionLogEntry or chosen store, preflight, no-key-leak tests. |
| Sprint 6 Daily UX / IA / AX | Daily product surfaces hide internal scaffolding and remain operable. | Page/component tests, AX evidence, responsive screenshots, copy replay. |

## Benchmark Use Rules

- Use industry references to define user expectations and acceptance bars.
- Do not cite an external product as evidence that OpenLife currently behaves correctly.
- Do not borrow provider/privacy claims unless there is a direct provider or OpenLife runtime source for the specific claim.
- When an industry pattern conflicts with local-first privacy, prefer explicit user control and local evidence over convenience.
- Re-check these references before making public-facing claims, because commercial product behavior can change.
