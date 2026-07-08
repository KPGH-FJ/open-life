# ADR 0003: AgentRun 追踪机制

> Historical ADR retained for AgentRun intent. Current trace semantics are
> extended by W10 preview audit, W98-W105 Plan-Execute product trace,
> W106-W113 runtime strategy trace vocabulary, and W114-W123 ReAct
> `react_trace` lifecycle hardening.

## 状态

- **状态**: 已接受；trace envelope/current query semantics 以后续治理文档为准
- **日期**: 2026-04-26
- **作者**: OpenLife Team

## 上下文

OpenLife 需要回答用户的问题：
- "刚才 AI 用了什么上下文？"
- "为什么选择了云端模型而不是本地？"
- "这个 LifeModel 变更是从哪来的？"

之前缺乏统一的执行追踪机制，只有零散的日志。需要一种结构化的方式来记录每次 AI 任务的完整执行过程。

## 决策

建立 **AgentRun 追踪系统**：

### 核心设计

1. **AgentRun 类型**:
   ```rust
   struct AgentRun {
       id: String,                      // 唯一标识
       task_id: String,                 // 关联的 AgentTask
       session_id: Option<String>,      // 会话 ID（Chat/Builder）
       kind: AgentTaskKind,             // Conversation | Builder | Calibration | ...
       status: AgentRunStatus,          // Running | Completed | Failed | Cancelled
       user_input: Option<String>,      // 用户输入（Chat）
       context_summary: ContextSummary, // 包含的记忆 hit 数、LifeModel 字段
       model_route: ModelRouteTrace,    // provider/model/route_type/reason
       output_preview: String,          // 输出摘要
       error: Option<AgentRunError>,    // 错误信息（phase/recoverable）
       generated_proposals: Vec<String>,// 本次生成的 Proposal IDs
       started_at: DateTime<Utc>,
       finished_at: Option<DateTime<Utc>>,
   }
   ```

2. **AgentRunStore**:
   - SQLite `agent_runs.db`
   - 按 session_id / time 查询
   - 关联到 Chat 会话和 Builder 会话

3. **与 Proposal 关联**:
   - `AgentProposal.source_run_id` → 指向生成该 Proposal 的 AgentRun
   - `AgentRun.generated_proposals` → 记录本次生成的所有 Proposal IDs
   - 双向溯源：Proposal 能追溯到哪次执行，AgentRun 能查看产生了哪些建议

### 创建时机

| 场景 | 创建时机 | kind |
|------|---------|------|
| Chat 对话 | 用户发送消息时 | Conversation |
| Builder 构建 | 启动 Builder 时 | Builder |
| Builder 创建 Proposal | 创建 Proposal 前 | Builder |
| Calibration | 创建 Proposal 前 | Calibration |

### 前端展示

- **Chat Trace**: 显示当前对话的 AgentRun（模型选择、记忆 hit 数）
- **Builder Result**: 显示生成的 Proposal 数量和 Run ID
- **Review Center**: 显示 Proposal 来源（Builder/Calibration + Run ID）

## 后果

### 正面

- ✅ 所有 AI 执行都可查询、可追溯
- ✅ 用户能知道"为什么 AI 这么回答"
- ✅ 调试时能快速定位问题（模型选择、上下文缺失）
- ✅ Proposal 能追溯到具体哪次执行产生的

### 负面

- ⚠️ 增加了数据库写入（每次 Chat/Builder 都创建记录）
- ⚠️ 需要维护 agent_runs.db 的迁移兼容
- ⚠️ 前端需要额外 UI 展示 Trace 信息

## 相关

- [ADR 0001: LifeModel Patch 机制](./0001-lifemodel-patch.md)
- [ADR 0002: Proposal 统一层](./0002-proposal-unified.md)
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/store.rs`
- `frontend/src/components/RunTracePanel.tsx`（当前 AgentRun trace detail surface）
