# Main Chat Agent Beta v1 Benchmark Lessons

> Date: 2026-06-18
> Status: source-backed preparation notes
> Scope: lessons from public first-class agent documentation, adapted to OpenLife

## 1. Source Confidence

Use this document as a product reference, not as a claim about private
implementation details.

When a benchmark claim becomes an implementation requirement, re-check the
linked public source in the same review cycle or downgrade it to product
intuition. Do not encode unverified competitor internals as OpenLife acceptance
criteria.

High-confidence sources:

- OpenAI Codex manual fetched on 2026-06-18:
  `https://developers.openai.com/codex/codex-manual.md`
- Codex best practices:
  `https://developers.openai.com/codex/learn/best-practices`
- Codex `AGENTS.md` guide:
  `https://developers.openai.com/codex/guides/agents-md`
- Codex skills:
  `https://developers.openai.com/codex/skills`
- Codex memories:
  `https://developers.openai.com/codex/memories`
- Claude Code memory:
  `https://code.claude.com/docs/en/memory`
- Claude skill authoring best practices:
  `https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices`
- Anthropic tool-writing guidance:
  `https://www.anthropic.com/engineering/writing-tools-for-agents`
- Claude memory tool:
  `https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool`
- Hermes architecture:
  `https://hermes-agent.nousresearch.com/docs/developer-guide/architecture`
- Hermes agent loop:
  `https://hermes-agent.nousresearch.com/docs/developer-guide/agent-loop`
- Hermes prompt assembly:
  `https://hermes-agent.nousresearch.com/docs/developer-guide/prompt-assembly`
- OpenClaw tools:
  `https://docs.openclaw.ai/tools`
- OpenClaw managed browser:
  `https://docs.openclaw.ai/tools/browser`

Lower-confidence sources:

- blog posts, videos, Reddit threads, and media summaries about Hermes,
  OpenClaw, or Claude internals. They may be useful for product intuition, but
  they must not be used as final technical authority.

## 2. Benchmark Pattern: Default Work Means Execution

Public Hermes and OpenClaw positioning emphasizes agents that do work, not only
answer questions. Hermes' public agent-loop documentation describes a loop that
assembles prompt/tool state, calls a provider, executes tool calls, appends tool
results, persists session state, and resumes later. OpenClaw's tools docs define
tools as typed functions for acting, with visible tools constrained by profile,
policy, provider, sandbox, channel, and plugin availability.

OpenLife implication:

- Work-like Main Chat requests should create visible task frames by default.
- DirectAnswer remains valid, but only for tasks classified as no-action.
- Tool-required prompts must not quietly become a plain answer.
- UI must show the action/observation/proposal/blocker trail.

Gap today:

- OpenLife has strong governed runtime objects, but users can still experience
  the product as a chat surface unless the default Main Chat path consistently
  renders execution states.

## 3. Benchmark Pattern: Durable Instructions Are Files

Codex uses layered `AGENTS.md` instructions across global, project, and nested
directory scopes. Claude Code documents `CLAUDE.md`-style project memory and
auto memory files that can be inspected. Hermes prompt assembly publicly
describes `SOUL.md`, `MEMORY.md`, `USER.md`, skills, and project context files
as distinct prompt layers.

OpenLife implication:

- Knowledge files should be product assets, not invisible prompt fragments.
- File scope, precedence, loaded/not-loaded state, digest, size limit, and last
  accepted update must be inspectable.
- `USER.md`, `MEMORY.md`, `SOUL.md`, `AGENTS.md`, and `SKILL.md` must have
  different product meanings.

Gap today:

- OpenLife can load bounded knowledge-format context, but the user-facing
  knowledge asset manager and lifecycle explanation are not yet strong enough.

## 4. Benchmark Pattern: Skills Teach; Tools Act

Codex and Claude skills use progressive disclosure: metadata is available
upfront, and full skill instructions are loaded only when selected or relevant.
OpenClaw distinguishes tools, skills, and plugins: tools act, skills teach
workflows, plugins add installable capabilities.

OpenLife implication:

- A selected `SKILL.md` should provide workflow guidance, not permission to
  bypass policy.
- Tool selection must remain governed by ExecutionPolicy and allowlists.
- Skills UI should show why a skill was selected and what tool dependencies it
  expects.

Gap today:

- OpenLife has selected-skill plumbing and skills/tool surface foundations, but
  not enough user-visible skill reasoning, dependency disclosure, or workflow
  verification.

## 5. Benchmark Pattern: Memory Is Bounded, Editable, And Evidence-Aware

Codex memories are optional local generated state and explicitly not the only
place for required team rules. Claude Code and Claude's memory tool document
file-based memory that can be inspected and updated through controlled memory
surfaces. Hermes docs describe persistent memory/profile snapshots in prompt
assembly and session persistence.

OpenLife implication:

- OpenLife should not dump raw conversation into long-term prompt context.
- Memory updates need evidence, confidence, confirmation, rollback, and active
  materialized view behavior.
- "Remember this" is a product flow: candidate -> evidence -> confirmation ->
  active memory -> rollback.

Gap today:

- If Product Maturity v2 rollback foundations are verified, Beta v1 must make
  them understandable and usable in the default Main Chat flow. If they are only
  partial, the missing rollback product slice remains a Beta blocker or required
  prerequisite.

## 6. Benchmark Pattern: Context Assembly Has Layers

Hermes separates stable identity, tool/model guidance, project context files,
volatile memory/user profile, skills, and ephemeral turn additions. Codex
separates prompt/task context, `AGENTS.md`, skills, plugins, MCP, config, hooks,
and memories.

OpenLife implication:

- OpenLife should publish and test a context assembly inventory for every task:
  which knowledge files were eligible, selected, loaded, skipped, truncated, or
  blocked.
- Runtime policy must outrank file-based instructions and memories.
- The user should be able to inspect context sources without reading raw prompt
  dumps.

Gap today:

- Bounded context loading exists, but product-level trace and source inspection
  need to be finished.

## 7. Benchmark Pattern: Good Tools Are Evaluated On Real Tasks

Anthropic's tool-writing guidance recommends prototyping tools, testing locally,
then running comprehensive evaluations grounded in realistic data and tasks.
The same article warns against overly superficial sandbox tasks that do not
stress tool use.

OpenLife implication:

- Beta v1 needs a realistic task vertical harness, not only unit or synthetic
  coverage.
- Scenarios must require multi-step action, failure recovery, permission, memory
  proposal, and final delivery.
- Eval output must explain exactly which user-visible product state failed.

Gap today:

- OpenLife has strong deterministic gates, but still needs product-realistic
  end-to-end scenarios that match how the user will judge the agent.

## 8. Benchmark Pattern: Long Work Requires Persistence And Recovery

Hermes public docs describe session persistence, resume, context compression,
and tool/result pairing. Codex best practices emphasize verifying work and
using reusable guidance over time. OpenClaw includes automation/background work
as a distinct category.

OpenLife implication:

- Existing task continuity should become a clear product home: task list,
  detail, last observation, next action, stale state, blockers, resume controls.
- Recovery must preserve permission scope and context digest.
- A terminal task must not be restartable as if it were paused.

Gap today:

- If Product Maturity v2 continuity objects are verified, Beta v1 must integrate
  them into normal user navigation and final delivery. If they are partial, the
  foundation inventory must identify the missing runtime, UI, or eval proof.

## 9. Benchmark Pattern: Approval Should Be Exact

Hermes docs describe dangerous command approval before execution. OpenClaw tools
are filtered by profile, policy, provider, sandbox, channel, and plugin
availability. Codex uses sandbox/approval modes and encourages tight defaults.

OpenLife implication:

- Permission requests should include action, tool, target, scope, risk, duration,
  and exact consequence.
- Approval resumes the exact pending action, not a broad class of future
  actions.
- Risky or write-like operations must remain proposal-first or blocked.

Gap today:

- OpenLife has strong policy mechanics. The product gap is making permission
  understandable without burying users in governance detail.

## 10. Product Translation For OpenLife

OpenLife should not copy Hermes/OpenClaw/Codex names. It should adopt the
following behaviors:

- execution-first default for work-like Main Chat requests;
- inspectable task timeline with action, observation, blocker, proposal, and
  final delivery states;
- bounded file-based knowledge surfaces with scope, digest, and lifecycle;
- progressive skill loading and tool policy separation;
- realistic task evals before broad claims;
- exact permission and recovery states;
- final readiness that distinguishes deterministic proof from opt-in external
  live proof.
