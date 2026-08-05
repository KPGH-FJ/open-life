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

- 明确区分 Agent Memory 和 LifeModel，并分别提供查看、编辑、归档、恢复和
  忘记能力；
- 从真实交互、任务和反馈中提取受治理候选证据，去重、累计、批处理并避免
  每个任务都产生 proposal；
- 重新收敛 LifeModel schema：身份、价值观、长期目标、稳定偏好、个人边界、
  重要关系、长期协作方式和决策原则；每日任务、工具输出和原始对话不进入
  LifeModel 主体；
- 区分用户明确陈述、生活事件、短期状态、模型推断和已确认长期模型；
- proposal-backed、版本化的 LifeModel 演进，proposal 包含来源、置信度、
  稳定性、敏感度、冲突和 before/after diff；
- 确立结构化存储与 YAML 的单一权威关系：结构化资产负责事务、版本、证据和
  回滚，YAML 作为确定性的人类可读、可导出表达；用户编辑 YAML 进入同一
  proposal 路径；
- 将完整 LifeModel 编译为任务相关 runtime packet，并分别验证它对 planning、
  reasoning、context building、memory retrieval 和 tool selection 的实际影响；
- 来源、使用原因、影响过的决策和当前新鲜度对用户可见；
- revision、conflict、stale、materialization 和 rollback；
- 隐私与敏感度控制；
- 建立受治理、可测试、可版本化的协作规则网络。AI 可以生成规则 diff 和行为
  检查建议，但只有用户审核后才能激活；规则不得成为任意源代码执行入口；
- 使用相同真实任务比较无 LifeModel、过期/冲突 LifeModel 和已确认相关
  LifeModel，验证个人模型是否真实改善结果。

#### 非目标

- 不自主修改 OpenLife 源码；
- 不做仓库自进化；
- 不让运行时 LifeModel 生成或执行未经审核的任意代码；
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
- 协作规则具有版本、来源、适用范围、冲突处理和行为检查；
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

当前阶段：**第四阶段——受治理的行动型 Agent 已完成（2026-08-05）。第五阶段
尚未开始。当前停在第四阶段交付与审阅边界；未经下一阶段单独审阅和确认，不进入
LifeModel 与 Memory 个人智能闭环开发。**

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
