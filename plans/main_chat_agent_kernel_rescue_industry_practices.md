# Main Chat Kernel Rescue Industry Practices

> Date: 2026-06-22
> Status: source-backed practice digest for the eight Main Chat kernel rescue goals
> Parent: `plans/main_chat_agent_kernel_rescue_goal_mode_index.md`

## 1. Sources Reviewed

This digest uses current primary or near-primary sources:

- OpenAI Agents SDK overview, results/state, tracing, guardrails, and agent eval
  documentation.
- Anthropic Engineering, "Building Effective Agents."
- Model Context Protocol specification. The current public specification family
  includes a 2025-11-25 version; older 2025-06-18 material remains useful only
  for compatibility notes and must not be treated as the latest security model.
- LangGraph/LangChain persistence and human-in-the-loop documentation.
- NSA Cybersecurity Information report, "Model Context Protocol: Security
  Design Considerations for AI-Driven Automation", May 2026.

## 2. Practices Applied To OpenLife

### Start With A Small Working Agent

Anthropic warns that frameworks can hide prompts/responses and tempt teams into
unnecessary complexity; their recommendation is to start simple and understand
the underlying code. OpenAI's Agents SDK overview similarly frames the first
step as getting one working run before adding advanced capabilities.

OpenLife application:

- Goal 1 builds direct-answer-only `MainChatKernel`.
- Tool execution, HS reintegration, MCP, live provider, and final gate work come
  later.

### Treat The Result Surface As A Product Contract

OpenAI distinguishes final output, replay-ready history, pending approvals, and
resumable state. It also notes that interrupted approval flows return state
rather than a final answer.

OpenLife application:

- `MainChatTurnResult` must distinguish final answer, blockers, proposals,
  pending permissions, direct-write flags, and legacy-fallback flags.
- Goal 4 must not present proposal/permission interruptions as completed work.

### Trace First, Then Formalize Evals

OpenAI describes traces as end-to-end records of model calls, tool calls,
handoffs, guardrails, and custom events, and recommends trace grading while
debugging behavior before moving to repeatable datasets and eval runs.

OpenLife application:

- The first goals should add high-signal kernel events and tests.
- Large final/live readiness gates should be realigned only after the kernel is
  stable.

### Put Guardrails Around Tool Calls, Not Only Around Prompts

OpenAI guardrail docs separate input, output, and tool guardrails, and note that
blocking guardrails can stop execution before token/tool cost or side effects.
LangChain's human-in-the-loop docs describe pausing risky tool calls, saving
state, and allowing approve/edit/reject/respond decisions.

OpenLife application:

- Goal 3 read-only tools need governed candidate input.
- Goal 4 write-like actions must become proposals, permission interruptions, or
  hard blockers.
- Permission acceptance must replay the exact pending action, not a newly
  interpreted request.

### Separate Short-Term Run State From Long-Term Memory

LangGraph separates checkpointers for thread-scoped state from stores for
long-term user facts/preferences. This maps closely to OpenLife's needed split
between task/session state and durable LifeModel/Memory truth.

OpenLife application:

- Kernel turn state and UI events are not accepted user truth.
- Memory and LifeModel writes require proposals and acceptance.
- HS summaries can inform a turn without mutating durable state.

### Design Tools For Agent-Computer Interaction

Anthropic emphasizes that tool definitions deserve as much design attention as
prompts, including examples, boundaries, clear parameters, and mistake-resistant
formats.

OpenLife application:

- Goal 3 must keep the first tool set small and obvious.
- File tools should use explicit workspace/safe-path resolution.
- Model-supplied arguments must not bypass governed executor input.

### Treat MCP As A High-Risk Integration Layer

The MCP specification says MCP can expose data and tools, and its authorization
and tools surfaces require careful user consent, authorization, and
implementation discipline. The NSA report says MCP adoption has outpaced
security model maturity and calls out risks such as weak access control, tool
invocation path confusion, context leakage, and prompt/tool poisoning.

OpenLife application:

- Goal 7 must restore MCP after deterministic read-only kernel tools work.
- MCP write-like tools stay disabled unless proposal/permission boundaries are
  explicit.
- MCP tool identity must be strict and source-scoped; name matching alone is
  insufficient.

## 3. Source Links

- OpenAI Agents SDK overview:
  https://developers.openai.com/api/docs/guides/agents
- OpenAI Agents SDK results and state:
  https://developers.openai.com/api/docs/guides/agents/results
- OpenAI Agents SDK tracing:
  https://openai.github.io/openai-agents-python/tracing/
- OpenAI Agents SDK guardrails:
  https://openai.github.io/openai-agents-python/guardrails/
- OpenAI agent evals:
  https://developers.openai.com/api/docs/guides/agent-evals
- Anthropic Engineering, Building Effective Agents:
  https://www.anthropic.com/engineering/building-effective-agents
- MCP specification:
  https://modelcontextprotocol.io/specification/2025-11-25
- LangChain human-in-the-loop:
  https://docs.langchain.com/oss/python/langchain/human-in-the-loop
- LangGraph persistence:
  https://docs.langchain.com/oss/python/langgraph/persistence
- NSA MCP security design considerations:
  https://www.nsa.gov/Portals/75/documents/Cybersecurity/CSI_MCP_SECURITY.pdf
