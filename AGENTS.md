# OpenLife - AI 助手上下文指南

> 本文档面向 AI Agent 和开发协作者，提供快速理解项目所需的一切上下文信息。

---

## 📋 项目概览

- **项目类型**：本地优先的个人 Agent 框架 / 个人 AI 操作系统（Tauri 桌面壳 + React 前端 + Rust 核心引擎）
- **技术栈**：Rust (Tauri 2.x + 自定义核心库) + React 18 + TypeScript + Tailwind CSS + SQLite
- **核心范式**：`LifeModel-HS Protocol Layer + Governed Agent Runtime + ReAct Default Strategy + Tool/Skill Execution + Memory/Feedback/Maturation Loop`
- **产品定义**：OpenLife 不是单纯聊天应用，也不是普通成长管理 App。它应当让用户用私人 LifeModel 驱动本地或云端模型完成对话、规划、写作、复盘、工具调用和状态更新，并在用户确认下持续更新对用户的理解。
- **当前阶段**：Main Chat Agent Execution v1 整改进行中，不能宣称 complete。普通 `send_message` / `start_stream_message` 已先进入 `AgentIngress` 并创建 governed task session / transcript / action queue；`DirectAnswer` 已从隐藏 legacy fallback 改为 governed strategy path，L2 DirectAnswer 现在会在 ordinary send/stream command surface 记录 scheduler/provider generation trace（scripted provider response proof）。ReActToolExecution 已从关键词核心执行改为先尝试带 governed plan guidance、metadata-safe tool-candidate contract 和 exact `toolset_allowlist` target enforcement / exact `tool_action_allowlist` enforcement 的 AgentLoop，再 fail-soft 到 single-step ActionExecutor-backed read action path（memory/session/file/web/MCP wrapper），并在成功 observation 后执行 governed follow-up synthesis；generic MCP read 现在可暴露 bounded registered read-only manifest candidate set，按 query/manifest capability/name/tag 做 deterministic ranking（不使用 raw manifest id/description），按 model-selectable target 去重后再应用 bounded limit，并在 contract/transcript 中记录 candidate rank/source/capability digest/bounded safe capability labels/sanitized match reason、model-selected allowed-tool metadata、model-selected ExecutionPolicy metadata 和 governed candidate arguments source/digest metadata（executor input 来自 governed candidate contract，而不是模型 `arguments`），且会排除 high-risk / critical / confirmation-required / write-like read-shaped manifests（包括 manifest id/name/action/capability/tag surfaces 中嵌入的 write-like terms）和 contract-unsafe 或 oversized model-facing manifest names/source labels，candidate contract label fields（含 `matchReason`）不安全时会被 bounded label redaction；当存在多候选 MCP read set、cloud/model route 允许且 provider credential 可用时，ReAct 会先用 metadata-safe candidate contract（含 capability digest 和 bounded safe capability labels）和 privacy-masked bounded context（不注入完整 LifeModel system prompt）走 provider/model-ranked preselection，只接受完整 bounded raw exact candidate id permutation 重排，provider ranking response 必须是 exact one-field JSON object（仅 `ranked_candidate_ids`），忽略未知/无效/partial id list 和模型参数，任一 contract-unsafe returned candidate id（包括带首尾 whitespace 后才匹配的 id）或额外 response field 会使排序 fail-soft 回 deterministic order，且 previewed ranking route 的 provider/model 必须匹配实际 request provider/model 才能调用并记录 provider-ranked evidence；若 source candidate set 本身包含 contract-unsafe candidate/action/target/source/match label 或 duplicate candidate id，则会在 provider request 前 fail-soft 回 deterministic order；只有被接受的完整排序会在 transcript / AgentLoop metadata 记录 ranked candidate ids，被忽略的 provider ranking 只保留 `toolSelectionModelRankingIgnored` 和 metadata-safe response digest，不持久化 provider 返回的无效 candidate order；explicit named MCP read target resolution 现在使用 permission-preserving governed-read target predicate，保留 safe read ToolPermission proposal flow，同时阻断 high-risk/critical/write-like/contract-unsafe read-shaped manifest 进入 AgentLoop candidate；若模型选择 allowlist 外 target、wrong action/target pair、write-like/unsupported action type 或其他非候选工具调用，AgentLoop 现在会返回显式 `model_selected_disallowed_tool` blocker，不再转入 single-step fallback；若 model-selected candidate 未通过 ExecutionPolicy，会返回显式 `model_selected_tool_policy_blocked` blocker；Main Chat context assembly 现在使用 controlled knowledge-format loader，把 workspace/configured `AGENTS.md`/`SOUL.md`/`USER.md`/`MEMORY.md`/selected `SKILL.md` 作为 bounded context surface，ordinary send/stream command surface、frontend Tauri wrapper 和 Chat composer 手动 `SKILL.md` context field 已可传入 sanitized optional selected skill id；runtime eval gate 已覆盖 memory/session multi-step AgentLoop、web network-policy blocker AgentLoop、fixture-backed successful web read AgentLoop、registered MCP AgentLoop success proof 和 registered MCP ToolPermission proposal proof；若 AgentLoop 没有观察到计划 action，会转入 ActionExecutor fallback，不再把无工具 final 当成已完成工具执行。provider/model-ranked preselection 已有 local HTTP OpenAI-compatible proof，但外部 live provider evidence 仍未执行。
  Tauri mock IPC 已覆盖 `send_message` 和 `start_stream_message` DirectAnswer run/task-session completion、L2 DirectAnswer scheduler/provider generation trace、send/stream governed file-read command surface、send/stream PlanExecute draft command surface、proposal-path command surface、send/stream registered-MCP AgentLoop success command surface、send/stream registered-MCP ToolPermission proposal command surface、send/stream web AgentLoop blocker command surface、send/stream fixture-backed web AgentLoop success command surface，并覆盖 `send_message` / `start_stream_message` web-policy / missing-MCP blocker command surface；新增 24-case send/stream command-surface eval gate，跨 DirectAnswer、scripted provider generation、file read、PlanExecute draft、proposal、web blocker、web AgentLoop blocker、fixture-backed web AgentLoop success、missing MCP blocker、registered MCP AgentLoop success 和 registered MCP ToolPermission proposal（fallback + AgentLoop）执行真实 mock IPC，并要求 legacy fallback=0、silent write=0。Tauri live-provider harness 现在另有非 ignored 的 `local_test_http` OpenAI-compatible provider-client proof，会通过普通 `send_message` 走真实 scheduler/HTTP client 并验证 response trace/no silent writes，但不会计入 external live-provider completion；final acceptance runner 现在可在 `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1` 时用隔离 eval AppState 执行 DirectAnswer / web AgentLoop / registered MCP AgentLoop / MCP ToolPermission proposal 四个 ordinary `send_message` live harness 场景，缺 key/network/provider 时返回四个 blocked scenario reports 且不调用 provider。仍未达最终验收：最终 eval gate 仍需覆盖真实 live 模型/provider、web/MCP manifest/provider cases；更广 provider/live proposal-permission 验收仍需补齐；legacy fallback 仍需继续收窄并保持显式可见。
- **Main Chat eval 补充**：runtime eval report 现在新增 `providerRouteCoverage`、`localOnlyProviderGuardCoverage`、`evalProviderGenerationCoverage`、`evalSchedulerGenerationCoverage`、`webSuccessfulReadCoverage`、`webAgentLoopCoverage`、`mcpAgentLoopCoverage` 和 `mcpToolPermissionProposalCoverage`，用现有 ModelRouter + HS LocalOnly packet 证明 provider route 决策、local-only cloud fallback 移除和 metadata-safe no-model-invocation trace，用显式 eval provider 经 `InferenceScheduler::generate` 证明 DirectAnswer prompt/context final-generation transcript，并用 scripted AgentLoop + ActionExecutor 证明 web network-policy blocker observation、context-scoped fixture-backed successful web read observation、registered read-only MCP success observation 和 registered MCP ToolPermission proposal observation；ordinary `send_message` / `start_stream_message` command-surface 现在也有 L2 DirectAnswer scheduler/provider generation trace proof、governed file-read proof、PlanExecute draft proof、registered-MCP command-surface AgentLoop success proof、registered-MCP ToolPermission proposal proof、web AgentLoop blocker proof 和 fixture-backed web AgentLoop success proof；scripted AgentLoop eval hook 现在不再是 core-test-only，因此 Tauri final runner 通过 openlife-core 依赖调用 100-case runtime gate 时也会执行同一套 memory/session/web/MCP AgentLoop proof；core 100-case runtime eval report 与 `main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix` 24-case command-surface report 都显式输出 `liveProviderGenerationCoverage=0`、`liveProviderWebMcpAgentLoopCoverage=0`、`liveProviderWebAgentLoopCoverage=0`、`liveProviderMcpAgentLoopCoverage=0`、`liveProviderProposalPermissionCoverage=0`、`finalCompletionReady=false` 和 named live-provider blockers（含 split `provider_backed_web_agent_loop_not_executed` / `provider_backed_mcp_agent_loop_not_executed`）；core 还新增 `evaluate_main_chat_agent_execution_v1_acceptance_gate`，会重新聚合 core runtime、send/stream command-surface 和 live-provider evidence，并复核关键 coverage，不能只靠伪造 `finalCompletionReady=true` 通过；`run_main_chat_agent_execution_v1_eval_gate` 现在作为非默认 Tauri command 暴露 core 100-case runtime eval gate，metadata-safe、no external provider、no app-store writes、migrationPermission=false，并以 typed `liveProviderPreflight` 字段和 summary 附带当前 config 的 metadata-safe live-provider preflight blockers（不序列化 key、不调用 provider），明确因缺 command-surface/live evidence blocked；live-provider evidence 现在拆分为 Direct generation、web AgentLoop、MCP AgentLoop 和 proposal-permission 四个结构化场景，并要求 harness report 的 scenario identity 匹配对应 evidence、`status=completed`、无 blockers，且必须带 metadata-safe run_id、task_session_id 和 bounded non-empty response_preview trace，不能用非 web/MCP 场景、失败状态或缺 trace 的布尔位冒充；Tauri 聚焦测试覆盖 live harness report 聚合、24-case send/stream command-surface gate 转入 core final acceptance gate，以及单一 final acceptance runner 在无 live opt-in 时运行 core 100-case + 24-case command-surface 并 fail closed；complete clean live harness evidence 现在会显式合并进 runtime live coverage 和 command-surface final evidence，runner 返回结构化报告，包含 runtime/command-surface case counts、live-provider attempted/report/ready/main-chat-invoked/model-invoked counts、metadata-safe live-provider blockers（含 no-live-report missing evidence blockers、post-invocation 场景化派生 blockers，以及 ready/completed report 缺 live-provider invocation allowance、Main Chat invocation 或 model invocation proof 时的显式 blockers）、direct-write flag 和嵌套 core acceptance report。fixture-backed web success 不是 live/provider-backed web completion，完整 live-provider-backed generation、provider-backed web/MCP manifest coverage 和更广 provider/live proposal-permission 验收仍未完成。
- **Live-provider ready-count 补充**：final acceptance runner 的 `liveProviderReadyCount` 只统计通过对应场景 credit 规则的可审计 live report，不统计裸 `ready=true` claim；缺 trace、缺治理字段、fallback、silent write、local/synthetic provider 或其他未获 credit 的 report 仍只进入 blocker / not-executed 统计。
- **Live-provider response-preview 补充**：credited live harness report 的 raw `response_preview` 必须是 bounded non-empty harness-normalized single-line trace，且不得包含首尾 whitespace、重复 whitespace run 或 control characters；final gate 会拒绝包含 control characters（包括首尾包裹换行/制表符）、首尾空格或未按 harness whitespace normalization 生成的 preview，harness 只会把 provider reply 规范化成单行 preview trace，不改变真实 reply / transcript / stored output。
- **Live eval preflight / harness**：`evaluate_main_chat_live_provider_eval_preflight` / `evaluate_main_chat_live_provider_eval_preflight_from_config` 现在提供 metadata-safe、fail-closed 的 live-provider 验收前置检查；缺少显式 live eval opt-in、provider API key、network enabled、非 scripted scheduler、或被 LocalOnly policy 约束时都会返回 blocker，并保持 `modelInvoked=false`、`directWritesExecuted=false`。config-backed adapter 只使用 key-presence boolean，不序列化 key；Tauri command-state test 已覆盖无 opt-in / 无 key / network disabled / scripted scheduler 时 fail closed。Tauri test harness 还新增非 ignored 的 local HTTP OpenAI-compatible provider-client proof：`local_test_http` endpoint 允许 harness 执行普通 `send_message` 主路径，证明 DirectAnswer 可以经 `InferenceScheduler` 和 HTTP provider client 得到真实 provider response trace，同时 `main_chat_live_provider_acceptance_evidence` 仍不把它计为 external live generation evidence，command-surface live coverage 仍为 0；若 local/synthetic report 或 local/localhost/mock/fixture-like/loopback-or-private-network-alias（含分隔符、字母数字嵌入式 alias，或嵌入式 local/mock/fixture/synthetic/scripted/ollama 标签）/contract-unsafe provider identity 进入 live evidence audit，会派生 `live_provider_external_provider_missing` blocker。final acceptance runner 的 opt-in live suite 通过 `run_main_chat_live_provider_eval_harness_suite_from_state` 在隔离 AppState 中复用同一普通 Main Chat path；无 opt-in 时不运行 live scenario，有 opt-in 但缺 credential 时生成四个 blocked reports、`mainChatInvoked=0`、`modelInvoked=0`。ignored opt-in live-provider proof paths 只有外部 provider endpoint、真实 key、network enabled、无 scripted scheduler 且显式 opt-in 时才会通过 ordinary `send_message` IPC 调用主路径；scenario 覆盖 DirectAnswer、provider-backed ReAct web AgentLoop、bounded multi-candidate registered MCP AgentLoop 和 MCP ToolPermission proposal，并检查 `liveProviderInvoked`、raw metadata-safe run/task trace ids（不接受首尾 whitespace/control characters）、raw metadata-safe provider identity（不接受首尾 whitespace/control characters）、raw metadata-safe provider model identity（不接受首尾 whitespace/control characters）、required-evidence manifest、AgentLoop action status、no single-step fallback、MCP target resolution / ToolPermission proposal evidence、model-selected candidate rank/raw source/capability digest/bounded safe capability labels/raw match reason（source/match reason 不接受首尾 whitespace/control characters，capability labels 不得为 write-like）、selected candidate id/target/action type、bounded candidate ids、target allowlist、exact action-target allowlist / ExecutionPolicy / governed-arguments trace 和 no silent writes；带 single-step fallback 的 ReAct live report 会派生场景化 not_completed blocker，不能获得 web/MCP/proposal live credit；缺少这些 ReAct governance trace 字段的 live report 不会获得 web/MCP/proposal live credit；web AgentLoop live credit 还必须证明 selected candidate id/target identity 和 action evidence 是 governed `web.*` 工具，且不能带 overlapping registered MCP read-success 或 ToolPermission proposal trace；registered MCP live credit 还必须证明至少两个 complete duplicate-free bounded model-selectable MCP candidates / targets / action-target pairs，并保留 provider-ranked selection metadata（`toolSelectionModelRanked=true`、`toolSelectionRankingSource=provider_model`、`toolSelectionRankingRouteType=cloud`、provider-backed ranking route、raw metadata-safe ranking provider identity matches the raw metadata-safe live report provider（不接受首尾 whitespace/control characters）、raw metadata-safe ranking model identity matches the raw metadata-safe live report model（不接受首尾 whitespace/control characters）、complete duplicate-free bounded candidate-id permutation、candidate ids match exact target/action-target allowlists、selected candidate id matches selected MCP target、selected rank matches provider-ranked order、selected action type `mcp_tool`、未 ignored、metadata-safe response digest with `bytes:<n> hash:sha256:<64-hex>` shape、且无 overlapping ToolPermission proposal outcome）；MCP ToolPermission proposal live credit 还必须证明 selected candidate id/target identity 与 pending ToolPermission proposal target 匹配、actionType 为 `mcp_tool`，且不能同时声明 registered MCP read-success / `mcp_read_target_resolved` outcome。当前环境未执行这些 ignored live runs。它只是让 live-provider gate 可执行/可审计，不等同于已经完成 live-provider-backed generation/web/MCP/proposal-permission 证明。
- **DirectAnswer live-credit 补充**：DirectAnswer live credit 还必须证明 direct provider generation 和 raw metadata-safe provider model identity（不接受首尾 whitespace/control characters），且没有 AgentLoop、single-step fallback、MCP/proposal 或 tool-selection metadata；带 fallback/tool metadata 的 completed report 会派生 `live_provider_generation_not_completed`，不能获得 external live generation credit。
- **Live ReAct digest 补充**：web/MCP/proposal ReAct live credit 中的 governed candidate arguments digest、selected candidate capability digest 和 provider-ranking response digest 都必须保留 raw exact metadata-safe `bytes:<positive-n> hash:sha256:<64-hex>` label，且 byte count 必须是 canonical positive decimal（不得有 leading zeros），不得包含 control characters 或首尾/内部额外 whitespace；harness ready predicate 和 final gate 都会拒绝 `bytes:` 前缀的弱 digest、`bytes:0` placeholder、leading-zero byte count、free-form `hash:` suffix、wrapping whitespace 或 wrapping control characters，这些不可获得 live credit。
- **Live ReAct ranked-manifest label 补充**：web/MCP/proposal ReAct live credit 中的 selected candidate source、bounded safe capability labels 和 match reason 也必须是 raw bounded metadata-safe label，且不得包含首尾 whitespace/control characters（包括包裹换行/制表符）；capability labels 还必须为非 `none`、slash-separated metadata-safe labels、包含离散 `read` label，并拒绝 write-like terms；自由文本 source/match reason、缺少 read label 或 write-like capability labels 不可获得 live credit。
- **Live ReAct selected-rank 补充**：web/MCP/proposal ReAct live credit 中的 selected candidate rank 必须匹配 selected candidate id 在 bounded candidate id list 中的 1-based 位置；rank 与 candidate order 不一致不可获得 live credit。
- **Live ReAct allowlist 补充**：web/MCP/proposal ReAct live credit 还必须证明 bounded candidate ids、target allowlist 和 action-target allowlist 具有同一 distinct bounded target set、同一 candidate cardinality，并且所有 allowed action 都使用 model-selected governed action type；每个 action-target allowlist entry 必须是 exact two-field `{actionType,target}` metadata-safe object，带额外 JSON fields 或只在 trim 后才 metadata-safe 的 action entry 不可获得 live credit，harness ready predicate 不得把 malformed raw action metadata 规范化成可审计 pair；registered MCP provider-ranked live credit 还必须证明 AgentLoop candidate id order 与 provider-ranked candidate id order 精确一致，而不是仅同集合匹配；web AgentLoop live credit 的 selected governed web action type 必须是 `mcp_tool`；额外、缺失、重复、contract-unsafe label、wrong action-type 或 provider-ranked order mismatch 的 action-target / allowlist target 不可获得 live credit。
- **Provider-ranked JSON shape / route identity 补充**：provider-ranked candidate preselection response 必须是 raw JSON object，且只能包含 `ranked_candidate_ids`；Markdown fenced JSON、额外解释文本或额外 response fields 都必须 fail-soft ignored，不能被规范化后获得 provider-ranked credit。previewed ranking route provider/model 必须 raw exact 匹配实际 request provider/model；只在 trim 后才匹配的 wrapping whitespace/control-character identity 必须 fail-soft ignored。
- **Live-provider evidence 清单**：`liveProviderPreflight.requiredEvidence` 和 core acceptance `requiredEvidence` 同时列出 combined `provider_backed_web_mcp_agent_loop` 与 split `provider_backed_web_agent_loop` / `provider_backed_mcp_agent_loop`，避免只用 combined web/MCP evidence 代替独立场景证明；credited live harness report 必须保留 exact metadata-safe required-evidence manifest，缺项、重复项、额外自由文本项或 contract-unsafe label 都不能获得 live credit。
- **Stabilization 当前进展**：final-gate aggregation/evidence normalization/blocker derivation 以及 live-provider required-evidence、blocked/completed report construction 已抽到 `src-tauri/src/main_chat_final_gate.rs`，final acceptance runner 会调用该模块；command-surface eval case matrix、scenario state setup、prompt/session-id mapping、case assertion/no-silent-write interpretation、report/coverage/evidence normalization 已抽到 `src-tauri/src/main_chat_command_surface_eval.rs`，isolated eval AppState factory 已抽到 `src-tauri/src/main_chat_eval_state.rs`，live-provider harness opt-in/suite/execution 已抽到 `src-tauri/src/main_chat_live_provider_harness.rs`，Main Chat task-control command state/resume/cancel/retry/replay helper 已抽到 `src-tauri/src/main_chat_task_controls.rs`，Main Chat generation support helper（chat persistence / vector persistence / AgentRun finalization / non-stream fallback / provider endpoint classification / preview text）已抽到 `src-tauri/src/main_chat_generation_support.rs`，Main Chat preprocessing / memory-hit merge helper 已抽到 `src-tauri/src/main_chat_preprocess.rs`，Main Chat auto-checkin / reasoning-trace prompt / conversation-signal helper 已抽到 `src-tauri/src/main_chat_conversation_updates.rs`，Main Chat legacy fallback route-plan / non-stream generation fallback helper 已抽到 `src-tauri/src/main_chat_legacy_fallback.rs`，Main Chat deprecated/non-default AgentLoop send/stream helper 已抽到 `src-tauri/src/main_chat_legacy_agent_loop.rs`，ReAct tool-selection plan/candidate helper 已抽到 `src-tauri/src/main_chat_react_tool_selection.rs`，ReAct AgentLoop attempt execution / runtime helper types / follow-up synthesis / action-to-tool-call conversion / tool-call/blocker metadata helpers 已抽到 `src-tauri/src/main_chat_react_runtime.rs`，ReAct ActionExecutor-backed fallback execution helper 已抽到 `src-tauri/src/main_chat_react_execution.rs`，Main Chat proposal / ToolPermission proposal support helper 已抽到 `src-tauri/src/main_chat_proposal_support.rs`，HS runtime packet/topic/tool-requirement helper 已抽到 `src-tauri/src/main_chat_hs_runtime.rs`，Main Chat task session / transcript / action-queue runtime support helper 已抽到 `src-tauri/src/main_chat_runtime_support.rs`，Main Chat send command state executor 已抽到 `src-tauri/src/main_chat_send.rs`，Main Chat strategy dispatcher 已抽到 `src-tauri/src/main_chat_strategy.rs`，Main Chat stream command state executor 已抽到 `src-tauri/src/main_chat_streaming.rs`，Main Chat context compiler / selected-skill sanitizer 已并入 `src-tauri/src/main_chat_context_loader.rs`，ReAct static/unit tests 已拆到 `src-tauri/src/main_chat_react_unit_tests.rs`，boundary command-surface tests 在 `src-tauri/src/main_chat_react_boundary_tests.rs`，final-acceptance helper/runner/evidence tests 已拆到 `src-tauri/src/main_chat_final_acceptance_tests.rs`，HS runtime behavior tests 已拆到 `src-tauri/src/main_chat_hs_runtime_tests.rs`，task-control behavior tests 已拆到 `src-tauri/src/main_chat_task_control_tests.rs`，context-loader / workspace-file resolver behavior tests 已拆到 `src-tauri/src/main_chat_context_loader_tests.rs`，command-surface/live harness 测试现在直接使用这些非 test-only helper；新增非默认 `run_main_chat_agent_execution_v1_final_acceptance_gate` Tauri command，使用同一 aggregation、运行 core runtime eval、附带当前 state/scheduler 的 metadata-safe live-provider preflight，并在隔离 eval AppState 中执行全部 24 个本地 send/stream command-surface cases（send 通过 `main_chat_send::send_message_with_state`，stream 通过 `main_chat_streaming::start_stream_message_with_state`），默认不调用 external provider、不写 app store，并在缺完整 live evidence 时 fail closed、`migrationPermission=false`；同一 runner 现在在显式 live opt-in 时会执行四场景 live harness suite，当前无 credential 环境只证明 no-invocation blocker/report path；安全 workspace file read 已从少量硬编码文件名改为 workspace-root scoped resolver，支持显式相对路径、canonicalize readable target，并阻断 traversal / outside-workspace read，command-surface FileReadSuccess eval 会把隔离状态的 safe_paths 明确限定到 canonical workspace root；ReAct AgentLoop 现在有 metadata-safe tool-candidate contract、generic MCP read bounded read-only manifest candidate set、deterministic capability/name/tag ranking（不使用 raw manifest id/description）、provider/model-ranked candidate preselection（只接受完整 bounded candidate id permutation 重排，未知/无效/partial/contract-unsafe id list fail-soft 回 deterministic order，ignored provider order 不写入 ranked-candidate metadata，仅保留 ignored flag 和 metadata-safe response digest）、candidate rank/source/capability digest/bounded safe capability labels/sanitized match reason evidence、model-selected ExecutionPolicy metadata、governed candidate arguments source/digest metadata、high-risk/confirmation/write-like embedded-surface/contract-unsafe/oversized candidate name/source exclusion、candidate contract unsafe label redaction、exact `toolset_allowlist` target enforcement / exact action-target allowlist enforcement，以及 allowlist 外 target / wrong action-target pair / write-like 或 unsupported action type / unknown model tool call 的显式 `model_selected_disallowed_tool` blocker proof、model-supplied arguments 被 governed candidate contract executor input 覆盖的 boundary proof 和 policy-denied selected candidate 的 `model_selected_tool_policy_blocked` blocker path（不触发 single-step fallback、不执行写入）；provider/model-ranked path 已通过 ordinary `send_message` + local HTTP OpenAI-compatible provider proof，但不计 external live credit；Main Chat context assembly 现在通过 controlled loader 读取 bounded workspace/configured `AGENTS.md`、`SOUL.md`、root / `memories/` `USER.md` / `MEMORY.md` 和 selected `SKILL.md`（仅在 selected skill id 存在且通过校验时），ordinary send/stream command surface、frontend wrapper 和 Chat composer 手动 `SKILL.md` context field 已支持 optional `selectedSkillId`。24-case send/stream command-surface gate 仍通过。Main Chat Agent Execution v1 仍未 complete，真实 final/live-provider evidence、剩余 live harness 外部证明和进一步 Main Chat runtime/strategy 模块清理仍是阻断项。
- **Stabilization 测试拆分补充**：live-provider command-surface harness tests 已拆到 `src-tauri/src/main_chat_live_provider_tests.rs`，覆盖 no-invocation preflight blocker、local HTTP provider proof 和 ignored external-provider opt-in proof paths；这不计入 external live-provider completion。
- **Command-surface 测试拆分补充**：proposal-path、DirectAnswer send/stream、web-policy blocker、missing-MCP blocker、registered-MCP read-success、registered-MCP / web-policy AgentLoop no-fallback、registered-MCP multi-candidate AgentLoop IPC tests 和 24-case command-surface eval gate coverage test 已拆到 `src-tauri/src/main_chat_command_surface_tests.rs`，继续证明 proposal-first、no-silent-write、DirectAnswer run completion、scheduler/provider trace、governed blocker、governed read-success、AgentLoop no-fallback、multi-candidate allowed-manifest selection 和 send/stream eval matrix 的普通 command surface 行为，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **HS runtime 测试拆分补充**：HS runtime helper extraction guard、sanitized HS packet construction、tools-prompt read-only/write-requirement separation、LocalOnly no-cloud fallback、sensitive-topic LocalOnly policy selection 和 no `src-tauri/src/lib.rs` root re-export guard 已拆到 `src-tauri/src/main_chat_hs_runtime_tests.rs`，继续覆盖 Main Chat HS runtime boundary，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Task-control 测试拆分补充**：retry manual blocker / automatic replay、permission-preserving resume / accepted ToolPermission replay 和 cancel queued-action stop tests 已拆到 `src-tauri/src/main_chat_task_control_tests.rs`，继续覆盖 Main Chat task-control command behavior，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Context-loader 测试拆分补充**：bounded knowledge-format surfaces、selected `SKILL.md` loading/sanitization、selectedSkillId send/stream plumbing 和 workspace file resolver explicit-path/traversal tests 已拆到 `src-tauri/src/main_chat_context_loader_tests.rs`，继续覆盖 bounded context 和 safe workspace read 边界，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Runtime-module guard 测试拆分补充**：Main Chat runtime/generation/proposal/final-gate/command-surface/live-provider module extraction guards、focused module helper import direction guard、send/stream state-executor guards、ordinary send/stream deprecated-helper isolation guard 和 Chat page migration-command isolation guard 已拆到 `src-tauri/src/main_chat_runtime_module_tests.rs`，继续覆盖 reusable helper 边界并防止 focused modules 通过 `src-tauri/src/lib.rs` root re-export 取用 Main Chat runtime helpers，同时减少 `src-tauri/src/lib.rs` 测试堆积。
- **Main Chat Agent v1 硬约束**：Main Chat Agent v1 不静默写 durable LifeModel-HS truth、long-term Memory、file/calendar/email/external/provider/plugin state 或危险 shell；显式记忆和 LifeModel 更新只能创建 Review Center proposal；外部/敏感写入只能生成 blocker/confirmation request；web/MCP 在缺少 governed route/manifest 时返回清晰 blocker；workspace `AGENTS.md`/materialized `USER.md`/`MEMORY.md`/`SOUL.md`/`SKILL.md` 只作为 bounded context，不能覆盖 privacy/model/tool policy，也不能自动晋升为用户真相。W1-W158 readiness/status/golden-path/Skill Runtime 结果仍不是迁移许可，旧 default-chat adapter guard 文档仅作历史回归参考。
- **历史阶段说明（W1-W158，Main Chat v1 完成前）**：W124-W149 LifeModel-Governed Backend Completion Goals 1-8 已完成。W124-W127 完成 pure backend readiness/contract report、typed LifeEvent schema/store skeleton、typed Signal schema/deterministic low-risk extractor，以及 safe LifeEvent/Signal/Evidence bridge；W128-W130 完成 pure backend Evidence Graph v1，包括 support/opposition links、dedupe clusters、source weights、cluster summaries、conflict/decay/cooldown state，以及 metadata-safe Evidence Timeline read model；W131-W133 完成 Maturation Engine v1，包括基于 Evidence Graph clusters 的 planning preference、energy pattern、work style、communication preference 低风险候选生成，proposal outcome 正向/纠正/负向 evidence 收敛，以及 deterministic suppression/correction；W134-W136 完成 Accepted Guidance And Materialization，包括 accepted/trial guidance lifecycle、governed materialized LifeModel view provenance，以及 metadata-safe version diff / rollback read model；W137-W140 完成 Runtime Guidance Integration，包括 RuntimeHSPacket v2 metadata-safe accepted/trial guidance refs、仅由 `RuntimeGuidanceConsumptionMode::ExplicitRuntime` 显式开启的非默认 ReAct guidance prompt/config/action-boundary consumption、显式 Plan-Execute weekly planning guidance consumption，以及 Guidance Impact read model/trace linkage；W141-W143 完成 Policy / Privacy / Tool Governance Hardening，包括 ModelRouter/Privacy HS LocalOnly hard enforcement、ActionExecutor HS tool governance、以及 Governor unified metadata-safe decision report；W144-W146 完成 Backend Golden Paths，包括 W144 Weekly Planning golden path、W145 Low-Energy Support golden path、W146 Preference Correction golden path；W147-W149 完成 Pre-UI Backend Contract Freeze，包括 Learning Inbox / Evidence Timeline / Proposal Review / Runtime Trace / Guidance Impact / Privacy Controls / LifeModel Overview metadata-safe read-model contracts、final backend completion gate report、以及 authority docs/progress/verification sync；W150-W158 完成 Skill Runtime Beta Maturity，包括 built-in skill readiness、bounded context assembly、HS privacy/model-route governance、fail-soft output envelope/trace、proposal candidate governance、plugin declarative-only boundary、非默认只读 Skill Runtime status command、Runs/Review trace integration 和 docs/progress sync。Goals 7-8 都是 pure backend/core/read-model proof，不迁移 default Chat，不替换普通 `send_message` / `start_stream_message`，不新增 Tauri command，不新增 UI，不直接写 durable LifeModel-HS truth，不静默写 Memory/file/calendar/email/external/provider/plugin state；普通 Chat 不得调用 W144-W146 golden path helpers、W147-W149 contract/gate helpers 或 W150-W158 Skill Runtime readiness/status/run helpers，也不得把 golden path ready、contract frozen、final gate ready 或 Skill Runtime ready/status 当作 migration permission。W114-W123 ReAct Beta Execution Hardening、W98-W105 非默认 weekly planning 产品纵切、W90-W97 Legacy Direct-Write Convergence 仍保持完成。
- **文档入口**：`plans/README.md` 是文档权威地图，`plans/main_chat_agent_v1_stabilization_goal_spec.md` 是下一次 Goal-mode 的当前入口，用于 Main Chat Agent Execution v1 stabilization / acceptance-blocker remediation；`plans/main_chat_agent_migration_v1_goal_spec.md` 是 Main Chat Agent Execution v1 整改 spec / audit trail 与能力目标（尚未完成），不应直接重启为下一轮大 Goal；`plans/skill_runtime_goal_spec.md` 是已完成的 W150-W158 Skill Runtime Goal-mode spec / audit trail，`plans/lifemodel_governed_backend_completion_goal_spec.md` 是已完成的 Backend Completion master spec，`plans/lifemodel_governed_runtime_progress.md` 是 W1-W158 状态索引。若旧长段仍写 Main Chat migration/execution 已 complete、fake deterministic eval gate 可作为最终验收、ReAct/web/MCP 已真实接入、或 legacy 已完全不是 fallback 风险，以本入口、`plans/README.md`、stabilization spec、Main Chat Agent Execution v1 spec、Skill Runtime spec、backend completion spec 和 progress index 为准。
- **仓库链接**：（需要人工补充）

### 当前架构文档优先级

后续 Agent 进入项目时，优先阅读：

1. [`plans/README.md`](plans/README.md)：文档权威地图。仓库和 GitHub 中旧计划很多，若文档互相冲突，以这里的优先级为准。
2. [`plans/main_chat_agent_v1_stabilization_goal_spec.md`](plans/main_chat_agent_v1_stabilization_goal_spec.md)：下一次 Goal-mode 的当前入口。只做 stabilization / acceptance-blocker remediation，不继续扩展旧大 Goal；目标是修复真实 final gate、external live-provider evidence、test-only harness 泄漏、ReAct tool-selection boundary、安全 workspace file read、知识格式 bounded context surfaces 和 `src-tauri/src/lib.rs` 巨型堆积。
3. [`plans/main_chat_agent_migration_v1_goal_spec.md`](plans/main_chat_agent_migration_v1_goal_spec.md)：Main Chat Agent Execution v1 整改 spec / audit trail 与能力目标。定义 Main Chat 从 partial migration/scaffold 升级为真实可验收主线 Agent 执行能力的目标、硬约束、eval gate、内部 stage order、Prompt/Memory/Knowledge format 要求和实现验收；当前不可宣称 complete。
4. [`plans/openlife_lifemodel_governed_agent_runtime.md`](plans/openlife_lifemodel_governed_agent_runtime.md)：下一阶段总纲。定义 LifeModel-HS 作为协议层、ReAct 作为默认策略、Maturation Loop 与未来 Multi-Strategy Runtime 的开发顺序，优先级最高。
5. [`plans/lifemodel_governed_backend_completion_goal_spec.md`](plans/lifemodel_governed_backend_completion_goal_spec.md)：已完成 Backend Completion master spec。定义进入大规模 UI/UX 前必须完成的 LifeModel-governed backend kernel、W124-W149 切片、验收 gates、golden paths、read model contract 和 CLI Agent 提示词；不是 default Chat migration。
6. [`plans/lifemodel_governed_runtime_progress.md`](plans/lifemodel_governed_runtime_progress.md)：W1-W158 结构化状态索引，按 stage id / 名称 / 状态 / command-surface 类型 / read-only-write-disabled-metadata-safe / default Chat 影响 / 下一步依赖整理；不是第二套路线图。
7. [`plans/skill_runtime_goal_spec.md`](plans/skill_runtime_goal_spec.md)：已完成 W150-W158 Skill Runtime Beta Maturity 的 CLI Goal-mode spec / audit trail。定义 Skill Runtime readiness、bounded context assembly、HS privacy/model-route governance、output envelope/trace、proposal candidate governance、plugin skill boundary、non-default read-only status、Runs/Review trace integration 和 docs sync；不是 default Chat migration。
8. [`plans/react_beta_execution_hardening_goal_spec.md`](plans/react_beta_execution_hardening_goal_spec.md)：已完成 CLI Goal-mode spec / audit trail。定义 W114-W123 ReAct Beta Execution Hardening：ReAct readiness contract、AgentLoop action schema/parser、Tool Registry taxonomy/readiness、ActionExecutor manifest authority、AgentRun action/observation trace envelope、permission/replay hardening、proposal-first writes、非默认 status harness、Runs/Trace UI hardening 和 docs sync；不是 default Chat migration，也不是完整 Beta 宣告。
9. [`plans/runtime_strategy_maturity_goal_spec.md`](plans/runtime_strategy_maturity_goal_spec.md)：已完成 CLI Goal-mode spec / audit trail。定义 W106-W113 RuntimeStrategy / Multi-Strategy Runtime Maturity：strategy capability descriptors、registry readiness、candidate selection matrix、execution report envelope、非默认 status command、preview/product trace vocabulary convergence、future strategy declarative boundary 和 default Chat isolation hardening；不是 default Chat migration，也不是 ReAct Beta execution hardening。
10. [`plans/plan_execute_product_vertical_goal_spec.md`](plans/plan_execute_product_vertical_goal_spec.md)：已完成 CLI Goal-mode spec / audit trail。定义 W98-W105 Plan-Execute Product Vertical：非默认 weekly planning 产品纵切、durable plan session、review/edit/finalize lifecycle、proposal-first step execution、AgentRun/trace linkage 和前端产品 surface；不是 default Chat migration。
11. [`plans/legacy_direct_write_convergence_goal_spec.md`](plans/legacy_direct_write_convergence_goal_spec.md)：Legacy Direct-Write Convergence 的已完成 CLI Goal-mode spec / audit trail。定义 W90-W97 一次性 Goal 开发目标、顺序、硬约束、验收矩阵和最终 convergence 条件。
12. [`plans/lifemodel_maturation_goal_plan.md`](plans/lifemodel_maturation_goal_plan.md)：已完成 W73-W78 LifeModel Maturation proof-slice 准备/spec 与 audit trail。定义窄域低能量/低压力规划偏好、非默认成熟化桥接、proposal-first 和 metadata-safe 验收边界。
13. [`plans/openlife_agent_framework_architecture.md`](plans/openlife_agent_framework_architecture.md)：Agent Framework 架构基准。现在应与总纲合读：ReAct 是当前默认 runtime strategy，不是唯一未来架构。
14. [`plans/openlife_react_beta_roadmap.md`](plans/openlife_react_beta_roadmap.md)：Alpha+ 到 Beta 的 ReAct 执行能力路线图，定义 Beta Gate 和工具执行严肃性。
15. [`plans/lifemodel_hs_mvp_task_specs.md`](plans/lifemodel_hs_mvp_task_specs.md)：Post-Beta LifeModel-HS MVP 的 coding-ready task specs。
16. [`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`](plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)：LifeModel-HS 的 source-of-truth、governance、privacy 和 materialized-view 硬约束。
17. [`plans/lifemodel_hs_legacy_write_path_audit.md`](plans/lifemodel_hs_legacy_write_path_audit.md)：Legacy direct-write 收口地图，后续治理化开发必须参考。
18. [`plans/lifemodel_hs_architecture_plan.md`](plans/lifemodel_hs_architecture_plan.md)：LifeModel-HS 设计基线，已由 ADR 0013、MVP specs 和总纲接管实现入口。
19. [`OpenLife_PRD_v2_Agent_Framework.md`](OpenLife_PRD_v2_Agent_Framework.md)：产品定义与需求基准；实现顺序不得覆盖 LifeModel-Governed 总纲。
20. [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md)：当前开发路线，已按 Agent Framework 重写。
21. [`README.md`](README.md)：面向用户与新开发者的当前状态说明。
22. [`OpenLife_Final_PRD.md`](OpenLife_Final_PRD.md)：旧版 PRD，仅作为历史参考，不再作为当前架构唯一依据。

### 后续开发总原则

- 不推倒重写，继续复用现有模块。
- 不继续平铺新页面，优先建立 Agent Runtime 主线。
- ReAct 是当前默认执行策略：后续核心能力必须先收敛到 `Reason -> Act(tool/skill) -> Observe -> Follow-up -> Proposal/Permission -> Apply/Replay -> Audit`，但架构上要为 Plan-Execute、Workflow、Proactive 等 RuntimeStrategy 留出位置。
- 当前分支已完成 W1-W158，但 Main Chat Agent Execution v1 仍在整改，尚未最终达标。W61-W64 是 docs/index整理和权威入口压缩验收，W65-W72 是 backend-only default Chat adapter guard stack，W73-W78 是 LifeModel maturation proof slice，W79-W89 是 legacy direct-write convergence 的 inventory、guard、caller restriction、proposal PatchSource mapping/readiness 历史切片；W90-W97 完成 legacy direct-write convergence 收口；W98-W105 完成第一个 Plan-Execute 产品纵切，仅限非默认 weekly planning；W106-W113 完成 RuntimeStrategy / Multi-Strategy Runtime Maturity；W114-W123 完成 ReAct Beta execution hardening；W124-W149 完成 Backend Completion Goals 1-8；W150-W158 完成 Skill Runtime Beta Maturity。Main Chat Agent v1 现在是 ordinary Chat 的入口/control plane scaffold/partial path，已具备 governed DirectAnswer send/stream AgentRun/task-session proof、send/stream L2 DirectAnswer scheduler/provider trace proof、governed plan-guided ReAct AgentLoop attempt、metadata-safe tool-candidate contract、generic MCP read bounded read-only manifest candidate set（deterministic capability/name/tag ranking（不使用 raw manifest id/description），provider/model-ranked candidate preselection complete-permutation local HTTP proof，candidate rank/source/capability digest/bounded safe capability labels/sanitized match reason evidence，并排除 high-risk/confirmation/write-like read-shaped manifests）、AgentLoop exact `toolset_allowlist` target enforcement / exact action-target allowlist enforcement、model-selected ExecutionPolicy metadata、governed candidate arguments source/digest metadata、allowlist 外 target / wrong action-target pair / write-like 或 unsupported action type / unknown model tool call 显式 blocker proof、model-supplied arguments 被 governed candidate contract executor input 覆盖的 boundary proof、policy-denied selected candidate blocker path、controlled knowledge-format bounded context loader、sanitized optional selected skill id command/wrapper/Chat composer manual field plumbing、single-step ActionExecutor-backed ReAct read fallback、eval-gated memory/session multi-step AgentLoop proof、web AgentLoop blocker proof、fixture-backed successful web read AgentLoop proof、registered MCP AgentLoop success proof、runtime eval webPolicyBlocker/mcpMissingReadTarget blocker-state proof、webSuccessfulReadCoverage proof、mcpRegisteredReadSuccess proof、mcpToolPermissionProposalCoverage proof、core runtime eval finalCompletionReady=false/live-provider blockers、core final acceptance gate fail-closed aggregation proof、local HTTP OpenAI-compatible provider-client proof（不计 external live credit）、accepted ToolPermission resume replay command-surface proof、cancel queued-action stop proof、governed follow-up synthesis、execution task panel、Review Center route-level accept-resume handoff、send/stream file-read command-surface proof、send/stream PlanExecute draft command-surface proof、send/stream proposal-path command-surface proof、send/stream registered-MCP command-surface AgentLoop success proof、send/stream registered-MCP ToolPermission proposal proof、send/stream web AgentLoop blocker proof、send/stream web/MCP blocker command-surface proof，以及 24-case send/stream command-surface eval gate（DirectAnswer/scripted provider/file/PlanExecute/proposal/web/MCP/ToolPermission，legacy fallback=0、silent write=0，finalCompletionReady=false/live-provider blockers），但仍必须证明 live-provider-backed provider eval、更广 provider-backed web/MCP AgentLoop proof 和更广 provider/live proposal-permission 验收，并保持 proposal-first、permission/blocker、metadata-safe trace、bounded context 和 no-silent-write 约束。
- W9-W18 只建立 non-default preview、preview audit 和 read-only migration gate/evidence surface；它们不替换 default Chat。
- W19-W23 只提供 controlled pilot eligibility、显式 pilot、reviewed promotion、source binding 和 metadata-safe promotion evidence；普通 Send 仍不调用 eligibility/gate/preview。
- W24-W48 的 readiness/review/preview/gate/draft/evidence 只允许人工审阅、对比、planning 或 implementation discussion；ready/approve/draftReady/previewReady 都不是 migration permission。
- W49-W55 是 pure ordinary-entry guard/preflight：默认 `send_message` / `start_stream_message` 只能在 typed contract ready、controlled executor unattached、migration disabled、zero runtime/model/tool/write budget 下进入 `legacy_stream`，并只能 fail closed。
- W56-W60 是 read-only/status/planning/review/readiness surfaces；普通 `Send` / `send_message` / `start_stream_message` 不得调用这些 command。
- W61-W63 是 docs/index整理阶段，不运行 runtime/model/tool、不写记录、不切换 routing，不是 default Chat migration。
- W64 是 W1-W63 文档权威压缩验收；W65 是纯后端 descriptor / mapper 骨架，metadata-safe、executor disabled/unattached、zero side-effect budget；W66 是纯后端 controlled adapter contract report / evaluator / ensure，复用 W65 descriptor 并保持 migrationPermission=false、controlled adapter invocation disabled、executor disabled/unattached、zero side-effect budget；W67 是纯后端 non-default invocation harness，只复用 W66 contract report，`harness_ready` 仅表示 invocation shape proof safe；W68 是纯后端 send-compatible proof/evaluator/ensure，只复用 W65-W67 metadata-safe 结果，`proof_ready` 仅表示 SendMessageResult-compatible shape safe；W69 是纯后端 stream-compatible boundary proof/evaluator/ensure，只复用 W65-W67 metadata-safe 结果，`proof_ready` 仅表示 `start_stream_message`-compatible metadata boundary safe，且 streamStarted/eventChannelOpened/streamEventsEmitted 必须全为 false；W70 是纯后端 executor attachment gate report/evaluator/ensure，复用 W65-W69 结果并明确 executor implementation missing / human review missing / route cutover not authorized blockers；W71 是纯后端 disabled executor skeleton contract/evaluator/ensure，复用 W70 gate report，只允许 metadata-only send result / stream boundary placeholder，保持 executor disabled/unattached/not runnable、invocation disallowed、zero side-effect budget；W72 是纯后端 skeleton binding integrity report/evaluator/ensure，复用 W71 skeleton、W71 input 和 W70 gate report，只验证 input/hash/route/shape/gate/no-run/no-write/no-stream metadata 一致性。W65-W72 都不是 command/surface，不是 executor implementation，不是 executor attachment，不是 route cutover，不是 default Chat migration。
- W10 的 preview AgentRun audit 是 metadata-safe 外层 run。ReAct payload 里的 inner run id 只能作为 child metadata 存在，不是 Runs 查询和产品 trace 的主 id。
- 下一轮 Goal-mode 默认应从 `plans/main_chat_agent_v1_stabilization_goal_spec.md` 开始，先修复验收阻断；不得直接重启旧 Main Chat Migration 大 Goal。除非用户显式启动 `plans/main_chat_agent_migration_v1_goal_spec.md` 定义的 Main Chat Agent Migration v1 Goal，否则后续 Agent 不得默认替换 `send_message`、`start_stream_message` 或 Chat 主流程；任何普通迁移必须先从非默认 preview/debug 入口或受控子路径开始，并保留稳定 fallback。Main Chat Agent Migration v1 是该规则要求的独立 reviewed migration Goal，必须按其 eval/fallback/policy/proposal/memory/UI/docs gate 完成，不能以 preview/status/readiness-only surface 替代。
- Tools 是 Agent 的执行能力，不是附属页面。OpenLife Beta 必须具备 OpenClaw-like 的 tool execution seriousness，但必须叠加 LifeModel、Privacy、Permission、Proposal、Audit 约束。
- Beta 的 Execution Tools 至少要覆盖 MCP、A2A、file、web、calendar、email、task proposal 等类别；未实现真实 executor 的工具必须 disabled/declarative-only，不能伪装成可执行。
- `calendar.propose_event` / `email.propose_draft` 当前是 P1 proposal-only governed executors：只创建 `ScheduledTask` / `DataExport` proposal，不执行真实日历写入、邮件发送或 `ExternalWriteAction` fallback。后续若接入真实 provider executor，必须重新补治理测试和 taxonomy。
- 文档入口和工具 taxonomy 必须与代码状态同步；过期 P1/P2 标签会误导后续 Agent，视为架构阻塞项。
- 新功能必须能挂到 `AgentTask`、`AgentRun`、`AgentAction`、`AgentProposal`、`LifeModel`、`Memory`、`ModelRouter` 或 `Workspace` 中。
- Chat、Builder、Calibration、Dashboard 都只是 Agent Framework 的不同表面，不是彼此孤立的产品中心。
- 高风险 LifeModel 更新、外部工具写操作、敏感数据上云必须可解释、可确认、可回滚。
- LifeModel-HS 开发必须遵守 ADR 0013：增量落地、Proposal-first、privacy as hard Policy、metadata-safe audit、YAML 仅作为 compatibility materialized view。
- LifeModel-HS 不是孤立功能区，而是跨 Chat、Builder、Calibration、Memory、Tools、ModelRouter、AgentRun、Proposal 的协议层；后续成熟化开发必须沿着 `LifeEvent -> Signal -> Evidence -> Governor -> Proposal -> Materialized View` 收敛。
- LifeModel Maturation Loop End-to-End Goal 必须以 `plans/lifemodel_maturation_goal_plan.md` 为入口；W73 只完成 read-only readiness report，W74 只完成显式 non-default maturation invocation，且仅在 W73 ready 时写 EvidenceStore + ProposalStore；W75 只把 maturation proposal 的 accept/reject/edit 结果回写为 metadata-safe `ProposalOutcome` evidence；W76 只把 accepted/edited/rejected outcome evidence 聚合为 reviewable pending collaboration rule candidate proposal，不激活 Heuristic、不写 active rule；W77 只证明已接受 candidate 可被 future RuntimeHSPacket selection 以 metadata-safe planning guidance 形式选中，且不放宽 privacy / model route policy；W78 只证明该 selected guidance 与 lineage 可在 future trace metadata 中以 id/hash/count/status/type 和 policy proof 可见，不写 AgentRun store 或 active truth。
  W79-W97 与 maturation runtime 无关，只推进 legacy direct-write convergence 的 inventory、manual editor governance、Builder/Calibration/Feedback retirement、Snapshot restore / Data import governance、State/Daily Goal source-data boundary、materializer caller matrix/restriction、proposal apply PatchSource mapping closure 和 final convergence inventory。
  W98-W105 是独立的 Plan-Execute Product Vertical：只面向 weekly planning、显式 session lifecycle、proposal-first step execution 和 metadata-safe trace。
  W106-W113 是独立的 RuntimeStrategy / Multi-Strategy Runtime maturity layer：ReAct 和 PlanExecute executable strategies descriptor/registry ready。
  Direct/Layered/Workflow/Proactive/Reflective 仅为 disabled/declarative future descriptors；status/readiness 命令只读、无 runtime/model/tool/store writes。
  不得自动接入 ordinary Chat，不得直接写 LifeModel/Memory/Heuristic active truth，不得绕过 Proposal/Governor，不得扩大到 identity/values/relationships/health/finance 等高风险域。
- 插件在 Beta 阶段默认是本地 Manifest / declarative-only；除非存在真实安全 executor，否则 plugin-declared tool 不能显示为可执行能力。

---

## 🏗️ 架构说明

### 目录结构

```
.
├── Cargo.toml                    # Rust Workspace 定义
├── README.md                     # 用户面向文档
├── AGENTS.md                     # 本文档 ← AI 助手上下文
├── .env.template                 # 环境变量模板
├── .env.example                  # 环境变量完整示例
├── Makefile                      # 跨平台快捷命令
├── setup.sh / setup.ps1          # 环境初始化脚本
├── dev.sh / dev.ps1              # 快速开发启动
├── start.sh / start.ps1          # 生产构建
├── startup.sh / startup.ps1      # 一体化工具（dev / a2a / check）
│
├── frontend/                     # React 前端（Vite 构建）
│   ├── package.json              # pnpm 依赖
│   ├── vite.config.ts            # Vite 配置 (port 5173)
│   ├── vitest.config.ts          # 测试配置 (Vitest + jsdom)
│   ├── tailwind.config.js        # Tailwind CSS 配置
│   └── src/
│       ├── main.tsx              # 入口 (HashRouter)
│       ├── App.tsx               # 路由 + 导航 + ErrorBoundary
│       ├── tauri.ts              # ⭐ Tauri Command 封装层
│       ├── types.ts              # 全局类型定义
│       ├── index.css             # Tailwind 导入
│       ├── test/
│       │   ├── setup.ts          # 测试初始化 (mock ResizeObserver 等)
│       │   └── mocks/tauri.ts    # Tauri invoke mock（约 30+ 命令）
│       ├── components/           # 通用组件
│       │   ├── ReasoningTracePanel.tsx
│       │   ├── ToolCallCard.tsx
│       │   └── LoadingSpinner.tsx
│       └── pages/                # 页面组件
│           ├── ChatPage.tsx
│           ├── DashboardPage.tsx
│           ├── LifeModelEditor.tsx
│           ├── BuilderPage.tsx
│           ├── A2APage.tsx
│           ├── McpPage.tsx
│           ├── MemorySearch.tsx
│           ├── VersionControl.tsx
│           ├── SettingsPage.tsx
│           └── CalibrationPage.tsx
│
├── openlife-core/                # Rust 核心业务库
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # 模块暴露入口
│       ├── config.rs             # AppConfig (YAML + env 覆盖)
│       ├── life_model.rs         # 四维人生模型（Identity/Goals/Capabilities/State）
│       ├── llm.rs                # DeepSeek / OpenAI-compatible 云端模型调用
│       ├── ollama.rs             # Ollama 本地模型调用
│       ├── scheduler.rs          # 推理调度器（本地优先策略）
│       ├── reasoning/            # 推理策略模块
│       │   ├── mod.rs            # ReasoningStrategy trait
│       │   ├── layered.rs        # 三层推理策略 (Meaning→Strategy→Generation)
│       │   └── direct.rs         # 直接推理策略
│       ├── memory.rs             # SQLite 消息/会话/状态存储
│       ├── vectors.rs            # 向量记忆 Tier 3（SQLite + 本地 embedding）
│       ├── router.rs             # 意图路由
│       ├── layer_router.rs       # 层路由
│       ├── mcp.rs                # MCP 客户端（JSON-RPC stdio）
│       ├── mcp_audit.rs          # MCP 调用审计
│       ├── a2a.rs                # A2A 协议实现
│       ├── privacy.rs            # PII 检测与脱敏
│       ├── versioning.rs         # Git-like 快照与回滚
│       ├── feedback.rs           # 用户反馈与进化信号
│       ├── builder.rs            # 构建模式（引导式人生模型创建）
│       ├── reflex_engine.rs      # 反射引擎
│       ├── evolution.rs          # 微进化系统
│       └── tool_manifest.rs      # 工具清单定义
│
├── src-tauri/                    # Tauri 应用壳
│   ├── Cargo.toml                # Tauri 依赖 + bin 定义
│   ├── tauri.conf.json           # Tauri 配置（identifier: ai.openlife.app）
│   ├── capabilities/default.json # Tauri 权限声明
│   └── src/
│       ├── main.rs               # 桌面应用入口
│       ├── lib.rs                # ⭐ 模块暴露与 Tauri 命令注册地（Main Chat 主体已拆到 focused modules）
│       ├── a2a_server.rs         # A2A 服务器模块
│       ├── a2a_sidecar.rs        # A2A 侧车进程管理
│       ├── commands/             # 67+ 命令按领域拆分为 13 个模块
│       │   ├── mod.rs            # 模块声明入口
│       │   ├── a2a.rs            # 7 个 A2A 命令
│       │   ├── builder.rs        # 11 个 Builder 命令
│       │   ├── calibration.rs    # 6 个 Calibration 命令
│       │   ├── chat.rs           # 6 个 Chat 会话命令
│       │   ├── diagnostics.rs    # 5 个诊断命令
│       │   ├── feedback.rs       # 5 个反馈命令
│       │   ├── reasoning.rs      # 推理策略命令
│       │   ├── life_model.rs     # 2 个 LifeModel 命令
│       │   ├── mcp.rs            # 8 个 MCP 命令
│       │   ├── memory.rs         # 10 个 Memory 命令
│       │   ├── settings.rs       # 12 个 Settings 命令
│       │   ├── state.rs          # 8 个 State 命令
│       │   └── version.rs        # 4 个版本命令
│       └── bin/
│           └── a2a_server.rs     # 独立 A2A HTTP 服务器（Axum，port 8765）
│
├── scripts/
│   └── quantize_int8.py          # ONNX INT8 量化工具
│
└── plans/                        # 项目规划文档
    ├── openlife_agent_framework_architecture.md
    ├── openlife_development_plan.md
    ├── openlife_codex_execution_playbook.md
    ├── frontend_experience_rebuild_plan.md
    └── engineering_structure_notes.md
```

### 核心模块

| 模块 | 文件路径 | 职责 | 依赖关系 |
|------|----------|------|----------|
| **AgentRun** | [`openlife-core/src/agent/`](openlife-core/src/agent/) | AgentRun 追踪：每次 Chat/Builder/Calibration 生成可查询的运行记录，包含模型路由 trace、上下文摘要、成功/失败状态 | 被 chat.rs、builder.rs 使用，存储在独立 SQLite agent_runs.db |
| **Proposal** | [`openlife-core/src/agent/proposal_store.rs`](openlife-core/src/agent/proposal_store.rs) | Proposal 统一层：LifeModel/Memory/Tool 权限变更必须经过用户确认（accept/reject/edit/postpone），应用前自动创建 snapshot | 被 builder.rs、calibration.rs 使用，存储在 SQLite proposals.db |
| **LifeModel** | [`openlife-core/src/life_model.rs`](openlife-core/src/life_model.rs) | 四维人生模型：Identity（身份/价值观）、Goals（短中长期目标）、Capabilities（技能/资源）、State（当前状态/情绪/健康） | 被 reasoning.rs、scheduler.rs、memory.rs 消费 |
| **LayeredReasoner** | [`openlife-core/src/agent/reasoning/layered.rs`](openlife-core/src/agent/reasoning/layered.rs) | 三层推理策略：MeaningPhase（语义理解/禁忌检测）→ StrategyPhase（策略规划）→ GenerationPhase（回复生成），SafetyChecker 安全检查。作为 AgentRuntime 的默认推理策略 | 依赖 scheduler.rs、life_model.rs |
| **InferenceScheduler** | [`openlife-core/src/scheduler.rs`](openlife-core/src/scheduler.rs) | 智能调度云端/本地模型：tool prompt → 强制云端；Ollama 可用 + prefer_local → 本地；否则 fallback 云端 | 依赖 llm.rs、ollama.rs |
| **MemoryStore** | [`openlife-core/src/memory.rs`](openlife-core/src/memory.rs) | SQLite 持久化：聊天记录、会话管理、人生模型快照、状态历史、自定义记忆记录 | 独立，被 lib.rs 调用 |
| **VectorStore** | [`openlife-core/src/vectors.rs`](openlife-core/src/vectors.rs) | 向量记忆 Tier 3：存储 embedding，支持余弦相似度检索、session 过滤、tier 升降维护 | 依赖 tract-onnx/tokenizers 做本地 embedding |
| **McpRegistry** | [`openlife-core/src/mcp.rs`](openlife-core/src/mcp.rs) | MCP 客户端管理：注册/注销服务器、list_tools、call_tool、内置工具、参数隐私检查 | 依赖 privacy.rs、tool_manifest.rs |
| **MultiStrategyRuntime** | [`openlife-core/src/agent/multi_strategy_runtime.rs`](openlife-core/src/agent/multi_strategy_runtime.rs) | Preview/core orchestrator：用 StrategySelector 在 ReAct、PlanExecute、Blocked payload 间选择，并输出 warnings | 仅通过 `run_multi_strategy_agent_preview` 非默认命令暴露；尚未接管 Chat |
| **Preview Audit** | [`src-tauri/src/commands/agent_runtime/`](src-tauri/src/commands/agent_runtime/) + [`frontend/src/utils/previewAudit.ts`](frontend/src/utils/previewAudit.ts) | W10 metadata-safe 外层 AgentRun audit；Runs / Trace 识别 preview strategy/payload/governance/warnings | ReAct inner run id 只作为 child metadata，不作为主查询 id |
| **Pilot Eligibility** | [`openlife-core/src/agent/runtime_migration_gate.rs`](openlife-core/src/agent/runtime_migration_gate.rs) + [`src-tauri/src/commands/agent_runtime/`](src-tauri/src/commands/agent_runtime/) | W19 sustained gate evidence evaluator：只读检查最近 preview AgentRun 的 gate report 是否连续干净 | Settings 显示 controlled Chat migration pilot 资格；不是 Chat 开关，不写入任何新 audit/run/proposal |
| **Controlled Chat Pilot / Promotion Evidence / Readiness / Draft / Review Decision / Implementation Gate / Shadow Run / Shadow Review** | [`frontend/src/pages/ChatPage.tsx`](frontend/src/pages/ChatPage.tsx) + [`src-tauri/src/commands/agent_runtime/`](src-tauri/src/commands/agent_runtime/) | W20 very small controlled pilot + W21 reviewed promotion + W22 source-bound validation + W23 evidence recorder + W24 readiness gate + W25 draft plan + W26 review decision evidence + W27 implementation gate + W28 shadow run + W29 shadow review evidence：Chat 页面显式按钮先查 eligibility，eligible 后运行单次 write-disabled preview；成功 `userOutput` 可 review/confirm promotion；Settings 可只读检查 promotion evidence readiness、生成 migration plan draft、记录/汇总 metadata-safe review decision evidence、检查 implementation eligibility、显式运行 write-disabled shadow run，并人工记录/汇总 metadata-safe shadow review evidence | 默认 Send 不变；blocked/failed/no-output/cancel/repeat 不写入；确认后只在 source/target session 一致时写一条 ordinary assistant message，并写入 metadata-safe promotion evidence，可带 `run_id` trace；readiness pass、migration draft、review approval、implementation eligibility、shadow readiness 和 shadow review approval 都不是迁移许可 |
| **Tauri Commands** | [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) | 30+ 个 `#[tauri::command]`：聊天、MCP、A2A、记忆、版本控制、Builder、进化、校准、系统诊断 | 依赖 openlife-core 全部模块 |
| **Frontend API** | [`frontend/src/tauri.ts`](frontend/src/tauri.ts) | TypeScript 封装层：所有后端调用的唯一入口，约 40+ 个 invoke 函数 | 仅依赖 `@tauri-apps/api/core` |

### 目标架构主线

当前代码已有 AgentRun / RuntimeInput-Output / HS packet / StrategySelector / MultiStrategy preview 等关键部件；普通 Main Chat 已进入 AgentIngress / governed task session 的 partial control-plane 路径并有 execution task panel、DirectAnswer send/stream AgentRun/task-session proof、memory/session multi-step AgentLoop proof、web AgentLoop blocker proof、fixture-backed successful web read AgentLoop proof、registered MCP AgentLoop success proof、no-planned-action AgentLoop fallback、runtime eval webPolicyBlocker/mcpMissingReadTarget blocker-state proof、webSuccessfulReadCoverage proof、mcpRegisteredReadSuccess proof、mcpToolPermissionProposalCoverage proof、accepted ToolPermission resume replay command-surface proof、cancel queued-action stop proof、Review Center route-level accept-resume handoff、send/stream file-read command-surface proof、send/stream PlanExecute draft command-surface proof、send/stream proposal-path command-surface proof、send/stream registered-MCP command-surface AgentLoop success proof、send/stream registered-MCP ToolPermission proposal proof、send/stream web AgentLoop blocker proof、send/stream web-policy / missing-MCP blocker command-surface proof，以及 24-case send/stream command-surface eval gate（legacy fallback=0、silent write=0），但仍需证明 live-provider-backed provider eval、更广 provider-backed web/MCP AgentLoop proof 和更广 provider/live proposal-permission 验收。后续开发应沿下面这条主线继续收敛：

```
用户意图 / 主动触发
    │
    ▼
AgentTask
    │
    ▼
AgentRun
    │
    ├── ContextAssembler：选择 LifeModel、记忆、会话上下文
    ├── ModelRouter：选择本地模型、云端模型或混合路径
    ├── ReAct Engine：Reason → Act → Observe → Reflect
    ├── ActionExecutor：内部动作 / MCP / A2A / LifeModel patch / Memory write
    └── ProposalEngine：生成待确认的 LifeModel、Memory、Tool 权限变更
    │
    ▼
用户确认 / 编辑 / 拒绝
    │
    ▼
LifeModel / Memory / Audit / Snapshot 持久化
```

未来新增模块时，优先放入这些概念，而不是继续增加互相孤立的页面级逻辑。

### 数据流

```
用户输入（ChatPage.tsx）
    │
    ▼
[frontend/src/tauri.ts] ──invoke──► [src-tauri/src/lib.rs]
    │                                    │
    │    ┌───────────────────────────────┼───────────────────────────────┐
    │    ▼                               ▼                               ▼
[Stream UI]                    [LayeredReasoner]                 [MemoryStore]
(逐字显示)               Meaning→Strategy→Generation        (保存消息/快照)
    │                            │
    │                    [InferenceScheduler]
    │                    ┌────────┴────────┐
    │                    ▼                 ▼
    │              [Ollama]          [OpenRouter]
    │            (localhost)         (云端 API)
    │                    │
    │                    ▼
    │              [VectorStore] ← 嵌入检索上下文
    │                    │
    └────────────────────┘
                         │
                    [McpRegistry] ← 工具调用（如有）
                         │
                    [PrivacyEngine] ← PII 检测/脱敏
```

关键处理节点：
1. **输入预处理**（[`preprocess_chat_input`](src-tauri/src/lib.rs:391)）：用户消息 → 向量检索相关记忆 → LayeredReasoner 请求构建
2. **三层推理**（[`LayeredReasoner::reason`](openlife-core/src/agent/reasoning/layered.rs)）：MeaningPhase（语义理解）→ StrategyPhase（JSON 策略）→ GenerationPhase（回复生成）
3. **模型调度**（[`InferenceScheduler::generate`](openlife-core/src/scheduler.rs:71)）：根据 tool prompt 和 Ollama 可用性决定使用本地或云端模型
4. **工具调用**（[`execute_tool_call_internal`](src-tauri/src/lib.rs:264)）：MCP 工具执行 + 隐私参数脱敏 + 审计日志
5. **流式输出**（[`start_stream_message`](src-tauri/src/lib.rs:822)）：SSE 风格流式传输到前端

> 注意：上面的数据流描述的是当前实现。目标架构会把这条链路收敛到 `AgentRun`，并让每次模型调用、上下文选择、工具调用和 LifeModel 更新都具备可查询 trace。

---

## 🛠️ 开发规范

### 命名约定

| 范畴 | 约定 | 示例 |
|------|------|------|
| **Rust 文件/目录** | `snake_case` | `life_model.rs`, `mcp_audit.rs` |
| **Rust 结构体/枚举** | `PascalCase` | `LifeModel`, `LayeredReasoner`, `AlertLevel` |
| **Rust 方法/函数** | `snake_case` | `save_message()`, `run_tier_maintenance()` |
| **Rust 常量** | `UPPER_SNAKE_CASE` | `OLLAMA_CACHE_TTL = 10` |
| **Rust 模块** | `snake_case` | `mod memory;` |
| **TS/前端 组件文件** | `PascalCase` | `App.tsx`, `ChatPage.tsx`, `LifeModelEditor.tsx` |
| **TS/前端 工具文件** | `camelCase` | `tauri.ts`, `modelEmpty.ts`, `setup.ts` |
| **TS/前端 组件/类** | `PascalCase` | `ErrorBoundary`, `ToolCallCard` |
| **TS/前端 函数/方法** | `camelCase` | `sendMessage()`, `safeInvoke()` |
| **TS/前端 接口/类型** | `PascalCase` | `AppConfig`, `ChatMessage`, `SendMessageResult` |
| **TS/前端 常量** | `UPPER_SNAKE_CASE` | （需要人工补充具体命名） |
| **YAML 配置键** | `snake_case` | `prefer_local_model`, `openai_base` |
| **数据库表名** | `snake_case` 复数 | `messages`, `chat_sessions`, `vector_chunks` |

### 代码风格

| 语言 | 项目 | 说明 |
|------|------|------|
| **Rust** | 缩进 | 4 空格 |
| **Rust** | 引号 | 双引号 |
| **Rust** | 行尾分号 | 使用 |
| **Rust** | 最大行宽 | 约 100-120 字符（推断） |
| **Rust** | 注释 | 行内注释 `//`，Doc 注释 `///` |
| **TypeScript** | 缩进 | 2 空格 |
| **TypeScript** | 引号 | 双引号（从 `App.tsx` 观察） |
| **TypeScript** | 行尾分号 | 使用（从 `App.tsx` 观察） |
| **TypeScript** | 最大行宽 | 约 100-120 字符（推断） |
| **TypeScript** | 严格模式 | `tsconfig.json` 启用 `strict: true`，`noUnusedLocals: true`，`noUnusedParameters: true` |
| **CSS** | 类名 | Tailwind 工具类为主，无自定义 BEM |
| **Python** | 缩进 | 4 空格（scripts/quantize_int8.py） |

### Git 提交规范

> ⚠️ 以下内容为合理推测，**需要人工补充**确认实际规范。

- **提交消息格式**：建议采用 [Conventional Commits](https://www.conventionalcommits.org/)（如 `feat:`, `fix:`, `refactor:`, `docs:`）
- **分支命名**：建议 `feature/xxx`, `bugfix/xxx`, `refactor/xxx`
- **PR 流程**：（需要人工补充）

---

## 📐 业务逻辑规则

### 核心业务实体

| 实体 | 关键属性 | 关系 |
|------|----------|------|
| **AgentTask** | `id`, `kind`, `user_intent`, `life_context_scope`, `execution_policy`, `status` | 目标架构中的任务入口；Chat、Builder、Calibration、Proactive Check-in 都应逐步映射为 AgentTask |
| **AgentRun** | `task_id`, `model_route`, `context_summary`, `actions`, `observations`, `output`, `proposals`, `errors` | 目标架构中的一次可追踪执行；后续用于解释“用了什么上下文、哪个模型、做了什么动作” |
| **AgentProposal** | `proposal_type`, `affected_path`, `before`, `after`, `reason`, `confidence`, `risk_level`, `status` | Builder、Calibration、Evolution、Memory 更新的统一确认结构 |
| **LifeModel** | `metadata`, `identity`, `goals`, `capabilities`, `state`, `relationships`, `preferences` | 1 个用户 1 个当前 LifeModel；支持快照版本控制 |
| **Identity** | `name`, `values[]`, `personality_traits[]`, `life_philosophy`, `mission_statement`, `role_definition`, `voice_style` | 属于 LifeModel 的子维度 |
| **Goals** | `short_term[]`, `medium_term[]`, `long_term[]`, `life_goals[]`, `daily[]` | 每个 GoalItem 有 `priority`, `progress`, `deadline`, `milestones[]` |
| **State** | `current_focus`, `health_status`, `emotional_state`, `habit_streaks[]`, `custom_dimensions[]`, `alerts[]` | 支持自定义维度 + 阈值预警 |
| **ChatMessage** | `role` (system/user/assistant), `content`, `tool_calls?`, `name?` | 属于 ChatSession；持久化到 SQLite |
| **MemoryChunk** | `session_id`, `content`, `embedding[]`, `source`, `tier`, `access_count` | 向量记忆，tier 1/2/3 分层 |
| **ToolManifest** | `name`, `description`, `source` (builtin/mcp/external), `parameters` | 注册到 McpRegistry |
| **ReasoningTrace** | `meaning_result`, `strategy_result`, `generation_result`, `safety_check_result` | 一次对话请求的推理过程痕迹 |

### 状态机和流程

#### 1. 目标 Agent Runtime 流程

```
用户请求 / 主动触发
    │
    ▼
AgentTask 创建
    │
    ▼
AgentRun 执行
    │
    ├── 组装 LifeModel + Memory + 会话上下文
    ├── 根据隐私、能力、成本、工具需求选择模型路径
    ├── Reason / Act / Observe 循环
    ├── 生成用户输出
    └── 生成 LifeModel / Memory / Tool 权限 Proposal
    │
    ▼
用户确认、编辑或拒绝 Proposal
    │
    ▼
写入 LifeModel / Memory / Snapshot / Audit
```

#### 2. 当前 LayeredReasoner 三层推理流程

```
用户输入
    │
    ▼
┌─────────────┐    语义理解 + 禁忌话题检测
│ MeaningPhase│ ──► 输出：user_text, forbidden_topics[]
└─────────────┘
    │
    ▼
┌───────────────┐    策略规划（JSON 输出）
│ StrategyPhase │ ──► 输出：strategy_json（含工具调用意图）
└───────────────┘
    │
    ▼
┌─────────────────┐    回复生成（最终回复）
│ GenerationPhase │ ──► 输出：assistant_reply
└─────────────────┘
    │
    ▼
┌──────────────┐    安全检查：验证输出是否偏离策略/意义
│ SafetyChecker│ ──► 最终返回给用户
└──────────────┘
```

每层有独立超时（Meaning 5s / Strategy 15s / Generation 30s，可配置），失败时由 AgentRuntime 决定降级策略。

LayeredReasoner 是 AgentRuntime 的默认推理策略，通过 `ReasoningStrategy` trait 注册。未来可扩展 DirectReasoner（直接推理）、ReActReasoner（工具循环推理）等策略。

#### 3. 当前模型调度策略

```
用户消息
    │
    ▼
是否有 tools_prompt ?
    │
   YES ──► 跳过本地模型，强制使用云端（7B 本地模型工具调用不可靠）
    │
   NO ──► Ollama 可用 && prefer_local=true ?
            │
           YES ──► 使用 Ollama（localhost:11434）
            │
           NO  ──► 有 API Key ?
                    │
                   YES ──► 使用云端（OpenRouter / OpenAI）
                    │
                   NO  ──► 返回"未配置后端"提示
```

目标状态下，这条逻辑应升级为 `ModelRouter`：

- 按任务类型选择模型角色：chat / planner / tool_use / summarizer / extractor / embedding
- 按隐私策略决定本地、云端或摘要上云
- 记录每次 AgentRun 的 provider、model、fallback、redaction trace

#### 4. 记忆检索流程

```
用户输入
    │
    ▼
提取用户消息文本
    │
    ▼
生成 embedding（本地 tract-onnx 或 OpenAI API）
    │
    ▼
VectorStore.search(query_embedding, top_k=5)
    │
    ▼
计算余弦相似度 + tier 加分（tier 1 > tier 2 > tier 3）
    │
    ▼
返回 MemoryChunk[] 作为上下文注入 prompt
    │
    ▼
访问计数 +1（bump_access_for_chunks）
```

#### 5. 每日目标自动打卡流程

[`try_auto_checkin_daily_goals`](src-tauri/src/lib.rs:357) 会在每次 assistant 回复后检查内容中是否提到完成了某个 daily goal 的名称，自动将 `done` 标记为 `true`。

### 业务规则约束

1. **AgentRun 优先**：后续任何重要 AI 执行都应逐步记录为 AgentRun，而不是只在页面 state 中完成。
2. **LLM 后端最低要求**：至少配置一个 LLM 后端（Ollama 或云端 API Key）才能使用模型对话功能。
3. **工具调用默认保守**：MCP/A2A/external 写操作必须走确认或 allowlist，不能只依赖前端隐藏结果。
4. **LifeModel 高风险更新需确认**：身份、价值观、人生使命、长期目标等字段必须由用户确认后写入。
5. **PII 本地拦截**：所有 outgoing 请求经过 [`PrivacyEngine`](openlife-core/src/privacy.rs) 检测，高敏感度 PII 应阻止发送或脱敏。
6. **消息 checksum**：保存消息到 SQLite 时，根据 `content + session_id + created_at` 生成 SHA256 checksum，用于完整性校验。
7. **Ollama 缓存 10 秒**：`ollama.rs` 每 10 秒缓存一次模型可用性检查，状态变化不会立即反映。
8. **向量记忆 tier 维护**：`vectors.rs` 定期运行 `run_tier_maintenance()`，高频访问 chunk 晋升 tier，低频降级。
9. **HashRouter 强制使用**：前端必须使用 `HashRouter` 而非 `BrowserRouter`，因为 Tauri 桌面应用基于 `file://` 协议。
10. **数据目录统一**：应用数据目录已统一为 `ai.openlife.app`（与 `tauri.conf.json` 的 `identifier` 一致），macOS 路径为 `~/Library/Application Support/ai.openlife.app/`。旧版本数据在 `com.openlife.app`，如需迁移请手动复制。

### Tool Taxonomy（Beta 工具分类）

OpenLife Beta 将工具按执行能力分为 **P1（真实可执行）**、**P2（declarative-only stub）** 和 **治理待校准**。未实现真实 executor 或治理语义未闭合的工具不得伪装为完成 P1。入口文档与本 taxonomy 必须随代码状态同步更新。

#### Core OS Tools（P1 — 真实可执行）

| 工具 | 用途 | 风险等级 | 状态 |
|------|------|----------|------|
| `life_model.read` | 读取 LifeModel 字段 | low | ✅ P1 |
| `life_model.propose_patch` | 提议 LifeModel 变更 | medium | ✅ P1（生成 Proposal） |
| `goal.read` | 读取目标 | low | ✅ P1 |
| `goal.propose_update` | 提议目标更新 | medium | ✅ P1（生成 Proposal） |
| `state.read` | 读取状态 | low | ✅ P1 |
| `memory.search` | 检索记忆 | low | ✅ P1 |
| `memory.propose_write` | 提议记忆写入 | medium | ✅ P1（生成 MemoryWrite Proposal） |
| `memory.propose_archive` | 提议归档记忆 | medium | ✅ P1（生成 MemoryArchive Proposal） |
| `proposal.create` | 创建提案 | low | ✅ P1 |
| `proposal.list` | 列出提案 | low | ✅ P1 |
| `agent_run.lookup` | 查询运行记录 | low | ✅ P1 |
| `snapshot.create` | 创建快照 | low | ⚠️ declarative-only（Beta 不进入 tools prompt） |
| `tool.list_available` | 列出可用工具 | low | ✅ P1 |
| `permission.check` | 检查权限策略 | low | ✅ P1 |
| `permission.request` | 请求权限 | medium | ✅ P1（生成 ToolPermission Proposal） |
| `permission.replay_action` | 重放已授权动作 | medium | ✅ P1 |

#### Execution Tools（混合 P1/P2）

| 工具 | 用途 | 风险等级 | 状态 |
|------|------|----------|------|
| `file.read` | 读取本地文件 | low/medium | ✅ P1（受 safe_paths 限制） |
| `file.write_proposal` | 提议文件写入 | high | ✅ P1（生成 ExternalWriteAction Proposal） |
| `web.search` | 搜索网页 | medium | ✅ P1（DuckDuckGo/Brave/SearXNG 三后端 + rate limit） |
| `web.fetch` | 获取 URL | medium | ✅ P1（受私网拦截 + summarize Ollama 支持） |
| `mcp.call_tool` | 调用 MCP 工具 | 取决于目标 | ✅ P1（wrapper，权限落在目标 tool scope） |
| `a2a.call_agent` | 调用 A2A Agent | medium | ✅ P1（30s超时+私网拦截） |
| `calendar.read` | 读取日历 | low | ✅ P1（ICS parser） |
| `calendar.propose_event` | 提议日历事件 | medium | ✅ P1 proposal-only governed executor（只生成 `ScheduledTask` Proposal；不写日历，不生成 `ExternalWriteAction`） |
| `email.read` | 读取邮件 | low | ⚠️ P2（declarative-only，需配置 IMAP account） |
| `email.propose_draft` | 提议邮件草稿 | medium | ✅ P1 proposal-only governed executor（只生成 `DataExport`/email-draft Proposal；不发送邮件，不生成 `ExternalWriteAction`） |
| `task.create_proposal` | 提议创建任务 | medium | ✅ P1（ScheduledTask Proposal + TaskStore） |

> **P1 / P2 判定标准**：P1 工具必须具备真实 executor 或明确的 proposal-only governed executor、治理语义、proposal/apply/replay 路径和集成测试；P2 工具仅有 manifest 声明，无真实执行能力，标记为 `declarative_only: true`。P2 工具不会进入模型的 tools prompt，前端 Tool Registry 中显示为 "⚠️ 声明-only"。治理待校准工具必须先进入 W1 Tool Proposal Hygiene，不能被未来 Agent 当成已完成 P1。

---

## ⚙️ 环境配置

### 必需的环境变量

| 变量名 | 用途 | 示例值 | 是否必须 |
|--------|------|--------|----------|
| `DEEPSEEK_API_KEY` | DeepSeek API Key（当前推荐试用） | `sk-xxxxxxxx` | 否（三选一） |
| `OPENROUTER_API_KEY` | OpenRouter API Key | `sk-or-v1-xxxxxxxx` | 否（三选一） |
| `OPENAI_API_KEY` | OpenAI API Key | `sk-xxxxxxxx` | 否（三选一） |
| `OPENAI_API_BASE` | 自定义 API Base URL | `https://api.openai.com/v1` | 否（有默认值） |
| `A2A_PORT` | A2A 独立服务器端口 | `8765` | 否（默认 8765） |
| `PORT` | Vite 开发服务器端口 | `5173` | 否（默认 5173） |
| `TAURI_DEBUG` | Tauri 调试日志开关 | `1` | 否 |

> 至少配置 `DEEPSEEK_API_KEY`、`OPENROUTER_API_KEY` 或 `OPENAI_API_KEY` 之一才能使用云端模型对话。如果不配置，必须本地运行 Ollama。

### 外部服务依赖

| 服务 | 用途 | 配置位置 | 本地替代方案 |
|------|------|----------|-------------|
| **Ollama** (localhost:11434) | 本地 LLM 推理 | `.env` / `config.yaml` | 无替代，需本地安装 |
| **DeepSeek API** | 当前推荐云端试用 Provider | `.env` / `config.yaml` | OpenAI-compatible Provider |
| **OpenRouter API** | 云端 LLM（多模型聚合） | `.env` / `config.yaml` | DeepSeek / OpenAI API |
| **OpenAI API** | 云端 LLM（官方） | `.env` / `config.yaml` | DeepSeek / OpenRouter API |
| **SQLite** | 本地数据持久化 | 自动 bundled | 无需替代，零配置 |

配置优先级：**环境变量 > `config.yaml` > 代码默认值**

运行时配置文件路径（macOS）：`~/Library/Application Support/ai.openlife.app/config.yaml`

### SystemConfig 配置项

`config.yaml` 中新增 `system` 字段，支持以下配置：

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `system.ollama_cache_ttl_seconds` | u64 | 10 | Ollama 模型可用性缓存 TTL |
| `system.memory_search_top_k` | usize | 3 | Chat 流程中记忆检索数量 |

### 启动和测试命令

#### 开发模式

```bash
# 一键初始化（新开发者首选）
make setup
# 或
./scripts/setup.sh      # macOS/Linux
.\setup.ps1             # Windows

# 快速开发启动
make dev
# 或
./scripts/dev.sh        # macOS/Linux
.\dev.ps1               # Windows

# 一体化脚本启动
./scripts/startup.sh dev # macOS/Linux
.\startup.ps1 dev       # Windows
```

#### 运行测试

```bash
# 前端测试
make test-front
# 或
cd frontend && pnpm test

# Rust 测试
make test-rust
# 或
cargo test -p openlife-core
cargo test -p openlife-tauri

# 全部测试
make test
```

### 包管理器策略

- **统一使用 pnpm**：项目通过 `packageManager` 字段锁定 `pnpm@9.1.0`
- **启用 corepack**：`corepack enable && corepack prepare pnpm@9.1.0 --activate`
- **禁止 npm**：所有脚本已移除 npm fallback，不会生成 `package-lock.json`
- **CI 保护**：`make check-lockfile` 会检查是否误生成 `package-lock.json`

### 启动命令

```bash
# 一键初始化（新开发者首选）
make setup
# 或
./scripts/setup.sh      # macOS/Linux
.\setup.ps1             # Windows

# 快速开发启动
make dev
# 或
./scripts/dev.sh        # macOS/Linux
.\dev.ps1               # Windows

# 一体化脚本启动
./scripts/startup.sh dev # macOS/Linux
.\startup.ps1 dev       # Windows

# 检查环境
make check
# 或
./scripts/startup.sh check
```

#### 构建生产版本

```bash
# macOS
make build
# 或
./scripts/start.sh
# 或
pnpm tauri build --target universal-apple-darwin

# Windows
.\start.ps1
# 或
pnpm tauri build --target x86_64-pc-windows-msvc

# Linux
./scripts/start.sh
# 或
pnpm tauri build --target x86_64-unknown-linux-gnu
```

---

## 🐛 已知问题和注意事项

### 历史遗留问题

1. **reqwest 版本不一致**：`openlife-core` 使用 `reqwest 0.11`，`src-tauri` 使用 `reqwest 0.12`。目前编译通过，但建议统一版本以避免潜在兼容性问题。
2. ~~Ollama 缓存固定 10 秒~~ **已修复**：`ollama_cache_ttl_seconds` 已加入 `SystemConfig`，可通过 `config.yaml` 配置，默认仍为 10 秒。
3. ~~数据目录与 Tauri identifier 不一致~~ **已修复**：数据目录已统一为 `ai.openlife.app`。旧版本数据如需迁移请手动复制。
4. **MCP 审计日志单独数据库**：`mcp_audit.db` 与 `messages.db`/`vectors.db` 分开存储，这是设计上的隔离，但备份/迁移时容易遗漏。

### 常见陷阱

1. **忘记使用 HashRouter**：如果新开发者习惯性使用 `BrowserRouter`，Tauri 桌面应用在 `file://` 协议下会白屏。
2. **工具调用时 local model 被跳过**：如果配置了 `prefer_local=true` 但消息触发了 tool prompt，会静默切换到云端模型，前端无显式提示。
3. **忘记 bump_access**：手动操作 `VectorStore` 后如果不调用 `bump_access_for_chunks`，tier 维护不会正确晋升高频记忆。
4. **PII 检测导致 MCP 调用失败**：如果工具参数包含被标记为高风险的 PII，`McpRegistry` 会阻止调用，错误信息可能不够明确。
5. **`.env` 修改后需重启**：Tauri dev 不会热重载 `.env` 变更，修改 API Key 后需要重启开发服务器。

### 性能敏感区域

1. **向量检索余弦相似度**：[`cosine_similarity`](openlife-core/src/vectors.rs:285) 已改为 4-wide 手动向量化，提升指令吞吐。
2. **embedding 生成**：已实现 LRU Embedding 缓存（默认 1000 条，1 小时 TTL），避免重复计算相同文本的 embedding。
3. **LayeredReasoner 三层串行调用**：Meaning → Strategy → Generation 是串行的，每层都有 LLM 请求，总延迟 = 三层之和。Strategy 层要求输出合法 JSON，重试逻辑可能增加额外延迟。作为 AgentRuntime 的默认推理策略，可通过配置调整超时或切换为 DirectReasoner 降低延迟。
4. **SQLite 写入锁**：`MemoryStore` 使用 `Mutex<Connection>`，高并发写入（如同时保存消息 + 向量化 + 审计日志）会串行化。
5. **Ollama 首次加载延迟**：本地模型首次加载到 GPU 内存时可能有数秒延迟，缓存机制只检查可用性，不预热模型。

### 待重构区域

1. ~~**reqwest 版本统一**：将 `openlife-core` 升级到 `reqwest 0.12`。~~ ✅ 已完成（Gate 0）
2. ~~**Agent Runtime 引入**：新增 `AgentTask`、`AgentRun`、`AgentAction`、`AgentProposal` 和 `AgentRunStore`，先从 Chat 主链路接入。~~ ✅ 已完成（AgentRun/Proposal 基线已落地）
3. ~~**ModelRouter 升级**：灰度中；已补齐 provider health 语义和隐私优先路由，后续继续做 role-aware 策略与真实探针覆盖。~~ ✅ 已完成（Gate 6：已毕业，移除 experimental flag）
4. ~~**Proposal 统一**：Builder、Calibration、Evolution、Memory 更新应统一走 Proposal/Confirmation，而不是各自实现审批流。~~ ✅ 已完成（Builder/Calibration/Chat/Memory/Tool Permission MVP 已接入 Proposal 流）
5. ~~**LayeredReasoner / ReAct 边界重构**：三层推理总线应纳入 AgentRuntime 或成为其中一种策略。~~ ✅ 已完成（LayeredReasoner 已作为 AgentRuntime 的默认推理策略，通过 ReasoningStrategy trait 注册）
6. ~~**前端信息架构重构**：从多页面工具箱收敛为 Workspace / Agent / LifeModel / Memory / Runs / Settings。~~ ✅ 已完成（Gate 7：导航收敛为 Chat/Review/Runs/Settings）
7. **前端 ErrorBoundary 过于简单**：目前只显示红色背景文本，可以添加重试按钮或错误上报。
8. ~~**核心逻辑测试覆盖**：Rust 测试集中在 config.rs、vectors.rs、builder.rs、versioning.rs，核心逻辑（AgentRuntime、ModelRouter、LayeredReasoner、scheduler）需要补充测试。~~ ✅ 已补充（AgentRuntime 4 个核心测试 + Tauri 命令 6 个测试 + 10 个集成测试）
9. ~~**Chat 流 Proposal 接入**：当前 Chat 对话不生成 LifeModel 更新 Proposal，未来应支持 Chat 中 AI 建议修改 LifeModel 时走 Proposal 确认流。~~ ✅ 已完成（Chat 流程自动调用 ProposalEngine 生成提案）
10. **Execution Tools 继续加固**：file/web/calendar/task/MCP/A2A 已有 P1 路径；后续重点是 provider 覆盖、失败可解释性和 taxonomy 同步，不能把 proposal-only 工具描述成真实外部写入。
11. **MultiStrategy 受控迁移**：当前 MultiStrategy 仅通过 preview/beta command 暴露；后续先做非默认 preview UI/debug entry，再做受控 Chat 子路径迁移，不能直接替换 Chat 主流程。

---

## 🧪 测试策略

### 测试类型

| 类型 | 工具/框架 | 覆盖范围 | 说明 |
|------|-----------|----------|------|
| **前端单元测试** | Vitest + jsdom + @testing-library/react | `src/**/*.test.{ts,tsx}` | 组件渲染、交互逻辑、Tauri mock 调用验证 |
| **前端覆盖率** | Vitest coverage | `src/**/*.{ts,tsx}` | reporter: text/json/html；排除 `src/test/**/*` |
| **Rust 单元测试** | `cargo test` | `openlife-core/src/**`, `src-tauri/src/**` | `#[cfg(test)]` 模块，使用 `tempfile` 做临时目录 |
| **Rust 集成测试** | （需要人工补充） | 端到端 Tauri 命令测试 | 目前缺乏 |
| **端到端测试** | （需要人工补充） | 完整用户流程 | 目前缺乏 |

### 测试数据

- **前端 Mock**：[`frontend/src/test/mocks/tauri.ts`](frontend/src/test/mocks/tauri.ts) 提供完整的 Tauri `invoke` mock，覆盖约 30+ 个命令。新增 command 时**必须同步更新此 mock**，否则组件测试会失败。
- **Rust 测试数据**：使用 `tempfile::TempDir` 创建临时 SQLite 数据库，每个测试独立隔离。
- **LifeModel 测试数据**：YAML 序列化/反序列化测试使用内存中的字符串，无需外部文件。

### 关键测试文件

| 文件 | 测试内容 |
|------|----------|
| [`openlife-core/src/config.rs`](openlife-core/src/config.rs:96) | YAML 保存/加载往返、默认值、环境变量覆盖 |
| [`openlife-core/src/vectors.rs`](openlife-core/src/vectors.rs:369) | 向量插入/批量插入/检索/tier 维护/导入导出/清空 |
| [`openlife-core/src/memory.rs`](openlife-core/src/memory.rs:702) | 消息保存加载、快照、会话管理、状态历史、迁移兼容 |
| [`openlife-core/src/scheduler.rs`](openlife-core/src/scheduler.rs:191) | 本地/云端调度策略逻辑（纯函数测试） |
| [`openlife-core/src/mcp.rs`](openlife-core/src/mcp.rs:536) | 参数隐私检查（低/中风险分级） |
| [`frontend/src/components/ToolCallCard.test.tsx`](frontend/src/components/ToolCallCard.test.tsx) | 工具调用卡片渲染 |
| [`frontend/src/pages/ChatPage.test.tsx`](frontend/src/pages/ChatPage.test.tsx) | 聊天页面交互 |

---

## 📚 参考资料

### 相关文档

| 文档 | 路径 | 说明 |
|------|------|------|
| 文档权威地图 | [`plans/README.md`](plans/README.md) | 解决旧计划/新计划冲突的最高优先级索引 |
| 下一阶段总纲 | [`plans/openlife_lifemodel_governed_agent_runtime.md`](plans/openlife_lifemodel_governed_agent_runtime.md) | LifeModel-Governed Runtime、Maturation Loop、多策略 Runtime 的当前开发顺序 |
| 架构基准 | [`plans/openlife_agent_framework_architecture.md`](plans/openlife_agent_framework_architecture.md) | Agent Framework 架构基准，需与总纲合读 |
| 产品需求 v2 | [`OpenLife_PRD_v2_Agent_Framework.md`](OpenLife_PRD_v2_Agent_Framework.md) | 产品定义与需求基准；不覆盖当前实现顺序 |
| 开发计划 | [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md) | 当前开发路线图，按 LifeModel-Governed Runtime 迁移路线维护 |
| 执行手册 | [`plans/openlife_codex_execution_playbook.md`](plans/openlife_codex_execution_playbook.md) | 单轮任务切分与验证方式 |
| 前端重构计划 | [`plans/frontend_experience_rebuild_plan.md`](plans/frontend_experience_rebuild_plan.md) | 历史前端体验重构计划，后续需按 Agent Workspace 更新 |
| 工程治理笔记 | [`plans/engineering_structure_notes.md`](plans/engineering_structure_notes.md) | 工程拆分和治理记录 |
| 用户文档 | [`README.md`](README.md) | 面向用户的快速开始指南 |
| 历史 PRD | [`OpenLife_Final_PRD.md`](OpenLife_Final_PRD.md) | 旧版完整需求，作为历史参考，不再覆盖新的 Agent Framework 定义 |

### 学习资源

| 资源 | 链接 | 说明 |
|------|------|------|
| Tauri 官方文档 | https://tauri.app | 桌面应用开发框架 |
| React 官方文档 | https://react.dev | 前端 UI 框架 |
| Rust 官方文档 | https://doc.rust-lang.org | 后端核心语言 |
| Tailwind CSS | https://tailwindcss.com | 原子化 CSS |
| MCP 协议规范 | https://modelcontextprotocol.io | 模型上下文协议 |
| A2A 协议规范 | （需要人工补充） | Agent-to-Agent 协议 |
| Ollama API | https://github.com/ollama/ollama/blob/main/docs/api.md | 本地模型服务 |
| OpenRouter API | https://openrouter.ai/docs | 云端模型聚合 API |

---

## 🔄 更新日志

| 日期 | 更新内容 | 作者 |
|------|----------|------|
| 2026-04-20 | 初始版本：完成项目结构、技术栈、启动脚本、环境配置、业务逻辑、测试策略 | AI Agent |
| 2026-04-20 | 新增完整启动脚本集合（setup/dev/start/startup + Makefile） | AI Agent |
| 2026-04-20 | 更新 AGENTS.md 为完整模板格式（命名约定、代码风格、业务规则、已知问题、测试策略） | AI Agent |
| 2026-04-22 | 拆分 src-tauri/src/lib.rs：67+ 命令按领域拆分为 13 个 commands/ 模块，lib.rs 保留共享类型和核心聊天命令 | AI Agent |
| 2026-04-22 | 清理未使用 import，cargo check 零警告零错误；前端 86 测试 + Rust 129 测试全部通过 | AI Agent |
| 2026-04-24 | 将项目上下文从“桌面 AI 伴侣应用”更新为“本地优先个人 Agent 框架”，新增 Agent Runtime、AgentRun、Proposal、ModelRouter 作为后续开发主线 | AI Agent |
| 2026-04-26 | Proposal/Confirmation 统一层收敛完成：Builder 和 Calibration 的 LifeModel 更新默认走 Proposal → Review Center → 用户确认 → Snapshot → Apply 链路；AgentRun ↔ Proposal 双向关联溯源；Safe Mode 限制 Proposal 操作；Review Center 强化（分类/风险筛选/编辑/批量/空状态/失败态/Dashboard 提醒） | AI Agent |
| 2026-04-28 | 推理架构治理：LayeredReasoner 成为 AgentRuntime 的默认推理策略，通过 ReasoningStrategy trait 注册；新增 DirectReasoner 作为备选策略；统一超时配置；SafetyChecker 替代 Arbitrator；更新 AGENTS.md 和架构文档 | AI Agent |
| 2026-04-29 | Stabilization / Spine Consolidation：Builder 默认 Proposal-Only；MemoryWrite/MemoryArchive/ToolPermission Proposal MVP 可应用；Chat Proposal 与 AgentRun.generated_proposals 关联收敛；ModelRouter provider health 与隐私优先路由增强；make ci 增加前端生产构建/typecheck | AI Agent |
| 2026-05-01 | **Beta 开发完成（Gates 0-8）**：reqwest 0.12 升级；AgentLoop 双轨（feature flag）；Action Parser JSON envelope + fail-soft；Core OS Tools + Execution Tools 注册；Permission/Replay 闭环；ModelRouter 毕业；UI 导航收敛；Settings 新增 safe paths / AgentLoop toggle；`make ci` 持续通过 | AI Agent |
| 2026-05-01 | **Week 1: ExternalWriteAction 闭环**：`is_path_in_safe_paths` 公共化；`file.write_proposal` 自动创建 Proposal（含 content_hash/size_bytes/operation）；ExternalWriteAction apply 真实文件写入（safe_paths + 100KB 限制 + 自动创建父目录） | AI Agent |
| 2026-05-08 | **Week 2: Stub 转 Proposal + Task MVP**：calendar.propose_event / email.propose_draft 生成 ScheduledTask/DataExport Proposal；ScheduledTask MVP（本地 JSON）；DataExport MVP（safe_paths 内文件导出） | AI Agent |
| 2026-05-08 | **Week 3: AgentLoop Stream + Fallback**：AgentLoop 句子级分 chunk 流式输出（30ms 间隔）；AgentLoop graceful fallback 到 legacy direct generation；保持默认关闭 | AI Agent |
| 2026-05-08 | **Week 4: 可观察性 + Feature Flag**：stream-message-start 传递真实 ReasoningTrace（AgentLoop actions/observations）；debug 构建默认启用 AgentLoop | AI Agent |
| 2026-05-08 | **Week 5: Workspace + Chat Trace**：Dashboard 新增 Workspace 概览卡片（Pending Proposals / Recent Runs / New Chat）；Chat 最后一条 assistant 消息显示模型/provider/工具数/Proposal 数/fallback 标记 | AI Agent |
| 2026-05-08 | **Week 6: Review Center + 页面收敛**：Chat 顶部 pending proposals banner；主导航从 7 个收敛到 6 个（Memory 移出主导航，保留路由） | AI Agent |
| 2026-05-08 | **Week 7: 测试补齐 + AgentLoop 默认启用**：新增 ExternalWriteAction/ScheduledTask/DataExport apply 测试；AgentLoop 生产环境默认启用 | AI Agent |
| 2026-05-08 | **Week 8: 收口回归**：文档更新；连续 8 周 `make ci` 通过 | AI Agent |
| 2026-05-03 | **Sprint 9: 架构优化**：action_executor.rs 拆分为 8 个模块 (core_os_tools/execution_tools/memory_ops/life_model_ops/declarative_stubs/helpers/tool_executor/mod.rs)；AgentLoop 实现真实 token 级流式输出（StreamingCallback trait + run_streaming + TauriStreamingCallback 适配）；P2 工具升级：calendar.read 从 P2→P1（ICS 文件解析器），task.create_proposal 从 P2→P1（TaskStore + ScheduledTask 本地持久化）；移除 sentence-based 伪流式（split_into_sentences）；config.rs 新增 calendar_ics_paths | AI Agent |
| 2026-05-05 | **Sprint 10: CI修复 + 技术债务**：P0 clippy 修复（AgentLoopContext 重构消除 too_many_arguments、privacy.rs manual_is_multiple_of、dead_code test_app_state）；web.search DuckDuckGo 双正则 fallback + 5s rate limit；AgentLoop 参数配置化（max_steps/tool_calls/timeout 进 SystemConfig）；lib.rs bootstrap 提取到 src-tauri/src/bootstrap.rs（3234→2821 行） | AI Agent |
| 2026-05-05 | **Sprint 11: 执行工具闭环**：a2a.call_agent P2→P1（30s超时+私网拦截+真实 A2AClient 调用）；calendar.propose_event / email.propose_draft 已在 W1 复核为 P1 proposal-only governed executors；ChatProposalGenerator LLM 升级（Ollama 信号提取优先，静默降级关键词匹配） | AI Agent |
| 2026-05-05 | **Sprint 12: Agent 深度能力**：AgentRole（Generalist/Planner）+ role_system_instruction 注入 tools prompt；scheduler_runner.rs（60s 轮询 scheduled_tasks.json + AgentLoop 自动执行）；E2E integration tests（AgentRole config 验证）；LifeModel 字段 GoalItem.updated_at、State.last_updated | AI Agent |
| 2026-05-05 | **Sprint 13: Proactive Agent MVP（Phase 6）**：ProactiveEngine（每日简报、每周复盘、目标陈旧检测、提案提醒、状态签到）；ProactiveConfig 集成 SystemConfig；Tauri 命令 get_proactive_suggestions；record_state 自动更新 last_updated；toolset_allowlist 过滤 AgentLoop 执行 | AI Agent |
| 2026-05-05 | **Sprint 14: Dashboard + 搜索增强**：Dashboard 主动建议卡片前端集成；web.search 多Provider支持（DuckDuckGo 默认/Brave API/SearXNG）；web.fetch 新增 summarize 参数 → Ollama 中文摘要；search_provider 配置入 SystemConfig | AI Agent |
| 2026-05-05 | **Sprint 15: Engineering Consolidation**：AGENTS.md、development_plan.md 文档同步；工具 Taxonomy 表更新（P2→P1 标记校正）；ProviderTab act() 测试警告修复；Email Settings 配置区；前端 ErrorBoundary 完善（重试+错误详情） | AI Agent |
| 2026-05-30 | **W11: LifeModel-Governed Runtime 状态同步**：W1-W10 标记完成；明确 MultiStrategy preview/audit-ready 但非默认 Chat；记录 W10 metadata-safe 外层 AgentRun audit 与下一步受控迁移顺序 | AI Agent |
| 2026-05-30 | **W16: RuntimeStrategy Trait Foundation**：W1-W16 标记完成；MultiStrategy Runtime 通过固定 ReAct / PlanExecute adapter registry 执行；该阶段不允许直接替换默认 Chat | AI Agent |
| 2026-06-03 | **W105: Plan-Execute Product Vertical**：W98-W105 标记完成；新增非默认 weekly planning Plan-Execute session lifecycle、proposal-first step execution、metadata-safe AgentRun trace 和 Workspace/Runs frontend surface；default Chat 仍为 `legacy_stream` | AI Agent |
| 2026-06-03 | **W113: RuntimeStrategy / Multi-Strategy Runtime Maturity**：W106-W113 标记完成；新增 RuntimeStrategy descriptors/readiness、StrategySelector candidate matrix、execution report envelope、非默认只读 registry status command、preview/product shared trace vocabulary 和 declarative-only future strategy taxonomy；default Chat 仍为 `legacy_stream`，status/readiness 不是 migration permission | AI Agent |
| 2026-05-30 | **W17: Runtime integration hardening / Chat migration gate**：新增只读 `check_runtime_migration_gate`，诊断默认 Chat 未替换、preview audit 健康、metadata-safe trace、fallback、无外部写入和 proposal-first 边界；下一步仍不能直接替换默认 Chat | AI Agent |
| 2026-05-30 | **W18: Runtime Migration Gate evidence surface**：Settings experimental 区域新增只读 Runtime Migration Gate 面板，显式展示 gate pass/block 字段与 blocking reasons；普通 Chat 发送路径仍不调用 gate 或 MultiStrategy preview；下一步只能在 gate evidence 连续干净后进入更小范围 controlled Chat migration pilot | AI Agent |
| 2026-05-30 | **W19: Sustained Gate Evidence / Pilot Eligibility**：新增只读 `check_controlled_chat_pilot_eligibility`，默认检查最近 3 条 preview gate report 是否连续干净；Settings 显示 clean run count、checked run ids、blocking reasons；不创建 AgentRun/Proposal/Action/Observation；作为 W20 very small controlled Chat migration pilot 的准入门槛 | AI Agent |
| 2026-05-30 | **W20: Very Small Controlled Chat Migration Pilot With Fallback**：Chat 页面新增显式 `Run Controlled Pilot` 单轮入口；先调用 `check_controlled_chat_pilot_eligibility`，blocked 不调用 preview；eligible 后才调用 `run_multi_strategy_agent_preview` 且 `allowWrites=false`；结果以 “Pilot response” 显示，不自动写普通 assistant message/history；默认 Send 仍未迁移 | AI Agent |
| 2026-05-30 | **W21: Reviewed Pilot Response Promotion**：Chat 页面成功 pilot response 默认继续隔离；只有成功且包含 `userOutput` 时显示 `Promote Pilot Response`，用户 review/confirm 后才通过现有 chat message save path 写入一条 assistant message，并保留可用 `run_id` trace；cancel/blocked/failed/no-output/repeat 不写入；默认 Send 仍未迁移 | AI Agent |
| 2026-05-30 | **W22: Post-Promotion Validation And Source Binding**：Controlled Pilot 结果绑定 source session；promotion review 展示 source session、target session、runId、strategy、governance summary；确认前校验 source/target 一致，session mismatch 不调用 `save_chat_message`，显示 blocking/fallback 和重新运行 pilot 提示；默认 Send 仍未迁移 | AI Agent |
| 2026-05-30 | **W23: Controlled Pilot Promotion Evidence Recorder**：promotion confirm 成功保存 assistant message 后写入 metadata-safe EvidenceStore evidence，包含 pilotRunId、source/target session、strategy/payload/governance、message length、checksum、promotedAt；Settings experimental 只读展示 promotion evidence summary；evidence 失败显示 degraded/error 且重试不重复写 chat message；默认 Send 仍未迁移 | AI Agent |
| 2026-05-30 | **W24: Promotion Evidence Readiness Gate**：新增只读 `check_controlled_pilot_promotion_readiness`，默认要求 3 条 metadata-safe promotion evidence，输出 ready/counts/recent run ids/latest timestamp/mismatch block count/metadataSafeEvidenceReady/defaultChatUnchanged/blockingReasons；Settings experimental 增加 readiness panel；`sessionId` 当前按 EvidenceStore global summary 说明；ready 仅表示可讨论下一阶段 Chat migration，默认 Send 仍不调用该 gate 且未迁移 | AI Agent |
| 2026-05-31 | **W25: Reviewed Migration Plan Draft Generator**：新增只读 `draft_controlled_chat_migration_plan`，复用 W24 readiness gate；blocked 返回 `draftReady=false` 和 blockers，不生成可执行 plan sections；passed 返回人工评审 scope/preconditions/rollback/fallback/test plan，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`；Settings experimental 增加 Draft Migration Plan 面板；默认 Send 仍不调用该 command 且未迁移 | AI Agent |
| 2026-05-31 | **W26: Manual Migration Review Decision Evidence**：新增 `record_controlled_chat_migration_review_decision` 和只读 `get_controlled_chat_migration_review_decision_summary`；record 先调用 W25 draft，blocked draft approve 不写 evidence，ready draft 可记录 approve/reject/request_rework metadata-safe evidence，reviewer note 仅保存 length/checksum/category；Settings experimental 增加 Migration Review Decision 面板；默认 Send 仍不调用这些 command 且未迁移 | AI Agent |
| 2026-05-31 | **W27: Approved Migration Implementation Gate**：新增只读 `check_controlled_chat_migration_implementation_gate`，要求 latest approve、当前 readiness pass、draft hash match；eligible 只允许 implementation discussion，不切换 default Chat | AI Agent |
| 2026-05-31 | **W28: Non-Default Controlled Migration Shadow Run**：新增显式 `run_controlled_chat_migration_shadow_run`，先查 W27 gate，eligible 后才运行 write-disabled bounded shadow runtime；只返回 metadata-safe summary，不写 Chat/Proposal/Memory/LifeModel/Evidence/外部工具结果 | AI Agent |
| 2026-05-31 | **W29: Controlled Chat Migration Shadow Review Evidence**：新增 shadow review decision evidence；approve/reject/request_rework 只保存 metadata-safe whitelisted fields，默认 Send 不调用 shadow review command | AI Agent |
| 2026-05-31 | **W30: Controlled Chat Cutover Planning Readiness Gate**：新增只读 `check_controlled_chat_cutover_readiness`，验证 W27 eligible、latest W29 approve 和 approved shadow run 当前仍 write-disabled/metadata-safe/side-effect-free；pass 不是默认 Chat migration | AI Agent |
| 2026-05-31 | **W31: Non-Default Controlled Chat Cutover Candidate Adapter**：新增显式 `run_controlled_chat_cutover_candidate`，先查 W30 readiness，blocked 不运行 runtime；eligible 后才运行一次 `allowWrites=false`、`maxToolCalls=0` candidate，返回 Chat-compatible contract shape 和 metadata-safe summary；不保存到 Chat、不 promotion、不改默认路径 | AI Agent |
| 2026-05-31 | **W32: Cutover Candidate Review Evidence**：新增 `record_controlled_chat_cutover_candidate_review_decision` 和只读 summary；approve 绑定 completed/ready/send_message-compatible/write-disabled/zero-tool/metadata-safe/side-effect-free candidate AgentRun；evidence 只保存白名单 metadata，不保存 reviewer 原文、candidate output、raw prompt/output 或 tool payload；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W33: Cutover Candidate Promotion Readiness Gate**：新增只读 `check_controlled_chat_cutover_candidate_promotion_readiness`，复用 W30 readiness 并读取 W32 metadata-safe approval evidence；要求 latest decision 为 approve、approved candidate AgentRun 当前仍 send_message-compatible/write-disabled/zero-tool/metadata-safe/side-effect-free；只返回 ready/blockers/counts/latest decision/defaultChatUnchanged/metadata-safe summary，不创建记录、不运行 runtime/tool/model call；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W34: Default Chat Runtime Boundary Status**：新增只读 `get_default_chat_runtime_boundary_status`，固定返回 `currentMode=legacy_stream`、`defaultChatUnchanged=true`、`automaticMigrationEnabled=false`、`controlledCandidateAvailable=false`、`candidatePromotionReadinessRequired=true`；只用于观察默认 Chat boundary，不调用 W19-W33 gates，不创建记录、不运行 runtime/tool/model call；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W35: Default Chat Adapter Activation Plan Draft**：新增只读 `draft_default_chat_adapter_activation_plan`，组合 W33 readiness 与 W34 boundary status；blocked 时不生成 plan sections，ready 时只返回 human-review activation plan sections，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`、`requiresSeparateImplementation=true`；不创建记录、不运行 runtime/tool/model call、不切换 feature flag；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W36: Default Chat Adapter Activation Review Decision Evidence**：新增 `record_default_chat_adapter_activation_review_decision` 和只读 summary；record 先调用 W35 draft，blocked draft approve 不写 evidence，ready draft 只记录 metadata-safe approve/reject/request_rework evidence；reviewer note 仅保存 checksum/length/category；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W37: Default Chat Adapter Activation Implementation Gate**：新增只读 `check_default_chat_adapter_activation_implementation_gate`，组合当前 W35 stable activation plan digest 与 W36 latest metadata-safe review decision evidence；latest approve、draft ready、digest match、candidate promotion ready、default Chat legacy stream 且 automatic migration disabled 时才 eligible；不创建记录、不运行 runtime/tool/model call；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W38: Default Chat Adapter Disabled Routing Scaffold**：新增只读 `get_default_chat_adapter_routing_status`，调用 W37 gate 并固定报告 `currentMode=legacy_stream`、`adapterScaffoldPresent=true`、`controlledAdapterEnabled=false`、`defaultSendPath=legacy_stream`、`startStreamPath=legacy_stream`；不创建记录、不运行 runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W39: Default Chat Adapter Contract Harness**：新增只读 `check_default_chat_adapter_contract_harness`，调用 W38 routing status 并验证 send_message / start_stream_message contract 均保持 `legacy_stream`、controlled adapter disabled、activation implementation gate eligible；不创建记录、不运行 runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W40: Default Chat Adapter Dry-Run Invocation Boundary**：新增显式非默认 `run_default_chat_adapter_dry_run`，先检查 W39 contract harness，ready 时只返回 metadata-safe dry-run contract result，强制 `allowWrites=false`、`maxToolCalls=0`、`defaultChatPathUnchanged=true`；不保存 Chat、不创建 AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit/external write、不运行 runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W41: Default Chat Adapter Dry-Run Review Evidence**：新增 `record_default_chat_adapter_dry_run_review_decision` 和只读 summary；record 先重新运行 W40 dry run，approve 只在 dry run ready 时写 metadata-safe evidence，blocked approve 不写 evidence，reject/request_rework 只写白名单 metadata；reviewer note 仅保存 checksum/length/category；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W42: Default Chat Adapter Implementation Readiness Gate**：新增只读 `check_default_chat_adapter_implementation_readiness`，组合 W37/W39/W40/W41 当前证据；要求 latest dry-run review approve、dry-run digest match、default Chat unchanged、controlled adapter disabled、automatic migration disabled、send/stream 保持 `legacy_stream`；不创建记录、不运行 runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W43: Default Chat Adapter Controlled Preview**：新增显式非默认 `run_default_chat_adapter_controlled_preview`，先查 W42 implementation readiness；blocked 不运行 runtime/不创建 AgentRun；ready 后运行 `allowWrites=false`、`maxToolCalls=0` controlled preview，返回 SendMessageResult-compatible fields，只允许 metadata-safe adapter preview AgentRun audit；默认 Send 仍未迁移 | AI Agent |
| 2026-05-31 | **W44: Default Chat Adapter Controlled Preview Review Evidence**：新增 `record_default_chat_adapter_controlled_preview_review_decision` 和只读 summary；approve 绑定 completed/ready/send-compatible/write-disabled/zero-tool/metadata-safe/side-effect-free W43 preview AgentRun；evidence 只保存白名单 metadata，reviewer note 仅 checksum/length/category；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W45: Default Chat Adapter Controlled Preview Approval Readiness Gate**：新增只读 `check_default_chat_adapter_controlled_preview_approval_readiness`，组合 W42 implementation readiness、W44 latest approve evidence、required approved preview count、digest match 与 approved W43 preview AgentRun 当前 completed/send-compatible/previewReady/write-disabled/zero-tool/metadata-safe/side-effect-free 状态；不创建记录、不运行 controlled preview/runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W46: Default Chat Adapter Cutover Implementation Plan Draft**：新增只读 `draft_default_chat_adapter_cutover_implementation_plan`，只调用 W45 readiness；blocked 时不生成 plan sections，ready 时返回 metadata-safe human-review implementation scope、adapter contract requirements、routing boundary、safety preconditions、fallback、rollback、observability、test plan、explicit non-goals 和 stable plan digest；不创建记录、不运行 controlled preview/runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W47: Default Chat Adapter Cutover Plan Review Decision Evidence**：新增 `record_default_chat_adapter_cutover_plan_review_decision` 和只读 `get_default_chat_adapter_cutover_plan_review_summary`；record 先调用 W46 draft，blocked draft approve 不写 evidence，reject/request_rework 可写 metadata-safe evidence；字段限于 decision/source session/draftReady/W45 readiness/cutover plan digest/section count/reviewer-note checksum-length-category/createdAt；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W48: Default Chat Adapter Cutover Plan Approval Readiness Gate**：新增只读 `check_default_chat_adapter_cutover_plan_approval_readiness`；组合当前 W46 draft、W47 latest review evidence、plan digest match、W45 readiness 与 default Chat isolation；ready 只表示后续 adapter implementation discussion，不创建记录、不运行 runtime/tool/model call、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W49: Default Chat Adapter Cutover Route Guard Scaffold**：新增共享 `default_chat_adapter` route resolver 与 fail-closed guard；routing status、`send_message`、`start_stream_message` 使用同一 source-of-truth，默认仍为 `legacy_stream` 且 controlled adapter / automatic migration disabled；路径漂移或 adapter 误启用时默认 Chat 阻断而非静默切换；不调用 W19-W48 gates，不迁移默认 Send | AI Agent |
| 2026-06-01 | **W50: Default Chat Adapter Cutover Invocation Harness**：新增纯后端 `DefaultChatAdapterCutoverHarness`、`evaluate_default_chat_adapter_cutover_harness` 与 `ensure_default_chat_cutover_harness`；默认 `send_message` / `start_stream_message` 只通过该 harness guard 保持 `legacy_guarded`、write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write 边界；route drift、scaffold 缺失、controlled adapter / automatic migration 误启用或 separate implementation 约束消失时 fail closed；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W51: Default Chat Adapter Invocation Plan**：新增纯后端 `DefaultChatAdapterInvocationPlan`、`plan_default_chat_adapter_invocation` 与 `ensure_default_chat_adapter_invocation_plan`；默认 `send_message` / `start_stream_message` 现在通过 invocation plan guard 选择 `legacy_stream`，保留 `controlled_adapter` 为 disabled candidate，controlled executor unattached，并保持 write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write；W50 harness blocking 会让 W51 plan blocking；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W52: Default Chat Adapter Invocation Boundary**：新增纯后端 `DefaultChatAdapterInvocationBoundary`、`evaluate_default_chat_adapter_invocation_boundary` 与 `ensure_default_chat_adapter_invocation_boundary`；默认 `send_message` / `start_stream_message` 现在通过 boundary guard 复用 W51 plan，只允许进入 `legacy_stream` callsite，并要求 legacy entry 前 write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write；W51 plan blocking 会让 W52 boundary blocking；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W53: Default Chat Adapter Typed Callsite Contract**：新增纯后端 `DefaultChatAdapterCallsite`、`DefaultChatAdapterCallsiteContract`、`evaluate_default_chat_adapter_callsite_contract` 与 `ensure_default_chat_adapter_callsite_contract`；默认 `send_message` / `start_stream_message` 现在通过 typed callsite contract guard 绑定 send/stream contract shape 和各自 legacy route path；W52 boundary blocking 或 callsite route drift 会 fail closed；默认 Send 仍未迁移 | AI Agent |
| 2026-06-01 | **W54: Authority Roadmap Sync**：同步 `AGENTS.md`、`README.md`、`plans/README.md`、`plans/openlife_lifemodel_governed_agent_runtime.md`、`plans/openlife_development_plan.md` 与 progress index，将高优先级路线从旧 W22 状态对齐到 W54/W1-W53 当前代码状态；不修改 runtime code；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W55: Default Chat Adapter Ordinary Entry Preflight**：新增纯后端 `DefaultChatAdapterOrdinaryEntryPreflight`、`evaluate_default_chat_adapter_ordinary_entry_preflight` 与 `ensure_default_chat_adapter_ordinary_entry_preflight`；默认 `send_message` / `start_stream_message` 进入 legacy stream 前必须满足 typed contract ready、legacy entry allowed、controlled executor unattached、migration disabled 和零 runtime/model/tool/write 预算；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W56: Default Chat Adapter Ordinary Entry Preflight Status**：新增只读 `get_default_chat_adapter_ordinary_entry_preflight_status`、frontend wrapper 和 Settings evidence surface；只展示 W55 send/stream preflight readiness、route state、blockers、side-effect lock 和 metadata-safe summary；不运行 runtime/model/tool、不写记录、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W57: Default Chat Adapter Narrow Implementation Discussion Gate**：新增只读 `check_default_chat_adapter_narrow_implementation_discussion_gate`、frontend wrapper 和 Settings evidence surface；组合 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status，eligible 只表示可讨论更窄 adapter implementation slice；不运行 runtime/model/tool、不写记录、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W58: Default Chat Adapter Narrow Implementation Plan Draft**：新增只读 `draft_default_chat_adapter_narrow_implementation_plan`、frontend wrapper 和 Settings evidence surface；先调用 W57 discussion gate，blocked 时不生成 plan sections，eligible 时只返回 metadata-safe human-review plan sections 与 stable digest；不创建记录、不运行 runtime/model/tool/preview、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W59: Default Chat Adapter Narrow Implementation Plan Review Evidence**：新增 `record_default_chat_adapter_narrow_implementation_plan_review_decision` 与只读 summary、frontend wrapper 和 Settings evidence surface；先调用 W58 draft，blocked draft approve 不写 evidence，ready draft decision 只写 metadata-safe Evidence；reviewer note 仅 checksum/length/category；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W60: Default Chat Adapter Narrow Implementation Plan Approval Readiness Gate**：新增只读 `check_default_chat_adapter_narrow_implementation_plan_approval_readiness`、frontend wrapper 和 Settings evidence surface；组合当前 W58 draft、W59 latest approve evidence、digest match、W57 eligible 与 default Chat isolation；不写记录、不运行 runtime/model/tool/preview、不切换 routing；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W61: Progress Index Compression Prep**：docs-only 整理 W1-W60 长路线状态，准备结构化索引字段；不改 Rust/TS、不新增 command、不影响 default Chat | AI Agent |
| 2026-06-02 | **W62: Plans README Authority Compression**：docs-only 压缩 `plans/README.md` 权威入口和阶段分组，保留 legacy_stream / ordinary-entry / not migration permission 硬约束；默认 Send 仍未迁移 | AI Agent |
| 2026-06-02 | **W63: Narrow Adapter Implementation Entry Index Freeze**：docs-only 冻结 W1-W63 结构化索引入口，下一步仅为未来 W64 narrow adapter implementation slice 做上下文准备；W63 不授权 W64，不是 default Chat migration | AI Agent |
| 2026-06-02 | **W64: W1-W63 Authority Compression Validation**：docs-only 验收压缩入口与结构化索引；不改 runtime/model/tool，不写记录，不影响 default Chat | AI Agent |
| 2026-06-02 | **W65: Default Chat Adapter Backend-Only Descriptor Skeleton**：新增纯后端 metadata-safe descriptor / mapper，只记录 input length/hash 与 route metadata；controlled executor disabled/unattached、zero side-effect budget、migrationPermission=false；不新增 command/surface、不运行 runtime/model/tool、不写业务记录、不切 routing，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W66: Default Chat Adapter Controlled Contract Report**：新增纯后端 controlled adapter contract report / evaluator / ensure，复用 W65 descriptor 验证 send/stream contract shape；controlled adapter invocation disabled、executor disabled/unattached、zero side-effect budget、migrationPermission=false；不新增 command/surface、不接 executor、不影响 ordinary Chat | AI Agent |
| 2026-06-02 | **W67: Default Chat Adapter Non-Default Controlled Invocation Harness**：新增纯后端 `DefaultChatControlledAdapterInvocationHarness`、`evaluate_default_chat_controlled_adapter_invocation_harness` 与 `ensure_default_chat_controlled_adapter_invocation_harness`，只读取/复用 W66 contract report，证明 future controlled adapter candidate 的 non-default invocation shape metadata-safe、zero-side-effect、executor disabled/unattached；`harness_ready` 不是 migration permission，不新增 command/surface、不接 executor、不运行 runtime/model/tool、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 harness，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W68: Default Chat Adapter Send-Compatible Contract Proof**：新增纯后端 `DefaultChatControlledAdapterSendCompatibleProof`、`evaluate_default_chat_controlled_adapter_send_compatible_proof` 与 `ensure_default_chat_controlled_adapter_send_compatible_proof`，只复用 W65 descriptor、W66 contract 和 W67 harness，证明 controlled adapter candidate 可映射为 SendMessageResult-compatible metadata-safe shape；仅 `SendMessage` callsite 可 ready，stream callsite fail closed；`proof_ready` 不是 migration permission，不新增 command/surface、不接 executor、不运行 runtime/model/tool、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 proof，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W69: Default Chat Adapter Stream-Compatible Boundary Proof**：新增纯后端 `DefaultChatControlledAdapterStreamBoundaryProof`、`evaluate_default_chat_controlled_adapter_stream_boundary_proof` 与 `ensure_default_chat_controlled_adapter_stream_boundary_proof`，只复用 W65 descriptor、W66 contract 和 W67 harness，证明 controlled adapter candidate 可形成 `start_stream_message`-compatible metadata boundary；仅 `StartStreamMessage` callsite 可 ready，`SendMessage` fail closed；streamStarted/eventChannelOpened/streamEventsEmitted=false，migrationPermission=false，不新增 command/surface、不接 executor、不 emit real stream、不运行 runtime/model/tool、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 proof，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W70: Default Chat Adapter Controlled Executor Attachment Gate Report**：新增纯后端 `DefaultChatControlledAdapterExecutorAttachmentGateReport`、`evaluate_default_chat_controlled_adapter_executor_attachment_gate` 与 `ensure_default_chat_controlled_adapter_executor_attachment_gate`，同时复用 W65-W67 metadata-safe descriptor/contract/harness、W68 send proof 和 W69 stream boundary proof，汇总是否足以进入下一步 executor skeleton 讨论；固定 executor_attachment_allowed=false、executor_attached=false、executor_enabled=false、route_cutover_permission=false、migrationPermission=false；executor implementation missing / human review missing / route cutover not authorized 为明确 blockers；不新增 command/surface、不接真实 executor、不运行 runtime/model/tool、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 gate，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W71: Default Chat Adapter Disabled Controlled Executor Skeleton Contract**：新增纯后端 `DefaultChatControlledAdapterDisabledExecutorSkeleton`、`DefaultChatControlledAdapterExecutorSkeletonInput`、`DefaultChatControlledAdapterExecutorSkeletonOutput`、`evaluate_default_chat_controlled_adapter_disabled_executor_skeleton` 与 `ensure_default_chat_controlled_adapter_disabled_executor_skeleton`，只复用 W70 gate report 和 metadata-safe input，定义 future controlled executor 的 disabled/unattached/no-run 输入/输出/预算/fail-closed 形态；固定 executor_skeleton_present=true、executor_enabled=false、executor_attached=false、executor_runnable=false、invocation_allowed=false、route_cutover_permission=false、migrationPermission=false；send/stream shape 仅返回 metadata-only placeholder，不新增 command/surface、不接 executor、不运行 runtime/model/tool、不 emit stream、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 skeleton，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W72: Default Chat Adapter Disabled Executor Skeleton Binding Integrity Report**：新增纯后端 `DefaultChatControlledAdapterSkeletonBindingIntegrityReport`、`evaluate_default_chat_controlled_adapter_skeleton_binding_integrity` 与 `ensure_default_chat_controlled_adapter_skeleton_binding_integrity`，复用 W71 skeleton、W71 skeleton input 和 W70 gate report，校验 input length/hash、route metadata、requested shape/callsite、skeleton output shape、legacy route、gate metadata 和 disabled/no-run/no-write/no-stream 约束一致；固定 executor_enabled=false、executor_attached=false、executor_runnable=false、invocation_allowed=false、route_cutover_permission=false、migrationPermission=false、selected_adapter_path=legacy_stream；不新增 command/surface、不接 executor、不运行 runtime/model/tool、不 emit stream、不写业务记录、不切 routing，ordinary `send_message` / `start_stream_message` 不调用该 binding report，default Chat 仍是 `legacy_stream` | AI Agent |
| 2026-06-02 | **W73: LifeModel Maturation End-to-End Readiness Report**：新增纯 core `LifeModelMaturationReadinessInput`、`LifeModelMaturationReadinessReport`、`evaluate_lifemodel_maturation_readiness` 与 `ensure_lifemodel_maturation_readiness`；只验证低能量/低压力规划偏好 LifeEventDraft 是否 metadata-safe、proposal-first、source-lineage-ready、default Chat unchanged、ordinary Chat unchanged、direct LifeModel/Memory/Heuristic writes disabled、side-effect budget zero，并返回 `nextAllowedStep=non_default_maturation_invocation`；不新增 command/surface、不运行 runtime/model/tool、不写 Evidence/Proposal/LifeModel/Memory/Heuristic/Chat/MCP audit/external write，ordinary `send_message` / `start_stream_message` 不调用该 readiness report | AI Agent |
| 2026-06-02 | **W74: Non-Default LifeModel Maturation Invocation**：新增纯 core `LifeModelMaturationNonDefaultInvocationInput`、`LifeModelMaturationNonDefaultInvocationReport`、`run_lifemodel_maturation_non_default_invocation` 与 `ensure_lifemodel_maturation_non_default_invocation`；显式 non-default invocation 必须先调用 W73 readiness，blocked 时不写任何 store，ready 时只允许写 EvidenceStore + pending ProposalStore；report 固定 no runtime/model/tool、no LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external write、metadata-safe；不新增 command/surface、不改变 default Chat，ordinary `send_message` / `start_stream_message` 不调用 W73/W74 maturation API | AI Agent |
| 2026-06-02 | **W75: LifeModel Maturation Proposal Outcome Evidence Link**：新增纯 core `MaturationProposalOutcome`、`MaturationProposalOutcomeEvidenceReport`、`evaluate_maturation_proposal_outcome_evidence` 与 `record_maturation_proposal_outcome_evidence`；proposal accept/reject/edit 成功处理后只对 maturation lineage proposal 写 metadata-safe `ProposalOutcome` evidence，reject 记录 negative/opposing，edit 不泄露 raw edited payload；不新增 command/frontend、不运行 runtime/model/tool、不改变 default Chat | AI Agent |
| 2026-06-02 | **W76: Low-Energy Collaboration Rule Candidate**：新增纯 core `LowEnergyCollaborationRuleCandidateInput`、`LowEnergyCollaborationRuleCandidateReport`、`evaluate_low_energy_collaboration_rule_candidate` 与 `propose_low_energy_collaboration_rule_candidate`；聚合 accepted/edited/rejected ProposalOutcome evidence，保留 outcome evidence/source evidence/proposal/agent run lineage，opposing evidence 阻止或弱化重复候选；ready 时只写 pending ProposalStore candidate proposal，不激活 Heuristic、不写 active rule、不新增 command/frontend、不运行 runtime/model/tool、不改变 default Chat | AI Agent |
| 2026-06-02 | **W77: Accepted Rule To RuntimeHSPacket Selection Proof**：新增纯 core `AcceptedLowEnergyRuleSelectionInput`、`AcceptedLowEnergyRuleSelectionReport`、`AcceptedLowEnergyRuleSelectionHSPacketAuditProof`、`evaluate_accepted_low_energy_rule_selection` 与 `ensure_accepted_low_energy_rule_selection`；只选择用户已接受的 W76 candidate proposal 进入 future RuntimeHSPacket metadata-safe planning guidance proof，保留 outcome evidence/proposal/agent run lineage，pending/rejected/non-W76、非 planning task、非 low-energy domain fail closed，local-only privacy policy 保持或强化；不新增 command/frontend、不运行 runtime/model/tool、不写 LifeModel/Memory/Heuristic、不激活 Heuristic、不改变 default Chat | AI Agent |
| 2026-06-03 | **W78: LifeModel Maturation Run Trace Visibility Proof**：新增纯 core `LowEnergyRuleTraceVisibilityInput`、`LowEnergyRuleTraceVisibilityReport`、`LowEnergyRuleTraceMetadata`、`evaluate_low_energy_rule_trace_visibility` 与 `ensure_low_energy_rule_trace_visibility`；只证明 W77 selected guidance 可被 future runtime/run trace 以 metadata-safe metadata 展示，保留 selected guidance summary/hash、candidate proposal id/hash、rule digest、evidence/proposal/agent run lineage id/hash/count/status/type、policy route proof；blocked/non-selected W77、raw trace payload、policy 放宽、default Chat cutover/runtime/model/tool/heuristic activation 暗示均 fail closed；不新增 command/frontend、不运行 runtime/model/tool、不写 AgentRun/LifeModel/Memory/Heuristic、不改变 default Chat | AI Agent |
| 2026-06-03 | **W79: Legacy Direct-Write Convergence Inventory Guard**：新增内部 Rust `src-tauri/src/legacy_write_convergence.rs`，定义 `LegacyWriteRiskClass`、`LegacyWriteConvergenceStatus`、`LegacyWritePathKind`、`LegacyWriteInventoryEntry`、`LegacyWriteConvergenceReport`、`legacy_write_convergence_inventory`、`evaluate_legacy_write_convergence_inventory` 与 `ensure_legacy_write_convergence_inventory_guard`；覆盖 LifeModel save/manual editor/Builder/Calibration/Feedback/restore/import/state/raw source/proposal apply/external proposal paths，report metadata-safe 且 raw-content-free，明确 high-risk direct writes 仍是 blockers、proposal-first paths 不是 unsafe blockers、calendar/email propose tools 不是真实 provider executor；不新增 command/frontend、不改变 default Chat、不收口 direct-write path | AI Agent |
| 2026-06-03 | **W80: Manual LifeModel Editor Explicit Override Audit Guard**：在 `src-tauri/src/commands/life_model.rs` 新增内部 `ManualLifeModelOverrideAuditReport`、`evaluate_manual_lifemodel_override_audit` 与 `record_manual_lifemodel_override_audit_with_state`；`save_life_model_with_state` 成功保存后写 metadata-safe `manual_lifemodel_override_audit` analytics event，仅含 source、before/after hash、rough changed section names/count、risk class、timestamp、command/function name、manualOverride/proposalFirst/stillLegacyDirectWrite flags；不写 raw LifeModel/identity/goals/relationships/health/privacy 内容，不创建 Proposal/AgentRun/Heuristic/Patch，不运行 runtime/model/tool，不改变 default Chat；W79 inventory 更新为 manual editor guard present 但仍是 high-risk legacy direct-write blocker，`overall_converged=false` / `all_direct_writes_converged=false` | AI Agent |
| 2026-06-03 | **W81: Builder Legacy Direct Apply Dev-Gate / No-Signal Completion Guard**：在 `src-tauri/src/commands/builder.rs` 新增 `BuilderLegacyDirectApplyOverride` 和 fail-closed legacy direct apply gate；`builder_apply_signals` 默认拒绝，只有显式 dev/migration override 才进入旧直写路径；旧直写响应移除 raw model/run/snapshot/feedback audit 输出，仅返回 metadata-safe applied path summary/counts/warnings；`builder_step_with_state` 的 no-signal completion 分支不再持久化 draft model，仅移除 session 并返回 `durable_lifemodel_write=false` / `completion_cleanup=session_only`；normal Builder flow 继续使用 `builder_create_proposals`，W79 inventory 标记 Builder legacy guard present 但仍是 high-risk blocker，不改变 default Chat | AI Agent |
| 2026-06-03 | **W82: Calibration Direct Apply Legacy Gate / Proposal-First Default**：在 `src-tauri/src/commands/calibration.rs` 新增 `CalibrationLegacyDirectApplyDevMigrationOverride` 和 fail-closed Calibration legacy direct apply gate；`apply_calibration(mode="direct")` 与 `run_micro_evolution` 默认拒绝，只有显式 dev/migration override 才进入旧直写路径；legacy 响应不返回 raw LifeModel、raw calibration change/reason 或 raw evolution payload，仅返回 metadata-safe count/snapshot/warning/signal-count 信息；normal Calibration/`DashboardPage` flow 继续通过 `calibration_create_proposals` / proposal mode 写 ProposalStore；legacy inventory 标记 Calibration proposal flow 为 proposal-first target，同时保留 Calibration direct/evolution 为 high-risk blocker，不改变 default Chat | AI Agent |
| 2026-06-03 | **W83: Feedback Evolution Legacy Direct Apply Gate / Proposal-First Candidate Path**：在 `src-tauri/src/commands/feedback.rs` 新增 `FeedbackEvolutionLegacyDirectApplyOverride` 和 fail-closed Feedback evolution legacy direct apply gate；`apply_feedback_evolution` 默认拒绝，只有显式 dev/migration override 才进入旧直写路径；legacy 响应不返回 raw feedback、conversation inference、LifeModel 或 evolution rule payload，仅返回 metadata-safe count/status/warning 信息；`generate_evolution_report` 改为 read-only metadata-safe report，不写 LifeModel / `evolution_rules` truth；设置页文案改为只读候选报告；legacy inventory 将 Feedback signals 标为 low-risk source data，将 read-only report 从 direct-write blocker 中拆出，同时保留 Feedback evolution direct apply override capability 为 high-risk blocker，不改变 default Chat | AI Agent |
| 2026-06-03 | **W84: Snapshot Restore / Data Import Legacy Direct Write Gate**：在 `src-tauri/src/commands/version.rs` 新增 `SnapshotRestoreLegacyDirectApplyOverride`，在 `src-tauri/src/commands/settings.rs` 新增 `DataImportLegacyDirectApplyOverride`；`restore_snapshot` 与 `import_all_data` 默认 fail closed，只有显式 dev/migration/manual restore override 才进入旧直写路径；restore 响应仅返回 metadata-safe snapshot id/status，import 响应仅返回 metadata-safe count/status，不返回 raw LifeModel、raw memory/vector、raw imported payload 或 snapshot YAML；`export_all_data` / snapshot list/diff/create 等 read-only/materialized 路径不被误伤；legacy inventory 标记 W84 guard present 但 restore/import 仍是 high-risk blocker，不改变 default Chat | AI Agent |
| 2026-06-03 | **W85: State / Daily Goal Source Data Boundary Proof**：在 `src-tauri/src/legacy_write_convergence.rs` 新增内部 `StateSourceDataBoundaryReport`、`evaluate_state_source_data_boundary` 与 `ensure_state_source_data_boundary`；只证明 `state_daily_goal_direct_writes` 是 low-risk transient/source-data compatibility write，且当前通过 `persist_life_model` 写 LifeModel compatibility view / YAML，但不是 accepted durable LifeModel-HS truth；report 仅含 path ids、source-data/low-risk classification、compatibility_lifemodel_materialized_write=true、writes_current_lifemodel_compatibility_view=true、accepted_durable_hs_truth_write=false、active_hs_lifemodel_patch=false、proposal_required_for_hs_truth_promotion=true、ordinary/default Chat unchanged 和 blocker codes；不新增 command/frontend，不改 State/Daily Goal 产品行为，不创建 Proposal/Evidence/AgentRun，不运行 runtime/model/tool，不把 State/Daily Goal 标为 proposal-first converted 或 fully converged，不改变 default Chat | AI Agent |
| 2026-06-03 | **W86: LifeModel Compatibility Materializer Caller Matrix**：在 `src-tauri/src/legacy_write_convergence.rs` 新增内部 `LifeModelMaterializerCallerKind/Risk/GovernanceState`、`LifeModelMaterializerCallerMatrixEntry/Report`、`lifemodel_materializer_caller_matrix`、`evaluate_lifemodel_materializer_caller_matrix` 与 `ensure_lifemodel_materializer_caller_matrix`；matrix 覆盖 16 个生产 `persist_life_model` callsite 和 3 个 production `LifeModelManager::save` 相关入口，分类 materializer root、ordinary Chat daily-goal auto-checkin source-data compatibility、manual override、State/Daily Goal source-data compatibility、accepted proposal apply、Builder/Calibration/Feedback legacy dev-migration override、Snapshot restore/Data import gated override；report 固定 migration_permission=false、runtime_authority_granted=false、proposal_first_convergence_complete=false，metadata-safe 且不含 raw LifeModel/memory/chat/daily-goal payload；不新增 command/frontend，不改 default Chat，不修改 `persist_life_model` 签名，不退休 legacy path，只为 W87 caller restriction 做准备 | AI Agent |
| 2026-06-03 | **W87: LifeModel Materializer Caller Restriction**：在 `src-tauri/src/legacy_write_convergence.rs` 新增内部 `LifeModelMaterializerCallerPurpose`、`LifeModelMaterializerCallerContext`、`LifeModelMaterializerCallerRestrictionReport`、`evaluate_lifemodel_materializer_caller_restriction`、`ensure_lifemodel_materializer_caller_allowed` 与 `ensure_lifemodel_materializer_caller_restriction`；`persist_life_model` 签名现在要求 typed caller context，16 个生产 callsite 均显式传入对应 W86 stable_id/kind/purpose；`restore_snapshot_direct_apply_after_gate` 在 direct `LifeModelManager::save(&restored_model)` 前增加 `snapshot_restore_legacy_direct_apply` guard；unknown 或 kind/purpose mismatched caller fail closed；不改 default Chat routing，不新增 command/frontend/Settings，不运行 runtime/model/tool，不写 Chat/AgentRun/Evidence/Proposal/Memory/MCP audit/external records，不退休 legacy path，不授予 migration/runtime authority；当时仍未完成 source-specific proposal patch mapping，后续由 W88/W89 继续 | AI Agent |
| 2026-06-03 | **W88: Proposal Application Source-Specific Patch Mapping**：在 `src-tauri/src/commands/proposal.rs` 新增私有 `LifeModelProposalPatchSourceMappingReport`、`evaluate_lifemodel_proposal_patch_source_mapping`、`ensure_lifemodel_proposal_patch_source_mapping` 与 `resolve_lifemodel_patch_source_for_proposal`；accepted LifeModel proposal apply 不再硬编码 `PatchSource::BuilderReview`，而是映射 BuilderReview→BuilderReview、CalibrationRun→Calibration、FeedbackEvolution→Evolution、Manual→Manual；ChatConversation/ProactiveAgent/SkillRuntime/Plugin/MemoryGovernance 在缺少专用 PatchSource variant 时使用 metadata-safe Manual fallback 并报告 W89 follow-up/blocker；不新增 command/frontend/Settings，不运行 runtime/model/tool，不改 default Chat，不退休 legacy path，`proposal_first_convergence_complete=false` 仍等 W89 audit/readiness | AI Agent |
| 2026-06-03 | **W89: Proposal Application Source-Specific Patch Audit / Readiness**：在 `src-tauri/src/commands/proposal.rs` 新增私有 `LifeModelProposalPatchSourceReadinessEntry`、`LifeModelProposalPatchSourceReadinessReport`、`evaluate_lifemodel_proposal_patch_source_readiness` 与 `ensure_lifemodel_proposal_patch_source_readiness`；readiness report 聚合 W88 mapping，证明 exact_mapping_count=4、metadata_safe_fallback_count=5、unsupported_or_unclassified_count=0、BuilderReview 只用于 BuilderReview，且 `apply_proposal_to_state` 调用 `ensure_lifemodel_proposal_patch_source_mapping`、通过 `resolve_lifemodel_patch_source_for_proposal(proposal)` 向 `LifeModelPatch::from_proposal` 传 PatchSource、apply path 不含硬编码 `PatchSource::BuilderReview`；ordinary `send_message` / `start_stream_message` 不调用 W88/W89 proposal PatchSource helper；report metadata-safe，不含 raw proposal payload、raw LifeModel patch value、memory text、chat text 或 tool payload；不新增 command/frontend/Settings，不运行 runtime/model/tool，不改 default Chat，不退休 legacy path，fallback source strategy 仍是 blocker，`proposal_first_convergence_complete=false` | AI Agent |

---

*本文档基于代码实际状态编写。如内容过时，请同步更新此文件。*
