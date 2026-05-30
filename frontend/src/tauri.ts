import { invoke } from "@tauri-apps/api/core";
import type {
  LifeModel,
  ChatMessage,
  DailyGoal,
  StateHistoryEntry,
  StateAlert,
  MultiStrategyAgentPreviewInput,
  MultiStrategyAgentPreviewOutput,
  RuntimeMigrationGateCheckInput,
  RuntimeMigrationGateReport,
} from "./types";

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

function safeInvoke<T>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (!isTauriEnv()) {
    return Promise.reject(
      new Error("当前不在 OpenLife 桌面应用环境中，无法调用原生功能。请在桌面窗口内操作。")
    );
  }
  const normalizedArgs = withTauriArgAliases(args);
  if (import.meta.env.DEV && import.meta.env.MODE !== "test") {
    console.log("[safeInvoke]", cmd, JSON.stringify(normalizedArgs));
  }
  return invoke<T>(cmd, normalizedArgs);
}

function sessionArgs(sessionId: string): { sessionId: string; session_id: string } {
  return { sessionId, session_id: sessionId };
}

function snakeToCamel(key: string): string {
  return key.replace(/_([a-z])/g, (_, char: string) => char.toUpperCase());
}

function withTauriArgAliases(args?: Record<string, any>): Record<string, any> | undefined {
  if (!args) return args;
  const normalized = { ...args };
  for (const [key, value] of Object.entries(args)) {
    if (!key.includes("_")) continue;
    const camelKey = snakeToCamel(key);
    if (!(camelKey in normalized)) {
      normalized[camelKey] = value;
    }
  }
  return normalized;
}

function optionalDualArg<T>(
  camelKey: string,
  snakeKey: string,
  value: T | undefined
): Record<string, T> {
  return value === undefined ? {} : { [camelKey]: value, [snakeKey]: value };
}

export async function getLifeModel(): Promise<LifeModel> {
  return safeInvoke<LifeModel>("get_life_model");
}

export async function saveLifeModel(model: LifeModel): Promise<void> {
  return safeInvoke("save_life_model", { lifeModel: model });
}

export interface ChatProposalConfig {
  enabled?: boolean;
  confidence_threshold?: number;
  min_message_length?: number;
  cooldown_seconds?: number;
}

export interface AppConfig {
  llm: {
    provider?:
      | "deepseek"
      | "openai"
      | "openrouter"
      | "siliconflow"
      | "moonshot"
      | "dashscope"
      | "zhipu"
      | "custom";
    openai_base: string;
    openai_key: string;
    embedding_model: string;
    chat_model: string;
    embedding_enabled?: boolean;
  };
  prefer_local_model: boolean;
  local_model: string;
  chat_proposal?: ChatProposalConfig;
  experimental_context_assembler?: boolean;
  use_agent_loop?: boolean;
  system?: {
    ollama_cache_ttl_seconds?: number;
    memory_search_top_k?: number;
    safe_paths?: string[];
    network_policy?: NetworkPolicy;
  };
}

export interface NetworkPolicy {
  enabled?: boolean;
  default_decision?: "ask" | "allow" | "deny";
  domain_allowlist?: string[];
  domain_denylist?: string[];
  tool_overrides?: Record<string, "ask" | "allow" | "deny">;
}

export async function getConfig(): Promise<AppConfig> {
  return safeInvoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return safeInvoke("save_config", { config });
}

// DEPRECATED: use sendMessageV2 for full trace support
export async function sendMessage(sessionId: string, messages: ChatMessage[]): Promise<string> {
  const result = await safeInvoke<SendMessageResult>("send_message", {
    ...sessionArgs(sessionId),
    messages,
  });
  return result.reply;
}

export type ToolCallStatus = "success" | "error" | "pending" | "blocked" | "needs_confirmation";

export interface ToolCallResult {
  name: string;
  arguments: Record<string, any>;
  sanitized_arguments?: Record<string, any>;
  success: boolean;
  output?: string;
  error?: string;
  permission_level?: string;
  status?: ToolCallStatus;
  requires_confirmation?: boolean;
  pii_found?: boolean;
  privacy_warnings?: string[];
  action_id?: string;
  run_id?: string;
  permission_decision?: string;
}

export interface ReasoningTrace {
  input?: string;
  meaning_result?: any;
  strategy_result?: any;
  generation_result?: any;
  output?: string;
  errors?: string[];
  tool_plan?: string[];
  safety_check_result?: {
    passed?: boolean;
    warnings?: string[];
    strict_mode?: boolean;
  };
  layer_timings_ms?: Record<string, number>;
  stable_steps?: string[];
  hsSelectionAudit?: HSSelectionAudit;
  behaviorChecks?: HSBehaviorCheckSummary[];
}

export interface HSAssetExclusion {
  assetId: string;
  assetKind: string;
  reason: string;
}

export interface HSSelectionAudit {
  selectedPolicyIds?: string[];
  selectedHeuristicIds?: string[];
  excludedAssets?: HSAssetExclusion[];
  estimatedTokens?: number;
  tokenBudget?: number;
}

export interface HSBehaviorCheckSummary {
  id: string;
  label: string;
  passed: boolean;
  summary?: string;
}

export interface HSEvidenceSummary {
  id: string;
  summary: string;
  sourceAssetIds?: string[];
  contentDigest?: string;
}

export interface SendMessageResult {
  reply: string;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  run_id?: string;
}

export interface StreamMessageStartPayload {
  session_id: string;
  run_id: string;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
}

export interface StreamMessageDonePayload {
  session_id: string;
  run_id: string;
  reply: string;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
}

export async function sendMessageV2(
  sessionId: string,
  messages: ChatMessage[]
): Promise<SendMessageResult> {
  return safeInvoke<SendMessageResult>("send_message", { ...sessionArgs(sessionId), messages });
}

export async function runMultiStrategyAgentPreview(
  input: MultiStrategyAgentPreviewInput
): Promise<MultiStrategyAgentPreviewOutput> {
  return safeInvoke<MultiStrategyAgentPreviewOutput>("run_multi_strategy_agent_preview", { input });
}

export async function checkRuntimeMigrationGate(
  input: RuntimeMigrationGateCheckInput = {}
): Promise<RuntimeMigrationGateReport> {
  return safeInvoke<RuntimeMigrationGateReport>("check_runtime_migration_gate", { input });
}

export async function startStreamMessage(
  sessionId: string,
  messages: ChatMessage[]
): Promise<void> {
  const payload = { ...sessionArgs(sessionId), messages };
  return safeInvoke<void>("start_stream_message", { ...payload, args: payload });
}

// Note: Hermes dispatch command has been removed. Use AgentRuntime instead.

export async function getChatHistory(sessionId: string): Promise<ChatMessage[]> {
  return safeInvoke<ChatMessage[]>("get_chat_history", sessionArgs(sessionId));
}

export async function saveChatMessage(sessionId: string, message: ChatMessage): Promise<void> {
  return safeInvoke("save_chat_message", { ...sessionArgs(sessionId), message });
}

export async function checkOllamaStatus(): Promise<boolean> {
  return safeInvoke<boolean>("check_ollama_status");
}

export interface RouterStatus {
  onnx_available: boolean;
  onnx_disabled: boolean;
  active_backend: string;
  latency_threshold_us: number;
}

export async function getRouterStatus(): Promise<RouterStatus> {
  return safeInvoke<RouterStatus>("get_router_status");
}

export async function getModelRouterStatus(): Promise<ModelRouterStatus> {
  return safeInvoke<ModelRouterStatus>("get_model_router_status");
}

export async function replayAgentAction(runId: string, actionId: string): Promise<AgentAction> {
  return safeInvoke<AgentAction>("replay_agent_action", { runId, actionId });
}

export interface BuilderCompletion {
  identity: number;
  goals: number;
  capabilities: number;
  state: number;
  overall: number;
  lowest_dimension?: string | null;
}

export interface DataFileStatus {
  messages_db_exists: boolean;
  messages_db_size_mb: number;
  vectors_db_exists: boolean;
  vectors_db_size_mb: number;
  mcp_audit_db_exists: boolean;
  mcp_audit_db_size_mb: number;
  config_yaml_exists: boolean;
  life_model_yaml_exists: boolean;
}

export interface OllamaModelInfo {
  name: string;
  size_mb: number;
}

export interface SystemDiagnostics {
  router: RouterStatus;
  mcp_server_count: number;
  mcp_tool_count: number;
  mcp_recent_audit_count: number;
  mcp_recent_pii_count: number;
  memory_chunk_count: number;
  vector_corrupt_embedding_count?: number;
  unfinished_builder_sessions: number;
  pending_builder_review_sessions?: number;
  ollama_online: boolean;
  local_model: string;
  resolved_local_model?: string | null;
  prefer_local_model: boolean;
  cloud_api_configured: boolean;
  cloud_provider?: string;
  cloud_api_validated?: boolean;
  cloud_api_last_error?: string | null;
  chat_ready: boolean;
  readiness_issues: string[];
  data_dir: string;
  active_data_dir?: string;
  legacy_data_dir?: string | null;
  database_status?: string;
  startup_warnings?: string[];
  snapshot_count: number;
  life_model_ready: boolean;
  app_version: string;
  model_empty: boolean;
  chat_session_count: number;
  onboarding_completed: boolean;
  beta_ready: boolean;
  beta_readiness_issues: string[];
  builder_completion: BuilderCompletion;
  data_files: DataFileStatus;
  ollama_models: OllamaModelInfo[];
  config_source: string;
  agent_run_count: number;
  agent_run_store_status: string;
  pending_proposal_count: number;
  high_risk_pending_proposal_count: number;
  proposal_store_status: string;
}

export async function getSystemDiagnostics(): Promise<SystemDiagnostics> {
  return safeInvoke<SystemDiagnostics>("get_system_diagnostics");
}

export async function getSchedulerConfig(): Promise<{ localModel: string; preferLocal: boolean }> {
  return safeInvoke<{ localModel: string; preferLocal: boolean }>("get_scheduler_config");
}

export async function setSchedulerConfig(localModel: string, preferLocal: boolean): Promise<void> {
  return safeInvoke("set_scheduler_config", {
    localModel,
    local_model: localModel,
    preferLocal,
    prefer_local: preferLocal,
  });
}

export async function executeToolCall(
  name: string,
  arguments_: Record<string, any>
): Promise<ToolCallResult> {
  return safeInvoke<ToolCallResult>("execute_tool_call", { name, arguments: arguments_ });
}

export interface McpPrivacyFinding {
  path: string;
  privacy_type: string;
  matched: string;
}

export interface McpArgumentInspection {
  permission_level: string;
  pii_found: boolean;
  findings: McpPrivacyFinding[];
  sanitized_arguments: Record<string, any>;
  requires_confirmation: boolean;
}

export async function inspectMcpCall(
  name: string,
  arguments_: Record<string, any>
): Promise<McpArgumentInspection> {
  return safeInvoke<McpArgumentInspection>("inspect_mcp_call", { name, arguments: arguments_ });
}

export async function registerMcpServer(
  name: string,
  command: string,
  args: string[],
  env?: Record<string, string>
): Promise<void> {
  return safeInvoke("register_mcp_server", { name, command, args, env });
}

export async function unregisterMcpServer(name: string): Promise<void> {
  return safeInvoke("unregister_mcp_server", { name });
}

export interface McpServerInfo {
  name: string;
  command: string;
  args: string[];
  tool_count: number;
}

export interface McpAuditLogEntry {
  id: number;
  tool_name: string;
  arguments: string;
  result: string;
  success: boolean;
  pii_found: boolean;
  created_at: string;
}

export async function listMcpServers(): Promise<McpServerInfo[]> {
  return safeInvoke<McpServerInfo[]>("list_mcp_servers");
}

export async function listMcpAuditLogs(limit = 20): Promise<McpAuditLogEntry[]> {
  return safeInvoke<McpAuditLogEntry[]>("list_mcp_audit_logs", { limit });
}

export async function clearMcpAuditLogs(days: number): Promise<number> {
  return safeInvoke<number>("clear_mcp_audit_logs", { days });
}

export async function listMcpTools(): Promise<any[]> {
  return safeInvoke<any[]>("list_mcp_tools");
}

export async function listToolManifests(): Promise<ToolManifest[]> {
  return safeInvoke<ToolManifest[]>("list_tool_manifests");
}

export interface McpTemplate {
  id: string;
  name: string;
  description: string;
  command: string;
  args: string[];
  required_args: string[];
  arg_labels?: Record<string, string>;
  env?: Record<string, string>;
  tags?: string[];
}

export async function listMcpTemplates(): Promise<McpTemplate[]> {
  return safeInvoke<McpTemplate[]>("list_mcp_templates");
}

export interface ToolManifest {
  id: string;
  name: string;
  description: string;
  parameters: any;
  permission_level: string;
  risk_level: string;
  version: string;
  source:
    | { type: "BuiltIn" }
    | { type: "Mcp"; server_name: string }
    | { type: "A2A"; agent_name: string }
    | { type: "Plugin"; plugin_id: string };
  capabilities: string[];
  requires_confirmation: boolean;
  enabled: boolean;
  declarative_only: boolean;
  action_type: string;
  tags: string[];
}

export async function recommendMcpManifests(topK?: number): Promise<ToolManifest[]> {
  const value = topK ?? 5;
  return safeInvoke<ToolManifest[]>("recommend_mcp_manifests", {
    topK: value,
    top_k: value,
  });
}

export async function createSnapshot(
  tag: string,
  note: string
): Promise<import("./types").LifeModelVersion> {
  return safeInvoke<import("./types").LifeModelVersion>("create_snapshot", { tag, note });
}

export async function listSnapshots(): Promise<import("./types").LifeModelVersion[]> {
  return safeInvoke<import("./types").LifeModelVersion[]>("list_snapshots");
}

export async function restoreSnapshot(version: string): Promise<import("./types").LifeModel> {
  return safeInvoke<import("./types").LifeModel>("restore_snapshot", { version });
}

export async function diffSnapshots(v1: string, v2: string): Promise<string> {
  return safeInvoke<string>("diff_snapshots", { v1, v2 });
}

export async function saveFeedback(
  sessionId: string,
  messageIndex: number,
  feedbackType: "up" | "down",
  contentPreview: string
): Promise<void> {
  return safeInvoke("save_feedback", {
    ...sessionArgs(sessionId),
    messageIndex,
    message_index: messageIndex,
    feedbackType,
    feedback_type: feedbackType,
    contentPreview,
    content_preview: contentPreview,
  });
}

export async function getFeedbackSummary(): Promise<{
  total_messages: number;
  total_feedback_up: number;
  total_feedback_down: number;
  session_count: number;
}> {
  return safeInvoke("get_feedback_summary");
}

export async function applyFeedbackEvolution(): Promise<string> {
  return safeInvoke<string>("apply_feedback_evolution");
}

export async function generateEvolutionReport(): Promise<{
  summary: string;
  liked_patterns: string[];
  disliked_patterns: string[];
  applied_rules: string[];
}> {
  return safeInvoke("generate_evolution_report");
}

export async function runMicroEvolution(): Promise<{
  changes: EvolutionChange[];
  applied: boolean;
  message: string;
  snapshot_version?: string | null;
}> {
  return safeInvoke("run_micro_evolution");
}

export async function generateCalibrationReport(periodDays: number): Promise<{
  period_days: number;
  feedback_up: number;
  feedback_down: number;
  top_liked_patterns: string[];
  top_disliked_patterns: string[];
  value_changes: string[];
  suggested_actions: string[];
  summary_text: string;
}> {
  return safeInvoke("generate_calibration_report", {
    periodDays,
    period_days: periodDays,
  });
}

export interface SignalSource {
  source: string;
  score: number;
  weight: number;
}

export interface EvolutionChange {
  dimension: string;
  target_name: string;
  old_value: number;
  new_value: number;
  reason: string;
  confidence: number;
  sources: SignalSource[];
}

export interface SignalContributor {
  name: string;
  score: number;
  source: string;
}

export interface EvolutionSignalSummary {
  feedback_terms: number;
  behavior_events: number;
  inference_items: number;
  top_feedback: SignalContributor[];
  top_behavior: SignalContributor[];
  top_inference: SignalContributor[];
}

export async function generateMicroEvolutionChanges(): Promise<{
  changes: EvolutionChange[];
  applied: boolean;
  message: string;
  before: Model4DCompletion;
  after: Model4DCompletion;
  requires_confirmation: boolean;
  signal_summary: EvolutionSignalSummary;
}> {
  return safeInvoke("generate_micro_evolution_changes");
}

export async function applyCalibration(
  changes: EvolutionChange[],
  mode: "direct" | "proposal" = "proposal"
): Promise<{
  success: boolean;
  snapshot_version?: string;
  applied_count?: number;
  created_count?: number;
  created_ids?: string[];
  run_id?: string;
  error_count?: number;
  errors?: string[];
  message: string;
}> {
  return safeInvoke("apply_calibration", { changes, mode });
}

export async function calibrationCreateProposals(changes: EvolutionChange[]): Promise<{
  created_count: number;
  created_ids: string[];
  run_id: string;
  error_count: number;
  errors: string[];
  message: string;
}> {
  return safeInvoke("calibration_create_proposals", { changes });
}

export async function shouldShowCalibration(): Promise<{
  weekly: boolean;
  monthly: boolean;
  today: string;
}> {
  return safeInvoke("should_show_calibration");
}

export async function markCalibrationShown(period: "weekly" | "monthly"): Promise<void> {
  return safeInvoke("mark_calibration_shown", { period });
}

export async function runMemoryTierMaintenance(): Promise<{
  promoted: number;
  demoted: number;
}> {
  return safeInvoke("run_memory_tier_maintenance");
}

export async function countMemoryChunks(): Promise<number> {
  return safeInvoke("count_memory_chunks");
}

export async function rebuildMemoryIndex(): Promise<{
  processed: number;
  indexed: number;
  skipped: number;
}> {
  return safeInvoke("rebuild_memory_index");
}

export async function logAnalyticsEvent(
  eventName: string,
  sessionId?: string,
  detail?: string
): Promise<void> {
  return safeInvoke("log_analytics_event", {
    eventName,
    event_name: eventName,
    ...optionalDualArg("sessionId", "session_id", sessionId),
    detail,
  });
}

export async function indexMemoryChunk(
  sessionId: string,
  content: string,
  source: string
): Promise<number> {
  return safeInvoke("index_memory_chunk", { ...sessionArgs(sessionId), content, source });
}

export async function searchMemory(
  query: string,
  topK: number
): Promise<
  Array<{
    chunk: { id: number; session_id: string; content: string; source: string; created_at: string };
    score: number;
  }>
> {
  const raw: Array<[any, number]> = await safeInvoke("search_memory", {
    query,
    topK,
    top_k: topK,
  });
  return raw.map(([chunk, score]) => ({ chunk, score }));
}

export async function a2aDiscoverAgent(url: string): Promise<any> {
  return safeInvoke("a2a_discover_agent", { url });
}

export async function a2aSendTask(url: string, requestJson: string): Promise<string> {
  return safeInvoke<string>("a2a_send_task", { url, requestJson, request_json: requestJson });
}

export async function a2aLocalAgentCard(): Promise<any> {
  return safeInvoke("a2a_local_agent_card");
}

export async function a2aHandleTask(requestJson: string): Promise<string> {
  return safeInvoke<string>("a2a_handle_task", { requestJson, request_json: requestJson });
}

export async function a2aBridgeLocal(
  method: string,
  text: string,
  sessionId?: string,
  skill?: string
): Promise<any> {
  return safeInvoke("a2a_bridge_local", {
    method,
    text,
    ...optionalDualArg("sessionId", "session_id", sessionId),
    skill,
  });
}

export async function a2aRestartSidecar(): Promise<void> {
  return safeInvoke("a2a_restart_sidecar");
}

export async function a2aStopSidecar(): Promise<void> {
  return safeInvoke("a2a_stop_sidecar");
}

export interface BuilderProgress {
  progress: number;
  current_step_label: string;
  step_index: number;
  total_steps: number;
  current_session?: number;
  waiting_pairwise?: boolean;
  waiting_phase_confirmation?: boolean;
  phase_summary?: string;
}

export interface BuilderAnalysis {
  completion: Model4DCompletion;
  gaps: string[];
}
export interface BuilderSignal {
  id: string;
  source_step: number;
  source_question_id: string;
  dimension: string;
  affected_path: string;
  proposed_value: unknown;
  confidence: number;
  reason: string;
  risk_level: "low" | "medium" | "high";
  user_status: "Pending" | "Accepted" | "Edited" | "Rejected";
}

export interface BuilderSummary {
  identity_summary: string;
  goals_summary: string;
  capabilities_summary: string;
  state_summary: string;
  assumptions: string[];
  unresolved_questions: string[];
  recommended_next_steps: string[];
}

export interface BuilderPatchReview {
  signals: BuilderSignal[];
  summary: BuilderSummary;
  assumptions: string[];
  uncertain_fields: string[];
  confidence_by_dimension: Record<string, number>;
}

export async function builderStart(
  mode: "quick" | "incremental" | "socratic",
  sessionId: string,
  targetDimension?: "identity" | "goals" | "capabilities" | "state"
): Promise<{
  prompt: string;
  progress: BuilderProgress;
  analysis?: BuilderAnalysis;
  finished?: boolean;
  pending_signals?: BuilderSignal[];
  mode?: string;
  target_dimension?: string;
}> {
  return safeInvoke("builder_start", {
    mode,
    ...sessionArgs(sessionId),
    ...optionalDualArg("targetDimension", "target_dimension", targetDimension),
  });
}

export async function builderStep(
  sessionId: string,
  userReply: string
): Promise<{
  prompt: string;
  finished: boolean;
  model?: LifeModel;
  progress: BuilderProgress;
  analysis?: BuilderAnalysis;
  pending_signals?: BuilderSignal[];
  mode?: string;
  target_dimension?: string;
}> {
  return safeInvoke("builder_step", {
    ...sessionArgs(sessionId),
    userReply,
    user_reply: userReply,
  });
}

export interface UnfinishedBuilderSession {
  session_id: string;
  mode: "Quick" | "Incremental" | "Socratic";
  step_index: number;
  finished: boolean;
  draft_yaml: string;
  current_prompt?: string;
  pending_signals?: BuilderSignal[];
  target_dimension?: "Identity" | "Goals" | "Capabilities" | "State";
}

export async function builderListUnfinished(): Promise<UnfinishedBuilderSession[]> {
  return safeInvoke("builder_list_unfinished");
}

export async function builderDeleteSession(sessionId: string): Promise<void> {
  return safeInvoke("builder_delete_session", sessionArgs(sessionId));
}
export async function builderGetPendingSignals(sessionId: string): Promise<{
  session_id: string;
  signals: BuilderSignal[];
  summary: BuilderSummary;
  finished: boolean;
}> {
  return safeInvoke("builder_get_pending_signals", sessionArgs(sessionId));
}

export interface BuilderSignalDecision {
  id: string;
  status: "accepted" | "rejected" | "edited";
  proposed_value?: unknown;
}

export interface SkippedField {
  path: string;
  reason: string;
  expected?: string;
}

export async function builderApplySignals(
  sessionId: string,
  decisions: BuilderSignalDecision[]
): Promise<{
  success: boolean;
  applied_fields: string[];
  merged_fields?: string[];
  skipped_fields: SkippedField[];
  edited_count: number;
  rejected_count: number;
  model: LifeModel;
}> {
  return safeInvoke("builder_apply_signals", { ...sessionArgs(sessionId), decisions });
}

export async function builderCreateProposals(
  sessionId: string,
  decisions: BuilderSignalDecision[]
): Promise<{
  success: boolean;
  created_count: number;
  rejected_count: number;
  proposal_ids: string[];
  run_id: string;
  warnings?: string[];
}> {
  return safeInvoke("builder_create_proposals", { ...sessionArgs(sessionId), decisions });
}

export async function goalCapabilityGapAnalysis(): Promise<string[]> {
  return safeInvoke<string[]>("goal_capability_gap_analysis");
}

export interface CapabilityGap {
  goal_name: string;
  skill_name: string;
  current_level: number;
  target_level: number;
  severity: string;
  suggestion: string;
}

export async function goalCapabilityGapReport(): Promise<CapabilityGap[]> {
  return safeInvoke<CapabilityGap[]>("goal_capability_gap_report");
}

export async function identityGoalAlignmentCheck(): Promise<string[]> {
  return safeInvoke<string[]>("identity_goal_alignment_check");
}

export interface AlignmentIssue {
  goal_name: string;
  severity: string;
  related_values: string[];
  reason: string;
  suggestion: string;
}

export async function identityGoalAlignmentReport(): Promise<AlignmentIssue[]> {
  return safeInvoke<AlignmentIssue[]>("identity_goal_alignment_report");
}

export interface Model4DCompletion {
  identity: number;
  goals: number;
  capabilities: number;
  state: number;
  overall: number;
}

export async function getModel4DCompletion(): Promise<Model4DCompletion> {
  return safeInvoke<Model4DCompletion>("get_model_4d_completion");
}

export interface ExportedMessage {
  session_id: string;
  role: string;
  content: string;
  created_at: string;
}

export interface ExportedVectorChunk {
  session_id: string;
  content: string;
  embedding: number[];
  source: string;
  created_at: string;
  tier: number;
  access_count: number;
  last_accessed_at: string;
}

export interface ExportPayload {
  version: string;
  app_version?: string;
  exported_at: string;
  life_model: LifeModel;
  messages: ExportedMessage[];
  vectors: ExportedVectorChunk[];
}

export async function exportAllData(): Promise<ExportPayload> {
  return safeInvoke<ExportPayload>("export_all_data");
}

export async function importAllData(payload: ExportPayload): Promise<void> {
  return safeInvoke("import_all_data", { payload });
}

export async function testApiKey(): Promise<boolean> {
  return safeInvoke<boolean>("test_api_key");
}

export interface LlmConnectionTestResult {
  ok: boolean;
  provider: string;
  message: string;
}

export async function testLlmConnection(config: AppConfig): Promise<LlmConnectionTestResult> {
  return safeInvoke<LlmConnectionTestResult>("test_llm_connection", { config });
}

export interface ChatSession {
  session_id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export async function listChatSessions(): Promise<ChatSession[]> {
  return safeInvoke<ChatSession[]>("list_chat_sessions");
}

export async function createChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("create_chat_session", { ...sessionArgs(sessionId), title });
}

export async function renameChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("rename_chat_session", { ...sessionArgs(sessionId), title });
}

export async function deleteChatSession(sessionId: string): Promise<void> {
  return safeInvoke("delete_chat_session", sessionArgs(sessionId));
}

export async function recordState(
  dimensionName: string,
  value: number,
  unit: string,
  note?: string,
  minThreshold?: number,
  maxThreshold?: number,
  alertDays?: number
): Promise<number> {
  return safeInvoke<number>("record_state", {
    dimensionName,
    dimension_name: dimensionName,
    value,
    unit,
    note,
    minThreshold,
    min_threshold: minThreshold,
    maxThreshold,
    max_threshold: maxThreshold,
    alertDays,
    alert_days: alertDays,
  });
}

export async function getStateHistory(
  dimensionName: string,
  limit: number
): Promise<StateHistoryEntry[]> {
  return safeInvoke<StateHistoryEntry[]>("get_state_history", {
    dimensionName,
    dimension_name: dimensionName,
    limit,
  });
}

export async function getStateAlerts(): Promise<StateAlert[]> {
  return safeInvoke<StateAlert[]>("get_state_alerts");
}

export async function getDailyGoals(): Promise<DailyGoal[]> {
  return safeInvoke<DailyGoal[]>("get_daily_goals");
}

export async function addDailyGoal(
  name: string,
  timeBlock?: { start: string; end: string }
): Promise<void> {
  return safeInvoke("add_daily_goal", {
    name,
    ...optionalDualArg("timeBlock", "time_block", timeBlock),
  });
}

export async function updateDailyGoal(
  index: number,
  name: string,
  timeBlock?: { start: string; end: string }
): Promise<void> {
  return safeInvoke("update_daily_goal", {
    index,
    name,
    ...optionalDualArg("timeBlock", "time_block", timeBlock),
  });
}

export async function deleteDailyGoal(index: number): Promise<void> {
  return safeInvoke("delete_daily_goal", { index });
}

export async function toggleDailyGoal(index: number): Promise<boolean> {
  return safeInvoke<boolean>("toggle_daily_goal", { index });
}

// ── Milestone D: Hot Memory Cache ──
export interface HotMemoryCache {
  identity_summary: string;
  top_values: string[];
  current_goals: string[];
  recent_state: string;
  last_refreshed: string;
  life_model_version: string;
}

export async function getHotCache(): Promise<HotMemoryCache> {
  return safeInvoke<HotMemoryCache>("get_hot_cache");
}

// ── Milestone D: Memory Tier / Archive ──
export interface ArchivedChunkSummary {
  id: number;
  session_id: string;
  content: string;
  source: string;
  created_at: string;
  archived_at: string;
  summary?: string;
  access_count: number;
  importance_score: number;
}

export interface TierStats {
  total: number;
  tier1: number;
  tier2: number;
  tier3: number;
  archived: number;
}

export async function archiveLowAccessMemories(): Promise<number> {
  return safeInvoke<number>("archive_low_access_memories");
}

export async function restoreArchivedChunks(chunkIds: number[]): Promise<number> {
  return safeInvoke<number>("restore_archived_chunks", {
    chunkIds,
    chunk_ids: chunkIds,
  });
}

export async function listArchivedChunks(limit: number): Promise<ArchivedChunkSummary[]> {
  return safeInvoke<ArchivedChunkSummary[]>("list_archived_chunks", { limit });
}

export async function getMemoryTierStats(): Promise<TierStats> {
  return safeInvoke<TierStats>("get_memory_tier_stats");
}

// ── Milestone D: MCP Audit Export / Cleanup ──
export interface ExportedAuditEntry {
  id: number;
  tool_name: string;
  arguments: string;
  result: string;
  success: boolean;
  created_at: string;
  pii_found: boolean;
}

export interface AuditExport {
  exported_at: string;
  entry_count: number;
  days: number;
  entries: ExportedAuditEntry[];
}

export async function exportMcpAuditLogs(days: number): Promise<AuditExport> {
  return safeInvoke<AuditExport>("export_mcp_audit_logs", { days });
}

export async function cleanupMcpAuditLogs(retentionDays: number): Promise<number> {
  return safeInvoke<number>("cleanup_mcp_audit_logs", {
    retentionDays,
    retention_days: retentionDays,
  });
}

export async function rotateMcpAuditKey(): Promise<void> {
  return safeInvoke("rotate_mcp_audit_key");
}

// ── Milestone D: Configurable Privacy Policy ──
export type PrivacyAction = "Mask" | "Block" | "Allow";

export interface PrivacyRule {
  ptype: string;
  enabled: boolean;
  action: PrivacyAction;
  custom_pattern?: string;
}

export interface PrivacyPolicy {
  rules: PrivacyRule[];
  enabled: boolean;
}

export async function getPrivacyPolicy(): Promise<PrivacyPolicy> {
  return safeInvoke<PrivacyPolicy>("get_privacy_policy");
}

export async function setPrivacyPolicy(policy: PrivacyPolicy): Promise<void> {
  return safeInvoke("set_privacy_policy", { policy });
}

export async function hasCompletedOnboarding(): Promise<boolean> {
  return safeInvoke<boolean>("has_completed_onboarding");
}

export async function markOnboardingCompleted(): Promise<void> {
  return safeInvoke("mark_onboarding_completed");
}

export interface LastModelError {
  message: string;
  phase: string;
  timestamp: string;
}

export async function getLastModelError(): Promise<LastModelError | null> {
  return safeInvoke<LastModelError | null>("get_last_model_error");
}

// ── AgentRun ──
export interface ModelRouteTrace {
  provider: string;
  model: string;
  routeType: string;
  preferLocal: boolean;
  localModel: string;
  reason: string;
  privacyLevel: string;
  latencyMs?: number;
  retryCount: number;
  fallbackReason?: string;
  providerHealthIsEstimated?: boolean;
}

export interface ContextSummary {
  lifeModelEmpty: boolean;
  includedLifeModelSections: string[];
  memoryHitCount: number;
  memorySources: string[];
  usedToolsPrompt: boolean;
  redactionApplied: boolean;
  redactionLevel: string;
}

export interface ToolActionScope {
  toolId: string;
  toolName: string;
  source: string;
  riskLevel: string;
  capabilities: string[];
  actionType: string;
}

export interface ProviderStatus {
  name: string;
  enabled: boolean;
  available: boolean;
  healthIsEstimated: boolean;
  lastError?: string;
  latencyMs?: number;
  lastChecked?: string;
}

export interface ModelRouterStatus {
  enabled: boolean;
  providers: ProviderStatus[];
  lastCheckAt?: string;
  message?: string;
}

export interface AgentAction {
  id: string;
  actionType: string;
  target?: string;
  input: any;
  output?: any;
  status: string;
  permissionDecision?: string;
  startedAt?: string;
  finishedAt?: string;
  error?: string;
  timestamp: string;
  toolScope?: ToolActionScope;
}

export interface AgentObservation {
  id: string;
  actionId?: string;
  content: string;
  source: string;
  structuredResult?: any;
  timestamp: string;
}

export interface AgentStatusUpdate {
  phase: string;
  message: string;
  stepIndex: number;
  toolCallIndex?: number;
  timestamp: string;
}

export interface AgentRunError {
  message: string;
  phase: string;
  recoverable: boolean;
}

export interface AgentRun {
  id: string;
  taskId: string;
  sessionId?: string;
  status: "running" | "waiting_permission" | "completed" | "failed" | "cancelled";
  kind:
    | "conversation"
    | "builder"
    | "calibration"
    | "evolution"
    | "tool_execution"
    | "proactive"
    | "planning"
    | "review"
    | "writing"
    | "memory_governance"
    | "skill"
    | "plugin";
  userInput?: string;
  contextSummary?: ContextSummary;
  modelRoute?: ModelRouteTrace;
  outputPreview?: string;
  error?: AgentRunError;
  generatedProposals: string[];
  actions: AgentAction[];
  observations: AgentObservation[];
  reasoningStrategy?: string;
  reasoningTrace?: ReasoningTrace;
  hsSelectionAudit?: HSSelectionAudit;
  behaviorChecks?: HSBehaviorCheckSummary[];
  statusUpdates?: AgentStatusUpdate[];
  stepCount?: number;
  toolCallCount?: number;
  warnings?: string[];
  deletedAt?: string;
  deleteReason?: string;
  startedAt: string;
  finishedAt?: string;
}

export async function getAgentRun(runId: string): Promise<AgentRun | null> {
  return safeInvoke<AgentRun | null>("get_agent_run", { runId });
}

export async function listAgentRuns(limit: number = 50, offset: number = 0): Promise<AgentRun[]> {
  return safeInvoke<AgentRun[]>("list_agent_runs", { limit, offset });
}

export async function listRuns(limit: number = 50, offset: number = 0): Promise<AgentRun[]> {
  return listAgentRuns(limit, offset);
}

export async function listAgentRunsForSession(
  sessionId: string,
  limit: number = 50
): Promise<AgentRun[]> {
  return safeInvoke<AgentRun[]>("list_agent_runs_for_session", { sessionId, limit });
}

export async function deleteAgentRun(runId: string, reason?: string): Promise<void> {
  return safeInvoke("delete_agent_run", { runId, reason });
}

export type ToolPermissionPolicy =
  | "allow"
  | "deny"
  | "ask_every_time"
  | "allow_once"
  | "allow_until_revoked";

export interface ToolPermissionRecord {
  id: string;
  toolName: string;
  source: string;
  riskLevel: string;
  actionType: string;
  policy: ToolPermissionPolicy;
  createdAt: string;
  expiresAt?: string;
  consumedAt?: string;
}

export interface ToolPermissionDecision {
  allowed: boolean;
  requiresConfirmation: boolean;
  decision: string;
  reason: string;
  policyId?: string;
}

export async function listToolPermissions(): Promise<ToolPermissionRecord[]> {
  return safeInvoke<ToolPermissionRecord[]>("list_tool_permissions");
}

export async function revokeToolPermission(permissionId: string): Promise<boolean> {
  return safeInvoke<boolean>("revoke_tool_permission", {
    permissionId,
    permission_id: permissionId,
  });
}

export interface SkillManifest {
  id: string;
  name: string;
  description: string;
  requiredContext: string[];
  allowedTools: string[];
  executionBudget: {
    maxSteps: number;
    maxToolCalls: number;
    timeoutSeconds: number;
    allowCloud: boolean;
    allowWrites: boolean;
  };
  outputSchema: any;
  proposalPolicy: string;
}

export interface SkillRunResponse {
  runId: string;
  status: string;
  summary: string;
  generatedProposals: string[];
}

export async function listSkills(): Promise<SkillManifest[]> {
  return safeInvoke<SkillManifest[]>("list_skills");
}

export async function runSkill(skillId: string, input: any): Promise<SkillRunResponse> {
  return safeInvoke<SkillRunResponse>("run_skill", { skillId, skill_id: skillId, input });
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  tools: ToolManifest[];
  skills: SkillManifest[];
  permissions: string[];
  settingsSchema?: any;
  enabled: boolean;
  trustLevel: string;
}

export interface PluginRecord {
  manifest: PluginManifest;
  path: string;
  enabled: boolean;
  error?: string;
}

export async function listPlugins(): Promise<PluginRecord[]> {
  return safeInvoke<PluginRecord[]>("list_plugins");
}

export async function reloadPlugins(): Promise<PluginRecord[]> {
  return safeInvoke<PluginRecord[]>("reload_plugins");
}

export async function enablePlugin(pluginId: string): Promise<void> {
  return safeInvoke("enable_plugin", { pluginId, plugin_id: pluginId });
}

export async function disablePlugin(pluginId: string): Promise<void> {
  return safeInvoke("disable_plugin", { pluginId, plugin_id: pluginId });
}

export async function listProposals(
  status?: string,
  proposalType?: string,
  riskLevel?: string,
  limit: number = 50
): Promise<AgentProposal[]> {
  return safeInvoke<AgentProposal[]>("list_proposals", { status, proposalType, riskLevel, limit });
}

export async function batchAcceptLowRiskProposals(proposalIds?: string[]): Promise<number> {
  return safeInvoke<number>("batch_accept_low_risk_proposals", { proposalIds });
}

// ── Proposal ──
export type ProposalStatus =
  | "pending"
  | "accepted"
  | "rejected"
  | "edited"
  | "postponed"
  | "expired";
export type ProposalType =
  | "goal_update"
  | "state_update"
  | "preference_update"
  | "capability_update"
  | "memory_write"
  | "memory_archive"
  | "tool_permission"
  | "plugin_permission"
  | "scheduled_task"
  | "external_write_action"
  | "model_policy_change"
  | "data_export"
  | "schedule_checkin"
  | "unsupported"
  | "life_model_update";
export type RiskLevel = "low" | "medium" | "high" | "critical";
export type ProposalSource =
  | "builder_review"
  | "calibration_run"
  | "feedback_evolution"
  | "memory_governance"
  | "skill_runtime"
  | "plugin"
  | "manual"
  | "chat_conversation"
  | "proactive_agent";

export interface AgentProposal {
  id: string;
  runId?: string;
  proposalType: ProposalType;
  source: ProposalSource;
  sourceDetail?: string;
  affectedPath: string;
  before?: any;
  after: any;
  reason: string;
  confidence: number;
  riskLevel: RiskLevel;
  status: ProposalStatus;
  whyOpenLifeThinksThis?: string;
  evidenceSummaries?: HSEvidenceSummary[];
  behaviorChecks?: HSBehaviorCheckSummary[];
  createdAt: string;
  resolvedAt?: string;
  expiresAt?: string;
}

export async function getPendingProposals(limit: number = 50): Promise<AgentProposal[]> {
  return safeInvoke<AgentProposal[]>("get_pending_proposals", { limit });
}

export interface PatchApplyResult {
  patchId: string;
  success: boolean;
  path: string;
  operation: string;
  error?: string;
}

export async function acceptProposal(
  proposalId: string
): Promise<{ success: boolean; patchResult: PatchApplyResult }> {
  return safeInvoke("accept_proposal", { proposalId, proposal_id: proposalId });
}

export async function rejectProposal(proposalId: string): Promise<void> {
  return safeInvoke("reject_proposal", { proposalId, proposal_id: proposalId });
}

export async function editProposal(
  proposalId: string,
  newAfter: any
): Promise<{ success: boolean; patchResult: PatchApplyResult }> {
  return safeInvoke("edit_proposal", {
    proposalId,
    proposal_id: proposalId,
    newAfter,
    new_after: newAfter,
  });
}

export async function postponeProposal(proposalId: string): Promise<void> {
  return safeInvoke("postpone_proposal", { proposalId, proposal_id: proposalId });
}

export interface ProactiveSuggestion {
  id: string;
  category: "daily_brief" | "weekly_review" | "stale_goal" | "pending_proposal" | "state_checkin";
  title: string;
  prompt: string;
  priority: "low" | "medium" | "high";
  seen: boolean;
  created_at: string;
}

export async function getProactiveSuggestions(): Promise<ProactiveSuggestion[]> {
  return safeInvoke("get_proactive_suggestions");
}
