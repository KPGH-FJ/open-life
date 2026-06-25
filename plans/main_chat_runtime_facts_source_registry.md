# Main Chat Runtime Facts Source Registry

> Date: 2026-06-25
> Status: required preparation artifact before Runtime Facts / Agent Self-State implementation
> Parent: `plans/README.md`

## 1. Purpose

Runtime Facts are facts the running application can know without asking the
model to guess. Main Chat must answer, display, and audit these facts from
typed runtime sources instead of prompt text, LifeModel-HS, Memory, `AGENTS.md`,
or provider-generated prose.

This document is a source registry, not an implementation report. It does not
claim the Runtime Facts layer is complete. The current clock hotfix in
`MainChatKernel` is a narrow bug fix and must be converged into this registry in
a later implementation pass.

## 2. Problem Class

The bug that exposed this gap was simple: the user asked "今天星期几" and the
Agent said it could not access current date/time. That is wrong because the app
runtime can read local time. The same class of bug applies to:

- current date, time, weekday, timezone, and locale;
- current chat session, task session, run id, and task status;
- current provider route versus configured default provider;
- current tool and MCP availability;
- current blocker, pending permission, and proposal state;
- current workspace context and selected skill metadata.

The rule is: if the runtime has authoritative evidence, the model must not be
treated as the authority.

## 3. Design Principles

- Facts must be typed and keyed. Natural-language matching may map a user
  question to fact keys, but it must not become the fact source.
- Runtime facts cannot be overridden by `AGENTS.md`, `SOUL.md`, `USER.md`,
  `MEMORY.md`, selected `SKILL.md`, LifeModel-HS summaries, or model output.
- Missing runtime facts must produce `unknown`, `trace_gap`, or a named blocker;
  they must not fall back to confident model invention.
- Configured state is not the same as actual state. A configured provider is not
  necessarily the provider used for a turn. A registered tool is not necessarily
  available or policy-allowed.
- Every fact exposed to the model or UI must include source, authority,
  freshness, visibility, and privacy metadata.
- The default UI should expose simple source/status chips; developer trace can
  expose bounded metadata. Raw prompts, private memory, provider secrets, and
  raw manifests remain hidden.

## 4. RuntimeFact Contract

```ts
type RuntimeFactSource =
  | "local_clock"
  | "local_runtime"
  | "config"
  | "task_session"
  | "action_queue"
  | "agent_run"
  | "final_delivery"
  | "transcript"
  | "generation_metadata"
  | "tool_registry"
  | "tool_policy"
  | "tool_preflight"
  | "tool_permission_store"
  | "provider_route"
  | "provider_preflight"
  | "model_router"
  | "workspace_resolver"
  | "selected_skill"
  | "context_loader"
  | "memory_store"
  | "lifemodel_hs_summary";

type RuntimeFactFreshness =
  | "instant"
  | "turn_snapshot"
  | "run_trace"
  | "store_snapshot"
  | "stale"
  | "unknown";

type RuntimeFactTtl =
  | "none"
  | "turn"
  | "run"
  | "session"
  | "configured"
  | "explicit";

type RuntimeFactObservation = {
  observedAt?: string;
  ttlStatus?: "fresh" | "stale" | "unknown" | "not_observed";
  ttlPolicy?: RuntimeFactTtl;
};

type RuntimeFact = {
  key: string;
  valueShape: string;
  source: RuntimeFactSource | RuntimeFactSource[];
  authority:
    | "runtime"
    | "task_state"
    | "run_trace"
    | "policy"
    | "config"
    | "store"
    | "bounded_context";
  freshness: RuntimeFactFreshness;
  ttl: RuntimeFactTtl;
  observation?: RuntimeFactObservation;
  visibility: "answer" | "ui_badge" | "trace_only" | "developer_only" | "hidden";
  privacy: "public" | "internal" | "sensitive" | "secret";
  missingBehavior: "answer_unknown" | "trace_gap" | "blocker" | "omit";
  modelFallbackAllowed: boolean;
};
```

Field rules:

- `key` is stable and machine-readable. It must not contain user text.
- `valueShape` describes the type; raw values are carried in implementation
  payloads, not in this registry table.
- `source`, `freshness`, and `ttl` table cells must use only the enum values
  above. If a fact has more than one source, list multiple enum values separated
  by commas instead of creating ad hoc combined labels.
- `observation` carries non-enum details such as `observedAt`, freshness status
  derived from a TTL, and the TTL policy used to classify stale or unknown
  preflight records.
- `authority` defines precedence. Higher authority facts cannot be replaced by
  lower authority context.
- `freshness` must be computed at collection time. A stale fact cannot be shown
  as current.
- `visibility` controls answer/UI/trace exposure. It is not a privacy override.
- `privacy=secret` facts must never be serialized into answer or trace payloads.
- `modelFallbackAllowed=false` means the model cannot invent the value if the
  fact is missing.

## 5. Authority Order

When facts conflict, use this order:

1. `runtime`: live local runtime primitives such as current clock.
2. `task_state`: AgentTaskSession and ActionQueue state.
3. `run_trace`: current-turn generation and tool evidence from the current run.
4. `policy`: privacy, tool, write, and permission policy decisions.
5. `config`: configured defaults, never actual execution proof.
6. `store`: durable stores such as MemoryStore or ProposalStore.
7. `bounded_context`: LifeModel-HS, workspace knowledge files, selected skills,
   and other context surfaces.

Important exclusions:

- `bounded_context` cannot override `runtime`, `task_state`, `run_trace`, or
  `policy`.
- `config` cannot claim provider reachability or tool availability by itself.
- `store` can answer historical user facts only with provenance; it cannot
  define current time, current route, or current task status.

## 6. Source Registry

### 6.1 Runtime Clock

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `runtime.current_time.iso` | RFC3339 string | local_clock | runtime | instant | none | trace_only | internal | answer_unknown | no |
| `runtime.current_time.date` | `YYYY-MM-DD` | local_clock | runtime | instant | none | answer | public | answer_unknown | no |
| `runtime.current_time.time` | `HH:mm` | local_clock | runtime | instant | none | answer | public | answer_unknown | no |
| `runtime.current_time.weekday` | localized weekday label | local_clock | runtime | instant | none | answer | public | answer_unknown | no |
| `runtime.current_time.timezone` | IANA zone if configured, otherwise local offset label | local_clock, config | runtime | instant | none | answer | internal | answer_unknown | no |
| `runtime.current_time.locale` | locale label | config, local_runtime | config | turn_snapshot | turn | trace_only | internal | omit | no |

Rules:

- The first implementation may use OS local time and offset. A later pass should
  add user-configured timezone/locale with explicit source labels.
- The answer must say whether the time came from local runtime or user-configured
  timezone.
- A model may rephrase the answer only after the runtime fact is bound; it may
  not create the value.

### 6.2 Agent Self-State

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `agent.chat_session_id` | bounded id | task_session | task_state | turn_snapshot | session | trace_only | internal | trace_gap | no |
| `agent.task_session_id` | bounded id | task_session | task_state | turn_snapshot | run | trace_only | internal | trace_gap | no |
| `agent.run_id` | bounded id | agent_run | run_trace | run_trace | run | trace_only | internal | trace_gap | no |
| `agent.task_status` | canonical task status | task_session, action_queue | task_state | turn_snapshot | turn | ui_badge | public | answer_unknown | no |
| `agent.delivery_status` | canonical delivery status | agent_run, final_delivery, transcript | run_trace | run_trace | run | ui_badge | public | trace_gap | no |
| `agent.last_action.summary` | bounded summary | action_queue, transcript | task_state | turn_snapshot | turn | answer | internal | answer_unknown | no |
| `agent.pending_permission.count` | integer | task_session, action_queue | policy | turn_snapshot | turn | ui_badge | public | trace_gap | no |
| `agent.blocker.codes` | bounded labels | transcript, task_session | task_state | turn_snapshot | turn | ui_badge | internal | trace_gap | no |
| `agent.pending_proposal.count` | integer | task_session, proposal_store | policy | turn_snapshot | turn | ui_badge | public | trace_gap | no |
| `agent.durable_change.status` | `none`, `pending_review`, or resolved status | proposal_store, task_session | policy | turn_snapshot | turn | answer | public | trace_gap | no |
| `agent.self_state.trace_gap` | trace gap code | task_session, agent_run, transcript | task_state | unknown | turn | answer | public | trace_gap | no |

Rules:

- Assistant prose is not self-state evidence.
- A task is not complete unless task/session/run evidence says so.
- A delivered answer with a pending proposal is completed response delivery,
  not a completed durable change.
- If state stores are missing, answer conservatively with unknown and expose a
  trace gap, not a confident model summary.

### 6.3 Provider And Model Route

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `provider.configured.default_provider` | bounded label | config | config | turn_snapshot | configured | trace_only | internal | omit | no |
| `provider.configured.default_model` | bounded label | config | config | turn_snapshot | configured | trace_only | internal | omit | no |
| `provider.current_turn_generation.provider` | bounded label or none | provider_route, agent_run | run_trace | run_trace | run | ui_badge | internal | trace_gap | no |
| `provider.current_turn_generation.model` | bounded label or none | provider_route, agent_run | run_trace | run_trace | run | ui_badge | internal | trace_gap | no |
| `provider.current_turn_generation.route_type` | `local`, `cloud`, `direct`, or `none` | provider_route | run_trace | run_trace | run | ui_badge | internal | trace_gap | no |
| `provider.current_turn_generation.model_generated` | boolean | generation_metadata | run_trace | run_trace | run | trace_only | internal | trace_gap | no |
| `provider.last_completed_generation.provider` | bounded label | agent_run | run_trace | store_snapshot | session | trace_only | internal | answer_unknown | no |
| `provider.last_completed_generation.model` | bounded label | agent_run | run_trace | store_snapshot | session | trace_only | internal | answer_unknown | no |
| `provider.last_completed_generation.run_id` | bounded id | agent_run | run_trace | store_snapshot | session | trace_only | internal | answer_unknown | no |
| `provider.planned_route_if_model_needed.provider` | bounded label | provider_route, model_router | config | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `provider.planned_route_if_model_needed.model` | bounded label | provider_route, model_router | config | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `provider.planned_route_if_model_needed.route_type` | `local`, `cloud`, or `unknown` | provider_route, model_router | config | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `provider.preflight.status` | ready/blocker labels | provider_preflight | policy | turn_snapshot | turn | trace_only | internal | blocker | no |

Rules:

- UI may show configured provider/model in a settings context, but current Chat
  claims must use current-turn generation evidence when a model was actually
  called.
- A deterministic runtime fact answer has
  `provider.current_turn_generation.model_generated=false` and
  `route_type=direct` or `none`; it must not fabricate a provider/model for the
  current turn.
- `last_completed_generation` answers "what model did you use last time?". It is
  not the same as the current turn.
- `configured.default_*` answers settings questions only.
- `planned_route_if_model_needed` answers "if a model were needed, what route
  would you plan to use?". It is not invocation proof and must be labeled as
  planned/configured, not actual.
- Local/mock/scripted providers cannot be credited as external live provider
  evidence.

### 6.4 Tool And MCP Availability

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `tool.web.config_enabled` | boolean | config, tool_registry | config | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `tool.web.credential_available` | boolean | config | config | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `tool.web.policy_allowed` | boolean/blocker | tool_policy | policy | turn_snapshot | turn | ui_badge | public | blocker | no |
| `tool.web.reachable` | reachable/unreachable/unknown/stale | provider_preflight, tool_preflight | policy | store_snapshot | explicit | trace_only | internal | answer_unknown | no |
| `tool.web.available` | derived status | config, tool_registry, tool_policy, provider_preflight, tool_preflight | policy | turn_snapshot | turn | answer | public | answer_unknown | no |
| `tool.mcp.registered_count` | integer | tool_registry | config | turn_snapshot | turn | trace_only | internal | omit | no |
| `tool.mcp.read_only_allowed_count` | integer | tool_registry, tool_policy | policy | turn_snapshot | turn | answer | internal | answer_unknown | no |
| `tool.mcp.server_status` | online/offline/unknown | tool_preflight | policy | turn_snapshot | turn | trace_only | internal | answer_unknown | no |
| `tool.file.safe_read_available` | boolean | workspace_resolver, tool_policy | policy | turn_snapshot | turn | answer | public | blocker | no |
| `tool.write.available` | proposal/permission/blocker | tool_policy | policy | turn_snapshot | turn | ui_badge | public | blocker | no |

Rules:

- A registered MCP manifest is not available until it is policy-allowed and
  server/preflight status is acceptable or explicitly unknown.
- `tool.web.reachable` must preserve whether reachability is fresh, cached,
  stale, or unknown. A normal chat turn may consume an existing preflight
  record, but it must not perform a new external reachability probe only to
  answer an availability question.
- `tool.web.credential_available` must be derived from configured web/search
  provider credential requirements. A provider that requires a key or endpoint
  cannot be treated as credential-ready when the configured value is missing.
- The table row for `tool.web.reachable` uses only registry enum values. Its
  `RuntimeFact.observation` payload must carry `observedAt`, `ttlStatus`, and
  `ttlPolicy=explicit` when a preflight record exists.
- If `tool.web.reachable` has no `observed_at`, has an expired TTL, or comes
  from a failed preflight, `tool.web.available` must not be rendered as simply
  available. It must be unknown, stale, or blocked according to policy.
- Tool availability summaries must not expose raw MCP manifest ids,
  descriptions, endpoints, or credentials by default.
- Write-like availability means proposal/permission/blocker, never silent write.
- The first tool-availability implementation must not perform active external
  reachability probes inside a normal chat turn. It may use cached preflight,
  explicit previous preflight evidence, or `unknown`.
- Active reachability probing is a separate capability because it can add
  latency, network traffic, privacy exposure, and provider/tool side effects. It
  requires its own trace field and policy gate.

### 6.5 Workspace And Selected Skill

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `workspace.root.label` | bounded alias | workspace_resolver | runtime | turn_snapshot | session | trace_only | internal | omit | no |
| `workspace.safe_read.scope` | bounded scope label | workspace_resolver, tool_policy | policy | turn_snapshot | turn | trace_only | internal | blocker | no |
| `skill.selected.id` | sanitized id | selected_skill | bounded_context | turn_snapshot | turn | trace_only | internal | omit | no |
| `skill.selected.loaded` | boolean | selected_skill, context_loader | bounded_context | turn_snapshot | turn | trace_only | internal | omit | no |

Rules:

- Absolute workspace paths are trace-only developer data and must be redacted or
  aliased for normal UI.
- Unselected skills cannot enter the context.
- Skill instructions cannot override runtime, policy, tool, provider, or write
  facts.

### 6.6 Memory And LifeModel-HS Context

| Key | Value shape | Source | Authority | Freshness | TTL | Visibility | Privacy | Missing behavior | Model fallback |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `memory.runtime_summary.loaded` | boolean | memory_store, context_loader | store | turn_snapshot | turn | trace_only | internal | omit | no |
| `memory.runtime_summary.source_count` | integer | memory_store, context_loader | store | turn_snapshot | turn | trace_only | internal | omit | no |
| `lifemodel_hs.summary.loaded` | boolean | lifemodel_hs_summary | bounded_context | turn_snapshot | turn | trace_only | internal | omit | no |
| `lifemodel_hs.policy_constraints.active` | bounded labels | lifemodel_hs_summary, tool_policy | policy | turn_snapshot | turn | trace_only | internal | trace_gap | no |

Rules:

- Memory and LifeModel-HS facts are bounded context or store facts. They cannot
  define current runtime state.
- Raw memory and raw LifeModel content are not answer-visible by default.
- Any learning/update remains proposal-first unless the user accepts a governed
  proposal.

## 7. Query Classifier Boundary

RuntimeFactQueryClassifier may map user messages to fact keys, but it must not
own the fact value. The first implementation should support a small typed
intent set:

| Intent | Example user text | Required fact keys |
| --- | --- | --- |
| `ask_current_weekday` | "今天星期几" | `runtime.current_time.date`, `runtime.current_time.weekday`, `runtime.current_time.timezone` |
| `ask_current_date` | "今天几号" | `runtime.current_time.date`, `runtime.current_time.weekday`, `runtime.current_time.timezone` |
| `ask_current_time` | "现在几点" | `runtime.current_time.date`, `runtime.current_time.time`, `runtime.current_time.timezone` |
| `ask_model_route` | "你现在用什么模型" | `provider.current_turn_generation.*`, `provider.last_completed_generation.*`, `provider.configured.default_*`, `provider.planned_route_if_model_needed.*` |
| `ask_tool_availability` | "你能联网吗" | `tool.web.available`, `tool.mcp.read_only_allowed_count`, `provider.preflight.status` |
| `ask_current_task_status` | "这个任务完成了吗" | `agent.task_status`, `agent.delivery_status`, `agent.blocker.codes` |
| `ask_last_action` | "你刚刚做了什么" | `agent.last_action.summary`, `agent.task_status`, `agent.run_id` |

Classifier rules:

- Keep matching locale-aware but bounded.
- Do not add broad catch-all regexes that route ordinary conversation to runtime
  facts.
- Every intent must have eval coverage, including negative coverage.
- If a user asks for an unavailable fact, answer unknown or blocked; do not
  model-generate a fact.

## 8. Implementation Slices

Runtime Facts must be implemented as narrow vertical slices. Do not combine
these slices into one broad "facts layer" pass.

### Slice A: Runtime Clock

Backend scope:

- `runtime.current_time.date`;
- `runtime.current_time.time`;
- `runtime.current_time.weekday`;
- `runtime.current_time.timezone`;
- runtime fact provenance fields.

Product UI scope:

- default answer source chip for `本机时钟`;
- expanded trace can show runtime clock fact keys and source, but not raw
  prompts, memory, provider secrets, absolute paths, or unrelated tool state.

Out of scope for Slice A:

- provider route facts;
- tool/MCP availability;
- task status/self-state;
- broad natural-language date parsing;
- calendar events or weather;
- active timezone geolocation;
- durable memory writes;
- rewriting AgentIngress or StrategyRouter wholesale.

### Slice B: Provider Route Semantics

Scope:

- `provider.current_turn_generation.*`;
- `provider.last_completed_generation.*`;
- `provider.configured.default_*`;
- `provider.planned_route_if_model_needed.*`;
- UI labels that clearly separate current, last, configured, and planned route.

### Slice C: Tool And MCP Availability

Scope:

- `tool.web.config_enabled`;
- `tool.web.credential_available`;
- `tool.web.policy_allowed`;
- `tool.web.reachable` reachability status and freshness from cached or
  explicit preflight only;
- `tool.web.available`;
- `tool.mcp.registered_count`;
- `tool.mcp.read_only_allowed_count`;
- `tool.mcp.server_status` as cached/known/unknown.

Out of scope for Slice C:

- active reachability probing during a normal chat turn;
- broad MCP health monitor;
- raw MCP manifest inspection in default UI.

### Slice D: Agent Self-State

Scope:

- `agent.task_status`;
- `agent.delivery_status`;
- `agent.last_action.summary`;
- `agent.pending_permission.count`;
- `agent.blocker.codes`;
- `agent.pending_proposal.count`;
- `agent.durable_change.status`;
- `agent.self_state.trace_gap`;
- task/session/run provenance for self-state answers.

## 9. Stop Conditions

Stop implementation and return to spec if:

- a runtime fact needs raw system prompt, raw memory, provider key, or raw MCP
  manifest to answer;
- a fact cannot name its source and authority;
- a missing fact would fall back to model invention;
- UI would need to infer status from assistant prose;
- a fact conflicts with privacy/tool/write policy;
- implementation starts adding unbounded natural-language regexes instead of
  typed fact keys.

## 10. Practice Reference Map

Access date for the references below: 2026-06-25.

The MCP reference is intentionally pinned to the 2025-06-18 versioned
specification for this preparation track. Updating it requires a registry
revision so eval expectations and implementation behavior change together.

| Practice | Primary source | Adopted contract |
| --- | --- | --- |
| Keep local runtime context separate from LLM-visible context. | [OpenAI Agents SDK - Context management](https://openai.github.io/openai-agents-python/context/) | `RuntimeFact` separates source/authority from model-visible answers; runtime facts cannot be replaced by prompt text. |
| Trace runs, tool calls, and runtime events separately from final text. | [OpenAI Agents SDK - Tracing](https://openai.github.io/openai-agents-python/tracing/) | UI uses source chips and expanded trace instead of treating assistant prose as evidence. |
| Use application/tool code for known structured values instead of asking the model to invent them. | [OpenAI Function Calling](https://developers.openai.com/api/docs/guides/function-calling) | `modelFallbackAllowed=false` for deterministic runtime facts and missing facts. |
| Provide current-date runtime context in assistant products. | [Anthropic System Prompts](https://platform.claude.com/docs/en/release-notes/system-prompts) | Runtime clock facts are first-class and cannot be guessed by the model. |
| Separate thread-scoped state from long-term store. | [LangGraph Persistence](https://docs.langchain.com/oss/python/langgraph/persistence) and [LangGraph Memory](https://docs.langchain.com/oss/python/concepts/memory) | Agent self-state uses task/session/run state; Memory/HS remain store or bounded context, not current runtime authority. |
| Side-effectful or risky actions require interruption/approval paths. | [LangChain Human-in-the-loop](https://docs.langchain.com/oss/python/langchain/human-in-the-loop) | Write-like availability means proposal, permission, or blocker, never silent write. |
| Tool metadata and safety annotations must be bounded and policy-aware. | [MCP Tools Specification](https://modelcontextprotocol.io/specification/2025-06-18/server/tools) | Tool/MCP availability distinguishes registry, policy, safe read candidates, server status, and hidden raw manifest details. |
