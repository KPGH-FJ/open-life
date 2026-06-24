# Main Chat Runtime Facts UI Contract

> Date: 2026-06-25
> Status: required preparation artifact before Runtime Facts / Agent Self-State implementation
> Parent: `plans/main_chat_runtime_facts_source_registry.md`

## 1. Purpose

The Runtime Facts UI must show where an answer came from without dumping
internal trace noise into the main Chat surface. It should make the Agent more
trustworthy, not more verbose.

This contract defines what is visible by default, what is available in an
expanded trace, and what must remain hidden.

## 2. Core Principle

Every visible source/status claim must be backed by runtime evidence:

- A clock answer must map to a `runtime.current_time.*` fact.
- A model/provider label must map to current-turn generation evidence,
  last-completed generation evidence, configured default, or planned route if a
  model were needed. The label must name which route class it is.
- A tool availability claim must map to config + policy + preflight/registry
  evidence, not only a registered manifest.
- A task status claim must map to AgentTaskSession, AgentRun, ActionQueue, or
  blocker evidence.
- A pending permission/proposal claim must map to the relevant store record.

If evidence is missing, UI shows unknown, blocked, or trace gap. It must not
infer runtime facts from assistant text.

## 3. UI Surfaces

### 3.1 Default Chat Header

Default header may show:

| UI field | Required evidence | Notes |
| --- | --- | --- |
| Provider/model badge | current-turn generation, last-completed generation, configured default, or planned-if-needed route evidence | Never conflate configured/planned route with actual invocation proof. |
| Tool availability badge | derived `tool.*.available` facts | Show concise labels like `External read not connected`. |
| LifeModel/HS status | bounded context metadata | Must not imply raw LifeModel truth was injected. |
| Pending confirmation badge | ToolPermission/proposal/task state | Only if evidence exists. |

Default header must not show:

- provider keys;
- raw endpoint URLs;
- raw MCP manifest ids/descriptions;
- raw system prompt;
- absolute workspace paths;
- full LifeModel/Memory content.

### 3.2 Answer Source Chip

Every assistant response should be eligible for one primary source chip and zero
or more secondary/supporting source chips. The primary chip represents the main
factual authority for the final answer, not every component used to phrase the
answer.

Primary source chips:

| Chip | `sourceType` | Required evidence |
| --- | --- | --- |
| `本机时钟` | `runtime_fact` | `runtime.current_time.*`, `modelGenerated=false`, `toolCalled=false` |
| `模型生成` | `model_generation` | provider/model route, `modelGenerated=true` |
| `工具观察` | `tool_observation` | action id + observation metadata |
| `读取文件` | `tool_observation` | workspace resolver + file read observation |
| `记忆检索` | `memory_retrieval` | bounded memory source ids/digests |
| `等待确认` | `permission_request` | pending ToolPermission/proposal evidence |
| `已阻塞` | `blocker` | blocker code and runtime source |
| `提案待审` | `proposal_record` | ProposalStore id/status |

Secondary chips:

- `无写入`;
- `无外部调用`;
- `外部读取未接入`;
- `上下文有限`;
- `需要用户确认`.

Rules:

- Source chips use user-facing labels, not internal enum names.
- Developer mode may show the enum in expanded trace.
- `模型生成` must not be shown for deterministic runtime facts.
- `工具观察` must not be shown unless a tool/action/observation object exists.
- For `runtime_fact + model explanation`, the primary chip remains the runtime
  fact if the main factual claim came from runtime; model synthesis can appear
  as a secondary chip only if model generation actually occurred.
- For `tool observation + model synthesis`, the primary chip remains the tool or
  source-specific observation when the answer depends on observed data; model
  synthesis is secondary.
- For `model reasoning over no external/runtime facts`, the primary chip is
  `模型生成`.
- For blocker or permission outcomes, the primary chip is `已阻塞` or
  `等待确认` even if the message also contains model-written explanation.
- A primary chip must never hide required supporting sources. Mixed-source
  answers must include supporting source chips or expanded trace entries.

### 3.3 Message-Level Status

Default message status can show:

| Status | Required evidence |
| --- | --- |
| `完成` | final answer/delivery evidence and no unresolved blocker for the turn |
| `执行中` | queued/running action or active stream |
| `等待确认` | pending permission/proposal/user input |
| `受限` | named policy/tool/provider/runtime blocker |
| `未知` | evidence gap, never a completion claim |

Rules:

- A proposal is not a completed durable change.
- A blocker is not completed work.
- Streamed text is not final delivery until finalization evidence exists.
- Legacy fallback must be visible in trace and cannot be styled as normal
  kernel-backed completion.

## 4. Expanded Trace

Expanded trace may show bounded metadata:

| Field | Visibility | Notes |
| --- | --- | --- |
| `taskSessionId` | developer trace | Bounded id only. |
| `runId` | developer trace | Bounded id only. |
| `selectedStrategy` | developer trace | Internal enum allowed only in expanded trace. |
| `sourceType` | expanded trace | Use canonical source type. |
| `runtimeFactKeys` | expanded trace | Show keys, not raw hidden values. |
| `modelGenerated` | expanded trace | Boolean. |
| `schedulerGenerationCalled` | expanded trace | Boolean. |
| `toolCalled` | expanded trace | Boolean. |
| `directWritesExecuted` | expanded trace | Boolean. |
| `toolObservationSummary` | expanded trace | Bounded summary. |
| `blockers` | expanded trace | Bounded labels and user action hints. |
| `pendingPermission` | expanded trace | Target label and action, no unsafe raw manifest details. |
| `contextSourceSummary` | expanded trace | Source kinds and counts. |
| `configuredProviderModel` | expanded trace | Must be labeled configured, not actual. |
| `currentTurnGenerationRoute` | expanded trace | Only when current run evidence exists; may explicitly say no model was generated. |
| `lastCompletedGenerationRoute` | expanded trace | Previous completed model route, not current turn. |
| `plannedRouteIfModelNeeded` | expanded trace | Route preview only, not invocation proof. |

Expanded trace must not show:

- raw system/developer prompt;
- full transcript by default;
- full LifeModel or raw Memory content;
- absolute workspace path;
- raw MCP manifest id/description;
- provider key, auth header, or secret endpoint parameters;
- unbounded eval blocker lists;
- internal digests unless developer mode explicitly requests them.

## 5. Developer Mode

Developer mode may expose additional diagnostics only when explicitly enabled.

Allowed:

- exact runtime fact keys;
- bounded source labels;
- route reason labels;
- selected strategy enum;
- bounded blocker labels;
- candidate/tool counts;
- redacted endpoint kind;
- digest labels already approved as metadata-safe.

Still forbidden:

- provider keys and auth headers;
- raw private memory;
- raw LifeModel YAML;
- raw MCP manifest descriptions;
- raw system prompt;
- unredacted absolute local paths.

## 6. Runtime Fact View Model

Backend should expose one coherent view model rather than forcing React to
reconstruct runtime logic.

```ts
type RuntimeFactUiSummary = {
  primarySource: {
    label: string;
    sourceType:
      | "runtime_fact"
      | "model_generation"
      | "tool_observation"
      | "memory_retrieval"
      | "proposal_record"
      | "permission_request"
      | "blocker";
    runtimeFactKeys: string[];
  };
  supportingSources: Array<{
    label: string;
    sourceType:
      | "runtime_fact"
      | "model_generation"
      | "tool_observation"
      | "memory_retrieval"
      | "proposal_record"
      | "permission_request"
      | "blocker";
    runtimeFactKeys: string[];
  }>;
  secondaryChips: string[];
  status: "completed" | "running" | "waiting_for_user" | "restricted" | "unknown";
  provenance: {
    modelGenerated: boolean;
    schedulerGenerationCalled: boolean;
    toolCalled: boolean;
    directWritesExecuted: boolean;
    legacyFallbackUsed: boolean;
  };
  trace: {
    taskSessionId?: string;
    runId?: string;
    selectedStrategy?: string;
    configuredRoute?: string;
    currentTurnGenerationRoute?: string;
    lastCompletedGenerationRoute?: string;
    plannedRouteIfModelNeeded?: string;
    blockers: string[];
    pendingPermissionCount: number;
    contextSourceSummary: string[];
  };
};
```

Rules:

- `primarySource.sourceType` must be derived from backend evidence.
- `supportingSources` is for mixed-source answers; it must be empty when there
  is no backend evidence for an additional source.
- `runtimeFactKeys` must be empty for pure model generation unless runtime facts
  were actually bound.
- `directWritesExecuted=true` must trigger a prominent warning and should be
  impossible for Runtime Facts paths.
- `legacyFallbackUsed=true` must disable normal completion styling unless the
  fallback is explicitly accepted for that surface.

## 7. Missing Evidence UI

| Missing evidence | Default UI behavior | Expanded trace |
| --- | --- | --- |
| Missing runtime clock | "当前时间未知" | `runtime.current_time.trace_gap` |
| Missing task session | Normal answer, no task completion badge | `task_session_missing` |
| Missing current-turn generation route | Hide current-turn route badge or show unknown | `provider_current_turn_generation_route_missing` |
| Missing tool policy | Show tool availability unknown | `tool_policy_missing` |
| Missing MCP server status | Show MCP availability unknown, not available | `mcp_server_status_unknown` |
| Missing final delivery | Show assistant text only | `final_delivery_missing` |

## 8. First UI Slice

The first UI slice is Slice A-UI for runtime clock answers. It should expose:

- source chip for `本机时钟`;
- expanded trace fields for `sourceType`, `modelGenerated`,
  `schedulerGenerationCalled`, `toolCalled`, `directWritesExecuted`, and
  `legacyFallbackUsed` when those fields are already present on the runtime
  clock answer.

It must not introduce generic source chips for model generation, blockers,
tools, task completion, or route status unless the corresponding Runtime Facts
slice has already implemented the backing evidence fields and eval scenarios.

Out of scope for the first slice:

- source chip for `模型生成`;
- source chip for `已阻塞`;
- message status for `完成` and `受限`;
- provider route badges;
- tool availability badges;
- full developer trace console;
- tool timeline redesign;
- memory browser;
- raw prompt export;
- exact MCP manifest inspection;
- time-travel UI.

## 9. Stop Conditions

Do not ship a UI change if:

- a visible chip can be created from assistant prose alone;
- configured or planned route is displayed as actual invocation proof;
- a registered tool is displayed as available without policy/preflight context;
- a proposal or blocker is displayed as completed durable work;
- raw sensitive fields appear in default UI;
- expanded trace requires frontend to parse raw transcript strings.
