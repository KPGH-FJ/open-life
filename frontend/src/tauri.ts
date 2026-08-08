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

const MANUAL_SNAPSHOT_RESTORE_REQUEST = {
  purpose: "manual_restore",
  explicitUserIntent: true,
  createPreChangeSnapshot: true,
} as const;

function manualDataImportRequest(operationId: string) {
  return {
    operationId,
    purpose: "manual_restore",
    explicitUserIntent: true,
    createPreChangeSnapshot: true,
    importTargets: ["life_model", "messages", "vectors", "state_store"],
  } as const;
}

export type AgentRuntimeMode = "local_first_default" | "capability_first";
export type CloudApiValidationStatus =
  | "unconfigured"
  | "unvalidated"
  | "validated"
  | "failed"
  | "stale"
  | "unknown"
  | "remote_unknown"
  | "runtime_generation_incoherent"
  | "validation_record_corrupt"
  | "validation_record_io_error"
  | "scripted_provider_probe"
  | "scripted_dogfood";

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
    // Rust omits the runtime secret when serializing get_config. The field is
    // present only when the user submits a replacement credential.
    openai_key?: string;
    // Non-secret credential-store reference used only to represent presence.
    openai_key_ref?: string;
    credential_version?: number;
    embedding_model: string;
    chat_model: string;
    embedding_enabled?: boolean;
  };
  runtime_mode?: AgentRuntimeMode;
  prefer_local_model: boolean;
  local_model: string;
  experimental_context_assembler?: boolean;
  use_agent_loop?: boolean;
  system?: {
    ollama_cache_ttl_seconds?: number;
    memory_search_top_k?: number;
    safe_paths?: string[];
    workspace_memory_root?: string;
    project_memory_root?: string;
    search_provider?: "duckduckgo" | "brave" | "deepseek" | "searxng";
    search_provider_key?: string;
    search_provider_key_ref?: string;
    searxng_url?: string;
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

export interface ArtifactOutputDirectorySelection {
  cancelled: boolean;
  selectedPath: string | null;
}

export async function selectArtifactOutputDirectory(): Promise<ArtifactOutputDirectorySelection> {
  return safeInvoke<ArtifactOutputDirectorySelection>("select_artifact_output_directory");
}

export type MarkdownMemoryScope = "workspace" | "project";

export interface MarkdownMemoryRootSelection {
  cancelled: boolean;
  scope: MarkdownMemoryScope;
  selectedPath: string | null;
}

export interface MarkdownMemoryRootView {
  scope: MarkdownMemoryScope;
  configured: boolean;
  rootPath: string | null;
  status: "ready" | "unavailable" | "unconfigured";
}

export interface MarkdownMemoryFileView {
  scope: MarkdownMemoryScope;
  relativePath: string;
  content: string;
  contentDigest: string;
  charCount: number;
  active: boolean;
}

export interface MarkdownMemoryViewModel {
  roots: MarkdownMemoryRootView[];
  files: MarkdownMemoryFileView[];
  totalCharCount: number;
  truncated: boolean;
  sourceRule: string;
}

export interface MarkdownMemoryProposalReceipt {
  proposalId: string;
  scope: MarkdownMemoryScope;
  relativePath: string;
  operation: "write" | "deactivate";
  status: "review_required";
}

export async function selectMarkdownMemoryRoot(
  scope: MarkdownMemoryScope
): Promise<MarkdownMemoryRootSelection> {
  return safeInvoke<MarkdownMemoryRootSelection>("select_markdown_memory_root", { scope });
}

export async function getMarkdownMemoryViewModel(): Promise<MarkdownMemoryViewModel> {
  return safeInvoke<MarkdownMemoryViewModel>("get_markdown_memory_view_model");
}

export async function draftMarkdownMemoryFileProposal(request: {
  scope: MarkdownMemoryScope;
  relativePath: string;
  content: string;
  expectedCurrentDigest?: string;
}): Promise<MarkdownMemoryProposalReceipt> {
  return safeInvoke<MarkdownMemoryProposalReceipt>("draft_markdown_memory_file_proposal", {
    request,
  });
}

export async function deactivateMarkdownMemoryFileProposal(request: {
  scope: MarkdownMemoryScope;
  relativePath: string;
  expectedCurrentDigest: string;
}): Promise<MarkdownMemoryProposalReceipt> {
  return safeInvoke<MarkdownMemoryProposalReceipt>("deactivate_markdown_memory_file_proposal", {
    request,
  });
}

export interface CredentialRecoveryItem {
  purpose:
    | "agent_run_receipts"
    | "main_chat_events"
    | "action_queue"
    | "task_store"
    | "mcp_audit"
    | "provider_api_key";
  status:
    | CredentialBootstrapStatus
    | "created"
    | "access_restored"
    | "compensated"
    | "cleanup_unknown";
}

export interface CredentialRecoveryReport {
  items: CredentialRecoveryItem[];
  initializationCompletedForRestart: boolean;
  restartRequired: boolean;
  cleanupStatus: "not_required" | "compensated" | "unknown";
  blockedReason?: string | null;
  bootstrapSnapshotDigest: string;
}

export async function recoverRequiredCredentialAccess(): Promise<CredentialRecoveryReport> {
  return safeInvoke<CredentialRecoveryReport>("recover_required_credential_access");
}

// DEPRECATED: use sendMessageV2 for full trace support
export interface MainChatMessageOptions {
  operationId: string;
  selectedSkillId?: string;
}

export async function sendMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions
): Promise<string> {
  const result = await safeInvoke<SendMessageResult>("send_message", {
    operationId: options.operationId,
    operation_id: options.operationId,
    ...sessionArgs(sessionId),
    messages,
    ...selectedSkillArgs(options.selectedSkillId),
  });
  return result.reply;
}

export type ToolCallStatus = "success" | "error" | "pending" | "blocked" | "needs_confirmation";

export type ProductToolCallStatus =
  | "success"
  | "failed"
  | "effect_unknown"
  | "not_dispatched"
  | "locally_aborted"
  | "remote_unknown"
  | "unknown";

export interface ProductToolReference {
  id: string;
  source: string;
}

export type ToolActionEffect =
  | "read_only"
  | "local_mutation"
  | "external_mutation"
  | "proposal_only"
  | "unknown";

export type ToolIdempotencyContract = "unspecified" | "non_idempotent" | "idempotent";

export type ToolDispatchKind =
  | "not_attempted"
  | "local"
  | "network"
  | "mcp_stdio"
  | "a2a"
  | "simulated"
  | "unknown";

export type ToolTransportStatus =
  | "not_attempted"
  | "dispatched"
  | "response_observed"
  | "local_aborted"
  | "remote_unknown";

export type ToolEffectStatus = "not_attempted" | "confirmed" | "unknown";
export type ToolExecutionOutcome = "not_observed" | "succeeded" | "failed" | "unknown";
export type ToolAuditPersistenceStatus =
  | "not_required"
  | "pending"
  | "committed"
  | "failed"
  | "unknown";

export interface ProductToolExecutionReceipt {
  receiptRef: string;
  requestDigest: string;
  actionEffect: ToolActionEffect;
  idempotencyContract: ToolIdempotencyContract;
  dispatchKind: ToolDispatchKind;
  dispatchAttemptCount: number;
  dispatchObserved: boolean;
  transportStatus: ToolTransportStatus;
  effectStatus: ToolEffectStatus;
  outcome: ToolExecutionOutcome;
  auditPersistenceStatus: ToolAuditPersistenceStatus;
  verified: boolean;
}

export type ProductToolFailureCode =
  | "tool_failed"
  | "tool_effect_unknown"
  | "tool_remote_state_unknown"
  | "tool_locally_aborted"
  | "tool_not_dispatched"
  | "tool_state_unknown"
  | "tool_evidence_unverified";

export interface ProductToolCallResult {
  toolRef: ProductToolReference;
  actionRef: string;
  runRef?: string;
  status: ProductToolCallStatus;
  requiresConfirmation: boolean;
  failureCode?: ProductToolFailureCode;
  privacyWarningCount: number;
  proposalRef?: string;
  executionReceipt?: ProductToolExecutionReceipt;
  outputReceipt?: ContentReceipt;
}

export type ToolCallResult = ProductToolCallResult;

export interface ProductReactActionTrace {
  actionId: string;
  stepIndex: number;
  toolCallIndex: number;
  actionType: string;
  toolName: string;
  toolSource: string;
  actionCategory: string;
  riskLevel: string;
  permissionDecision?: string;
  status: string;
  proposalId?: string;
  observationId?: string;
  outputPreview?: string;
  outputReceipt?: ContentReceipt;
  startedAt?: string;
  finishedAt?: string;
  metadataSafe: boolean;
}

/**
 * Client-only compatibility shape for historical fixtures and archived views.
 * Shipped commands return ProductReactActionTrace and do not promise these
 * legacy optional fields.
 */
export interface ReactActionTraceEnvelope extends ProductReactActionTrace {
  runId?: string;
  toolId?: string;
  observationStatus?: string;
  outputItemCount?: number;
}

export interface ContentReceipt {
  version: number;
  kind: "tool_output" | "tool_error";
  provenance: "observed_tool_adapter_body";
  byteCount: number;
  digest: string;
  verified: boolean;
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
  status?: MainChatTurnStatus;
  blockers?: string[];
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  run_id?: string;
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
  turn_terminal?: OpenLifeTurnTerminal;
}

export interface ImportedResourceReceipt {
  resourceId: string;
  bindingId: string;
  filename: string;
  digest: string;
  byteCount: number;
  chunkCount: number;
  reusedExisting: boolean;
  eventId?: string | null;
}

export interface ResourceImportReceipt {
  operationId: string;
  messageId: string;
  resources: ImportedResourceReceipt[];
  committedAt: string;
}

export interface ResourceImportSelectionResult {
  cancelled: boolean;
  receipt: ResourceImportReceipt | null;
}

export interface ResourceImportStatus {
  status: "active" | "committed" | "not_found";
  receipt: ResourceImportReceipt | null;
}

export interface ResourceDetachReceipt {
  operationId: string;
  messageId: string;
  resourceId: string;
  bindingRemoved: boolean;
  resourceDeleted: boolean;
  eventId: string;
  committedAt: string;
}

export type ProviderInvocationStatus =
  | "not_attempted"
  | "started"
  | "completed"
  | "failed"
  | "locally_aborted"
  | "remote_unknown"
  | "invalid";

export type MainChatTurnStatus =
  | "completed"
  | "completed_with_pending_items"
  | "blocked"
  | "failed"
  | "remote_unknown"
  | "cancelled"
  | "interrupted";

export interface OpenLifeTurnTerminal {
  runtimeOwner: string;
  status: MainChatTurnStatus;
  state: string;
  runId?: string | null;
  taskSessionId?: string | null;
  blockers: string[];
  proposals: string[];
  legacyFallbackUsed: boolean;
  legacyRuntimeInvoked: boolean;
  singleStepFallbackUsed: boolean;
  directWritesExecuted: boolean;
  providerInvocationStatus: ProviderInvocationStatus;
  modelInvoked: boolean;
  toolInvoked: boolean;
  finalDelivery: ProductFinalDeliveryView;
}

export interface ProductFinalDeliveryView {
  deliveryRef: string;
  taskRef: string;
  runRef: string;
  status: MainChatTurnStatus | "unknown";
  completedActionCount: number;
  observationCount: number;
  proposalCount: number;
  blockerCount: number;
  pendingUserActionCount: number;
  durableChangeCount: number;
  nextStepCount: number;
  traceAvailable: boolean;
  kernelEventCount: number | null;
  durableEventCount: number;
  hasAssistantMessage: boolean;
  toolCallCount: number;
}

export interface StreamMessageStartPayload {
  session_id: string;
  operation_id: string;
  task_session_id: string;
  run_id: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
}

export interface StreamMessageChunkPayload {
  session_id: string;
  operation_id: string;
  task_session_id: string;
  run_id: string;
  request_id?: string;
  chunk: string;
}

export interface StreamMessageDonePayload {
  session_id: string;
  operation_id: string;
  task_session_id: string;
  run_id: string;
  reply: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
  reasoning_trace: ReasoningTrace;
  tool_calls: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  agent_state?: MainChatAgentStateSnapshot;
  execution_transcript?: MainChatExecutionTranscriptEntry[];
  turn_terminal?: OpenLifeTurnTerminal;
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
    providerConfigGeneration?: string;
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
  | "reflection"
  | "fallback";

export interface MainChatExecutionTranscriptEntry {
  id: string;
  sessionId: string;
  kind: MainChatExecutionTranscriptKind;
  summary: string;
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

export interface ProductExecutionPolicyDecision {
  level: string;
  reasonCode: string;
  executionAllowed: boolean;
  requiresConfirmation: boolean;
  requiresProposal: boolean;
  requiresBlocker: boolean;
  silentWriteAllowed: boolean;
}

export interface ProductQueuedExecutionAction {
  id: string;
  sessionId: string;
  actionType: string;
  policy: ProductExecutionPolicyDecision;
  status: MainChatExecutionQueueStatus;
  attempts: number;
  revision: number;
  failureCode?: "action_failed" | "action_blocked";
  createdAt: string;
  updatedAt: string;
}

export interface ProductTaskSession {
  id: string;
  chatSessionId: string;
  selectedStrategy: MainChatAgentStrategy;
  status: MainChatAgentTaskStatus;
  actionQueueIds: string[];
  pendingBlockers: string[];
  contextSnapshotCount: number;
  hasPlanSummary: boolean;
  hasFinalSummary: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ProductTaskProposal {
  id: string;
  runRef?: string;
  proposalType: ProposalType;
  source: ProposalSource;
  riskLevel: RiskLevel;
  status: ProposalStatus;
  createdAt: string;
  resolvedAt?: string;
  expiresAt?: string;
}

export interface MainChatAgentTaskState {
  session?: ProductTaskSession | null;
  actions: ProductQueuedExecutionAction[];
  transcript: MainChatExecutionTranscriptEntry[];
  pendingApprovalCount: number;
  activeToolCount: number;
  canResume: boolean;
  canCancel: boolean;
  canRetry: boolean;
  cancellationPending: boolean;
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
  routeEvidence: ProductRouteEvidence | null;
  evidenceView: ProductRunEvidenceView;
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
  taskSession: ProductTaskSession;
  actions: ProductQueuedExecutionAction[];
  transcript: MainChatExecutionTranscriptEntry[];
  proposals: ProductTaskProposal[];
  blockers: string[];
  finalDelivery?: Record<string, unknown> | null;
  continuityDiagnostics: MainChatContinuityDiagnostics;
  allowedControls: string[];
  nextRecommendedControl: string;
  lastSafeResumePoint?: string | null;
  retryTargetActionId?: string | null;
  contextDigest: string;
  selectedSkillDigest?: string | null;
  toolManifestDigest: string;
  evidenceView: ProductRunEvidenceView;
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

export interface DurableTurnLifecycleReceiptView {
  eventId: string;
  runId: string;
  sequence: number;
  eventType: string;
  sourceRef: string;
  lifecycleState: string;
  failureKind: string | null;
  createdAt: string;
  payloadDigest: string;
}

export interface ProductRunEvidenceView {
  runId: string | null;
  taskSessionId: string;
  title: string;
  lifecycleState: string;
  projectionState: string;
  identityState: string;
  snapshotState: string;
  durableSequenceBefore: number | null;
  durableSequenceAfter: number | null;
  durableLifecycleReceipt: DurableTurnLifecycleReceiptView | null;
  routeEvidence: ProductRouteEvidence | null;
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

export async function openExternalHttpsSource(url: string): Promise<void> {
  return safeInvoke("open_external_https_source", { url });
}

export async function sendMessageV2(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions
): Promise<SendMessageResult> {
  return safeInvoke<SendMessageResult>("send_message", {
    operationId: options.operationId,
    operation_id: options.operationId,
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

export async function startStreamMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions
): Promise<StreamMessageDonePayload> {
  const payload = {
    operationId: options.operationId,
    operation_id: options.operationId,
    ...sessionArgs(sessionId),
    messages,
    ...selectedSkillArgs(options.selectedSkillId),
  };
  return safeInvoke<StreamMessageDonePayload>("start_stream_message", {
    ...payload,
    args: payload,
  });
}

export async function pickAndImportResources(
  importOperationId: string,
  turnOperationId: string
): Promise<ResourceImportSelectionResult> {
  return safeInvoke<ResourceImportSelectionResult>("pick_and_import_resources", {
    importOperationId,
    import_operation_id: importOperationId,
    turnOperationId,
    turn_operation_id: turnOperationId,
  });
}

export async function cancelResourceImport(operationId: string): Promise<boolean> {
  return safeInvoke<boolean>("cancel_resource_import", {
    operationId,
    operation_id: operationId,
  });
}

export async function getResourceImportStatus(operationId: string): Promise<ResourceImportStatus> {
  return safeInvoke<ResourceImportStatus>("get_resource_import_status", {
    operationId,
    operation_id: operationId,
  });
}

export async function detachResourceFromTurn(
  operationId: string,
  turnOperationId: string,
  resourceId: string
): Promise<ResourceDetachReceipt> {
  return safeInvoke<ResourceDetachReceipt>("detach_resource_from_turn", {
    operationId,
    operation_id: operationId,
    turnOperationId,
    turn_operation_id: turnOperationId,
    resourceId,
    resource_id: resourceId,
  });
}

// Note: Hermes dispatch command has been removed. Use AgentRuntime instead.

export async function getChatHistory(sessionId: string): Promise<ChatMessage[]> {
  return safeInvoke<ChatMessage[]>("get_chat_history", sessionArgs(sessionId));
}

export async function saveChatMessage(
  sessionId: string,
  message: ChatMessage,
  operationId: string
): Promise<void> {
  return safeInvoke("save_chat_message", {
    ...sessionArgs(sessionId),
    message,
    operationId,
    operation_id: operationId,
  });
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
  devExtensionsEnabled: boolean;
  authenticatedDevA2aEnabled: boolean;
  unauthenticatedDevA2aEnabled: boolean;
  arbitraryMcpRegistrationEnabled: boolean;
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

export interface ProductRouteIdentity {
  provider: string;
  model_ref: string;
  route_type: string;
  privacy_level: string;
  reason_ref: string;
  provider_health_is_estimated: boolean;
}

export interface ProductProviderReadiness {
  configured: boolean;
  credential_present: boolean;
  validated: boolean;
  validation_status: string;
  preferred: string;
  actually_used: string | null;
  stale: boolean;
  failed: boolean;
  last_checked_at: string | null;
}

export interface ProductFallbackEvidence {
  from_route: ProductRouteIdentity | null;
  to_route: ProductRouteIdentity | null;
  reason_ref: string;
  blocker_codes: string[];
}

export interface ProductRouteSourceRef {
  source: string;
  ref_id: string | null;
  status: string | null;
  route_type: string | null;
}

export interface ProductRouteEvidence {
  evidence_id: string;
  generated_at: string;
  conversation_id: string | null;
  run_id: string | null;
  task_session_id: string | null;
  answer_scope: string;
  planned_route: ProductRouteIdentity | null;
  actual_route: ProductRouteIdentity | null;
  last_completed_route: ProductRouteIdentity | null;
  provider_readiness: ProductProviderReadiness;
  fallback: ProductFallbackEvidence | null;
  external_transmission: string;
  source_refs: ProductRouteSourceRef[];
  truth_confidence: string;
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

export interface PersistenceHealthSnapshot {
  mode:
    | "initializing"
    | "read_write"
    | "read_only_degraded"
    | "unavailable_degraded"
    | "ephemeral_development"
    | "isolated_evaluation";
  canonicalWritesAllowed: boolean;
  providerDispatchAllowed: boolean;
  toolDispatchAllowed: boolean;
  liveOrCanonicalCreditEligible: boolean;
  sealed: boolean;
  stores: Array<{
    store: string;
    mode: "read_write_canonical" | "read_only_canonical" | "unavailable" | "ephemeral_development";
    reasonCode?: string | null;
    errorDigest?: string | null;
  }>;
  globalReasonCodes: string[];
}

export interface SystemDiagnostics {
  persistence_health?: PersistenceHealthSnapshot;
  policy_router: PolicyRouterStatus;
  mcp_server_count: number;
  mcp_tool_count: number;
  mcp_recent_audit_count: number;
  mcp_recent_pii_count: number;
  memory_chunk_count: number;
  vector_corrupt_embedding_count?: number;
  vector_unknown_profile_count?: number;
  vector_profile_dimension_mismatch_count?: number;
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
  cloud_api_validation_status?: CloudApiValidationStatus;
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

export type CredentialBootstrapStatus =
  | "available"
  | "initialization_required"
  | "missing_existing_data"
  | "invalid"
  | "unavailable"
  | "unknown";

export interface CredentialBootstrapSnapshot {
  version: string;
  digest: string;
  purposes: Array<{
    purpose:
      | "agent_run_receipts"
      | "main_chat_events"
      | "action_queue"
      | "task_store"
      | "mcp_audit"
      | "provider_api_key";
    status: CredentialBootstrapStatus;
  }>;
}

export interface LifeStateProjection {
  version: string;
  generatedAt: string;
  persistence: PersistenceHealthSnapshot;
  pending: LifePendingProjection;
  readiness: LifeReadinessProjection;
  taskState: LifeTaskStateProjection;
  safeMode: LifeSafeModeProjection;
  credentialBootstrap?: CredentialBootstrapSnapshot;
  toolPermissions: LifeToolPermissionProjection;
  safePaths: string[];
  surfaces: LifeSurfaceProjection[];
  sourceRefs: string[];
}

export async function getLifeStateProjection(): Promise<LifeStateProjection> {
  return safeInvoke<LifeStateProjection>("get_life_state_projection");
}

// Backend-owned shared product read-model contract.
// Canonical Rust owner: openlife-core/src/agent/product_read_model.rs.
export type ViewModelStatus = "loading" | "ready" | "empty" | "error" | "stale";

export type EvidenceSource =
  | "backend-readmodel"
  | "audit"
  | "task"
  | "review"
  | "memory"
  | "lifemodel"
  | "settings"
  | "provider";

export type EvidenceSensitivity = "public" | "local_private" | "sensitive" | "redacted";

export type EvidenceRef = {
  id: string;
  label: string;
  source: EvidenceSource;
  sensitivity?: EvidenceSensitivity;
};

export type ViewModelWarningSeverity = "info" | "warning" | "error";

export type ViewModelWarning = {
  code: string;
  message: string;
  severity: ViewModelWarningSeverity;
  evidenceRefs?: EvidenceRef[];
};

export type ProductActionKind =
  | "open"
  | "start"
  | "continue"
  | "retry"
  | "cancel"
  | "refresh"
  | "inspect"
  | "configure";

export type ProductAction = {
  id: string;
  label: string;
  kind: ProductActionKind;
  enabled: boolean;
  disabledReason?: string;
  targetRef?: string;
};

export type ReviewItemMaterializationStatus =
  | "not_applicable"
  | "not_started"
  | "applying"
  | "applied"
  | "failed"
  | "rolled_back"
  | "unknown";

export type ReviewActionBase = {
  id: string;
  label: string;
  enabled: boolean;
  disabledReason?: string;
  requiresConfirmation?: boolean;
  targetReviewItemId: string;
  expectedMaterializationStatusAfterDispatch?: ReviewItemMaterializationStatus;
  completionProofAfterDispatch: boolean;
};

export type ReviewActionKindEffectInvariant =
  | { kind: "approve" | "reject" | "edit" | "later" | "revoke"; effect: "decision_only" }
  | { kind: "apply"; effect: "materialization_request" }
  | { kind: "resume"; effect: "task_resume_request" }
  | { kind: "view_evidence"; effect: "evidence_only" };

export type ReviewAction = ReviewActionBase & ReviewActionKindEffectInvariant;

export type DebugAction = {
  id: string;
  label: string;
  kind: "raw_trace" | "raw_json" | "export" | "provider_health" | "route_evidence" | "transcript";
  enabled: boolean;
  developerOnly?: boolean;
  targetRef?: string;
};

export type ViewModelEnvelope<T> = {
  data: T | null;
  status: ViewModelStatus;
  lastUpdatedAt: string | null;
  source: "backend-readmodel";
  evidenceRefs?: EvidenceRef[];
  warnings?: ViewModelWarning[];
  actions: {
    primary: ProductAction[];
    review?: ReviewAction[];
    debugOnly?: DebugAction[];
  };
};

export type ProductRiskLevel = "none" | "low" | "medium" | "high" | "critical" | "unknown";

export type ProviderPrivacyBoundarySummary = {
  routeType: "local" | "cloud" | "hybrid" | "auto" | "unknown";
  externalTransmission: "not_sent" | "sent" | "possible" | "unknown";
  providerLabel: string;
  modelLabel: string;
  privacyLabel: string;
  risk: ProductRiskLevel;
  localOnlyRequired: boolean;
  blockedReason?: string;
  evidenceRefs: EvidenceRef[];
};

export type MemoryLane =
  | "turn_context"
  | "episodic_life_event"
  | "semantic_fact_preference"
  | "procedural_rule"
  | "evidence_record"
  | "canonical_lifemodel_truth";

export type BackendEntityRef = {
  id: string;
  kind:
    | "task"
    | "run"
    | "conversation"
    | "review_item"
    | "memory"
    | "lifemodel"
    | "proposal"
    | "tool_permission"
    | "evidence"
    | "external_resource"
    | "schedule"
    | "policy";
  label: string;
  href?: string;
};

export type ReviewItemType =
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
  | "life_model_update"
  | "unsupported";

export type ReviewItemDecisionStatus =
  | "pending"
  | "approved"
  | "rejected"
  | "edited"
  | "deferred"
  | "unknown";

export type ReviewItemSource = {
  kind: "proposal";
  proposalId: string;
  proposalSource: string;
  sourceDetail?: string;
  runId?: string;
};

export type ReviewItemTaskResumeRelation = {
  taskSessionId: string;
  resumeRequiresMaterialization?: boolean;
  canRequestResume: boolean;
  resumeActionId?: string;
  blockedReason?: string;
};

export type ReviewReadableValue = {
  kind: "text" | "number" | "boolean" | "list" | "object" | "redacted" | "unknown";
  summary: string;
  detail?: string;
  sensitivity: "public" | "local_private" | "sensitive" | "redacted";
  truncated: boolean;
};

export type PermissionTransmissionBoundary = {
  externalTransmission: "not_sent" | "sent" | "possible" | "unknown";
  summary: string;
  targetLabel?: string;
  evidenceRefs: EvidenceRef[];
};

export type PermissionDecisionContext = {
  status: "ready" | "incomplete";
  scopeKind: "action_bound" | "network_policy" | "unknown";
  policy: "allow_once" | "unknown";
  toolLabel: string;
  toolName?: string;
  capabilityLabels: string[];
  requestedTargetLabel?: string;
  resolvedTargetLabel?: string;
  purposeSummary: string;
  scopeDigest?: string;
  requestDigestKind: "input" | "endpoint" | "unknown";
  requestDigest?: string;
  requestLengthBytes?: number;
  blockedRunId?: string;
  blockedStepIndex?: number;
  networkPolicyDecisionId?: string;
  transmissionBoundary: PermissionTransmissionBoundary;
  expiresAt?: string;
  revocationSummary: string;
  missingFields: string[];
  evidenceRefs: EvidenceRef[];
};

export type ReviewDecisionContext = {
  reviewItemId: string;
  title: string;
  summary: string;
  before?: ReviewReadableValue;
  after: ReviewReadableValue;
  reasonSummary: string;
  sourceSummary: string;
  impactSummary: string;
  affectedObjectLabels: string[];
  expiresAt?: string;
  permission?: PermissionDecisionContext;
  actionContract?: {
    capabilityId: string;
    operation:
      | "create"
      | "overwrite"
      | "move"
      | "trash"
      | "restore"
      | "create_local_calendar_projection"
      | "create_scheduled_task"
      | "open_email_draft"
      | "open_browser_url"
      | "run_local_utility"
      | "export_data"
      | string;
    confirmationSummary: string;
    terminalEvidenceSummary: string;
    effectBoundary: string;
  };
  evidenceRefs: EvidenceRef[];
};

export type ReviewItem = {
  id: string;
  type: ReviewItemType;
  source: ReviewItemSource;
  status: ReviewItemDecisionStatus;
  materializationStatus: ReviewItemMaterializationStatus;
  decisionContext: ReviewDecisionContext;
  allowedActions: ReviewAction[];
  risk: ProductRiskLevel;
  expiresAt?: string;
  evidenceRefs: EvidenceRef[];
  targetRefs: BackendEntityRef[];
  taskResumeRelation?: ReviewItemTaskResumeRelation;
  artifactEvidence?: {
    state: "prepared" | "staged" | "confirmed" | "failed_before_effect" | "unknown" | string;
    targetReferenceDigest: string;
    contentDigest: string;
    observedContentDigest?: string;
    byteSize: number;
    mediaType: string;
    errorCode?: string;
  };
};

export type ReviewCenterSummary = {
  total: number;
  actionRequiredCount: number;
  blockedActionCount: number;
  byStatus: Record<string, number>;
  byRisk: Record<string, number>;
  byMaterializationStatus: Record<string, number>;
};

export type ReviewBatchDomain =
  | "memory"
  | "life_model"
  | "tool_permission"
  | "external_action"
  | "other";

export type ReviewBatch = {
  id: string;
  domain: ReviewBatchDomain;
  sessionId?: string;
  itemIds: string[];
  actionRequiredCount: number;
  highestRisk: ProductRiskLevel;
};

export type ReviewCenterViewModel = {
  batches: ReviewBatch[];
  items: ReviewItem[];
  summary: ReviewCenterSummary;
};

// Backend-owned LifeModel read-model contract.
// Canonical Rust owner: openlife-core/src/agent/life_model_view_model.rs.
export type LifeModelTruthMode =
  | "canonical"
  | "current_compatibility"
  | "candidate"
  | "pending_review"
  | "manual_override"
  | "unknown"
  | "unavailable";

export type LifeModelDimensionId = "identity" | "goals" | "capabilities" | "state";

export type LifeModelConfidence = "low" | "medium" | "high" | "unknown";

export type LifeModelOwnerStatus = "PARTIAL" | "PHASE_2_REQUIRED" | "UNKNOWN";

export type LifeModelProvenance = "limited" | "unknown" | "PHASE_2_REQUIRED";

export type LifeModelReviewItemRef = BackendEntityRef & {
  kind: "review_item";
};

export type LifeModelCanonicalSummary = {
  lifeModelRef: BackendEntityRef;
  title: string;
  summary: string;
  versionLabel: string;
  lastMaterializedAt: string | null;
  evidenceRefs: EvidenceRef[];
  humanProjection: LifeModelHumanProjectionV2;
};

export type LifeModelHumanProjectionV2 = {
  schemaVersion: "openlife.lifemodel.v2.yaml-projection.v1";
  modelId: string;
  modelVersion: number;
  itemCount: number;
  documentDigest: string;
  yamlContentDigest: string;
  projectionDigest: string;
  yaml: string;
};

export type LegacyLifeModelMigrationItemV2 = {
  sourcePath: string;
  valuePreview: string;
  valueDigest: string;
  valueTruncated: boolean;
  disposition:
    | "review_required"
    | "external_owner"
    | "manual_classification"
    | "not_migrated"
    | "migration_metadata";
  targetOwner:
    | "life_model_v2"
    | "state_store"
    | "tasks"
    | "agent_memory"
    | "agent_runtime"
    | "migration_metadata"
    | "legacy_compatibility_projection"
    | "unassigned";
  targetSection:
    | "identity"
    | "values"
    | "long_term_goals"
    | "stable_preferences"
    | "personal_boundaries"
    | "important_relationships"
    | "capabilities"
    | "resources"
    | "decision_principles"
    | "collaboration_preferences"
    | null;
  reasonCode: string;
  sensitive: boolean;
};

export type LegacyLifeModelMigrationPreviewV2 = {
  schemaVersion: string;
  sourceDigest: string;
  items: LegacyLifeModelMigrationItemV2[];
  reviewRequiredCount: number;
  externalOwnerCount: number;
  manualClassificationCount: number;
  notMigratedCount: number;
  migrationMetadataCount: number;
  containsSensitiveItems: boolean;
};

export type LifeModelCurrentViewSummary = {
  currentViewRef: BackendEntityRef;
  compatibilityMode: boolean;
  label: string;
  summary: string;
  divergenceFromCanonical: "none" | "minor" | "material" | "unknown";
  evidenceRefs: EvidenceRef[];
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelDimensionSummary = {
  id: LifeModelDimensionId;
  label: string;
  summary: string;
  confidence: LifeModelConfidence;
  stale: boolean;
  pendingReviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  provenance: LifeModelProvenance;
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelTrustQualityState = {
  readiness: "not_built" | "limited" | "usable_with_limits" | "ready" | "stale" | "unknown";
  completionScore: number | null;
  missingDimensionCount: number;
  staleDimensionCount: number;
  warningRefs: EvidenceRef[];
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelPendingUpdateCounts = {
  candidate: number;
  pendingReview: number;
  approvedNotApplied: number;
  failedMaterialization: number;
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelCandidateChange = {
  changeRef: BackendEntityRef;
  title: string;
  changeKind: "add" | "update" | "remove" | "merge" | "manual_override" | "unknown";
  affectedDimensionIds: string[];
  reviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  decisionStatus: "pending" | "accepted" | "edited" | "postponed" | "unknown";
};

export type LifeModelMaterializedChange = {
  changeRef: BackendEntityRef;
  title: string;
  materializationStatus: ReviewItemMaterializationStatus;
  materializedAt: string | null;
  rollbackAvailable: boolean;
  evidenceRefs: EvidenceRef[];
};

export type LifeModelManualOverrideState = {
  active: boolean;
  blockedReason?: string;
  draftRef: BackendEntityRef | null;
  saveAction: ProductAction | null;
  reviewItemRefs: LifeModelReviewItemRef[];
  evidenceRefs: EvidenceRef[];
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelMemoryLinkageSummary = {
  linkedMemoryCount: number;
  candidateMemoryCount: number;
  materializedMemoryCount: number;
  conflictCount: number;
  memoryRefs: BackendEntityRef[];
  evidenceRefs: EvidenceRef[];
  linkageStatus: "partial" | "unknown";
  tierSummary: {
    total: number | null;
    tier1: number | null;
    tier2: number | null;
    tier3: number | null;
    archived: number | null;
  };
  ownerStatus: LifeModelOwnerStatus;
};

export type LifeModelViewModel = {
  truthMode: LifeModelTruthMode;
  canonicalSummary: LifeModelCanonicalSummary | null;
  legacyMigrationPreview: LegacyLifeModelMigrationPreviewV2 | null;
  currentViewSummary: LifeModelCurrentViewSummary | null;
  dimensionSummaries: LifeModelDimensionSummary[];
  trustQualityState: LifeModelTrustQualityState;
  pendingUpdateCounts: LifeModelPendingUpdateCounts;
  provenanceRefs: EvidenceRef[];
  candidateChanges: LifeModelCandidateChange[];
  materializedChanges: LifeModelMaterializedChange[];
  manualOverrideState: LifeModelManualOverrideState | null;
  relatedReviewItemRefs: LifeModelReviewItemRef[];
  memoryLinkage: LifeModelMemoryLinkageSummary;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

// Backend-owned task and workspace read-model contracts.
// Canonical Rust owner: openlife-core/src/agent/tasks_view_model.rs.
export type TaskLifecycleStatus =
  | "running"
  | "waiting_permission"
  | "blocked"
  | "failed"
  | "remote_unknown"
  | "cancelled"
  | "completed"
  | "completed_with_pending_review"
  | "completed_needs_evidence"
  | "unknown";

export type TaskTerminalDeliveryStatus =
  | "not_terminal"
  | "delivered"
  | "missing_final_delivery_evidence"
  | "completed_with_pending_review"
  | "blocked"
  | "failed"
  | "cancelled"
  | "unknown";

export type TaskControlKind =
  | "resume"
  | "retry"
  | "cancel"
  | "refresh_context"
  | "open_trace"
  | "open_run"
  | "open_review_item"
  | "view_evidence";

export type TaskControlEffect =
  | "task_resume_request"
  | "task_retry_request"
  | "task_cancel_request"
  | "task_refresh_request"
  | "navigation_only"
  | "evidence_only";

export type TaskControl = {
  id: string;
  label: string;
  kind: TaskControlKind;
  effect: TaskControlEffect;
  enabled: boolean;
  disabledReason?: string;
  requiresConfirmation?: boolean;
  targetTaskId: string;
  targetActionId?: string;
  completionProofAfterDispatch?: boolean;
};

export type TaskLatestResultPreview = {
  status: TaskTerminalDeliveryStatus;
  label: string;
  preview?: string;
  finalDeliveryRef?: BackendEntityRef;
  evidenceRefs: EvidenceRef[];
};

export type TaskViewModelItem = {
  canonicalTaskId: string;
  taskSessionId?: string;
  relatedRunIds: string[];
  conversationId?: string;
  title: string;
  strategy: string;
  lifecycleStatus: TaskLifecycleStatus;
  terminalDeliveryStatus: TaskTerminalDeliveryStatus;
  finalDeliveryEvidencePresent: boolean;
  pendingBlockers: string[];
  pendingReviewItemRefs: BackendEntityRef[];
  allowedControls: TaskControl[];
  nextRecommendedControl: string;
  latestResultPreview?: TaskLatestResultPreview;
  evidenceRefs: EvidenceRef[];
  updatedAt?: string;
};

export type TasksViewModelSummary = {
  total: number;
  activeCount: number;
  waitingPermissionCount: number;
  blockedCount: number;
  pendingReviewCount: number;
  completedCount: number;
  completedNeedsEvidenceCount: number;
  failedCount: number;
  cancelledCount: number;
  byLifecycleStatus: Record<string, number>;
};

export type TasksViewModel = {
  items: TaskViewModelItem[];
  summary: TasksViewModelSummary;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

export type WorkspaceActivityKind =
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
  | "reflection"
  | "fallback"
  | "blocker"
  | "durable_lifecycle"
  | "unknown";

export type WorkspaceActivityItem = {
  id: string;
  kind: WorkspaceActivityKind;
  label: string;
  summary: string;
  status: "recorded" | "waiting_decision" | "blocked" | "failed" | "completed" | "unknown";
  evidenceRefs: EvidenceRef[];
  occurredAt?: string;
};

export type WorkspaceViewModel = {
  activeTask?: TaskViewModelItem;
  recentTaskRefs: BackendEntityRef[];
  pendingReviewItems: ReviewItem[];
  activity: WorkspaceActivityItem[];
  providerPrivacyBoundarySummary: ProviderPrivacyBoundarySummary;
  activityRedactionState: string;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

export type MemoryTierSummary = {
  total: number;
  tier1: number;
  tier2: number;
  tier3: number;
  archived: number;
};

export type MemoryLifecycleSummary = {
  candidateCount: number;
  pendingReviewCount: number;
  editedPendingReviewCount: number;
  acceptedCount: number;
  confirmedCount: number;
  pendingMaterializationCount: number;
  materializedCount: number;
  materializationFailedCount: number;
  rejectedCount: number;
  deferredCount: number;
  supersededCount: number;
  rolledBackCount: number;
  expiredCount: number;
  archivedCount: number;
  byStatus: Record<string, number>;
  byMaterializationStatus: Record<string, number>;
};

export type MemoryLaneSummary = {
  lane: MemoryLane;
  label: string;
  totalCount: number;
  activeCount: number;
  candidateCount: number;
  pendingReviewCount: number;
  confirmedCount: number;
  materializedCount: number;
  rolledBackCount: number;
  archivedCount: number;
  reviewItemRefs: BackendEntityRef[];
  evidenceRefs: EvidenceRef[];
};

export type MemoryLifeModelLinkageSummary = {
  linkedMemoryCount: number;
  candidateMemoryCount: number;
  materializedMemoryCount: number;
  conflictCount: number;
  boundaryMemoryCount: number;
  linkageStatus: "partial" | "unknown";
  memoryRefs: BackendEntityRef[];
  evidenceRefs: EvidenceRef[];
};

export type MemoryViewModelSummary = {
  totalLifecycleRecords: number;
  activeMemoryCount: number;
  reviewRequiredCount: number;
  materializedCount: number;
  pendingMaterializationCount: number;
  failedMaterializationCount: number;
  rolledBackCount: number;
  archivedVectorCount: number;
  conflictCount: number;
  tierSummary?: MemoryTierSummary;
};

export type MemoryItemView = {
  memoryId: string;
  content?: string;
  scope: string;
  category: string;
  status: string;
  materializationStatus: string;
  recallState: "active" | "paused" | "archived" | "historical" | "erased" | "unavailable";
  sensitivity: string;
  whyRemembered: string;
  recallExplanation: string;
  acceptedAt?: string;
  evidenceIds: string[];
  sourceRefs: EvidenceRef[];
  supersedesMemoryId?: string;
  replacementMemoryId?: string;
  privacyErased: boolean;
  canCorrect: boolean;
  canStopRecall: boolean;
  canArchive: boolean;
  canRestore: boolean;
  canRollback: boolean;
  canPrivacyErase: boolean;
};

export type MemoryViewModel = {
  summary: MemoryViewModelSummary;
  lifecycleSummary: MemoryLifecycleSummary;
  laneSummaries: MemoryLaneSummary[];
  recentMemoryRefs: BackendEntityRef[];
  reviewItemRefs: BackendEntityRef[];
  lifeModelLinkage: MemoryLifeModelLinkageSummary;
  items: MemoryItemView[];
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

export async function getRuntimeBuildInfo(): Promise<RuntimeBuildInfo> {
  return safeInvoke<RuntimeBuildInfo>("get_runtime_build_info");
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
  manifests: ToolManifest[],
  env?: Record<string, string>
): Promise<void> {
  return safeInvoke("register_mcp_server", { name, command, args, env, manifests });
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
  manifests?: ToolManifest[];
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
  idempotency_contract: ToolIdempotencyContract;
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

export async function generateEvolutionReport(): Promise<FeedbackEvolutionReportResult> {
  return safeInvoke<FeedbackEvolutionReportResult>("generate_evolution_report");
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
  embeddingProfileId?: string | null;
  embeddingProfileRoute?: string | null;
  embeddingDimension?: number | null;
  providerInvocations?: number;
  cacheHits?: number;
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

export async function createKnowledgeNote(
  sessionId: string,
  content: string,
  source: string,
  operationId: string
): Promise<{
  operationId: string;
  replayed: boolean;
  embeddingId?: number;
  embeddingProfile?: {
    id: string;
    route: "unknown" | "cloud" | "ollama" | "deterministic_hash";
    provider: string;
    model: string;
    deploymentIdentity: string;
    modelArtifactIdentity: string;
    dimension: number;
  };
  embeddingReceipt?: {
    requestId: string;
    route: "unknown" | "cloud" | "ollama" | "deterministic_hash";
    profileId: string;
    status: "not_attempted" | "completed" | "failed";
    source: string;
    routeReasonCode: string;
    cacheHit: boolean;
    errorDigest?: string | null;
    providerDispatches: Array<{
      kind: "model_manifest" | "embedding";
      startedAt: string;
    }>;
  };
  knowledgeNoteId: number;
  outboxEventId: string;
  canonicalCommitted: boolean;
  projectionState: "pending" | "degraded" | "applied" | "superseded" | "compensated";
  projectionErrorDigest?: string;
}> {
  return safeInvoke("create_knowledge_note", {
    ...sessionArgs(sessionId),
    content,
    source,
    operationId,
    operation_id: operationId,
  });
}

export async function searchMemory(
  query: string,
  topK: number
): Promise<{
  hits: Array<{
    chunk: { id: number; session_id: string; content: string; source: string; created_at: string };
    score: number;
  }>;
  embeddingProfile: {
    id: string;
    route: "unknown" | "cloud" | "ollama" | "deterministic_hash";
    provider: string;
    model: string;
    deploymentIdentity: string;
    modelArtifactIdentity: string;
    dimension: number;
  };
  embeddingReceipt: {
    requestId: string;
    route: "unknown" | "cloud" | "ollama" | "deterministic_hash";
    profileId: string;
    status: "not_attempted" | "completed" | "failed";
    source: string;
    routeReasonCode: string;
    cacheHit: boolean;
    errorDigest?: string | null;
    providerDispatches: Array<{
      kind: "model_manifest" | "embedding";
      startedAt: string;
    }>;
  };
  vectorStatus: "ready" | "rebuild_required" | "embedding_failed" | "vector_search_failed";
  routeQuality:
    | "semantic_model_verified"
    | "deterministic_hash_approximation"
    | "identity_unknown"
    | "unavailable";
  rebuild?: {
    expectedProfileId: string;
    expectedDimension: number;
    incompatibleProfiles: string[];
    unknownProfileCount: number;
    profileMismatchCount: number;
    dimensionMismatchCount: number;
    corruptEmbeddingCount: number;
  };
  degradedEvidence?: {
    reasonCode: string;
    errorDigest?: string | null;
  };
}> {
  const raw: {
    hits: Array<[any, number]>;
    embeddingProfile: any;
    embeddingReceipt: any;
    vectorStatus: "ready" | "rebuild_required" | "embedding_failed" | "vector_search_failed";
    routeQuality:
      | "semantic_model_verified"
      | "deterministic_hash_approximation"
      | "identity_unknown"
      | "unavailable";
    rebuild?: any;
    degradedEvidence?: any;
  } = await safeInvoke("search_memory", {
    query,
    topK,
    top_k: topK,
  });
  return {
    ...raw,
    hits: raw.hits.map(([chunk, score]) => ({ chunk, score })),
  };
}

export async function a2aDiscoverAgent(url: string): Promise<any> {
  return safeInvoke("a2a_discover_agent", { url });
}

export async function a2aSendTask(
  url: string,
  requestJson: string,
  pairingToken?: string
): Promise<string> {
  return safeInvoke<string>("a2a_send_task", {
    url,
    requestJson,
    request_json: requestJson,
    ...optionalDualArg("pairingToken", "pairing_token", pairingToken),
  });
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
  dimension: "Identity" | "Goals" | "Capabilities" | "State";
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

export interface BuilderPendingSignalsView {
  session_id: string;
  signals: BuilderSignal[];
  summary: BuilderSummary;
  finished: boolean;
}

export interface BuilderTurnResponse {
  prompt: string;
  finished: boolean;
  progress: BuilderProgress;
  analysis?: BuilderAnalysis;
  review?: BuilderPendingSignalsView | null;
  waiting_for_review?: boolean;
  durable_lifemodel_write?: false;
  mode?: string;
  target_dimension?: string;
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
): Promise<BuilderTurnResponse> {
  return safeInvoke("builder_start", {
    mode,
    ...sessionArgs(sessionId),
    ...optionalDualArg("targetDimension", "target_dimension", targetDimension),
  });
}

export async function builderStep(
  sessionId: string,
  userReply: string
): Promise<BuilderTurnResponse> {
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
  current_prompt: string;
  pending_signal_count: number;
  waiting_for_review: boolean;
  review_in_progress: boolean;
  target_dimension?: "Identity" | "Goals" | "Capabilities" | "State";
  retention_status?: "active" | "expired_recoverable" | null;
  expires_at?: string | null;
  purge_after?: string | null;
}

export async function builderListUnfinished(): Promise<UnfinishedBuilderSession[]> {
  return safeInvoke("builder_list_unfinished");
}

export async function builderDeleteSession(sessionId: string): Promise<void> {
  return safeInvoke("builder_delete_session", sessionArgs(sessionId));
}
export async function builderGetPendingSignals(
  sessionId: string
): Promise<BuilderPendingSignalsView> {
  return safeInvoke("builder_get_pending_signals", sessionArgs(sessionId));
}

export interface BuilderSignalDecision {
  id: string;
  status: "accepted" | "rejected" | "edited";
  proposed_value?: unknown;
}

export async function builderCreateProposals(
  sessionId: string,
  decisions: BuilderSignalDecision[]
): Promise<{
  success: boolean;
  created_count: number;
  reused_count: number;
  updated_count: number;
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

export interface PortableDailyTaskV1 {
  title: string;
  status: "pending" | "completed";
  dueAt?: string | null;
  timeBlock?: { start: string; end: string } | null;
  createdAt: string;
  updatedAt: string;
  expiresAt: string;
}

export interface PortableDailyTaskArchiveV1 {
  schema: "openlife.state-store-daily-tasks-portable.v1";
  exportedAt: string;
  canonicalDigest: string;
  payloadDigest: string;
  skippedExpiredCount: number;
  dailyTasks: PortableDailyTaskV1[];
}

interface ExportPayloadBase {
  app_version?: string;
  exported_at: string;
  life_model: LifeModel;
  messages: ExportedMessage[];
  vectors: ExportedVectorChunk[];
}

export interface ExportPayloadV1 extends ExportPayloadBase {
  version: "1.0";
}

export interface ExportPayloadV2 extends ExportPayloadBase {
  version: "2.0";
  state_store: PortableDailyTaskArchiveV1;
}

export type ExportPayload = ExportPayloadV1 | ExportPayloadV2;

export const MAX_OPENLIFE_IMPORT_FILE_BYTES = 64 * 1024 * 1024;
export const MAX_OPENLIFE_IMPORT_MESSAGES = 50_000;
export const MAX_OPENLIFE_IMPORT_VECTORS = 50_000;
export const MAX_OPENLIFE_IMPORT_STATE_TASKS = 512;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null | undefined {
  return value === undefined || value === null || typeof value === "string";
}

function isPortableDailyTask(value: unknown): value is PortableDailyTaskV1 {
  return (
    isRecord(value) &&
    typeof value.title === "string" &&
    (value.status === "pending" || value.status === "completed") &&
    isNullableString(value.dueAt) &&
    (value.timeBlock === undefined ||
      value.timeBlock === null ||
      (isRecord(value.timeBlock) &&
        typeof value.timeBlock.start === "string" &&
        typeof value.timeBlock.end === "string")) &&
    typeof value.createdAt === "string" &&
    typeof value.updatedAt === "string" &&
    typeof value.expiresAt === "string"
  );
}

export function parseOpenLifeExportPayload(text: string): ExportPayload {
  if (text.length > MAX_OPENLIFE_IMPORT_FILE_BYTES) {
    throw new Error("OpenLife 备份超过 64 MiB 导入上限");
  }
  const value: unknown = JSON.parse(text);
  if (
    !isRecord(value) ||
    (value.version !== "1.0" && value.version !== "2.0") ||
    typeof value.exported_at !== "string" ||
    !isRecord(value.life_model) ||
    !Array.isArray(value.messages) ||
    !Array.isArray(value.vectors)
  ) {
    throw new Error("OpenLife 备份格式无效或版本不受支持");
  }
  if (value.messages.length > MAX_OPENLIFE_IMPORT_MESSAGES) {
    throw new Error("OpenLife 备份超过消息条目导入上限");
  }
  if (value.vectors.length > MAX_OPENLIFE_IMPORT_VECTORS) {
    throw new Error("OpenLife 备份超过向量条目导入上限");
  }
  if (value.version === "2.0") {
    const stateStore = value.state_store;
    if (
      !isRecord(stateStore) ||
      stateStore.schema !== "openlife.state-store-daily-tasks-portable.v1" ||
      typeof stateStore.exportedAt !== "string" ||
      typeof stateStore.canonicalDigest !== "string" ||
      typeof stateStore.payloadDigest !== "string" ||
      typeof stateStore.skippedExpiredCount !== "number" ||
      !Array.isArray(stateStore.dailyTasks) ||
      stateStore.dailyTasks.length > MAX_OPENLIFE_IMPORT_STATE_TASKS ||
      !stateStore.dailyTasks.every(isPortableDailyTask)
    ) {
      throw new Error("OpenLife v2 备份缺少有效的 StateStore portable archive");
    }
  } else if ("state_store" in value) {
    throw new Error("OpenLife v1 备份不得伪装携带 v2 StateStore archive");
  }
  return value as unknown as ExportPayload;
}

export type DangerActionType =
  | "data_export"
  | "data_import_overwrite"
  | "data_import_abandon_recovery"
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
  /** Metadata-only pointer to an interrupted governed import, when one exists. */
  recoveryOperationId?: string | null;
  /** Metadata-only durable journal stage for the interrupted import. */
  recoveryStage?: string | null;
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
  status?:
    | "completed"
    | "replayed"
    | "recovery_completed_restart_required"
    | "projection_degraded_recovery_required"
    | string;
  legacy: boolean;
  warning: string;
  metadata_safe: boolean;
  durable_lifemodel_write: boolean;
  imported_message_count: number;
  /** Null on a historical replay when execution-only vector counts were not durably recorded. */
  imported_vector_count: number | null;
  state_store_targeted?: boolean;
  state_store_replayed?: boolean;
  state_store_restored_count?: number;
  state_store_skipped_expired_count?: number;
  state_store_projection_status?: "not_requested" | "pending" | "applied" | "degraded" | string;
}

export function describeDataImportResult(result: DataImportResult): string {
  const projectionRecoveryRequired =
    result.status === "projection_degraded_recovery_required" ||
    // Older backend builds used this status. Keep it fail-closed while rolling forward.
    result.status === "completed_with_projection_degraded" ||
    result.state_store_projection_status === "degraded";

  if (projectionRecoveryRequired) {
    return "导入未完成：canonical 数据已经写入，但兼容投影处于降级恢复状态；请勿将投影页面视为最新事实，并按诊断提示恢复。";
  }
  if (result.status === "recovery_completed_restart_required") {
    return "中断的导入已恢复完成；当前进程仍处于恢复隔离状态，请重启 OpenLife 后再继续使用数据功能。";
  }
  if (result.state_store_replayed || result.status === "replayed") {
    return "同一导入操作已安全重放，没有重复写入。";
  }
  if (result.success && result.status === "completed") {
    return "导入成功，请刷新页面以查看最新数据";
  }

  const status = result.status?.trim() || "unknown";
  return `导入未完成：后端返回 ${status} 状态，请保留原备份并检查数据诊断。`;
}

export async function importAllData(
  payload: ExportPayload,
  confirmationEvidence?: DangerActionConfirmationEvidence,
  operationId: string = crypto.randomUUID()
): Promise<DataImportResult> {
  const importRequest = manualDataImportRequest(operationId);
  return safeInvoke<DataImportResult>("import_all_data", {
    payload,
    importRequest,
    import_request: importRequest,
    ...(confirmationEvidence
      ? { confirmationEvidence, confirmation_evidence: confirmationEvidence }
      : {}),
  });
}

export interface GovernedDataImportAbandonmentResult {
  success: true;
  status: "abandoned_preserving_current_restart_required" | "abandoned_preserving_current";
  operation_id: string;
  stage: "abandoned_preserving_current";
  recovery_terminalized: true;
  original_import_completed: false;
  rollback_completed: false;
  preserved_current_canonical_data: boolean;
  abandonment_mutated_canonical_owners: false;
  original_import_effect_state: "preserved_current_observed_per_owner";
  owner_resolution_counts: {
    before: number;
    target: number;
    other: number;
  };
  resolution_evidence_count: number;
  restart_required: boolean;
}

export interface GovernedDataImportStatusView {
  status: string;
  operationId: string | null;
  stage: string | null;
  terminal: boolean;
  terminalAt: string | null;
  recoveryRequired: boolean;
  runtimeRecoveryIsolationActive: boolean;
  restartRequired: boolean;
  originalImportCompleted: boolean;
  rollbackCompleted: boolean;
  preservedCurrent: boolean;
  ownerCount: number;
  resolutionEvidenceCount: number;
  ownerResolutionCounts: {
    before: number;
    target: number;
    other: number;
  };
  observedAt: string;
}

export async function getGovernedDataImportStatus(): Promise<GovernedDataImportStatusView> {
  return safeInvoke<GovernedDataImportStatusView>("get_governed_data_import_status");
}

export async function abandonGovernedDataImportRecovery(
  operationId: string,
  confirmationEvidence: DangerActionConfirmationEvidence
): Promise<GovernedDataImportAbandonmentResult> {
  return safeInvoke<GovernedDataImportAbandonmentResult>("abandon_governed_data_import_recovery", {
    operationId,
    operation_id: operationId,
    confirmationEvidence,
    confirmation_evidence: confirmationEvidence,
  });
}

export interface LlmConnectionTestResult {
  ok: boolean;
  provider: string;
  message: string;
  validation_status: string;
  network_policy_decision_id?: string;
  effective_network_policy_decision_id?: string;
  consent_status?: string;
  review_proposal_id?: string;
  permission_id?: string;
  provider_invocation_receipt?: ProviderInvocationReceipt;
}

export interface ProviderInvocationReceipt {
  request_id: string;
  provider: string;
  model: string;
  status: ProviderInvocationStatus;
  started_at: string;
  finished_at: string;
  error_digest?: string;
  simulated: boolean;
  policy_evidence?: Record<string, unknown>;
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

// ── Canonical Memory retrieval / access-tier telemetry ──
export interface CanonicalMemoryOwner {
  ownerKind: string;
  ownerId: string;
}

export interface LowAccessCanonicalMemoryCandidate {
  owner: CanonicalMemoryOwner;
  tier: number;
  accessCount: number;
  lastAccessedAt?: string;
  importanceScore: number;
  candidateOnly: boolean;
}

export interface ArchivedCanonicalMemoryView {
  owner: CanonicalMemoryOwner;
  revision: number;
  lastEventId: string;
  changedAt: string;
  canonicalDisposition: string;
}

export interface MemoryRetrievalMutationResult {
  owner: CanonicalMemoryOwner;
  disposition: string;
  changed: boolean;
  canonicalCommitted: boolean;
  revision?: number;
  outboxEventId?: string;
  projectionState: "applied" | "pending" | "degraded" | "superseded" | "compensated";
  projectionErrorDigest?: string;
}

export interface TierStats {
  total: number;
  tier1: number;
  tier2: number;
  tier3: number;
  archived: number;
}

export async function getLowAccessMemoryCandidates(): Promise<LowAccessCanonicalMemoryCandidate[]> {
  return safeInvoke<LowAccessCanonicalMemoryCandidate[]>("archive_low_access_memories");
}

export async function restoreArchivedMemory(
  owner: CanonicalMemoryOwner
): Promise<MemoryRetrievalMutationResult> {
  return safeInvoke<MemoryRetrievalMutationResult>("restore_archived_chunks", { owner });
}

export async function listArchivedChunks(limit: number): Promise<ArchivedCanonicalMemoryView[]> {
  return safeInvoke<ArchivedCanonicalMemoryView[]>("list_archived_chunks", { limit });
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
export interface ProductModelRouteTrace {
  provider: string;
  model: string;
  routeType: string;
  reason?: string;
  privacyLevel: string;
  retryCount: number;
  fallbackReason?: string;
  providerHealthIsEstimated?: boolean;
}

export interface ProductContextSummary {
  lifeModelEmpty: boolean;
  memoryHitCount: number;
  usedToolsPrompt: boolean;
  redactionApplied: boolean;
  redactionLevel: string;
}

export interface ProductToolActionScope {
  toolName: string;
  source: string;
  riskLevel: string;
  capabilities: string[];
}

/** Frontend-only compatibility fields; ProductModelRouteTrace is the IPC contract. */
export interface ModelRouteTrace extends ProductModelRouteTrace {
  preferLocal?: boolean;
  localModel?: string;
  latencyMs?: number;
}

/** Frontend-only compatibility fields; ProductContextSummary is the IPC contract. */
export interface ContextSummary extends ProductContextSummary {
  includedLifeModelSections?: string[];
  memorySources?: string[];
}

/** Frontend-only compatibility fields; ProductToolActionScope is the IPC contract. */
export interface ToolActionScope extends ProductToolActionScope {
  toolId?: string;
  actionType?: string;
  requiresConfirmation?: boolean;
  allowed?: boolean;
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

export interface ProductAgentAction {
  id: string;
  actionType: string;
  target?: string;
  status: string;
  permissionDecision?: string;
  startedAt?: string;
  finishedAt?: string;
  error?: string;
  timestamp: string;
  toolScope?: ProductToolActionScope;
  reactTrace?: ProductReactActionTrace;
}

export interface ProductAgentObservation {
  id: string;
  actionId?: string;
  content: string;
  source: string;
  timestamp: string;
  reactTrace?: ProductReactActionTrace;
}

/** Frontend-only compatibility shape for historical AgentRun views. */
export interface AgentAction extends Omit<ProductAgentAction, "toolScope" | "reactTrace"> {
  input?: unknown;
  output?: unknown;
  toolScope?: ToolActionScope;
  reactTrace?: ReactActionTraceEnvelope;
}

/** Frontend-only compatibility shape for historical AgentRun views. */
export interface AgentObservation extends Omit<ProductAgentObservation, "reactTrace"> {
  structuredResult?: unknown;
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

export interface ProductAgentRunError {
  message: string;
  phase: string;
  recoverable: boolean;
}

export interface ProductHSSelectionAudit {
  selectedPolicyIds: string[];
  selectedHeuristicIds: string[];
  estimatedTokens: number;
  tokenBudget: number;
}

export interface ProductHSBehaviorCheckSummary {
  id: string;
  label: string;
  passed: boolean;
  summary?: string;
}

export interface ProductAgentStatusUpdate {
  phase: string;
  message: string;
  stepIndex: number;
  toolCallIndex?: number;
  timestamp: string;
}

export interface ProductAgentRun {
  id: string;
  taskId: string;
  sessionId?: string;
  status:
    | "running"
    | "waiting_permission"
    | "completed"
    | "failed"
    | "remote_unknown"
    | "cancelled";
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
  contextSummary?: ProductContextSummary;
  modelRoute?: ProductModelRouteTrace;
  outputPreview?: string;
  error?: ProductAgentRunError;
  generatedProposals: string[];
  actions: ProductAgentAction[];
  observations: ProductAgentObservation[];
  reasoningStrategy?: string;
  legacyPayloadUnverified: boolean;
  hsSelectionAudit?: ProductHSSelectionAudit;
  behaviorChecks: ProductHSBehaviorCheckSummary[];
  statusUpdates: ProductAgentStatusUpdate[];
  stepCount: number;
  toolCallCount: number;
  warnings: string[];
  deletedAt?: string;
  deleteReason?: string;
  startedAt: string;
  finishedAt?: string;
}

/**
 * Frontend-only compatibility view. Product AgentRun IPC commands return the
 * exact ProductAgentRun contract; the adapter below does not synthesize any
 * removed body, trace, route, or tool fields.
 */
export interface AgentRunView extends Omit<
  ProductAgentRun,
  "contextSummary" | "modelRoute" | "actions" | "observations"
> {
  userInput?: string;
  inputRef?: string;
  inputDigest?: string;
  contextSummary?: ContextSummary;
  modelRoute?: ModelRouteTrace;
  actions: AgentAction[];
  observations: AgentObservation[];
  reasoningTrace?: ReasoningTrace;
  reasoningTraceDigest?: string;
}

/** @deprecated Prefer ProductAgentRun at IPC boundaries or AgentRunView in UI code. */
export type AgentRun = AgentRunView;

export function productAgentRunToView(run: ProductAgentRun): AgentRunView {
  return run;
}

export async function getAgentRun(runId: string): Promise<AgentRunView | null> {
  const run = await safeInvoke<ProductAgentRun | null>("get_agent_run", { runId });
  return run ? productAgentRunToView(run) : null;
}

export async function listAgentRuns(
  limit: number = 50,
  offset: number = 0
): Promise<AgentRunView[]> {
  const runs = await safeInvoke<ProductAgentRun[]>("list_agent_runs", { limit, offset });
  return runs.map(productAgentRunToView);
}

export async function listProviderTransmissionHistory(
  limit: number = 20
): Promise<ProviderTransmissionHistoryItem[]> {
  return safeInvoke<ProviderTransmissionHistoryItem[]>("list_provider_transmission_history", {
    limit,
  });
}

export async function listRuns(limit: number = 50, offset: number = 0): Promise<AgentRunView[]> {
  return listAgentRuns(limit, offset);
}

export async function listAgentRunsForSession(
  sessionId: string,
  limit: number = 50
): Promise<AgentRunView[]> {
  const runs = await safeInvoke<ProductAgentRun[]>("list_agent_runs_for_session", {
    sessionId,
    limit,
  });
  return runs.map(productAgentRunToView);
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

export async function getReviewCenterViewModel(): Promise<
  ViewModelEnvelope<ReviewCenterViewModel>
> {
  return safeInvoke<ViewModelEnvelope<ReviewCenterViewModel>>("get_review_center_view_model");
}

export async function getLifeModelViewModel(): Promise<ViewModelEnvelope<LifeModelViewModel>> {
  return safeInvoke<ViewModelEnvelope<LifeModelViewModel>>("get_life_model_view_model");
}

export async function getMemoryViewModel(): Promise<ViewModelEnvelope<MemoryViewModel>> {
  return safeInvoke<ViewModelEnvelope<MemoryViewModel>>("get_memory_view_model");
}

export async function getProviderPrivacyBoundarySummary(): Promise<
  ViewModelEnvelope<ProviderPrivacyBoundarySummary>
> {
  return safeInvoke<ViewModelEnvelope<ProviderPrivacyBoundarySummary>>(
    "get_provider_privacy_boundary_summary"
  );
}

export async function getTasksViewModel(): Promise<ViewModelEnvelope<TasksViewModel>> {
  return safeInvoke<ViewModelEnvelope<TasksViewModel>>("get_tasks_view_model");
}

export async function getWorkspaceViewModel(): Promise<ViewModelEnvelope<WorkspaceViewModel>> {
  return safeInvoke<ViewModelEnvelope<WorkspaceViewModel>>("get_workspace_view_model");
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
  sensitivity: string;
  auditDigest: string;
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
  canonicalMutation: {
    eventId: string;
    aggregateKind: string;
    aggregateId: string;
    mutationKind: string;
    aggregateRevision: number;
    payloadDigest: string;
    tombstoneId?: string | null;
    createdAt: string;
  };
  canonicalCommitted: boolean;
  projectionState: "pending" | "degraded" | "applied" | "superseded" | "compensated";
  projectionErrorDigest?: string;
}

export interface MemoryPrivacyEraseReport {
  memoryId: string;
  erasedAt: string;
  materializedView: MemoryMaterializedView;
  canonicalMutation: {
    eventId: string;
    aggregateKind: string;
    aggregateId: string;
    mutationKind: string;
    aggregateRevision: number;
    payloadDigest: string;
    tombstoneId?: string | null;
    createdAt: string;
  };
  canonicalCommitted: boolean;
  projectionState: "pending" | "degraded" | "applied" | "superseded" | "compensated";
  projectionErrorDigest?: string;
}

export interface MemoryActionProposalReceipt {
  proposalId: string;
  memoryId: string;
  action: "correct" | "stop_recall" | "archive";
  status: "review_required";
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

export interface ArtifactMaterializationReceipt {
  artifactId: string;
  proposalId: string;
  targetReference: string;
  targetReferenceDigest: string;
  contentDigest: string;
  observedContentDigest: string;
  byteSize: number;
  mediaType: string;
  status: "confirmed";
}

export interface ConfirmedAcceptProposalResult {
  success: true;
  patchResult?: PatchApplyResult;
  effectStatus: "confirmed";
  proposalProjectionStatus: "confirmed" | "reconciliation_required";
  proposalId?: string;
  terminalOwnerTransition?: unknown;
  warnings: string[];
  mainChatTaskSync?: unknown[];
  memoryGateway?: unknown;
  memoryLifecycle?: MemoryLifecycleRecord;
  memoryPersistence?: {
    canonicalCommitted: boolean;
    outboxEventId?: string;
    projectionState: "pending" | "degraded" | "applied" | "superseded" | "compensated";
    pending?: number;
    degraded?: number;
    applied?: number;
    reasonCode?: string;
    errorDigest?: string;
  };
  artifactMaterialization?: ArtifactMaterializationReceipt;
  blockedAction?: unknown;
  canContinue?: boolean;
}

export interface DeferredAcceptProposalResult {
  success: false;
  status: "deferred";
  reasonCode: string;
  proposalId: string;
  dispatchState: string;
  durableWriteExecuted: false;
  warnings: string[];
}

export type AcceptProposalResult = ConfirmedAcceptProposalResult | DeferredAcceptProposalResult;

export async function acceptProposal(proposalId: string): Promise<AcceptProposalResult> {
  return safeInvoke("accept_proposal", { proposalId, proposal_id: proposalId });
}

export async function rollbackMemoryAsset(
  memoryId: string,
  reason: string
): Promise<MemoryRollbackReport> {
  return safeInvoke("rollback_memory_asset", { memoryId, memory_id: memoryId, reason });
}

export async function draftMemoryCorrectionProposal(
  memoryId: string,
  content: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_correction_proposal", {
    memoryId,
    memory_id: memoryId,
    content,
  });
}

export async function draftMemoryArchiveProposal(
  memoryId: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_archive_proposal", { memoryId, memory_id: memoryId });
}

export async function draftMemoryStopRecallProposal(
  memoryId: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_stop_recall_proposal", { memoryId, memory_id: memoryId });
}

export async function privacyEraseMemoryAsset(memoryId: string): Promise<MemoryPrivacyEraseReport> {
  return safeInvoke("privacy_erase_memory_asset", { memoryId, memory_id: memoryId });
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
