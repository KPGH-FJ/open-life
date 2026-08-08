# OpenLife Product Development Program

## 1. 计划目的

这是 OpenLife 唯一生效的产品开发总计划。

本计划的目标按以下顺序固定：

1. 让 OpenLife 安全、可靠并真正可用；
2. 让它具备一线 Agent 产品应有的工作记忆和任务能力；
3. 让它能够在用户控制下安全执行行动；
4. 让 Agent Memory 与用户拥有的 LifeModel 分工协同，形成真正的个人
   智能闭环；
5. 让源码版本达到稳定、可复现的内部试用水平。

仓库治理平台、开发自进化系统、机器化计划系统、通用 Agent
开发平台都不属于本计划。

## 2. 权威与变更规则

当前权威关系如下：

- `PRODUCT.md` 定义产品；
- `AGENTS.md` 定义长期开发和安全规则；
- `plans/README.md` 索引本计划及已接受的 ADR；
- ADR 只记录需要长期保持的架构决定；
- 当前源码和真实运行证据决定实现事实；
- Git 历史中的旧计划不再拥有执行权。

六个阶段的顺序、产品目标和完成边界固定。Agent 不得自行插入、改名、
拆分、替换或重排阶段。

如果新的事实确实要求修改总计划，必须：

1. 给出当前源码或产品证据；
2. 解释为什么原计划不能继续；
3. 只提出对本文件的修改建议；
4. 获得用户明确批准后再修改。

阶段内部的具体问题和实现方案可以根据当前事实调整，但不得为了让计划
看起来完整而提前猜测。

禁止为本计划建立 JSON 镜像、问题 ledger、task packet、审批注册表、
证据注册表、validator、digest chain 或自进化工作流。

## 3. 初始事实基线

- 唯一开发目录：`/Users/tw/Desktop/open-life`；
- 本地与远端长期分支：`main`；
- 初始基线提交：
  `56fe8aa895ad817bd26a002ae8f84761fe4d6dfe`；
- 技术栈：Tauri 2、Rust、React 18、TypeScript、SQLite；
- 产品区域：Today、Workspace、Tasks、Review、Life Model、Settings；
- 仓库清理已经完成，但产品黄金路径和一线 Agent 能力尚未完成。

进入本计划时已经确认或需要重新验证的问题包括：

- 整体保存 LifeModel 的接口尚未在持久化边界证明真实原生用户确认；
- 启动阶段 Keychain 操作超时后存在尚未证伪的延迟交互窗口；
- 前端 bridge 与 shipped Tauri command 仍有退役或不匹配接口；
- 当前 Workspace 尚未完整提供流式响应、取消、资源、技能和工具体验；
- 部分 Settings 入口仍明确处于不可用状态；
- browser shell、mock、fixture、scripted provider、native Tauri 和
  external live 是不同证据等级；
- 部分运行时文件过大，只应在产品切片真正触及时拆分。

这些是阶段输入，不是永久冻结的问题清单。每个阶段开始时必须重新验证
与该阶段相关的事实。

## 4. 产品优先开发契约

每个开发切片必须回答：

1. 用户完成了什么以前不能可靠完成的事情？
2. 哪一条真实源码和运行路径拥有该行为？
3. 如何在真实 Tauri 产品中验证？
4. 哪些部分仍是未知或明确不在范围内？
5. 是否直接推进当前阶段？

第五项如果答案是否定的，就不进入当前开发。

测试、评估器、文档、诊断和重构都是辅助工作。除非它们是交付或保护
当前用户结果所必需的，否则不计算为产品进展。

开发切片必须尽量形成最小垂直闭环，避免：

- 后端已经存在能力，但产品没有入口；
- 前端有控件，但后端没有真实状态；
- 测试通过，但生产路径并未使用；
- 为未来想象中的平台预建大量扩展点。

### Agent Memory、LifeModel 与业务事实的长期边界

OpenLife 是 Personal Agent OS。Agent Memory 与 LifeModel 是协同的两个域，
不能互相替代，也不能把同一事实同时交给多个写入权威。

- **Agent Memory** 服务“如何完成任务”：包括会话历史、Workspace/项目
  上下文、任务步骤、中间结果、情景记忆、程序性经验、Reflection 和有界
  Markdown 工作记忆；
- **LifeModel** 服务“如何长期理解并为这个用户决策”：包括身份、价值观、
  长期目标、稳定偏好、个人边界、重要关系、长期协作方式和决策原则；
- **Evidence / Proposal** 是二者之间的受治理桥梁。日常交互、任务和行动结果
  可以形成候选证据，但只有具有长期意义、去重并处理冲突的信息才可以形成
  LifeModel proposal；只有用户确认并完成物化后才成为 LifeModel 的一部分；
- **业务事实继续由各自域拥有**：短期状态和每日任务属于 StateStore，任务与
  行动生命周期属于 Tasks/AgentRun/action receipt，日历和邮件对象属于对应
  connector。它们不得因为被 Agent 使用就自动复制为 LifeModel 真相；
- **索引与表达不是第二权威**：FTS、vector、cache 和 runtime packet 是可重建
  投影。Markdown 是工作上下文或人类可读表面，不是权限；YAML 是用户可读、
  可导出和可比较的 LifeModel 表达，不得与结构化存储形成独立双写权威；
- **用户直接编辑 YAML** 时，修改应先转为结构化 diff 并进入与其他 LifeModel
  更新相同的校验和 proposal 流程，不能绕过版本、冲突和审核；
- **每个任务可以反思，但不必产生 proposal**。候选证据先经过相关性、稳定性、
  重复、冲突和敏感度判断，避免 Review Center proposal 疲劳。

运行时不得把完整 LifeModel 或所有 Memory 无边界注入 prompt。它必须按当前
任务选择有来源、版本、置信度和新鲜度的最小上下文。默认决策优先级为：

1. 产品安全、隐私与权限政策；
2. 用户当前明确指令；
3. 已确认且与当前任务相关的 LifeModel；
4. 当前 Workspace、项目和任务上下文；
5. 相关情景、语义和程序性 Memory；
6. Agent 推断与 Reflection。

当前指令与 LifeModel 冲突时，产品必须区分“本次例外”和“长期改变”。本次
例外不能静默更新 LifeModel；长期改变通过 proposal。LifeModel 可以影响
planning、reasoning、context building、memory retrieval 和 tool selection，
但不能授予工具权限、凭据或外部写入许可。

## 5. 每个阶段统一采用的执行方法

以下七步是阶段内部的方法，不是七个新的开发阶段。

### 第一步：建立当前事实

- 追踪真实 import、handler、runtime、persistence 和 read model；
- 尽可能在产品中复现问题；
- 将证据区分为已复现、源码确认、历史证据和未知；
- 不因为测试名称或旧文档而认定功能存在；
- 区分 production、dev-only、test-only、browser、native 和 external。

### 第二步：确定本阶段问题

- 只纳入阻碍本阶段产品目标的问题；
- 按用户伤害、安全、产品价值和依赖关系排序；
- 区分根因与表面现象；
- 明确非目标；
- 不重新启动全仓治理审计。

### 第三步：研究相关最佳实践

- 优先使用官方文档、规范和第一方资料；
- 交互问题参考成熟 Agent 产品的实际行为；
- 涉及确认、幂等、取消、超时、恢复、持久化和隐私时进行专项研究；
- 只保留会改变解决方案的结论；
- 不把研究扩展成新的长期框架。

### 第四步：选择解决方案

重要问题至少比较：

- 最小安全修复；
- 当前推荐的产品方案；
- 长期架构方案。

选择能够完成当前阶段、又不会产生平行权威、兼容壳或过早平台化的方案。

### 第五步：实现垂直切片

- 每个切片范围清晰、能够独立 review；
- 保持 proposal-first 和禁止静默写入；
- 根据用户结果连接 UI、后端事实、运行时和恢复路径；
- 只拆分该切片实际触及的模块；
- 不为想象中的未来平台增加抽象。

### 第六步：三层验证与反幻觉检查

1. **源码证据**：生产实际使用的路径与声明一致；
2. **自动化证据**：测试覆盖真实契约和关键失败反例；
3. **产品证据**：真实 Tauri 应用走通用户路径。

外部 live 行为必须单独授权和验证。Browser 或 mock 证据不能代替 native
或 external-live 证据。

每个关键节点必须检查：

- 是否引用了陈旧结论；
- 是否把旧文档当作当前事实；
- 测试名称是否夸大了实际证明；
- 是否把 unknown 显示或描述成 success；
- 当前结论是否绑定正在审查的精确提交。

### 第七步：阶段验收

进入下一阶段前必须：

- 总结实际交付的用户能力；
- 列出剩余 blocker 和 unknown；
- 给出真实产品试用结果；
- 逐项检查退出标准；
- 更新本文件的“当前阶段”；
- 进入既定的下一阶段，不重新发明路线。

## 6. 固定六阶段

### 第一阶段：安全与接口收口

#### 阶段目标

形成可信的产品开发底座：特权持久化必须具有真实授权，启动凭据行为安全
失败，前端与 Tauri 产品接口不存在已知的损坏或退役入口。

#### 主要范围

- 修正整体保存 LifeModel 的授权边界；
- 复现并处理 Keychain 启动超时及延迟交互风险；
- 删除或正确绑定不匹配的前端 IPC；
- 删除没有当前产品所有者的退役 shipped command 和 dev bridge；
- 对齐 command 注册、前端 bridge 与真实产品调用者；
- 保持现有用户数据和凭据兼容。

#### 大致路径

1. 对每个对象建立真实调用和持久化路径；
2. 使用隔离 QA 数据复现原生凭据行为；
3. 必要时研究 Tauri 权限边界和 macOS Keychain 行为；
4. 选择最小安全的授权或取消设计；
5. 用少量聚焦 PR 完成；
6. 重跑对应门禁和真实原生启动验证。

#### 非目标

- 不整体重写 runtime；
- 不重做全部 Settings；
- 不增加新产品功能；
- 不建设通用安全平台；
- 不因为代码看起来旧就删除生产代码。

#### 退出标准

- 相关持久写入都绑定后端或原生拥有的用户授权；
- Keychain 超时行为得到复现和解决，或保持可证明安全的显式 blocker；
- 当前前端 wrapper 与 shipped handler 无已知不匹配；
- 范围内退役入口已经删除；
- 真实 Tauri 启动无非预期凭据弹窗；
- 精确审查提交的本地门禁和 CI 通过；
- 没有新发现但未解决的 P0/P1 阻塞。

### 第二阶段：真正可用的产品黄金路径

#### 阶段目标

用户能够启动 OpenLife、理解状态、配置并验证 Provider、进行真实对话、
取消或恢复失败、处理权限或 proposal，并继续同一任务。

#### 标准黄金路径

```text
启动 OpenLife
  -> 理解当前状态
  -> 配置 Provider
  -> 验证 Provider 可用性
  -> 创建或继续会话
  -> 获得流式响应
  -> 必要时取消、重试或恢复
  -> 处理 blocker、权限或 proposal
  -> 恢复同一任务
  -> 看见真实终态
```

#### 主要范围

- 首次启动和凭据恢复；
- Provider 配置和显式验证；
- 区分未配置、未验证、可用、失败、过期和未知；
- 流式会话和取消；
- 会话创建、继续、重命名、删除和历史恢复；
- 产品可见的任务、阻塞、权限和 proposal；
- Review Center 决定与同任务继续；
- 模型、网络和 runtime 失败恢复。

#### 非目标

- 不同时支持所有 Provider；
- 不建设完整工具生态；
- 不允许自主外部写入；
- 不一次性重做六个产品区域；
- 不提前宣称一线 Agent 能力。

#### 退出标准

- 全新 QA profile 能走完整条黄金路径；
- streaming、cancel、retry 和 resume 终态可信；
- “已配置”不会显示为“已验证可用”；
- proposal 批准不会提前显示为已经写入；
- 用户无需开发者诊断即可恢复；
- 成功和代表性失败路径都有 native 证据；
- 产品适合日常内部对话使用。

### 第三阶段：一线 Agent 基础能力

#### 阶段目标

OpenLife 能够像成熟 Agent 产品一样，可靠完成有用的多步骤读取和内容生成
任务，并提供真实证据、工具状态和失败恢复。

#### 主要范围

- 可靠的多轮 AgentLoop；
- direct answer、planning、read、tool 和 governed action 路由；
- 有界上下文和长对话处理；
- 本地文件及资源读取；
- 真实网页研究和来源绑定；
- 文件及 artifact 生成；
- MCP 读取能力；
- 产品中的资源、技能和工具选择；
- 多步骤任务进度、暂停、恢复、重试和取消；
- 基于证据的回答与引用；
- 禁止隐藏 fallback 完成。

#### 建议能力顺序

1. 本地文件和资源读取；
2. live web 研究和引用；
3. 文件和 artifact 生成；
4. 已注册 MCP 读取；
5. 混合能力的多步骤任务。

如当前证据要求不同顺序，必须说明原因并获得批准。

#### 非目标

- 不进行无边界自主执行；
- 不开放广泛外部写入；
- 不宣称能够观察远端模型内部过程；
- 不建设通用技能市场；
- 不建设自进化平台。

#### 退出标准

- 代表性真实任务通过 shipped 产品路径完成；
- 工具尝试、证据、blocker 和终态产品可见；
- 文件、网页、资源和 MCP 读取保持来源与信任边界；
- cancel 和 retry 不会重复不确定副作用；
- 长任务可以恢复且不虚构先前成功；
- 获得授权的 external-live 试用验证相应能力；
- mock、fixture 和 scripted evidence 始终保持明确标记。

### 第四阶段：受治理的行动型 Agent

#### 阶段目标

OpenLife 能够在用户控制下执行有用改变，同时保持确认、审核、幂等、
副作用确定性、恢复和用户所有权；Agent 能够通过成熟但有界的工作记忆继续
任务，并使用经确认的 LifeModel 上下文改善规划和选择，而不混淆事实与权限。

#### 主要范围

- 先收敛 Agent Memory、LifeModel、StateStore、Tasks 和 action receipt 的
  当前读写职责，修复会让第四阶段读取错误事实的交叉路径；
- 补足行动所必需的 Agent 工作记忆：会话恢复、有界历史与压缩、Workspace/
  项目作用域、Markdown 索引和专题上下文、Reflection、来源和删除控制；
- 建立任务相关的 LifeModel runtime packet v1，只选择与当前任务相关、已确认、
  有来源和新鲜度的最小长期上下文；
- 受治理的文件创建、修改、移动、回收和常用工作产物；
- 持久化本地任务、提醒和可撤销状态变更；
- 日历、邮件和 Web/browser 等外部 connector action 按能力逐项加入；
- 有边界的本地执行，以及可暂停、恢复和取消的后台或长时间工作；
- 风险分级的确认和 Review Center 流程；
- 幂等和精确 action identity；
- cancel、timeout、unknown effect、reconcile 和 rollback；
- 区分 proposed、approved、dispatched、unknown、failed、
  materialized、rolled back 和 completed。

#### 大致路径

1. 建立真实 source map 和职责表，只修复第四阶段会触及的双权威、旧投影或
   错误读取，不全面重构 LifeModel；
2. 补足可靠行动所需的 Agent Memory 基础，不在本阶段建设完整个人学习系统；
3. 建立 LifeModel runtime packet v1，并证明它只提供决策上下文、不授予权限；
4. 依次完成文件/工作产物、本地任务与提醒、日历、邮件、Web/browser、有边界
   本地执行和长任务；一次只加入一种行动能力；
5. 行动结束后分别保存业务事实、action receipt 和 Agent Reflection。具有长期
   意义的信息只成为候选证据，不直接写入 LifeModel；
6. 先验证可逆本地行动，再验证敏感外部行动，最后进行真实 Tauri 多步骤验收。

#### 非目标

- 不静默执行外部或持久写入；
- 不从助手文本推断用户授权；
- 不从 LifeModel 偏好、Memory 或 Markdown 推断工具权限；
- 不自动重试可能已经产生远端副作用的行动；
- 不同时上线邮件、日历、shell、plugin 和 Provider 写入；
- 不在本阶段完成 LifeModel schema、证据网络和长期学习闭环的全面重构；
- 不把所有 Agent Memory 迁入 LifeModel；
- 不脱离真实用户场景建设通用 action 平台。

#### 退出标准

- 每种 shipped action 都有明确 capability、risk、confirmation 和
  terminal evidence；
- 会话、Workspace 和任务所需工作记忆能够跨重启恢复，Markdown 记忆按作用域
  有界加载，过期、冲突或来源不明内容不能冒充事实；
- 至少在 planning、context building、memory retrieval 或 tool selection 的代表性
  场景中，LifeModel runtime packet 产生可追溯的正面影响，同时当前用户指令和
  权限边界保持更高优先级；
- 代表性可逆行动能够完成和回滚；
- 敏感行动没有所需授权就不能 dispatch；
- 远端副作用不明时保持 unknown，直到完成 reconciliation；
- Tasks 和 Review Center 与持久执行事实一致；
- 行动结果、Agent Reflection、Memory 候选和 LifeModel proposal 保持可验证的
  分离，没有静默长期画像写入；
- native 内部试用覆盖成功、拒绝、取消、超时、防重复和恢复。

### 第五阶段：LifeModel 与 Memory 个人智能闭环

#### 阶段目标

OpenLife 在成熟 Agent Memory 之上构建用户拥有的长期个人模型。LifeModel
持续刻画用户并真实改善对话、规划、写作和行动；每条进入长期模型的信息都有
证据、proposal、版本、用户决定和回滚能力。

#### 标准闭环

```text
交互、任务与行动
  -> Agent Memory / Workspace / Reflection
  -> 候选证据提取
  -> 相关性、稳定性、重复、冲突与敏感度判断
  -> LifeModel proposal 或无长期写入
  -> 用户审核
  -> 版本化 LifeModel 物化
  -> YAML 人类视图与 runtime packet 投影
  -> 后续任务中可解释地使用
  -> 用户反馈、修正、归档或回滚
```

#### 主要范围

阶段五采用一个已经完成的前置架构边界切片和以下六个顺序固定的产品板块。
前一板块未达到退出标准前，不以补写计划、增加验证系统或并行建设下一套架构来
绕过问题。

##### Agent Memory 权威矩阵

阶段五不得再把层级、作用域、类型和存储载体混为一个枚举或一个总 Memory 表：

| 维度 | 固定语义 |
| --- | --- |
| 生命周期层 | 当前任务状态、Workspace Markdown Memory、跨会话 Memory、Task Reflection |
| 作用域 | Conversation、Workspace、Project、Global；Task 只是来源引用和检索绑定，不新增为第五种持久作用域 |
| 信息类型 | semantic fact、episodic experience、procedural working rule；Reflection 是候选来源，不是第四种长期 Memory |
| canonical owner | 原始会话归 messages/checkpoint；明确作用域内的工作文档归对应 Markdown 文件；跨会话可治理 Memory 归 MemoryLifecycleStore；Reflection 归 Task/AgentRun transcript |
| projection | 摘要、FTS、Vector、hot cache 和 runtime packet 均可重建，不能反向成为 canonical truth |

Markdown 只在对应 Workspace/Project 文件域内拥有该文档内容；进入 Agent 上下文
后仍是不授予权限、不冒充用户长期事实的 working context。当前用户明确指令在
当前任务中优先，但不会因此静默修改任何长期 Memory 或 LifeModel；发现冲突时
生成候选或提示用户，而不是在读取阶段决定新的 durable truth。

##### 5.1 完成主流 Agent Memory

目标：先让 Agent 本身真正会长期工作，不依赖 LifeModel 保存工作过程。

5.1 固定拆成以下六个顺序切片；不得把六个切片合并成一次基础设施重构。

###### 5.1A 跨重启继续当前会话

- 固定 messages/checkpoint 为原始会话和任务恢复权威；
- 核对 Main Chat、TaskSession、AgentRun 和 Workspace read model 的恢复边界；
- 同一操作的重放保持幂等，缺失或损坏 checkpoint 显式 degraded/failed；
- 不在本切片引入长期 Memory、Vector 或 LifeModel 变化。

退出标准：同一真实项目经过对话、任务暂停、应用重启后能够从已确认 checkpoint
继续；没有从助手文本或旧摘要重建虚假完成状态。

本切片实施边界（2026-08-06）：

- 用户能力：重启进入 Workspace 时，优先打开后端活动任务绑定的真实对话，而不是
  简单打开更新时间最新的无关对话；用户手动选定对话或开始输入后不再自动切换；
- owner：会话正文继续由 `MemoryStore.messages` 拥有，暂停与恢复资格继续由
  TaskSession、ActionQueue、AgentRun 和 durable event 共同证明，Workspace
  ViewModel 只提供活动任务及其 `conversationId`；前端不新增持久恢复状态；
- 失败边界：活动任务对应会话不存在时明确显示恢复依据缺失并关闭继续动作；运行中
  进程异常退出继续由 startup reconciliation 终止为可验证失败，不猜测成功；
- 非目标：不新增 summary、长期 Memory、Vector、LifeModel 或第二套 checkpoint；
  不把前端当前选择写成新的 canonical owner；
- 产品反例：覆盖“无关对话更新更晚”“任务读模型晚于会话历史返回”“用户已手动
  选择其他对话”“任务引用的原始会话缺失”；
- 清理：本切片没有建立替代 owner，因此没有可安全删除的后端恢复路径；后续不得
  保留按最新时间选择和按活动任务选择两套互相竞争的首次恢复规则。

###### 5.1B 摘要与长上下文压缩

- 原始 transcript 继续作为 canonical owner；summary 是带 source range、版本和
  digest 的可重建 projection；
- 压缩必须保留当前目标、未完成事项、用户明确约束、工具结果来源和未决 Review；
- summary 过期、来源缺失或与原始 transcript 不一致时不进入正常上下文；
- 上下文预算必须有上限，不把全部历史消息、全部 Markdown 和全部检索结果同时
  注入。

本切片实施边界（2026-08-06）：

- canonical owner：每个新回合先由 TurnRuntime 持久化当前用户消息，再从
  `MemoryStore.messages` 按该 operation 重新构建完整会话前缀；前端提交的旧消息
  不再被当成 provider 上下文权威；
- 有界上下文：Conversation provider message 正文总预算固定为 65,536 个字符；近期
  原文优先保留 45,056 个字符，旧消息使用最多 16,384 个字符的派生投影；单条当前用户输入不能
  超过总预算，避免无法压缩的当前指令突破上限；
- projection：`conversation_context_summaries` 只保存 schema version、canonical
  message ID 范围、source digest、summary digest 和确定性摘录；它不属于长期
  Memory 或 LifeModel，删除后可由 messages 重建，会话删除时同步删除；
- 保真与权限层级：明确约束、未完成事项、Review/权限和证据引用优先于普通首尾
  消息；被选中的历史摘录保持原始 `user` / `assistant` 角色，不把旧正文提升为
  system 指令，也不把摘要当作任务完成证明；
- 收敛：普通 send、stream、Provider consent continuation 和 read-tool replay
  共用同一个 bounded canonical context 入口；退役固定“最近 64 条”的恢复路径；
- 非目标：本切片不调用额外 Provider 生成摘要，不新增 Markdown Memory、Vector、
  LifeModel 或通用摘要平台，不扩展 durable event schema 来证明摘要系统。

退出标准：代表性长会话压缩前后，关键约束、未完成事项和证据引用保持一致；删除
summary 后可从 canonical transcript 重建。

###### 5.1C Workspace/Project Markdown Memory

- 明确允许的根目录、文件名、Workspace/Project scope 和最大加载预算；
- 使用简洁入口文件加按需主题文件，不把所有内容塞进一个无限增长的 MEMORY.md；
- 文件创建、编辑、移动和删除复用现有受治理 file-write 路径；Agent 推断不能
  静默写文件；
- 每次加载显示文件来源、scope、选中段落和选择原因；错误 Workspace 的内容不能
  跨项目进入上下文。

退出标准：用户可在一个真实项目中创建、查看、修改和停用 Markdown Memory；
另一个项目无法召回该内容，重启后作用域和来源仍一致。

本切片实施边界（2026-08-06）：

- scope owner：`SystemConfig.workspace_memory_root` 与
  `project_memory_root` 分别绑定用户通过原生目录选择器明确选定的 Workspace 和
  Project 根目录；进程 cwd 与通用 `knowledge_roots` 不再获得 Markdown Memory
  权威；两个 scope 指向同一物理目录时只加载一次；
- 文件契约：每个 root 只承认 `MEMORY.md` 与一层 `memories/*.md`；
  `*.disabled.md`、符号链接、嵌套/逃逸路径、超限或非 UTF-8 文件不进入正常读取；
  ViewModel 总计最多 65,536 个字符、每文件最多 32,768 个字符、每 scope 最多
  16 个文件；
- runtime 选择：仅把与当前任务标题或段落相关的最多 4 个文件、总计不超过 4,800
  字符注入 Main Chat；每个 context block 明示 scope、相对来源和选择原因，并标明
  它不是用户身份、权限或完成证据；
- 用户控制：Workspace 提供最小 Markdown 编辑器和两个 scope 的原生目录选择；
  读取来自 backend ViewModel，写入与停用分别只生成带 current digest/absent
  precondition 的 `ExternalWriteAction` proposal 和受审 move proposal；
- 物化与停用：批准后继续复用既有 artifact materializer；停用把文件移动为同目录
  `*.disabled.md`，在批准和确认物化前仍保持当前召回状态；
- 非目标：本切片不把 Markdown 写入 SQLite/LifeModel，不实现 5.1D 的跨会话
  semantic/episodic/procedural 生命周期，不增加 Vector/FTS，也不建立通用文件记忆
  平台。

###### 5.1D 显式跨会话 Memory 生命周期

- 完成“请记住”、受治理推断候选、纠正和 supersede；
- 精确定义四种不同产品动作：停止召回只移出 runtime context；归档可恢复；回滚
  撤销某次变更但保留历史；隐私擦除删除 canonical 正文以及 FTS、Vector、cache
  和其他内容投影，只保留不含正文的最小 tombstone/audit metadata；
- semantic、episodic 和 procedural Memory 分别有 scope、来源、敏感度、新鲜度和
  冲突语义；
- “忘记”不得作为模糊后端操作名；界面必须告诉用户实际执行的是停止召回、归档
  还是不可恢复擦除。

退出标准：显式记忆能够跨会话召回、纠正、归档和恢复；隐私擦除后正文不再出现
于 canonical store、FTS、Vector、cache、runtime context 或普通产品读取路径。

本切片实施边界（2026-08-06）：

- 复用现有 `MemoryLifecycleStore`、MemoryStore/VectorStore outbox projection、
  Review Center 和 Main Chat 显式记住路径，不新增 Memory 数据库或生命周期平台；
- 纠正只接受一个精确 `memory:<uuid>`，以受审 `MemoryWrite` 创建 replacement；
  Review 接受前旧 owner 不变，接受后旧记录成为 superseded 并退出 runtime；
- 停止召回、归档、恢复分别使用 `paused`、`archived`、`active` canonical retrieval
  disposition；paused 不冒充 archived，二者都不会进入正常检索；
- 回滚保留历史正文；隐私擦除使用原生危险操作确认，清空 canonical 正文、来源和
  content-bearing metadata，留下 body-free tombstone，并通过既有 outbox 删除
  MemoryStore/VectorStore 派生内容；
- backend `MemoryViewModel.items` 统一给出正文、scope、来源解释、召回状态和允许
  动作；`/life-model` 的 Memory 区域不从原始 store/telemetry 拼装产品真相；
- 非目标：不在本切片增加 FTS/Vector 排序、新鲜度算法、跨 scope 检索策略或
  provider 驱动 Reflection；这些属于 5.1E 及其后的独立验证边界。

###### 5.1E 混合检索与召回解释

- FTS 和 Vector 只检索允许 scope 内的 active Memory，合并、去重并按任务相关性、
  新鲜度、冲突和来源质量排序；
- embedding 不可用时降级到文本检索，不把空 Vector 结果冒充完整检索；
- 每条进入上下文的 Memory 都带 source ref、scope、freshness 和 selected reason；
- 建立少量产品黄金场景，验证 should-recall、must-not-recall、冲突、过期、中文和
  无 embedding 降级，不建设通用评估平台；
- 在本切片测量并固定 context token budget 与本机检索延迟基线，禁止无界加载。

退出标准：must-not-recall 与跨 scope 泄漏场景为零；所有已召回 Memory 都能追溯
真实来源，检索降级对用户可见且基础 Agent 可继续工作。

本切片实施边界（2026-08-06）：

- production owner：普通 send、stream 和 replay 继续经 `MainChatKernel` 编译上下文，
  lifecycle Memory 的唯一检索适配器改为调用现有 `MemoryGateway`；不恢复已退役的
  `main_chat_preprocess` runtime caller，也不新增检索或评测平台；
- scope authority：global Memory 可跨会话召回；conversation Memory 只允许精确的
  canonical conversation owner；Workspace/Project Memory 必须绑定用户明确选择的
  root 的不透明 owner ref。历史非 global 记录缺少 owner 时保持可查看但不进入
  runtime，禁止从当前 cwd、标签或相似文本猜测归属；
- hybrid retrieval：先在允许 scope 内 over-fetch FTS 与 Vector 派生候选，再以
  `MemoryLifecycleStore` 的 active owner 为最终过滤权威；按 memory id 去重，并综合
  lexical/vector relevance、freshness、未解决冲突、confidence 与来源质量作确定性
  排序；
- degradation：embedding 调用失败、profile unknown、rebuild required 或 vector
  query failure 时保留 FTS 结果，并把精确 degradation code 放入 context evidence；
  FTS 与 canonical lifecycle 读取失败仍然 fail closed，不把空结果描述为健康；
- explanation 与预算：每个注入块包含 `memory:<id>` source ref、scope/owner、
  freshness 和 selected reason；每回合最多 4 条、正文总计最多 4,800 字符，禁止把
  全部历史 Memory 注入 prompt；
- 验证：只新增少量产品黄金场景，覆盖 should-recall、must-not-recall、跨 scope、
  conflict、stale、中文和无 embedding fallback，并记录同一 QA 机器上的有界本地
  检索基线；不建立通用 benchmark、case registry 或治理 JSON；
- 非目标：本切片不实现 5.1F 的完整 Memory UI，不改变 LifeModel，不用 Memory
  授予权限，也不以检索命中证明任务完成或外部事实为真。

###### 5.1F 用户控制界面与原生验收

- 在现有 `/life-model` 产品区域中把“Agent 记忆”和“关于我 / LifeModel”呈现为
  两个平级对象，不让 Memory 看起来是 LifeModel 的附属数据库；
- 提供单条查看、来源、召回原因、纠正、停止召回、归档、恢复和隐私擦除动作；
- 读取、写入和终态都来自 backend ViewModel/receipt，不从前端计数推断已应用；
- 在同一隔离 QA profile 中完成多次会话、重启、降级和恢复验证。

退出标准：同一个真实项目能够跨多次会话和重启继续工作；用户能完成“记住、
召回、纠正、停止召回、归档、恢复、擦除”闭环；错误作用域、过期或冲突 Memory
不会冒充当前事实；工作记忆不需要塞入 LifeModel。

本切片实施边界（2026-08-06）：

- 保留规范路由 `/life-model`，把产品标签收敛为“个人智能”，以可访问的平级切换
  分别呈现“关于我 / LifeModel”和“Agent 记忆”；不新增路由或第二套设置页面；
- 两侧分别消费 `LifeModelViewModel` 与 `MemoryViewModel`。单侧读取失败不遮蔽另一侧，
  但 stale/error owner 对应的写动作必须关闭；创建 Memory Review 还要求当前
  `ReviewCenterViewModel` 可用；
- 单条 Memory 展示 canonical lifecycle/recall state、为什么记住、每回合如何参与
  召回及 backend-owned source refs；没有独立 receipt 时不得声称某次 prompt 实际
  使用了它；
- 纠正、停止召回、归档继续创建 Review proposal；恢复、回滚和隐私擦除必须核对
  exact owner 与 projection receipt，隐私擦除保留原生确认；
- 复用 5.1A—5.1E 已有跨重启、scope、降级、恢复和命令级失败反例，再增加少量
  用户旅程与真实 Tauri 验收；不建立新的验收平台或持久化测试账本；
- 非目标：本切片不开始 LifeModel v2 数据迁移，不新增学习候选系统，也不把
  Memory 控制动作解释为 LifeModel 更新。

##### 5.2 重建 LifeModel 核心

目标：让 LifeModel 只描述长期的用户，而不是 Agent Runtime、Memory、业务数据
或权限系统。

- schema 收敛到身份与自我定义、价值观、长期目标、稳定偏好、个人边界、重要
  关系、用户自身的长期能力与资源、决策原则和长期协作方式；
- 临时状态迁往 StateStore，每日任务和短期计划归 Tasks/State，程序性 Agent
  经验归 Agent Memory，Agent 工具能力和权限归各自所有者；
- LifeModel 中的长期目标只保存方向和用户确认的意义，不保存任务 progress、
  deadline 或 milestone；长期能力只保存用户确认的技能、专长和稳定资源，不保存
  Agent 工具能力、自动推断的 proficiency score 或资源的实时 availability；
- 重要关系属于高敏感字段，默认不从普通对话自动提取第三方画像；只有用户明确
  提出或审核具体 typed diff 后才能进入 canonical LifeModel；
- 未经用户确认的人格分数、模型推断和虚构默认值不得成为 canonical truth；空
  模型保持 empty/unknown；
- 以 SQLite 中的版本、父版本、摘要、来源关系和经过 schema 验证的结构化 JSON
  document 作为 canonical store；
- 每个文档具有 schema_version、model_version 和 parent_version；数组/集合元素
  使用稳定 ID，typed patch 同时校验 document base version 与目标字段/元素的
  before value 或 digest，避免无关字段变化造成虚假冲突；
- Patch 只开放 schema allowlist 内的 add、replace、remove 等受限操作，不提供任意
  JSON Patch 或自由路径写入；
- YAML 由指定 canonical version 确定性生成，只作为可读、可导出、可比较的
  projection；用户编辑 YAML 时先解析为结构化 diff，再进入 proposal；
- 迁移按“冻结旧 YAML 写入 -> 备份原文件与 digest -> 解析 v2 candidate -> 隔离
  无法自动分类字段 -> 写入新 store -> 从新版本生成 YAML -> 逐字段和 digest 对照
  -> 原子切换 read owner -> 拒绝旧写入口”执行；
- 切换后旧 YAML 只作为有期限的只读恢复备份，不参与正常读取或写入；验证完成后
  按明确清理条件删除，迁移期间允许两个物理副本但只有一个写权威；
- 提供版本、来源、新鲜度、冲突、删除和回滚能力，并同步修正仍依赖旧 4D
  Identity/Goals/Capabilities/State 分类的 Builder 与读模型。

退出标准：结构化 store 与 YAML 不存在双写权威；用户能看懂、导出、修改和
回滚自己的模型；State、Tasks、Memory 与 LifeModel 的职责没有重叠。

###### 5.2A 空模型语义与版本化 canonical owner

用户能力：新建或尚未建立 LifeModel 的用户看到 empty/unknown，不再获得虚构的
健康、情绪、能量或当前目标；后端开始能够读取经过 schema 验证的结构化 LifeModel
版本，并在已有 canonical version 时通过现有 `LifeModelViewModel` 展示版本与摘要。

- 当前入口：`LifeModelManager`、`get_life_model_view_model` 与既有 `/life-model`；
  新增的 SQLite version store 是 v2 结构化文档的唯一 owner，现有 YAML 在本切片
  仍是尚未迁移用户的兼容 owner；
- v2 文档只包含身份/自我定义、价值观、长期目标、稳定偏好、个人边界、重要关系、
  用户长期能力与资源、决策原则和长期协作方式；所有集合元素使用稳定 ID，并携带
  用户确认时间与最小 source ref；
- store 只接受 schema 验证通过、摘要匹配、精确 parent version/digest 的 append-only
  commit；同一 materialization identity 只能幂等重放同一内容；空 store 的读取不
  自动创建画像或迁移数据；
- 正常场景：空文档保持 empty；首个有效版本和后续精确父版本提交可跨 reopen 读取，
  同一文档生成确定性 JSON digest 与 YAML；
- 失败反例：旧/未知字段、短期进度/截止日期、无来源条目、重复稳定 ID、错误父版本、
  错误父摘要、同 identity 不同内容、被篡改 document/digest 全部 fail-closed；
- 本切片不迁移现有 YAML、不切换运行时读取、不开放 UI/YAML 编辑、不应用 proposal、
  不实现 rollback，也不建立自动学习或候选系统；
- 被替代路径：只移除新 profile 的虚构默认画像。旧 YAML manager、Builder 4D、旧 patch
  materializer 因尚未完成迁移继续保留并明确为后续替换对象。

退出标准：空模型无虚构事实；v2 schema/store 有真实 shipped read consumer；版本、
父版本、摘要、来源和重放边界由自动化证明；未发生真实用户数据迁移或静默写入。

###### 5.2B 旧 YAML 迁移预览与字段归属

用户能力：仍使用旧 YAML 的用户可以在“关于我”中看到一份只读迁移预览，明确每个
已有字段未来应进入 LifeModel v2、State/Tasks、Agent Memory、Agent Runtime，还是
必须由用户重新判断；预览不会把旧内容解释成已经确认的 v2 事实。

- 迁移预览从当前实际加载的旧 `LifeModel` 生成，并绑定源内容 digest；只读取，不创建
  v2 版本、不改写 YAML、不创建 proposal，也不切换 canonical owner；
- 对旧模型的每个非空叶子字段给出精确 source path、目标 owner、可选 v2 section、
  disposition、理由和敏感度；新增但尚未分类的旧字段必须使预览 fail-closed；
- 可明确映射的长期用户内容标记为 `review_required`，仍需用户确认后才能成为 v2
  item；短期目标、每日任务和当前状态转交 State/Tasks，reflection 与 evolution rule
  转交 Agent Memory，工具能力转交 Agent Runtime；
- 人格分数、proficiency、实时 availability、目标 progress/deadline/milestone 等不进入
  v2；关系内容按敏感信息展示并要求重新确认；含义不唯一的字段标记
  `manual_classification`，不得猜测目标；
- `/life-model` 只在存在旧 owner 且尚无 non-empty canonical v2 时展示完整预览和分类
  计数；已有 canonical v2 时不把旧 YAML 再包装成待迁移真相；
- 本切片不实现批量确认、typed diff、materialization、备份、原子 cutover、rollback 或
  删除旧 YAML；这些必须在后续切片沿用 Review Center 与 exact version gateway 完成。

退出标准：代表性旧模型的所有非空字段均被确定性分类；未知字段、非有限数值或无法
安全展示的值 fail-closed；用户能区分“可审核迁移”“属于其他 owner”“需要人工判断”
和“不会迁移”，且产品没有发生任何持久化变更。

##### 5.3 建立真实学习闭环

目标：从日常使用中稳定产生少量、高质量、可物化的 LifeModel proposal。

- 从对话、任务结果、用户纠正和反馈形成 Observation；
- Observation 和 Reflection 只是来源，不自动成为 Memory 或 LifeModel；
- Observation 默认只保存完成判断所需的最小摘要、source ref 和 metadata，不复制
  整段对话、文件或工具输出；
- 跨多次任务累计证据，并进行去重、冲突、稳定性、敏感度、明确陈述/推断和
  长期意义判断；
- Candidate 按 sensitivity、decision status 和 usefulness 设置有限保留期；被拒绝、
  放弃或长期未处理的候选到期清理正文，保留最小无内容审计事实；
- 不跨 Workspace 聚合候选；涉及第三方关系、健康、凭据或私密文件的内容默认不
  自动形成 proposal；
- 如候选提取需要外部 Provider，必须沿用当前 Provider privacy route 和用户配置的
  传输边界；没有允许的 Provider 或本地提取能力时宁可跳过，不静默外发原始数据；
- 不值得沉淀、来源不足或只反映短期状态的内容直接结束；
- 相似候选先累计和合并，再批量进入 Review，避免每次任务都产生 proposal；
- proposal 必须包含受支持的精确字段、typed before/after diff、来源、置信度、
  稳定性、敏感度、冲突和 base version；
- 用户可以修改、确认、拒绝或稍后处理；确认后仍需经过 materialization gateway
  才能创建新 LifeModel 版本；
- Heuristic learning 只作为候选提炼逻辑存在，不再维护独立通用 Maturation 或
  自进化平台。

退出标准：真实产品路径完成“使用 -> Observation -> 候选 -> proposal -> 人审 ->
版本化物化 -> 后续使用”，且没有静默长期画像写入和 proposal 疲劳。

##### 5.4 让 LifeModel 真正增强 Agent

目标：证明 LifeModel 不是静态档案或被动数据库。

- 将已确认且与任务相关的 LifeModel 编译为有界 runtime packet；
- 分别验证它对 planning、reasoning、context building、memory retrieval 排序、
  写作/沟通风格和已获准工具之间的 tool selection 优先级的影响；
- 当前用户指令、Policy、权限、凭据和效果确认始终优先；LifeModel 和 Memory
  均不能授予能力或授权持久/外部写入；
- 用户能够看到使用了哪条信息、来源版本、为什么相关、是否新鲜，以及它影响了
  哪个决定；
- 过期、冲突、来源不明或当前任务不相关的字段不进入正常决策；
- 使用相同真实任务比较无 LifeModel、过期/冲突 LifeModel 和已确认相关
  LifeModel，评估实际结果而不是只检查 prompt 中出现了字段。

退出标准：代表性对照任务证明受治理 LifeModel 对结果有可解释的正面帮助，同时
不改变权限和当前指令边界。

##### 5.5 贯穿式替换清理与最终收敛

目标：不再先做数月大清理，也不让已经被替代的历史平台继续增加维护成本。

- 5.5 的清理规则从 5.1A 开始贯穿 5.1 至 5.4：每个新 owner 或产品能力激活的
  同一切片中，追踪并删除已经被替代的旧写入、读取、command、bridge、兼容和
  迁移路径；不得等到 5.4 完成后才一次性清理；
- 5.5 作为独立板块时只做最终 caller、数据和 authority 收敛，不重新实现已经完成
  的替代能力；
- 重点检查 HSAssetAuthorityRegistry、独立 Maturation Engine、runtime canonical
  RegressionSuite、通用 Heuristic 平台、Calibration/Micro Evolution 历史入口和
  无生产调用者的 Tauri command；
- EvidenceStore 中仍有价值的来源、证据与冲突概念可以迁入 LifeModel 学习桥梁，
  但不能继续作为广义 HS 平台；
- 触及到的巨型文件只按已经建立的领域 owner 拆分，不为追求文件大小而机械拆分；
- 删除前用真实 caller、数据迁移和恢复测试证明替代已经完成；不因为名称像历史
  模块就直接删除生产代码。

退出标准：阶段五主路径不存在并行旧权威；保留的历史兼容均有真实消费者、明确
退出条件和测试，未使用路径从源码而不是仅从索引中消失。

##### 5.6 原生闭环验收

最后用真实 Tauri 和隔离 QA profile 完成：

- 多次会话和跨重启的 Agent Memory；
- Markdown Memory 查看、受治理编辑、遗忘和恢复；
- 真实任务产生候选，用户修改并确认 LifeModel proposal；
- 结构化版本物化和确定性 YAML 更新；
- 后续任务真实使用并解释使用原因；
- 冲突、过期、拒绝、回滚和删除；
- LifeModel 或增强 Memory 故障时，健康的普通 Agent 仍可继续工作；
- must-not-recall 和跨 Workspace/Project 泄漏场景为零；所有进入上下文的 Memory
  与 LifeModel fact 均有真实 source ref；
- 隐私擦除后正文不再存在于 canonical、FTS、Vector、cache、YAML projection 和
  runtime context；
- 长上下文压缩保留黄金场景中的目标、约束、未决 Review 与关键证据引用；
- 同一任务的 LifeModel A/B 人工 rubric 至少检查正确性、个性化相关性、当前指令
  遵循和权限边界，不以“字段出现在 prompt”代替效果改善；
- 5.1E 固定的 context budget 与检索延迟目标在同一 QA 机器上满足，任何超出保持
  可测量、可解释而不是通过扩大超时掩盖；
- 只在具备产品意义的候选构建上做原生验收，不因每次小修改重复建立新的 finalN
  profile、反复初始化同类凭据或把人工授权过程当作开发成果。

退出标准：第五阶段完整黄金路径在同一精确构建和隔离 QA 中跨重启成立，底层
数据库、产品读模型和用户界面相互一致；自动化、本地原生和 external-live 证据
等级保持分离。

#### 小板块执行规则

5.1 至 5.6 是固定顺序的大板块，每个大板块继续拆成可独立交付的小垂直切片。
每个小切片开始前必须在现有 Program 或当期唯一 Markdown 实施计划中写清：

1. 用户会获得的具体产品能力；
2. 当前源码入口、唯一 owner 和权威存储；
3. 输入、输出、持久写入和权限边界；
4. 本切片明确不做什么；
5. 至少一个正常场景和多个关键失败反例；
6. 用户在哪里查看、纠正、撤销或恢复结果；
7. 同一切片将删除哪些已被替代的旧 caller，以及哪些历史路径因尚无替代而继续
   保留；
8. 退出标准对应的自动化、原生和 external-live 证据等级。

实现顺序固定为“源码与现状核对 -> 失败反例 -> 最小产品实现 -> focused tests ->
比例适当的全量门禁 -> 自审 -> 必要的真实 UI/原生验证 -> 停在审阅边界”。测试
优先验证产品行为，不增加计划账本、任务包、审批文本验证器、行数检查或大型治理
JSON。不得为了完成某个小切片而提前建设下一板块的平台能力。

#### 非目标

- 不自主修改 OpenLife 源码；
- 不做仓库自进化；
- 现阶段不建设 LifeModel Coding、任意代码生成执行或通用规则编程系统；未来如
  要加入，只能另行提出、研究和审批，不由阶段五当前实现预留大型平台；
- 不把会话、Workspace、工具日志和全部 Agent Memory 塞入 LifeModel；
- 不让 LifeModel 或 Memory 授予工具权限、凭据或外部写入许可；
- 不因为一次行为或一次推断就修改长期画像；
- 不自动把模型推断提升为用户事实；
- 不隐藏 profiling；
- 不建设通用学习平台；
- 不把 proposal 接受当作持久应用完成。

#### 退出标准

- 用户能够分别看见并控制 Agent Memory 与 LifeModel，理解二者职责；
- 从日常使用到候选证据、proposal、用户确认、物化、新版本、YAML 表达和
  runtime 使用的完整闭环通过当前产品区域完成；
- 敏感信息或推断不能静默成为 canonical truth；
- YAML 与结构化存储没有可独立漂移的双写权威，导出、用户编辑、冲突和回滚
  均走受治理路径；
- 后续对话和任务能够证明受到受治理 LifeModel 的正面帮助，并能解释使用了
  哪些长期信息；
- 过期或冲突内容保持可见且可恢复；
- 删除和回滚会从活跃上下文移除对应信息；
- 内部 dogfood 证明个性化有用且没有夺走用户控制权。

### 第六阶段：内部产品完善与源码试用

#### 阶段目标

仓库中的 OpenLife 达到稳定、清晰、可复现的内部长期使用水平；具备技术
能力的用户可以从 GitHub 克隆源码并进行真实体验。

#### 主要范围

- 对已完成产品闭环进行长期内部 dogfood；
- 启动、重启、恢复和长时间运行稳定性；
- 隔离 QA profile 与真实用户数据；
- 性能、响应速度和资源边界；
- 与源码试用相适应的备份、导出、导入、删除和恢复；
- 可访问性、键盘操作、中文产品语言和错误恢复；
- 准确的 README、环境模板、依赖要求和故障排查；
- 可复现的 clone、setup、build、test 和 `make dev`；
- 明确支持范围内的 Provider 兼容性；
- 修复内部真实使用发现的问题。

#### 标准源码试用路径

```text
访问 GitHub
  -> 克隆仓库
  -> 安装明确列出的依赖
  -> 配置隔离环境
  -> 通过唯一入口启动 OpenLife
  -> 走通黄金路径
  -> 体验 Agent、行动与个人智能能力
```

#### 明确延期

- 面向公众的 macOS 安装包；
- App Store；
- 面向公众发行的签名和公证；
- 自动更新；
- 公共二进制发行基础设施；
- 商业发布流程；
- 大规模公众 Beta。

只有源码试用达到内部完善后，未来才可以另行提出并审批产品发行计划。

#### 退出标准

- 全新 clone 无需口头知识即可按文档完成源码试用；
- 内部能够持续使用且没有未解决的 P0/P1 产品故障；
- 已完成功能能够跨重启并承受代表性失败；
- 产品文档与当前源码及可见行为一致；
- 已知限制和未支持证据等级明确；
- 备份、恢复和危险操作保持用户控制；
- 性能和可用性达到双方确认的内部质量标准；
- OpenLife 在当前源码产品范围内达到内部完整状态。

## 7. 当前阶段

当前阶段：**第五阶段——LifeModel 与 Memory 个人智能闭环已开始。前置“架构
边界校准”于 2026-08-06 完成；Agent Memory 四层边界、LifeModel v2 范围、
versioned JSON + SQLite canonical store 与 YAML projection 关系已经用户确认。
5.1A“跨重启继续当前会话”已于提交 `d77e38f` 完成并经用户确认；5.1B“摘要与
长上下文压缩”已于提交 `a21d9f4` 完成并经用户确认。5.1C“Workspace/Project
Markdown Memory”已于提交 `b88e879` 完成并经用户确认。5.1D“显式跨会话 Memory
生命周期”已于提交 `177b144` 完成并经用户确认。5.1E“混合检索与召回解释”已经
于提交 `7110e99` 完成并经用户确认。5.1F“用户控制界面与原生验收”已于提交
`653693b` 完成并经用户确认。5.2A“空模型语义与版本化 canonical owner”已于提交
`3a6c2bb` 完成并经用户确认。5.2B“旧 YAML 迁移预览与字段归属”已完成实现、
本地自审和全量门禁，当前停在用户审阅与提交边界；尚未开始真实数据迁移、v2
proposal materialization 或 canonical owner cutover。**

第五阶段第一步实际完成：

- ADR 0016 取代 ADR 0013 的广义 LifeModel-HS 方向，固定 Agent Runtime、Agent
  Memory、LifeModel、业务域事实、安全与治理五个所有权边界；
- LifeModel、学习和增强检索的可选存储故障不再自动关闭健康的基础 Agent，具体
  缺失能力仍由精确读写网关 fail-closed；启动 reconciliation 与多 owner recovery
  继续保持保守；
- 缺失 MemoryLifecycleStore 时，Main Chat 使用显式 degraded context marker，而
  不是中止基础上下文编译或把空结果冒充健康；
- 程序性未来规则归入 Agent Memory proposal 候选，不再写入 LifeModel；
- Main Chat 只为已支持的精确 LifeModel 字段和值创建 proposal。通用请求
  返回 `lifemodel_typed_diff_required`，不再生成无法物化的
  `lifemodel.pending.chat_conversation` 占位记录；Markdown 编辑则进入受治理的
  file-write proposal 路径，不再被冒充为 LifeModel 变更；
- 本切片没有实现完整 Agent Memory、没有重构完整 LifeModel schema、没有进入
  自动候选证据与长期学习闭环。

第四阶段实际完成：

- 任务所需的 Markdown 工作记忆、会话/Workspace 上下文和 Reflection 保持
  有界、可追溯且不冒充业务事实；任务相关 LifeModel runtime packet v1 只读取
  已确认且相关的长期上下文，并且不能授予工具权限；
- shipped runtime 支持 proposal-first 的文件创建、修改、移动、回收和恢复，
  本地计划任务及可选 ICS 投影，邮件草稿与浏览器的 OS handoff，以及精确
  allowlist 内的只读本地工具；邮件未获得发送或送达信用，浏览器未获得页面
  加载信用，日历未获得远端 connector 信用；
- 文件创建和覆盖 proposal 精确绑定审核时的目标不存在状态或内容摘要；目标在
  审核后发生变化时 fail-closed，新文件通过不覆盖既有目标的原子链接提交；ICS
  投影复用同一 staged/commit 路径，不再直接把可能不完整的内容写到最终文件；
- LifeModel runtime packet 只接收具有非空版本和合法 RFC3339 更新时间的来源；
  同一邮件任务的确定性有/无 LifeModel A/B 集成测试已证明相关长期沟通偏好会
  改变最终回答，并保留来源元数据。该测试是产品路径证据，不是外部模型质量
  或 external-live 证据；
- 每类行动都通过 capability、operation、risk、confirmation、effect boundary、
  terminal evidence 和精确 action identity 进入 Review Center；批准、dispatch、
  materialization、unknown、failed、rejected 和 rolled-back 不再混为同一状态；
- action receipt、幂等恢复、取消、拒绝和启动 reconciliation 已接入持久任务
  事实；修复了高风险拒绝后 TaskSession 仍等待、启动时先恢复 proposal 后恢复
  task，以及 move/trash/restore 未同时校验源和目标安全路径的问题；
- 当前全量 Rust（Core 1511 通过/2 条件忽略，Tauri 1141 通过/13 条件忽略）、
  严格 Clippy、前端格式/typecheck/Vitest/build、absence guard 和 browser shell
  门禁均通过；没有调用 Provider 或外部网络来替代行动证据；
- 自动化测试覆盖取消竞争、超时终态、同 operation replay 去重、staged/final
  artifact 重启恢复和并发目标变化；同 operation ingress replay 的防重复信用来自
  自动化产品测试，不能用启动 reconciliation 的无重复结果冒充；
- 精确构建
  `5f4d6c72675f0fe67b62efd5d2577a3dfdd490b35d150905c23392ade916f5cc`
  已在全新隔离数据目录完成 5 类系统凭据初始化，关闭安全模式，并真实渲染
  `/today` 与 `/settings`；原生取消一个已 sealed、等待文件写入 Review 的任务时，
  复现了普通 open-turn 写通道被终态 fence 拒绝的真实缺陷。数据库证明取消事件
  已持久化、目标文件未生成，但 AgentRun/Task 投影仍为 waiting_permission；
- 该缺陷已有修复前同构失败反例。修复没有放宽 sealed fence，而是把取消仍未
  dispatch 的阻断 Review 映射为既有 ReviewWorkflow rejection successor，使
  Proposal、TaskSession、AgentRun 和 action queue 一起终态收敛，不留下可继续
  批准的悬空 Proposal。修复后的精确构建
  `02117e78573f44b9cc8b9d167f2366cf329cdb71ab2c1ac2c50964ff2f228f20`
  在隔离 QA 中完成 5 类凭据初始化和原生复核：取消 sealed 文件写入任务后，
  Proposal 为 rejected/unclaimed，TaskSession 与 AgentRun 为 cancelled，目标文件
  不存在，重启后仍保持同一终态且没有新增 terminal successor；
- `02117e78` 的原生超时试用进一步暴露了产品层错误分类：ToolGateway 已返回
  `tool_gateway_timeout`，但 Kernel 稳定错误码白名单遗漏该值，导致界面退回低层
  `tool_locally_aborted` transport truth。修复前失败反例先证明该遗漏，随后仅把
  `tool_gateway_timeout` 映射为产品稳定码 `timeout`，没有改写 action receipt 的
  本地中止事实；
- 修复后的精确构建
  `55ffd1b37452da8a3b4dcf632fc5f752588002a413c83ac02b16d43eb902aa66`
  已恢复同一隔离 QA 的 5 类既有凭据并关闭安全模式。真实 `file.read` FIFO 任务
  在 120 秒边界失败，界面与最终交付均显示 `timeout`；对应 run 只有一次 dispatch
  prepared、一次 tool started、一次低层 local-aborted receipt 和一次 failed final
  delivery。删除临时 FIFO 并重启同一构建后，任务仍为失败，run、事件、派发与
  最终交付数量均未增加。该试用没有调用外部 Provider 或网络；
- 精确构建
  `a6d6b8091e4a3f9fef33c72c1893ff9fcf73f103950e510a1d9198f6769a7dca`
  在全新隔离 QA 中完成原生成功、拒绝和回滚验证：浏览器动作拒绝后未打开
  URL；文件在批准前不存在，批准后以匹配摘要落盘；随后经独立审核移入
  OpenLife 恢复区，原路径消失且 41-byte 内容摘要保持
  `1392b76ef08c6a12669eebbf48381bfeb554d5c1f5596f317ce54c3df3ed03cd`；
  再次重启后安全模式保持关闭，Review receipt 与 completed/cancelled 任务终态
  继续存在且没有悬挂任务。
- 阶段收口代码 Review 发现启动 reconciliation 曾把本地 `ScheduledTask` 与无法
  检查真实副作用的 OS handoff 一并封存为 unknown：如果任务已创建但 Proposal
  receipt 尚未确认便崩溃，会丢失可由本地规范记录证明的效果。修复先以失败反例
  复现，再只在 `source_proposal_id`、来源 run、内容、日期、优先级、动作类型和
  `LocalOnly` route 全部精确匹配时，把 claimed action 收敛为待投影确认；重复或
  不匹配记录继续 fail-closed，浏览器、邮件和云端动作仍保持 unknown 且不会重放。
- 首次远端 Linux Review 门禁还暴露了 Resource citation 的随机隐私碰撞：旧的
  24 位十六进制 token 偶尔包含 18 位连续数字，被默认隐私规则当作身份证号
  block，导致 Provider 看不到可回传的原始 citation。新 token 保持 96-bit
  request-scoped 绑定和固定长度，但用 `a`–`p` 字母编码，从构造上避开号码类
  PII；历史十六进制 citation 仍可读取。1024 组生成回归、真实碰撞反例、引用
  输出契约和混合 Resource/Web 产品测试共同覆盖该边界。

第三阶段完成的产品切片：

1. **3A——本地文件和资源读取**：用户通过原生选择器明确选择资源；资源与
   精确 operation identity 绑定；来源、移除、失败和重启恢复在产品中可见。
2. **3B——live web 研究和引用**：自然语言请求进入真实受治理网页读取；
   一次性工具许可和精确端点许可分别审核；外部内容保持未背书并绑定引用。
3. **3C——文件和 artifact 生成**：有界内容先形成 proposal，Review Center
   批准后才由后端安全路径物化；产品区分批准与真实落盘。
4. **3D——已注册 MCP 读取**：只暴露后端当前注册并分类为只读的工具；调用、
   MCP 审计、来源、失败和超时保持可核验，manifest 文本不授予权限。
5. **3E——混合能力多步骤任务**：同一任务组合 Web、MCP 和 external-live
   Provider；暂停、逐层授权、恢复和最终合成绑定精确 task/action identity。

最终原生验收（2026-08-03）使用隔离 final19 profile。原始任务暴露了两处真实
恢复缺陷：ActionResume proposal 被通用 Review 投影提前标记为完成，以及启动
修复 AgentRun 时无条件重写正确 TaskSession、破坏 owner revision。两者均先以
失败反例复现，再修正为由 canonical TaskSession 和幂等启动投影保持真实状态；
旧任务保留失败证据，没有被改写或计为成功。

修正版替代任务 `21c05c3d-1299-4f77-ae9d-779414fc736c` 通过 shipped
`OpenLife.app` 完成真实混合路径：`https://example.com/` 网络读取和
`builtin_echo` MCP 读取各 dispatch 一次并形成成功回执；DeepSeek
`deepseek-v4-flash` 形成 `provider.started` 和 `provider.completed`；最终交付
同时证明 `modelInvoked=true`、`providerInvocationStatus=completed`、
`bodyStored=true`、`toolInvoked=true`、`blockerCount=0`。持久回答分别总结了
Example Domain 与 MCP 观察，并保留未背书 Web 引用。mock、fixture、本地
Ollama 和旧 final11 失败试用均未替代这次 external-live 信用。

第三阶段没有新增平行编排平台、计划 JSON、能力 ledger 或自进化系统。第四
阶段已经在用户批准后开始；不得把计划、测试或治理代码本身计为行动能力。

短收口完成（2026-08-04）：CSV spreadsheet formula injection 已在生成阶段
fail-closed；会话技能选择已从进程内状态迁移到现有 `MemoryStore` 会话所有者，
并验证跨重启恢复；工作区会明确区分当前对话任务和属于其他对话的全局活动任务。
签名原生构建 `18bd829d148639b9a1c81f247ce179f6295e251e37a334d28ca4121c43a1a0d3`
在隔离 QA 中完成凭据恢复、真实 DeepSeek Web 搜索、Provider 生成和
proposal-first artifact 落盘。原生证据同时发现 terminal-owner 在 Provider replay
后使用旧 final-event identity 的缺陷；修复后，重启 reconciliation 将同一任务收敛
为 completed，且 artifact 的 SHA-256、inode、mtime 和大小均未改变，因此没有把
重复网络调用或重复写入计为成功。LifeModel mutation journal 与 memory 表保持为
零。全量 Rust、前端、production build 和 browser smoke 门禁均通过。

第一阶段实际完成：

- 产品前端 bridge 与 shipped Tauri handler 已收敛为一一对应；
- 没有当前产品调用者的退役 command 和 `tauriDev` bridge 已从 shipped
  surface 移除；
- 未受原生确认保护的 whole-model LifeModel 保存入口已从产品接口移除，
  LifeModel 变更继续使用 proposal-first review；
- macOS 启动凭据读取改为单次查询的非交互策略，不再修改进程级 Keychain
  交互状态；用户发起的 Settings 凭据流程保持原生交互能力；
- 全量 Rust、前端、coverage、production build 和 browser smoke 门禁通过；
- 打包后的真实 `OpenLife.app` 可以启动并进入 `/settings`，没有崩溃或
  非预期凭据弹窗。

第一阶段没有调用外部 Provider，也没有写入产品数据或既有凭据。原生
Keychain round-trip 只创建并清理了随机隔离测试项；browser、mock 和本地
Keychain 证据均不作为 external-live 证明。

第二阶段实际完成：

- Workspace 对话已接入 shipped streaming TurnRuntime，按精确 session、operation
  和 task identity 接收事件，并支持真实流式显示与精确取消；
- 全新隔离 QA profile 已完成系统凭据初始化、同一签名 App 重启、Provider
  配置、显式连接测试、保存、首次对话、取消、同会话重试、重命名、历史恢复
  和确认删除；
- Provider 的 configured、validated、failed、stale 和 unknown 边界继续由后端
  读模型区分；连接测试成功不再被描述成必然云端可用；
- Review Center 的一次性网络许可在真实请求前消费；批准请求与实际完成继续
  分离，LifeModel proposal 的 approved-not-applied 状态由产品测试保持；
- Provider 一次性许可批准后，同一 Main Chat task 会得到后端允许的恢复控制；
  恢复后只消费一次精确许可并形成真实 completed 终态；
- 取消请求的迟到失败不能再覆盖已经返回的真实流终态，且有失败反例保护；
- Proposal 接受 IPC 明确区分 confirmed 与 deferred 结果，不再因合法的
  terminal-owner 响应缺少 LifeModel patch 而误报前端失败；
- 同一签名构建重启后，系统凭据、Provider 配置、对话历史、任务终态和本地
  直连能力均保持可用；重启后新的本机回合真实返回，且显示本地路由、未外传；
- 全量 Rust、前端、coverage、production build、browser smoke 和最终原生重启
  门禁通过，没有以测试 fixture 或旧签名 QA profile 代替当前源码结论。

本阶段使用的真实生成来自本机 Ollama，不能计为 external-live Provider 证据。
第三阶段后续已经使用隔离 DeepSeek 配置补足 external-live Provider 证据；第四
阶段已经完成，第五和第六阶段尚未开始。

## 8. 进度记录规则

- 阶段开始或关闭时，只更新本文件的“当前阶段”；
- 本文件最多保留当前已批准切片及其验收边界；
- 具体实现讨论使用正常 GitHub Issue 和 PR；
- 测试详情进入 CI 或 PR，不建立永久证据注册表；
- 只有真正长期的架构决定才新增 ADR；
- 临时计划被替代后直接删除，不在工作树积累第二套历史；
- 不以计划文件、validator、测试数量或治理代码增长衡量成功。

本计划唯一的成功标准是：OpenLife 变得更有用、更强大、更安全，也更
容易真实使用。
