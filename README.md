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

当前项目处于 **W57 Default Chat Adapter Narrow Implementation Discussion Gate** 阶段：

- **ReAct 执行闭环已建立**：AgentLoop 迭代执行、Action Parser JSON envelope、Tool Registry 统一注册、Permission/Proposal/Replay 闭合。
- **W1-W57 已完成**：当前已经建立 Runtime Migration Gate 只读诊断层、Settings evidence surface、controlled Chat migration pilot eligibility 只读资格检查、Chat 页面显式单轮 Controlled Pilot、reviewed pilot response promotion、promotion 后的 source/target session 验证、metadata-safe promotion evidence 记录与只读 summary、基于既有 promotion evidence 的只读 readiness gate、reviewed migration plan draft 只读草案生成器、人工 migration review decision metadata-safe evidence 记录阶段、只读 implementation gate、非默认 controlled migration shadow run、人工 shadow review decision metadata-safe evidence 记录、只读 cutover planning readiness gate、非默认 cutover candidate adapter、人工 cutover candidate review metadata-safe evidence 闭环、只读 cutover candidate promotion readiness gate、只读 default Chat runtime boundary status、只读 default Chat adapter activation plan draft、人工 activation review decision metadata-safe evidence、只读 default Chat adapter activation implementation gate、只读 default Chat adapter disabled routing scaffold、只读 default Chat adapter contract harness、write-disabled default Chat adapter dry-run invocation boundary、人工 dry-run review decision metadata-safe evidence、只读 default Chat adapter implementation readiness gate、显式非默认 default Chat adapter controlled preview、人工 controlled preview review decision metadata-safe evidence、只读 controlled preview approval readiness gate、只读 default Chat adapter cutover implementation plan draft、人工 cutover plan review decision metadata-safe evidence、只读 cutover plan approval readiness gate、共享 default Chat adapter route guard scaffold、纯后端 default Chat adapter cutover invocation harness、default Chat adapter invocation plan、default Chat adapter invocation boundary、typed callsite contract、authority roadmap sync、ordinary entry preflight / side-effect lock、ordinary entry preflight status surface，以及只读 narrow implementation discussion gate；完整状态索引见 [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)。
- **ReAct 仍是当前默认 Chat 主链路**：MultiStrategy Runtime 已有 preview command 和 audit-ready 路径，但尚未接管默认 `send_message` / Chat 主流程。
- **MultiStrategy preview 已可审计**：`run_multi_strategy_agent_preview` 已存在，preview run 会写入 metadata-safe 外层 AgentRun audit；Runs / Trace 已能展示 preview strategy、payload、governance 和 warnings。
- **Runtime Migration Gate 已建立**：`check_runtime_migration_gate` 只读取既有 preview AgentRun / audit，输出 `defaultChatUnchanged`、`previewPathHealthy`、`metadataSafeTraceReady`、`fallbackAvailable`、`noExternalWrites`、`proposalFirstPreserved` 和 `blockingReasons`；它不执行 ReAct、PlanExecute、工具调用或外部写入。
- **Gate evidence surface 已可见**：Settings / 实验区域的 Runtime Migration Gate 面板可显式调用 `check_runtime_migration_gate`，展示 pass/block 与 blocking reasons；它不是 Chat 切换开关，也不会自动运行 preview。
- **Pilot Eligibility 已可见**：`check_controlled_chat_pilot_eligibility` 默认只读检查最近 3 条 MultiStrategy preview AgentRun 的 gate report 是否连续干净，并返回 `eligible`、clean count、checked run ids、blocking reasons 和 last gate report。Settings / 实验区域展示 controlled Chat migration pilot 资格；它不是 Chat 切换开关，即使 eligible 也不会自动替换默认 Chat。
- **Controlled Pilot 已进入 Chat 页面**：用户必须显式点击 `Run Controlled Pilot`；执行前先调用 `check_controlled_chat_pilot_eligibility`，blocked 时只展示 blocking reasons 和 fallback，不调用 preview；eligible 时才调用 `run_multi_strategy_agent_preview`，并强制 `allowWrites=false`。成功结果默认仍以独立 “Pilot response” 展示，不自动作为普通 assistant message，不自动写入普通 chat history。
- **Reviewed Pilot Response Promotion 已完成**：只有成功且包含 `userOutput` 的 pilot response 才显示 `Promote Pilot Response`。用户点击后会进入 review/confirmation 状态，展示 response 文本、runId、selected strategy、governance summary、payload summary 和“确认后将写入当前聊天历史”的提示；确认后仅通过现有 chat message 保存机制写入一条 ordinary assistant message，并在可用时保留 `run_id` trace。取消、blocked、failed、no-output 和重复 promotion 都不会写入。
- **Post-Promotion Validation 已完成**：Controlled Pilot 成功后会绑定发起时的 source chat session；promotion review 展示 source session、target session、runId、strategy 和 governance summary。确认前必须校验当前 target session 与 source session 一致；若用户切换 session 后尝试 promotion，会阻止写入新 session，显示 blocking/fallback 文案，并提示在当前 session 重新运行 Controlled Pilot。
- **Promotion Evidence Recorder 已完成**：promotion confirm 在成功保存 ordinary assistant message 后，会通过 `EvidenceStore` 记录一条 metadata-safe runtime evidence，包含 pilotRunId、source/target session、strategy/payload/governance、promoted message length、checksum 和 promotedAt；不持久化 raw pilot response、raw user prompt 或 full tool payload。Settings / 实验区域提供只读 summary，显示 promoted count、recent promoted pilot run ids、latest timestamp 和 mismatch block count。若 evidence 记录失败，UI 显示 degraded/error，重试只补 evidence，不重复写 chat message。
- **Promotion Readiness Gate 已完成**：`check_controlled_pilot_promotion_readiness` 只读取 W23 promotion evidence，默认要求 3 条 metadata-safe promotion evidence，返回 ready、requiredPromotions、promotedCount、recent run ids、latest timestamp、source/target mismatch block count、metadataSafeEvidenceReady、defaultChatUnchanged 和 blockingReasons。`sessionId` 参数已预留；当前 EvidenceStore summary 仍是 global summary。gate pass 只表示“可进入下一阶段讨论”，不是自动迁移许可。
- **Reviewed Migration Plan Draft 已完成**：`draft_controlled_chat_migration_plan` 复用 W24 readiness gate 结果。readiness blocked 时仅返回 `draftReady=false` 与 blocking reasons，不生成可执行迁移方案；readiness passed 时返回 migration scope、required preconditions、rollback/fallback/test plan，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`。该 command 不切换 default Chat、不改 feature flag、不创建 AgentRun/Proposal/Memory/LifeModel patch/promotion evidence，也不输出 raw user content、raw assistant output 或 tool payload。
- **Manual Migration Review Decision Evidence 已完成**：`record_controlled_chat_migration_review_decision` 会先调用 W25 draft command，再记录用户显式 `approve` / `reject` / `request_rework` 决策。`draftReady=false` 时拒绝记录 approve 且不写 evidence；ready draft 的 decision 只写 metadata-safe EvidenceStore 记录，包含 `evidenceKind=migration_review_decision`、`metadataSafe=true`、draftReady、decisionKind、readiness counts、draft hash 和 createdAt。reviewer note 不原文存储，仅保存 length、checksum 和 bounded category。`get_controlled_chat_migration_review_decision_summary` 只读返回 latest decision、approved count、rework/reject count、latest timestamp 和 blockers，不读取 raw transcript，不创建 AgentRun/Proposal/Memory/LifeModel patch。
- **Approved Migration Implementation Gate 已完成**：`check_controlled_chat_migration_implementation_gate` 只读读取当前 W24 readiness、当前 W25 draft hash 和 W26 metadata-safe review decision evidence，返回 `implementationEligible`、`latestDecision`、`readinessReport`、`draftHashMatched`、`approvedAfterLatestDraft` 和 `blockingReasons`。只有 latest metadata-safe decision 为 `approve`、当前 readiness 通过、且 approved evidence draftHash 匹配当前 draft hash 时才 eligible；latest `reject` / `request_rework`、hash mismatch 或 readiness blocked 都会阻断。eligible 只表示可进入 controlled Chat migration implementation discussion，不会切换 default Chat。
- **Controlled Migration Shadow Run 已完成**：`run_controlled_chat_migration_shadow_run` 是非默认、显式触发的 shadow command。它先调用 W27 implementation gate；blocked 时直接返回 blockers，不执行 controlled runtime。eligible 时才用 bounded descriptor 运行 write-disabled controlled runtime preview，并可写入 metadata-safe `controlled_migration_shadow_run` AgentRun audit。返回值只包含 strategy/payload、metadata-safe summary、warnings 和 blockers；不返回 raw user prompt、raw assistant output 或 full tool payload，不写 Chat message、Proposal、Memory、LifeModel patch、Evidence 或外部工具结果。Settings 实验区提供 Shadow Run 面板，明确“不保存到 Chat，不切换 default Chat”。
- **Shadow Review Evidence 已完成**：`record_controlled_chat_migration_shadow_review_decision` 和 `get_controlled_chat_migration_shadow_review_summary` 只服务人工审阅 W28 shadow run 结果。任何 `approve` / `reject` / `request_rework` decision 都仅允许记录在已完成、`reasoning_strategy=controlled_migration_shadow_run`、`allowWrites=false`、`metadataSafe=true` 且无 Chat/Proposal/Memory/LifeModel patch/external write 副作用的 AgentRun 上。Evidence metadata 字段白名单为 `shadowRunId`、`decisionKind`、`reviewerNoteChecksum`、`reviewerNoteLength`、`reviewerNoteCategory`、`readinessSummaryDigest`、`createdAt`；不保存 reviewer 原文、shadow prompt、shadow output 或 tool payload。Settings Shadow Review 区域只能人工点击记录/读取，不自动触发 review。
- **Cutover Planning Readiness Gate 已完成**：`check_controlled_chat_cutover_readiness` 是只读 command。它要求 W27 implementation gate 当前 eligible、latest W29 shadow review decision 为 `approve`、approved `shadowRunId` 对应 AgentRun 仍存在且 completed / `controlled_migration_shadow_run` / `allowWrites=false` / `metadataSafe=true` / 无副作用。返回 `cutoverPlanningEligible`、W27 report、latest W29 decision、verified shadow run id、readiness digest、defaultChatUnchanged、requiredEvidenceReady、blockers 和 metadata-safe summary；不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 ReAct、PlanExecute、shadow run 或 preview。Settings Cutover Readiness 只能显式点击检查，不自动触发。
- **Non-Default Cutover Candidate Adapter 已完成**：`run_controlled_chat_cutover_candidate` 是显式、非默认 candidate command。它先调用 W30 `check_controlled_chat_cutover_readiness`；未 eligible 时直接返回 `contractShape=blocked`、blocking reasons 和 metadata-safe summary，不运行 runtime。eligible 后才执行一次 controlled runtime candidate，并强制 `allowWrites=false`、`maxToolCalls=0`、不 apply Proposal、不写 Memory、不 patch LifeModel、不做 external write。返回 `candidateReady`、`candidateRunId`、`outputPreview`/`userOutput`、`contractShape`、`metadataSafeSummary`、warnings 和 blockers，用于验证默认 Chat response contract 兼容形状。它只允许创建 metadata-safe `controlled_chat_cutover_candidate` AgentRun audit，不保存 raw user prompt、raw assistant output、tool payload、Chat message、Proposal、Memory、LifeModel patch、Evidence、MCP audit 或外部工具结果。Settings Cutover Candidate 只能人工点击运行，不提供保存到 Chat、promotion 或自动调用。
- **Cutover Candidate Review Evidence 已完成**：`record_controlled_chat_cutover_candidate_review_decision` 和 `get_controlled_chat_cutover_candidate_review_summary` 只服务人工审阅 W31 candidate 结果。只允许 `approve` / `reject` / `request_rework`；approve 必须绑定已完成、`reasoning_strategy=controlled_chat_cutover_candidate`、`contractShape=send_message_compatible`、`candidateReady=true`、`allowWrites=false`、`maxToolCalls=0`、`metadataSafe=true` 且无 Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/external write 副作用的 AgentRun。Evidence metadata 白名单为 `candidateRunId`、`decisionKind`、`contractShape`、`candidateSummaryDigest`、`reviewerNoteChecksum`、`reviewerNoteLength`、`reviewerNoteCategory`、`createdAt`；不保存 reviewer 原文、candidate userOutput、raw prompt、raw assistant output 或 tool payload。Summary 只读，不创建记录；Settings 只提供显式记录/刷新，不提供保存到 Chat、promotion、migration 或 feature flag 操作。
- **Cutover Candidate Promotion Readiness Gate 已完成**：`check_controlled_chat_cutover_candidate_promotion_readiness` 只读检查 W30 cutover readiness、W32 metadata-safe candidate review approval evidence、latest decision、approved candidate AgentRun 当前安全状态和 default Chat isolation。它返回 `ready`、approved candidate counts、latest decision、candidate blockers、`defaultChatUnchanged` 和 metadata-safe summary；不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call。Settings 只提供显式刷新面板，不提供默认 Chat 切换动作。
- **Default Chat Runtime Boundary Status 已完成**：`get_default_chat_runtime_boundary_status` 只读返回当前默认 Chat runtime boundary，固定声明 `currentMode=legacy_stream`、`defaultChatUnchanged=true`、`automaticMigrationEnabled=false`、`controlledCandidateAvailable=false` 和 `candidatePromotionReadinessRequired=true`。它不读取/写入 runtime/evidence/proposal/memory/lifemodel/chat/tool/model 状态，只把默认 Chat 仍是 legacy stream path 这件事显式化、可测试化、可展示化。Settings 只提供显式刷新面板，不提供 switch/migrate/enable 操作。
- **Default Chat Adapter Activation Plan Draft 已完成**：`draft_default_chat_adapter_activation_plan` 组合 W33 candidate promotion readiness 和 W34 default Chat boundary status，返回人工 review-only activation plan draft。blocked 时不生成 plan sections；ready 时只返回 activation scope、preconditions、adapter contract checks、fallback、rollback、observability 和 test plan，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`、`requiresSeparateImplementation=true`。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call，不切换 feature flag。Settings 只提供显式刷新面板，不提供 switch/migrate/enable 操作。
- **Default Chat Adapter Activation Review Decision Evidence 已完成**：`record_default_chat_adapter_activation_review_decision` 会先调用 W35 draft command，再记录用户显式 `approve` / `reject` / `request_rework` 决策。`draftReady=false` 时拒绝记录 approve 且不写 evidence；ready draft 的 decision 只写 metadata-safe EvidenceStore 记录，字段限于 decision、draftReady、activationPlanDigest、candidatePromotionReady、currentMode、automaticMigrationEnabled、reviewer note checksum/length/category 和 createdAt。`get_default_chat_adapter_activation_review_summary` 只读返回 latest decision、approved count、reject/rework count、latest timestamp、blockers 和 metadata-safe summary。Settings 只提供显式记录/刷新，不提供 switch/migrate/enable 操作。
- **Default Chat Adapter Activation Implementation Gate 已完成**：`check_default_chat_adapter_activation_implementation_gate` 只读组合当前 W35 activation plan stable digest 与 W36 metadata-safe latest review decision evidence。只有当前 draft ready、latest decision 为 approve、digest match、candidate promotion ready、default Chat 仍是 `legacy_stream` 且 automatic migration disabled 时才 eligible；reject/request_rework、draft blocked、digest mismatch 或 boundary drift 都会阻断。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call，不切换 feature flag。Settings 只提供显式检查面板，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Disabled Routing Scaffold 已完成**：`get_default_chat_adapter_routing_status` 只读调用 W37 implementation gate，并固定返回 `currentMode=legacy_stream`、`adapterScaffoldPresent=true`、`controlledAdapterEnabled=false`、`defaultSendPath=legacy_stream`、`startStreamPath=legacy_stream`、`activationImplementationGateEligible`、`requiresSeparateCutoverImplementation=true`、blocking reasons 和 metadata-safe summary。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call，不切换 default Chat routing。Settings 只提供显式刷新面板，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Contract Harness 已完成**：`check_default_chat_adapter_contract_harness` 只读调用 W38 routing status，验证 send_message 和 start_stream_message contract 均仍指向 `legacy_stream`、controlled adapter 仍 disabled、activation implementation gate 当前 eligible，并返回 metadata-safe contract checks。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call，不切换 routing。Settings 只提供显式检查面板，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Dry-Run Invocation Boundary 已完成**：`run_default_chat_adapter_dry_run` 是显式、非默认、write-disabled dry-run command。它先调用 W39 contract harness；blocked 时不运行 adapter dry run，只返回 blockers；ready 时只返回 send-message-compatible 的 metadata-safe dry-run contract result，并强制 `allowWrites=false`、`maxToolCalls=0`、`defaultChatPathUnchanged=true`。它不保存 Chat message，不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/external write，不运行 runtime/tool/model call，不切换 routing。Settings 只提供显式 dry-run 面板，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Dry-Run Review Evidence 已完成**：`record_default_chat_adapter_dry_run_review_decision` 会先重新运行 W40 dry run 检查。approve 只有在 dry run ready 时才记录 evidence；blocked dry-run approve 不写 evidence；reject/request_rework 可记录 metadata-safe review evidence。`get_default_chat_adapter_dry_run_review_summary` 只读返回 latest decision、approved/reject-or-rework count、latest timestamp、blockers 和 metadata-safe summary。reviewer note 只保存 checksum/length/category；不保存 raw note、raw prompt、candidate output 或 tool payload。Settings 只提供显式记录/刷新，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Implementation Readiness Gate 已完成**：`check_default_chat_adapter_implementation_readiness` 只读组合 W37 activation implementation gate、W39 contract harness、W40 dry run 和 W41 latest dry-run review decision evidence。只有 activation gate eligible、contract harness ready、dry run ready、latest dry-run review 为 approve、当前 dry-run digest 与 approved review digest 匹配、default Chat 仍为 `legacy_stream`、controlled adapter disabled 且 automatic migration disabled 时才 ready。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 runtime/tool/model call，不切换 routing。Settings 只提供显式检查面板，不提供 switch/migrate/enable/activate 操作。
- **Default Chat Adapter Controlled Preview 已完成**：`run_default_chat_adapter_controlled_preview` 是显式、非默认 W43 command。它先调用 W42 implementation readiness；blocked 时不运行 runtime、不创建 AgentRun、不写 Chat/Evidence/Proposal/Memory/LifeModel/MCP audit；ready 时才运行一次 `allowWrites=false`、`maxToolCalls=0` 的 controlled preview，返回 SendMessageResult-compatible shape，并只允许 metadata-safe adapter preview AgentRun audit。它不保存 Chat message、不 promotion、不切换 feature flag、不改变 default Chat routing。Settings 只提供显式 preview 面板，不提供 save/promote/switch/migrate/enable/activate 操作。
- **Default Chat Adapter Controlled Preview Review Evidence 已完成**：`record_default_chat_adapter_controlled_preview_review_decision` 和 `get_default_chat_adapter_controlled_preview_review_summary` 只服务人工审阅 W43 controlled preview。Approve 必须绑定已完成、`reasoning_strategy=default_chat_adapter_controlled_preview`、`contractShape=send_message_compatible`、`previewReady=true`、`allowWrites=false`、`maxToolCalls=0`、`metadataSafe=true` 且无 Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/external write 副作用的 AgentRun；reject/request_rework 可对结构安全的 preview run 记录 metadata-safe evidence。Evidence 白名单仅包含 previewRunId、decisionKind、contractShape、previewSummaryDigest、reviewer note checksum/length/category 和 createdAt；summary 只读，不保存 reviewer 原文、raw prompt/output、preview userOutput 或 tool payload。
- **Default Chat Adapter Controlled Preview Approval Readiness Gate 已完成**：`check_default_chat_adapter_controlled_preview_approval_readiness` 只读组合 W42 implementation readiness、W44 latest metadata-safe review decision、required approved preview count、approved preview digest match，以及 approved W43 preview AgentRun 当前 completed / send-message-compatible / previewReady / write-disabled / zero-tool / metadata-safe / side-effect-free 状态。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 controlled preview/runtime/tool/model call，不切换 routing 或 feature flag。Settings 只提供显式检查面板，不提供 save/promote/switch/migrate/enable/activate 操作。
- **Default Chat Adapter Cutover Implementation Plan Draft 已完成**：`draft_default_chat_adapter_cutover_implementation_plan` 只读调用 W45 readiness。blocked 时返回 `draftReady=false`、blocking reasons 和空 plan sections；ready 时只返回 metadata-safe human-review implementation scope、adapter contract requirements、routing boundary、safety preconditions、fallback、rollback、observability、test plan、explicit non-goals 与 stable plan digest。它不创建 AgentRun/Evidence/Proposal/Memory/LifeModel patch/MCP audit/chat message，不运行 controlled preview/runtime/tool/model call，不切换 routing 或 feature flag。Settings 只提供显式 draft 面板，不提供 save/promote/switch/migrate/enable/activate 操作。
- **Default Chat Adapter Cutover Plan Review Decision Evidence 已完成**：`record_default_chat_adapter_cutover_plan_review_decision` 会先调用 W46 draft；blocked draft 的 approve 不写 evidence，reject/request_rework 可记录 metadata-safe 人工 review evidence。`get_default_chat_adapter_cutover_plan_review_summary` 只读返回 latest decision、approved/rejected/request_rework counts、latest approved plan digest、latest timestamp、blockers 和 metadata-safe summary。reviewer note 仅保存 checksum/length/category；不保存 raw note、raw prompt/output、tool payload 或 plan review 原文。Settings 只提供显式记录/刷新，不提供 save/promote/switch/migrate/enable/activate 操作。
- **Default Chat Adapter Cutover Plan Approval Readiness Gate 已完成**：`check_default_chat_adapter_cutover_plan_approval_readiness` 只读组合当前 W46 draft、W47 latest review evidence、plan digest match、W45 readiness 和 default Chat isolation。只有 latest decision 为 approve、当前 plan digest 匹配、W45 仍 ready、default Send/stream 仍为 `legacy_stream`、controlled adapter disabled 且 automatic migration disabled 时才 ready。它不创建记录、不运行 preview/runtime/tool/model call、不切换 routing；ready 只是后续 adapter implementation discussion readiness，不是 default Chat migration。
- **Default Chat Adapter Cutover Route Guard Scaffold 已完成**：新增 `src-tauri/src/default_chat_adapter.rs` 作为共享 route source-of-truth。默认解析结果仍固定为 `legacy_stream`、`controlledAdapterEnabled=false`、`automaticMigrationEnabled=false`，`get_default_chat_adapter_routing_status` 与默认 `send_message` / `start_stream_message` 入口都使用同一个 route guard；若未来路径被误改为非 legacy 或 adapter 被误启用，默认 Chat 会 fail closed，而不是静默切换。它不调用 W19-W48 gates，不运行 preview/runtime/tool/model call，不写 Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/外部工具结果，不迁移 default Chat。
- **Default Chat Adapter Cutover Invocation Harness 已完成**：W50 将默认 `send_message` / `start_stream_message` 入口升级为调用共享 `evaluate_default_chat_adapter_cutover_harness` / `ensure_default_chat_cutover_harness`。该 harness 只允许 `legacy_guarded` invocation mode，固定 `allowWrites=false`、`maxToolCalls=0`、runtime/model/tool calls disabled、controlled adapter invocation disabled，并在 route drift、adapter scaffold 缺失、automatic migration 启用或 separate cutover implementation 约束消失时 fail closed。它不调用 W19-W49 gates，不运行 preview/runtime/tool/model call，不写任何业务数据，不迁移 default Chat。
- **Default Chat Adapter Invocation Plan 已完成**：W51 新增纯后端 `DefaultChatAdapterInvocationPlan`、`plan_default_chat_adapter_invocation` 与 `ensure_default_chat_adapter_invocation_plan`。默认 `send_message` / `start_stream_message` 现在通过 invocation plan guard 明确选择 `legacy_stream`，保留 `controlled_adapter` 作为 disabled candidate，固定 send/stream contract shape、write-disabled、zero-tool、runtime/model/tool calls disabled、controlled executor unattached；当 W50 harness blocking 时 invocation plan 同步 blocked。它不调用 W19-W50 gates，不运行 preview/runtime/tool/model call，不写任何业务数据，不迁移 default Chat。
- **Default Chat Adapter Invocation Boundary 已完成**：W52 新增纯后端 `DefaultChatAdapterInvocationBoundary`、`evaluate_default_chat_adapter_invocation_boundary` 与 `ensure_default_chat_adapter_invocation_boundary`。默认 `send_message` / `start_stream_message` 现在通过 boundary guard 复用 W51 plan，只允许进入 `legacy_stream` callsite，要求 controlled executor unattached、write-disabled、zero-tool、side-effect-free before legacy entry；当 W51 plan blocking 时 boundary 同步 blocked。它不调用 W19-W51 gates，不运行 preview/runtime/tool/model call，不写任何业务数据，不迁移 default Chat。
- **Default Chat Adapter Typed Callsite Contract 已完成**：W53 新增纯后端 `DefaultChatAdapterCallsite`、`DefaultChatAdapterCallsiteContract`、`evaluate_default_chat_adapter_callsite_contract` 与 `ensure_default_chat_adapter_callsite_contract`。默认 `send_message` / `start_stream_message` 现在通过 typed contract guard 分别声明 `send_message_compatible` / `stream_message_compatible`，并校验各自实际 route path 必须保持 `legacy_stream`；W52 boundary blocking 或 callsite route drift 都会 fail closed。它不调用 W19-W52 gates，不运行 preview/runtime/tool/model call，不写任何业务数据，不迁移 default Chat。
- **Authority Roadmap Sync 已完成**：W54 将高优先级路线文档重新对齐到 W1-W53 当前代码状态，避免后续 Agent 被仍停在 W22 的旧路线误导。它只更新文档权威入口，不修改 runtime code，不迁移 default Chat。
- **Default Chat Adapter Ordinary Entry Preflight 已完成**：W55 新增纯后端 `DefaultChatAdapterOrdinaryEntryPreflight`、`evaluate_default_chat_adapter_ordinary_entry_preflight` 与 `ensure_default_chat_adapter_ordinary_entry_preflight`。默认 `send_message` / `start_stream_message` 现在通过 ordinary-entry preflight guard 明确要求 typed contract ready、legacy entry allowed、controlled executor unattached、default Chat migration disabled、runtime/model/tool calls disabled、`allowWrites=false`、`maxToolCalls=0`、无 Chat/AgentRun/Evidence 预写入。它不调用 W19-W54 gates，不运行 preview/runtime/tool/model call，不写任何业务数据，不迁移 default Chat。
- **Default Chat Adapter Ordinary Entry Preflight Status 已完成**：W56 新增只读 `get_default_chat_adapter_ordinary_entry_preflight_status`、前端 wrapper 与 Settings 面板。它只读取当前 route 和 W55 pure preflight，返回 send/stream preflight、side-effect lock、default Chat unchanged 与 metadata-safe summary；不接 controlled executor，不调用 runtime/model/tool、migration gate、preview 或 evidence recorder，不写 Chat/AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit，也不迁移 default Chat。
- **Default Chat Adapter Narrow Implementation Discussion Gate 已完成**：W57 新增只读 `check_default_chat_adapter_narrow_implementation_discussion_gate`、前端 wrapper 与 Settings 面板。它组合 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status，只有当前 cutover plan approval 仍 ready、send/stream preflight 均 ready、default Chat 仍保持 `legacy_stream`、controlled adapter disabled 且 automatic migration disabled 时才 eligible。它不创建记录、不运行 runtime/model/tool call、不切换 routing，只表示是否可讨论一个更窄的 adapter implementation slice，不是 default Chat migration。
- **PlanExecute V1 是受治理 runtime slice**：当前可通过 MultiStrategy preview 产生 planExecute payload/report，但不是产品化周计划流程。
- **LifeModel-HS 仍是协议层方向**：Maturation V1 service、Evidence/Governor 等基础能力已存在，但 Chat 自动成熟化和产品化反馈闭环仍需 gate。
- **RuntimeStrategy trait 已成型**：MultiStrategy Runtime 通过固定 ReAct / PlanExecute adapter registry 执行；这不是插件化加载，也不是默认 Chat 替换。
- **ModelRouter 已毕业**：移除 experimental flag，成为默认路由基础设施。
- **Execution Tools 分层落地**：P1 工具必须有真实 executor 或明确的 proposal-only governed executor 和治理测试；`calendar.propose_event` / `email.propose_draft` 当前只创建 `ScheduledTask` / `DataExport` proposal，不执行真实日历写入或邮件发送。
- **Core OS Tools 注册**：life_model.read、goal.read、memory.search、proposal.list 等 9 个 builtin 工具。
- **下一步仍不能直接替换默认 Chat**：W57 只是 narrow implementation discussion gate，W56 只是 ordinary-entry preflight status surface，W55 只是 ordinary-entry preflight / side-effect lock，W54 只是 authority roadmap sync，W53 只是 default Chat adapter typed callsite contract，它们都不是 default Chat migration。previewReady、review approval、approval readiness、cutover plan draft、cutover plan review approval、cutover plan approval readiness、route guard、cutover invocation harness、invocation plan、invocation boundary、typed callsite contract、ordinary-entry preflight、ordinary-entry preflight status 与 narrow implementation discussion gate 只能作为后续人工审阅与更窄 adapter implementation discussion 的证据；默认 `Send` / `send_message` / `start_stream_message` 路径保持 `legacy_stream`，只允许调用共享 pure ordinary-entry preflight guard，不得调用 eligibility、gate、preview、promotion、evidence recorder、promotion readiness gate、migration draft、migration review decision、implementation gate、shadow run、shadow review、cutover readiness、cutover candidate、candidate review、candidate promotion readiness、default Chat boundary status、activation plan draft、activation review decision、activation implementation gate、adapter routing status、contract harness、ordinary-entry preflight status、narrow implementation discussion gate、dry-run command、dry-run review command、implementation readiness command、controlled preview command、controlled preview review command、controlled preview approval readiness command、cutover implementation plan draft command、cutover plan review command 或 cutover plan approval readiness command。
- **文档与 taxonomy 同步**：入口文档和 Tool Taxonomy 必须随代码状态更新，避免后续 Agent 按过期 P1/P2 标签开发。
- **双轨架构**：`use_agent_loop` feature flag 控制 Chat 路径，旧路径完整保留作为 fallback。
- **UI 最小收敛**：导航聚焦 Chat/Review/Runs/Settings，Settings 新增 safe paths 和 AgentLoop toggle。
- **`make ci` 为发布门控**：文档不写死测试数量；以本地 `make ci` 最新结果为准。

下一阶段总纲和架构基准文档见：

- [Plans Document Governance](/Users/fujing/Desktop/偶来福/plans/README.md)
- [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
- [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
- [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
- [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)

Post-Beta 的下一阶段是 LifeModel-HS MVP：把当前 LifeModel 从 YAML 兼容视图升级为受治理的 Personal Heuristic System。实现入口见：

- [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
- [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
- [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)

## 核心能力

| 能力 | 当前状态 | 目标形态 |
|---|---|---|
| LifeModel | 已有四维模型和编辑器 | 成为所有 Agent 任务的私人上下文层 |
| Builder | 已支持快速、渐进、苏格拉底式构建；默认只创建 Proposal | 通过 Review Center 确认后安全写入 LifeModel |
| Chat | 已支持流式对话、历史持久化、AgentRun 和 Chat Proposal；默认主链路尚未切到 MultiStrategy Runtime | 继续稳定迁移受控子路径，展示上下文、模型路由和运行轨迹 |
| MultiStrategy Runtime | Preview/audit-ready：`run_multi_strategy_agent_preview` 可选择 ReAct/PlanExecute/Blocked payload，并写入 metadata-safe 外层 AgentRun audit；Settings Runtime Migration Gate、Pilot eligibility、Promotion evidence summary、Promotion readiness gate、Draft Migration Plan、Migration Review Decision、Implementation Gate、Shadow Run、Shadow Review、Cutover Readiness、Cutover Candidate、Cutover Candidate Review、Candidate Promotion Readiness、Default Chat Runtime Boundary、Default Chat Adapter Activation Plan、Activation Review Decision、Activation Implementation Gate、Adapter Routing Status、Adapter Contract Harness、Adapter Dry Run、Adapter Dry Run Review、Adapter Implementation Readiness、Adapter Controlled Preview、Adapter Controlled Preview Review、Adapter Controlled Preview Approval Readiness、Adapter Cutover Implementation Plan、Adapter Cutover Plan Review、Adapter Cutover Plan Approval Readiness、Adapter Route Guard Scaffold、Adapter Cutover Invocation Harness、Adapter Invocation Plan、Adapter Invocation Boundary、Adapter Typed Callsite Contract、Adapter Ordinary Entry Preflight、Adapter Ordinary Entry Preflight Status 和 Adapter Narrow Implementation Discussion Gate 展示 gate/promotion evidence、人工评审草案、metadata-safe review decision summary、implementation eligibility、write-disabled shadow readiness、shadow review evidence、cutover planning readiness、非默认 candidate contract shape、candidate review evidence、candidate promotion readiness、default Chat boundary status、activation plan draft、activation review evidence、activation implementation gate、disabled routing scaffold、contract harness、dry-run invocation boundary、dry-run review evidence、implementation readiness、controlled preview、controlled preview review evidence、approval readiness、cutover implementation plan draft、cutover plan review evidence、cutover plan approval readiness、ordinary-entry preflight status 与 narrow implementation discussion readiness；Chat 有 W20 显式 Controlled Pilot 单轮入口、W21 reviewed promotion、W22 source-bound validation、W23 metadata-safe promotion evidence recorder、W24 readiness gate、W25 draft plan generator、W26 review decision evidence、W27 implementation gate、W28 shadow run、W29 shadow review evidence、W30 cutover planning readiness、W31 non-default cutover candidate adapter、W32 candidate review evidence、W33 candidate promotion readiness gate、W34 default Chat runtime boundary status、W35 activation plan draft、W36 activation review evidence、W37 activation implementation gate、W38 disabled routing scaffold、W39 contract harness、W40 dry-run boundary、W41 dry-run review evidence、W42 implementation readiness gate、W43 controlled preview、W44 controlled preview review evidence、W45 controlled preview approval readiness gate、W46 cutover implementation plan draft、W47 cutover plan review evidence、W48 cutover plan approval readiness gate、W49 route guard scaffold、W50 cutover invocation harness、W51 invocation plan、W52 invocation boundary、W53 typed callsite contract、W54 authority roadmap sync、W55 ordinary-entry preflight、W56 ordinary-entry preflight status surface 和 W57 narrow implementation discussion gate | 继续保持默认 Chat 不迁移；promotion 只是用户确认且 source/target session 一致后写入 assistant message，并记录 metadata-safe evidence 的受控台阶；readiness pass、migration draft、review approval、implementation eligibility、shadow readiness、shadow review approval、cutover planning eligible、candidateReady、candidate review approval、candidate promotion readiness、default Chat boundary status、activation plan draft、activation review approval、activation implementation gate eligible、adapter routing status、contract harness、dry run、dry-run review approval、implementationReady、previewReady、controlled preview review approval、controlled preview approval readiness、cutover implementation plan draft、cutover plan review approval、cutover plan approval readiness、route guard、cutover invocation harness、invocation plan、invocation boundary、typed callsite contract、ordinary-entry preflight、ordinary-entry preflight status 和 narrow implementation discussion gate 都只表示可进入人工讨论/开发讨论/实现讨论、显式对比/审阅、contract shape 验证、boundary 观察、activation planning、separate implementation discussion、disabled scaffold 观察、adapter contract 检查、write-disabled invocation 检查、controlled preview review、approval readiness、fail-closed 守卫、pre-entry side-effect locking、只读 preflight status、narrow implementation discussion 或后续迁移讨论，不是迁移许可 |
| Runs / Trace | 已能展示 MultiStrategy preview strategy / payload / governance / warnings | 成为所有 runtime strategy 的统一 metadata-safe trace viewer |
| **ModelRouter** | ✅ **任务/隐私感知路由已毕业，带真实健康检查语义** | 按任务类型、隐私需求、成本和延迟智能选择模型 |
| Memory | 已有 SQLite 与向量记忆；Memory Proposal 可写入/归档 | 升级为可治理、可归档、可追踪来源的长期记忆层 |
| MCP/A2A | 已有工具和外部 Agent 接入基础 | 成为 AgentAction 执行层，并默认受权限和审计保护 |
| Tools/Skills | 已有 ToolManifest、MCP/A2A、内置 Skill MVP | 成为 ReAct Agent 的执行能力层，覆盖 Core OS tools、Execution tools、Governance tools、Skill tools |
| Calibration/Evolution | 已有建议和校准雏形 | 统一进入 Proposal/Confirmation 机制 |
| Diagnostics/Safe Mode | 已有试用稳定化能力 | 成为系统控制台和恢复中枢 |
| **Chat Proposal** | ✅ **自动从对话中提取目标/状态/能力** | 自动感知用户意图并生成 LifeModel 更新提案 |
| **ContextAssembler** | ✅ **模块化上下文组装（V2 灰度中）** | 可插拔的记忆/隐私/工具上下文组装 |
| PlanExecute | Governed V1 runtime slice，可在 preview 中生成受治理计划 payload/report | 产品化周计划 vertical slice，必须先经过用户 review/edit |
| **Workspace** | ✅ **驾驶舱首页，实时状态概览** | 统一的 Agent 任务入口和监控中心 |
| **Feedback Loop** | ✅ **应用内反馈收集** | Chat 消息 👍/👎 反馈，诊断报告导出，Workspace 统计 |
| **Memory Governance** | ✅ **显式/隐式记忆提取** | "记住这个"生成 Proposal，自动记忆建议，异步 Embedding |
| **Skill Runtime** | ✅ **内置 Skill MVP** | weekly_review、goal_breakdown 等 Skill 可执行并生成 Proposal |
| **Network Policy** | ✅ **网络访问策略配置** | 域名白名单/黑名单，工具级覆盖，Privacy  tab 配置 |

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
│       ├── pages/                # Chat / Dashboard / Builder / Settings 等当前页面
│       ├── components/           # 通用组件
│       ├── tauri.ts              # Tauri command 封装层
│       └── App.tsx               # 路由与全局布局
├── openlife-core/                # Rust 核心业务库
│   └── src/
│       ├── life_model.rs         # LifeModel
│       ├── builder/              # LifeModel 构建与 Review
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
2. [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
3. [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
4. [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
5. [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)
6. [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
7. [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
8. [LifeModel-HS Legacy Write Path Audit](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_legacy_write_path_audit.md)
9. [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)
10. [OpenLife PRD v2: Personal Agent Framework](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md)
11. [OpenLife Development Plan](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
12. [Codex Execution Playbook](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)
13. [OpenLife Final PRD](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)，仅作为历史需求参考

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

## 当前推荐试用路径

### 主线体验（推荐）

1. **`Workspace`**（首页驾驶舱）查看系统状态、待处理 Proposal、今日 AgentRun 统计。
2. **`Agent`** 发起个性化对话，观察 Chat Proposal 自动提取目标和状态。
3. **`Review`** 审查 AI 生成的 LifeModel 更新提案，确认或拒绝。
4. **`Builder`** 完成一次快速构建，或恢复待确认 Review。
5. **`Runs`** 查看所有 Agent 执行记录，按状态/类型过滤，批量管理。
6. **`Settings`** 完成模型配置、诊断检查，开启实验性功能（ContextAssembler V2 / ModelRouter）。

### 实验性功能（灰度测试）

在 Settings → 实验性功能中可开启：

- **ContextAssembler V2**：使用模块化组装器构建对话上下文（灰度中，可回滚）
- **ModelRouter**：智能路由选择本地/云端模型（默认路由基础设施，云端 Provider 需配置并通过轻量健康检查）

```text
Workspace -> Agent Task -> Agent Run Trace -> Proposal Review -> LifeModel/Memory Update
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

### Phase 6: Workspace 重设计
- ✅ Workspace 驾驶舱（系统状态、待处理 Proposal、Run 统计）
- ✅ Runs 页面增强（过滤、搜索、分页、批量操作、回收站）
- ✅ 导航重构（Workspace 为默认首页）

### Phase 7: Stabilization / Spine Consolidation
- ✅ Builder 正常路径改为 Proposal-Only，legacy direct apply 仅保留给迁移/调试。
- ✅ Proposal 应用器覆盖 LifeModel/Goal、MemoryWrite、MemoryArchive、ToolPermission MVP。
- ✅ Chat Proposal 持久化与 AgentRun.generated_proposals 关联收敛到共享 helper。
- ✅ `make ci` 覆盖格式检查、Rust tests、frontend tests、frontend production build/typecheck。

### W1-W57: LifeModel-Governed Runtime Preview, Gate Evidence, Controlled Pilot, Promotion Evidence, Readiness, Draft Planning, Review Decision Evidence, Implementation Gate, Shadow Run, Shadow Review Evidence, Cutover Readiness, Cutover Candidate Adapter, Candidate Review Evidence, Candidate Promotion Readiness, Default Chat Runtime Boundary, Activation Plan Draft, Activation Review Evidence, Activation Implementation Gate, Disabled Routing Scaffold, Contract Harness, Dry-Run Invocation Boundary, Dry-Run Review Evidence, Implementation Readiness Gate, Controlled Preview, Controlled Preview Review Evidence, Controlled Preview Approval Readiness, Cutover Implementation Plan Draft, Cutover Plan Review Evidence, Cutover Plan Approval Readiness, Route Guard Scaffold, Cutover Invocation Harness, Invocation Plan, Invocation Boundary, Typed Callsite Contract, Authority Roadmap Sync, Ordinary Entry Preflight, Ordinary Entry Preflight Status, And Narrow Implementation Discussion Gate
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

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder Review 后模型没有变化：先确认 Proposal 是否仍在 Review Center 待处理；Builder 默认不会绕过确认直接写入。

## License

MIT License
