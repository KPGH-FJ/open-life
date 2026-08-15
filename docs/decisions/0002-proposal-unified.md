# ADR 0002: Proposal 统一层

> Historical ADR retained for Proposal-layer intent. Current semantics are
> governed by `AGENTS.md`, ADR 0016, and current source.

## 状态

- **状态**: 已被 ADR 0016 与当前 v2 Proposal 路径取代；仅保留历史背景
- **日期**: 2026-04-24
- **作者**: OpenLife Team

## 上下文

以下内容描述 2026-04-24 的历史问题与当时决策，不是当前 Builder、Calibration
或 LifeModel 写入接口。当前产品只允许 v2 typed diff 或受治理 legacy migration
Proposal 写入 LifeModel；旧 Builder/Calibration command、4D patch batch 和直接
应用路径已经退役。

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

### 历史接入规则（已退役）

| 模块 | 接入方式 | 默认路径 |
|------|---------|----------|
| Builder | `builder_create_proposals` | Proposal（默认）；`builder_apply_signals` 仅 legacy/migration/debug |
| Calibration | `calibration_create_proposals` | Proposal（推荐）/ 直接应用（兼容） |
| Chat | Chat Proposal generator | Proposal；执行状态经 AgentRun 关联 |
| Evolution | 未来接入 | 仅 Proposal |

## 后果

### 正面

- ✅ 统一了所有 LifeModel 变更的确认流
- ✅ 用户可以集中审阅所有建议
- ✅ 支持分类、筛选、编辑、批量操作
- ✅ 高风险字段默认必须经过确认
- ✅ Safe Mode 下自动阻止 apply/edit

### 当时已知负面

- ⚠️ Builder/Calibration 需要额外步骤（发送到 Review Center）
- ⚠️ 需要维护 proposals.db 的迁移兼容
- ⚠️ 当时仍保留 Builder legacy direct apply；当前实现已经删除该产品路径

## 相关

- [ADR 0001: LifeModel Patch 机制](./0001-lifemodel-patch.md)
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `frontend/src/pages/ChatPage.tsx`（当前 Chat proposal review/action handoff surface）
