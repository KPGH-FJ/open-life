# OpenLife 项目现实审计与发展指导报告

> - 审计日期：2026-07-22（Asia/Shanghai）
> - 审计性质：生产源码只读、源码优先、本地与远端交叉核验
> - 当前审计 SHA：`83dc3ac`（`codex/phase4f-desktop-product-acceptance`）
> - 远端主线 SHA：`7a167f4`（`origin/main`）
> 总体结论：不能回到旧 V4 分支续跑，也不应推倒重写；应以最新 `origin/main` 为唯一代码基线，先吸收当前 Phase4F 修复，再把 V4 的 72 项发现重组为少量根因计划。架构已明显收敛，但当前仍有 P0/P1 级密钥、状态真相、跨语言契约、路径边界和真实 E2E 问题。

## 1. 执行摘要

OpenLife 不是普通聊天应用，也不只是人生规划工具。它当前最准确的定义是：一个本地优先、以私人 LifeModel 为上下文和治理对象、允许本地或云模型参与对话、规划、写作、读取、工具执行和用户批准后状态更新的桌面个人 Agent OS。

经过长期 AI Coding，项目已经形成了真实而非空壳的技术系统：Tauri 2 桌面壳、React 工作台、Rust Core、SQLite 多类存储、模型路由、ReAct/AgentLoop、工具/MCP、Memory、LifeModel、Proposal/Review、任务恢复、证据与审计机制都存在实际实现与测试。当前代码量约 43 万行，已经远超原型规模。

项目也经历了一次正确且必要的方向修正：从历史上的 Stage/Beta/Migration/多运行时并存，转向 Phase7 的 single-system deletion。当前普通 Main Chat 的 send/stream 已共享 `OpenLifeTurnRuntime`；语义与治理路由集中到 `IntentFrame + PolicyRouter`；Proposal、Memory、LifeModel、Tool 分别通过受控网关；前端生产入口已切换到统一 Workbench，并删除旧页面树和兼容重定向。这些不是文档愿景，而有源码、静态禁用测试和当前绿门支持。

但项目仍不能被描述为“完成”“Beta ready”或“可以稳定试用”。最新真实 Tauri 试用只证明：新壳和六个规范路由可启动；当后端如实传播 unknown/expired/error 时，前端能 fail closed；Settings、错误呈现、焦点管理等问题被真实发现并修复。它没有证明权限到执行、提案到持久化、外部 Provider、长期重启恢复等关键闭环，而且后端目前仍有把 store 错误折叠成 `0/ok/Ready` 的路径。凭据恢复在当前进程显示可用，但重启后仍回到 Safe Mode；dev/release/qa 文件系统隔离了，Keychain 命名空间却未隔离。

因此，本报告的核心判断是：

1. 产品理念与治理原则没有根本偏移，甚至比早期更清晰。
2. 实现已从“功能堆积失控”转向“单系统收敛”，方向正确。
3. 主要 AI Coding 偏移已经转化为结构性复杂度：超大文件、过多状态/证据层、文档数量失控、测试和合同替代真实产品证明、分支/工作树碎片化。
4. 旧 V4 分支相对主线已经是 `13 ahead / 195 behind`；只读虚拟合并在 7 个关键文件产生冲突，直接续跑会把历史实现和当前系统重新混在一起。
5. 当前不应继续扩展功能面；应进入一轮“安全边界与产品闭环优先的复杂度偿还”。
6. 下一开发计划不应叫 Phase8、新 Beta 或笼统的 V5，而应是两个顺序门：`Restart Baseline -> Trial Green 1`。先关闭 P0 和会伪造真相、阻断闭环的 P1，再用真实桌面产品完成 3 条可重复、可重启、可审计的纵向闭环。

## 2. 审计范围、方法与证据边界

### 2.1 已核对

- 当前权威链：`AGENTS.md`、`plans/README.md`、Phase7 删除清单、single-system 开发准备。
- 本地全部 Git refs、分支、标签、worktree 指向、近期历史。
- 执行 `git fetch --all --prune --tags` 后的全部远端跟踪分支。
- 远端公开仓库、开放 PR、Actions 运行摘要、开放 Issues。
- Rust workspace、Tauri handler、Main Chat send/stream、TurnRuntime、Kernel、PolicyRouter、各网关、状态投影、前端生产入口与路由。
- 当前 Phase4F 原生试用报告、缺陷登记和截图证据索引。
- Backend Remediation v4 的冻结发现、追加发现和追踪状态。
- 当前 SHA 上的全 workspace Rust 测试、严格 Clippy、前端全量单测、构建、依赖审计和 Playwright 收集/现行 smoke。
- 27 个本地 worktree、42 个本地分支、Cargo 派生产物占用和关键 Git 历史/大提交。

这里的“全仓审计”指：覆盖全部一层源码/构建/测试表面的 source map，并对高风险入口、运行链、持久化边界和跨语言合同做调用链追踪，再用 gate、历史和真实试用交叉核对；它不是逐行形式化证明，也不意味着所有潜在缺陷已经被发现。未被上述方法覆盖到的缺陷仍是 `UNKNOWN`。

### 2.2 没有被冒充为已验证的内容

- 未使用真实外部模型密钥，未发起真实外部 Provider 调用。
- 未执行真实权限批准后外部写入。
- 未创建或批准真实 LifeModel/Memory 提案并跨重启核对落盘。
- 未进行 VoiceOver 人工审查。
- 未做两个真实进程争用同一密钥/数据库的故障注入，也未做 100+ Proposal、200+ Task、10,000+ audit 的规模压测。
- 未做 snapshot IPC 利用、真实 MCP secret 泄漏、symlink TOCTOU 或负 retention 删除等破坏性复现；这些按源码确认或候选风险标注。
- GitHub CLI 因本机 Keychain 登录超时未能读取 API；远端 PR/Actions 使用公开网页核对。
- CodeRabbit CLI 在本机不可用，因此没有把 CodeRabbit 输出冒充第二方审查；源码问题由手工调用链追踪、并行独立复核和真实 gate/试用交叉确认。
- “全部远端”限于 Git remote 中已配置的 `origin` 及其公开 GitHub 表面；不存在的私有服务、未配置 remote 或未提交本地资料不可被推断。

### 2.3 防幻觉分级

- `REPRODUCED`：本次命令或真实试用已经触发。
- `SOURCE-CONFIRMED`：入口、输入、错误分支和影响链在当前源码上闭合，但没有执行破坏性/外部复现。
- `TRIAL-OBSERVED`：已有真实 Tauri 试用和截图记录，本次重新核对了代码与报告。
- `CANDIDATE`：存在危险结构，但普通产品可达性或最终影响仍需 fault injection。
- `UNKNOWN`：本次证据不足，禁止当作完成或不存在。

## 3. 仓库与远端现实

### 3.1 当前分支关系

| 层级 | 当前事实 | 判断 |
| --- | --- | --- |
| 远端长期主线 | `origin/main = 7a167f4` | 已合入 Phase4E 前端原子切换 |
| 当前本地/远端 PR 分支 | `83dc3ac`，比 main 多 2 个提交 | Phase4F 真实桌面验收与修复，尚未合入 |
| 开放 PR | GitHub PR #64 | 唯一开放 PR，等待人类合并审查 |
| 远端 main CI | `7a167f4` 的 Frontend、Rust、Smoke、Security 等公开 jobs 成功 | 证明主线机械门为绿，不等于产品 Trial Green |
| PR #64 | 开放、2 个提交、尚无人类 review | PR 当前检查状态未通过已认证 API 复核，记为 `UNKNOWN` |
| Roadshow 冻结输入 | tag `backend-freeze-c9e75c8` | 已被主线后续历史吸收，作为冻结证据而非第二主线 |
| Backend Remediation WIP | 多个历史 slice/local/remote ref | 物理 worktree 已清理；refs 只作语义或证据参考，不能逐个视作待合并产品分支 |

`origin/main..HEAD` 的两个独有提交是：`52571d0`（Phase4F 前端修复）与 `83dc3ac`（试用记录/证据）。`openlife-core/`、`src-tauri/` 相对 main 没有差异；本地独有生产改动集中在 frontend，另外是 docs/plans 和试用截图。工作树相对 HEAD 没有未提交生产源码修改；本报告自身是新增的未跟踪审查产物。

远端仓库公开，GitHub 显示 1 个开放 PR；公开 backlog 与仓库内 72 项 V4 发现明显脱节，真正的 backlog authority 目前仍在仓库 JSON/Markdown，而不是 GitHub Issues。

V4 不能直接续跑还有一条机械证据：`git merge-tree --write-tree origin/main codex/wip-openlife-backend-remediation-v4` 在 `main_chat_agent_v1.rs`、`main_chat_event_stream.rs`、`main_chat_turn_runtime.rs`、`main_chat_runtime_module_tests.rs` 和三份 V4 traceability/finding JSON 上产生冲突。V4 的 13 个独有提交应逐项做 semantic port/closure review，不能合并整个分支。

### 3.2 规模与集中度

统计口径：文件数来自 `git ls-files`；行数只统计表中标出的 tracked 文本扩展名。未跟踪的本报告不进入下面的 docs 数字。

| 区域 | tracked 文件数 | tracked 文本行数 |
| --- | ---: | ---: |
| `openlife-core/src` | 132 | 202,754 `.rs` |
| `src-tauri/src` | 107 | 193,371 `.rs` |
| `frontend/src` | 129 | 39,836 `.ts/.tsx/.css` |
| `frontend/e2e` | 3 | 2,129 `.ts` |
| `docs` | 418 | 23,795 `.md/.json` |
| `plans` | 214 | 71,480 `.md/.json` |

最大源码文件包括：

- `main_chat_agent_v1.rs`：23,096 行；
- `main_chat_kernel.rs`：18,688 行；
- `main_chat_event_stream.rs`：15,954 行；
- `agent/store.rs`：13,176 行；
- `tasks.rs`：12,868 行；
- `main_chat_turn_runtime.rs`：11,971 行；
- `main_chat_command_surface_tests.rs`：11,526 行。

这说明系统不是“没有实现”，而是实现密度过高。单个文件承担合同、状态机、持久化、错误、证据和测试辅助的概率很高。对于 AI Coding 项目，这会放大局部修复的不可预测性，也让后续 Agent 更依赖关键词搜索和旧文档，而不是能稳定建立心智模型。

### 3.3 “许多派生文件”的真实来源

审计开始时测量得到：27 个 registered worktree、42 个本地分支，但只有两个 worktree 还保留 Cargo `target/`；两者合计约 76.6 GiB：

| 路径 | 约占用 | 主要内容 |
| --- | ---: | --- |
| `/Users/tw/Desktop/open-life-roadshow/target` | 56.2 GiB | `debug/deps` 42.1 GiB、`debug/incremental` 13.5 GiB |
| `/Users/tw/Desktop/open-life/target` | 20.4 GiB | `debug/deps` 11.2 GiB、`debug/incremental` 8.5 GiB |

这些是 Rust/Cargo 编译依赖和增量缓存，不是需要“恢复”的 V4 源文件，也没有被 Git 跟踪。本次完整测试本身会继续增长当前 `target/`。同时，未被生产调用的 `ReflexEngine` 仍无条件引入 `tokenizers`、`tract-onnx`、`ndarray`，对应缓存约 939 MiB；如果只清缓存、不删除或 feature-gate 死依赖，它还会重新生成。

`make clean` 只删除前端临时目录、测试结果子集、`target/openlife-dev` 和 `src-tauri/target`，不会清理 workspace 根 `target/`；真正释放 Cargo 空间的是 `make clean-rust-target` / `cargo clean`。

在完成 clean/ref/process/evidence 双重核对后，本次随后执行了受控清理：

- 删除 26 个旧 registered worktree checkout，保留唯一 `/Users/tw/Desktop/open-life`；旧分支 refs 没有随 checkout 删除。
- 删除 `open-life-roadshow` 的 56.2 GiB Cargo target；其 HEAD 已在 main，远端 ref 与 freeze tag 仍在。
- 用 `cargo clean --target-dir` 删除 4 个只包含 Cargo `debug/tmp/CACHEDIR.TAG` 的 review target，共约 27.9 GiB。
- 删除 15 个已经合入 main 的冗余本地分支；本地分支从 42 降至 27，26 个未合入 ref 暂留给 V4 语义分类。
- 在 `.git/cleanup-backups/openlife-all-refs-before-single-worktree-20260722.bundle` 保存并验证了清理前全部 109 个 refs。
- APFS 可用空间从约 8 GiB 增至约 92 GiB；当前正在运行的主仓库 `target`、`.env*`、试用截图和 `frontend/test-results` 未触碰。
- `AGENTS.md` 已加入唯一可写 checkout 规则：未经用户当次明确授权，不得再创建 roadshow、D0xx 或其他 sibling worktree。

桌面还保留 `open-life-uiux-audit-2026-06-21`，它不是 Git worktree，而是独有报告/截图证据，不作为开发入口。现在 Git 层面只有一个物理开发入口。

## 4. 产品与架构的真实认知

### 4.1 产品核心

OpenLife 的差异化不在于多一个聊天 UI，而在于四个组合：

1. 私人 LifeModel：身份、目标、状态、偏好、能力、关系与演化规则。
2. 受治理的 Agent Runtime：模型不能把“说了”当“做了”，写入和外部动作需要权限与证据。
3. 长期 Memory/反馈/成熟化：从对话和事件中形成候选事实、经验和规则，但不静默提升为真相。
4. 本地优先的可审计执行：本地存储、隐私路由、任务/事件/收据、恢复与回滚。

这个产品定义仍然成立，并且代码已经围绕它形成了大量真实基础设施。

### 4.2 当前主运行链

```text
React Workbench
  -> frontend/src/tauri.ts
  -> Tauri command: send_message / start_stream_message
  -> OpenLifeTurnRuntime
       -> AgentRun / TaskSession / transcript / event evidence
       -> IntentFrame
       -> PolicyRouter
       -> Main Chat Kernel
            -> DirectAnswer
            -> governed read / AgentLoop
            -> Plan draft
            -> Memory or LifeModel Proposal
            -> permission / network consent / blocker
       -> ToolGateway / ReviewWorkflow / MemoryGateway / LifeModelWriteGateway
       -> canonical FinalDelivery + backend-owned ViewModels/Projection
  -> Today / Workspace / Tasks / Review / LifeModel / Settings
```

send 与 stream 是并列传输入口，不互相调用；它们共同委托同一 Runtime owner。`src-tauri/src/lib.rs` 是命令注册和 AppState 组合位置，不是 Main Chat 运行时本身。

### 4.3 路由与模型

`IntentFrame` 表达用户语义，`PolicyRouter` 决定允许的产品动作，`model_router` 决定本地/云 Provider。这个三层区分是正确的：语义、治理、模型选择不应混成关键词判断。

DirectAnswer、读取、工具、计划、提案、阻塞不再被视作多个平行产品运行时，而是同一 Runtime 的不同状态。ReAct/AgentLoop 主要服务于多步读取和工具观察；模型选择的工具必须落在受控 candidate/target/action allowlist 中，失败时应产生 blocker，而不是隐藏 fallback completion。

### 4.4 Proposal、权限与耐久写入

系统最有价值的工程原则是“assistant prose 不是写入授权”。当前目标权威是：

- `ReviewWorkflow`：Proposal 创建与生命周期的治理入口；
- `ProposalStore`：存储，不应成为第二业务权威；
- `MemoryGateway`：Memory lane 分类与写入；
- `LifeModelWriteGateway`：LifeModel 规范写入；
- `ToolGateway`：工具执行合同与权限；
- `TerminalOwnerWriteGateway`：终态与执行事实线性化；
- `LifeStateProjection` 和 typed ViewModel：前端产品状态来源。

Proposal 被创建不等于 durable change 完成；accepted 也必须与 applied/materialized 区分。当前静态测试已经对这些边界提供较强保护。

### 4.5 LifeModel 与 LifeModel-HS

当前 LifeModel 仍有 YAML compatibility/materialized view，代码也有 patch、patch store、SQLite 状态、Memory lifecycle、evidence、heuristic/policy 等多类资产。ADR 0013 描述的 LifeModel-HS 是目标治理架构，不代表所有资产已经完成 canonical migration。

因此最准确的说法是：OpenLife 已有“受治理的 LifeModel + HS 资产方向”，但还没有完成一个单一、全面、经过真实迁移/回滚验证的 LifeModel-HS source of truth。YAML 不应被重新升级为规范真相；同时也不能因为存在 HS 类型和测试就宣称迁移完成。

### 4.6 Memory

Memory 已区分 turn context、episodic event、semantic fact/preference、procedural rule、evidence、canonical LifeModel truth。普通聊天不是自动长期记忆；低置信候选不进入耐久路径；未来规则、身份/偏好类事实倾向进入 Proposal。

工程上仍同时存在 MemoryStore、向量、热缓存、MemoryLifecycle、Evidence、LifeEvent 等表面。当前网关方向正确，但多存储的一致性、删除 tombstone、向量回退、隐私副本和跨存储恢复仍是后端发现的重要来源。

### 4.7 工具、Web、MCP 与外部 Provider

ToolManifest 有显式 capability/risk/action/permission 合同；ToolGateway 对不完整合同、disabled/declarative-only、不可用执行器等 fail closed。MCP 注册和命令也有限定。

本地 HTTP compatible provider proof、scripted provider、fixture web、mock IPC 能证明 plumbing 和治理形状，但不等于外部真实能力。完整信用仍要求：

- 外部 live Direct generation；
- live Web AgentLoop；
- live registered MCP AgentLoop；
- live permission/proposal 场景；
- 每个场景有真实 run/task/trace 且无 silent write/fallback。

截至本审计，这些仍未完整完成。

### 4.8 前端现实

Phase4E 已把生产 UI 切换到 `OpenLifeWorkbenchShell`，规范路由只有：

- `/today`
- `/workspace`
- `/tasks`
- `/review`
- `/life-model`
- `/settings`

旧 `/companion`、`/mailbox`、`/runs`、`/memory`、`/builder`、`/mcp`、`/a2a` 等明确显示“已退役/不可用”，不兼容重定向。产品 Journey 分为 read-only、governed action、durable truth、settings/privacy，方向比旧页面级拼装更清楚。

当前前端强项是：对收到的未知/过期/错误状态 fail closed、backend-owned ViewModel、Inspector 证据、可访问性焦点与 live region。弱项是：后端并不总会把 store 错误如实传播；真实后端状态不足时，大量路径也只能显示 Safe Mode、空状态或不可用。因此视觉/合同完成度显著高于真实任务完成度。

## 5. 当前验证结果

### 5.1 本次重新执行

| Gate | 结果 |
| --- | --- |
| tracked worktree `git diff --check` | PASS；本报告另经 `git diff --no-index --check` |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all --locked -- -D warnings` | PASS |
| `cargo test --all --locked` | PASS；全 workspace 无失败，Tauri 主 lib 最终段为 1,179 passed / 13 ignored |
| 前端 typecheck / format check / production build / absence guard | PASS |
| 前端 Vitest | 37 files / 273 tests PASS |
| `pnpm audit --json` | 1 critical / 5 high / 7 moderate / 1 low |
| `cargo audit --no-fetch` | 未报告已知 vulnerability；17 unmaintained + 1 unsound warning |
| `cargo audit` 在线模式 | advisory DB 已更新；crates.io yanked 查询超时，因此 yanked 状态 `UNKNOWN` |
| `playwright test --list` | FAIL；2 个已删除模块 import，0 tests collected |
| 当前 `e2e/smoke.spec.ts` | 5/5 FAIL；全部仍断言旧路由/旧文案 |

这些结果证明编译、静态合同和大量单元/集成测试相当健康；同时 Playwright 与本次源码问题证明，绿门没有覆盖真实 Rust 序列化 shape、枚举穷尽、运行期 store 故障、跨进程密钥争用和当前产品路由。这是“测试很多但证据错位”，不是“项目没有测试”。

### 5.2 Phase4F 原生试用

已证明：

- 真实打包 Tauri 产品启动；
- 六个规范路由可达；
- Today/Workspace/Tasks/LifeModel/Settings 的未知、错误、过期状态不伪装成完成；
- Settings 脱敏配置不再崩溃；
- 结构化 Tauri 错误不再显示 `[object Object]`；
- 路由焦点、Inspector 焦点返回、live region 修复；
- Safe Mode 恢复动作可达且有双重确认。

未证明或失败：

- permission -> review -> refresh -> resume：BLOCKED；
- proposal -> decision -> application：BLOCKED；
- provider test -> save -> boundary refresh：BLOCKED；
- credential recovery -> restart：FAILED_FAIL_CLOSED；
- external provider：BLOCKED；
- VoiceOver 人工验证：BLOCKED。

所以当前 `PHASE4F_COMPLETE=NO`、`PHASE7_TRIAL_GREEN=NO` 是正确结论。

## 6. 当前源码问题清单

### 6.1 先看总账，而不是把“implemented”当“closed”

当前 V4 discovered finding registry 已有 72 项，而不是最初的 35 项。追踪 JSON 的机械统计为：

| 维度 | 统计 |
| --- | --- |
| implementation | 41 implemented / 24 in progress / 7 not started |
| verification | 10 complete / 55 partial / 7 none |
| closure | 64 open / 7 closure candidate / 1 independently verified |

因此，“做过代码”远多于“独立证明关闭”。下面是本次从当前 `origin/main`/HEAD 源码重新确认的高信号问题；它们不是用来另建第二份 72 项 backlog，而是用于把旧 finding 重组为根因计划。完整历史 finding 仍以 `plans/openlife_backend_remediation_v4_discovered_findings.json` 和 `plans/openlife_backend_remediation_v4_discovered_traceability.json` 为底稿。

### 6.2 P0 / 恢复开发前必须处理

#### P0-01 MCP audit 密钥、profile/store identity 与单写者没有绑定

`SOURCE-CONFIRMED`，尚未执行双进程破坏性复现。

- `src-tauri/src/secret_store.rs` 的 service 固定为 `com.openlife.desktop`，MCP account 只包含数字 epoch；`dev`、`qa`、`release` 和自定义数据目录没有进入引用。
- `src-tauri/src/storage.rs` 明确把文件系统数据分到 `ai.openlife.app`、`.dev`、`.qa`，形成“数据库隔离、Keychain 不隔离”。
- `openlife-core/src/mcp_audit.rs` 的 store 可 Clone，并按操作重新打开连接；当前没有跨进程 writable-owner lease。
- 两个创建者可同时观察 key 不存在，随后覆盖同一个 Keychain account；已写 ciphertext 可能失去可解密密钥。
- Main Chat、AgentLoop、Scheduler 的 ToolGateway 都使用该 audit store；V4 `BR4-D064` 仍是 `not_started/open`。

这不是普通开发便利问题，而是审计真相与恢复能力的底层完整性问题。

#### P0-02 开发凭据身份不能形成可重复重启基线

`TRIAL-OBSERVED + SOURCE-CONFIRMED`。

- Phase4F 的交互恢复在当前进程中把四类凭据显示为 available，但完全退出并以 ad-hoc bundle 或 `make dev` 重启后仍回到 Safe Mode。
- 现有 macOS Keychain ACL 绑定旧 worktree executable/cdhash；重新编译的 ad-hoc binary 身份变化。
- 所有 provider、search、event integrity、action queue、task store、run receipt、MCP audit 引用又共用固定 service/account 命名空间，进一步放大 profile 迁移风险。

在稳定身份、可回滚迁移和两次完整进程重启通过之前，外部 Provider、durable proposal 和权限恢复都不能获得产品信用。

#### P0-REMOTE 远端 main 仍保留真实 Settings 崩溃，本地 PR 才修复

`REPRODUCED/TRIAL-OBSERVED` 于 Phase4F；当前本地 HEAD 已修，`origin/main` 未修。

- Rust `LlmConfig.openai_key` 使用 `skip_serializing`；真实 `get_config` 不会返回该 secret 字段。
- `origin/main` 的 `frontend/src/tauri.ts` 把它声明为必填，Settings presentation 对 `undefined` 直接 `.trim()`。
- 单测使用手造的 `"***"` fixture，掩盖真实 Rust shape。
- 当前提交 `52571d0` 已把字段改为可选并使用 `openai_key_ref`；PR #64 尚未 review/merge。

恢复开发若直接从 `origin/main` 新开分支而不先吸收这项修复，会重新带回崩溃。

### 6.3 P1 / 当前主线的安全、真相和产品阻断问题

#### P1-01 LifeStateProjection 把读取失败伪装成 0/ok/idle

`SOURCE-CONFIRMED`，未做运行期数据库故障注入。

- `src-tauri/src/life_state_projection.rs` 在 Proposal、Task、ToolPermission 查询失败时回退空集合/零计数，随后仍写 `proposal_store_status="ok"`、`task_store_status="ok"`。
- `src-tauri/src/commands/diagnostics.rs` 也把 ProposalStore 错误降为 0 后返回 ok。
- 产品可据此显示“无待办、无阻塞、无授权”，而不是 unknown/degraded。

这是当前最明确的“假绿”实现，违反项目自己的 fail-closed 产品规则。

#### P1-02 Rust 的 `remote_unknown` 任务状态会让 Tasks/Workspace 崩溃

`SOURCE-CONFIRMED`，临时单测已确认 presentation 返回 `undefined`。

- `openlife-core/src/agent/tasks_view_model.rs` 定义并实际生成 `TaskLifecycleStatus::RemoteUnknown`。
- `frontend/src/tauri.ts` 的 `TaskLifecycleStatus` 联合类型遗漏 `"remote_unknown"`。
- `readOnlySpinePresentation.ts` 的 switch 无该分支、也无 default；`TasksReadOnlyView.tsx` 随后解引用 `lifecycle.label/status`。
- Workspace 使用同一个 presentation，故同样受影响。

远端和当前本地都未修。这是第二个被全绿 typecheck/单测漏掉的真实跨语言契约问题。

#### P1-03 Snapshot 版本字符串可越出 snapshot 目录读取 YAML

`SOURCE-CONFIRMED`，未执行 IPC 利用。

- `openlife-core/src/versioning.rs` 直接执行 `versions_dir.join(format!("{}.yaml", version))`，没有 snapshot ID 校验或 canonical containment。
- `diff_snapshots` 是 shipped Tauri command，原样返回 diff；绝对路径或 `../` 可读取应用进程权限范围内、位于 snapshot 目录外的 `*.yaml`。
- 固定追加 `.yaml` 限制了扩展名，但攻击者可通过位于 `.yaml` 的 symlink 间接指向其他文件。
- `restore_snapshot` 另有明确治理请求和 LifeModel YAML 结构限制；主要泄漏面是 diff。

#### P1-04 MCP 对 credential/token 的真正传输边界没有脱敏

`SOURCE-CONFIRMED`，未执行真实 MCP 泄漏。

- credential 规则只存在于 `PrivacyEngine::desensitize_secrets_only`。
- `openlife-core/src/mcp.rs` 的 inspect/dispatch 使用普通 `detect/desensitize`；`sk-*`、token、password 可不被识别。
- 脱敏后 JSON 解析失败还会回退原始 arguments；非法 custom regex 会被静默忽略并允许保存。
- 用户消息敏感评估、local-only 和外部 MCP confirmation 是缓解，不是传输边界修复；确认后原 secret 仍可能发送。

#### P1-05 ProposalStore 的损坏/未来值会降低风险或取消过期

`SOURCE-CONFIRMED`。

- 未知 `risk_level` 被解码成 `Medium`，现有测试甚至把该 fallback 固化为预期。
- 无效 `expires_at` 被解码成 `None`，从而表现为永不过期。
- High/Critical 才触发的 native confirmation 可能因风险降级而被绕开。

触发通常需要数据库损坏、旧迁移或未来枚举，但正确行为应是拒绝读取/进入 degraded，而不是猜默认值。

#### P1-06 Review Center 的固定 100 条窗口可隐藏旧 pending proposal

`SOURCE-CONFIRMED`；当前 dev profile 数据量不足以动态触发。

- `src-tauri/src/read_models/review_center.rs` 固定 `list_all_proposals(100, 0)`。
- 底层按 `created_at DESC` 取全部状态；较新的 accepted/rejected/expired 可挤掉较旧 pending。
- summary 把窗口当完整 total/action-required，没有 pagination 或 `isComplete=false`。

#### P1-07 TasksViewModel 可丢失失败任务但仍标 Ready

`SOURCE-CONFIRMED`，未做 mixed-failure 注入。

- 单条 task detail 失败只记录 warning 后 `continue`。
- 只要剩余任何 item，envelope 无视 warnings 直接标 Ready。
- blocked/waiting task 可以从 UI 消失，Tasks 与 Workspace 仍表现健康。

#### P1-08 shipped `save_chat_message` 是 dormant canonical mutation bypass

`SOURCE-CONFIRMED`；当前生产 UI 没 caller，因此实际可达性低于普通主链。

- handler 接受任意 `user`/`assistant` 消息并直接保存 canonical history。
- 它不经过 `OpenLifeTurnRuntime`、AgentRun、terminal owner、PolicyRouter 或 final-delivery proof。
- 当前 UI 使用 `sendMessageV2`，说明该命令更像旧兼容写入口；按 Phase7 应删除或证明唯一合法用途。

#### P1-09 默认 Playwright 与现行产品完全脱节

`REPRODUCED`。

- `playwright test --list` 因 `stage1BrowserEvidence`、`step6ProductAcceptance` 已删除/迁移而收集失败，结果为 0 tests。
- 单独运行 `e2e/smoke.spec.ts` 5/5 失败；它仍断言 `/#/chat`、`/#/builder`、`/#/mailbox` 和旧 Settings 文案。
- 两个旧 Stage spec 在非 Tauri 环境可写 blocked report 后 return，通过本身也不能证明产品能力。
- 旧 spec、webdriver、archive helper 约 8,524 行，是 V4/Stage 遗留，不应只修 import 后继续冒充现行 E2E。

#### P1-10 CI 的绿色 Smoke/Acceptance 名称大于证据

`SOURCE-CONFIRMED + 远端运行已核对`。

- GitHub `Smoke Test` 只运行 `scripts/smoke.sh`；该脚本做 Rust 编译/库测试、Vitest、macOS 路径探测和函数名存在性扫描，不启动桌面产品。
- 主 CI 不执行 `tauri build`、安装包启动、Playwright、旧数据库到当前版本升级或 resolved-capability artifact proof。
- Stage1/Step6 workflow 都标为 Retired，却仍在每次相关 push/PR 上运行静态合同并快速变绿。

因此远端 main CI 全绿是真实事实，但不能被解释为桌面产品、IPC、迁移或真实 journey 通过。

### 6.4 P2 / 应在恢复功能扩张前进入稳定化 backlog

| 问题 | 证据与影响 |
| --- | --- |
| PatchStore 损坏值被改写成另一种操作 | `life_model/patch_store.rs` 将未知 op→Replace、source→Manual、risk→Medium、status→Pending、坏 JSON→null，可能把腐败行物化成不同 LifeModel 变更。 |
| MemoryLifecycle 解码语义不一致 | 风险/敏感度较保守，但未知 lifecycle→Candidate、materialization→NotRequired、坏 evidence/rollback JSON→空、坏时间→now，投影和审计会失真。 |
| NetworkPolicy 缺失默认继续 | `ActionExecutionContext` 默认为 `None`，ask/deny/allowlist 只在 `Some` 执行；正式 Main Chat 会提供，dev `execute_tool_call` 不提供。网络工具缺 policy 应失败关闭。 |
| Calendar 读取无资源预算 | fallback 会读取目录中全部 `.ics`，无文件数/单文件/总量限制；日期范围用字符串字典序比较，存在 DoS 与筛选错误。 |
| MCP audit 导出/清理边界 | 导出固定最近 10,000 条却无 `truncated/hasMore`；负 retention days 可生成未来 cutoff 并大范围删除。 |
| Builder revision CAS 非跨进程原子 | 读 revision→比较→写文件没有文件锁/事务；两个 Store/进程可同时成功造成 lost update。 |
| Plugin override 损坏回退空配置 | 解析失败 `unwrap_or_default()`，可能让 manifest enabled，同时 store 看似健康；当前 executor quarantine 只降低可达性。 |
| 文件读取 symlink/metadata TOCTOU | 安全路径校验、metadata 和最终读取分离且读取原始路径；本地并发替换 symlink/扩容可突破先前判断。 |
| 同步 builtin 不能被 timeout 抢占 | callback 在首个 await 前同步运行，Tokio timeout 无法中止，panic 也未隔离；当前 builtin 简单但扩展风险存在。 |
| Tasks pagination/N+1/read 时写入 | 固定先 hydrate 最新 200、filter 后分页，`offset>=200` 永远空；Tasks 再 hydrate 100；Workspace+Tasks 最坏约 600 次 detail，且 detail read 会 reconciliation 写入。 |
| runtime status 假阴性 | Kernel 实际可跑 AgentLoop，但 TurnRuntime 把两个 observed 布尔硬编码 false，诊断永远不能给 AgentLoop credit。 |
| dev tool success 可无 AgentRun | dev `execute_tool_call` 先执行 effect，再忽略 `create_run` 错误，IPC 可返回成功与不存在的审计链。 |
| audit key rotation 有旧 writer | ToolGateway snapshot Clone `McpAuditStore`；rotation 只替换 AppState 实例，已捕获上下文可继续用旧 epoch 写入。 |
| Settings 深链不自动加载 | 直接打开/刷新 `/settings` 只加载 Today，不调用 Settings data source，显示“暂不可用”直到手动刷新。 |
| Tauri bridge 无运行时 schema | 4,700+ 行手写 `tauri.ts` 仅用泛型强转；AppConfig shape 与 enum variant 完整性无共享 schema/codegen，正是两个崩溃穿过绿门的根因。 |
| Today 仍携带旧 route refs | 生产 bundle 中保留 `route:companion`、`route:mailbox` action metadata，实际点击却去 Workspace/Review，遥测/自动化语义冲突。 |
| 关键 UI 覆盖率接近零 | 全局 coverage gate 可通过，但 WorkspaceConversationPanel/SettingsPrivacyView 等关键页面接近零，无法阻止真实 route/serialization 崩溃。 |
| release WebView 权限过宽 | `default.json` 给 main window 未预配 scope 的 text-file read/write command；前端没有生产 caller，应按 least privilege 移除并做打包 ACL proof。 |
| dev A2A 仍有直接 bridge | release quarantine、loopback、Bearer 和 body limit基本成立；但 dev IPC 的 `a2a_handle_task/a2a_bridge_local` 绕过 HTTP middleware/外部 request validation。 |
| 迁移没有 artifact upgrade matrix | migration 分散在各 Store Rust 文件；CI 没有旧版本真实数据库→当前发布包的升级/回滚验证。 |
| 无效 V4 配置仍存在 | `runtime_mode`、两个 `use_agent_loop`、`experimental_context_assembler` 留在 config/bridge，却无生产读取者，设置表面不控制真实运行时。 |
| 死 ReflexEngine 与重依赖 | 没有生产 caller，却无条件编译 tokenizers/tract/ndarray；增加约 939 MiB 缓存并放大首次构建/派生文件。 |
| 依赖维护债 | `pnpm audit` 有 14 条 advisory（多数为 dev server/test tooling，React Router 一条为直接生产依赖）；Rust 无已知漏洞但有 18 条 unmaintained/unsound warning。需按可达性升级而非简单忽略。 |

### 6.5 P3 / 防御性和维护性问题

- Streaming 的 Tauri emit 错误被丢弃；当前前端无实际 streaming listener，未来启用后会出现 durable final 已完成但 UI 无 recovery signal。
- Performance observer 无全局初始化锁/cleanup，开发 StrictMode/HMR 可重复注册；CLS 每次 callback 从零计算，不是累计值。
- Chat/Builder hook 缺同步 in-flight ref；同一 render 闭包直接双调用可双 dispatch，但真实 DOM 双击/连续 Enter 未复现，因此不是当前用户可达故障。
- 开发日志输出敏感值长度和无盐 FNV-1a hash；不含明文，但短值可字典猜测、同值可跨调用关联。
- `make test` / `make ci` 不覆盖 A2A feature test、Playwright、artifact、security audit、coverage或跨平台，名称不应被当作完整发布门。
- A2A autostart 在 enable=1 且 token 未定义时使用 `${#OPENLIFE_A2A_PAIRED_TOKEN}`，`set -u` 会先因 unbound variable 退出，而不是给预期提示。
- 多个重构后孤儿 frontend 模块仍在非 test 源码树，虽可 tree-shake，但会制造看似并列 authority；删除前需按 expected-absent/test-only/product-valid 分类。

### 6.6 本次明确没有发现或已确认改善的边界

- ordinary send/stream 都委托 `OpenLifeTurnRuntime`；没有发现隐藏的 legacy completion fallback。
- Provider network consent 当前有 endpoint/capability/subject/proposal binding 与 AllowOnce 消耗。
- 旧 scheduler 持锁跨 provider/tool await 问题当前已修。
- release A2A 不可达，dev HTTP sidecar 默认关闭、仅 loopback、需要强 pairing token；未认证、超大 body 和 ContextManifest 拒绝有真实测试。
- release CSP 严格；`csp:null` 只在 dev config。
- 未找到普通产品输入可直接触发的 production `unwrap/panic`。
- 全 workspace 测试、严格 Clippy、前端 build/typecheck/Vitest 都通过；问题在盲区，不在基本工程门完全失效。

## 7. AI Coding 偏移审计

### 7.1 已经被纠正的偏移

#### A. 多阶段旧系统并存

历史上 Stage/Beta/Migration/Productization/Maturity 形成过平行命令、路由、测试和状态字段。Phase7 删除策略已经大范围消除它们，并用 expected-absent guard 防止复活。这是当前最成功的治理修正。

#### B. 文档或测试替代运行时真相

当前权威链明确把历史文档降级；完成状态要求真实 trial，local/scripted evidence 不得冒充 live evidence。这个原则已经写进测试与报告。

#### C. 前端自行重建产品状态

新前端通过 backend-owned Projection/ViewModel 读取状态，不再主动从 diagnostics/proposal/config 碎片拼出“ready”；当后端返回 unknown/error 时会保持 fail closed。但 Projection/ViewModel 后端本身仍存在 error-to-zero/Ready 路径，因此这项纠偏只完成了前端 authority，不代表端到端状态真相已经闭合。

#### D. Proposal 等于完成、模型文本等于授权

Proposal/accepted/applied/completed 的区分已经成为系统级合同，静态门也在保护。

### 7.2 当前仍然严重的偏移

#### P0：真实闭环落后于合同和测试闭环

系统能非常精确地解释为何不能做，但还没有稳定证明可以完成最核心的 3 条用户旅程。对个人 Agent OS 来说，长期处于 fail-closed 但无法完成任务，会从“安全”退化为“不可用”。

#### P0：凭据身份与持久化恢复未形成开发基线

当前 Keychain ACL 绑定旧 worktree/ad-hoc binary 身份，交互恢复不能跨重启；dev/release/qa 共用 credential service/ref。这个问题阻断所有 durable journey 和 live provider 证明，优先级高于继续做 UI 或增加能力。

#### P1：核心代码超大，所有权虽命名收敛但物理边界未收敛

`OpenLifeTurnRuntime`、Kernel、event stream、Main Chat Agent v1 都是万行级。一个 owner 不等于一个可维护模块。当前可能把“单一权威”误解成“所有逻辑集中在单文件”。正确目标应是一位 owner、多组内部不可绕过的子组件。

#### P1：后端修复 backlog 仍在增长

Backend Remediation 最初 35 项，追加发现已到 D072。追踪表显示：

- 72 项总计；
- 41 项 implemented、24 项 in progress、7 项 not started；
- 仅 10 项 verification complete；
- 64 项仍 open，7 项 closure candidate，1 项 independently verified。

这不表示代码质量一无是处，而表示“实现提交”远快于“独立闭环验证”。继续按新发现逐个创建 slice/worktree，会永久保持局部修复模式。

#### P1：文档数量已经成为认知攻击面

`plans` 有 214 个文件、约 7.1 万行；大量历史文档仍在仓库根和 plans 中。虽然权威链已修正，但 AI 仍可能搜索到过时符号并被带偏。当前源支持文档也出现漂移：例如 testing 文档仍说 Cargo workspace 有两个 member，而实际已有第三个 A2A server；README 的“主产品页面”叙述与当前规范路由也不完全一致。

#### 已缓解：物理 worktree 碎片化；历史 refs 仍待分类

审计开始时本机存在大量 D0xx slice、RED/GREEN、WIP 和 integration worktree。本次已经收敛为唯一 `/Users/tw/Desktop/open-life` checkout，并删除全部已合入 main 的冗余本地分支。剩余 26 个未合入 local ref 和少量 remote ref 只用于语义/证据分类；在完成 V4 13 个独有提交的归类前不删除，也不重新创建 checkout。

#### P2：公开 backlog 与真实 backlog 脱节

GitHub 只有两个旧 LifeModel-HS Issue；真实工作由仓库内 JSON/plan 驱动。这对单人 AI Coding 尚可，但不利于人类审查、优先级和终止决策。

#### P2：证据系统本身过度复杂

项目在 AgentRun、TaskSession、EventStore、TerminalOwner、Receipt、Projection、Audit、Outbox 等方面构建了很强的可信系统，但每增加一层也增加一致性故障。需要明确哪些是产品必须的最小 durable facts，哪些只是为了让测试可观察而产生的重复事实。

### 7.3 过去开发中反复犯过的错误与防复发规则

Git 历史提供了比阶段名称更可信的过程证据：`origin/main` 自 2026-04-23 起约 586 个提交，其中 554 个非 merge；2026-07-15 单日 61 个提交，7 月 14 日 40 个，7 月 16 日 32 个。高速度本身不是错误，但结合超大 checkpoint、长期 worktree 和独立验证滞后，形成了稳定的失败模式。

几个代表性事实：

- `6704d6d` 一次改动约 236k 行/320 文件，提交正文明确写着“Local integration checkpoint only; do not publish”，并保留已知 RED；它后来仍成为当前主线历史的一部分。
- `64fd02e` 前端切换约 53k 行/152 文件；切换本身方向正确，但旧 E2E、脚本、route refs 和 archive helper 没有一起退出默认入口。
- `proposal_engine.rs` 曾作为“统一 Proposal 层”加入，后来又因形成第二权威被删除。
- `main_chat_final_acceptance_tests.rs` 曾被创建为最终验收 owner，后来删除；退役 Stage1/Step6 workflow 却仍在运行。
- 旧 `ChatPage.tsx` 经历大量 churn 后被整页删除；这说明许多局部 UI 投资没有先验证信息架构和后端合同是否值得保留。
- 历史中有大量 `stabilize`、延长 timeout、fixture clock/watchdog 修复；其中部分合理，但也显示测试稳定化经常先于真实产品根因 closure。

| 历史错误模式 | 本次证据 | 以后强制规则 |
| --- | --- | --- |
| 新权威加在旧权威旁边 | ProposalEngine、旧 runtime/route、dormant `save_chat_message` | 新 owner 进入前必须有旧 owner 删除清单；禁止“旧写法 + 新网关”长期共存。 |
| 把一个 owner 误解为一个巨型文件 | 多个 8k–23k 行核心文件 | 单一权威可以有多个内部组件；按 transaction/read-write/invariant 拆，不按行数机械拆。 |
| 手造 fixture 代替真实跨语言 contract | Settings `openai_key`、Task `remote_unknown` | Rust 必须导出真实序列化 golden/schema；TS enum/DTO 由 codegen 或运行时 parser 校验。 |
| 解析失败给默认值 | Proposal/Patch/Memory/Plugin/store projection | persisted truth 解码一律严格；损坏/未来值进入 degraded/UNKNOWN，不猜默认业务语义。 |
| 把有界窗口当完整真相 | Review 100、Task 200、audit 10,000 | 所有 bounded read 必须携带 total/hasMore/truncated/completeness；产品不得据局部窗口宣称无待办。 |
| 用 source-string/static guard 当行为 GREEN | 数十个 `include_str!` guard、假 smoke | 静态 guard 只算 absence/shape 证据，不计用户行为、迁移、并发或外部 live credit。 |
| 大 checkpoint 混合多类改动 | 236k/53k/45k 行级提交 | 默认一个 invariant/一个失败模式/一个 closure 证据包；checkpoint 不得直接进入长期主线。 |
| 阶段名和完成文案先于真实试用 | 多轮 Beta/Stage/final gate 后仍 Trial RED | 完成状态只由固定真实 journey、重启与 failure recovery 证据生成；文档/测试不能自证完成。 |
| 每个 finding 建长期分支/worktree | 27 worktree、42 local branches、V4 13/195 分叉 | `/Users/tw/Desktop/open-life` 是唯一开发入口；短期 worktree 必须有 owner、expiry、remote preservation、删除条件。 |
| 修复提交停在 feature branch | `origin/main` 仍有 Settings crash，PR #64 未 review | 恢复开发前先清空关键 integration gap；主线红问题不能靠“本地已修”计入完成。 |
| 安全/一致性修复只覆盖一个 Store | Task/Run 严格 enum，Proposal/Patch/Memory 仍 fallback | 每个 store-level 修复必须附横向 inventory 和 corrupt-row matrix，防止局部 remediation。 |
| read model 混入 repair write | Task detail read 会 reconciliation 写入 | Query 与 repair command 分开；read model 不应在用户刷新时隐式产生 durable effect。 |
| 清理只看小缓存或盲删 ignored files | Cargo target 76.6 GiB；证据与 `.env` 也在 ignored 集合 | 先 `du`/worktree/source 分类，再 path-limited 清理；保留 secrets 配置和 trial evidence。 |

关键节点的防幻觉检查固定为四问：

1. 这是当前 `origin/main`、当前 HEAD、旧 V4，还是历史报告中的事实？
2. 这是源码可达、真实复现、外部 live，还是只存在于 fixture/source string？
3. 列表/计数是完整集合还是有界窗口？错误是否被降成 0/空/ok？
4. “implemented”“tests pass”“PR open”“trial green”“independently closed”是否被错误地当成同一状态？

## 8. 风险判断

| 风险 | 等级 | 现实影响 |
| --- | --- | --- |
| MCP audit key/profile/single-writer 未绑定 | P0 | 并发/profile key 覆盖可让历史审计密文不可解密 |
| Keychain/二进制身份不稳定 | P0 | 重启后 Safe Mode，阻断真实 durable/live journey |
| 远端 main Settings 崩溃尚未合入修复 | P0-REMOTE | 从 main 直接恢复会重新遇到真实页面崩溃 |
| 真实纵向闭环未绿 | P0 | 产品可能“很安全但做不成事” |
| Projection/read model error-to-zero | P1 | 待审、阻塞、授权可消失且 UI 仍显示 Ready/ok |
| Snapshot 越界与 MCP secret 边界 | P1 | WebView 可越界读 YAML；确认后的 credential 可能发往 MCP |
| 跨语言 DTO/enum 漂移 | P1 | Rust 可合法返回前端无法处理的 shape，运行时直接崩溃 |
| 现行 E2E 为 0，CI smoke 非产品 smoke | P1 | 主线全绿无法防止当前路由、IPC、迁移和桌面 journey 回归 |
| 万行级核心模块 | P1 | 修复易产生远端副作用，AI 难以建立完整上下文 |
| 72 项发现仅 1 项独立验证 | P1 | 完成声明与真实可信度之间存在巨大间隙 |
| 多存储/多事实副本一致性 | P1 | crash、cancel、retry、delete、migration 时易分叉 |
| 文档与源码漂移 | P1 | 后续 Agent 被过时路径/数量/页面名误导 |
| 历史分支 refs | P2-MITIGATED | 物理 worktree 已收敛为 1；未合入 refs 仍需语义分类，禁止重新 checkout 并行开发 |
| 外部 Provider/MCP 真实证据不足 | P1 | 核心 Agent 能力仍主要由 scripted/local proof 支撑 |
| UI 合同成熟度高于任务完成度 | P2 | 容易产生“看起来产品化”的错误信心 |
| GitHub backlog 失真 | P2 | 人类无法从远端快速理解真实优先级 |

## 9. 我对项目成熟度的判断

按不同维度分开看：

| 维度 | 判断 |
| --- | --- |
| 产品愿景 | 清晰且有独特价值 |
| 安全/治理原则 | 较强，明显高于普通 AI 原型 |
| 架构方向 | 已纠偏，single-system 方向正确 |
| 代码可维护性 | 偏低，核心模块与状态体系过大 |
| 自动化合同 | 很强，甚至可能过强/过多 |
| 真实桌面可用性 | 初步可运行，但主闭环仍红 |
| 外部模型/工具真实性 | 局部证明，未达到产品信用 |
| 数据可靠性 | 设计认真，但凭据、跨存储、重启恢复仍是关键风险 |
| 文档治理 | 权威链清晰，存量和漂移仍严重 |
| 发布成熟度 | 不适合宣告 Beta complete；适合受控开发试用与修复 |

如果用一句话定位：OpenLife 已经跨过“玩具原型”，进入“复杂系统重构后的受控 Alpha”；它最大的危险不是做不出功能，而是继续用更多功能、合同和阶段名掩盖尚未完成的真实闭环与复杂度偿还。

## 10. 发展指导与优先级

### 10.1 立即停止的事项

- 暂停新产品能力、新路由、新 Provider、新 Skill/MCP 表面。
- 暂停新 Phase/Stage 命名；除非旧阶段有明确 closure，不再创建新路线图层。
- 暂停以“再加一组大测试”替代真实桌面纵向验证。
- 暂停为每个发现默认创建独立长期 worktree；优先在单一集成入口做经过批准的窄切片。
- 不重建任何已删除旧命令、页面或 fallback。

### 10.2 第一优先级：建立 Restart Baseline

目标不是立刻做 Developer ID 发布，也不是给旧 V4 换一个新阶段名，而是得到一个唯一、可复现、没有已知 P0 的开发底座：

1. 人工 review PR #64；将有效修复合入最新 `origin/main`，并在合入 SHA 重跑机械门与真实 `/settings`。
2. 在同一个 `/Users/tw/Desktop/open-life` checkout 内切到更新后的 main，再创建短期稳定化分支；不恢复 roadshow/D0xx worktree，不做旧 V4 整分支 merge。
3. 逐个审查 V4 的 13 个独有提交：标为 `already semantically present`、`still needed-port`、`obsolete` 或 `evidence-only`。
4. 修复 MCP audit key 的 profile/store identity、原子创建、共享 active generation 和跨进程单写者。
5. dev/qa/release credential namespace 明确隔离；现有 canonical key 有非旋转、可回滚迁移；测试 binary 不占产品 slot。
6. 关闭 snapshot containment、MCP secret redaction、Projection error-to-zero、Task `remote_unknown` 和 Proposal strict decode。
7. 删除/隔离 dormant mutation 命令，建立 shipped command allowlist。
8. 退役旧 Stage1/Step6 默认 E2E，先让“当前六条 Workbench 路由”的 Playwright collection 与 smoke 真实变绿。

凭据完成证据必须包含两次全进程重启，而不是当前进程的 `available`；所有故障必须保持 Safe Mode，不删除/重建 canonical data。

### 10.3 第二优先级：只做三条真实纵向闭环

建议把 Trial Green 限定为：

1. **普通规划闭环**：真实 Provider 或明确 Local Provider -> 生成计划 -> 任务状态 -> 重启后可见。
2. **记忆/人生模型闭环**：用户明确提出偏好或规则 -> Proposal -> edit/accept -> materialize -> 重启 -> context 中可验证 -> rollback。
3. **权限工具闭环**：read 或低风险 external action -> permission proposal -> accept -> resume -> receipt/final delivery -> 重启后审计可见。

每条必须用真实生产 Workbench、真实 Tauri command、隔离数据目录和真实持久化；fixture、mock、直接数据库注入不能获得产品信用。

### 10.4 第三优先级：做复杂度预算，不做大重写

先给核心 owner 设置约束：

- 新行为不得直接继续堆进万行文件；
- 每次改动必须画出 owner 与内部 component 边界；
- 拆分只允许沿稳定 invariant，例如 provider lifecycle、terminal transition、policy admission、projection assembly；
- 子模块不能成为第二 router/gateway/terminal owner；
- 拆分前后用同一真实纵向场景验证。

优先拆的是“可独立证明的纯合同/状态转换”，而不是把文件按行数机械切开。

### 10.5 第四优先级：收缩 backlog 与证据

- 把 72 项发现聚合为不超过 8 个 root-cause program，而不是 72 条并行开发线。
- closure candidate 必须安排独立只读验证，否则不能继续积累新的 implemented 状态。
- 对每一种 durable fact 说明唯一用途、唯一 owner、保留期和删除/恢复语义。
- 把 GitHub Issues 更新为当前 8 个 program 和 Trial Green milestone，使远端对人可读。

### 10.6 第五优先级：文档减负

- 保持当前四级权威链。
- 修复当前 source-backed docs 的明显漂移（workspace member、路由名、已删除文件引用）。
- 为历史 plans 生成机器可读索引和默认排除规则；不要移动 ADR 0013，也不要为归档创建新的活跃命名空间。
- 新报告必须以当前 SHA、验证日期和 UNKNOWN 边界开头。
- 不再为每个小修复创建 5-10 个长期 Markdown；优先更新一个当前决策记录和一个证据附件。

## 11. 建议的下一开发计划与里程碑

### 11.1 Gate A：`OpenLife Restart Baseline`

这是恢复开发的入口，不是产品完成里程碑。退出条件：

- PR #64 经人工 review 后合入，并在新的 `origin/main` SHA 重新通过机械门与真实 `/settings`；
- 旧 V4 的 13 个独有提交完成四类语义归档，不进行整分支 merge；
- 当前已知 P0 为零；credential 和 MCP audit key 能在隔离 profile 下跨两次完整进程重启保持一致且 fail closed；
- MCP audit 另须通过同路径双进程 writer rejection、并发首次建 key 只产生一个 generation、rotation 时旧 writer 拒绝或受控交接，以及旧密文持续可解密；
- Projection error-to-zero、Snapshot containment、MCP secret redaction、Proposal strict decode、Task `remote_unknown` 等直接安全/真相 P1 已关闭；
- 默认 Playwright 能收集现行 Workbench 测试，至少有一个会真实启动桌面产品的 smoke；
- 始终只保留 `/Users/tw/Desktop/open-life` 一个物理开发入口；旧 checkout 清理和全 refs bundle 已完成，剩余历史 refs 只做语义分类，不用于并行开发。

### 11.2 Gate B：`OpenLife Trial Green 1`

进入条件：

- `Restart Baseline` 已绿；
- 选择三个纵向闭环的固定隔离数据集、用户脚本和恢复脚本；
- 每条闭环的唯一 runtime/store/projection owner 已画清，不再新增平行 authority。

退出条件：

- 三条纵向闭环全部在真实 Tauri Workbench 完成；
- 至少一次完整应用重启后状态、权限、提案和结果一致；
- 没有 silent durable write；
- 没有 proposal-only 被标为 completed；
- 没有 fixture/mock 获得真实产品信用；
- 每条闭环都有最小 transcript、receipt、projection 和人工可读截图；
- 当前 P0 为零；
- Backend Remediation 中与这三条闭环直接相关的 finding 完成独立验证。

非目标：

- 不追求所有 72 项一次关闭；
- 不追求完整 Beta；
- 不增加新功能表面；
- 不进行全仓重写；
- 不恢复旧页面或旧 runtime。

## 12. 最终结论

OpenLife 最值得保留的是它对私人 Agent 的核心判断：模型输出、工具动作、长期记忆和人生模型真相必须被区分，并且用户应拥有最后的治理权。项目已经为这个判断构建了罕见的深度基础设施，不能把它当作失败的原型推倒重来。

但必须改变后续开发节奏。过去的 AI Coding 强项是高速扩展和快速补证；现在这正成为负担。下一阶段的成功标准不是新增多少模块、测试、计划或“完成项”，而是能否让一个真实用户在稳定开发环境中完成少数关键任务，并在重启、失败、取消、审批和回滚后仍得到一致、可信、可理解的结果。

我的指导结论是：**选择“重新组织开发”，不是“沿旧 V4 分支继续”，也不是“全仓重写”。保留产品理念与 single-system 架构，冻结功能扩张，先建立 Restart Baseline，再用三条真实纵向闭环把产品从“合同可信”推进到“使用可信”，同时对万行核心和 72 项 backlog 进行根因级收缩。**

在这之前，项目应被公开描述为“受控 Alpha / Trial Green remediation”，而不是 Beta complete、Phase7 complete 或完整个人 AI OS。
