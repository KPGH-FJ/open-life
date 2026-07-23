# Navigation And Settings UX

Status: `REVIEW_CANDIDATE`

## 1. Fixed Information Priority

Every product surface follows the same order:

1. current user goal or current truth;
2. blocker, risk, or important exception;
3. next allowed action;
4. evidence entry;
5. raw technical/debug information.

The same fact should not be repeated in the header, banner, metric, list,
button reason, and Inspector. One page owns one primary conclusion. Supporting
areas add new information, not paraphrases.

## 2. Primary Information Architecture

| Entry | One job | Does not own |
|---|---|---|
| 今日 | today's focus, schedule, and items that need attention | live execution timeline or task history |
| 工作区 | one current conversation/task and its execution | the full history queue or final review authority |
| 任务 | resume, retry, cancel, compare, and inspect current/recent tasks | primary composition or proposal decisions |
| 审核中心 | decide proposals, permissions, external actions, memory/LifeModel changes | raw runtime evidence or task history |
| LifeModel | inspect current personal understanding, provenance, candidates, applied changes | silently editing canonical truth |

Utility navigation:

- 设置;
- help/support;
- evidence/debug access only from context, never a top-level `高级` product
  destination.

`任务` stays in the IA because current `TasksViewModel` has a real continuity
contract. React migration may initially mark the route unavailable until the
page is ported; it must not silently redirect to Workspace.

## 3. Desktop Shell

- left sidebar: 232px, stable product navigation;
- top context bar: page name, at most one primary state, evidence trigger;
- main work surface: one primary user job;
- right Inspector: 344px when opened, otherwise zero-width;
- utility controls at sidebar bottom;
- QA toolbar outside the shell in prototypes only.

Workspace specifically avoids dashboard density. The current task header,
condensed timeline, active blocker, and composer are the main surface. Task
counts, route ids, raw events, provider receipts, and source digests move to
Tasks or Inspector.

## 4. Mobile Shell

Mobile does not stack the full desktop sidebar above content.

- 48-56px compact app bar with page title, navigation trigger, and evidence
  trigger;
- four-item bottom navigation: 今日 / 工作区 / 审核 / LifeModel;
- Tasks and Settings in the navigation drawer;
- Inspector becomes a bottom sheet with a visible grab region and close button;
- a risk/blocker summary is pinned near the top of the sheet so evidence never
  starts 1500px below the decision;
- decision bars remain visible above the safe-area inset without obscuring
  content;
- focus returns to the element that opened the drawer, dialog, or evidence
  sheet.

## 5. Dedicated Settings Context

Selecting Settings changes the sidebar from product navigation to settings
navigation. It is not another dashboard page.

Sidebar order:

1. 返回工作台;
2. 搜索设置;
3. 模型与供应商;
4. 隐私与网络;
5. 工具与权限;
6. 数据与恢复;
7. LifeModel 与记忆;
8. 外观;
9. 高级与支持.

The bottom area shows only current product build/account information that is
actually available. It must not invent a plan, cloud account, or subscription.

### 5.1 Search

Settings search matches category, control label, and help text. It must:

- update the result count through `aria-live`;
- never match API-key values, masked secret strings, raw config JSON, or
  evidence bodies;
- preserve keyboard order;
- show “没有匹配设置” with a clear reset action;
- not mutate any value while filtering.

### 5.2 Model And Provider Page

Recommended order:

1. current route/privacy conclusion from `ProviderPrivacyBoundarySummary`;
2. local preference/config controls, clearly labelled as configuration rather
   than current route truth;
3. cloud provider, model, endpoint, and masked credential;
4. connection test with explicit external-request confirmation;
5. save action;
6. last validation and transmission evidence;
7. advanced raw fields collapsed.

State model:

```text
clean
  -> editing
  -> testing (may require review/permission)
  -> test_succeeded | test_failed | test_blocked | test_unknown
  -> saving
  -> saved_awaiting_boundary_refresh
  -> refreshed_known | refreshed_unknown
```

Rules:

- testing does not save;
- saving does not prove the endpoint works;
- a successful test proves only that exact validation request;
- after provider/endpoint/model/credential changes, privacy truth becomes
  `待后端重新确认` until the refreshed summary says otherwise;
- masked or empty credential submissions preserve the existing secret only
  under the backend's provider/endpoint identity rules;
- changing provider/endpoint cannot carry an old masked secret to a new
  destination;
- external test confirmation names provider, endpoint host, model, and the fact
  that a network request may occur. It never prints the secret.

### 5.3 Privacy And Network

This page explains and controls policy without claiming route outcomes:

- current external transmission: `not_sent`, `sent`, or `unknown`;
- current route type and provider/model labels;
- network policy state and blocked reason;
- local-only requirement;
- durable transmission evidence entry;
- safe paths and data-minimization rules where relevant.

Green `本地处理` appears only when the backend summary proves a local route and
`not_sent`. A configured local model, `preferLocal`, or a closed network toggle
alone is insufficient.

### 5.4 Tools And Permissions

- tool manifests and enabled state are grouped by user job, not internal
  registry names;
- permission history distinguishes broad legacy records from exact
  action-bound one-time grants;
- revoke is shown only when the backend says a revocable persistent grant
  exists;
- an already-consumed one-time grant is historical evidence, not an enabled
  permission;
- dev-only MCP/A2A/plugin configuration is absent in production builds.

### 5.5 Data, Recovery, Memory, And Advanced

- export/import and destructive recovery use preflight confirmation;
- snapshots and rollback describe scope and evidence limitations;
- memory lifecycle, archive, rollback, and LifeModel linkage are user-language
  summaries over backend read models;
- raw route ids, operation ids, source digests, rebuild controls, and diagnostic
  JSON remain collapsed under Advanced/Support;
- advanced content uses position, type scale, and background for visual
  de-emphasis, never whole-block opacity.

## 6. Page-Level Primary Conclusions

| Page/state | One primary conclusion |
|---|---|
| Today ready | “今天先完成什么” |
| Today stale | “当前计划已陈旧，只读且不执行” |
| Workspace running | “当前任务正在做什么” |
| Workspace permission | “任务暂停在一个明确动作之前” |
| Tasks | “哪些任务需要我或可以继续” |
| Review pending | “建议改变什么，需要怎样决定” |
| Review approved | “决定已记录，但尚未应用” |
| LifeModel | “当前有来源的长期理解是什么” |
| Settings | “当前配置与真实传输边界分别是什么” |

## 7. Navigation Feedback

- current entry uses `aria-current="page"`;
- a ported screen navigates and moves focus to its main heading;
- a planned but unported screen opens a real unavailable state with reason and
  available alternatives;
- no clickable entry is allowed to do nothing;
- Back in Settings restores the previous product route and focus;
- browser/static prototype query state is QA convenience only, not a production
  route contract.
