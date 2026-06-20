# OpenLife 剩余任务规划

> Historical sprint/debt plan. Do not use as current Agent development
> authority without re-checking code and `plans/README.md`.
> Current order is defined by
> `plans/openlife_lifemodel_governed_agent_runtime.md`.

> 历史背景：基于早期代码审查，制定旧阶段详细执行计划
> 文档版本：2026-05-05
> 目标：完成 Beta 发布前的所有技术债务清理

---

## 一、历史状态快照

### 1.1 已完成（Sprint 1-4）

- **死代码清除**：orchestrator.rs (-2,195), streaming.rs (-113)
- **阻塞级修复**：启动 panic、NaN panic、SQL 注入、Privacy 日志洪水
- **AppError 迁移**：12/24 文件完成（metrics, life_model, feedback, storage, a2a, version, mcp, memory, diagnostics, state, execution, calibration, settings）
- **架构改进**：StoreManager 创建、ContextAssembler Arc 优化、AgentLoop 状态统一
- **测试基础设施**：Playwright E2E、Criterion 基准、vitest coverage provider
- **代码去重**：json_utils 共享模块

### 1.2 剩余任务统计

| 类别 | 数量 | 优先级 | 预估工时 |
|------|------|--------|----------|
| AppError 迁移（proposal.rs） | 25 处 | P0 | 4-6h |
| AppError 迁移（lib.rs） | 18 处 | P0 | 3-4h |
| AppError 迁移（router.rs） | 1 处 | P1 | 15min |
| VectorStore 真实索引 | 1 项 | P1 | 8-12h |
| 硬编码值配置化 | 8 处 | P1 | 2-3h |
| 函数体积重构 | 4 个 | P2 | 6-8h |
| E2E 测试扩展 | 3 场景 | P2 | 4-6h |

**总预估工时**：27.25 - 39.25 小时

---

## 二、详细执行计划

### Phase 1: AppError 统一收口（P0，2 天）

**目标**：消除所有剩余的 `Result<..., String>`，实现错误处理 100% 统一

#### 任务 1.1: proposal.rs AppError 迁移

**当前状态**：25 个函数仍使用 `Result<..., String>`
**影响范围**：最大的命令模块（1,809 行），Proposal 系统的核心
**迁移策略**：

```rust
// 迁移模式
// Before:
pub async fn accept_proposal(...) -> Result<serde_json::Value, String> {
    store.get_proposal(id).map_err(|e| e.to_string())?
    // ...
    Err(format!("Proposal 不存在：{}", id))
}

// After:
pub async fn accept_proposal(...) -> Result<serde_json::Value, AppError> {
    store.get_proposal(id).map_err(AppError::from)?
    // ...
    Err(AppError::not_found(format!("Proposal 不存在：{}", id)))
}
```

**关键注意点**：
1. `format!()` 错误需根据语义选择 AppError 变体：
   - 资源不存在 -> `AppError::not_found(...)`
   - 验证失败 -> `AppError::validation(...)`
   - 数据库错误 -> `AppError::db(...)`
   - 权限拒绝 -> `AppError::permission(...)`
   - 其他 -> `AppError::internal(...)`
2. `safe_write_utf8` 等内部函数也需迁移
3. 测试中的 `unwrap()` 调用需更新

**验收标准**：
- [ ] `grep "Result<.*String>" proposal.rs` 返回 0 结果
- [ ] `cargo test -p openlife-tauri --lib` 全部通过
- [ ] Proposal 相关功能手动测试通过

#### 任务 1.2: lib.rs AppError 迁移

**当前状态**：18 个函数/位置使用 `Result<..., String>`
**影响范围**：核心聊天路径、store 初始化、工具执行
**关键函数**：
1. `init_memory_store` 等 5 个初始化函数（line 213-337）
2. `persist_life_model`（line 421）
3. `try_auto_checkin_daily_goals`（line 485）
4. `persist_chat_message_if_needed`（line 570）
5. `send_message` / `send_message_with_agent_loop`（line 1203, 1366）
6. `start_stream_message` 系列（line 1610, 1767, 2027）
7. `execute_tool_call_internal`（line 2657）
8. `inspect_mcp_arguments`（line 2758）
9. `init_store` helper（line 2795）

**迁移策略**：
- 将 `init_store` 的签名从 `Result<T, String>` 改为 `Result<T, AppError>`
- 所有调用 `init_store` 的初始化函数同步更新
- 聊天路径的错误使用 `AppError::external()` 包装外部服务错误

**验收标准**：
- [ ] `grep "Result<.*String>" lib.rs` 返回 0 结果（排除非函数签名的字符串）
- [ ] `cargo check` 零错误

#### 任务 1.3: router.rs AppError 迁移

**当前状态**：1 个函数 `get_model_router_status` 使用 String
**工作量**：15 分钟

---

### Phase 2: 性能基础设施（P1，3 天）

#### 任务 2.1: VectorStore 索引实现

**当前状态**：O(n) 暴力扫描，2000 条硬限制
**目标**：实现 O(log n) 或 O(1) 的向量检索

**技术选型**：
| 方案 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| sqlite-vss | 零外部依赖，SQLite 原生 | 需要编译 SQLite 扩展 | 首选 |
| faiss-rs | 工业级性能 | 增加系统依赖 | 备选 |
| 自定义 KD-Tree | 纯 Rust | 高维效果差 | 不推荐 |
| 保持现状 | 简单 | 2000条后性能崩溃 | 不可接受 |

**推荐方案**：sqlite-vss
- 使用 `sqlite-vss` 扩展提供 SQLite 原生向量索引
- 保留现有 Schema，添加虚拟表索引
- 需要更新 `Cargo.toml` 添加 `sqlite-vss` 依赖或自编译扩展

**实施步骤**：
1. 调研 `sqlite-vss` 在 Rust 中的绑定（`sqlite-vss-rs` 或手动加载扩展）
2. 在 `vectors.rs` 中实现索引创建逻辑
3. 将 `search` 方法从暴力扫描切换到索引查询
4. 移除 2000 条硬限制
5. 更新基准测试，对比索引前后性能

**验收标准**：
- [ ] 10000 条向量检索 < 100ms（当前 ~500ms）
- [ ] 移除 MAX_VECTORS 硬限制
- [ ] 所有向量测试通过
- [ ] 基准测试显示 5x+ 性能提升

#### 任务 2.2: 硬编码值配置化

**当前状态**：8 处硬编码值散落在代码中
**目标**：全部提取到 `AppConfig`

| 硬编码值 | 位置 | 配置键建议 | 默认值 |
|----------|------|------------|--------|
| 11434 | model_router.rs, ollama.rs | `ollama_port` | 11434 |
| 8765 | a2a_sidecar.rs | `a2a_port` | 8765 |
| 100KB | proposal.rs (safe_write_utf8) | `max_safe_file_size` | 1048576 |
| 2000 | vectors.rs | `max_vectors` | 10000 |
| 60s | a2a_sidecar.rs | `a2a_start_timeout` | 60 |
| 30s | lib.rs (start_stream_message) | `stream_timeout` | 30 |
| 5s | model_router.rs (health timeout) | `provider_health_timeout` | 5 |
| 1000 | vectors.rs (embedding cache) | `embedding_cache_size` | 1000 |

**实施步骤**：
1. 在 `openlife-core/src/config.rs` 的 `AppConfig` 中添加新字段
2. 更新 `SystemConfig` 或创建新的配置段落
3. 逐一替换硬编码值
4. 更新配置文档

**验收标准**：
- [ ] `grep -rn "11434\|8765\|100KB\|2000" src/ openlife-core/src/` 无业务逻辑硬编码（允许配置定义处）
- [ ] `config.yaml` 模板包含所有新配置项
- [ ] 向后兼容：缺少配置时使用默认值

---

### Phase 3: 代码质量深化（P2，2-3 天）

#### 任务 3.1: 函数体积控制

**当前状态**：4 个超大型函数

| 函数 | 文件 | 行数 | 建议拆分 |
|------|------|------|----------|
| `start_stream_message` | lib.rs | 632 | 拆分为 5-6 个子函数 |
| `run` | agent_loop.rs | 402 | 已拆分为 execute_action 等 |
| `apply_proposal` | proposal.rs | 210 | 按 proposal_type 分派 |
| `send_message` | lib.rs | 163 | 提取准备/执行阶段 |

**拆分策略 - start_stream_message**：
```rust
// 拆分为：
fn prepare_stream_context(...) -> Result<StreamContext, AppError>;
fn build_system_prompt(...) -> String;
fn execute_stream_loop(...) -> Result<(), AppError>;
fn handle_stream_completion(...) -> Result<(), AppError>;
fn emit_stream_event(...) -> Result<(), AppError>;
```

**验收标准**：
- [ ] 无函数超过 200 行
- [ ] 每个子函数有单一职责
- [ ] 测试覆盖率不下降

#### 任务 3.2: E2E 测试扩展

**当前状态**：5 个 smoke 场景（页面加载、导航、主题切换）
**目标**：覆盖核心用户旅程

**新增场景**：
1. **Chat 完整流程**：
   - 打开应用 -> 进入 Chat -> 发送消息 -> 等待回复 -> 验证消息显示
   - 测试流式输出和最终渲染

2. **Builder 引导流程**：
   - 进入 Builder -> 完成 LifeModel 创建 -> 验证 Dashboard 显示
   - 测试 Proposal 生成和确认

3. **Proposal 审批流程**：
   - 触发 Proposal -> 进入 Review -> 接受/拒绝 -> 验证状态变化
   - 测试 Safe Mode 拦截

4. **Settings 配置持久化**：
   - 修改设置 -> 重启应用 -> 验证设置保留

**实施步骤**：
1. 在 `frontend/e2e/` 添加新测试文件
2. 为需要 mock 的 Tauri 命令添加 `page.evaluate()` 注入
3. 添加数据清理（测试隔离）

**验收标准**：
- [ ] 8+ E2E 场景覆盖核心用户旅程
- [ ] 测试运行稳定（无 flaky）
- [ ] CI 中集成 `playwright test`

---

## 三、执行时间表

### 建议排期（总计 7-8 天）

```
Day 1-2:  Phase 1 - AppError 迁移
  - 上午：proposal.rs（25处）
  - 下午：lib.rs（18处）
  - 晚上：router.rs + 全面测试

Day 3-5:  Phase 2 - 性能基础设施
  - Day 3: VectorStore 索引调研 + 原型
  - Day 4: VectorStore 索引集成 + 基准测试
  - Day 5: 硬编码值配置化

Day 6-7:  Phase 3 - 代码质量
  - Day 6: 函数体积重构
  - Day 7: E2E 测试扩展

Day 8:   缓冲 + 回归测试
  - 全面测试运行
  - 文档更新
  - make ci 验证
```

### 里程碑

| 里程碑 | 日期 | 验收标准 |
|--------|------|----------|
| AppError 100% | Day 2 | `grep -r "Result<.*String>" src/ openlife-core/src/` 返回 0 |
| 索引上线 | Day 5 | 10000 条向量 < 100ms |
| Beta Ready | Day 8 | `make ci` 通过，所有测试通过 |

---

## 四、风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| proposal.rs AppError 迁移引入回归 | 中 | 高 | 分批次迁移，每批测试；保留功能测试 |
| sqlite-vss 编译问题 | 中 | 高 | 准备 faiss-rs 备选方案；保留暴力扫描 fallback |
| 函数拆分破坏逻辑 | 低 | 中 | 提取而非重写；使用编译器验证 |
| E2E 测试不稳定 | 高 | 低 | 添加重试机制；隔离测试数据 |
| 时间超支 | 中 | 中 | Phase 2/3 可并行；VectorStore 索引可延至 Beta 后 |

---

## 五、验收清单

### 5.1 技术债务清零

- [ ] 零 `Result<..., String>`（除第三方库边界外）
- [ ] 零 `panic!` / `expect()`（除不可恢复错误外）
- [ ] 零硬编码业务常数
- [ ] 零超过 200 行的函数
- [ ] 死代码扫描：无未使用的 pub 函数

### 5.2 质量门控

- [ ] `cargo test -p openlife-core`：全部通过
- [ ] `cargo test -p openlife-tauri --lib`：全部通过
- [ ] `cd frontend && pnpm test`：全部通过
- [ ] `cd frontend && pnpm exec playwright test`：全部通过
- [ ] `make ci`：通过

### 5.3 性能基准

- [ ] VectorStore 10000 条检索 < 100ms
- [ ] 内存占用无显著增长（索引加载）
- [ ] 启动时间 < 3 秒

---

## 六、附录

### A. 快速命令参考

```bash
# 检查剩余 String 错误
grep -rn "Result<.*String>" src-tauri/src/ openlife-core/src/

# 检查硬编码值
grep -rn "11434\|8765\|100KB\|2000" src-tauri/src/ openlife-core/src/

# 检查函数体积（按行数排序）
wc -l src-tauri/src/lib.rs src-tauri/src/commands/*.rs openlife-core/src/**/*.rs | sort -n

# 完整测试套件
make ci
```

### B. 相关文档

- [AGENTS.md](/Users/fujing/Desktop/偶来福/AGENTS.md) - 项目上下文指南
- [openlife_development_plan.md](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md) - 开发路线图
- [openlife_react_beta_roadmap.md](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md) - Beta 路线图
- [OpenLife_PRD_v2_Agent_Framework.md](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md) - 产品需求
