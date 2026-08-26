# OpenLife 产品重建计划

状态：唯一活动计划
更新：2026-08-26
目标：把已有 Agent 能力重建为清晰、可靠、可长期自用的桌面产品。

历史进展、测试计数、构建摘要和阶段记录保留在 Git，不追加到本文件。

## 1. 当前判断

OpenLife 已有一条 canonical Chat/Work runtime、持久化 Provider Connection / Model
Profile、真实 Project 文件范围、Task/Run/Artifact、Review、Result、来源、修订和 Undo。
当前瓶颈不是继续扩展后端能力，而是生产 UI 没有忠实落地已接受的产品体验合同。

现状定义：

- 后端与 Agent harness：工程 Alpha，Slice 2 能力已经深入；
- 产品体验：生产 Shell 重建未完成，不是内部 Beta；
- 发布目标：仅内部自用，暂缓 Developer ID、公证、公共更新和对外发布；
- 交付事实：仍以用户实际安装并启动的 OpenLife 应用为准，浏览器壳、fixture 和
  源码测试不能替代真实应用验收。

## 2. 冻结边界

生产 UI 重建完成前：

- 不增加新的 Agent 能力、工具平台或产品一级概念；
- 只允许修复 P0/P1 正确性、安全、数据真实性和重建所需的窄后端契约；
- 不引入 multi-agent、Computer Use、schedule/automation、connector/plugin 平台；
- 不恢复退役页面、旧 IPC、旧 runtime、兼容 fallback 或第二套 Shell；
- 不改变 LifeModel 的独立边界，不让它成为普通任务执行前提。

## 3. 固定产品结构

权威体验合同：`docs/architecture/product-experience-contract.md`。

生产桌面只保留三层：

1. 左侧：New chat、Open folder、Projects、Conversations、History、Settings；
2. 中央：一个 Conversation、紧凑 Work 状态、steering 和一个 composer；
3. 右侧：按需打开的 Preview、Diff、Review、Result、来源和技术详情。

具体约束：

- 右侧上下文面板默认关闭；
- Task、Run、Attempt、Activity 和 Evidence 是后端事实，不是并行主导航；
- 当前 Work 只在对话中展示“正在处理 / 需要决定 / 失败 / 已完成”等用户语言；
- 全局工作状态进入 History 或紧凑的 Needs Attention 入口；
- 不常驻 Global Activity、Results 控制台、完整计划、长时间线或技术收据；
- 高级 Project、Memory、工具、权限和诊断按需展开；
- 首屏必须明显提供 New chat、Open folder 和最近 Project/Conversation。

## 4. 数据与清理规则

- 保留凭据、用户外部文件、Projects、Conversations、Tasks/Runs、Artifacts、必要的
  Memory/LifeModel 和恢复数据；
- 新路径接管一条旅程后，立即删除对应旧组件、CSS、路由、测试 fixture 和 fallback；
- 不用隐藏、折叠或新包装器冒充删除；
- production absence 检查必须同时覆盖源码 import graph 和构建产物中的旧界面标记；
- 不创建额外 checkout、worktree、治理平台或机器可读开发程序。

## 5. 实施阶段

### U0 — 状态真实性与计划收口

- 区分“后端变更已提交”和“依赖读模型刷新失败”；
- 修复模型选择器默认隐藏已就绪远端模型；
- 把本文件压缩为当前执行合同；
- 为旧 UI 建立明确的 production absence 清单。

退出条件：已提交的 Project 范围变更不再被误报为未发生；当前计划没有追加式日志；
旧 UI 删除目标有真实生产消费者映射。

### U1 — 唯一生产 Shell 与导航

- 左栏承担 New chat、Open folder、Projects、Conversations、History、Settings；
- 删除 Workbench/Personal Intelligence 两项式图标栏和内嵌 Conversation 管理栏；
- 标题显示当前 Conversation 或 Project，而不是抽象“工作区”；
- 1440×900、1024×768、窄窗口和等效 200% 缩放保持单一主层级。

退出条件：首屏三个主要动作明显；不存在第二套导航、横向五栏或重复任务树。

### U2 — 单一 Conversation 与 Composer

- Chat/Work 共用一个线程和 composer；
- 模式、模型、Project/资源和 send/stop 保持可见但紧凑；
- Project 生命周期、Memory、Skill、工具和授权进入渐进披露；
- planning、streaming、steering、stop、等待决定、失败、恢复和完成在同一线程表达。

退出条件：用户无需理解 Task/Run/Activity 就能开始、跟进和改变一项工作。

### U3 — 上下文结果与决定

- Result、Artifact、Preview、Diff、Review、来源和失败详情进入右侧上下文面板；
- 对话只显示一个 canonical Result 摘要和直接动作；
- 多文件 Review、Undo、修订和来源仍绑定后端真实 Task/Run/Artifact；
- 技术收据和证据 ID 只在二级详情展示。

退出条件：结果可检查、可继续、可恢复，但主线程不再成为任务控制台。

### U4 — History、Settings 与 Personal Intelligence

- History 统一承载进行中、需要处理、最近完成和搜索；
- Settings 优先展示 Connection/Profile、隐私、数据与恢复，诊断进入高级区域；
- Agent Memory 与 LifeModel 保持两个独立、可选的个人智能区域；
- 普通 Chat/Work 不暴露无关个人智能写入或内部状态码。

退出条件：历史没有与 Conversation/Result 重复的导航树；Settings 和 Personal
Intelligence 不再像调试控制台。

### U5 — 旧前端删除与内部原生验收

- 删除 GlobalActivity 常驻面、嵌入式 Results 主列、旧 Conversation 管理栏及其 CSS；
- 删除被替代的选择器、标记、测试断言和构建残留；
- 完成前端 typecheck、tests、format、production build 和 production absence；
- 运行相关 Rust 检查；
- 构建并安装内部 OpenLife，核对提交、Bundle ID、可执行摘要和安装路径；
- 用真实 Provider、Project 和文件完成核心旅程截图与恢复验证。

退出条件：用户打开的实际 OpenLife 只包含新产品结构；旧 UI 在源码和构建产物中均
不存在；没有开放 P0/P1，且核心自用旅程可以自然完成。

## 6. 当前执行顺序

当前阶段：U0。
下一阶段：U1。
U1–U5 完成前，不恢复 Slice 3–5 的新能力扩展。

每次提交只关闭一个可复核的用户旅程或删除边界。工程测试通过后仍需在真实内部应用
中复核；如果源码、测试与用户实际界面冲突，以实际界面为失败事实。
