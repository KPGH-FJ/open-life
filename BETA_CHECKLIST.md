# OpenLife Beta 发布检查清单

## 核心功能验证

### Agent Runtime
- [x] AgentLoop 多步执行正常（max_steps=4, max_tool_calls=6）
- [x] AgentLoop 状态事件流式传输
- [x] AgentRun 完整追踪（status/step_count/tool_call_count）
- [x] AgentLoop fallback 到旧路径正常
- [x] WaitingPermission 状态处理

### Tool Execution
- [x] Core OS Tools 真实可执行（life_model.read, goal.read, memory.search 等）
- [x] ToolCallCard 显示权限决策、风险等级、结果、错误
- [x] Replay 功能可用
- [x] P1/P2 工具分类正确

### Permission & Governance
- [x] 权限策略配置面板（Privacy tab）
- [x] Tool Registry 显示可执行/声明-only/禁用状态
- [x] Proposal 统一确认流（Builder/Chat/Calibration/Memory）
- [x] Snapshot 自动创建

### Memory
- [x] 显式记忆提取（"记住这个"）
- [x] 隐式记忆建议（自动检测）
- [x] 异步 Embedding（不阻塞 Chat）
- [x] 向量记忆 tier 维护

### Model Router
- [x] 本地/云端智能路由
- [x] Provider health 检查
- [x] Privacy 优先路由
- [x] Fallback 机制

### Network Policy
- [x] 域名白名单/黑名单配置
- [x] 默认决策（ask/allow/deny）
- [x] 工具级覆盖
- [x] Web 工具执行前检查

## UI/UX

### 导航
- [x] 主导航收敛为 Workspace/Agent/Review/Runs/Settings
- [x] Settings 分标签页（Overview/Provider/Privacy/Data/Plugins）
- [x] Chat 网络状态指示器

### Chat
- [x] 流式输出
- [x] AgentStateIndicator 实时状态
- [x] 执行摘要行（模型/工具/提案/fallback）
- [x] 快捷指令（/goal, /state, 重试）
- [x] 反馈按钮（👍/👎）

### Workspace
- [x] 系统状态 Banner
- [x] 待处理 Proposal 数
- [x] 今日/累计 Agent Run 数
- [x] 会话数、记忆块、反馈统计
- [x] 内置 Skills 快捷入口

### Runs Detail
- [x] 统计摘要（步数/工具/Actions/Observations）
- [x] 持续时间
- [x] 状态时间线
- [x] 模型路由详情
- [x] 生成的提案列表

### Settings
- [x] Provider 配置（DeepSeek/OpenAI/Ollama）
- [x] Chat Proposal 设置
- [x] 实验性功能（AgentLoop toggle）
- [x] Safe Paths 管理
- [x] 网络策略配置
- [x] 隐私策略配置
- [x] 工具权限管理
- [x] 数据导出/导入
- [x] 诊断报告导出
- [x] 系统维护（记忆层级、向量重建）

## 质量门控

### 测试
- [x] 前端测试 214 passed
- [x] Rust 测试 799 passed (737 core + 62 tauri)
- [x] `make ci` 全部通过
- [x] 前端生产构建成功

### 代码质量
- [x] cargo fmt 检查通过
- [x] cargo clippy 零警告
- [x] Prettier 格式化检查通过
- [x] TypeScript 类型检查通过

### 文档
- [x] README 更新 Beta 功能列表
- [x] AGENTS.md 开发指南更新
- [x] 架构文档同步

## 首次体验

- [x] OnboardingWizard 引导流程
- [x] 试用路径 Checklist（Settings Overview）
- [x] Beta 闭环定义（4 步）
- [x] Safe Mode 降级处理
- [x] 诊断状态实时显示

## 发布准备

- [x] 版本号确认（package.json, Cargo.toml）
- [x] 数据库迁移兼容性
- [x] 数据目录统一（ai.openlife.desktop）
- [x] 环境变量模板更新

## 已知限制（Beta 阶段）

1. **AgentLoop Streaming**: 当前为句子级分块，非真实 token 流
2. **Tool Execution**: email.read 为 declarative-only stub (需IMAP); calendar/email/task 其余工具已升级为 P1
3. **Network Policy**: 仅覆盖 web.fetch/web.search，不影响其他工具
4. **Memory**: 异步 embedding 在首次写入后延迟生成
5. **Skill Runtime**: 仅内置 3 个 Skill，不支持外部 Skill 注册
6. **Universal Binary**: 当前仅 aarch64，x86_64 target 需安装
7. **Code Signing**: macOS 未签名公证，需手动允许运行

## 下一步（Post-Beta）

参考 [`plans/openlife_post_beta_roadmap.md`](plans/openlife_post_beta_roadmap.md)：

- [ ] 执行路径收敛 (lib.rs 瘦身, ExecutionFacade)
- [ ] PromptStack 全路径审计
- [ ] LifeModel Evolution 管线端到端闭环
- [ ] Universal Binary (x86_64 + aarch64)
- [ ] macOS 代码签名与公证
- [ ] Windows/Linux 跨平台验证
- [ ] ChatPage 重构 (解锁 ADR 0010)
- [ ] 外部 Skill 注册和 Marketplace
- [ ] 性能优化（启动速度、内存占用）
