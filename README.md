# OpenLife

OpenLife 是一个**本地优先的个人 Agent 框架**。它不是单纯的聊天应用，也不是普通的目标管理工具，而是围绕用户私人数据构建的个人 AI 操作系统雏形。

OpenLife 的核心范式正在从单一 ReAct 叙述升级为：

```text
LifeModel-HS Protocol Layer
  + Governed Agent Runtime
  + ReAct Default Strategy
  + Tool/Skill Execution
  + Memory/Feedback/Maturation Loop
```

用户先构建自己的 LifeModel，包括身份、目标、能力、状态、偏好和关系等私人上下文。之后，OpenLife 会让本地模型或云端模型在这个人生模型的约束下完成对话、规划、写作、复盘、工具调用、状态更新和长期反馈。系统不只是回答问题，还应该通过 LifeEvent、Signal、Evidence、Governor、Proposal 和用户确认持续打磨 LifeModel，并让不同 RuntimeStrategy 在同一套 LifeModel-HS 协议约束下执行。

## 当前定位

- **当前阶段是 Main Chat Agent Execution v1 整改中，不是 complete**：W124-W149 LifeModel-Governed Backend Completion Goals 1-8、W150-W158 Skill Runtime Beta Maturity 已完成；普通 `send_message` / `start_stream_message` 已接入 AgentIngress 和 governed task session/transcript/action queue。DirectAnswer deterministic reflex path 现在也会记录 Main Chat strategy AgentRun、prompt/context transcript 并完成 task session；ReActToolExecution 现在会先尝试带 governed plan guidance、metadata-safe tool-candidate contract 和 exact `toolset_allowlist` target enforcement / exact `tool_action_allowlist` enforcement 的 AgentLoop，再 fail-soft 到 single-step ActionExecutor-backed read action path，保留 governed follow-up synthesis、direct read parser/executor input 对齐、eval-gated memory/session multi-step AgentLoop read/observe/follow-up proof、web network-policy blocker AgentLoop proof、fixture-backed successful web read AgentLoop proof、registered MCP AgentLoop success proof、registered MCP ToolPermission proposal runtime proof、命名 read-only MCP manifest resolution，以及 generic MCP read 的 bounded read-only manifest candidate selection proof；generic MCP candidates 现在按 query/manifest capability/name/tag 做 deterministic ranking（不使用 raw manifest id/description），按 model-selectable target 去重后再应用 bounded limit，在 metadata-safe contract 中记录 rank/source/capability digest/sanitized match reason、model-selected ExecutionPolicy metadata 和 governed candidate arguments source/digest metadata，并排除 high-risk / critical / confirmation-required / write-like read-shaped manifests（包括 manifest id/name/action/capability/tag surfaces 中嵌入的 write-like terms）和 contract-unsafe 或 oversized model-facing manifest names/source labels；explicit named MCP read target resolution 使用 permission-preserving governed-read target predicate，保留 safe read ToolPermission proposal flow，同时阻断 high-risk/critical/write-like/contract-unsafe read-shaped manifest 进入 AgentLoop candidate；allowlist 外 target、wrong action/target pair、write-like 或 unsupported action type、unknown non-candidate model calls 现在会变成显式 `model_selected_disallowed_tool` blocker，不触发 single-step fallback、不执行写入；model-supplied arguments 会被 exact allowlist 中的 governed candidate executor input 覆盖；policy-denied selected candidate 会变成显式 `model_selected_tool_policy_blocked` blocker。
  Tauri mock IPC 已覆盖 `send_message` 和 `start_stream_message` DirectAnswer run/task-session completion、L2 DirectAnswer scheduler/provider generation trace（scripted provider response）、send/stream governed file-read command surface、send/stream PlanExecute draft command surface、proposal-path command surface、send/stream registered-MCP AgentLoop success command surface、send/stream registered-MCP ToolPermission proposal command surface、send/stream web AgentLoop blocker command surface、send/stream fixture-backed web AgentLoop success command surface，并覆盖 `send_message` / `start_stream_message` web-policy / missing-MCP blocker command surface；新增 24-case send/stream command-surface eval gate（`main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix`）跨 DirectAnswer、scripted provider generation、file read、PlanExecute draft、proposal、web blocker、web AgentLoop blocker、fixture-backed web AgentLoop success、missing MCP blocker、registered MCP AgentLoop success 和 registered MCP ToolPermission proposal（fallback + AgentLoop）执行真实 mock IPC，并断言 legacy fallback 和 silent write 均为 0；scripted AgentLoop eval hook 现在不再是 core-test-only，Tauri final runner 通过 openlife-core 依赖调用 100-case runtime gate 时也会执行同一套 memory/session/web/MCP AgentLoop proof；core 100-case runtime eval report 和 command-surface report 都显式输出 live-provider generation / combined web-MCP / split web / split MCP / proposal-permission coverage 均为 0、`finalCompletionReady=false` 和 named live-provider blockers（含 split `provider_backed_web_agent_loop_not_executed` / `provider_backed_mcp_agent_loop_not_executed`）；core final acceptance gate 会聚合 runtime、command-surface 和 live-provider evidence，并复核关键 coverage，防止伪造 ready flag；`run_main_chat_agent_execution_v1_eval_gate` 现在作为非默认 Tauri command 暴露 core 100-case runtime eval gate，metadata-safe、no external provider、no app-store writes、migrationPermission=false，并以 typed `liveProviderPreflight` 字段和 summary 附带当前 config 的 metadata-safe live-provider preflight blockers（不序列化 key、不调用 provider），明确因缺 command-surface/live evidence blocked；live-provider evidence 现在拆分为 Direct generation、web AgentLoop、MCP AgentLoop 和 proposal-permission 四个结构化场景，并要求 scenario identity 匹配对应 evidence、`status=completed`、无 blockers，且必须带非空 run_id、task_session_id 和 response_preview trace，不能用非 web/MCP 场景、失败状态或缺 trace 的布尔位冒充；Tauri 聚焦测试现在会聚合 live harness reports，会实际运行 24-case command-surface gate 把结果转入 core final acceptance gate，并有单一 final acceptance runner 在无 live opt-in 时运行 core 100-case + 24-case command-surface 后 fail closed；complete clean live harness evidence 会显式合并进 runtime live coverage 和 command-surface final evidence，该 runner 报告会暴露 runtime/command-surface case counts、live-provider attempted/report/ready/main-chat-invoked/model-invoked counts、metadata-safe live-provider blockers（含 post-invocation 场景化派生 blockers）、direct-write flag 和嵌套 core acceptance result；Tauri live-provider harness 还新增非 ignored 的 `local_test_http` OpenAI-compatible provider-client proof，经普通 `send_message` 走真实 scheduler/HTTP client 并验证 response trace/no silent writes，但不计入 external live-provider generation evidence，command-surface live coverage 仍为 0；旧 deterministic 100-case suite 已降级为 legacy scaffold。最终 Main Chat Agent Execution v1 仍需证明完整 live-provider-backed model generation eval gate、更广 provider-backed web/MCP AgentLoop 覆盖，以及更广 provider/live proposal-permission 验收。
- **Live-provider evidence 清单**：`liveProviderPreflight.requiredEvidence` 和 core acceptance `requiredEvidence` 同时列出 combined `provider_backed_web_mcp_agent_loop` 与 split `provider_backed_web_agent_loop` / `provider_backed_mcp_agent_loop`，避免只用 combined web/MCP evidence 代替独立场景证明。
- **十五大板块按各自 scope 已完成**：Default Chat Adapter guard/prep（W65-W72）、LifeModel Maturation proof slice（W73-W78）、Legacy Direct-Write Convergence（W90-W97）、Plan-Execute Product Vertical（W98-W105）、RuntimeStrategy / Multi-Strategy Runtime Maturity（W106-W113）、ReAct Beta Execution Hardening（W114-W123）、Backend Completion Goal 1 Master Contract And Schemas（W124-W127）、Backend Completion Goal 2 Evidence Graph v1（W128-W130）、Backend Completion Goal 3 Maturation Engine v1（W131-W133）、Backend Completion Goal 4 Accepted Guidance And Materialization（W134-W136）、Backend Completion Goal 5 Runtime Guidance Integration（W137-W140）、Backend Completion Goal 6 Policy / Privacy / Tool Governance Hardening（W141-W143）、Backend Completion Goal 7 Backend Golden Paths（W144-W146）、Backend Completion Goal 8 Pre-UI Backend Contract Freeze（W147-W149）、Skill Runtime Beta Maturity（W150-W158）。
- **Main Chat 现在是 Agent control plane 的 partial 主线**：每条普通 Chat 消息先进入 AgentIngress，再按 DirectAnswer、ReActToolExecution、PlanExecute、MemoryProposal、LifeModelProposal、ReviewMaturation 或 BlockedConfirmation 路由。已支持 durable task session、execution transcript、action queue、policy decision、bounded context、governed DirectAnswer AgentRun/task completion、governed plan-guided AgentLoop attempt、metadata-safe tool-candidate contract、AgentLoop exact `toolset_allowlist` target enforcement / exact action-target candidate enforcement、generic MCP read 的 bounded read-only manifest candidate set、deterministic capability/name/tag ranking（不使用 raw manifest id/description）、model-selectable target 去重、candidate rank/source/capability digest/sanitized match reason evidence、model-selected allowed-tool metadata、model-selected ExecutionPolicy metadata 和 governed candidate arguments source/digest metadata（排除 high-risk / critical / confirmation-required / write-like read-shaped manifests 和 contract-unsafe/oversized model-facing manifest names/source labels，并用 candidate contract input 覆盖模型 `arguments`）、controlled knowledge-format loader（bounded `AGENTS.md`/`SOUL.md`/`USER.md`/`MEMORY.md`/selected `SKILL.md`，ordinary send/stream、frontend Tauri wrapper 和 Chat composer 手动 `SKILL.md` context field 可传入 sanitized optional selected skill id）、ActionExecutor-backed read-only fallback observations、memory/session multi-step AgentLoop proof、web AgentLoop blocker proof、fixture-backed successful web read AgentLoop proof、registered MCP AgentLoop success proof、registered MCP ToolPermission proposal proof、governed follow-up synthesis、PlanExecute draft、proposal/blocker、resume/cancel/retry controls、safe read automatic retry replay、permission-preserving resume、accepted ToolPermission resume replay proof、cancel queued-action stop proof、非 replayable retry manual blocker、provider route/local-only guard eval proof、scheduler-backed eval-provider DirectAnswer generation proof、local HTTP OpenAI-compatible provider-client proof（不计 external live credit）、runtime eval `mcpToolPermissionProposalCoverage` proof、send/stream L2 DirectAnswer scheduler/provider trace proof、execution task panel、Mailbox accept-resume UI handoff、send/stream DirectAnswer proof、send/stream file-read command-surface proof、send/stream PlanExecute draft command-surface proof、send/stream proposal-path command-surface proof、send/stream registered-MCP AgentLoop success proof、send/stream registered-MCP ToolPermission proposal proof、send/stream web AgentLoop blocker proof、send/stream fixture-backed web AgentLoop success proof、send/stream web/MCP blocker command-surface proof、24-case send/stream command-surface eval gate（legacy fallback=0、silent write=0）和可见 legacy fallback；但完整 live-provider-backed generation、provider-backed web/MCP AgentLoop、更广 provider/live proposal-permission 验收和更完整 provider-backed/model-ranked manifest/capability selection 仍未达到最终标准。
- **Live provider gate 仍是 blocker**：core 现在有 `evaluate_main_chat_live_provider_eval_preflight` 和 config-backed adapter，会在缺少显式 live eval opt-in、provider key、network enabled、非 scripted scheduler 或 LocalOnly policy 时 fail closed，并输出 metadata-safe blockers；config-backed adapter 只暴露 key presence，不序列化 key，Tauri command-state test 已覆盖无 opt-in / 无 key / network disabled / scripted scheduler 的 no-invocation blocker。Tauri test harness 还新增非 ignored 的 local HTTP OpenAI-compatible provider-client proof，用 `local_test_http` endpoint 通过普通 `send_message` 执行 DirectAnswer 主路径，证明 scheduler/provider HTTP client plumbing 可用并保留 response trace/no silent writes；该 proof 明确不计入 external live generation evidence。final acceptance runner 现在支持 `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1` opt-in live suite，会在隔离 AppState 中通过普通 Main Chat path 执行 DirectAnswer、web AgentLoop、bounded multi-candidate registered MCP AgentLoop 和 MCP ToolPermission proposal 四个场景；无 opt-in 时不运行 live scenario，有 opt-in 但缺 key/network/provider 时会生成四个 blocked scenario reports 且不调用模型。ignored opt-in live-provider proof paths 只有外部 provider endpoint、真实 key、network enabled、无 scripted scheduler 且显式 opt-in 时才会通过 ordinary `send_message` IPC 调用主路径；scenario 覆盖 DirectAnswer、provider-backed ReAct web AgentLoop、bounded multi-candidate registered MCP AgentLoop 和 MCP ToolPermission proposal，并检查 `liveProviderInvoked`、AgentLoop action status、no single-step fallback、MCP target resolution / ToolPermission proposal evidence、model-selected candidate rank/source/capability digest/match reason / ExecutionPolicy / governed-arguments trace 和 no silent writes；缺少这些 ReAct governance trace 字段的 live report 不会获得 web/MCP/proposal live credit；web AgentLoop live credit 还必须证明 selected candidate target/action 是 governed `web.*` 工具；registered MCP live credit 还必须证明至少两个 distinct bounded model-selectable MCP candidates / targets / action-target pairs；MCP ToolPermission proposal live credit 还必须证明 selected candidate target/action 与 pending ToolPermission proposal target 匹配且 actionType 为 `mcp_tool`。当前环境未执行这些 ignored live runs。这个 preflight/harness 不能替代完整 live-provider-backed generation / web/MCP / proposal-permission 证明。
- **Plan-Execute 已有非默认产品纵切**：W98-W105 提供 weekly planning session、review/edit/finalize、proposal-first step execution、AgentRun/trace linkage 和 Runs evidence surface；它不是 default Chat migration，也不是外部 provider 写入。
- **完整 Beta 尚未宣告**：W114-W158 和 Main Chat Agent v1 提升了执行严肃性、LifeModel-HS backend kernel、Runtime Guidance、ModelRouter/Privacy、Tool/Governor governance、read-model contracts、Skill Runtime governance 和 Chat control-plane 能力，但完整 Beta 仍需要更多产品 UI/UX productization 和真实 provider executor hardening。
- **Stage 1 自动化工程 dogfood 已通过，broader v1 仍未 complete**：Linux CI run `27807633105` 已用真实 Tauri Chat UI D01-D36 产生 `tauri_command_surface_browser_observed` pass evidence（36 observed / 36 passed / 0 failed / blockers=[]）。这只证明 Stage 1 default deterministic engineering dogfood；manual/internal-trial、external live-provider-backed generation、provider-backed web/MCP AgentLoop 和更广 provider/live proposal-permission 验收仍是 separate follow-up scopes。`plans/main_chat_agent_v1_stabilization_goal_spec.md` 现在保留为 stabilization audit trail，不应被当成 Stage 1 仍未 ready 的证据。
- **Stabilization 进展**：final-gate aggregation/evidence normalization/blocker derivation 已抽到 `src-tauri/src/main_chat_final_gate.rs`，final acceptance runner 会调用该模块；command-surface eval case matrix、scenario state setup、prompt/session-id mapping、case assertion/no-silent-write interpretation、report/coverage/evidence normalization 已抽到 `src-tauri/src/main_chat_command_surface_eval.rs`，isolated eval AppState factory 已抽到 `src-tauri/src/main_chat_eval_state.rs`，live-provider harness opt-in/suite/execution 已抽到 `src-tauri/src/main_chat_live_provider_harness.rs`，Main Chat task-control command state/resume/cancel/retry/replay helper 已抽到 `src-tauri/src/main_chat_task_controls.rs`，Main Chat generation support helper（chat persistence / vector persistence / AgentRun finalization / non-stream fallback / provider endpoint classification / preview text）已抽到 `src-tauri/src/main_chat_generation_support.rs`，Main Chat preprocessing / memory-hit merge helper 已抽到 `src-tauri/src/main_chat_preprocess.rs`，Main Chat auto-checkin / reasoning-trace prompt / conversation-signal helper 已抽到 `src-tauri/src/main_chat_conversation_updates.rs`，Main Chat legacy fallback route-plan / non-stream generation fallback helper 已抽到 `src-tauri/src/main_chat_legacy_fallback.rs`，Main Chat deprecated/non-default AgentLoop send/stream helper 已抽到 `src-tauri/src/main_chat_legacy_agent_loop.rs`，ReAct tool-selection plan/candidate helper 已抽到 `src-tauri/src/main_chat_react_tool_selection.rs`，ReAct AgentLoop attempt execution / runtime helper types / follow-up synthesis / action-to-tool-call conversion / tool-call/blocker metadata helpers 已抽到 `src-tauri/src/main_chat_react_runtime.rs`，ReAct ActionExecutor-backed fallback execution helper 已抽到 `src-tauri/src/main_chat_react_execution.rs`，Main Chat proposal / ToolPermission proposal support helper 已抽到 `src-tauri/src/main_chat_proposal_support.rs`，HS runtime packet/topic/tool-requirement helper 已抽到 `src-tauri/src/main_chat_hs_runtime.rs`，Main Chat task session / transcript / action-queue runtime support helper 已抽到 `src-tauri/src/main_chat_runtime_support.rs`，Main Chat send command state executor 已抽到 `src-tauri/src/main_chat_send.rs`，Main Chat strategy dispatcher 已抽到 `src-tauri/src/main_chat_strategy.rs`，Main Chat stream command state executor 已抽到 `src-tauri/src/main_chat_streaming.rs`，Main Chat context compiler / selected-skill sanitizer 已并入 `src-tauri/src/main_chat_context_loader.rs`，ReAct static/unit tests 已拆到 `src-tauri/src/main_chat_react_unit_tests.rs`，boundary command-surface tests 在 `src-tauri/src/main_chat_react_boundary_tests.rs`，final-acceptance helper/runner/evidence tests 已拆到 `src-tauri/src/main_chat_final_acceptance_tests.rs`，HS runtime behavior tests 已拆到 `src-tauri/src/main_chat_hs_runtime_tests.rs`，task-control behavior tests 已拆到 `src-tauri/src/main_chat_task_control_tests.rs`，context-loader / workspace-file resolver behavior tests 已拆到 `src-tauri/src/main_chat_context_loader_tests.rs`，command-surface/live harness 测试现在直接使用这些非 test-only helper；新增非默认 `run_main_chat_agent_execution_v1_final_acceptance_gate` Tauri command，使用同一 aggregation、运行 core runtime eval、附带当前 state/scheduler 的 metadata-safe live-provider preflight，并在隔离 eval AppState 中执行全部 24 个本地 send/stream command-surface cases（send 通过 `main_chat_send::send_message_with_state`，stream 通过 `main_chat_streaming::start_stream_message_with_state`），默认不调用 external provider、不写 app store，并在缺完整 live evidence 时 fail closed、`migrationPermission=false`；同一 runner 在显式 live opt-in 时会执行四场景 live harness suite，当前无 credential 环境只证明缺 key blocker/report path；安全 workspace file read 已从少量硬编码文件名改为 workspace-root scoped resolver，支持显式相对路径、canonicalize readable target，并阻断 traversal / outside-workspace read，command-surface FileReadSuccess eval 会把隔离状态的 safe_paths 明确限定到 canonical workspace root；ReAct AgentLoop 现在有 metadata-safe tool-candidate contract、generic MCP read bounded read-only manifest candidate set、deterministic capability/name/tag ranking（不使用 raw manifest id/description）、candidate rank/source/capability digest/sanitized match reason evidence、model-selected ExecutionPolicy metadata、governed candidate arguments source/digest metadata、high-risk/confirmation/write-like embedded-surface/contract-unsafe/oversized candidate name/source exclusion、exact `toolset_allowlist` target enforcement / exact action-target allowlist enforcement 和显式 disallowed model tool / model arguments override / policy-denied selected candidate blocker proof；Main Chat context assembly 现在通过 controlled loader 读取 bounded workspace/configured `AGENTS.md`、`SOUL.md`、root / `memories/` `USER.md` / `MEMORY.md` 和 selected `SKILL.md`（仅在 selected skill id 存在且通过校验时），ordinary send/stream command surface、frontend wrapper 和 Chat composer 手动 `SKILL.md` context field 已支持 optional `selectedSkillId`。24-case send/stream command-surface gate 仍通过。但 v1 仍未 complete，真实 final/live-provider evidence、剩余 live harness 外部证明、更完整 provider-backed/model-ranked manifest/capability selection 和进一步 Main Chat runtime/strategy 模块清理仍是阻断项。
- **Stabilization 测试拆分补充**：live-provider command-surface harness tests 已拆到 `src-tauri/src/main_chat_live_provider_tests.rs`，覆盖 no-invocation preflight blocker、local HTTP provider proof 和 ignored external-provider opt-in proof paths；这不计入 external live-provider completion。
- **Command-surface 测试拆分补充**：proposal-path、DirectAnswer send/stream、web-policy blocker、missing-MCP blocker、registered-MCP read-success、registered-MCP / web-policy AgentLoop no-fallback、registered-MCP multi-candidate AgentLoop IPC tests 和 24-case command-surface eval gate coverage test 已拆到 `src-tauri/src/main_chat_command_surface_tests.rs`，继续证明 proposal-first、no-silent-write、DirectAnswer run completion、scheduler/provider trace、governed blocker、governed read-success、AgentLoop no-fallback、multi-candidate allowed-manifest selection 和 send/stream eval matrix 的普通 command surface 行为，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **HS runtime 测试拆分补充**：HS runtime helper extraction guard、sanitized HS packet construction、tools-prompt read-only/write-requirement separation、LocalOnly no-cloud fallback、sensitive-topic LocalOnly policy selection 和 no `src-tauri/src/lib.rs` root re-export guard 已拆到 `src-tauri/src/main_chat_hs_runtime_tests.rs`，继续覆盖 Main Chat HS runtime boundary，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Task-control 测试拆分补充**：retry manual blocker / automatic replay、permission-preserving resume / accepted ToolPermission replay 和 cancel queued-action stop tests 已拆到 `src-tauri/src/main_chat_task_control_tests.rs`，继续覆盖 Main Chat task-control command behavior，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Context-loader 测试拆分补充**：bounded knowledge-format surfaces、selected `SKILL.md` loading/sanitization、selectedSkillId send/stream plumbing 和 workspace file resolver explicit-path/traversal tests 已拆到 `src-tauri/src/main_chat_context_loader_tests.rs`，继续覆盖 bounded context 和 safe workspace read 边界，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Runtime-module guard 测试拆分补充**：Main Chat runtime/generation/proposal/final-gate/command-surface/live-provider module extraction guards、focused module helper import direction guard、send/stream state-executor guards、ordinary send/stream deprecated-helper isolation guard 和 Chat page migration-command isolation guard 已拆到 `src-tauri/src/main_chat_runtime_module_tests.rs`，继续覆盖 reusable helper 边界并防止 focused modules 通过 `src-tauri/src/lib.rs` root re-export 取用 Main Chat runtime helpers，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **文档与 taxonomy 是硬约束**：入口文档、progress index、Tool Taxonomy 和代码状态必须同步。过期 P1/P2 标签、旧 W60/W65 当前状态、或把 readiness 当迁移许可的文案都视为开发阻塞项。

下一阶段总纲和架构基准文档见：

- [Plans Document Governance](/Users/fujing/Desktop/偶来福/plans/README.md)
- [Main Chat Agent Execution v1 Stabilization Goal Spec](/Users/fujing/Desktop/偶来福/plans/main_chat_agent_v1_stabilization_goal_spec.md)
- [Main Chat Agent Migration v1 Goal Spec](/Users/fujing/Desktop/偶来福/plans/main_chat_agent_migration_v1_goal_spec.md)
- [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
- [Skill Runtime Beta Maturity Goal Spec](/Users/fujing/Desktop/偶来福/plans/skill_runtime_goal_spec.md)
- [LifeModel-Governed Backend Completion Goal Spec](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_backend_completion_goal_spec.md)
- [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
- [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
- [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)

LifeModel-HS MVP / architecture 文档仍是后续开发的硬基线，但当前 Goal-mode
实现入口以上面的 Backend Completion spec 为准：

- [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
- [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
- [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)

## 核心能力

| 能力 | 当前状态 | 目标形态 |
|---|---|---|
| LifeModel | 已有四维模型、编辑器、Proposal-first 更新基础和 W73-W78 maturation proof slice | 成为所有 AgentTask 的私人协议层和可治理 source-of-truth |
| Builder / Calibration / Feedback | 已收敛到 Proposal / Mailbox，W90-W97 移除高风险 legacy direct-write | 继续作为 LifeModel maturation 和用户确认闭环的输入面 |
| Chat | Main Chat Agent Execution v1 partial：普通 send/stream 已进入 AgentIngress、governed task session、execution transcript、action queue、proposal/blocker、bounded context、task controls 和 traceable fallback；DirectAnswer 已成为真实 strategy，包含 send/stream command-surface AgentRun、prompt/context transcript、task completion proof 和 L2 scheduler/provider generation trace proof；ReActToolExecution 已接入 governed plan-guided AgentLoop attempt、metadata-safe tool-candidate contract、generic MCP read bounded read-only manifest candidate set、deterministic capability/name/tag ranking（不使用 raw manifest id/description）、candidate rank/source/capability digest/sanitized match reason evidence、model-selected ExecutionPolicy metadata 和 governed candidate arguments source/digest metadata（排除 high-risk/critical/confirmation-required/write-like read-shaped manifests，并用 exact allowlist governed input 覆盖模型 `arguments`）、exact `toolset_allowlist` target enforcement / exact action-target candidate enforcement 和 disallowed model tool / model arguments override / policy-denied selected candidate blocker proof；Main Chat context assembly 已接入 controlled knowledge-format loader（bounded workspace/configured `AGENTS.md`/`SOUL.md`/`USER.md`/`MEMORY.md`/selected `SKILL.md`，ordinary send/stream、frontend Tauri wrapper 和 Chat composer 手动 field 已有 sanitized optional selected skill id plumbing）；并保留 single-step ActionExecutor-backed read fallback、direct read parser/executor input 对齐、eval-gated memory/session multi-step read/observe/follow-up proof、web AgentLoop blocker proof、fixture-backed successful web read AgentLoop proof、registered MCP AgentLoop success proof、registered MCP ToolPermission proposal proof 和 governed follow-up synthesis；safe retry/replay、permission-preserving resume、accepted ToolPermission resume replay、cancel、`proposal.create`、execution task panel 和 Mailbox accept-resume handoff 已覆盖；100-case runtime eval harness 输出分项 coverage，包括 `mcpToolPermissionProposalCoverage`，并显式 `finalCompletionReady=false` 与 live-provider generation/web/MCP/proposal blockers；core final acceptance gate 聚合 runtime、command-surface 和 live-provider evidence，并复核关键 coverage；live-provider evidence 拆分为 Direct generation、web AgentLoop、MCP AgentLoop 和 proposal-permission 四个结构化场景；Tauri 聚焦测试会聚合 live harness reports，会实际运行 24-case command-surface gate 并把结果转入 core final acceptance gate，也有单一 final acceptance runner 默认运行 core 100-case + 24-case command-surface 后因缺 live evidence fail closed；24-case send/stream command-surface eval gate 覆盖 DirectAnswer、scripted provider generation、file read、PlanExecute draft、proposal、web blocker、web AgentLoop blocker、fixture-backed web AgentLoop success、missing MCP blocker、registered MCP AgentLoop success、registered MCP ToolPermission proposal（fallback + AgentLoop），并要求 legacy fallback=0、silent write=0，同时显式 `finalCompletionReady=false` 与 split live-provider blockers；完整 live-provider-backed generation eval、live/provider-backed web/MCP manifest 覆盖、更广 provider/live proposal-permission 验收和更完整 provider-backed/model-ranked manifest/capability selection 仍待补齐 | 证明完整 live-provider-backed provider eval gate、provider-backed web/MCP AgentLoop、live proposal/permission flow 和更完整 provider-backed/model-ranked manifest/capability selection，并保持 proposal/permission/audit 边界 |
| Default Chat Adapter Guard | W65-W72 完成 backend-only descriptor、contract、harness、send/stream proof、gate、disabled skeleton 和 integrity proof | 仅作为未来受控迁移准备；当前不接 ordinary Chat，不接 executor |
| MultiStrategy Runtime | W106-W113 完成 strategy descriptor、registry readiness、selection matrix、execution envelope、status command 和 trace vocabulary | 支持 ReAct / PlanExecute 之外的未来策略，但 disabled/declarative-only 策略不能伪装可执行 |
| ReAct Execution | W114-W123 完成 Beta execution hardening：action schema/parser、Tool Registry readiness、manifest authority、trace、permission/replay、proposal-first writes | 继续补齐产品 surface 后再评估完整 Beta |
| LifeEvent / Signal / Evidence Graph / Maturation / Accepted Guidance | W124-W149 完成 typed LifeEvent/Signal schema、deterministic low-risk extractor、safe Signal -> Evidence bridge、Evidence Graph v1、Evidence Timeline read model、Maturation Engine v1、accepted guidance lifecycle、materialized LifeModel provenance、version rollback read model、RuntimeHSPacket guidance integration、ModelRouter/Privacy hard enforcement、ActionExecutor HS tool governance、Governor unified decision report、W144-W146 Weekly Planning / Low-Energy Support / Preference Correction Backend Golden Paths，以及 W147-W149 Pre-UI Backend Contract Freeze read models / final gate | 进入 pre-UI product surface design，或另行人工评审的 default Chat route migration Goal |
| PlanExecute | W98-W105 完成 weekly planning 产品纵切；Main Chat partial path 可从普通 Chat 创建 governed PlanExecute draft session | 扩展更多 Plan-Execute 产品场景，并保持受治理边界 |
| Runs / Trace | 支持 preview/product/ReAct trace lifecycle 的 metadata-safe 展示 | 成为所有 runtime strategy 的统一可审计视图 |
| ModelRouter | 已具备任务/隐私感知路由、健康检查语义和 W141 High/Critical / HS LocalOnly cloud hard-filter | 继续在 golden paths 中证明 privacy policy、local-only 阻断和 route trace |
| Memory | 已有 SQLite、向量记忆、Memory Proposal 和治理化归档基础 | 升级为来源可追踪、可回滚、可审计的长期记忆层 |
| Tools / Skills | ToolManifest、MCP/A2A、proposal-only file/calendar/email/task 工具、W150-W158 governed Skill Runtime、plugin declarative-only boundary 和 metadata-safe trace 已存在 | 真实 executor 接入必须另行遵守 permission/proposal/audit |
| Today / Mailbox / Runs / Settings | 当前默认入口为 Today、Companion、Mailbox、Life Model、Runs、Settings；旧 Workspace / Review / Dashboard URL 只保留 redirect | 继续作为 Agent OS control plane，而不是新增孤立页面 |
| Diagnostics / Safe Mode | 已有恢复、诊断、网络策略和安全模式基础 | 成为系统恢复、策略检查和发布门控的一部分 |

## 技术栈

| 层级 | 技术 |
|---|---|
| 前端 | React 18 + TypeScript + Tailwind CSS + Vite |
| 桌面壳 | Tauri 2.x |
| 后端核心 | Rust Workspace (`openlife-core` + `openlife-tauri`) |
| 本地模型 | Ollama |
| 云端模型 | DeepSeek / OpenAI / OpenRouter / Custom OpenAI-compatible |
| 数据存储 | SQLite + YAML |

## 项目结构

```text
.
├── frontend/                     # React 前端
│   └── src/
│       ├── pages/                # Today / Companion / Mailbox / Life Model / Runs / Settings 等页面
│       ├── components/           # 通用组件
│       ├── tauri.ts              # Tauri command 封装层
│       └── App.tsx               # 路由与全局布局
├── openlife-core/                # Rust 核心业务库
│   └── src/
│       ├── life_model.rs         # LifeModel
│       ├── builder/              # LifeModel 构建与确认候选
│       ├── agent/                # AgentRuntime、AgentLoop、Proposal、ModelRouter
│       ├── scheduler.rs          # 当前模型调度器
│       ├── llm.rs / ollama.rs    # 云端与本地模型调用
│       ├── memory.rs             # 消息、会话、状态等 SQLite 存储
│       ├── vectors.rs            # 向量记忆
│       ├── mcp.rs / a2a.rs       # 工具与外部 Agent 接入
│       ├── privacy.rs            # 隐私检测与脱敏
│       ├── feedback.rs           # 反馈信号
│       ├── evolution.rs          # 微进化
│       └── versioning.rs         # 快照与回滚
├── src-tauri/                    # Tauri 命令层和桌面壳
│   └── src/
│       ├── lib.rs                # 核心状态与聊天主链路
│       └── commands/             # 按领域拆分的 Tauri commands
├── plans/                        # 架构与开发计划
└── OpenLife_Final_PRD.md         # 旧版 PRD，当前作为历史参考
```

## 推荐阅读顺序

1. [Plans Document Governance](/Users/fujing/Desktop/偶来福/plans/README.md)
2. [Main Chat Agent Execution v1 Stabilization Goal Spec](/Users/fujing/Desktop/偶来福/plans/main_chat_agent_v1_stabilization_goal_spec.md)
3. [Main Chat Agent Migration v1 Goal Spec](/Users/fujing/Desktop/偶来福/plans/main_chat_agent_migration_v1_goal_spec.md)
4. [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
5. [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
6. [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
7. [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)
8. [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
9. [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
10. [LifeModel-HS Legacy Write Path Audit](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_legacy_write_path_audit.md)
11. [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)
12. [OpenLife PRD v2: Personal Agent Framework](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md)
13. [OpenLife Development Plan](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
14. [Codex Execution Playbook](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)
15. [OpenLife Final PRD](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)，仅作为历史需求参考

## 快速开始

### 前置要求

- Rust >= 1.75
- Node.js 18+
- pnpm 9.x（推荐通过 Corepack 启用）
- 可选：Ollama，本地模型服务

### 安装依赖

```bash
corepack enable
corepack prepare pnpm@9.1.0 --activate
cd frontend && pnpm install
cd ..
```

项目统一使用 pnpm；请不要使用 npm 安装依赖或提交 `package-lock.json`。

### 配置模型

当前推荐先使用 DeepSeek 跑通云端试用链路，也可以使用 Ollama 本地模型。

```bash
# DeepSeek，推荐试用路径
export DEEPSEEK_API_KEY="sk-..."

# OpenRouter
export OPENROUTER_API_KEY="sk-..."

# OpenAI
export OPENAI_API_KEY="sk-..."
```

桌面端中进入 `Settings`，选择 Provider，填写 Key，点击测试连接，成功后保存。

### 开发运行

```bash
# 首次使用：初始化环境
make setup

# 启动开发模式
make dev
# 或
./scripts/dev.sh
```

开发模式通过 Vite dev server 加载当前源码，不依赖 `frontend/dist`。
如需验证生产前端产物，运行 `cd frontend && pnpm run build` 或 `make ci` 重新生成
`frontend/dist` 后再使用 preview / Tauri build 路径。

### 测试

```bash
# Rust 测试
cargo test -q

# 前端测试
cd frontend && pnpm test

# 前端生产构建
cd frontend && pnpm run build

# 完整 CI 检查
make ci
```

### 本地缓存和旧产物清理

以下目录都是本地缓存或生成产物，已被 `.gitignore` 忽略，可以按需要删除：

- `frontend/dist/`：前端生产构建产物；`make dev` 不依赖它，`pnpm run build` 会重新生成。
- `frontend/test-results/`、`frontend/playwright-report/`：本地 E2E / readiness 报告产物；普通应用运行不依赖它们。`frontend/test-results/product-audit-*` 是产品审计证据，默认 `make clean` 会保留。
- `target/`：Cargo workspace 编译缓存，通常很大；删除后会释放空间，但下一次 Rust 构建/测试会重新编译，耗时明显增加。

推荐先清理轻量产物：

```bash
make clean
```

如果确实要丢弃本地产品审计截图和报告，再显式运行：

```bash
make clean-audit-results
```

只有在需要释放大量磁盘空间时再清理 Rust target：

```bash
make clean-rust-target
```

## 当前推荐试用路径

### 本机工程试用（推荐）

1. **`Today`** 查看系统状态、今日建议和待处理确认项。
2. **`Companion`** 发起 Main Chat 对话，观察任务状态、工具/观察记录、Proposal、blocker 和安全下一步。
3. **`Mailbox`** 审查 AI 生成的 LifeModel / Memory / ToolPermission 等提案，确认、拒绝或延后。
4. **`Life Model`** 查看个人 LifeModel；需要构建或补全时进入 `/life-model/build` 二级流程。
5. **`Runs`** 查看 AgentRun 记录，按状态和类型过滤。
6. **`Settings`** 完成模型配置、诊断检查和实验性功能开关。

旧 URL 仅作为兼容重定向保留：`/`、`/workspace`、`/dashboard` → `/today`；
`/chat`、`/agent` → `/companion`；`/review` → `/mailbox`；`/builder` →
`/life-model/build`；`/life`、`/map` → `/life-model`。旧 URL 不再渲染旧页面或旧
onboarding。

### 试用状态边界

本机 deterministic 工程试用可以直接运行；limited internal trial 仍需要
manual dogfood 和真实 external live provider P0 evidence，不能用本地、mock、
fixture 或旧报告冒充通过证据。

### 实验性功能（灰度测试）

在 Settings → 实验性功能中可开启：

- **ContextAssembler V2**：使用模块化组装器构建对话上下文（灰度中，可回滚）
- **ModelRouter**：智能路由选择本地/云端模型（默认路由基础设施，云端 Provider 需配置并通过轻量健康检查）

```text
Today -> Companion task -> Runs trace -> Mailbox -> LifeModel/Memory update
```

## 最近完成的重要更新

### Phase 1-3: Agent Runtime 基础设施
- ✅ AgentRun 增强（RedactionLevel、AgentAction、AgentObservation）
- ✅ LifeModel Patch 系统（5 种操作、冲突检测、自动解决）
- ✅ Proposal 统一层（Builder/Calibration/Feedback/Memory 统一确认流）

### Phase 4-5: 路由与上下文
- ✅ **ModelRouter**：任务类型感知、隐私级别、Provider 健康检查
- ✅ **ContextAssembler**：模块化 LifeModel/Memory/Privacy/Tools 组装

### Phase 2.5: Chat Proposal
- ✅ 关键词提取（中英文目标/状态/能力识别）
- ✅ 动态置信度计算（信号强度 + 强调标记）
- ✅ 可配置冷却时间和阈值

### Historical Phase 6: Workspace 重设计（历史参考）
- ✅ 旧 Workspace 驾驶舱（系统状态、待处理 Proposal、Run 统计；当前默认入口已迁移到 Today）
- ✅ Runs 页面增强（过滤、搜索、分页、批量操作、回收站）
- ✅ 历史导航重构（旧 Workspace 曾为默认首页；当前默认入口是 Today）

### Phase 7: Stabilization / Spine Consolidation
- ✅ Builder 正常路径改为 Proposal-Only，legacy direct apply 仅保留给迁移/调试。
- ✅ Proposal 应用器覆盖 LifeModel/Goal、MemoryWrite、MemoryArchive、ToolPermission MVP。
- ✅ Chat Proposal 持久化与 AgentRun.generated_proposals 关联收敛到共享 helper。
- ✅ `make ci` 覆盖格式检查、Rust tests、frontend tests、frontend production build/typecheck。

### W1-W60: LifeModel-Governed Runtime Preview, Gate Evidence, Controlled Pilot, Promotion Evidence, Readiness, Draft Planning, Review Decision Evidence, Implementation Gate, Shadow Run, Shadow Review Evidence, Cutover Readiness, Cutover Candidate Adapter, Candidate Review Evidence, Candidate Promotion Readiness, Default Chat Runtime Boundary, Activation Plan Draft, Activation Review Evidence, Activation Implementation Gate, Disabled Routing Scaffold, Contract Harness, Dry-Run Invocation Boundary, Dry-Run Review Evidence, Implementation Readiness Gate, Controlled Preview, Controlled Preview Review Evidence, Controlled Preview Approval Readiness, Cutover Implementation Plan Draft, Cutover Plan Review Evidence, Cutover Plan Approval Readiness, Route Guard Scaffold, Cutover Invocation Harness, Invocation Plan, Invocation Boundary, Typed Callsite Contract, Authority Roadmap Sync, Ordinary Entry Preflight, Ordinary Entry Preflight Status, Narrow Implementation Discussion Gate, Narrow Implementation Plan Draft, Narrow Implementation Plan Review Evidence, And Narrow Implementation Plan Approval Readiness
- ✅ Tool / Proposal Hygiene、Thin Runtime Spine、ReAct Runtime Contract Convergence。
- ✅ LifeModel Maturation Loop Foundation、LifeModel Governor MVP、PlanExecute Core MVP。
- ✅ StrategySelector、MultiStrategy Runtime Orchestrator、Preview Command。
- ✅ MultiStrategy Preview AgentRun Audit Persistence：metadata-safe 外层 run 可在 Runs / Trace 展示。
- ✅ Non-default Settings preview、guarded Chat preview subpath、Maturation V1 service。
- ✅ PlanExecute governed V1 report、RuntimeStrategy trait、ReAct / PlanExecute adapter registry。
- ✅ Runtime Migration Gate：只读诊断默认 Chat 未替换、preview 健康、metadata-safe trace、fallback、无外部写入和 proposal-first 边界，并在 Settings 显式展示 evidence。
- ✅ Sustained Gate Evidence / Pilot Eligibility：只读检查最近 3 条 preview gate report 是否连续干净，展示 clean count、checked run ids、blocking reasons；不创建 AgentRun、Proposal、Action、Observation。
- ✅ Very Small Controlled Chat Migration Pilot With Fallback：Chat 页面新增显式 `Run Controlled Pilot` 单轮入口；先查 eligibility，blocked 不调用 preview；eligible 后才运行 `allowWrites=false` preview；成功默认只显示 “Pilot response”，不自动写普通 chat history，默认 Send 不变。
- ✅ Reviewed Pilot Response Promotion：成功且包含 `userOutput` 的 pilot response 可由用户显式 review/confirm 后提升为一条 ordinary assistant message；取消、blocked、failed、no-output、重复 promotion 均不写入，promotion 不写 LifeModel/Memory/Proposal/外部工具结果。
- ✅ Post-Promotion Validation And Source Binding：Controlled Pilot 结果绑定 source session；review 展示 source/target session、runId、strategy 和 governance summary；确认前校验 source/target 一致，session mismatch 时不调用 `save_chat_message`，显示 blocking/fallback 和重新运行 pilot 提示。
- ✅ Controlled Pilot Promotion Evidence Recorder：确认 promotion 且 assistant message 保存成功后，写入一条 metadata-safe runtime evidence；只保存 pilotRunId、source/target session、strategy/payload/governance、message length、checksum 和 promotedAt。Settings 实验区只读 summary 展示 promoted count、recent pilot run ids、latest timestamp 和 mismatch block count；evidence 失败显示 degraded/error，重试不重复写 chat message。
- ✅ Promotion Evidence Readiness Gate：新增只读 `check_controlled_pilot_promotion_readiness`，默认要求 3 条 metadata-safe promotion evidence；Settings 实验区展示 ready/block、counts、recent pilot run ids、blocking reasons 和 mismatch block count。`sessionId` 已预留，当前 EvidenceStore 不支持时按 global summary 读取；默认 Send 不调用该 gate。
- ✅ Reviewed Migration Plan Draft Generator：新增只读 `draft_controlled_chat_migration_plan`，复用 W24 readiness gate。blocked 时不生成 plan sections；passed 时生成仅供人工评审的 scope/preconditions/rollback/fallback/test plan。Settings 实验区展示 Draft Migration Plan 面板；默认 Send 不调用该 command。
- ✅ Manual Migration Review Decision Evidence：新增 `record_controlled_chat_migration_review_decision` 和只读 summary command。record 先调用 W25 draft；blocked draft approve 不写 evidence；ready draft 可记录 approve/reject/request_rework metadata-safe evidence，reviewer note 仅存 length/checksum/category。Settings 实验区展示 Migration Review Decision 面板；approval 不是 Chat migration。
- ✅ Approved Migration Implementation Gate：新增只读 `check_controlled_chat_migration_implementation_gate`。它要求 latest metadata-safe decision 为 approve、当前 W25 draft hash 与 approved evidence draftHash 匹配、当前 W24 readiness 通过；reject/request_rework、hash mismatch 或 readiness blocked 均阻断。Settings 实验区展示 Implementation Gate；eligible 也不会切换 default Chat。
- ✅ Non-Default Controlled Migration Shadow Run：新增 `run_controlled_chat_migration_shadow_run`。先查 W27 implementation gate；blocked 不执行 runtime；eligible 后才运行 write-disabled bounded controlled runtime preview，并只返回 metadata-safe strategy/payload/summary/warnings/blockers。可写 metadata-safe shadow AgentRun audit，但不写 Chat message、Proposal、Memory、LifeModel patch、Evidence 或外部工具结果；默认 Send 不调用它。
- ✅ Controlled Chat Migration Shadow Review Evidence：新增 `record_controlled_chat_migration_shadow_review_decision` 和只读 summary command。只记录人工 approve/reject/request_rework；所有 decision 都必须绑定已完成且 metadata-safe、write-disabled、无副作用的 shadow AgentRun。Evidence metadata 只保存 shadowRunId、decisionKind、reviewerNote checksum/length/category、readiness digest 和 createdAt；Settings Shadow Review 不自动触发，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Planning Readiness Gate：新增只读 `check_controlled_chat_cutover_readiness`。它要求 W27 implementation gate 当前 eligible、latest W29 shadow review decision 为 approve、approved shadowRunId 对应 AgentRun 仍存在且 completed/write-disabled/metadata-safe/side-effect-free。Settings Cutover Readiness 只能显式点击检查；pass 只表示可进入默认 Chat 迁移实现讨论，不迁移默认 Chat。
- ✅ Non-Default Controlled Chat Cutover Candidate Adapter：新增显式 `run_controlled_chat_cutover_candidate`。它先调用 W30 readiness；blocked 时不运行 runtime，eligible 后才执行一次 `allowWrites=false`、`maxToolCalls=0` 的 controlled runtime candidate，返回 `candidateReady`、`candidateRunId`、`outputPreview`/`userOutput`、`contractShape`、metadata-safe summary、warnings 和 blockers。允许 metadata-safe AgentRun audit；不保存 raw prompt/output/tool payload，不写 Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/外部工具结果。Settings Cutover Candidate 只能人工点击运行，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Candidate Review Evidence：新增 `record_controlled_chat_cutover_candidate_review_decision` 和只读 summary command。只允许人工 approve/reject/request_rework；approve 要求 candidate AgentRun 已完成、strategy/contract shape/candidateReady/runtime limits/storage/side-effect audit 全部符合 W32 约束。Evidence 只保存 candidateRunId、decisionKind、contractShape、candidateSummaryDigest、reviewerNote checksum/length/category 和 createdAt；不保存 reviewer 原文、candidate output、raw prompt/output 或 tool payload。Settings Cutover Candidate Review 只能显式记录/刷新，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Candidate Promotion Readiness Gate：新增只读 `check_controlled_chat_cutover_candidate_promotion_readiness`。它复用 W30 readiness，读取 W32 candidate review evidence，要求 latest decision 为 approve、approved candidate run 仍存在且 completed/send_message-compatible/write-disabled/zero-tool/metadata-safe/side-effect-free，并返回 ready/blockers/approved candidate counts/latest decision/defaultChatUnchanged/metadata-safe summary。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Runtime Boundary Status：新增只读 `get_default_chat_runtime_boundary_status`。它固定返回 `currentMode=legacy_stream`、`defaultChatUnchanged=true`、`automaticMigrationEnabled=false`、`controlledCandidateAvailable=false` 和 `candidatePromotionReadinessRequired=true`，只用于显式观察默认 Chat 仍未迁移；不读取/写入任何 runtime/evidence/proposal/memory/lifemodel/chat/tool/model 状态。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Plan Draft：新增只读 `draft_default_chat_adapter_activation_plan`。它组合 W33 candidate promotion readiness 与 W34 default Chat boundary status；blocked 时不生成 plan sections，ready 时只返回 human-review-only activation scope、preconditions、adapter contract checks、fallback、rollback、observability 和 test plan，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`、`requiresSeparateImplementation=true`。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Review Decision Evidence：新增 `record_default_chat_adapter_activation_review_decision` 和只读 `get_default_chat_adapter_activation_review_summary`。record 会先调用 W35 draft；blocked draft approve 不写 evidence；ready draft 可记录 approve/reject/request_rework metadata-safe evidence，reviewer note 仅存 checksum/length/category。Settings 只能显式记录/刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Implementation Gate：新增只读 `check_default_chat_adapter_activation_implementation_gate`。它组合当前 W35 stable activation plan digest 与 W36 latest metadata-safe activation review decision evidence；latest approve、draft ready、digest match、candidate promotion ready、default Chat 仍为 legacy stream 且 automatic migration disabled 时才 eligible。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Disabled Routing Scaffold：新增只读 `get_default_chat_adapter_routing_status`。它调用 W37 gate，但固定保持 `currentMode=legacy_stream`、`adapterScaffoldPresent=true`、`controlledAdapterEnabled=false`、`defaultSendPath=legacy_stream` 和 `startStreamPath=legacy_stream`，只展示 disabled scaffold 状态与 blockers。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Contract Harness：新增只读 `check_default_chat_adapter_contract_harness`。它调用 W38 routing status，检查 send_message / start_stream_message contract 仍为 legacy stream、controlled adapter disabled、activation implementation gate eligible，并返回 metadata-safe contract checks。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Dry-Run Invocation Boundary：新增显式 `run_default_chat_adapter_dry_run`。它先检查 W39 contract harness；blocked 时不运行 dry run，ready 时只返回 metadata-safe dry-run contract result，强制 `allowWrites=false`、`maxToolCalls=0`、`defaultChatPathUnchanged=true`，不保存 Chat、不创建 AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit/external write、不运行 runtime/tool/model call、不切换 routing。Settings 只能显式运行 dry run，默认 Send 不调用它。
- ✅ Default Chat Adapter Dry-Run Review Evidence：新增 `record_default_chat_adapter_dry_run_review_decision` 和只读 `get_default_chat_adapter_dry_run_review_summary`。record 会先重新运行 W40 dry run；approve 只在 dry run ready 时写 metadata-safe evidence，blocked approve 不写 evidence，reject/request_rework 只写白名单 metadata。reviewer note 仅存 checksum/length/category；默认 Send 不调用它。
- ✅ Default Chat Adapter Implementation Readiness Gate：新增只读 `check_default_chat_adapter_implementation_readiness`。它组合 W37/W39/W40/W41 当前证据，要求 activation implementation gate eligible、contract harness ready、dry run ready、latest dry-run review approve、dry-run digest match、default Chat unchanged、controlled adapter disabled、automatic migration disabled、send/stream 均保持 `legacy_stream`。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview：新增显式非默认 `run_default_chat_adapter_controlled_preview`。它先检查 W42 implementation readiness；blocked 不运行 runtime、不创建 AgentRun；ready 后才运行一次 write-disabled/zero-tool controlled preview，返回 SendMessageResult-compatible shape，并只写 metadata-safe adapter preview AgentRun audit；不保存 Chat、不 promotion、不切换 routing。Settings 只能显式运行 preview，默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview Review Evidence：新增 `record_default_chat_adapter_controlled_preview_review_decision` 和只读 `get_default_chat_adapter_controlled_preview_review_summary`。approve 必须绑定 completed / `default_chat_adapter_controlled_preview` / send-message-compatible / previewReady / write-disabled / zero-tool / metadata-safe / side-effect-free preview AgentRun；reject/request_rework 也只写白名单 metadata。reviewer note 仅保存 checksum/length/category，不保存原文、preview output、raw prompt/output 或 tool payload；默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview Approval Readiness Gate：新增只读 `check_default_chat_adapter_controlled_preview_approval_readiness`。它组合 W42 implementation readiness、W44 latest approve evidence、required approved preview count、digest match 和 approved W43 preview AgentRun 当前安全状态；不创建记录、不运行 preview/runtime/tool/model call、不切换 routing；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Implementation Plan Draft：新增只读 `draft_default_chat_adapter_cutover_implementation_plan`。它只调用 W45 readiness；blocked 时不生成 plan sections，ready 时只返回 metadata-safe human-review implementation scope、adapter contract requirements、routing boundary、safety preconditions、fallback、rollback、observability、test plan、explicit non-goals 和 stable plan digest；不创建记录、不运行 preview/runtime/tool/model call、不切换 routing；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Plan Review Evidence：新增 `record_default_chat_adapter_cutover_plan_review_decision` 和只读 `get_default_chat_adapter_cutover_plan_review_summary`。record 会先调用 W46 draft；blocked draft approve 不写 evidence，reject/request_rework 可写 metadata-safe evidence；reviewer note 仅存 checksum/length/category；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Plan Approval Readiness Gate：新增只读 `check_default_chat_adapter_cutover_plan_approval_readiness`。它组合当前 W46 draft、W47 latest approve evidence、plan digest match、W45 readiness 与 default Chat isolation；ready 只表示可进入后续 adapter implementation discussion，不迁移 default Chat；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Route Guard Scaffold：新增共享 `default_chat_adapter` route resolver 和 fail-closed guard。`get_default_chat_adapter_routing_status`、`send_message`、`start_stream_message` 使用同一 route source-of-truth；默认仍为 `legacy_stream` 且 controlled adapter / automatic migration disabled。若未来路径漂移或 adapter 被误启用，默认 Chat 入口会阻断而不是静默切换；不调用 W19-W48 gates，不运行 runtime/tool/model call，不写任何业务数据。
- ✅ Default Chat Adapter Cutover Invocation Harness：新增纯后端 `DefaultChatAdapterCutoverHarness`、`evaluate_default_chat_adapter_cutover_harness` 与 `ensure_default_chat_cutover_harness`。默认 Send / `send_message` / `start_stream_message` 现在只通过该 harness guard 确认 `legacy_guarded` invocation mode、write-disabled/zero-tool/no-runtime/no-model/no-tool/no-business-write 边界；route drift、adapter scaffold 缺失、controlled adapter/automatic migration 误启用或 separate implementation 约束消失时 fail closed。它不是 default Chat migration。
- ✅ Default Chat Adapter Invocation Plan：新增纯后端 `DefaultChatAdapterInvocationPlan`、`plan_default_chat_adapter_invocation` 与 `ensure_default_chat_adapter_invocation_plan`。默认 Send / `send_message` / `start_stream_message` 现在通过 invocation plan guard 明确选择 `legacy_stream`，保留 `controlled_adapter` 为 disabled candidate，并固定 send/stream contract shape、write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write 边界；W50 harness blocking 会让 plan blocking。它不是 default Chat migration。
- ✅ Default Chat Adapter Invocation Boundary：新增纯后端 `DefaultChatAdapterInvocationBoundary`、`evaluate_default_chat_adapter_invocation_boundary` 与 `ensure_default_chat_adapter_invocation_boundary`。默认 Send / `send_message` / `start_stream_message` 现在通过 invocation boundary guard 复用 W51 plan，只允许进入 `legacy_stream` callsite，要求 controlled executor unattached、write-disabled、zero-tool、side-effect-free before legacy entry；W51 plan blocking 会让 boundary blocking。它不是 default Chat migration。
- ✅ Default Chat Adapter Typed Callsite Contract：新增纯后端 `DefaultChatAdapterCallsite`、`DefaultChatAdapterCallsiteContract`、`evaluate_default_chat_adapter_callsite_contract` 与 `ensure_default_chat_adapter_callsite_contract`。默认 Send / `send_message` / `start_stream_message` 现在通过 typed callsite contract guard 分别声明 send/stream contract shape，并校验各自 actual route path 必须保持 `legacy_stream`；W52 boundary blocking 或 callsite route drift 都会 fail closed。它不是 default Chat migration。
- ✅ Authority Roadmap Sync：W54 将高优先级 roadmap 与 execution docs 从旧 W22 状态同步到 W54/W1-W53 当前代码状态，避免后续 Agent 按过期路线开发。它不是 default Chat migration。
- ✅ Default Chat Adapter Ordinary Entry Preflight：W55 新增纯后端 ordinary-entry preflight / side-effect lock。默认 Send / `send_message` / `start_stream_message` 现在通过 preflight guard 明确要求 typed contract ready、legacy entry allowed、controlled executor unattached、default migration disabled 和零副作用预算；route drift 或 contract blocking 会 fail closed。它不是 default Chat migration。
- ✅ Default Chat Adapter Ordinary Entry Preflight Status：W56 新增只读 status command、frontend wrapper 和 Settings evidence surface。它只展示 send/stream W55 preflight 状态、side-effect lock 和 metadata-safe summary；不运行 runtime/model/tool，不写任何业务数据，不迁移 default Chat。
- ✅ Default Chat Adapter Narrow Implementation Discussion Gate：W57 新增只读 discussion gate、frontend wrapper 和 Settings evidence surface。它组合 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status；eligible 只表示可讨论更窄 adapter implementation slice，不运行 runtime/model/tool，不写记录，不切换 routing，不迁移 default Chat。
- ✅ Default Chat Adapter Narrow Implementation Plan Draft：W58 新增只读 `draft_default_chat_adapter_narrow_implementation_plan`、frontend wrapper 和 Settings evidence surface。它先调用 W57 gate；blocked 时不生成 plan sections，eligible 时只返回 metadata-safe human-review plan sections 与 stable digest；不创建记录、不运行 runtime/model/tool/preview、不切换 routing，默认 Send 不调用它。
- ✅ Default Chat Adapter Narrow Implementation Plan Review Evidence：W59 新增 `record_default_chat_adapter_narrow_implementation_plan_review_decision` 与只读 summary、frontend wrapper 和 Settings evidence surface。它先调用 W58 draft；blocked draft approve 不写 evidence，ready draft decision 只写 metadata-safe Evidence；reviewer note 仅 checksum/length/category，默认 Send 不调用它。
- ✅ Default Chat Adapter Narrow Implementation Plan Approval Readiness Gate：W60 新增只读 `check_default_chat_adapter_narrow_implementation_plan_approval_readiness`、frontend wrapper 和 Settings evidence surface。它组合当前 W58 draft、W59 latest approve evidence、digest match、W57 eligible 与 default Chat isolation；不写记录、不运行 runtime/model/tool/preview、不切换 routing，默认 Send 不调用它。

## 当前重要开发方向

1. 保持 `send_message` / `start_stream_message` 默认 Chat 主路径稳定，不能直接替换。
2. 用 Settings Runtime Migration Gate 或 `check_runtime_migration_gate` 对最近 preview AgentRun 做只读迁移诊断。
3. 用 Settings Pilot eligibility 或 `check_controlled_chat_pilot_eligibility` 对最近 3 条 preview gate evidence 做只读资格检查；普通 Chat Send 不调用该 command。
4. Chat 页面 Controlled Pilot 只能由用户显式点击触发；blocked/failed 时显示 fallback，不自动重试；普通 Send 保持可用且不调用 eligibility/gate/preview。
5. Pilot response 默认隔离；只有用户显式点击 `Promote Pilot Response`、确认 review，且当前 target session 与 pilot source session 一致后，才写入一条 ordinary assistant message，并记录 metadata-safe promotion evidence。不得自动 promotion，不得把 promotion 当成默认 Chat 迁移；默认 Send 路径不得调用 evidence recorder。
6. 用 Settings Promotion readiness 或 `check_controlled_pilot_promotion_readiness` 只读判断是否具备讨论下一步 Chat migration 的资格；ready 不是自动迁移许可。
7. 用 Settings Draft Migration Plan 和 Migration Review Decision 进行人工决策记录；approve 只允许进入下一阶段 implementation discussion，不是默认 Chat migration，默认 Send 路径不得调用 review decision record/summary。
8. 用 Settings Implementation Gate 或 `check_controlled_chat_migration_implementation_gate` 只读判断是否具备进入 controlled Chat migration implementation discussion 的资格；eligible 不是默认 Chat migration，默认 Send 路径不得调用 implementation gate。
9. 用 Settings Shadow Run 或 `run_controlled_chat_migration_shadow_run` 做非默认 controlled migration shadow 对比；只有 implementation gate eligible 才执行 runtime，且必须 `allowWrites=false`、metadata-safe、不写 Chat/Proposal/Memory/LifeModel/Evidence/外部工具结果。默认 Send 路径不得调用 shadow run。
10. 用 Settings Shadow Review 或 `record_controlled_chat_migration_shadow_review_decision` 人工记录 shadow run 审阅证据；approve 只是 evidence，不是默认 Chat 迁移许可。默认 Send 路径不得调用 shadow review record/summary。
11. 用 Settings Cutover Readiness 或 `check_controlled_chat_cutover_readiness` 只读判断是否可以进入默认 Chat 迁移实现讨论；eligible 不是默认 Chat migration，默认 Send 路径不得调用 cutover readiness。
12. 用 Settings Cutover Candidate 或 `run_controlled_chat_cutover_candidate` 显式验证 controlled runtime candidate 是否产出 Chat-compatible contract shape；candidateReady 不是默认 Chat migration，默认 Send 路径不得调用 cutover candidate。
13. 用 Settings Cutover Candidate Review 或 `record_controlled_chat_cutover_candidate_review_decision` 人工记录 candidate review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 candidate review record/summary。
14. 用 Settings Candidate Promotion Readiness 或 `check_controlled_chat_cutover_candidate_promotion_readiness` 只读判断 W30/W32 证据是否足以进入后续 adapter boundary / activation planning；ready 不是默认 Chat migration，默认 Send 路径不得调用该 gate。
15. 用 Settings Default Chat Runtime Boundary 或 `get_default_chat_runtime_boundary_status` 只读观察默认 Chat 仍是 legacy stream path；它不是 activation control，默认 Send 路径不得调用该 command。
16. 用 Settings Default Chat Adapter Activation Plan 或 `draft_default_chat_adapter_activation_plan` 只读生成人工 activation plan draft；draftReady 不是 migration approval，默认 Send 路径不得调用该 command。
17. 用 Settings Default Chat Adapter Activation Review Decision 或 `record_default_chat_adapter_activation_review_decision` 人工记录 activation plan 审阅证据；approve 不是默认 Chat 迁移许可，默认 Send 路径不得调用 record/summary command。
18. 用 Settings Default Chat Adapter Activation Implementation Gate 或 `check_default_chat_adapter_activation_implementation_gate` 只读判断 W35/W36 证据是否足以进入 separate implementation discussion；eligible 不是默认 Chat migration，默认 Send 路径不得调用该 gate。
19. 用 Settings Default Chat Adapter Routing Status 或 `get_default_chat_adapter_routing_status` 只读观察 adapter scaffold 仍为 disabled；它不是 routing switch，默认 Send 路径不得调用该 command。
20. 用 Settings Default Chat Adapter Contract Harness 或 `check_default_chat_adapter_contract_harness` 只读验证 disabled adapter contract；它不是 adapter implementation，默认 Send 路径不得调用该 command。
21. 用 Settings Default Chat Adapter Dry Run 或 `run_default_chat_adapter_dry_run` 显式验证未来 adapter invocation contract 的 write-disabled 形状；它不是默认 Chat migration，默认 Send 路径不得调用该 command。
22. 用 Settings Default Chat Adapter Dry Run Review 或 `record_default_chat_adapter_dry_run_review_decision` 人工记录 dry-run review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
23. 用 Settings Default Chat Adapter Implementation Readiness 或 `check_default_chat_adapter_implementation_readiness` 只读判断 W37/W39/W40/W41 证据是否足以进入真正 adapter implementation coding discussion；implementationReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
24. 用 Settings Default Chat Adapter Controlled Preview 或 `run_default_chat_adapter_controlled_preview` 显式验证 W42 之后的非默认 adapter preview 是否能返回 Send-compatible shape；previewReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
25. 用 Settings Default Chat Adapter Controlled Preview Review 或 `record_default_chat_adapter_controlled_preview_review_decision` 人工记录 controlled preview review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
26. 用 Settings Default Chat Adapter Controlled Preview Approval Readiness 或 `check_default_chat_adapter_controlled_preview_approval_readiness` 只读判断 W42/W44 证据和 approved preview AgentRun 当前安全状态是否足以进入后续 adapter cutover implementation discussion；ready 不是默认 Chat migration，默认 Send 路径不得调用该 command。
27. 用 Settings Default Chat Adapter Cutover Implementation Plan 或 `draft_default_chat_adapter_cutover_implementation_plan` 只读生成 W45 readiness 之后的人工 cutover implementation plan draft；draftReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
28. 用 Settings Default Chat Adapter Cutover Plan Review 或 `record_default_chat_adapter_cutover_plan_review_decision` 人工记录 cutover plan review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
29. 用 Settings Default Chat Adapter Cutover Plan Approval Readiness 或 `check_default_chat_adapter_cutover_plan_approval_readiness` 只读判断 W46/W47 证据、plan digest match 和 default Chat isolation 是否足以进入后续 adapter implementation discussion；ready 不是默认 Chat migration，默认 Send 路径不得调用该 command。
30. W49 的 default Chat route guard 是纯后端 fail-closed 守卫；默认 Send / `send_message` / `start_stream_message` 可以调用它确认当前仍为 `legacy_stream`，但不能借此启用 controlled adapter 或自动迁移。
31. W50 的 default Chat adapter cutover invocation harness 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它确认 `legacy_guarded`、write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write 边界，不能借此调用 controlled adapter 或自动迁移。
32. W51 的 default Chat adapter invocation plan 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它选择 `legacy_stream` 并声明 `controlled_adapter` 仍是 disabled candidate，不能借此 attach controlled executor 或自动迁移。
33. W52 的 default Chat adapter invocation boundary 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它确认当前 callsite 必须进入 `legacy_stream` 且在 legacy entry 前无 runtime/model/tool/business write 副作用，不能借此接入 controlled executor 或自动迁移。
34. W53 的 default Chat adapter typed callsite contract 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它通过类型化 callsite 绑定 `send_message_compatible` / `stream_message_compatible` contract shape 和各自 legacy route path，不能借此接入 controlled executor 或自动迁移。
35. W54 的 authority roadmap sync 是文档治理工作；高优先级路线文件必须与当前代码状态同步，不能再按旧 W22 “下一步”误导后续开发。
36. W55 的 default Chat adapter ordinary-entry preflight 是纯后端 side-effect lock；默认 Send / `send_message` / `start_stream_message` 只能用它确认 typed contract ready、legacy entry allowed、controlled executor unattached 和零副作用预算，不能借此调用 controlled adapter 或自动迁移。
37. W56 的 default Chat adapter ordinary-entry preflight status 是只读 evidence surface；Settings 可显式刷新它，但普通 Send 路径不得调用该 command，也不能把 statusReady 解释为迁移许可。
38. W57 的 default Chat adapter narrow implementation discussion gate 是只读讨论资格 gate；Settings 可显式检查 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status 是否同时干净，但普通 Send 路径不得调用该 command，也不能把 eligible 解释为迁移许可。
39. W58 的 default Chat adapter narrow implementation plan draft 是只读人工评审草案；Settings 可显式生成 metadata-safe plan sections 和 stable digest，但普通 Send 路径不得调用该 command，也不能把 draftReady 解释为迁移许可。
40. W59 的 default Chat adapter narrow implementation plan review evidence 只记录人工 review metadata；Settings 可显式 approve/reject/request_rework 并读取 summary，但普通 Send 路径不得调用该 command，也不能把 approval 解释为迁移许可。
41. W60 的 default Chat adapter narrow implementation plan approval readiness 是只读 gate；Settings 可显式检查当前 W58 draft 与 W59 approve evidence 是否仍匹配且 default Chat isolation 仍干净，但普通 Send 路径不得调用该 command，也不能把 ready 解释为迁移许可。

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder 发送后模型没有变化：先确认 Proposal 是否仍在 Mailbox 待处理；Builder 默认不会绕过确认直接写入。

## License

MIT License
