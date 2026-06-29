import { invoke } from "@tauri-apps/api/core";
import type {
  LifeModel,
  ChatMessage,
  DailyGoal,
  StateHistoryEntry,
  StateAlert,
  MultiStrategyAgentPreviewInput,
  MultiStrategyAgentPreviewOutput,
  MultiStrategyRuntimeMaturityReport,
  ReactBetaExecutionStatusReport,
  ControlledChatPilotEligibilityCheckInput,
  ControlledChatPilotEligibilityReport,
  ControlledPilotPromotionEvidenceInput,
  ControlledPilotPromotionEvidenceResult,
  ControlledPilotPromotionEvidenceSummary,
  ControlledPilotPromotionReadinessCheckInput,
  ControlledPilotPromotionReadinessReport,
  ControlledChatMigrationPlanDraftInput,
  ControlledChatMigrationPlanDraft,
  ControlledChatMigrationReviewDecisionInput,
  ControlledChatMigrationReviewDecisionResult,
  ControlledChatMigrationReviewDecisionSummary,
  ControlledChatMigrationImplementationGateInput,
  ControlledChatMigrationImplementationGateReport,
  ControlledChatCutoverReadinessInput,
  ControlledChatCutoverReadinessReport,
  ControlledChatCutoverCandidateInput,
  ControlledChatCutoverCandidateOutput,
  ControlledChatCutoverCandidateReviewDecisionInput,
  ControlledChatCutoverCandidateReviewDecisionResult,
  ControlledChatCutoverCandidateReviewSummary,
  ControlledChatCutoverCandidatePromotionReadinessInput,
  ControlledChatCutoverCandidatePromotionReadinessReport,
  ControlledChatMigrationShadowRunInput,
  ControlledChatMigrationShadowRunOutput,
  ControlledChatMigrationShadowReviewDecisionInput,
  ControlledChatMigrationShadowReviewDecisionResult,
  ControlledChatMigrationShadowReviewSummary,
  DefaultChatAdapterActivationPlanDraftInput,
  DefaultChatAdapterActivationPlanDraft,
  DefaultChatAdapterActivationReviewDecisionInput,
  DefaultChatAdapterActivationReviewDecisionResult,
  DefaultChatAdapterActivationReviewSummary,
  DefaultChatAdapterActivationImplementationGateInput,
  DefaultChatAdapterActivationImplementationGateReport,
  DefaultChatAdapterRoutingStatusInput,
  DefaultChatAdapterRoutingStatus,
  DefaultChatAdapterContractHarnessInput,
  DefaultChatAdapterContractHarnessReport,
  DefaultChatAdapterOrdinaryEntryPreflightStatus,
  DefaultChatAdapterNarrowImplementationDiscussionGateInput,
  DefaultChatAdapterNarrowImplementationDiscussionGateReport,
  DefaultChatAdapterNarrowImplementationPlanInput,
  DefaultChatAdapterNarrowImplementationPlanDraft,
  DefaultChatAdapterNarrowImplementationPlanApprovalReadinessInput,
  DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport,
  DefaultChatAdapterNarrowImplementationPlanReviewDecisionInput,
  DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult,
  DefaultChatAdapterNarrowImplementationPlanReviewSummary,
  DefaultChatAdapterDryRunInput,
  DefaultChatAdapterDryRunReport,
  DefaultChatAdapterDryRunReviewDecisionInput,
  DefaultChatAdapterDryRunReviewDecisionResult,
  DefaultChatAdapterDryRunReviewSummary,
  DefaultChatAdapterControlledPreviewInput,
  DefaultChatAdapterControlledPreviewReport,
  DefaultChatAdapterControlledPreviewApprovalReadinessInput,
  DefaultChatAdapterControlledPreviewApprovalReadinessReport,
  DefaultChatAdapterCutoverImplementationPlanInput,
  DefaultChatAdapterCutoverImplementationPlanDraft,
  DefaultChatAdapterCutoverPlanReviewDecisionInput,
  DefaultChatAdapterCutoverPlanReviewDecisionResult,
  DefaultChatAdapterCutoverPlanReviewSummary,
  DefaultChatAdapterCutoverPlanApprovalReadinessInput,
  DefaultChatAdapterCutoverPlanApprovalReadinessReport,
  DefaultChatAdapterControlledPreviewReviewDecisionInput,
  DefaultChatAdapterControlledPreviewReviewDecisionResult,
  DefaultChatAdapterControlledPreviewReviewSummary,
  DefaultChatAdapterImplementationReadinessInput,
  DefaultChatAdapterImplementationReadinessReport,
  DefaultChatRuntimeBoundaryStatus,
  RuntimeMigrationGateCheckInput,
  RuntimeMigrationGateReport,
  CreatePlanExecuteSessionInput,
  PlanExecuteSession,
  UpdatePlanExecuteSessionDraftInput,
  ExecutePlanExecuteStepInput,
  ExecutePlanExecuteStepOutput,
  SkipPlanExecuteStepInput,
  SkipPlanExecuteStepOutput,
  ReviewPlanExecuteSessionOutput,
  PlanExecuteReviewSummary,
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
    console.log("[safeInvoke]", cmd, redactInvokeArgs(cmd, normalizedArgs));
  }
  return invoke<T>(cmd, normalizedArgs);
}

type RedactedValue =
  | null
  | string
  | number
  | boolean
  | {
      redacted?: true;
      type?: string;
      keys?: string[];
      length?: number;
      itemCount?: number;
      hash?: string;
      items?: RedactedValue[];
      [key: string]: any;
    };

const SECRET_KEY_RE = /(openai_key|api_key|password|token|secret|authorization|credential)/i;
const PAYLOAD_KEY_RE = /(payload|import|export)/i;
const TOOL_ARGUMENT_KEY_RE = /^(arguments|args|toolArguments|tool_arguments)$/i;
const CONTENT_KEY_RE = /^(content|fileContent|file_content|body|emailBody|email_body)$/i;
const NOTES_KEY_RE = /^(notes|note|testerNotes|tester_notes)$/i;
const SESSION_KEY_RE = /^(sessionId|session_id)$/;

export function redactInvokeArgs(
  cmd: string,
  args?: Record<string, any>
): Record<string, RedactedValue> | undefined {
  void cmd;
  if (!args) return args;
  const redacted: Record<string, RedactedValue> = {};
  for (const [key, value] of Object.entries(args)) {
    redacted[key] = redactValue(value, key);
  }
  return redacted;
}

function redactValue(value: any, key: string): RedactedValue {
  if (value == null) return value;
  if (SESSION_KEY_RE.test(key) && typeof value === "string") return value;
  if (SECRET_KEY_RE.test(key)) return summarizeSensitive(value);
  if (PAYLOAD_KEY_RE.test(key)) return summarizeSensitive(value);
  if (TOOL_ARGUMENT_KEY_RE.test(key)) return summarizeSensitive(value);
  if (CONTENT_KEY_RE.test(key)) return summarizeSensitive(value);
  if (NOTES_KEY_RE.test(key)) return summarizeSensitive(value);

  if (Array.isArray(value)) {
    if (key === "messages") {
      return {
        type: "array",
        itemCount: value.length,
        items: value.map(item =>
          item && typeof item === "object"
            ? {
                role: typeof item.role === "string" ? item.role : summarizeSensitive(item.role),
                content: summarizeSensitive(item.content),
              }
            : summarizeSensitive(item)
        ),
      };
    }
    return {
      type: "array",
      itemCount: value.length,
      hash: stableHash(JSON.stringify(value)),
    };
  }

  if (typeof value === "object") {
    const output: Record<string, RedactedValue | string[] | undefined> = {
      type: "object",
      keys: Object.keys(value).sort(),
    };
    for (const [childKey, childValue] of Object.entries(value)) {
      output[childKey] = redactValue(childValue, childKey);
    }
    return output as RedactedValue;
  }

  if (typeof value === "string") {
    return summarizeSensitive(value);
  }
  return value;
}

function summarizeSensitive(value: any): RedactedValue {
  const serialized = typeof value === "string" ? value : JSON.stringify(value);
  return {
    redacted: true,
    type: Array.isArray(value) ? "array" : typeof value,
    keys:
      value && typeof value === "object" && !Array.isArray(value)
        ? Object.keys(value).sort()
        : undefined,
    length: serialized.length,
    hash: stableHash(serialized),
  };
}

function stableHash(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return `fnv1a:${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

function sessionArgs(sessionId: string): { sessionId: string; session_id: string } {
  return { sessionId, session_id: sessionId };
}

function selectedSkillArgs(
  selectedSkillId?: string
): { selectedSkillId: string; selected_skill_id: string } | undefined {
  const trimmed = selectedSkillId?.trim();
  return trimmed ? { selectedSkillId: trimmed, selected_skill_id: trimmed } : undefined;
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

export interface LifeModelChangeView {
  path: string;
  proposalId: string;
  proposalStatus: string;
  proposalSource: string;
  proposalSourceDetail?: string | null;
  proposalRunId?: string | null;
  sourceExcerpt?: string | null;
  sourceUnavailableReason?: string | null;
  confidence: number;
  riskLevel: string;
  before?: any;
  after: any;
  patchId?: string | null;
  patchStatus?: string | null;
  patchPath?: string | null;
  patchUnavailableReason?: string | null;
  snapshotVersions: string[];
  snapshotUnavailableReason?: string | null;
  currentMatchesAcceptedAfter: boolean;
}

export interface LifeModelCurrentView {
  path: string;
  label: string;
  value?: string | null;
  unavailableReason?: string | null;
  currentValueSource: string;
  change?: LifeModelChangeView | null;
}

export async function getLifeModelCurrentView(): Promise<LifeModelCurrentView> {
  return safeInvoke<LifeModelCurrentView>("get_life_model_current_view");
}

const MANUAL_LIFEMODEL_EDITOR_SAVE_REQUEST = {
  purpose: "manual_lifemodel_editor_save",
  explicitUserIntent: true,
  riskAcknowledged: true,
  createPreChangeSnapshot: true,
} as const;

const MANUAL_SNAPSHOT_RESTORE_REQUEST = {
  purpose: "manual_restore",
  explicitUserIntent: true,
  createPreChangeSnapshot: true,
} as const;

const MANUAL_DATA_IMPORT_REQUEST = {
  purpose: "manual_restore",
  explicitUserIntent: true,
  createPreChangeSnapshot: true,
  importTargets: ["life_model", "messages", "vectors"],
} as const;

export async function saveLifeModel(model: LifeModel): Promise<void> {
  return safeInvoke("save_life_model", {
    lifeModel: model,
    manualOverrideRequest: MANUAL_LIFEMODEL_EDITOR_SAVE_REQUEST,
    manual_override_request: MANUAL_LIFEMODEL_EDITOR_SAVE_REQUEST,
  });
}

export interface ChatProposalConfig {
  enabled?: boolean;
  confidence_threshold?: number;
  min_message_length?: number;
  cooldown_seconds?: number;
}

export type AgentRuntimeMode = "local_first_default" | "capability_first_beta";
export type CloudApiValidationStatus =
  | "unconfigured"
  | "unvalidated"
  | "validated"
  | "failed"
  | "stale";

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
  runtime_mode?: AgentRuntimeMode;
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
export interface MainChatMessageOptions {
  selectedSkillId?: string;
}

export async function sendMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions = {}
): Promise<string> {
  const result = await safeInvoke<SendMessageResult>("send_message", {
    ...sessionArgs(sessionId),
    messages,
    ...selectedSkillArgs(options.selectedSkillId),
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
  react_trace?: ReactActionTraceEnvelope;
  replayable?: boolean;
}

export interface ReactActionTraceEnvelope {
  runId?: string;
  actionId: string;
  stepIndex: number;
  toolCallIndex: number;
  actionType: string;
  toolId: string;
  toolName: string;
  toolSource: string;
  actionCategory: string;
  riskLevel: string;
  permissionDecision?: string;
  status: string;
  proposalId?: string;
  observationId?: string;
  observationStatus?: string;
  outputPreview?: string;
  outputHash?: string;
  outputByteCount?: number;
  outputItemCount?: number;
  startedAt?: string;
  finishedAt?: string;
  metadataSafe: boolean;
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
  selectedGuidanceIds?: string[];
  selectedGuidanceRefs?: SelectedGuidanceRef[];
  excludedAssets?: HSAssetExclusion[];
  estimatedTokens?: number;
  tokenBudget?: number;
}

export interface SelectedGuidanceRef {
  guidanceId: string;
  guidanceDigest: string;
  guidanceType: string;
  lifecycleStatus: string;
  domain: string;
  triggerDigest: string;
  selectedReason: string;
  impactKind: string;
  impactSummary: string;
  riskLevel: string;
  privacyLevel: string;
  sourceProposalId?: string;
  sourceEvidenceCount: number;
  sourceLineageDigest: string;
  policyBoundary: {
    hardPolicyBoundary: boolean;
    routePolicyRelaxed: boolean;
    toolPolicyRelaxed: boolean;
    proposalFirstPreserved: boolean;
    privacyConstraintCount: number;
    modelConstraintCount: number;
    toolConstraintCount: number;
    constraintDigest: string;
  };
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
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  legacy_fallback_used?: boolean;
}

export interface StreamMessageStartPayload {
  session_id: string;
  run_id: string;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  legacy_fallback_used?: boolean;
}

export interface StreamMessageDonePayload {
  session_id: string;
  run_id: string;
  reply: string;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  legacy_fallback_used?: boolean;
}

export type MainChatKernelEvent =
  | {
      type: "turn_started";
      session_id: string;
      selected_skill_id?: string | null;
    }
  | {
      type: "context_loaded";
      context_snapshot_ref: string;
      selected_source_count: number;
      selected_skill_instruction_loaded: boolean;
    }
  | {
      type: "hs_context_loaded";
      available: boolean;
      warning_count: number;
      selected_policy_count: number;
      accepted_guidance_count: number;
    }
  | {
      type: "route_selected";
      route_metadata: {
        provider: string;
        model: string;
        routeType: string;
        preferLocal: boolean;
        localModel: string;
        reason: string;
        privacyLevel: string;
        toolsEnabled: boolean;
        liveEvalRequired: boolean;
        finalAcceptanceGateRequired: boolean;
        readinessGateRequired: boolean;
        scriptedResponseConfigured: boolean;
      };
    }
  | {
      type: "final_answer";
      content_preview: string;
      content_chars: number;
    }
  | {
      type: "tool_decision";
      tool_name: string;
      action_type: string;
      target: string;
      reason: string;
      model_arguments_ignored: boolean;
    }
  | {
      type: "tool_observation";
      tool_name: string;
      status: string;
      output_preview: string;
      blocker?: string | null;
    }
  | {
      type: "write_intent_decision";
      outcome_kind: string;
      action_type: string;
      target: string;
      reason: string;
      requires_confirmation: boolean;
      hard_blocked: boolean;
    }
  | {
      type: "blocker";
      code: string;
    };

export type MainChatAgentProductStrategyRoute =
  | "direct_answer"
  | "read_action"
  | "react_tool_execution"
  | "plan_execute"
  | "memory_proposal"
  | "permission_request"
  | "task_control"
  | "blocked"
  | "legacy_fallback"
  | "unknown";

export interface MainChatAgentStateSnapshot {
  task: {
    taskId: string;
    runId: string;
    conversationId: string;
    userMessageId: string;
    title: string;
    strategy: MainChatAgentProductStrategyRoute;
    status: string;
    createdAt: string;
    updatedAt: string;
    traceAvailable: boolean;
    controls: string[];
    actionIds: string[];
    observationIds: string[];
    blockerIds: string[];
    proposalIds: string[];
    finalDeliveryId?: string;
  };
  route: { strategy: MainChatAgentProductStrategyRoute; reason: string; confidence?: number };
  context: Array<{
    contextId: string;
    sourceKind: string;
    sourceLabel: string;
    evidenceId: string;
  }>;
  provider?: {
    provider: string;
    model: string;
    routeType: string;
    reason: string;
    evidenceId: string;
  };
  plan?: {
    planId: string;
    planSessionId?: string | null;
    taskSessionId?: string | null;
    runId?: string | null;
    status: string;
    summary: string;
    editable: boolean;
    source: string;
    evidenceId: string;
    revision?: number | null;
    revisionId?: string | null;
    confirmedAt?: string | null;
    reviewId?: string | null;
    reviewSummary?: PlanExecuteReviewSummary | null;
    sourceEvidenceIds?: string[];
    supersededByPlanId?: string | null;
    controls?: string[];
    steps?: Array<{
      stepId: string;
      planId: string;
      index: number;
      title: string;
      description: string;
      kind: string;
      status: string;
      revision: number;
      basePlanRevision: number;
      linkedActionIds: string[];
      linkedObservationIds: string[];
      linkedProposalIds: string[];
      blockerIds: string[];
      linkedFinalDeliveryIds?: string[];
      skipReason?: string | null;
      policyDecisionId?: string | null;
      reason?: string | null;
      evidenceIds?: string[];
      controls?: string[];
    }>;
  };
  actions: Array<{
    actionId: string;
    actionType: string;
    target: string;
    label: string;
    status: string;
    riskLevel: string;
    policyDecisionId: string;
    startedAt?: string;
    finishedAt?: string;
    observationIds: string[];
    retryable: boolean;
  }>;
  observations: Array<{
    observationId: string;
    actionId: string;
    sourceKind: string;
    sourceLabel: string;
    preview: string;
    citationAvailable: boolean;
    readExecution?: {
      kind: string;
      sourceKind: string;
      sourceLabel: string;
      target: string;
      realReadOnlyExecution: boolean;
      fixtureBacked: boolean;
      networkReadAttempted: boolean;
      directWritesExecuted: boolean;
    };
    createdAt: string;
  }>;
  blockers: Array<{
    blockerId: string;
    reasonCode: string;
    title: string;
    detail: string;
    affectedActionId?: string;
    recoverable: boolean;
    controls: string[];
  }>;
  proposals: Array<{
    proposalId: string;
    proposalType: string;
    status: string;
    title: string;
    summary: string;
    evidenceIds: string[];
    actionIds: string[];
    controls: string[];
    memoryLifecycle?: MemoryLifecycleRecord;
  }>;
  finalDelivery?: {
    deliveryId: string;
    taskId: string;
    runId: string;
    status: string;
    headline: string;
    answer: string;
    completedActions: unknown[];
    observationsUsed: unknown[];
    proposalsCreated: unknown[];
    blockers: unknown[];
    skippedWork?: unknown[];
    pendingUserActions: unknown[];
    durableChanges: unknown[];
    nextSteps: string[];
    traceAvailable: boolean;
  };
  diagnostics: Array<{ gapId: string; gapCode: string; detail: string; evidenceId?: string }>;
  sequence: number;
  emittedAt: string;
  events: Array<{ eventType: string; sequence: number; objectId: string; evidenceId: string }>;
}

export interface MainChatAgentDurableEvent {
  eventId: string;
  taskSessionId: string;
  runId: string;
  sequence: number;
  eventType: string;
  objectType: string;
  objectId: string;
  createdAt: string;
  source: string;
  payloadDigest: string;
  payload: Record<string, unknown> | null;
  backfilled: boolean;
}

export type MainChatAgentStrategy =
  | "direct_answer"
  | "react_tool_execution"
  | "plan_execute"
  | "memory_proposal"
  | "life_model_proposal"
  | "review_maturation"
  | "blocked_confirmation";

export interface MainChatPrivacyRiskSummary {
  riskLevel: string;
  privacyClass: string;
  policyReasonCode: string;
  localOnlyRequired: boolean;
  writeLike: boolean;
  externalWriteLike: boolean;
}

export interface MainChatAgentIngressDecision {
  requestId: string;
  sourceSessionId: string;
  taskKind: string;
  selectedStrategy: MainChatAgentStrategy;
  confidence: number;
  reasonSummary: string;
  fallbackEligible: boolean;
  privacyRisk: MainChatPrivacyRiskSummary;
  agentTaskSessionId?: string;
}

export type MainChatExecutionTranscriptKind =
  | "user_input"
  | "route_decision"
  | "plan"
  | "action"
  | "observation"
  | "follow_up"
  | "permission_request"
  | "proposal_request"
  | "error"
  | "retry"
  | "final_result"
  | "fallback";

export interface MainChatExecutionTranscriptEntry {
  id: string;
  sessionId: string;
  kind: MainChatExecutionTranscriptKind;
  summary: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
}

export type MainChatAgentTaskStatus =
  | "running"
  | "waiting_permission"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled";

export type MainChatExecutionQueueStatus =
  | "planned"
  | "pending_permission"
  | "executing"
  | "observed"
  | "failed"
  | "retrying"
  | "cancelled"
  | "completed";

export interface MainChatExecutionAction {
  actionType: string;
  description: string;
}

export interface MainChatExecutionPolicyDecision {
  level: string;
  reasonCode: string;
  executionAllowed: boolean;
  requiresConfirmation: boolean;
  requiresProposal: boolean;
  requiresBlocker: boolean;
  silentWriteAllowed: boolean;
}

export interface MainChatQueuedExecutionAction {
  id: string;
  sessionId: string;
  action: MainChatExecutionAction;
  policy: MainChatExecutionPolicyDecision;
  status: MainChatExecutionQueueStatus;
  attempts: number;
  observationMetadata?: Record<string, unknown>;
  error?: string;
  createdAt: string;
  updatedAt: string;
}

export interface MainChatAgentTaskSession {
  id: string;
  chatSessionId: string;
  userGoal: string;
  selectedStrategy: MainChatAgentStrategy;
  status: MainChatAgentTaskStatus;
  currentPlanSummary?: string;
  actionQueueIds: string[];
  pendingBlockers: string[];
  contextSnapshotRefs: string[];
  createdAt: string;
  updatedAt: string;
  finalSummary?: string;
}

export interface MainChatAgentTaskState {
  session?: MainChatAgentTaskSession | null;
  actions: MainChatQueuedExecutionAction[];
  transcript: MainChatExecutionTranscriptEntry[];
  pendingApprovalCount: number;
  activeToolCount: number;
  canResume: boolean;
  canCancel: boolean;
  canRetry: boolean;
}

export interface MainChatAgentTaskFilter {
  statuses?: MainChatAgentTaskStatus[];
  conversationId?: string | null;
  includeTerminal?: boolean;
  includeStale?: boolean;
}

export interface MainChatTaskSummary {
  taskSessionId: string;
  conversationId: string;
  runId: string;
  title: string;
  strategy: MainChatAgentStrategy;
  status: MainChatAgentTaskStatus;
  lastUpdatedAt: string;
  lastObservationPreview: string;
  pendingBlockerCount: number;
  pendingProposalCount: number;
  nextRecommendedControl: string;
  staleState: string;
  resumeSafetyDigest: string;
  lifecycleState?: string;
  lastSafeEvent?: string | null;
  actionCount?: number;
  observationCount?: number;
  allowedControls?: string[];
  redactionState?: string;
  routeEvidence?: RuntimeRouteEvidence | null;
  evidenceView?: RunEvidenceView;
}

export interface MainChatContinuityDiagnostics {
  staleContext: boolean;
  missingActionEvidence: boolean;
  permissionScopeMismatch: boolean;
  terminalNoResume: boolean;
  providerUnavailable: boolean;
  toolUnavailable: boolean;
  requiresUserDecision: boolean;
  selectedSkillContextDigestMismatch?: boolean;
  planRevisionMismatch?: boolean;
  reasonCodes: string[];
  automaticReplayAllowed: boolean;
}

export interface MainChatTaskDetail {
  taskSession: MainChatAgentTaskSession;
  actions: MainChatQueuedExecutionAction[];
  transcript: MainChatExecutionTranscriptEntry[];
  proposals: AgentProposal[];
  blockers: string[];
  finalDelivery?: Record<string, unknown> | null;
  continuityDiagnostics: MainChatContinuityDiagnostics;
  allowedControls: string[];
  nextRecommendedControl: string;
  lastSafeResumePoint?: string | null;
  contextDigest: string;
  selectedSkillDigest?: string | null;
  toolManifestDigest: string;
  evidenceView?: RunEvidenceView;
}

export interface RunEvidenceTimelineEvent {
  id: string;
  kind: string;
  summary: string;
  createdAt?: string | null;
  failureKind?: string | null;
  normalizedLifecycleState?: string | null;
  sourceRef?: string | null;
}

export interface RunEvidenceView {
  runId?: string | null;
  taskSessionId: string;
  title: string;
  lifecycleState: string;
  routeEvidence?: RuntimeRouteEvidence | null;
  eventTimeline: RunEvidenceTimelineEvent[];
  actionCount: number;
  observationCount: number;
  blockers: string[];
  proposals: string[];
  planRefs: string[];
  allowedControls: string[];
  nextRecommendedControl: string;
  redactionState: string;
}

export interface MainChatRuntimeEvalReport {
  totalCases: number;
  runtimeExecutedCaseCount: number;
  deterministicStubCaseCount: number;
  passedCases: number;
  failedCases: number;
  silentWriteCount: number;
  finalCompletionReady: boolean;
  finalCompletionBlockers: string[];
  failures: unknown[];
  [key: string]: unknown;
}

export interface MainChatAgentExecutionV1AcceptanceReport {
  ready: boolean;
  status: string;
  blockers: string[];
  requiredEvidence: string[];
  runtimeGateReady: boolean;
  commandSurfaceGateReady: boolean;
  liveProviderGateReady: boolean;
  directWritesExecuted: boolean;
}

export interface MainChatLiveProviderEvalPreflightReport {
  ready: boolean;
  status: string;
  provider: string;
  blockers: string[];
  requiredEvidence: string[];
  liveProviderInvocationAllowed: boolean;
  modelInvoked: boolean;
  directWritesExecuted: boolean;
}

export interface MainChatAgentExecutionV1EvalGateReport {
  reportKind: "main_chat_agent_execution_v1_eval_gate";
  runtimeEval: MainChatRuntimeEvalReport;
  acceptance: MainChatAgentExecutionV1AcceptanceReport;
  liveProviderPreflight: MainChatLiveProviderEvalPreflightReport;
  commandSurfaceGateExecuted: boolean;
  liveProviderAttempted: boolean;
  migrationPermission: boolean;
  metadataSafe: boolean;
  noExternalProviderInvocation: boolean;
  noAppStoreWrites: boolean;
  metadataSafeSummary: Record<string, unknown>;
}

export interface MainChatAgentProductizationRouteCount {
  passed: number;
  failed: number;
  expectedBlocker: number;
  unsupported: number;
}

export interface MainChatAgentProductizationUnsupportedScenario {
  scenarioId: string;
  route: string;
  reason: string;
}

export interface MainChatAgentProductizationFailedScenario {
  scenarioId: string;
  route: string;
  reason: string;
}

export interface ProductScenarioRuntimeProof {
  scenarioId: string;
  group: string;
  passed: boolean;
  runtimeObjectCount: number;
  observationCount: number;
  createdActionIds: string[];
  createdObservationIds: string[];
  createdProposalIds: string[];
  createdMemoryIds: string[];
  rollbackEventIds: string[];
  materializedViewVersions: number[];
  inactiveMemoryIds: string[];
  finalDeliveryId?: string;
  diagnostics: string[];
}

export interface MainChatAgentProductizationV1GateReport {
  totalScenarioCount: number;
  defaultDeterministicScenarioCount: number;
  readinessSemantics: "full_deterministic_productization_v1_runtime_ready";
  runtimeExecutionScope: "default_deterministic_scenarios_runtime_backed_external_live_excluded";
  executedScenarioCount: number;
  passedScenarioCount: number;
  expectedBlockerScenarioCount: number;
  failedScenarioCount: number;
  externalLiveExcludedCount: number;
  runtimePayloadSnapshotEventGatePassed: boolean;
  runtimeRequiredGroupCount: number;
  runtimeRequiredGroupPassedCount: number;
  representativeRuntimeGroupCount: number;
  representativeRuntimeGroupPassedCount: number;
  fullDeterministicRuntimeScenarioCount: number;
  fullDeterministicRuntimeScenarioExecutedCount: number;
  runtimeRequiredGroupEvidence: ProductScenarioRuntimeProof[];
  eventSemantics: string;
  finalReadinessReady: boolean;
  fullProductizationV1Complete: boolean;
  futureWork: string[];
  routeCounts: Record<string, MainChatAgentProductizationRouteCount>;
  unsupportedScenarios: MainChatAgentProductizationUnsupportedScenario[];
  failedScenarios: MainChatAgentProductizationFailedScenario[];
  blockers: string[];
}

export interface MainChatLiveProductScenarioProof {
  scenarioId: string;
  passed: boolean;
  status: string;
  provider: string;
  providerModel?: string | null;
  providerEndpointKind: string;
  taskSessionId?: string | null;
  runId?: string | null;
  actionIds: string[];
  observationIds: string[];
  proposalIds: string[];
  blockerIds: string[];
  finalDeliveryId?: string | null;
  eventTypes: string[];
  eventSequenceStart?: number | null;
  eventSequenceEnd?: number | null;
  uiStateAssertions: string[];
  runtimeEvidence: string[];
  controls: string[];
  negativeAssertions: string[];
  blockers: string[];
}

export interface MainChatExternalLiveProductizationGateReport {
  reportKind: "main_chat_external_live_productization_gate";
  scenarioCount: number;
  defaultGateScenarioCount: number;
  readinessSemantics: "opt_in_external_live_product_evidence_only_default_readiness_unchanged";
  runMode: "external_live_opt_in";
  liveProviderAttempted: boolean;
  passedScenarioCount: number;
  blockedScenarioCount: number;
  failedScenarioCount: number;
  ready: boolean;
  externalProviderInvoked: boolean;
  directWritesExecuted: boolean;
  legacyFallbackUsed: boolean;
  deterministicReadinessUnchanged: boolean;
  blockers: string[];
  proofs: MainChatLiveProductScenarioProof[];
}

export interface MainChatProductMaturityV2EventProof {
  scenarioId: string;
  capabilityGroup: string;
  passed: boolean;
  runtimeObjectCount: number;
  emittedEventIds: string[];
  replayedEventIds: string[];
  emittedSequences: number[];
  replayedSequences: number[];
  uiState: string[];
  diagnostics: string[];
}

export interface MainChatProductMaturityV2EventGateReport {
  scenarioCount: number;
  defaultGateScenarioCount: number;
  passedScenarioCount: number;
  expectedBlockerCount: number;
  ready: boolean;
  blockers: string[];
  proofs: MainChatProductMaturityV2EventProof[];
}

export interface MainChatProductMaturityV2PlanScenario {
  id: string;
  capabilityGroup: string;
  prompt: string;
  preconditions: string[];
  expectedRoute: string;
  requiredRuntimeEvidence: string[];
  requiredUiState: string[];
  requiredControls: string[];
  negativeAssertions: string[];
  expectedOutcome: string;
  defaultGate: boolean;
}

export interface MainChatProductMaturityV2PlanProof {
  scenarioId: string;
  passed: boolean;
  expectedBlocker: boolean;
  planId?: string | null;
  revision?: number | null;
  stepIds: string[];
  eventTypes: string[];
  linkedActionIds: string[];
  linkedObservationIds: string[];
  linkedProposalIds: string[];
  blockerIds: string[];
  controls: string[];
  diagnostics: string[];
}

export interface MainChatProductMaturityV2PlanGateReport {
  scenarioCount: number;
  defaultGateScenarioCount: number;
  passedScenarioCount: number;
  expectedBlockerCount: number;
  ready: boolean;
  blockers: string[];
  scenarios: MainChatProductMaturityV2PlanScenario[];
  proofs: MainChatProductMaturityV2PlanProof[];
}

export interface MainChatSkillSummary {
  skillId: string;
  name: string;
  source: string;
  scope: string;
  description: string;
  riskLevel: string;
  available: boolean;
  selected: boolean;
  instructionDigest: string;
  sourceKind: "global" | "workspace" | "project" | "bundled" | string;
  lastUsedAt?: string | null;
}

export interface MainChatSkillDetail {
  skillId: string;
  manifest: Record<string, unknown>;
  boundedInstructionsPreview: string;
  allowedTools: string[];
  disallowedTools: string[];
  policyNotes: string[];
  requiredPermissions: string[];
  evidenceDigest: string;
  redactionSummary: string;
  lastModifiedAt?: string | null;
}

export interface MainChatSelectedSkill {
  sessionId: string;
  selectedSkillId?: string | null;
  selectedSkillDigest?: string | null;
  selectionReason: string;
  boundedInstructionsPreview: string;
  evidenceDigest: string;
  policyNotes: string[];
  includedAsBoundedContextOnly: boolean;
  unselectedSkillsInjected: boolean;
  controls: string[];
}

export interface MainChatToolCandidate {
  candidateId: string;
  toolName: string;
  source: string;
  capabilityLabels: string[];
  riskLevel: string;
  selectionReason: string;
  policyDecision: string;
  requiresPermission: boolean;
  candidateDigest: string;
  linkedActionId?: string | null;
}

export interface MainChatBlockedTool {
  toolName: string;
  reasonCode: string;
  policyDecision: string;
  requiresPermission: boolean;
  blockerId?: string | null;
}

export interface MainChatToolFailureRecovery {
  failedCandidateId: string;
  failureReason: string;
  retryAvailable: boolean;
  alternativeCandidateId?: string | null;
  controls: string[];
}

export interface MainChatToolCandidateList {
  taskSessionId?: string | null;
  candidates: MainChatToolCandidate[];
  blockedTools: MainChatBlockedTool[];
  failureRecovery?: MainChatToolFailureRecovery | null;
  evidenceDigest: string;
  controls: string[];
}

export interface MainChatProductMaturityV2SkillsProof {
  scenarioId: string;
  passed: boolean;
  expectedBlocker: boolean;
  runtimeObjectCount: number;
  selectedSkillIds: string[];
  candidateIds: string[];
  blockerIds: string[];
  actionIds: string[];
  observationIds: string[];
  controls: string[];
  runtimeEvidence: string[];
  uiState: string[];
  negativeAssertions: string[];
  diagnostics: string[];
}

export interface MainChatProductMaturityV2SkillsScenario {
  id: string;
  capabilityGroup: string;
  prompt: string;
  preconditions: string[];
  expectedRoute: string;
  requiredRuntimeEvidence: string[];
  requiredUiState: string[];
  requiredControls: string[];
  negativeAssertions: string[];
  expectedOutcome: string;
  defaultGate: boolean;
}

export interface MainChatProductMaturityV2SkillsGateReport {
  scenarioCount: number;
  defaultGateScenarioCount: number;
  passedScenarioCount: number;
  expectedBlockerCount: number;
  ready: boolean;
  blockers: string[];
  scenarios: MainChatProductMaturityV2SkillsScenario[];
  proofs: MainChatProductMaturityV2SkillsProof[];
}

export interface MainChatProductMaturityV2ScenarioStatus {
  scenarioId: string;
  phaseId: string;
  capabilityGroup: string;
  status: string;
  reason: string;
}

export interface MainChatProductMaturityV2PhaseCount {
  phaseId: string;
  phaseLabel: string;
  capabilityGroup: string;
  scenarioCount: number;
  passed: number;
  expectedBlocker: number;
  failed: number;
  blocked: number;
  status: string;
  ready: boolean;
  defaultGate: boolean;
  optInOnly: boolean;
  blockers: string[];
  supportedScenarios: string[];
  blockedScenarios: string[];
  unsupportedScenarios: string[];
  futureScenarios: string[];
}

export interface MainChatProductMaturityV2FinalReadinessReport {
  reportKind: "main_chat_agent_product_maturity_v2_final_readiness_gate";
  readinessSemantics: "phase_g_final_readiness_default_deterministic_live_product_opt_in_separate";
  defaultReadinessScope: "MR_EV_PI_LT2_SK2_deterministic_only";
  optInLiveReadinessScope: "LIVE_PROD_external_live_opt_in_only";
  finalReady: boolean;
  deterministicReady: boolean;
  optInLiveReady: boolean;
  finalReadinessStatus: string;
  deterministicReadinessStatus: string;
  optInLiveReadinessStatus: string;
  defaultDeterministicScenarioCount: number;
  defaultLiveProdExcludedCount: number;
  externalLiveScenarioCount: number;
  defaultScenarioPassedCount: number;
  defaultScenarioExpectedBlockerCount: number;
  defaultScenarioFailedCount: number;
  defaultScenarioBlockedCount: number;
  externalLivePassedCount: number;
  externalLiveBlockedCount: number;
  externalLiveFailedCount: number;
  phaseCounts: MainChatProductMaturityV2PhaseCount[];
  supportedScenarios: MainChatProductMaturityV2ScenarioStatus[];
  blockedScenarios: MainChatProductMaturityV2ScenarioStatus[];
  unsupportedScenarios: MainChatProductMaturityV2ScenarioStatus[];
  futureScenarios: MainChatProductMaturityV2ScenarioStatus[];
  blockers: string[];
  deterministicBlockers: string[];
  optInLiveBlockers: string[];
  directWritesExecuted: boolean;
  noSilentDurableWrites: boolean;
  defaultLiveProdExcluded: boolean;
}

export interface MainChatAgentBetaV1ReadinessDimension {
  dimension: string;
  status: string;
  optInOnly: boolean;
  evidence: string[];
  blockers: string[];
}

export interface MainChatAgentBetaV1FoundationInventoryItem {
  component: string;
  status: string;
  evidence: string[];
  developmentDecision: string;
}

export interface MainChatAgentBetaV1WorkstreamStatus {
  workstreamId: string;
  label: string;
  status: string;
  ready: boolean;
  evidence: string[];
  blockers: string[];
}

export interface MainChatAgentBetaV1ProductMaturityPhaseCount {
  phaseId: string;
  capabilityGroup: string;
  scenarioCount: number;
  passed: number;
  expectedBlocker: number;
  failed: number;
  blocked: number;
  ready: boolean;
  optInOnly: boolean;
}

export interface MainChatAgentBetaV1ReadinessReport {
  reportKind: "main_chat_agent_beta_v1_readiness_gate";
  readinessSemantics: "beta_v1_execution_first_default_deterministic_live_opt_in_separate";
  defaultReadinessScope: "beta_v1_default_deterministic_local_only";
  optInLiveReadinessScope: "beta_v1_external_live_opt_in_only";
  foundationInventoryExists: boolean;
  foundationInventoryItems: MainChatAgentBetaV1FoundationInventoryItem[];
  workstreams: MainChatAgentBetaV1WorkstreamStatus[];
  productMaturityPhaseCounts: MainChatAgentBetaV1ProductMaturityPhaseCount[];
  defaultReadinessStatus: string;
  defaultReady: boolean;
  optInLiveReady: boolean;
  externalLiveAttempted: boolean;
  defaultRealTaskScenarioCount: number;
  defaultRealTaskPassedCount: number;
  optInLiveRealTaskScenarioCount: number;
  defaultExperienceRequiredStateCount: number;
  defaultExperienceVerifiedStateCount: number;
  productMaturityDefaultScenarioCount: number;
  commandSurfaceTotalCases: number;
  commandSurfaceFailedCases: number;
  legacyFallbackCount: number;
  silentDurableWriteCount: number;
  noSilentDurableWrites: boolean;
  defaultBlockers: string[];
  optInLiveBlockers: string[];
  readinessDimensions: MainChatAgentBetaV1ReadinessDimension[];
}

export interface MainChatAgentStage2ManualDogfoodSummary {
  attempted: boolean;
  ready: boolean;
  reviewerCount: number;
  requiredScenarioCount: number;
  attemptedScenarioCount: number;
  passedScenarioCount: number;
  missingScenarioIds: string[];
  failedScenarioIds: string[];
  traceIdsPresent: boolean;
  artifactDigest?: string | null;
  blockers: string[];
}

export interface MainChatAgentStage2LiveProviderSummary {
  attempted: boolean;
  ready: boolean;
  provider?: string | null;
  model?: string | null;
  requiredScenarioCount: number;
  passedScenarioCount: number;
  failedScenarioIds: string[];
  modelInvokedCount: number;
  mainChatInvokedCount: number;
  localOrMockCreditRejected: number;
  artifactDigest?: string | null;
  blockers: string[];
  scenarioPlans: MainChatAgentStage2LiveProviderScenarioPlan[];
  scenarioReports: MainChatAgentStage2LiveProviderScenarioReport[];
}

export interface MainChatAgentStage2LiveProviderScenarioPlan {
  scenarioId: string;
  scenario: string;
  scenarioSetup: string;
  requiredRuntimeEvidence: string[];
  failClosedBlocker: string;
  executionSource: string;
  runnerStatus: string;
}

export interface MainChatAgentStage2LiveProviderScenarioReport {
  scenarioId: string;
  status: string;
  credited: boolean;
  providerEndpointKind?: string | null;
  blockers: string[];
  mainChatInvoked: boolean;
  modelInvoked: boolean;
  runIdPresent: boolean;
  taskSessionIdPresent: boolean;
  responsePreviewPresent: boolean;
}

export interface MainChatAgentStage2CoverageItem {
  id: string;
  passed: boolean;
  evidence: string[];
  blockers: string[];
}

export interface MainChatAgentStage2CoverageSummary {
  ready: boolean;
  requiredCount: number;
  attemptedCount: number;
  passedCount: number;
  failedIds: string[];
  coverage: MainChatAgentStage2CoverageItem[];
  blockers: string[];
}

export interface MainChatAgentStage2FinalDeliverySummary {
  ready: boolean;
  p0ScenarioCount: number;
  finalDeliveryEvidenceCount: number;
  finalDoneOverclaimCount: number;
  blockers: string[];
}

export interface MainChatAgentStage2SafetySummary {
  silentDurableWriteCount: number;
  hiddenLegacyFallbackCount: number;
  fakeBrowserEvidenceCount: number;
  fakeLiveEvidenceCount: number;
  localProviderCreditedAsLiveCount: number;
  unscopedPermissionReplayCount: number;
  finalDoneOverclaimCount: number;
}

export interface MainChatAgentStage2ArtifactRef {
  kind: string;
  path: string;
  digest?: string | null;
  status: string;
}

export interface MainChatAgentStage2ReadinessReport {
  reportKind: "main_chat_agent_stage2_readiness_gate";
  schemaVersion: "stage2-readiness-v1";
  runId: string;
  commit: string;
  recommendation: "ready_for_limited_internal_trial" | "not_ready_for_limited_internal_trial";
  implementationStatus:
    | "implementation_complete_for_stage2_mechanism"
    | "implementation_incomplete_for_stage2_mechanism"
    | "ready_for_limited_internal_trial";
  blockers: string[];
  deterministicStage1Ready: boolean;
  betaFoundationReady: boolean;
  manualDogfood: MainChatAgentStage2ManualDogfoodSummary;
  liveProvider: MainChatAgentStage2LiveProviderSummary;
  controlPlane: MainChatAgentStage2CoverageSummary;
  memoryProposal: MainChatAgentStage2CoverageSummary;
  failureRecovery: MainChatAgentStage2CoverageSummary;
  finalDelivery: MainChatAgentStage2FinalDeliverySummary;
  safety: MainChatAgentStage2SafetySummary;
  artifacts: MainChatAgentStage2ArtifactRef[];
}

export interface MainChatStage3ExecutionUxCoverageRow {
  scenarioId: string;
  scenario: string;
  status: "passed" | "failed" | "blocked";
  evidence: string[];
  blockers: string[];
}

export interface MainChatStage3ExecutionUxReport {
  reportKind: "main_chat_stage3_execution_ux";
  schemaVersion: "stage3-execution-ux-v1";
  dataPath: string;
  totalScenarioCount: number;
  passedScenarioCount: number;
  failedScenarioCount: number;
  blockedScenarioCount: number;
  executionFirstRequiredIds: string[];
  executionFirstPassedIds: string[];
  executionFirstClaimValid: boolean;
  readyForLimitedInternalTrial: false;
  readinessRecommendation: "not_ready_for_limited_internal_trial";
  stage2ReadinessPreserved: string;
  nonGoals: string[];
  coverage: MainChatStage3ExecutionUxCoverageRow[];
  blockers: string[];
}

export interface MainChatAgentStage1SeedManifest {
  seedWorkspaceRootKind: "temp_isolated";
  knowledgeAssetCount: number;
  skillCount: number;
  sessionSeedCount: number;
  memorySeedCount: number;
  proposalSeedCount: number;
  taskSeedCount: number;
  planSeedCount: number;
  mcpManifestSeedCount: number;
  webFixtureSeedCount: number;
  seedDigest: string;
  fileDigests: Record<string, string>;
  runtimeObjectDigests: Record<string, string>;
  secretsDetected: boolean;
}

export interface MainChatAgentStage1DogfoodScenarioEvidence {
  scenarioId: string;
  scenarioType: string;
  entryPoint: string;
  scenarioPromptId: string;
  boundedPromptPreview: string;
  userPromptDigest: string;
  taskSessionId: string;
  runId: string;
  routeStrategy: string;
  expectedOutcome: string;
  actualOutcome: string;
  runtimeEvents: string[];
  actions: string[];
  observations: string[];
  proposals: string[];
  blockers: string[];
  uiStates: string[];
  finalDeliverySections: string[];
  controlEvidence: string;
  runtimeEvidencePassed: boolean;
  uiEvidencePassed: boolean;
  finalDeliveryEvidencePassed: boolean;
  nonFakeEvidencePassed: boolean;
  legacyFallbackUsed: boolean;
  silentDurableWriteDetected: boolean;
  fakeExecutionDetected: boolean;
  seedManifestDigest: string;
  liveProviderEvidence?: string | null;
  passed: boolean;
  failureReason?: string | null;
}

export interface MainChatAgentStage1DogfoodReport {
  reportKind: "main_chat_agent_stage1_dogfood_gate";
  readinessSemantics: string;
  defaultReadinessScope: "stage1_default_deterministic_seeded_dogfood";
  optInLiveReadinessScope: "stage1_external_live_opt_in_only";
  defaultReady: boolean;
  optInLiveReady: boolean;
  readinessRecommendation: string;
  scenarioCount: number;
  defaultScenarioCount: number;
  defaultPassedCount: number;
  defaultFailedCount: number;
  taskSessionCreatedCount: number;
  ordinaryChatScenarioCount: number;
  seededTaskControlScenarioCount: number;
  uiVerifiedScenarioCount: number;
  finalDeliveryVerifiedScenarioCount: number;
  legacyFallbackCount: number;
  silentDurableWriteCount: number;
  fakeExecutionDetectedCount: number;
  externalLiveAttempted: boolean;
  externalLiveScenarioCount: number;
  externalLivePassedCount: number;
  externalLiveBlockedCount: number;
  externalLiveBlockers: string[];
  defaultReadinessUnaffectedByLive: boolean;
  browserE2eEnvironmentReady: boolean;
  browserE2eReportPath?: string | null;
  browserE2eRequiredJourneyCount: number;
  browserE2ePassedJourneyCount: number;
  browserE2eFailedJourneyCount: number;
  manualDogfoodStatus: string;
  betaV1DefaultReady: boolean;
  productMaturityDefaultScenarioCount: number;
  seedManifest: MainChatAgentStage1SeedManifest;
  scenarios: MainChatAgentStage1DogfoodScenarioEvidence[];
  blockers: string[];
  acceptedResidualRisks: string[];
}

export interface MainChatStep6JourneyReport {
  journeyId: string;
  status: string;
  credited: boolean;
  blockedLiveEvidenceReport: boolean;
  evidenceSource: string;
  answerEvidenceCount: number;
  runtimeEvidenceCount: number;
  uiStateCount: number;
  finalDeliverySectionCount: number;
  blockers: string[];
}

export interface MainChatStep6FinalGateSummary {
  collected: boolean;
  finalAcceptanceReady: boolean;
  finalAcceptanceBlockers: string[];
  commandSurfaceLegacyFallbackCount: number;
  commandSurfaceSilentWriteCount: number;
  liveProviderAttempted: boolean;
  liveProviderReadyCount: number;
  liveProviderWebCredit: boolean;
  liveProviderMcpCredit: boolean;
  liveProviderScenarioReports: unknown[];
  liveProviderBlockers: string[];
  blockers: string[];
}

export interface MainChatStep6ProductAcceptanceReport {
  reportKind: "main_chat_step6_product_acceptance_gate";
  schemaVersion: "step6-product-acceptance-v1";
  overallReady: boolean;
  localDeterministicReady: boolean;
  externalLiveReady: boolean;
  browserE2eEnvironmentReady: boolean;
  browserE2eReportPath?: string | null;
  requiredJourneyCount: number;
  localJourneyCount: number;
  externalLiveJourneyCount: number;
  passedJourneyCount: number;
  blockedLiveJourneyCount: number;
  failedJourneys: string[];
  noSilentDurableWrite: boolean;
  noHiddenLegacyFallback: boolean;
  noLocalFixtureMarkedExternalLive: boolean;
  noLocalEvidenceCreditedAsExternalLive: boolean;
  noInventedUnavailableEvidence: boolean;
  uiStatusFromStructuredEvidence: boolean;
  finalGateSummary: MainChatStep6FinalGateSummary;
  journeys: MainChatStep6JourneyReport[];
  blockers: string[];
}

export interface MainChatStep6LiveProviderEvalStatePrepReport {
  reportKind: "main_chat_step6_live_provider_eval_state_prep";
  configured: boolean;
  ready: boolean;
  debugBuild: boolean;
  explicitLiveEvalRequested: boolean;
  provider: string;
  model: string;
  baseConfigured: boolean;
  apiKeyPresent: boolean;
  networkEnabled: boolean;
  providerEndpointKind: string;
  preflightReady: boolean;
  preflightBlockers: string[];
  appConfigPersisted: boolean;
  directWritesExecuted: boolean;
  blockers: string[];
}

export async function listMainChatSkills(sessionId?: string): Promise<MainChatSkillSummary[]> {
  return safeInvoke<MainChatSkillSummary[]>("list_main_chat_skills", {
    ...optionalDualArg("sessionId", "session_id", sessionId),
  });
}

export async function getMainChatSkillDetail(skillId: string): Promise<MainChatSkillDetail> {
  return safeInvoke<MainChatSkillDetail>("get_main_chat_skill_detail", {
    skillId,
    skill_id: skillId,
  });
}

export async function selectMainChatSkill(
  sessionId: string,
  skillId: string
): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("select_main_chat_skill", {
    sessionId,
    session_id: sessionId,
    skillId,
    skill_id: skillId,
  });
}

export async function clearMainChatSkill(sessionId: string): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("clear_main_chat_skill", {
    sessionId,
    session_id: sessionId,
  });
}

export async function listMainChatToolCandidates(
  taskSessionId?: string
): Promise<MainChatToolCandidateList> {
  return safeInvoke<MainChatToolCandidateList>("list_main_chat_tool_candidates", {
    ...optionalDualArg("taskSessionId", "task_session_id", taskSessionId),
  });
}

export async function sendMessageV2(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions = {}
): Promise<SendMessageResult> {
  return safeInvoke<SendMessageResult>("send_message", {
    ...sessionArgs(sessionId),
    messages,
    ...selectedSkillArgs(options.selectedSkillId),
  });
}

export async function getMainChatAgentTaskState(
  taskSessionId: string
): Promise<MainChatAgentTaskState> {
  return safeInvoke<MainChatAgentTaskState>("get_main_chat_agent_task_state", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function listMainChatAgentTasks(
  filter?: MainChatAgentTaskFilter,
  limit = 50,
  offset = 0
): Promise<MainChatTaskSummary[]> {
  return safeInvoke<MainChatTaskSummary[]>("list_main_chat_agent_tasks", {
    filter: filter ?? null,
    limit,
    offset,
  });
}

export async function getMainChatAgentTaskDetail(
  taskSessionId: string
): Promise<MainChatTaskDetail> {
  return safeInvoke<MainChatTaskDetail>("get_main_chat_agent_task_detail", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function refreshMainChatAgentTaskContext(
  taskSessionId: string
): Promise<MainChatTaskDetail> {
  return safeInvoke<MainChatTaskDetail>("refresh_main_chat_agent_task_context", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function resumeMainChatAgentTask(
  taskSessionId: string
): Promise<MainChatAgentTaskState> {
  return safeInvoke<MainChatAgentTaskState>("resume_main_chat_agent_task", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function cancelMainChatAgentTask(
  taskSessionId: string
): Promise<MainChatAgentTaskState> {
  return safeInvoke<MainChatAgentTaskState>("cancel_main_chat_agent_task", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function retryMainChatAgentAction(
  taskSessionId: string,
  actionId: string
): Promise<MainChatAgentTaskState> {
  return safeInvoke<MainChatAgentTaskState>("retry_main_chat_agent_action", {
    taskSessionId,
    task_session_id: taskSessionId,
    actionId,
    action_id: actionId,
  });
}

export async function listMainChatAgentEvents(
  taskSessionId: string,
  afterSequence: number = 0,
  limit: number = 100
): Promise<MainChatAgentDurableEvent[]> {
  return safeInvoke<MainChatAgentDurableEvent[]>("list_main_chat_agent_events", {
    taskSessionId,
    task_session_id: taskSessionId,
    afterSequence,
    after_sequence: afterSequence,
    limit,
  });
}

export async function getMainChatAgentStateSnapshot(
  taskSessionId: string
): Promise<MainChatAgentStateSnapshot> {
  return safeInvoke<MainChatAgentStateSnapshot>("get_main_chat_agent_state_snapshot", {
    taskSessionId,
    task_session_id: taskSessionId,
  });
}

export async function runMultiStrategyAgentPreview(
  input: MultiStrategyAgentPreviewInput
): Promise<MultiStrategyAgentPreviewOutput> {
  return safeInvoke<MultiStrategyAgentPreviewOutput>("run_multi_strategy_agent_preview", { input });
}

export async function getRuntimeStrategyRegistryStatus(): Promise<MultiStrategyRuntimeMaturityReport> {
  return safeInvoke<MultiStrategyRuntimeMaturityReport>("get_runtime_strategy_registry_status");
}

export async function getReactBetaExecutionStatus(): Promise<ReactBetaExecutionStatusReport> {
  return safeInvoke<ReactBetaExecutionStatusReport>("get_react_beta_execution_status");
}

export async function runMainChatAgentExecutionV1EvalGate(): Promise<MainChatAgentExecutionV1EvalGateReport> {
  return safeInvoke<MainChatAgentExecutionV1EvalGateReport>(
    "run_main_chat_agent_execution_v1_eval_gate"
  );
}

export async function runMainChatAgentProductizationV1Gate(): Promise<MainChatAgentProductizationV1GateReport> {
  return safeInvoke<MainChatAgentProductizationV1GateReport>(
    "run_main_chat_agent_productization_v1_gate"
  );
}

export async function runMainChatStage3ExecutionUxReport(): Promise<MainChatStage3ExecutionUxReport> {
  return safeInvoke<MainChatStage3ExecutionUxReport>("run_main_chat_stage3_execution_ux_report");
}

export async function runMainChatExternalLiveProductizationGate(): Promise<MainChatExternalLiveProductizationGateReport> {
  return safeInvoke<MainChatExternalLiveProductizationGateReport>(
    "run_main_chat_external_live_productization_gate"
  );
}

export async function runMainChatAgentProductMaturityV2EventGate(): Promise<MainChatProductMaturityV2EventGateReport> {
  return safeInvoke<MainChatProductMaturityV2EventGateReport>(
    "run_main_chat_agent_product_maturity_v2_event_gate"
  );
}

export async function runMainChatAgentProductMaturityV2PlanGate(): Promise<MainChatProductMaturityV2PlanGateReport> {
  return safeInvoke<MainChatProductMaturityV2PlanGateReport>(
    "run_main_chat_agent_product_maturity_v2_plan_gate"
  );
}

export async function runMainChatAgentProductMaturityV2SkillsGate(): Promise<MainChatProductMaturityV2SkillsGateReport> {
  return safeInvoke<MainChatProductMaturityV2SkillsGateReport>(
    "run_main_chat_agent_product_maturity_v2_skills_gate"
  );
}

export async function runMainChatAgentProductMaturityV2FinalReadinessGate(): Promise<MainChatProductMaturityV2FinalReadinessReport> {
  return safeInvoke<MainChatProductMaturityV2FinalReadinessReport>(
    "run_main_chat_agent_product_maturity_v2_final_readiness_gate"
  );
}

export async function runMainChatAgentBetaV1ReadinessGate(): Promise<MainChatAgentBetaV1ReadinessReport> {
  return safeInvoke<MainChatAgentBetaV1ReadinessReport>(
    "run_main_chat_agent_beta_v1_readiness_gate"
  );
}

export async function runMainChatAgentStage2ReadinessGate(): Promise<MainChatAgentStage2ReadinessReport> {
  return safeInvoke<MainChatAgentStage2ReadinessReport>(
    "run_main_chat_agent_stage2_readiness_gate"
  );
}

export async function validateMainChatAgentStage2ManualDogfoodArtifact(): Promise<MainChatAgentStage2ManualDogfoodSummary> {
  return safeInvoke<MainChatAgentStage2ManualDogfoodSummary>(
    "validate_main_chat_agent_stage2_manual_dogfood_artifact"
  );
}

export async function runMainChatAgentStage1DogfoodGate(): Promise<MainChatAgentStage1DogfoodReport> {
  return safeInvoke<MainChatAgentStage1DogfoodReport>("run_main_chat_agent_stage1_dogfood_gate");
}

export async function runMainChatAgentStep6ProductAcceptanceGate(): Promise<MainChatStep6ProductAcceptanceReport> {
  return safeInvoke<MainChatStep6ProductAcceptanceReport>(
    "run_main_chat_agent_step6_product_acceptance_gate"
  );
}

export async function prepareMainChatStep6LiveProviderEvalState(): Promise<MainChatStep6LiveProviderEvalStatePrepReport> {
  return safeInvoke<MainChatStep6LiveProviderEvalStatePrepReport>(
    "prepare_main_chat_step6_live_provider_eval_state"
  );
}

export async function createPlanExecuteSession(
  input: CreatePlanExecuteSessionInput
): Promise<PlanExecuteSession> {
  return safeInvoke<PlanExecuteSession>("create_plan_execute_session", { input });
}

export async function getPlanExecuteSession(sessionId: string): Promise<PlanExecuteSession | null> {
  return safeInvoke<PlanExecuteSession | null>("get_plan_execute_session", {
    input: { sessionId },
  });
}

export async function listPlanExecuteSessions(limit: number = 5): Promise<PlanExecuteSession[]> {
  return safeInvoke<PlanExecuteSession[]>("list_plan_execute_sessions", {
    input: { limit },
  });
}

export async function updatePlanExecuteSessionDraft(
  input: UpdatePlanExecuteSessionDraftInput
): Promise<PlanExecuteSession> {
  return safeInvoke<PlanExecuteSession>("update_plan_execute_session_draft", { input });
}

export async function finalizePlanExecuteSession(
  sessionId: string,
  baseRevision?: number
): Promise<PlanExecuteSession> {
  return safeInvoke<PlanExecuteSession>("finalize_plan_execute_session", {
    input: { sessionId, ...(baseRevision !== undefined ? { baseRevision } : {}) },
  });
}

export async function cancelPlanExecuteSession(
  sessionId: string,
  baseRevision?: number
): Promise<PlanExecuteSession> {
  return safeInvoke<PlanExecuteSession>("cancel_plan_execute_session", {
    input: { sessionId, ...(baseRevision !== undefined ? { baseRevision } : {}) },
  });
}

export async function reviewPlanExecuteSession(
  sessionId: string,
  baseRevision?: number
): Promise<ReviewPlanExecuteSessionOutput> {
  return safeInvoke<ReviewPlanExecuteSessionOutput>("review_plan_execute_session", {
    input: { sessionId, ...(baseRevision !== undefined ? { baseRevision } : {}) },
  });
}

export async function executePlanExecuteStep(
  input: ExecutePlanExecuteStepInput
): Promise<ExecutePlanExecuteStepOutput> {
  return safeInvoke<ExecutePlanExecuteStepOutput>("execute_plan_execute_step", { input });
}

export async function skipPlanExecuteStep(
  input: SkipPlanExecuteStepInput
): Promise<SkipPlanExecuteStepOutput> {
  return safeInvoke<SkipPlanExecuteStepOutput>("skip_plan_execute_step", { input });
}

export async function checkRuntimeMigrationGate(
  input: RuntimeMigrationGateCheckInput = {}
): Promise<RuntimeMigrationGateReport> {
  return safeInvoke<RuntimeMigrationGateReport>("check_runtime_migration_gate", { input });
}

export async function getDefaultChatRuntimeBoundaryStatus(): Promise<DefaultChatRuntimeBoundaryStatus> {
  return safeInvoke<DefaultChatRuntimeBoundaryStatus>("get_default_chat_runtime_boundary_status");
}

export async function checkControlledChatPilotEligibility(
  input: ControlledChatPilotEligibilityCheckInput = {}
): Promise<ControlledChatPilotEligibilityReport> {
  return safeInvoke<ControlledChatPilotEligibilityReport>(
    "check_controlled_chat_pilot_eligibility",
    { input }
  );
}

export async function recordControlledPilotPromotionEvidence(
  input: ControlledPilotPromotionEvidenceInput
): Promise<ControlledPilotPromotionEvidenceResult> {
  return safeInvoke<ControlledPilotPromotionEvidenceResult>(
    "record_controlled_pilot_promotion_evidence",
    { input }
  );
}

export async function getControlledPilotPromotionEvidenceSummary(): Promise<ControlledPilotPromotionEvidenceSummary> {
  return safeInvoke<ControlledPilotPromotionEvidenceSummary>(
    "get_controlled_pilot_promotion_evidence_summary"
  );
}

export async function checkControlledPilotPromotionReadiness(
  input: ControlledPilotPromotionReadinessCheckInput = {}
): Promise<ControlledPilotPromotionReadinessReport> {
  return safeInvoke<ControlledPilotPromotionReadinessReport>(
    "check_controlled_pilot_promotion_readiness",
    { input }
  );
}

export async function draftControlledChatMigrationPlan(
  input: ControlledChatMigrationPlanDraftInput = {}
): Promise<ControlledChatMigrationPlanDraft> {
  return safeInvoke<ControlledChatMigrationPlanDraft>("draft_controlled_chat_migration_plan", {
    input,
  });
}

export async function recordControlledChatMigrationReviewDecision(
  input: ControlledChatMigrationReviewDecisionInput
): Promise<ControlledChatMigrationReviewDecisionResult> {
  return safeInvoke<ControlledChatMigrationReviewDecisionResult>(
    "record_controlled_chat_migration_review_decision",
    { input }
  );
}

export async function getControlledChatMigrationReviewDecisionSummary(): Promise<ControlledChatMigrationReviewDecisionSummary> {
  return safeInvoke<ControlledChatMigrationReviewDecisionSummary>(
    "get_controlled_chat_migration_review_decision_summary"
  );
}

export async function checkControlledChatMigrationImplementationGate(
  input: ControlledChatMigrationImplementationGateInput = {}
): Promise<ControlledChatMigrationImplementationGateReport> {
  return safeInvoke<ControlledChatMigrationImplementationGateReport>(
    "check_controlled_chat_migration_implementation_gate",
    { input }
  );
}

export async function runControlledChatMigrationShadowRun(
  input: ControlledChatMigrationShadowRunInput
): Promise<ControlledChatMigrationShadowRunOutput> {
  return safeInvoke<ControlledChatMigrationShadowRunOutput>(
    "run_controlled_chat_migration_shadow_run",
    { input }
  );
}

export async function recordControlledChatMigrationShadowReviewDecision(
  input: ControlledChatMigrationShadowReviewDecisionInput
): Promise<ControlledChatMigrationShadowReviewDecisionResult> {
  return safeInvoke<ControlledChatMigrationShadowReviewDecisionResult>(
    "record_controlled_chat_migration_shadow_review_decision",
    { input }
  );
}

export async function getControlledChatMigrationShadowReviewSummary(): Promise<ControlledChatMigrationShadowReviewSummary> {
  return safeInvoke<ControlledChatMigrationShadowReviewSummary>(
    "get_controlled_chat_migration_shadow_review_summary"
  );
}

export async function checkControlledChatCutoverReadiness(
  input: ControlledChatCutoverReadinessInput = {}
): Promise<ControlledChatCutoverReadinessReport> {
  return safeInvoke<ControlledChatCutoverReadinessReport>(
    "check_controlled_chat_cutover_readiness",
    { input }
  );
}

export async function runControlledChatCutoverCandidate(
  input: ControlledChatCutoverCandidateInput
): Promise<ControlledChatCutoverCandidateOutput> {
  return safeInvoke<ControlledChatCutoverCandidateOutput>("run_controlled_chat_cutover_candidate", {
    input,
  });
}

export async function recordControlledChatCutoverCandidateReviewDecision(
  input: ControlledChatCutoverCandidateReviewDecisionInput
): Promise<ControlledChatCutoverCandidateReviewDecisionResult> {
  return safeInvoke<ControlledChatCutoverCandidateReviewDecisionResult>(
    "record_controlled_chat_cutover_candidate_review_decision",
    { input }
  );
}

export async function getControlledChatCutoverCandidateReviewSummary(): Promise<ControlledChatCutoverCandidateReviewSummary> {
  return safeInvoke<ControlledChatCutoverCandidateReviewSummary>(
    "get_controlled_chat_cutover_candidate_review_summary"
  );
}

export async function checkControlledChatCutoverCandidatePromotionReadiness(
  input: ControlledChatCutoverCandidatePromotionReadinessInput = {}
): Promise<ControlledChatCutoverCandidatePromotionReadinessReport> {
  return safeInvoke<ControlledChatCutoverCandidatePromotionReadinessReport>(
    "check_controlled_chat_cutover_candidate_promotion_readiness",
    { input }
  );
}

export async function draftDefaultChatAdapterActivationPlan(
  input: DefaultChatAdapterActivationPlanDraftInput = {}
): Promise<DefaultChatAdapterActivationPlanDraft> {
  return safeInvoke<DefaultChatAdapterActivationPlanDraft>(
    "draft_default_chat_adapter_activation_plan",
    { input }
  );
}

export async function recordDefaultChatAdapterActivationReviewDecision(
  input: DefaultChatAdapterActivationReviewDecisionInput
): Promise<DefaultChatAdapterActivationReviewDecisionResult> {
  return safeInvoke<DefaultChatAdapterActivationReviewDecisionResult>(
    "record_default_chat_adapter_activation_review_decision",
    { input }
  );
}

export async function getDefaultChatAdapterActivationReviewSummary(): Promise<DefaultChatAdapterActivationReviewSummary> {
  return safeInvoke<DefaultChatAdapterActivationReviewSummary>(
    "get_default_chat_adapter_activation_review_summary"
  );
}

export async function checkDefaultChatAdapterActivationImplementationGate(
  input: DefaultChatAdapterActivationImplementationGateInput = {}
): Promise<DefaultChatAdapterActivationImplementationGateReport> {
  return safeInvoke<DefaultChatAdapterActivationImplementationGateReport>(
    "check_default_chat_adapter_activation_implementation_gate",
    { input }
  );
}

export async function getDefaultChatAdapterRoutingStatus(
  input: DefaultChatAdapterRoutingStatusInput = {}
): Promise<DefaultChatAdapterRoutingStatus> {
  return safeInvoke<DefaultChatAdapterRoutingStatus>("get_default_chat_adapter_routing_status", {
    input,
  });
}

export async function checkDefaultChatAdapterContractHarness(
  input: DefaultChatAdapterContractHarnessInput = {}
): Promise<DefaultChatAdapterContractHarnessReport> {
  return safeInvoke<DefaultChatAdapterContractHarnessReport>(
    "check_default_chat_adapter_contract_harness",
    { input }
  );
}

export async function getDefaultChatAdapterOrdinaryEntryPreflightStatus(): Promise<DefaultChatAdapterOrdinaryEntryPreflightStatus> {
  return safeInvoke<DefaultChatAdapterOrdinaryEntryPreflightStatus>(
    "get_default_chat_adapter_ordinary_entry_preflight_status"
  );
}

export async function checkDefaultChatAdapterNarrowImplementationDiscussionGate(
  input: DefaultChatAdapterNarrowImplementationDiscussionGateInput
): Promise<DefaultChatAdapterNarrowImplementationDiscussionGateReport> {
  return safeInvoke<DefaultChatAdapterNarrowImplementationDiscussionGateReport>(
    "check_default_chat_adapter_narrow_implementation_discussion_gate",
    { input }
  );
}

export async function draftDefaultChatAdapterNarrowImplementationPlan(
  input: DefaultChatAdapterNarrowImplementationPlanInput
): Promise<DefaultChatAdapterNarrowImplementationPlanDraft> {
  return safeInvoke<DefaultChatAdapterNarrowImplementationPlanDraft>(
    "draft_default_chat_adapter_narrow_implementation_plan",
    { input }
  );
}

export async function recordDefaultChatAdapterNarrowImplementationPlanReviewDecision(
  input: DefaultChatAdapterNarrowImplementationPlanReviewDecisionInput
): Promise<DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult> {
  return safeInvoke<DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult>(
    "record_default_chat_adapter_narrow_implementation_plan_review_decision",
    { input }
  );
}

export async function getDefaultChatAdapterNarrowImplementationPlanReviewSummary(): Promise<DefaultChatAdapterNarrowImplementationPlanReviewSummary> {
  return safeInvoke<DefaultChatAdapterNarrowImplementationPlanReviewSummary>(
    "get_default_chat_adapter_narrow_implementation_plan_review_summary"
  );
}

export async function checkDefaultChatAdapterNarrowImplementationPlanApprovalReadiness(
  input: DefaultChatAdapterNarrowImplementationPlanApprovalReadinessInput
): Promise<DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport> {
  return safeInvoke<DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport>(
    "check_default_chat_adapter_narrow_implementation_plan_approval_readiness",
    { input }
  );
}

export async function runDefaultChatAdapterDryRun(
  input: DefaultChatAdapterDryRunInput
): Promise<DefaultChatAdapterDryRunReport> {
  return safeInvoke<DefaultChatAdapterDryRunReport>("run_default_chat_adapter_dry_run", {
    input,
  });
}

export async function recordDefaultChatAdapterDryRunReviewDecision(
  input: DefaultChatAdapterDryRunReviewDecisionInput
): Promise<DefaultChatAdapterDryRunReviewDecisionResult> {
  return safeInvoke<DefaultChatAdapterDryRunReviewDecisionResult>(
    "record_default_chat_adapter_dry_run_review_decision",
    { input }
  );
}

export async function getDefaultChatAdapterDryRunReviewSummary(): Promise<DefaultChatAdapterDryRunReviewSummary> {
  return safeInvoke<DefaultChatAdapterDryRunReviewSummary>(
    "get_default_chat_adapter_dry_run_review_summary"
  );
}

export async function checkDefaultChatAdapterImplementationReadiness(
  input: DefaultChatAdapterImplementationReadinessInput
): Promise<DefaultChatAdapterImplementationReadinessReport> {
  return safeInvoke<DefaultChatAdapterImplementationReadinessReport>(
    "check_default_chat_adapter_implementation_readiness",
    { input }
  );
}

export async function runDefaultChatAdapterControlledPreview(
  input: DefaultChatAdapterControlledPreviewInput
): Promise<DefaultChatAdapterControlledPreviewReport> {
  return safeInvoke<DefaultChatAdapterControlledPreviewReport>(
    "run_default_chat_adapter_controlled_preview",
    { input }
  );
}

export async function recordDefaultChatAdapterControlledPreviewReviewDecision(
  input: DefaultChatAdapterControlledPreviewReviewDecisionInput
): Promise<DefaultChatAdapterControlledPreviewReviewDecisionResult> {
  return safeInvoke<DefaultChatAdapterControlledPreviewReviewDecisionResult>(
    "record_default_chat_adapter_controlled_preview_review_decision",
    { input }
  );
}

export async function getDefaultChatAdapterControlledPreviewReviewSummary(): Promise<DefaultChatAdapterControlledPreviewReviewSummary> {
  return safeInvoke<DefaultChatAdapterControlledPreviewReviewSummary>(
    "get_default_chat_adapter_controlled_preview_review_summary"
  );
}

export async function checkDefaultChatAdapterControlledPreviewApprovalReadiness(
  input: DefaultChatAdapterControlledPreviewApprovalReadinessInput
): Promise<DefaultChatAdapterControlledPreviewApprovalReadinessReport> {
  return safeInvoke<DefaultChatAdapterControlledPreviewApprovalReadinessReport>(
    "check_default_chat_adapter_controlled_preview_approval_readiness",
    { input }
  );
}

export async function draftDefaultChatAdapterCutoverImplementationPlan(
  input: DefaultChatAdapterCutoverImplementationPlanInput
): Promise<DefaultChatAdapterCutoverImplementationPlanDraft> {
  return safeInvoke<DefaultChatAdapterCutoverImplementationPlanDraft>(
    "draft_default_chat_adapter_cutover_implementation_plan",
    { input }
  );
}

export async function recordDefaultChatAdapterCutoverPlanReviewDecision(
  input: DefaultChatAdapterCutoverPlanReviewDecisionInput
): Promise<DefaultChatAdapterCutoverPlanReviewDecisionResult> {
  return safeInvoke<DefaultChatAdapterCutoverPlanReviewDecisionResult>(
    "record_default_chat_adapter_cutover_plan_review_decision",
    { input }
  );
}

export async function getDefaultChatAdapterCutoverPlanReviewSummary(): Promise<DefaultChatAdapterCutoverPlanReviewSummary> {
  return safeInvoke<DefaultChatAdapterCutoverPlanReviewSummary>(
    "get_default_chat_adapter_cutover_plan_review_summary"
  );
}

export async function checkDefaultChatAdapterCutoverPlanApprovalReadiness(
  input: DefaultChatAdapterCutoverPlanApprovalReadinessInput
): Promise<DefaultChatAdapterCutoverPlanApprovalReadinessReport> {
  return safeInvoke<DefaultChatAdapterCutoverPlanApprovalReadinessReport>(
    "check_default_chat_adapter_cutover_plan_approval_readiness",
    { input }
  );
}

export async function startStreamMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions = {}
): Promise<StreamMessageDonePayload> {
  const payload = {
    ...sessionArgs(sessionId),
    messages,
    ...selectedSkillArgs(options.selectedSkillId),
  };
  return safeInvoke<StreamMessageDonePayload>("start_stream_message", {
    ...payload,
    args: payload,
  });
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

export interface RuntimeBuildInfo {
  profile: "dev" | "qa" | "release" | string;
  gitSha: string;
  buildTime: string;
  currentExe: string;
  binaryKind: "debug_binary" | "debug_bundle" | "release_bundle" | "unknown" | string;
  frontendMode: "dev_server" | "bundled_dist" | "unknown" | string;
  devUrl: string;
  frontendDist: string;
  dataDir: string;
  a2aPort: number;
  a2aStatus: string;
  bundleIdentifier: string;
  productName: string;
}

export interface RouteIdentity {
  provider: string;
  model: string;
  route_type: "local" | "cloud" | "agent_runtime" | "scripted" | "unknown" | string;
  privacy_level: string;
  reason: string;
  provider_health_is_estimated: boolean;
}

export interface ProviderReadiness {
  configured: boolean;
  credential_present: boolean;
  validated: boolean;
  validation_status:
    | "unconfigured"
    | "unvalidated"
    | "stale"
    | "validated"
    | "failed"
    | "scripted_dogfood"
    | string;
  preferred: string;
  actually_used?: string | null;
  stale: boolean;
  failed: boolean;
  last_checked_at?: string | null;
}

export interface FallbackEvidence {
  from_route?: RouteIdentity | null;
  to_route?: RouteIdentity | null;
  reason: string;
  blocker_codes: string[];
}

export interface RuntimeRouteEvidence {
  evidence_id: string;
  generated_at: string;
  conversation_id?: string | null;
  run_id?: string | null;
  task_session_id?: string | null;
  answer_scope:
    | "current_turn"
    | "last_completed_turn"
    | "settings_readiness"
    | "planned_next_turn"
    | "unknown"
    | string;
  planned_route?: RouteIdentity | null;
  actual_route?: RouteIdentity | null;
  last_completed_route?: RouteIdentity | null;
  provider_readiness: ProviderReadiness;
  fallback?: FallbackEvidence | null;
  external_transmission: "not_sent" | "sent" | "unknown" | "not_instrumented" | string;
  source_refs: unknown[];
  truth_confidence: "verified" | "inferred" | "unknown" | string;
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
  ollama_service_online?: boolean;
  ollama_online: boolean;
  local_model: string;
  resolved_local_model?: string | null;
  prefer_local_model: boolean;
  cloud_api_configured: boolean;
  cloud_provider?: string;
  cloud_api_validated?: boolean;
  cloud_api_last_error?: string | null;
  cloud_api_validation_status?: CloudApiValidationStatus | string;
  cloud_api_validated_at?: string | null;
  cloud_api_failed_at?: string | null;
  cloud_api_validation_source?: string | null;
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
  runtime_build_info?: RuntimeBuildInfo;
  runtime_route_evidence?: RuntimeRouteEvidence | null;
}

export async function getSystemDiagnostics(): Promise<SystemDiagnostics> {
  return safeInvoke<SystemDiagnostics>("get_system_diagnostics");
}

export async function getRuntimeBuildInfo(): Promise<RuntimeBuildInfo> {
  return safeInvoke<RuntimeBuildInfo>("get_runtime_build_info");
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

export interface SnapshotRestoreResult {
  success: boolean;
  legacy: boolean;
  warning: string;
  metadata_safe: boolean;
  durable_lifemodel_write: boolean;
  restored_snapshot_version: string;
  restored_model_version?: string;
  pre_restore_snapshot_created: boolean;
  pre_restore_snapshot_version?: string | null;
}

export async function restoreSnapshot(version: string): Promise<SnapshotRestoreResult> {
  return safeInvoke<SnapshotRestoreResult>("restore_snapshot", {
    version,
    governedRequest: MANUAL_SNAPSHOT_RESTORE_REQUEST,
    governed_request: MANUAL_SNAPSHOT_RESTORE_REQUEST,
  });
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

export interface FeedbackEvolutionLegacyDirectApplyResult {
  success: boolean;
  legacy: boolean;
  warning: string;
  applied: boolean;
  applied_change_count: number;
  durable_lifemodel_write: boolean;
  message: string;
  metadata_safe: boolean;
}

export interface FeedbackEvolutionReportResult {
  success: boolean;
  read_only: boolean;
  metadata_safe: boolean;
  durable_lifemodel_write: boolean;
  evolution_rules_write: boolean;
  applied_rule_count: number;
  liked_pattern_count: number;
  disliked_pattern_count: number;
  suggested_rule_count: number;
  proposal_candidate_count: number;
  candidate_status: string;
  summary: string;
}

export async function applyFeedbackEvolution(): Promise<FeedbackEvolutionLegacyDirectApplyResult> {
  return safeInvoke<FeedbackEvolutionLegacyDirectApplyResult>("apply_feedback_evolution");
}

export async function generateEvolutionReport(): Promise<FeedbackEvolutionReportResult> {
  return safeInvoke<FeedbackEvolutionReportResult>("generate_evolution_report");
}

export async function runMicroEvolution(): Promise<{
  success?: boolean;
  legacy?: boolean;
  applied: boolean;
  message: string;
  warning?: string;
  change_count?: number;
  snapshot_version?: string | null;
  signal_counts?: {
    feedback_terms: number;
    behavior_events: number;
    inference_items: number;
  };
  metadata_safe?: boolean;
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
  legacy?: boolean;
  warning?: string;
  snapshot_version?: string;
  applied_count?: number;
  metadata_safe?: boolean;
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

export interface DataImportResult {
  success: boolean;
  legacy: boolean;
  warning: string;
  metadata_safe: boolean;
  durable_lifemodel_write: boolean;
  imported_message_count: number;
  imported_vector_count: number;
}

export async function importAllData(payload: ExportPayload): Promise<DataImportResult> {
  return safeInvoke<DataImportResult>("import_all_data", {
    payload,
    importRequest: MANUAL_DATA_IMPORT_REQUEST,
    import_request: MANUAL_DATA_IMPORT_REQUEST,
  });
}

export async function testApiKey(): Promise<boolean> {
  return safeInvoke<boolean>("test_api_key");
}

export interface LlmConnectionTestResult {
  ok: boolean;
  provider: string;
  message: string;
  validation_status?: string;
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
  reactTrace?: ReactActionTraceEnvelope;
}

export interface AgentObservation {
  id: string;
  actionId?: string;
  content: string;
  source: string;
  structuredResult?: any;
  timestamp: string;
  reactTrace?: ReactActionTraceEnvelope;
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
  inputSchema?: any;
  outputSchema: any;
  proposalPolicy: string;
  sourceKind?: "built_in" | "plugin";
  executionStatus?:
    | "executable_built_in"
    | "disabled_declarative_only"
    | "model_only_no_tools"
    | "blocked";
  capabilityFlags?: string[];
  pluginId?: string;
}

export interface SkillRunResponse {
  runId: string;
  status: string;
  summary: string;
  generatedProposals: string[];
}

export interface SkillRuntimeDescriptor {
  id: string;
  name: string;
  sourceKind: "built_in" | "plugin";
  executionStatus:
    | "executable_built_in"
    | "disabled_declarative_only"
    | "model_only_no_tools"
    | "blocked";
  inputSchemaDigest: string;
  outputSchemaDigest: string;
  proposalPolicy: string;
  requiredContextIds: string[];
  allowedToolIds: string[];
  allowedToolCount: number;
  executionBudget: SkillManifest["executionBudget"];
  capabilityFlags: string[];
  metadataSafe: boolean;
  containsRawContent: boolean;
  directWriteImplied: boolean;
}

export interface SkillRuntimeStatusReport {
  reportKind: string;
  readiness: {
    reportKind: string;
    ready: boolean;
    metadataSafe: boolean;
    containsRawContent: boolean;
    requiredBuiltinsPresent: boolean;
    builtInSkillCount: number;
    pluginSkillCount: number;
    descriptors: SkillRuntimeDescriptor[];
    pluginBoundarySummary: any;
    proposalGovernanceSummary: any;
    privacyModelRouteBoundarySummary: any;
    traceContractSummary: any;
    defaultChatUnchanged: boolean;
    migrationPermission: boolean;
    runtimeExecutionPerformed: boolean;
    modelCallPerformed: boolean;
    toolCallPerformed: boolean;
    businessWritesPerformed: boolean;
    blockers: string[];
  };
  defaultChatUnchanged: boolean;
  migrationPermission: boolean;
  readOnly: boolean;
  runtimeExecutionPerformed: boolean;
  modelCallPerformed: boolean;
  toolCallPerformed: boolean;
  businessWritesPerformed: boolean;
  metadataSafe: boolean;
  blockers: string[];
}

export async function listSkills(): Promise<SkillManifest[]> {
  return safeInvoke<SkillManifest[]>("list_skills");
}

export async function getSkillRuntimeStatus(): Promise<SkillRuntimeStatusReport> {
  return safeInvoke<SkillRuntimeStatusReport>("get_skill_runtime_status");
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
  | "proactive_agent"
  | "planning_session";

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

export interface MemoryLifecycleRecord {
  memoryId: string;
  proposalId: string;
  sourceTaskSessionId?: string;
  sourceRunId?: string;
  content: string;
  scope: string;
  category: string;
  riskLevel: string;
  status: string;
  materializationStatus: string;
  materializationErrorCode?: string;
  createdBy: string;
  acceptedBy?: string;
  acceptedAt?: string;
  materializedViewId?: string;
  materializedViewVersion?: number;
  evidenceIds: string[];
  confidence: number;
  conflictIds: string[];
  supersedesMemoryId?: string;
  replacementMemoryId?: string;
  rolledBackByEventId?: string;
  runtimeContextExcludedAt?: string;
}

export interface MemoryRollbackEvent {
  rollbackEventId: string;
  memoryId: string;
  proposalId: string;
  requestedBy: string;
  reason: string;
  previousStatus: string;
  nextStatus: string;
  affectedMaterializedViewIds: string[];
  affectedRuntimeSurfaceIds: string[];
  createdAt: string;
  auditDigest: string;
}

export interface MemoryMaterializedView {
  materializedViewId: string;
  scope?: string;
  version: number;
  activeMemoryIds: string[];
  runtimeSurfaceIds: string[];
  updatedAt: string;
  contentDigest: string;
}

export interface MemoryRollbackReport {
  record: MemoryLifecycleRecord;
  rollbackEvent: MemoryRollbackEvent;
  materializedView: MemoryMaterializedView;
}

export interface MemoryProposalDraftEditReport {
  proposalId: string;
  draftOnly: boolean;
  durableWriteExecuted: boolean;
  originalProvenancePreserved: boolean;
  status: string;
  beforeDigest: string;
  afterDigest: string;
}

export interface Stage4KnowledgeAssetLoaded {
  assetId: string;
  relativePath: string;
  source: string;
  digest: string;
  sizeBytes: number;
  truncated: boolean;
  reason: string;
  selectedSkillId?: string;
  contextOnly: boolean;
}

export interface Stage4KnowledgeAssetSkipped {
  assetId: string;
  relativePath: string;
  source: string;
  reason: string;
  selectedSkillId?: string;
}

export interface Stage4KnowledgeAssetInventory {
  inventoryId: string;
  root: string;
  selectedSkillId?: string;
  loadedAssets: Stage4KnowledgeAssetLoaded[];
  skippedAssets: Stage4KnowledgeAssetSkipped[];
}

export interface ManagedKnowledgeValidation {
  allowed: boolean;
  targetKind: string;
  blocker?: string;
}

export interface ManagedKnowledgeContextReloadProof {
  loaded: boolean;
  digest: string;
  source: string;
  reason: string;
}

export interface ManagedKnowledgeWriteDraft {
  proposalId: string;
  targetPath: string;
  sourceProvenanceProposalId: string;
  linkedMemoryIds: string[];
  beforeDigest: string;
  afterDigest: string;
  previewDiff: string;
  validation: ManagedKnowledgeValidation;
  fileWrittenBeforeConfirmation: boolean;
}

export interface ManagedKnowledgeWriteApplyReport {
  proposalId: string;
  targetPath: string;
  versionId: string;
  auditId: string;
  rollbackSnapshotId: string;
  beforeDigest: string;
  afterDigest: string;
  contextReload: ManagedKnowledgeContextReloadProof;
}

export interface ManagedKnowledgeWriteRollbackReport {
  proposalId: string;
  targetPath: string;
  restoredVersionId: string;
  rolledBackVersionId: string;
  auditId: string;
  restoredDigest: string;
  contextReload: ManagedKnowledgeContextReloadProof;
}

export interface MainChatStage4MemoryKnowledgeRow {
  id: string;
  scenario: string;
  status: string;
  evidenceIds: string[];
  blockers: string[];
}

export interface MainChatStage4MemoryKnowledgeReport {
  reportKind: string;
  schemaVersion: string;
  scenarioCount: number;
  passedScenarioCount: number;
  blockedScenarioCount: number;
  notAReadinessGate: boolean;
  readinessClaim: boolean;
  stage2ReadinessPreserved: boolean;
  rows: MainChatStage4MemoryKnowledgeRow[];
  evidenceIds: string[];
  blockers: string[];
  activeMemoryIds: string[];
  excludedMemoryIds: string[];
  loadedKnowledgeAssetIds: string[];
  skippedKnowledgeAssetIds: string[];
  managedKnowledgeWriteAssetIds: string[];
  managedKnowledgeWriteVersionIds: string[];
  managedKnowledgeWriteAuditIds: string[];
  managedKnowledgeRollbackSnapshotIds: string[];
  directWriteCount: number;
  confirmedKnowledgeWriteCount: number;
  rollbackEventCount: number;
}

export interface MainChatStage5BuildInfo {
  commit?: string | null;
  branch?: string | null;
  appVersion: string;
  buildTimestamp?: string | null;
  dirtyState?: boolean | null;
  blockers: string[];
}

export interface MainChatStage5ProviderPreflight {
  provider: string;
  model: string;
  routeType: string;
  keyPresent: boolean;
  networkOptIn: boolean;
  liveProviderInvocationAllowed: boolean;
  liveProviderPreflightStatus: string;
  blockers: string[];
}

export interface MainChatStage5FailureClassification {
  class: string;
  severity: string;
  scope: string;
  recoverability: string;
  recoveryRecommendation: string;
  evidence: string[];
}

export interface MainChatStage5GateSummary {
  recommendation: string;
  blockers: string[];
}

export interface MainChatStage5PreflightReport {
  reportKind: string;
  schemaVersion: string;
  createdAt: string;
  build: MainChatStage5BuildInfo;
  provider: MainChatStage5ProviderPreflight;
  scheduler: {
    schedulerType: string;
    scriptedProviderResponsePresent: boolean;
    preferLocal: boolean;
    localModelConfigured: boolean;
  };
  workspace: {
    rootDigest: string;
    safePathCount: number;
    safePathsDigest: string;
    safePathsConfigured: boolean;
    blockers: string[];
  };
  mcp: {
    registryAvailable: boolean;
    manifestCount: number;
    readCandidateCount: number;
    blockers: string[];
  };
  database: {
    memoryStoreAvailable: boolean;
    agentRunStoreAvailable: boolean;
    taskSessionStoreAvailable: boolean;
    actionQueueStoreAvailable: boolean;
    proposalStoreAvailable: boolean;
    memoryLifecycleStoreAvailable: boolean;
    blockers: string[];
  };
  stage2Readiness: MainChatStage5GateSummary;
  finalAcceptance: MainChatStage5GateSummary;
  failure: MainChatStage5FailureClassification;
  externalProviderInvokedByDefault: boolean;
  modelInvoked: boolean;
  directWritesExecuted: boolean;
  metadataSafe: boolean;
  blockers: string[];
}

export interface MainChatStage5ArtifactMetadata {
  artifactId: string;
  artifactKind: string;
  schemaVersion: string;
  createdAt: string;
  storageAlias: string;
  digest: string;
  byteSize: number;
}

export interface MainChatStage5UiEvidence {
  frontendRoute: string;
  surface: string;
  visibleControlLabels: string[];
  taskSessionId: string;
  backendSnapshotId?: string | null;
  timestamp: string;
  domDigest?: string | null;
  screenshotDigest?: string | null;
}

export interface MainChatStage5DebugBundle {
  bundleId: string;
  schemaVersion: string;
  createdAt: string;
  build: MainChatStage5BuildInfo;
  environment: MainChatStage5PreflightReport;
  scenario: {
    scenarioId?: string | null;
    reviewerId?: string | null;
    status?: string | null;
    notesDigest?: string | null;
  };
  task: {
    chatSessionId: string;
    taskSessionId: string;
    runId?: string | null;
    strategy: string;
    status: string;
    userGoalDigest: string;
    transcriptEntryCount: number;
    actionCount: number;
    proposalCount: number;
    blockerCount: number;
    finalDeliveryId?: string | null;
  };
  route: {
    routeType: string;
    provider?: string | null;
    model?: string | null;
    localOnly: boolean;
    liveProviderAttempted: boolean;
    providerEndpointKind?: string | null;
  };
  timeline: Array<{
    itemId: string;
    kind: string;
    summaryPreview: string;
    metadataDigest: string;
  }>;
  tools: {
    candidateCount: number;
    selectedTool?: string | null;
    actionType?: string | null;
    targetDigest?: string | null;
    policyDecision?: string | null;
    observationCount: number;
    actionStatuses: string[];
  };
  context: {
    activeMemoryIds: string[];
    excludedMemoryIds: string[];
    knowledgeAssetIds: string[];
    selectedSkillId?: string | null;
    contextSourceDigests: string[];
  };
  memory: {
    proposalIds: string[];
    acceptedMemoryIds: string[];
    rolledBackMemoryIds: string[];
    managedKnowledgeVersionIds: string[];
  };
  finalDelivery: {
    completedWorkCount: number;
    durableChangeCount: number;
    pendingUserActionCount: number;
    skippedWorkCount: number;
    blockerCount: number;
    finalDeliveryDigest?: string | null;
  };
  failure: MainChatStage5FailureClassification;
  redaction: {
    mode: string;
    rawContentIncluded: boolean;
    secretsDetected: boolean;
    unsafeFieldCount: number;
    unsafeFieldsDropped: string[];
    previewLimit: number;
    promptDigest?: string | null;
    responseDigest?: string | null;
    contextDigest?: string | null;
  };
  uiEvidence?: MainChatStage5UiEvidence | null;
  artifact: MainChatStage5ArtifactMetadata;
}

export interface MainChatStage5IssueReportInput {
  scenarioId: string;
  reviewerId: string;
  status: string;
  taskSessionId?: string | null;
  runId?: string | null;
  bundleId?: string | null;
  failureClass?: string | null;
  notes?: string | null;
  preflightOnlyMissingTaskReason?: string | null;
}

export interface MainChatStage5IssueReport {
  reportId: string;
  schemaVersion: string;
  createdAt: string;
  scenarioId: string;
  reviewerId: string;
  status: string;
  taskSessionId?: string | null;
  runId?: string | null;
  bundleId?: string | null;
  buildCommit?: string | null;
  appVersion: string;
  redactionMode: string;
  failureClass?: string | null;
  notesDigest?: string | null;
  notesPreview?: string | null;
  missingTaskRunReason?: string | null;
  blockers: string[];
  artifact: MainChatStage5ArtifactMetadata;
}

export interface MainChatStage5ReportRow {
  id: string;
  scenario: string;
  status: string;
  evidenceIds: string[];
  bundleIds: string[];
  issueArtifactIds: string[];
  blockers: string[];
}

export interface MainChatStage5ManagedKnowledgeEval {
  isolatedEvalAppState: boolean;
  tempWorkspace: boolean;
  realWorkspaceWriteExecuted: boolean;
  userWriteCompleted: boolean;
  memoryRollbackCompleted: boolean;
  managedKnowledgeWriteVersionIds: string[];
  managedKnowledgeAuditIds: string[];
  rollbackSnapshotIds: string[];
  evidenceIds: string[];
  blockers: string[];
}

export interface MainChatStage5ReleaseDebugReport {
  reportKind: string;
  schemaVersion: string;
  scenarioCount: number;
  passedScenarioCount: number;
  blockedScenarioCount: number;
  notAReadinessGate: boolean;
  readinessClaim: boolean;
  rows: MainChatStage5ReportRow[];
  evidenceIds: string[];
  blockers: string[];
  build: MainChatStage5BuildInfo;
  preflightSummary: MainChatStage5PreflightReport;
  bundleIds: string[];
  issueArtifactIds: string[];
  artifactStorageSummary: MainChatStage5ArtifactMetadata[];
  redactionSummary: MainChatStage5DebugBundle["redaction"];
  managedKnowledgeEval: MainChatStage5ManagedKnowledgeEval;
  stage2ReadinessPreserved: boolean;
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

export async function acceptProposal(proposalId: string): Promise<{
  success: boolean;
  patchResult: PatchApplyResult;
  memoryLifecycle?: MemoryLifecycleRecord;
}> {
  return safeInvoke("accept_proposal", { proposalId, proposal_id: proposalId });
}

export async function rollbackMemoryAsset(
  memoryId: string,
  reason: string
): Promise<MemoryRollbackReport> {
  return safeInvoke("rollback_memory_asset", { memoryId, memory_id: memoryId, reason });
}

export async function listMemoryAssets(
  options: {
    scope?: string;
    status?: string;
    limit?: number;
    offset?: number;
  } = {}
): Promise<MemoryLifecycleRecord[]> {
  return safeInvoke("list_memory_assets", {
    scope: options.scope,
    status: options.status,
    limit: options.limit ?? 100,
    offset: options.offset ?? 0,
  });
}

export async function getMemoryAsset(memoryId: string): Promise<MemoryLifecycleRecord> {
  return safeInvoke("get_memory_asset", { memoryId, memory_id: memoryId });
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

export async function draftEditMemoryProposal(
  proposalId: string,
  newAfter: any
): Promise<MemoryProposalDraftEditReport> {
  return safeInvoke("draft_edit_memory_proposal", {
    proposalId,
    proposal_id: proposalId,
    newAfter,
    new_after: newAfter,
  });
}

export async function listStage4KnowledgeAssetInventory(
  selectedSkillId?: string
): Promise<Stage4KnowledgeAssetInventory> {
  return safeInvoke("list_stage4_knowledge_asset_inventory", {
    selectedSkillId,
    selected_skill_id: selectedSkillId,
  });
}

export async function createManagedKnowledgeWriteDraft(
  targetPath: string,
  afterContent: string,
  sourceProposalId?: string,
  linkedMemoryIds: string[] = []
): Promise<ManagedKnowledgeWriteDraft> {
  return safeInvoke("create_managed_knowledge_write_draft", {
    targetPath,
    target_path: targetPath,
    afterContent,
    after_content: afterContent,
    sourceProposalId,
    source_proposal_id: sourceProposalId,
    linkedMemoryIds,
    linked_memory_ids: linkedMemoryIds,
  });
}

export async function confirmManagedKnowledgeWrite(
  proposalId: string
): Promise<ManagedKnowledgeWriteApplyReport> {
  return safeInvoke("confirm_managed_knowledge_write", {
    proposalId,
    proposal_id: proposalId,
  });
}

export async function rollbackManagedKnowledgeWrite(
  versionId: string
): Promise<ManagedKnowledgeWriteRollbackReport> {
  return safeInvoke("rollback_managed_knowledge_write", {
    versionId,
    version_id: versionId,
  });
}

export async function runMainChatStage4MemoryKnowledgeReport(): Promise<MainChatStage4MemoryKnowledgeReport> {
  return safeInvoke("run_main_chat_stage4_memory_knowledge_report");
}

export async function evaluateMainChatStage5ReleaseDebugPreflight(): Promise<MainChatStage5PreflightReport> {
  return safeInvoke("evaluate_main_chat_stage5_release_debug_preflight");
}

export async function exportMainChatAgentDebugBundle(
  taskSessionId: string,
  options: {
    scenarioId?: string;
    reviewerId?: string;
    uiEvidence?: MainChatStage5UiEvidence;
  } = {}
): Promise<MainChatStage5DebugBundle> {
  return safeInvoke("export_main_chat_agent_debug_bundle", {
    taskSessionId,
    task_session_id: taskSessionId,
    scenarioId: options.scenarioId,
    scenario_id: options.scenarioId,
    reviewerId: options.reviewerId,
    reviewer_id: options.reviewerId,
    uiEvidence: options.uiEvidence,
    ui_evidence: options.uiEvidence,
  });
}

export async function createMainChatInternalIssueReport(
  input: MainChatStage5IssueReportInput
): Promise<MainChatStage5IssueReport> {
  return safeInvoke("create_main_chat_internal_issue_report", { input });
}

export async function listMainChatDebugBundles(): Promise<MainChatStage5ArtifactMetadata[]> {
  return safeInvoke("list_main_chat_debug_bundles");
}

export async function getMainChatDebugBundle(bundleId: string): Promise<MainChatStage5DebugBundle> {
  return safeInvoke("get_main_chat_debug_bundle", { bundleId, bundle_id: bundleId });
}

export async function deleteMainChatDebugBundle(bundleId: string): Promise<boolean> {
  return safeInvoke("delete_main_chat_debug_bundle", { bundleId, bundle_id: bundleId });
}

export async function listMainChatInternalIssueReports(): Promise<
  MainChatStage5ArtifactMetadata[]
> {
  return safeInvoke("list_main_chat_internal_issue_reports");
}

export async function getMainChatInternalIssueReport(
  reportId: string
): Promise<MainChatStage5IssueReport> {
  return safeInvoke("get_main_chat_internal_issue_report", { reportId, report_id: reportId });
}

export async function deleteMainChatInternalIssueReport(reportId: string): Promise<boolean> {
  return safeInvoke("delete_main_chat_internal_issue_report", { reportId, report_id: reportId });
}

export async function runMainChatStage5ReleaseDebugReport(): Promise<MainChatStage5ReleaseDebugReport> {
  return safeInvoke("run_main_chat_stage5_release_debug_report");
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
