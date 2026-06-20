# Main Chat Stage 5 Release And Debug Best Practices

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation research notes

## 1. Sources Reviewed

Primary/source-adjacent references used for this stage:

- OpenAI Agents SDK tracing:
  `https://openai.github.io/openai-agents-python/tracing/`
- Claude Code memory documentation:
  `https://code.claude.com/docs/en/memory`
- Anthropic advanced tool use:
  `https://www.anthropic.com/engineering/advanced-tool-use`
- Anthropic Claude Code auto mode / permission fatigue:
  `https://www.anthropic.com/engineering/claude-code-auto-mode`
- LangSmith observability documentation:
  `https://docs.langchain.com/langsmith/observability`
- Google Cloud ADK OpenTelemetry instrumentation:
  `https://docs.cloud.google.com/stackdriver/docs/instrumentation/ai-agent-adk`
- OpenTelemetry GenAI semantic conventions:
  `https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/`

The point is not to copy these systems. The point is to learn the operational
shape of first-class Agent products: trace every run, keep memory/instructions
as context rather than policy authority, discover/load tools on demand, classify
permissions, export metadata-safe evidence, and turn traces into evals.

## 2. Distilled Practices

### 2.1 Trace first, eval second

OpenAI Agents tracing treats an Agent run as a sequence of LLM generations,
tool calls, handoffs, guardrails, and custom events. LangSmith similarly
positions observability as full visibility from individual traces to production
metrics. Stage 5 should therefore make debug traces exportable before trying to
expand manual dogfood.

OpenLife implication:

- every debug bundle must start from `task_session_id`, `run_id`, transcript
  entries, actions, observations, blockers, proposals, and final delivery;
- eval rows must reference concrete trace/bundle ids, not free-form notes;
- a scenario cannot pass just because the chat answer looked good.

### 2.2 Context is not policy

Claude Code documentation distinguishes memory/instruction files from enforced
configuration. Project memory can guide the agent, but blocking actions belongs
to hooks/permissions. Stage 4 already adopted this principle for `USER.md`,
`MEMORY.md`, `SOUL.md`, `AGENTS.md`, and `SKILL.md`.

OpenLife implication:

- debug bundles may report loaded context assets, but they must also report
  ExecutionPolicy/privacy/tool/model route decisions separately;
- a loaded knowledge file cannot be shown as the reason a risky action was
  allowed;
- issue reports must preserve the difference between "context influenced the
  answer" and "policy allowed execution".

### 2.3 Tool libraries should be loaded on demand

Anthropic's advanced tool-use guidance argues against stuffing every tool
definition into context. Tools should be discovered and loaded only when
relevant. OpenLife has already built bounded MCP candidate selection and
selected `SKILL.md` loading.

OpenLife implication:

- debug bundles should report candidate counts, selected tool, allowlist,
  policy decision, and skipped/loaded tool context;
- export should not dump entire tool manifests or all skills;
- tool-selection failures should distinguish no candidate, disallowed candidate,
  provider ranking ignored, policy block, and execution failure.

### 2.4 Permission friction needs classification, not bypass

Anthropic's Claude Code auto-mode discussion identifies permission fatigue as a
real product problem, while warning that bypassing prompts is unsafe. Stage 5
should make permission failures explainable and recoverable without weakening
policy.

OpenLife implication:

- debug UI should label whether a blocker is privacy, policy, permission,
  missing credential, network, high-risk write, or unsupported action;
- recovery suggestions should be specific: retry, refresh context, approve
  proposal, provide key, enable network, or stop;
- no Stage 5 code should downgrade confirmation requirements.

### 2.5 OpenTelemetry vocabulary is useful, but content capture is risky

Google ADK instrumentation and OpenTelemetry GenAI conventions show the value of
standard attributes: agent id/version, conversation id, provider/model, prompt,
tool call id/name/result, retrieval documents, token usage, and evaluation
labels. They also make clear that prompts and responses can be collected, which
is sensitive.

OpenLife implication:

- use stable field names aligned with common observability vocabulary where
  possible: agent, workflow, conversation/session, provider, model, tool call,
  retrieval/context source, evaluation result;
- default export should use ids, digests, bounded previews, and low-cardinality
  labels rather than raw prompts/responses;
- raw content export should require explicit user/tester action and remain out
  of readiness evidence by default.

### 2.6 Internal dogfood needs reproducible artifacts

First-class Agent testing ties human feedback to traceable runs. Manual notes
without build/version/session ids are not actionable. Stage 2 already enforces
known commit and reviewer evidence; Stage 5 should make producing those
artifacts easy.

OpenLife implication:

- task-attached issue reports must include build commit, branch, app version,
  scenario id, task session id, run id, bundle id, reviewer id, timestamp, and
  status; preflight-only or environment-blocked reports may omit task/run ids
  only with named blockers and missing-id reasons;
- stale or unknown build evidence must remain invalid;
- local debug artifacts should be schema-versioned, atomically written, and
  reloadable without depending on the git workspace;
- Stage 5 can prepare artifact creation, but cannot fabricate manual dogfood.

## 3. Stage 5 Design Principles

1. Debug bundles are product objects, not log dumps.
2. Every exported claim must map to a runtime object or explicit blocker.
3. Redaction is mandatory by default.
4. Recovery recommendations must come from failure taxonomy, not model prose.
5. Provider/live evidence remains separate from deterministic local evidence.
6. Build provenance is part of the artifact.
7. The UI should help internal testers produce useful reports quickly.
8. Stage 5 does not change the meaning of Stage 2 readiness.
9. Local artifact lifecycle is part of the product contract, not an incidental
   file write.

## 4. What OpenLife Should Not Copy Blindly

- Do not add a cloud telemetry dependency for Stage 5. OpenLife is local-first;
  export a local metadata-safe bundle first.
- Do not export full prompts/responses by default even if external observability
  tools support it.
- Do not use an auto-permission classifier in Stage 5. The current product
  problem is debuggability, not reducing approval prompts.
- Do not over-standardize on OpenTelemetry before the local debug bundle is
  useful. A future adapter can map the local bundle to OTel-like attributes.
