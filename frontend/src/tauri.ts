import { invoke } from "@tauri-apps/api/core";
import type {
  LifeModel,
  ChatMessage,
  DailyGoal,
  StateHistoryEntry,
  StateAlert,
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

export type AgentRuntimeMode = "local_first_default" | "capability_first";
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
  generation_result?: MainChatGenerationResult;
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

export interface MainChatMemoryCandidateTrace {
  candidateId?: string;
  kind?: string;
  destination?: string;
  sourcePreview?: string;
  normalizedClaim?: string;
  sensitivity?: string;
  stability?: string;
  explicitness?: string;
  futureActionability?: string;
  confidence?: number;
  reasonCodes?: string[];
}

export interface MainChatMemoryGovernanceEvidence {
  candidateCount?: number;
  candidateTrace?: MainChatMemoryCandidateTrace[];
  lifeEventIds?: string[];
  memoryProposalIds?: string[];
  lifeModelProposalIds?: string[];
  sessionOnlyCandidateIds?: string[];
  noOpCandidateIds?: string[];
  blockers?: string[];
  directWritesExecuted?: boolean;
  directLifeModelWrite?: boolean;
  directMemoryWrite?: boolean;
  acceptedDurableTruthWritten?: boolean;
  localLifeEventCaptureExecuted?: boolean;
}

export interface MainChatGenerationResult {
  memoryGovernance?: MainChatMemoryGovernanceEvidence;
  [key: string]: any;
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
  status?: "completed" | "failed";
  blockers?: string[];
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  run_id?: string;
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  model_invoked?: boolean;
  tool_invoked?: boolean;
}

export interface StreamMessageStartPayload {
  session_id: string;
  run_id: string;
  status?: "completed" | "failed";
  blockers?: string[];
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  model_invoked?: boolean;
  tool_invoked?: boolean;
}

export interface StreamMessageDonePayload {
  session_id: string;
  run_id: string;
  reply: string;
  status?: "completed" | "failed";
  blockers?: string[];
  model_invoked?: boolean;
  tool_invoked?: boolean;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
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
  | "unknown";

export interface MainChatPlanArtifactSourceEvidence {
  evidenceId: string;
  sourceKind: string;
  sourceLabel: string;
  toolName?: string | null;
  preview?: string | null;
}

export interface MainChatPlanArtifactFactView {
  label: string;
  detail: string;
  evidenceIds: string[];
  sourceToolEvidence: MainChatPlanArtifactSourceEvidence[];
}

export interface MainChatPlanArtifactStepView {
  stepId: string;
  index: number;
  title: string;
  description: string;
  status: string;
  kind: string;
  evidenceIds: string[];
  sourceToolEvidence: MainChatPlanArtifactSourceEvidence[];
  controls: string[];
}

export interface MainChatPlanArtifactRouteEvidence {
  strategy: string;
  reason: string;
  confidence?: number | null;
  evidenceIds: string[];
}

export interface MainChatPlanArtifactRunEvidence {
  taskSessionId: string;
  runId: string;
  planSessionId: string;
  actionIds: string[];
  observationIds: string[];
  proposalIds: string[];
  blockerIds: string[];
  finalDeliveryId?: string | null;
  metadataSafe: boolean;
}

export interface MainChatPlanArtifactView {
  planId: string;
  planSessionId: string;
  taskSessionId: string;
  runId: string;
  status: string;
  title: string;
  summary: string;
  body: string;
  steps: MainChatPlanArtifactStepView[];
  assumptions: MainChatPlanArtifactFactView[];
  unknowns: MainChatPlanArtifactFactView[];
  controls: string[];
  routeEvidence: MainChatPlanArtifactRouteEvidence;
  runEvidence: MainChatPlanArtifactRunEvidence;
}

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
    artifactView?: MainChatPlanArtifactView | null;
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

export interface MainChatRuntimeStatus {
  statusVersion: 2;
  authoritativeRuntime: "main_chat_kernel";
  defaultSendPath: "main_chat_kernel";
  startStreamPath: "main_chat_kernel";
  sourceOfTruth: "main_chat_turn_pipeline";
  kernelEvidence: {
    kernelBackedDefault: boolean;
    finalGateEvidencePresent: boolean;
    finalGateReady: boolean;
    latestKernelRouteObserved: boolean;
  };
  latestRouteEvidence: {
    status: "observed" | "not_observed";
    directAnswerObserved: boolean;
    governedBlockerObserved: boolean;
    agentLoopObserved: boolean;
    kernelBackedDefaultObserved: boolean;
    lastKernelEventCount?: number;
    lastRouteReasonCode?: string | null;
    lastKernelSupportDisposition?: string | null;
  };
  finalGateReadiness: {
    authority: "main_chat_final_acceptance_gate";
    status: "ready" | "blocked" | "not_run";
    blockers: string[];
    lastReportRunId?: string | null;
  };
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

export async function getMainChatRuntimeStatus(): Promise<MainChatRuntimeStatus> {
  return safeInvoke<MainChatRuntimeStatus>("get_main_chat_runtime_status");
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

export interface PolicyRouterStatus {
  activeAuthority: string;
  authorityChain: string[];
  routeOutputs: string[];
  appStateOldRoutersPresent: boolean;
  diagnosticsSurface: string;
}

export async function getPolicyRouterStatus(): Promise<PolicyRouterStatus> {
  return safeInvoke<PolicyRouterStatus>("get_policy_router_status");
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
    | "scripted_provider_probe"
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

export type ProviderTransmissionStatus =
  | "sent"
  | "not_sent"
  | "blocked"
  | "unknown"
  | "not_instrumented"
  | string;

export interface ProviderTransmissionSourceRef {
  source: string;
  ref_id?: string | null;
  status?: string | null;
  route_type?: string | null;
}

export interface ProviderTransmissionHistoryItem {
  status: ProviderTransmissionStatus;
  run_id: string;
  task_session_id?: string | null;
  provider: string;
  model: string;
  route_type: string;
  reason: string;
  evidence_id: string;
  truth_confidence: "verified" | "inferred" | "unknown" | string;
  data_category: string;
  source_refs: ProviderTransmissionSourceRef[];
  started_at: string;
  finished_at?: string | null;
}

export interface SystemDiagnostics {
  policy_router: PolicyRouterStatus;
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
  database_status?: string;
  startup_warnings?: string[];
  snapshot_count: number;
  life_model_ready: boolean;
  app_version: string;
  model_empty: boolean;
  chat_session_count: number;
  usage_ready?: boolean;
  usage_readiness_issues?: string[];
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

export interface LifePendingProjection {
  pendingProposalCount: number;
  editedProposalCount: number;
  totalReviewRequiredCount: number;
  highRiskReviewRequiredCount: number;
  proposalStoreStatus: string;
  requiresUserAction: boolean;
}

export interface LifeReadinessProjection {
  chatReady: boolean;
  usageReady: boolean;
  lifeModelReady: boolean;
  modelEmpty: boolean;
  pendingBuilderReviewSessions: number;
  unfinishedBuilderSessions: number;
  databaseStatus: string;
  readinessIssues: string[];
  usageReadinessIssues: string[];
}

export interface LifeTaskStateProjection {
  taskStoreStatus: string;
  latestTaskId?: string | null;
  latestTaskStatus?: string | null;
  runningCount: number;
  waitingPermissionCount: number;
  blockedCount: number;
  failedCount: number;
  cancelledCount: number;
  completedCount: number;
  activeCount: number;
}

export interface LifeSafeModeProjection {
  active: boolean;
  reason: string;
  sourceRefs: string[];
}

export interface LifeToolPermissionProjection {
  totalCount: number;
  activeCount: number;
  consumedCount: number;
  allowCount: number;
  denyCount: number;
  askEveryTimeCount: number;
  allowOnceCount: number;
  allowUntilRevokedCount: number;
}

export interface LifeSurfaceProjection {
  surface: "today" | "mailbox" | "chat" | "companion" | "life_model" | "settings" | string;
  pendingReviewCount: number;
  editedReviewCount: number;
  totalReviewRequiredCount: number;
  readinessStatus: "ready" | "partial" | "blocked" | string;
  taskStatus: string;
  safeModeActive: boolean;
  waitingPermissionCount: number;
  activeToolPermissionCount: number;
}

export interface LifeStateProjection {
  version: string;
  generatedAt: string;
  pending: LifePendingProjection;
  readiness: LifeReadinessProjection;
  taskState: LifeTaskStateProjection;
  safeMode: LifeSafeModeProjection;
  toolPermissions: LifeToolPermissionProjection;
  safePaths: string[];
  surfaces: LifeSurfaceProjection[];
  sourceRefs: string[];
}

export async function getLifeStateProjection(): Promise<LifeStateProjection> {
  return safeInvoke<LifeStateProjection>("get_life_state_projection");
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

export async function rebuildMemoryIndex(
  confirmationEvidence?: DangerActionConfirmationEvidence
): Promise<{
  processed: number;
  indexed: number;
  skipped: number;
}> {
  return safeInvoke("rebuild_memory_index", {
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
  });
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

export type DangerActionType =
  | "data_export"
  | "data_import_overwrite"
  | "mcp_audit_export"
  | "mcp_audit_cleanup"
  | "mcp_audit_key_rotation"
  | "agent_run_delete"
  | "agent_run_bulk_delete"
  | "vector_rebuild";

export interface DangerActionPreflightView {
  actionType: DangerActionType;
  riskTier: "medium" | "high" | "critical" | string;
  scopeSummary: string;
  dataCategories: string[];
  writesDurableState: boolean;
  privacySensitive: boolean;
  externalTransmission: "not_sent_externally" | "sent_externally" | "unknown" | string;
  dryRunAvailable: boolean;
  backupStatus: string;
  requiresTypedConfirmation: boolean;
  confirmationRequired: boolean;
  confirmationPhrase?: string | null;
  confirmationScopeDigest: string;
  preflightId: string;
  affectedItemCount: number;
  affectedItemDigest: string;
  finalActionEnabled: boolean;
  safeModeBlocked: boolean;
  blockingReasons: string[];
  sourceRefs: string[];
}

export interface DangerActionConfirmationEvidence {
  actionType: DangerActionType;
  preflightId: string;
  confirmationPhrase: string;
  confirmationScopeDigest: string;
  safeMode: boolean;
  targetIds: string[];
}

export function buildDangerActionConfirmationEvidence(
  view: DangerActionPreflightView,
  targetIds: string[] = []
): DangerActionConfirmationEvidence {
  return {
    actionType: view.actionType,
    preflightId: view.preflightId,
    confirmationPhrase: view.confirmationPhrase ?? "",
    confirmationScopeDigest: view.confirmationScopeDigest,
    safeMode: view.safeModeBlocked,
    targetIds,
  };
}

export async function getDangerActionPreflight(
  actionType: DangerActionType,
  safeMode: boolean,
  options: { targetIds?: string[]; affectedCount?: number } = {}
): Promise<DangerActionPreflightView> {
  return safeInvoke<DangerActionPreflightView>("get_danger_action_preflight", {
    actionType,
    action_type: actionType,
    safeMode,
    safe_mode: safeMode,
    ...(options.targetIds ? { targetIds: options.targetIds, target_ids: options.targetIds } : {}),
    ...(options.affectedCount !== undefined
      ? { affectedCount: options.affectedCount, affected_count: options.affectedCount }
      : {}),
  });
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

export async function importAllData(
  payload: ExportPayload,
  confirmationEvidence?: DangerActionConfirmationEvidence
): Promise<DataImportResult> {
  return safeInvoke<DataImportResult>("import_all_data", {
    payload,
    importRequest: MANUAL_DATA_IMPORT_REQUEST,
    import_request: MANUAL_DATA_IMPORT_REQUEST,
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
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

export async function cleanupMcpAuditLogs(
  retentionDays: number,
  confirmationEvidence?: DangerActionConfirmationEvidence
): Promise<number> {
  return safeInvoke<number>("cleanup_mcp_audit_logs", {
    retentionDays,
    retention_days: retentionDays,
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
  });
}

export async function rotateMcpAuditKey(
  confirmationEvidence?: DangerActionConfirmationEvidence
): Promise<void> {
  return safeInvoke("rotate_mcp_audit_key", {
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
  });
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

export async function listProviderTransmissionHistory(
  limit: number = 20
): Promise<ProviderTransmissionHistoryItem[]> {
  return safeInvoke<ProviderTransmissionHistoryItem[]>("list_provider_transmission_history", {
    limit,
  });
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

export async function deleteAgentRun(
  runId: string,
  reason?: string,
  confirmationEvidence?: DangerActionConfirmationEvidence
): Promise<void> {
  return safeInvoke("delete_agent_run", {
    runId,
    run_id: runId,
    reason,
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
  });
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
    runtimeExecutionPerformed: boolean;
    modelCallPerformed: boolean;
    toolCallPerformed: boolean;
    businessWritesPerformed: boolean;
    blockers: string[];
  };
  defaultChatUnchanged: boolean;
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
