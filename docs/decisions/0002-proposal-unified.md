# ADR 0002: Proposal 统一层

## 状态

- **状态**: 已接受
- **日期**: 2026-04-24
- **作者**: OpenLife Team

## 上下文

OpenLife 有多个模块会产生 LifeModel 更新：

- **Builder**: 构建时生成理解建议
- **Calibration**: 定期校准产生变更
- **Evolution**: 微进化系统（未来）
- **Chat**: 对话中 AI 建议修改（未来）

之前每个模块都有自己的确认流：
- Builder: 页面内 Review + 直接应用
- Calibration: 页面内预览 + 直接应用
- 没有统一的地方查看所有待确认变更

## 决策

建立统一的 **Proposal/Confirmation 层**：

### 核心设计

1. **AgentProposal 类型**（统一数据结构）:
   ```rust
   struct AgentProposal {
       id: String,
       proposal_type: ProposalType,    // LifeModelUpdate | GoalUpdate | MemoryUpdate | ToolPermission
       affected_path: String,           // "identity.values"
       before: Option<Value>,           // 变更前值
       after: Value,                    // 变更后值
       reason: String,                  // 为什么建议这个变更
       confidence: f32,                 // 置信度 0-1
       risk_level: RiskLevel,           // Low | Medium | High | Critical
       status: ProposalStatus,          // Pending | Accepted | Rejected | Edited | Postponed
       source: String,                  // "builder:session_id"
       source_run_id: Option<String>,   // 关联的 AgentRun
       source_kind: Option<String>,     // "builder" | "calibration" | "chat"
   }
   ```

2. **ProposalStore**（统一存储）:
   - SQLite `proposals.db`
   - 支持按 status/type/risk 筛选
   - 支持批量操作（仅低风险）

3. **Review Center**（统一审阅界面）:
   - `/review` 页面
   - 分类筛选（全部/LifeModel/Goal/Memory/Tool）
   - 风险筛选（low/medium/high/critical）
   - 行内编辑 after 值
   - 批量接受（仅低风险）
   - 空状态引导（去 Builder/Calibration）

4. **Tauri Commands**（统一操作接口）:
   - `accept_proposal`
   - `reject_proposal`
   - `edit_proposal`
   - `postpone_proposal`
   - `list_proposals`
   - `batch_accept_low_risk_proposals`

### 接入规则

| 模块 | 接入方式 | 默认路径 |
|------|---------|----------|
| Builder | `builder_create_proposals` | Proposal（推荐）/ 直接应用（兼容） |
| Calibration | `calibration_create_proposals` | Proposal（推荐）/ 直接应用（兼容） |
| Chat | 未来接入 | 仅 Proposal |
| Evolution | 未来接入 | 仅 Proposal |

## 后果

### 正面

- ✅ 统一了所有 LifeModel 变更的确认流
- ✅ 用户可以集中审阅所有建议
- ✅ 支持分类、筛选、编辑、批量操作
- ✅ 高风险字段默认必须经过确认
- ✅ Safe Mode 下自动阻止 apply/edit

### 负面

- ⚠️ Builder/Calibration 需要额外步骤（发送到 Review Center）
- ⚠️ 需要维护 proposals.db 的迁移兼容
- ⚠️ 直接应用路径仍保留，需要用户教育

## 相关

- [ADR 0001: LifeModel Patch 机制](./0001-lifemodel-patch.md)
- [ADR 0003: AgentRun 追踪](./0003-agent-run-tracking.md)
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `frontend/src/pages/ProposalReviewPage.tsx`
