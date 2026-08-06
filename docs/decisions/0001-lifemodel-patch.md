# ADR 0001: LifeModel Patch 机制

> Historical ADR. The proposal-based direction remains useful, but any
> direct-write compatibility path described here is superseded by ADR 0016 and
> current gateways.
>
> Current durable LifeModel mutation must follow proposal-first/governed
> boundaries unless a later authoritative document explicitly defines a
> metadata-safe governed exception.

## 状态

- **状态**: 历史参考；部分直接写入假设已被 ADR 0016 覆盖
- **日期**: 2026-04-20
- **作者**: OpenLife Team

## 上下文

OpenLife 需要一种机制来更新用户的 LifeModel（人生模型）。LifeModel 是用户的核心私人数据，包含身份、目标、能力、状态等敏感信息。任何更新都必须满足：

1. **可追溯**: 知道什么被改了、为什么改、谁改的
2. **可回滚**: 改错了可以恢复
3. **可确认**: 高风险变更需要用户明确同意

## 决策

采用 **Proposal-based Patch 机制**：

1. **直接应用路径**（兼容/快速）:
   - Builder `apply_signals` 直接修改 LifeModel
   - 适用于低风险字段（如 state.current_focus）
   - 自动创建 Snapshot（版本控制）

2. **Proposal 路径**（推荐/安全）:
   - Builder/Calibration 生成 `AgentProposal`
   - 包含 before/after/risk_level/confidence
   - 用户必须在 Review Center 确认后才应用
   - 应用前自动创建 Snapshot

## 后果

### 正面

- ✅ 所有 LifeModel 变更都有审计日志
- ✅ 高风险字段（价值观、使命）必须经过用户确认
- ✅ 支持回滚（通过 Snapshot）
- ✅ Builder 和 Calibration 统一使用同一套机制

### 负面

- ⚠️ 增加了用户操作步骤（需要审阅 Proposal）
- ⚠️ 需要额外的存储（proposals.db）
- ⚠️ 直接应用路径仍作为兼容保留，可能被误用

## 替代方案

| 方案 | 说明 | 拒绝原因 |
|------|------|----------|
| 直接写入无确认 | 所有变更立即应用 | 高风险，无法回滚 |
| 全字段强制确认 | 所有变更都必须走 Proposal | 体验差，低风险字段不需要 |
| 自动确认（置信度>阈值） | 高置信度自动应用 | 不够安全，用户可能不知情 |

## 相关

- [ADR 0002: Proposal 统一层](./0002-proposal-unified.md)
- [ADR 0003: AgentRun 追踪](./0003-agent-run-tracking.md)
- `openlife-core/src/agent/proposal_store.rs`
- `src-tauri/src/commands/proposal.rs`
