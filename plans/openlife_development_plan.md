# OpenLife v1 主开发计划

> 基于 PRD v3.0 与当前代码基线的差距分析生成
> 当前定位：Alpha 收口阶段主计划
> 计划时间：约 8-10 周高强度迭代

---

## 0. 使用说明

本文件从“剩余需求罗列”调整为 **当前阶段唯一主计划**，使用规则如下：

1. 先执行本文件，再回看大 PRD
2. 新需求默认只能挂到本文件已有 Phase 中，不能直接插入大 PRD 主路线
3. 每一轮 Codex 开发都要围绕“核心闭环是否更完整”来判断优先级

当前阶段的执行原则：

- **先收口，后扩张**：优先修复一致性、契约、持久化、审批流，再扩新功能
- **先主链，后侧链**：先做对话、人生模型、校准、记忆，后做生态与联邦能力
- **先可验证，后理想化**：优先选择可测试、可观测、可回滚的实现
- **先真实能力，后概念包装**：先让 Builder、Memory、Calibration 真能用，再谈长期叙事

### 0.1 当前阶段 Definition of Done

一个功能只有同时满足以下条件，才算进入“已完成”：

- 后端命令或核心逻辑已有稳定入口
- 前端路径可操作，不依赖开发者手工构造状态
- 关键状态会持久化，不因刷新/重启丢失
- 至少有一条自动化测试覆盖主成功路径
- 出错时能给出明确反馈，而不是静默失败
- 不破坏已有聊天主链路

---

## 一、当前开发进度总结

### 1.1 已完成功能（按 2026-04-18 代码基线重算）

| 模块 | 已完成功能 | 完成度 |
|------|-----------|--------|
| **人生模型** | `LifeModel` schema 已与前后端对齐，编辑器可维护身份/目标/能力/关系/偏好，Builder 可沉淀到模型 | 85% |
| **对话系统** | 流式聊天、会话管理、混合调度、快捷指令持久化、工具审批流、自动打卡每日目标 | 85% |
| **MCP 集成** | Registry、ToolManifest、工具发现与执行、模板安装、推荐卡片、高风险确认执行 | 82% |
| **A2A 集成** | AgentCard、Task 收发、Sidecar、本地桥接调试、Hermes ↔ A2A 转换预览 | 76% |
| **版本控制** | Snapshot/List/Restore/Diff、自动备份、语义标签、维度着色 diff、语义化版本递增、每日首次保存自动快照 | 82% |
| **反馈/进化** | 反馈采集、报告生成、微进化建议、校准确认、应用后自动快照 | 72% |
| **记忆系统** | `memories` 统一表、`fts5`、混合检索、聊天同步写入、MemorySearch 页面、状态历史记录 | 78% |
| **隐私层** | `PrivacyEngine` 脱敏/重构、A2A 价值观评估过滤、MCP 高风险确认 | 58% |
| **Hermes / 路由** | 三层 trace、超时重试、策略回退、Router 状态可观测、前端可视化 | 74% |

### 1.2 已跑通的核心链路

```
用户输入 → IntentRouter[规则版] → LayerRouter → HermesBus[Meaning/Strategy/Execution]
        → 隐私脱敏 → 向量记忆检索 → Scheduler[L2 Ollama / L3 OpenRouter] → 工具调用
        → 隐私重构 → 流式返回前端 → 向量化存储
```

### 1.3 当前总体完成度

- **产品可体验度**：约 `78%`
- **核心主链闭环度**：约 `84%`
- **工程化成熟度**：约 `68%`

当前判断：

- `Phase 1 ~ Phase 4` 的目标都已完成主闭环，项目已经从“复杂原型”进入“可体验 Alpha”
- 现在最缺的不是新页面，而是把剩余的深层能力和产品稳定性继续收口
- 后续开发不再适合沿着旧 Phase 继续平铺，而更适合进入 **Alpha+ 强化阶段**

### 1.4 已开发但仍需继续打磨的部分

- `Builder` 已可恢复与沉淀模型，且已补上首版 4 会话脚本化深问与全对组合的价值排序；但峰值体验深挖、排序解释和后续联动还不够强
- `状态系统` 已有录入、历史、预警，但趋势展示和对话联动仍偏基础
- `版本控制` 已能对比和回滚，但差异摘要、版本语义提示和演化路径展示还可以更强
- `Hermes` 已具备工具建议、仲裁提醒与 trace 展示，但更复杂的跨工具自治编排仍属于后续增强
- `隐私 / MCP` 已具备确认机制，但参数级脱敏网关和调用日志加密还没完成

---

## 二、待开发/待完善需求清单

### 2.1 还没有完成的高优先级缺口

- **STATE-UI**：状态历史已经写入 SQLite，但 Dashboard 还缺可读性更强的趋势图与连续异常解释
- **DAILY-NLU**：目前只有基础自动打卡，尚未形成稳定的对话式目标更新与更丰富的自然语言识别
- **BUILD-DEEP**：Socratic Builder 主链已完成，但峰值体验深挖和排序结果解释仍可继续增强
- **EVOL-FUSION**：显式反馈、行为事件、对话推断已接入融合微进化，后续重点转向权重校准和更细粒度信号
- **HERM-CONFLICT**：Hermes 已具备首版冲突仲裁与工具规划提示，后续重点转向更复杂的跨工具执行编排
- **MCP-PRIVACY**：MCP 调用前还没有统一的参数级 PII 扫描、脱敏网关和日志加密
- **A2A-SERVICE**：当前更偏调试桥接，面向外部 Agent 的稳定服务接口仍需继续产品化
- **TEST-E2E**：页面单测已经跟上，但关键用户旅程的端到端测试还没建立

### 2.2 中优先级工程化问题

- **PERF-L1**：Layer 1 当前具备 ONNX/规则双轨与状态观测，但性能指标尚未压到 `<50ms P99`
- **MEM-TIERING**：记忆有统一表和混合检索，但热记忆常驻、归档分层和访问驱动晋升策略仍未成型
- **VCS-SUMMARY**：版本 diff 已经可视化，但还缺更高层的差异摘要、关键变化解释和演化路径呈现
- **OBSERVABILITY**：Hermes、MCP、进化和 Builder 的诊断信息已有基础，但还未形成统一的调试面板

### 2.3 暂不进入当前开发主线的事项

- **联邦 Agent 网络**
- **数字遗产**
- **多端同步**
- **复杂自治工作流**

这些方向保留在长期 PRD 中，但当前阶段不进入主开发节奏。

---

## 三、分 Phase 开发计划

> 总时长：**8-10 周**
> 原则：**先堵数据模型 → 再补智能层 → 最后做体验和生态**

```mermaid
flowchart LR
    A[Phase 1: 数据与记忆基座] --> B[Phase 2: 智能分层与路由]
    B --> C[Phase 3: 进化与Hermes闭环]
    C --> D[Phase 4: 生态与体验打磨]
```

### 3.1 四个 Phase 的目标边界

| Phase | 目标 | 不做什么 |
|------|------|----------|
| **Phase 1** | 修正核心数据结构、记忆入口、基础契约 | 不追求平台化，不做新概念扩张 |
| **Phase 2** | 让路由、Hermes、Builder、四维关联真正开始产出价值 | 不做过早的复杂自治 |
| **Phase 3** | 形成校准、进化、快照、回滚闭环 | 不把“自动进化”做成不可解释黑盒 |
| **Phase 4** | 补生态和体验，让系统更像产品而不是实验台 | 不做数字遗产、多端同步、联邦网络等远期功能 |

---

## Phase 1: 数据模型与记忆基座升级（Week 1-3）

### Phase 1 完成说明（2026-04-17）

这一阶段已按“Alpha 收口”目标完成主闭环，当前代码基线已经具备以下结果：

- `LifeModel schema` 已完成前后端对齐，并兼容旧字段名反序列化
- `LifeModelEditor` 已能编辑新增身份、角色、关系、偏好、工具与知识域字段
- 聊天消息写入已统一同步到 `messages + memories`，不再是单表孤立持久化
- `memory.rs` 已引入统一 `memories` 表、`fts5` 全文索引、关键词降级检索和手动记忆写入入口
- 聊天预处理和独立记忆搜索都已升级为“关键词 + 向量”混合检索
- Layer 1 已基于现有 ONNX/规则双轨实现补齐可观测状态，前端设置页可看到当前是否已降级到规则兜底

本阶段未继续扩张的内容：

- 没有为了满足名义 checklist 额外重写新的 `MemoryManager` 大类，而是优先把现有 `MemoryStore` 收敛成统一写入与检索底座
- 没有替换现有 ONNX 推理实现为另一套 runtime；当前是沿用已有 `tract-onnx` 方案并补齐验证与可见性

因此，Phase 1 可以视为“完成并收口”，后续工作默认进入 Phase 2。

### Sprint 1: LifeModel Schema 对齐 + Migration（Week 1）

**目标**: 将核心数据结构与 PRD 对齐，确保后续分析功能有数据可算。

- [ ] **LM-Schema-01** 扩展 `Identity`：增加 `mission_statement`、`role_definition`（primary_role, sub_roles, responsibilities, boundaries）、`voice_style`
- [ ] **LM-Schema-02** 扩展 `Goals`：增加 `daily` 数组（`DailyGoal` + `TimeBlock`）、`progress` f32、`related_memories` UUID 数组
- [ ] **LM-Schema-03** 扩展 `Capabilities`：增加 `tools`（`ToolCapability`）和 `knowledge_domains`（`KnowledgeDomain`）
- [ ] **LM-Schema-04** 新增 `Relationships` 和 `Preferences` 顶层模块到 `LifeModel`
- [ ] **LM-Schema-05** 前端编辑器同步更新表单字段（人生模型编辑器、Dashboard 雷达图数据源更新）
- [ ] **LM-Schema-06** 编写数据迁移逻辑：读取旧 YAML → 补默认值 → 写入新格式

**验收标准**: 
- `serde_yaml` 序列化/反序列化新结构不报错
- 前端能正常编辑新增字段
- 旧用户数据打开后自动升级

### Sprint 2: 记忆系统 Tier 2/3 重构（Week 2）

**目标**: 建立符合 PRD 的温记忆与检索记忆基础设施。

- [ ] **MEM-01** 重构 `memory.rs`：引入统一的 `memories` 表，字段覆盖 `id/content/content_type/importance_score/access_count/last_accessed/tags/privacy_level/embedding_id/checksum`
- [ ] **MEM-02** 添加 `fts5` 虚拟表实现全文搜索（需在编译期启用 `sqlite3` fts5 扩展，或通过 `rusqlite` bundled feature）
- [ ] **MEM-03** 实现 `MemoryManager` Trait/Struct：统一封装 `store/retrieve/get_context_window/promote/archive`
- [ ] **MEM-04** 实现混合检索接口：`search_hybrid(query, limit)` 综合向量相似度 + FTS 关键词匹配 + 时间衰减
- [ ] **MEM-05** 更新 `src-tauri/src/lib.rs` 中所有直接操作 `messages` 表的代码，改为通过 `MemoryManager` 写入（统一入口）

**验收标准**:
- `cargo test` 通过 `MemoryManager` 的 CRUD + 检索单元测试
- 全文搜索能命中历史对话中的关键词
- 混合检索结果包含 `relevance_score` 和 `source_tier`

### Sprint 3: Layer 1 反应层 — ONNX 意图分类（Week 3）

**目标**: 让 Layer 1 从玩具级升级为工程级，达到 P99 < 50ms 的目标。

- [ ] **L1-01** 引入 `ort` (ONNX Runtime) crate，设计 `ReflexEngine` 结构体
- [ ] **L1-02** 集成 DistilBERT-Q8（或更小的中文模型如 `bert-base-chinese` ONNX）进行意图分类
- [ ] **L1-03** 实现并行流水线：意图分类(ONNX) || 隐私检测(规则引擎) || 价值观过滤(HashSet) → 结果合并 → 路由决策
- [ ] **L1-04** 实现智能降级：`ReflexLatencyMonitor` + `DegradationLevel`（Normal/Light/Medium/Bypass）
- [ ] **L1-05** 预加载策略：Tauri `setup` 阶段加载 ONNX Session，避免冷启动延迟
- [ ] **L1-06** 替换 `IntentRouter::classify` 中纯正则实现，优先走 ONNX，正则作为降级兜底

**验收标准**:
- 本地运行 100 次分类，P99 延迟 < 80ms（MVP 阶段先达成 <80ms，后续优化到 <50ms）
- 意图分类准确率 > 85%（先用小测试集验证）
- 降级机制在 CPU 高负载时能正常 fallback

---

## Phase 2: 智能分层、Builder 与四维关联（Week 4-6）

### Phase 2 完成说明（2026-04-17）

这一阶段已按“让路由、Builder 和关联分析开始真正产出价值”的目标完成主闭环，当前代码基线已经具备以下结果：

- `Hermes` 三层节点已具备超时、重试、结构化 trace 和前端可视化展示
- `StrategyNode` 在 LLM 不可用时会退回确定性策略，避免深度链路完全失效
- `HermesTracePanel` 已可展示对齐价值观、优先目标、计划步骤、工具需求和分层耗时
- `BuilderSession` 新增 `current_prompt` 持久化字段，未完成会话恢复时不再错误推进到下一步
- `Builder` 的模型提取结果已扩展到 `mission_statement`、`role_definition`、`tools`、`knowledge_domains`、`relationships`、`preferences`
- `Dashboard` 已从字符串提示升级为结构化展示：目标-能力缺口与价值观-目标一致性都有独立卡片和严重度标记
- 后端已补充结构化分析接口：`goal_capability_gap_report` 与 `identity_goal_alignment_report`

本阶段的执行取舍：

- 没有把 Hermes 做成完全自治的工具编排系统，而是优先补强“稳定策略 + 可解释 trace + 前端可见”
- 没有把 Builder 改造成复杂工作流引擎，而是优先修正恢复逻辑和提取深度，确保构建链路连续可用

因此，Phase 2 可以视为“完成并收口”，后续工作默认进入 Phase 3。

### Sprint 4: LayerRouter / HermesBus 工程化（Week 4）

**目标**: 让层间路由和 Hermes 三层决策真正可用。

- [ ] **HERM-01** 实现真正的 `MeaningNode`：读取人生模型核心价值观，对请求进行意义评估，输出 `meaning_result` 约束文本
- [ ] **HERM-02** 实现 `StrategyNode`：基于人生模型目标层，对复杂请求生成策略计划（如“如何达成某目标”）
- [ ] **HERM-03** 实现 `ExecutionNode`：将策略拆解为工具调用序列，委托给 MCP Registry 执行
- [ ] **HERM-04** 完善 `HermesBus::dispatch`：增加层间超时（Meaning 500ms / Strategy 2s / Execution 5s）和错误重试
- [ ] **HERM-05** `LayerRouter` 引入基于 `TaskComplexity` 的智能评估（prompt token 数、工具需求、上下文长度）
- [ ] **HERM-06** 前端 `HermesTracePanel` 增强：可视化展示三层节点的输入/输出/耗时

**验收标准**:
- 问“帮我规划职业发展”能触发 L3 → Hermes Meaning+Strategy → 输出结构化建议
- HermesTrace 中三层都有非空结果
- 超时场景能正确 fallback 到 L2

### Sprint 5: Builder 完善 + 四维关联引擎 v1（Week 5）

**目标**: 让苏格拉底构建器成为产品卖点，同时启动模型智能分析。

- [ ] **BUILD-01** 设计苏格拉底 4 次会话剧本：Session 1 峰值体验/价值观、Session 2 角色与使命、Session 3 目标体系、Session 4 能力与缺口
- [ ] **BUILD-02** 实现峰值体验挖掘提示词工程：引导用户回忆高光时刻，自动提取价值关键词
- [ ] **BUILD-03** 实现价值排序交互：成对比较（Pairwise Comparison）算法，让用户在 2 选 1 中确定优先级
- [ ] **BUILD-04** 实现 Builder 会话状态的持久化与恢复（已部分实现，需对接 4 次会话进度）
- [ ] **LINK-01** 实现四维关联引擎 v1：`goal_capability_gap_analysis` 函数，对比目标所需能力 vs 当前能力评分
- [ ] **LINK-02** 前端 Dashboard 新增“目标-能力缺口”卡片：列出 Top 3 缺口及建议学习路径
- [ ] **LINK-03** 实现 `identity_goal_alignment_check`：检查高优先级目标是否与核心价值观冲突

**验收标准**:
- 用户能完成一次 4 会话苏格拉底构建，最终生成完整人生模型
- Dashboard 能显示至少 1 个能力缺口分析结果

### Sprint 6: 状态层管理 + 每日目标（Week 6）

**目标**: 补齐状态层和日常目标管理，形成用户每日使用的理由。

- [ ] **STATE-01** 扩展 `State` 结构：支持用户自定义状态维度（如体重、睡眠、专注度）和阈值
- [ ] **STATE-02** 实现状态历史记录表 `state_history`（SQLite），支持按维度查询时间序列
- [ ] **STATE-03** 前端新增状态录入弹窗和趋势折线图（可用 Recharts 或 SVG 自绘）
- [ ] **STATE-04** 实现状态预警：当某指标连续 N 天超出阈值时，在 Dashboard 显示预警卡片
- [ ] **DAILY-01** 实现 `DailyGoal` 的 CRUD 和完成打卡
- [ ] **DAILY-02** 前端新增“今日目标”小组件（放在 Dashboard 顶部）
- [ ] **DAILY-03** 每日目标与对话系统打通：用户说“我今天完成了…”能自动识别并更新进度

**验收标准**:
- 用户能在 Dashboard 记录体重，7 天后看到趋势图
- 用户能创建 3 个今日目标并打卡
- 对话中提到完成目标时，系统能更新状态

---

## Phase 3: 进化系统闭环与版本控制（Week 7-8）

### Phase 3 完成说明（2026-04-17）

这一阶段已按“进化、校准、快照、回滚形成可确认闭环”的目标完成主链路，当前代码基线已经具备以下结果：

- `run_micro_evolution` 现在会返回结构化结果，并在成功应用后带回自动快照版本号
- `generate_micro_evolution_changes` 已补充应用前/应用后 4D 完成度预览和确认标记
- `CalibrationPage` 已成为真正的确认界面：可勾选变更、查看 before/after 完成度，并在应用后收到明确快照编号
- `apply_calibration` 不再依赖前端重复创建快照，后端会统一创建 `auto:calibration` 快照并记录分析事件
- `restore_snapshot` 在回滚前会自动创建 `auto:pre-restore` 备份，降低误回滚风险
- `VersionManager` 已补齐版本索引持久化，`tag / note / timestamp` 不再只存在于瞬时返回值中
- `VersionControl` 页面现在能稳定展示自动进化、校准、回滚相关快照语义，而不只是裸文件名

本阶段的执行取舍：

- 没有把“自动进化”做成不可解释黑盒，而是坚持“先预览、再确认、再快照”
- 没有引入新的复杂工作流系统，而是优先收口已有 Tauri 命令、页面和版本存储语义

因此，Phase 3 可以视为“完成并收口”，后续工作默认进入 Phase 4。

### Sprint 7: 微进化 + 周期校准（Week 7）

**目标**: 让系统能自动成长，用户只需轻量确认。

- [ ] **EVOL-01** 实现 `MicroEvolutionEngine`：自动分析最近 7 天反馈、行为日志、对话记录，输出权重调整建议
- [ ] **EVOL-02** 设计权重调整规则：价值观权重 ±0.01~0.03、目标优先级微调、能力评分根据行为修正
- [ ] **EVOL-03** 实现一致性校验：调整后的模型必须通过 `identity_goal_alignment_check`
- [ ] **EVOL-04** 实现周期校准报告自动生成（每周一/每月1日），通过系统通知或 Dashboard 红点提醒用户
- [ ] **EVOL-05** 前端新增“校准确认”页面：展示变更列表（Before/After），用户可一键确认或逐个修改
- [ ] **EVOL-06** 实现多源融合 `FusionEngine`：将反馈(0.5) + 行为(0.3) + 对话推断(0.2) 加权求和，冲突时以显式反馈为准

**验收标准**:
- 连续 7 天有反馈/对话后，点击微进化能看到具体的权重调整建议
- 校准报告能正常生成，用户确认后模型更新并自动创建快照

### Sprint 8: 版本控制 + 进化回滚 + 隐私 MCP 网关（Week 8）

**目标**: 建立安全感和生态信任。

- [ ] **VCS-01** 实现语义化版本自动递增：`MAJOR`（深度进化/人生大事）、`MINOR`（周期校准）、`PATCH`（微进化/手动保存）
- [ ] **VCS-02** 实现自动快照触发：每次校准确认后、每次深度进化后、每日首次保存时
- [ ] **VCS-03** 前端版本对比可视化：用 diff 组件高亮 Identity/Goals/Capabilities/State 的变更字段
- [ ] **VCS-04** 实现进化回滚 UI：在版本列表中标记“进化版本”，支持一键回滚并恢复旧模型
- [ ] **PRIV-01** 实现 `McpPrivacyGateway`：在 MCP 工具调用前扫描参数，自动识别 PII 并脱敏
- [ ] **PRIV-02** 实现高风险 MCP 操作确认弹窗（如写文件、发送邮件前二次确认）
- [ ] **PRIV-03** MCP 调用日志本地加密存储（调用记录写入 SQLite 并 AES 加密）

**验收标准**:
- 版本号能根据操作类型自动递增
- 能对比两个版本的差异，高亮变更字段
- 调用 MCP 写文件工具时，参数中的手机号/邮箱被脱敏，且用户收到确认弹窗

---

## Phase 4: 生态集成与体验打磨（Week 9-10）

### Phase 4 完成说明（2026-04-17）

这一阶段已按“补生态和体验，让系统更像产品而不是实验台”的目标完成主闭环，当前代码基线已经具备以下结果：

- `McpRegistry` 与前端页面已围绕统一 `ToolManifest` 工作，工具具备名称、描述、权限级别、标签和来源标记
- 内置 MCP 模板资源已接入前端安装向导，推荐卡片可直接跳转到模板预览并完成注册
- `recommend_mcp_manifests` 已在前端落地为“推荐工具”面板，可根据当前阶段展示内置能力与外部 MCP 建议
- 高风险 MCP 工具保持“先确认、后执行”的审批语义，页面展示与后端行为已经一致
- A2A 页面新增本地 sidecar 启停控制，能直接验证本地服务状态
- 新增 `a2a_bridge_local` 调试链路，可在页面中把本地 Hermes 请求转换为 A2A Task，再查看 A2A 响应与回转后的 Hermes 结果
- `McpPage` 与 `A2APage` 都已补齐页面级自动化测试，生态相关能力不再只靠手工点点看

本阶段的执行取舍：

- 没有继续扩张到联邦网络、多端同步或数字遗产，而是优先把 `MCP 推荐/安装` 与 `A2A 本地桥接` 做成可点击、可验证的真实功能
- 没有把生态系统做成高度自治黑盒，而是优先补强模板、推荐、桥接调试和 sidecar 控制这类更容易体验和排错的产品层能力

因此，Phase 4 可以视为“完成并收口”，当前项目已具备可体验的 Alpha 产品基线；后续迭代更适合转向体验反馈、稳定性增强和新一轮产品取舍，而不是继续按旧清单机械铺功能。

### Sprint 9: MCP/A2A 生态完善（Week 9）

**目标**: 让 OpenLife 真正融入 Agent 生态。

- [ ] **MCP-01** 定义 `ToolManifest` 统一结构：名称/描述/参数 Schema/权限要求/版本/来源标注（MCP/WASM/Native）
- [ ] **MCP-02** 重构 `McpRegistry`：所有工具统一包装为 `ToolManifest`
- [ ] **MCP-03** 内置 5+ 常用 MCP Server 配置模板（filesystem、fetch、brave-search、sqlite、github）
- [ ] **MCP-04** 前端 MCP 页面新增“一键安装模板”向导
- [ ] **MCP-05** 实现工具推荐引擎 v1：根据能力缺口推荐相关 MCP Server（如缺时间管理推荐 calendar）
- [ ] **A2A-01** 实现 `Hermes-A2A Bridge`：`HermesRequest` ↔ `A2A Task` 双向转换，保持三层语义
- [ ] **A2A-02** 外部 Agent 调用 OpenLife 时，能查询脱敏后的人生模型和价值观评估服务

**验收标准**:
- 新用户能在 3 分钟内通过向导配置好 filesystem MCP
- 系统能根据能力缺口推荐至少 1 个相关 MCP
- 外部 Agent 通过 A2A 能成功获取 OpenLife 的 AgentCard

### Sprint 10: 全局体验打磨与性能优化（Week 10）

**目标**: 让产品从“可用”变为“好用”。

- [ ] **UI-01** Dashboard 重构：顶部今日目标 + 能力雷达图 + 目标缺口 + 状态趋势 + 版本/校准提醒
- [ ] **UI-02** ChatPage 优化：支持快捷指令（如“/goal 查看目标”、“/state 记录状态”）
- [ ] **UI-03** 全局加载态与空状态统一设计
- [ ] **PERF-01** 优化 `VectorStore` 批量写入：对话结束后批量 insert embedding，减少 SQLite WAL 竞争
- [ ] **PERF-02** 优化 `MemoryManager` 查询：为 `session_id + created_at` 和 `embedding_id` 添加联合索引
- [ ] **PERF-03** Layer 1 ONNX 延迟专项：尝试 INT8 量化模型，目标 P99 < 50ms
- [ ] **TEST-01** 补充核心 Rust 模块单元测试覆盖率至 60%+
- [ ] **TEST-02** 补充前端关键页面 E2E 测试（Builder 流程、Chat 流程、Dashboard 数据展示）

**验收标准**:
- 核心功能页面无明显的加载白屏
- Chat 快捷指令能被正确解析并跳转
- `cargo test` 单元测试全部通过

---

## 四、关键依赖路径与风险

## Alpha+ 三阶段路线

> 说明：旧 `Phase 1 ~ Phase 4` 已经完成主闭环。接下来的开发不再继续扩旧 Phase，而是按下面 3 个阶段推进。

### Stage 1：体验强化（当前阶段）

**完成状态（2026-04-18）**

- 已完成并收口
- 本阶段已落地：Dashboard 状态趋势解释、DailyGoal 对话联动增强、VersionControl 差异摘要、Builder 阶段建议反馈
- 后续若继续改动这些模块，默认视为 `Stage 2 / Stage 3` 的延伸打磨，而不再算作 Stage 1 主任务

**阶段目标**

- 让 OpenLife 从“功能已经很多”变成“用户每天愿意打开”
- 补齐高频页面的解释力、反馈感和可读性
- 优先增强 `Dashboard / DailyGoal / VersionControl / Builder` 这几条最常被直接感知的体验链路

**主要开发项**

- `STATE-UI`：Dashboard 状态趋势图、异常解释卡片、趋势摘要
- `DAILY-NLU`：日目标对话联动增强，支持更稳定的自然语言更新
- `VCS-SUMMARY`：版本差异摘要、版本语义标签、关键变化解释
- `BUILD-UX`：Builder 缺口提示、完成度提醒、恢复体验和阶段反馈
- `GLOBAL-FEEDBACK`：页面加载态、空状态、成功/失败反馈统一

**完成标准**

- Dashboard 能清楚解释最近状态变化，而不只是列数值
- 用户通过对话补日报/打卡时，系统反馈足够稳定且可理解
- 版本页不只显示 diff，还能告诉用户“这次变化主要发生在哪”
- Builder 的中断恢复和当前进展对首次体验用户更友好

### Stage 2：智能深化

**阶段目标**

- 让系统真正更“懂你”，而不是只有很多模块并列存在
- 把 Builder、Hermes、Evolution 三条智能链路打通

**主要开发项**

- `BUILD-DEEP`：Socratic Builder 4 会话深度剧本
- `VALUE-PAIRWISE`：Pairwise 价值排序与峰值体验提取
- `EVOL-FUSION`：反馈、行为、对话推断的融合引擎
- `HERM-CONFLICT`：Hermes 冲突仲裁器
- `HERM-TOOLS`：更稳定的多工具规划与执行编排

**当前进展（2026-04-18）**

- `BUILD-DEEP` 已完成：Socratic Builder 改为脚本化 4 会话深问，价值排序从相邻比较升级为全对组合（上限 6 组），降低了生成漂移并提升了排序信号质量
- `EVOL-FUSION` 已完成首版收口：Builder / 对话 / 反馈三路信号已进入融合微进化链路，校准页可看到融合信号概览
- `HERM-CONFLICT` 已完成首版收口：Hermes trace 现已带出建议工具链与仲裁提醒，前端可直接查看
- 因此 `Stage 2` 视为完成，后续相关改动默认并入 `Stage 3` 的安全与工程化增强

**完成标准**

- Builder 能支持一次完整深度构建，而不只是问答外壳
- 微进化不再只依赖单一信号，建议更连贯、更可信
- Hermes 面对复杂任务时能更稳定地产出策略与工具链路

### Stage 3：安全与工程化

**阶段目标**

- 让 Alpha 从“可体验”走向“可长期使用”
- 补齐生态调用的隐私与审计能力，降低后续迭代风险

**主要开发项**

- `MCP-PRIVACY`：参数级 PII 扫描、脱敏网关、调用日志加密
- `TEST-E2E`：关键用户旅程端到端测试
- `PERF-L1`：Layer 1 延迟继续压缩
- `MEM-TIERING`：热记忆常驻、归档和访问驱动晋升
- `OBSERVABILITY`：统一诊断面板与更清晰的运行时调试信息

**完成标准**

- 高风险 MCP 调用不只需要确认，还能被清晰审计和脱敏
- 核心用户旅程有自动化回归保护
- 记忆、路由、Hermes 的工程可观测性明显提升

**当前进展（2026-04-18）**

- `MCP-PRIVACY` 已完成首版收口：工具参数现支持结构化 PII 扫描、脱敏参数预览、敏感参数确认执行，以及加密审计日志回看
- `OBSERVABILITY` 已完成首版收口：设置页新增统一诊断面板，MCP 管理页可直接查看近期审计与 PII 命中
- `TEST-E2E` 已完成当前基线所需的关键旅程回归保护：MCP/ToolCard 相关回归测试已补齐，后续若需要更完整的流式聊天旅程，将继续补浏览器级 E2E
- 因此 `Stage 3` 视为完成；后续更多性能优化和更重型的浏览器级 E2E 将作为下一阶段增强

### 推荐执行顺序

1. `Stage 1`
2. `Stage 2`
3. `Stage 3`

当前立刻开始的事项：

1. `Dashboard 状态趋势图 + 异常解释`
2. `DailyGoal 对话联动增强`

### 依赖链

```
LM Schema 对齐 ──┬──> 四维关联引擎 ──> Builder 完善
                 ├──> Hermes Meaning ──> 进化系统
                 └──> Dashboard 数据展示

ONNX Layer 1 ──> LayerRouter 智能评估 ──> HermesBus 工程化

MemoryManager ──> 混合检索 ──> Chat 记忆上下文质量提升
```

### 技术风险与缓解

| 风险 | 缓解策略 |
|------|----------|
| ONNX Rust 绑定 (`ort`) 编译复杂 | 先使用 CPU EP，验证通后再考虑 CoreML；保留规则 fallback |
| `sqlite` fts5 在 Windows/macOS 编译问题 | 使用 `rusqlite` 的 `bundled-full` feature，或降级为 `LIKE` 搜索 |
| Hermes 三层节点提示词工程量大 | 先用 hard-coded system prompt 验证，再逐步提炼 |
| 微进化权重算法可能过拟合 | 限制单次调整幅度 ±0.03，必须经过一致性校验 |
| 前端页面过多重构导致回归 | 每 Sprint 结束前跑一遍手动核心链路检查 |

---

## 五、建议的启动顺序

如果你希望**立刻开始高强度开发**，建议按以下顺序执行：

1. **Week 1 立即启动**: `LM Schema 对齐` —— 这是所有上层功能的地基，必须在第一周封板
2. **同步启动**: `ONNX Layer 1` —— 可与 Schema 工作并行（不同的人/模式负责）
3. **Week 2 启动**: `MemoryManager 重构` —— 依赖于 Schema 中 `related_memories` 等字段定义
4. **Week 4 启动**: `HermesBus 工程化` + `Builder 完善` —— 此时底层已稳，可以堆智能层
5. **Week 7 启动**: `微进化引擎` —— 需要前面产生的数据才能跑通闭环
6. **Week 9 启动**: `MCP/A2A 生态` —— 锦上添花，放在最后

---

## 六、下一步行动

确认本计划后，我们将：
1. 为每个 Sprint 创建独立的 TODO 清单
2. 进入 **Code Mode** 逐条实现
3. 每完成一个 Sprint 进行编译验证和手动验收
---

## 七、Alpha → Beta 后续开发规划（2026-04-19 起）

> 当前状态：旧 `Phase 1 ~ Phase 4` 与 `Alpha+ Stage 1 ~ Stage 3` 已完成主闭环。后续开发目标从“补模块”切换为“让试用稳定、让核心智能变深、让产品可长期使用”。

### 7.1 总体目标

- **Alpha 稳定化**：试用用户能顺利完成建模、对话、记录目标、校准、回滚和工具审批。
- **Beta 产品化**：核心体验形成稳定闭环，错误可解释，数据可迁移，关键旅程有自动化保护。
- **v1.0 前置能力**：深度 Builder、长期记忆分层、隐私治理、Hermes 工具编排具备可持续扩展基础。

### 7.2 里程碑划分

| 阶段 | 周期 | 核心目标 | 交付判断 |
|------|------|----------|----------|
| **Milestone A：试用稳定化** | 1-2 周 | 修掉试用中“点了没反应 / 不知道为什么失败”的问题 | 用户能按引导完成 5 条核心路径 |
| **Milestone B：核心体验产品化** | 2-4 周 | Chat、Dashboard、Builder、Calibration 从功能页变成顺畅体验 | 主要页面有清晰空状态、错误态和下一步建议 |
| **Milestone C：智能深化** | 3-5 周 | Builder、Evolution、Hermes 的输出更可信、更可解释 | 模型构建和校准建议可被用户理解和确认 |
| **Milestone D：长期记忆与安全治理** | 4-6 周 | 记忆分层、MCP 安全、审计和数据管理进入长期可用状态 | 数据可查、可导、可回滚、可审计 |
| **Milestone E：Beta 发布准备** | 1-2 周 | 端到端测试、打包、安装、首次启动体验、文档完整 | 可交给非开发者试用 |

### 7.3 Milestone A：试用稳定化

优先级最高。目标是把“功能基本都无法实现”的反馈变成可定位、可修复、可验证的问题。

开发项：

- `DIAG-READY-01`：扩展 `get_system_diagnostics`，返回本地模型、云端 Key、数据目录、快照数量和聊天就绪状态。
- `DIAG-READY-02`：设置页新增“试用就绪检查”，直接告诉用户 Ollama/API Key/数据目录/记忆/Builder 是否正常。
- `CHAT-FALLBACK-01`：聊天页在本地和云端都不可用时给出明确操作建议，而不是只显示失败。
- `RUNTIME-ERROR-01`：统一关键页面错误提示，不再静默吞掉 `getLifeModel`、`getDailyGoals`、`getStateHistory` 等失败。
- `DATA-PATH-01`：持续验证配置、LifeModel、版本快照、数据库都写到统一应用数据目录。

验收标准：

- 设置页能显示“聊天可用 / 需要启动 Ollama / 需要配置 API Key”等明确状态。
- 新用户不配置云端 Key 时，如果本地 Ollama 可用，聊天可以走本地模型。
- 如果本地和云端都不可用，用户能看到下一步操作，而不是误以为应用坏了。

### 7.4 Milestone B：核心体验产品化

目标是让用户每天使用的路径更像产品，而不是功能集合。

开发项：

- `CHAT-UX-01`：聊天页增加首屏状态条，显示当前推理后端、本地模型、云端 Key 状态和快捷修复入口。
- `DASH-UX-01`：Dashboard 对缺数据状态给出“先做 Builder / 先添加目标 / 先记录状态”的分步引导。
- `BUILDER-UX-01`：Builder 完成后给出“已沉淀到哪些字段”的摘要，并提供跳转到 Dashboard/Editor 的入口。
- `CALIB-UX-01`：校准页增强变更解释，把每条变化关联到反馈、行为或对话推断来源。
- `VERSION-UX-01`：版本页增加“推荐保留 / 可回滚 / 风险提示”标签。

验收标准：

- 首次启动用户知道先做什么。
- 每个核心页面都有清晰的空状态和下一步动作。
- 关键操作完成后能看到结果落在了哪里。

### 7.5 Milestone C：智能深化

目标是让 OpenLife 的差异化真正体现为“更懂用户”。

开发项：

- `BUILDER-DEEP-02`：增强峰值体验提取，将用户回答拆成价值观、角色、能力、偏好和状态信号。
- `PAIRWISE-EXPLAIN-01`：价值排序完成后输出排序解释和冲突提醒。
- `EVOL-WEIGHT-01`：微进化信号权重可配置，并在校准页显示每条建议的置信度。
- `HERMES-PLAN-01`：Hermes 对复杂任务输出更稳定的步骤计划、风险、工具建议和回退方案。
- `HERMES-TOOLS-01`：多工具执行从“提示建议”升级为受控计划：计划 → 确认 → 执行 → 总结。

验收标准：

- Builder 输出不只是填字段，还能解释“为什么这样理解你”。
- 校准建议能追溯来源。
- Hermes 对复杂任务的计划更稳定，失败时能说明原因。

### 7.6 Milestone D：长期记忆与安全治理

目标是把“能记录”升级为“能长期可靠地记住、检索、治理”。

开发项：

- `MEM-TIER-01`：实现热记忆摘要缓存，常驻核心身份、Top values、当前目标和近期状态。
- `MEM-TIER-02`：实现访问驱动晋升/降级，基于 access_count、last_accessed_at 和 importance_score 调整记忆权重。
- `MEM-ARCHIVE-01`：归档低访问旧记忆，保留摘要和可恢复原文。
- `MCP-AUDIT-02`：MCP 审计日志加密密钥管理、导出与清理策略。
- `PRIVACY-POLICY-01`：将隐私规则从硬编码升级为可配置策略。

验收标准：

- 长对话后检索质量不会明显下降。
- 用户能理解某条记忆为什么被注入。
- 工具调用历史可审计、可清理、可导出。

### 7.7 Milestone E：Beta 发布准备

目标是准备交给非开发者试用。

开发项：

- `E2E-01`：补 Playwright 或等价浏览器级关键旅程：首次启动 → Builder → Chat → Dashboard → Calibration → Version rollback。
- `PACKAGING-01`：整理 macOS 打包、图标、权限、首次启动说明。
- `ONBOARDING-01`：新增首次启动向导：选择本地/云端模型、创建初始 LifeModel、解释隐私策略。
- `IMPORT-EXPORT-01`：导入导出增加版本校验和错误提示。
- `DOCS-01`：整理用户试用指南、开发者接手指南、常见问题。

验收标准：

- 非开发者能按文档启动并完成核心体验。
- 打包产物可以安装运行。
- 关键旅程自动化测试稳定通过。

### 7.8 当前立即执行顺序

1. `DIAG-READY-01 / DIAG-READY-02`：增强系统诊断和设置页试用就绪检查。
2. `CHAT-UX-01`：聊天页增加运行后端状态条和失败修复建议。
3. `RUNTIME-ERROR-01`：关键页面错误提示统一。
4. `BUILDER-UX-01`：Builder 完成摘要和跳转。
5. `CALIB-UX-01`：校准建议来源解释。

当前开发从第 1 项开始。
