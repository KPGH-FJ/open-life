# UI Terminology Inventory

## Current User-Facing Terms

| Term | Language | Location | Context | Notes |
| --- | --- | --- | --- | --- |
| `Today` | English | `productShellContract.ts`; ProductShell nav | Primary route label | Page title is `今日`, creating route/page mismatch. |
| `Companion` | English | `productShellContract.ts` | Primary route label | Component wraps Chat with agent stage. |
| `Mailbox` | English | `productShellContract.ts`; `MailboxPage` | Primary route and review surface | Competes with `Review Center` concept from task brief and `Open Mailbox` copy. |
| `Life Model` / `LifeModel` | Mixed | route labels, pages, proposal copy | Domain object | Strong domain term; Chinese-first decision needed. |
| `Runs` | English | route label, page | Run/task history | Page labels are partly Chinese. |
| `Settings` | English | route label, page | Settings | Several tabs use English names. |
| `Advanced` / `Technical surfaces` | English | `ProductShell` | Advanced menu | Candidate `高级/开发者`. |
| `MCP / Tools`, `A2A`, `Metrics`, `Calibration`, `Versions` | English/mixed | advanced routes | Technical/maintenance surfaces | Need visibility and naming decision. |
| `今日` | Chinese | `TodayPage` | Daily page heading | Good Chinese-first anchor. |
| `待确认` | Chinese | Today/Mailbox/Settings | Pending review/proposal state | Strong candidate for review-required state. |
| `Safe Mode` | English | ProductShell, Today, Memory, Settings | Safety mode | Could stay as branded technical term or become `安全模式`. |
| `Execution evidence` | English | `MainChatExecutionEvidence` | Chat evidence strip | Too technical for default workspace copy. |
| `Agent Control Plane` | English | `AgentControlPlane` aria label | Chat control/evidence panel | Internal term. |
| `Provider`, `Model`, `Route reason` | English | `runtimeDisclosure.ts`; Settings | Runtime/provider proof | Advanced or renamed user-facing trust terms needed. |
| `Tool Permissions` | English | Settings Tools tab | Tool authorization state | Chinese candidate needed. |
| `Local only`, `Auto`, `Cloud` | English | Settings Provider tab | Model route choice | Mixed with Chinese descriptions. |
| `构建`, `概览`, `依据` | Chinese | `LifeModelPage` | LifeModel tabs | Good Chinese-first anchors. |
| `记忆`, `工具权限`, `外部操作`, `模型策略` | Chinese | `reviewDecision.ts` | Review group labels | Good review taxonomy anchors. |

Finding: Product navigation labels are mostly English while many page bodies are Chinese.
Evidence: `PRIMARY_PRODUCT_ROUTES` labels are English; `TodayPage`, `MailboxPage`, `AgentStage`, `reviewDecision`, and `proposalDisplay` use substantial Chinese copy.
File location: `frontend/src/productShellContract.ts`; `frontend/src/pages/TodayPage.tsx`; `frontend/src/pages/MailboxPage.tsx`; `frontend/src/components/AgentStage.tsx`; `frontend/src/utils/reviewDecision.ts`; `frontend/src/utils/proposalDisplay.ts`.
Confidence: High.
Impact: Chinese-first V2 needs a route/name glossary before implementation.

## Status Terms

| Concept | Current terms found | Candidate Chinese | Notes |
| --- | --- | --- | --- |
| running | `Running`, `运行中`, `执行中` | `运行中` | Use one label across Chat/Runs/tools. |
| waiting permission | `Waiting for you`, `Permission pending`, `等待确认`, `待授权` | `等待你确认` | Distinguish from generic pending. |
| blocked | `Blocked`, `Restricted`, `已阻断`, `治理阻断` | `已阻断` | Keep blocker reason visible. |
| failed | `failed`, `失败`, `发生错误` | `失败` | Preserve failure, do not flatten to completed. |
| cancelled | `cancelled`, `已取消` | `已取消` | Already mostly consistent. |
| completed | `Completed`, `已完成`, `completed_with_pending_items` | `已完成` / `已完成但有待确认项` | Need special label for pending-proposal completion. |
| pending | `pending`, `待确认`, `读取中` | `待确认` or `读取中` by context | Avoid generic `pending` when the state is loading. |
| proposal required | `Proposal pending`, `待确认记忆`, `待确认 LifeModel 更新`, `待确认项` | `需要审核` / `待确认更新` | Use for Review Center. |
| approved | `accepted`, `已同意` | `已同意` | Current Mailbox label is good. |
| rejected | `rejected`, `不同意` | `不同意` | Current Mailbox label is good. |
| applied | `已应用到人生模型`, `已应用到记忆治理` | `已应用` plus target | Must only appear after durable apply/materialization. |
| materialized | `materialized` in lifecycle/evidence | `已写入长期状态` | Needs human approval; do not imply proposal acceptance alone. |
| rolled back | rollback/restore terms in memory/version surfaces | `已回滚` / `已恢复` | Context-specific; memory rollback and version restore differ. |

Finding: Status vocabulary is not yet canonical.
Evidence: Chat product status labels are English; Runs and Mailbox map statuses to Chinese; runtime disclosure maps status to Chinese; final delivery uses backend statuses.
File location: `frontend/src/pages/ChatPage.tsx`; `frontend/src/pages/RunsPage.tsx`; `frontend/src/pages/MailboxPage.tsx`; `frontend/src/utils/runtimeDisclosure.ts`; `frontend/src/components/AgentControlPlane.tsx`.
Confidence: High.
Impact: V2 must define canonical status labels before UI design, especially for pending/proposal/final-delivery states.

## Product Naming Conflicts

| Conflict | Evidence | Impact |
| --- | --- | --- |
| `Mailbox` vs `Review Center` / `审核中心` | Current route and copy say Mailbox; task brief expects Review Center candidate. | Review decisions may feel like messages rather than governed approvals. |
| `Companion` vs `Chat` vs `Workspace` | `/companion` renders `ChatPage`; old `/chat` redirects to `/companion`; goal asks about Agent Workspace. | IA decision needed before route/component implementation. |
| `Runs` vs `Tasks` | Runs page merges `AgentRun` history with Main Chat task controls. | User may not know whether to look for current tasks or past evidence. |
| `Life Model`, `LifeModel`, `人生模型` | All appear in UI/docs. | Chinese-first naming and brand casing needed. |
| `Memory`, `记忆`, `长期记忆`, `热记忆`, `memory governance` | MemorySearch and settings use several terms. | Need lane/status glossary. |
| `Safe Mode` vs `安全模式` | UI currently uses `Safe Mode` with Chinese explanation. | Human choice: keep branded English or localize. |
| `Provider`, `云端模型`, `模型路线`, `Route` | Settings mixes English provider statuses with Chinese copy. | Privacy trust copy needs standardization. |

## Proposed Chinese-first Naming Candidates

These are candidates only, not final decisions.

| Concept | Current terms | Candidate Chinese | Notes |
| --- | --- | --- | --- |
| Today | Today, 今日 | `今日` | Strong default. |
| Workspace / Workbench | Companion, Chat, Workspace | `工作区` or `任务工作台` | `工作区` is shorter; `任务工作台` is clearer for agent execution. |
| Companion | Companion, 陪伴 | `陪伴` | Could become a mode inside `工作区`. |
| Chat | Chat, 对话 | `对话` | If merged, use as composer mode not top-level route. |
| Tasks | Runs, task sessions, AgentRun | `任务` | Include active/past tasks and run evidence. |
| Review Center | Mailbox, Review, pending proposals | `审核中心` | Better matches governed approvals than `信箱`. |
| Mailbox | Mailbox, 待确认 | `待确认` or `审核收件箱` | If retained as sub-view under `审核中心`. |
| Memory | Memory, 记忆, 长期记忆 | `记忆` | Add lane terms: `上下文`, `待确认`, `长期记忆`, `已归档`. |
| LifeModel | Life Model, LifeModel, 人生模型 | `LifeModel` or `人生模型` | Product/brand decision required. |
| Agent | Agent, OpenLife Agent | `OpenLife` / `智能体` | Prefer `OpenLife` in user copy; reserve `智能体` for advanced concepts. |
| Proposal | proposal, 确认项, 候选更新 | `待确认项` / `候选更新` | Use `候选更新` for object, `待确认项` for state. |
| Evidence | evidence, 依据, 证据 | `依据` | More user-friendly than `证据`; use `证据` in audit/legal contexts. |
| Provenance | provenance, 来源, source refs | `来源` | Use `来源与记录` for technical drawers. |
| Advanced diagnostics | Advanced, Technical surfaces, diagnostics | `高级检查` / `开发者` | Split user advanced inspection from developer-only tools. |
| Safe Mode | Safe Mode | `安全模式` or `Safe Mode（安全模式）` | Human decision needed. |
| Tool Permissions | Tool Permissions, 工具权限 | `工具权限` | Existing good Chinese term. |
| Provider readiness | Provider readiness, 云端模型, 模型路线 | `模型路线` / `云端可用性` | Default copy should focus on where data goes. |

## Human Decisions Needed

1. Choose final top-level Chinese route names.
2. Decide whether `LifeModel` remains English-branded or becomes `人生模型` in navigation.
3. Decide whether `Safe Mode` remains English-branded.
4. Decide whether `Mailbox` is renamed to `审核中心`.
5. Decide whether `Runs` is renamed to `任务`.
6. Approve canonical status labels for `completed_with_pending_items`, `waiting_permission`, `blocked`, `failed`, and proposal lifecycle states.
7. Approve the vocabulary distinction between `依据`, `证据`, and `来源`.
