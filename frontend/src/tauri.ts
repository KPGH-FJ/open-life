import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage } from "./types";

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

function safeInvoke<T>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (!isTauriEnv()) {
    return Promise.reject(
      new Error("当前不在 OpenLife 桌面应用环境中，无法调用原生功能。请在桌面窗口内操作。")
    );
  }
  if (import.meta.env.DEV && import.meta.env.MODE !== "test") {
    console.log("[safeInvoke]", cmd, redactInvokeArgs(cmd, args));
  }
  return invoke<T>(cmd, args);
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

function sessionArgs(sessionId: string): { sessionId: string } {
  return { sessionId };
}

function selectedSkillArgs(selectedSkillId?: string): { selectedSkillId: string } | undefined {
  const trimmed = selectedSkillId?.trim();
  return trimmed ? { selectedSkillId: trimmed } : undefined;
}

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
  prefer_local_model: boolean;
  local_model: string;
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
    | "canonical_task_receipts"
    | "task_store"
    | "mcp_audit"
    | "provider_api_key"
    | "search_provider_api_key";
  status:
    | CredentialBootstrapStatus
    | "created"
    | "pending_restart_verification"
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

export interface MainChatMessageOptions {
  operationId: string;
  selectedSkillId?: string;
  mode?: "chat" | "work";
  taskId?: string;
  runId?: string;
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

export interface ProductToolActionTrace {
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
 * Shipped commands return ProductToolActionTrace and do not promise these
 * legacy optional fields.
 */
export interface ToolActionTraceEnvelope extends ProductToolActionTrace {
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

export interface ProductAgentTrace {
  generation_result?: MainChatGenerationResult;
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

export interface SendMessageResult {
  reply: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  reasoning_trace: ProductAgentTrace;
  tool_calls: ToolCallResult[];
  run_id?: string;
  agent_ingress?: MainChatAgentIngressDecision;
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
  life_model_influence?: MainChatLifeModelProductReceipt;
}

export interface MainChatLifeModelSelectedItemReceipt {
  itemRef: string;
  statement: string;
  sourceRefs: string[];
  confirmedAt: string;
  reasonCode: string;
}

export interface MainChatLifeModelProductReceipt {
  status: string;
  sourceId?: string | null;
  modelVersion?: number | null;
  versionDigest?: string | null;
  documentDigest?: string | null;
  selectedItems: MainChatLifeModelSelectedItemReceipt[];
  appliedSurfaces: string[];
  currentInstructionPriorityPreserved: boolean;
  policyPriorityPreserved: boolean;
  permissionGranted: boolean;
  durableWriteAuthorized: boolean;
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

export interface StreamMessageStartPayload {
  session_id: string;
  operation_id: string;
  conversation_id?: string;
  turn_id?: string;
  task_id?: string;
  run_id?: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  reasoning_trace?: ProductAgentTrace;
  tool_calls?: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
}

export interface StreamMessageChunkPayload {
  session_id: string;
  operation_id: string;
  conversation_id?: string;
  turn_id?: string;
  task_id?: string;
  run_id?: string;
  request_id?: string;
  chunk: string;
}

export interface StreamMessageDonePayload {
  session_id: string;
  operation_id: string;
  conversation_id?: string;
  turn_id?: string;
  task_id?: string;
  run_id?: string;
  reply: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  provider_invocation_status?: ProviderInvocationStatus;
  model_invoked?: boolean;
  tool_invoked?: boolean;
  life_model_influence?: MainChatLifeModelProductReceipt;
  reasoning_trace?: ProductAgentTrace;
  tool_calls?: ToolCallResult[];
  agent_ingress?: MainChatAgentIngressDecision;
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
      type: "life_model_context_loaded";
      available: boolean;
      model_version?: number | null;
      selected_item_count: number;
      status: string;
      source_id?: string | null;
      selected_item_refs: string[];
      reason_codes: string[];
      receipt: MainChatLifeModelProductReceipt;
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

export type MainChatDisposition =
  | "direct_answer"
  | "read_only_tool"
  | "plan_draft"
  | "transient_state_command"
  | "reversible_memory_commit"
  | "memory_proposal"
  | "life_model_proposal"
  | "file_write_proposal"
  | "action_proposal"
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
  disposition: MainChatDisposition;
  confidence: number;
  reasonSummary: string;
  fallbackEligible: boolean;
  privacyRisk: MainChatPrivacyRiskSummary;
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
  taskId?: string | null;
  candidates: MainChatToolCandidate[];
  blockedTools: MainChatBlockedTool[];
  failureRecovery?: MainChatToolFailureRecovery | null;
  evidenceDigest: string;
  controls: string[];
}

export async function listMainChatSkills(sessionId?: string): Promise<MainChatSkillSummary[]> {
  return safeInvoke<MainChatSkillSummary[]>("list_main_chat_skills", {
    ...(sessionId === undefined ? {} : { sessionId }),
  });
}

export async function selectMainChatSkill(
  sessionId: string,
  skillId: string
): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("select_main_chat_skill", {
    sessionId,
    skillId,
  });
}

export async function clearMainChatSkill(sessionId: string): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("clear_main_chat_skill", {
    sessionId,
  });
}

export async function listMainChatToolCandidates(
  taskId?: string
): Promise<MainChatToolCandidateList> {
  return safeInvoke<MainChatToolCandidateList>("list_main_chat_tool_candidates", {
    taskId,
  });
}

export async function openExternalHttpsSource(url: string): Promise<void> {
  return safeInvoke("open_external_https_source", { url });
}

export interface CancelChatTurnResult {
  conversationId: string;
  turnId: string;
  status: "running" | "completed" | "failed" | "cancelled" | "interrupted";
  activeTurnFound: boolean;
}

export async function cancelChatTurn(
  conversationId: string,
  turnId: string
): Promise<CancelChatTurnResult> {
  return safeInvoke<CancelChatTurnResult>("cancel_chat_turn", {
    conversationId,
    turnId,
  });
}

export interface CanonicalWorkControlResult {
  taskId: string;
  runId: string;
  turnId: string;
  status:
    | "running"
    | "waiting_review"
    | "completed"
    | "blocked"
    | "failed"
    | "cancelled"
    | "interrupted"
    | "effect_unknown";
}

export async function cancelWorkTask(taskId: string): Promise<CanonicalWorkControlResult> {
  return safeInvoke<CanonicalWorkControlResult>("cancel_work_task", { taskId });
}

export async function retryWorkTask(
  taskId: string,
  priorRunId: string
): Promise<SendMessageResult> {
  const newRunId = crypto.randomUUID();
  const newTurnId = crypto.randomUUID();
  return safeInvoke<SendMessageResult>("retry_work_task", {
    taskId,
    priorRunId,
    newRunId,
    newTurnId,
  });
}

export type MainChatSteeringStatus = "pending" | "consumed" | "blocked";

export interface MainChatSteeringRecord {
  steeringId: string;
  itemId: string;
  taskId: string;
  runId: string;
  sourceMessageRef: string;
  sourceMessageDigest: string;
  steeringDigest: string;
  basePlanRevision: number;
  status: MainChatSteeringStatus;
  createdAt: string;
  consumedAt?: string;
}

export interface SubmitMainChatSteeringResponse {
  steering: MainChatSteeringRecord;
  scopeExpansionBlocked: boolean;
}

export async function submitMainChatTaskSteering(request: {
  steeringId: string;
  taskId: string;
  runId: string;
  sessionId: string;
  content: string;
}): Promise<SubmitMainChatSteeringResponse> {
  return safeInvoke<SubmitMainChatSteeringResponse>("submit_main_chat_task_steering", request);
}

export async function startStreamMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions
): Promise<StreamMessageDonePayload> {
  const payload = {
    operationId: options.operationId,
    ...sessionArgs(sessionId),
    messages,
    mode: options.mode ?? "chat",
    taskId: options.taskId,
    runId: options.runId,
    ...selectedSkillArgs(options.selectedSkillId),
  };
  return safeInvoke<StreamMessageDonePayload>("start_stream_message", {
    args: payload,
  });
}

export async function pickAndImportResources(
  importOperationId: string,
  turnOperationId: string
): Promise<ResourceImportSelectionResult> {
  return safeInvoke<ResourceImportSelectionResult>("pick_and_import_resources", {
    importOperationId,
    turnOperationId,
  });
}

export async function detachResourceFromTurn(
  operationId: string,
  turnOperationId: string,
  resourceId: string
): Promise<ResourceDetachReceipt> {
  return safeInvoke<ResourceDetachReceipt>("detach_resource_from_turn", {
    operationId,
    turnOperationId,
    resourceId,
  });
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
  devExtensionsEnabled: boolean;
  arbitraryMcpRegistrationEnabled: boolean;
  bundleIdentifier: string;
  productName: string;
}

export interface ProductStoreDiagnostic {
  store: string;
  status: string;
  reasonCode?: string | null;
}

export interface ProductContentCounts {
  projectCount?: number | null;
  conversationCount?: number | null;
  taskCount?: number | null;
  activeTaskCount?: number | null;
  waitingTaskCount?: number | null;
  completedTaskCount?: number | null;
  failedTaskCount?: number | null;
  unresolvedAttentionCount?: number | null;
}

export interface ProductDiagnosticsViewModel {
  generatedAt: string;
  status: "ready" | "degraded" | "blocked" | string;
  appVersion: string;
  runtimeBuild: RuntimeBuildInfo;
  persistenceMode: string;
  canonicalWritesAllowed: boolean;
  providerDispatchAllowed: boolean;
  toolDispatchAllowed: boolean;
  stores: ProductStoreDiagnostic[];
  counts: ProductContentCounts;
  credentialBootstrap: CredentialBootstrapSnapshot;
  blockerCodes: string[];
}

export async function getProductDiagnosticsViewModel(): Promise<ProductDiagnosticsViewModel> {
  return safeInvoke<ProductDiagnosticsViewModel>("get_product_diagnostics_view_model");
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
  task_id?: string | null;
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
      | "canonical_task_receipts"
      | "task_store"
      | "mcp_audit"
      | "provider_api_key"
      | "search_provider_api_key";
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
  lifeModelLearning?: {
    candidateId: string;
    candidateSnapshotDigest: string;
    section: string;
    proposedStatement: string;
    explicitness: string;
    stability: string;
    sensitivity: string;
    conflictStatus: string;
    supportCount: number;
    independentSupportCount: number;
    confirmedAt: string;
    sourceRefs: string[];
    sourceKinds: string[];
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
  | "candidate"
  | "pending_review"
  | "manual_override"
  | "unknown"
  | "unavailable";

export type LifeModelOwnerStatus = "PARTIAL" | "PHASE_2_REQUIRED" | "UNKNOWN";

export type LifeModelReviewItemRef = BackendEntityRef & {
  kind: "review_item";
};

export type LifeModelCanonicalSummary = {
  lifeModelRef: BackendEntityRef;
  title: string;
  summary: string;
  versionLabel: string;
  parentVersion: number | null;
  documentDigest: string;
  lastMaterializedAt: string | null;
  freshnessStatus: string;
  conflictStatus: string;
  evidenceRefs: EvidenceRef[];
  document: LifeModelDocumentV2;
  humanProjection: LifeModelHumanProjectionV2;
};

export type LifeModelStatementV2 = {
  id: string;
  statement: string;
  sourceRefs: string[];
  confirmedAt: string;
};

export type LifeModelLongTermGoalV2 = {
  id: string;
  direction: string;
  meaning: string;
  sourceRefs: string[];
  confirmedAt: string;
};

export type LifeModelRelationshipV2 = {
  id: string;
  personLabel: string;
  relationship: string;
  significance: string;
  sourceRefs: string[];
  confirmedAt: string;
};

export type LifeModelNamedItemV2 = {
  id: string;
  name: string;
  description: string;
  sourceRefs: string[];
  confirmedAt: string;
};

export type LifeModelDocumentV2 = {
  schemaVersion: "openlife.lifemodel.v2";
  modelId: string;
  identity: LifeModelStatementV2[];
  values: LifeModelStatementV2[];
  longTermGoals: LifeModelLongTermGoalV2[];
  stablePreferences: LifeModelStatementV2[];
  personalBoundaries: LifeModelStatementV2[];
  importantRelationships: LifeModelRelationshipV2[];
  capabilities: LifeModelNamedItemV2[];
  resources: LifeModelNamedItemV2[];
  decisionPrinciples: LifeModelStatementV2[];
  collaborationPreferences: LifeModelStatementV2[];
};

export type LifeModelVersionHistoryEntryV2 = {
  modelId: string;
  modelVersion: number;
  parentVersion: number | null;
  documentDigest: string;
  itemCount: number;
  summary: string;
  sourceRefs: string[];
  createdAt: string;
  changeSummary: { added: number; replaced: number; removed: number };
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
    | "tool_capability"
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
  candidates: LegacyLifeModelMigrationCandidateV2[];
};

export type LifeModelSectionV2 =
  | "identity"
  | "values"
  | "long_term_goals"
  | "stable_preferences"
  | "personal_boundaries"
  | "important_relationships"
  | "capabilities"
  | "resources"
  | "decision_principles"
  | "collaboration_preferences";

export type LegacyLifeModelMigrationCandidateValueV2 =
  | { kind: "statement"; value: { statement: string } }
  | { kind: "long_term_goal"; value: { direction: string; meaning: string } }
  | {
      kind: "relationship";
      value: { person_label: string; relationship: string; significance: string };
    }
  | { kind: "capability"; value: { name: string; description: string } }
  | { kind: "resource"; value: { name: string; description: string } };

export type LifeModelUserValueV2 = LegacyLifeModelMigrationCandidateValueV2;

export type LifeModelV2UserChange =
  | { operation: "add"; section: LifeModelSectionV2; value: LifeModelUserValueV2 }
  | {
      operation: "replace";
      section: LifeModelSectionV2;
      item_id: string;
      value: LifeModelUserValueV2;
    }
  | { operation: "remove"; section: LifeModelSectionV2; item_id: string }
  | { operation: "clear" };

export type DraftLifeModelV2ChangeRequest = {
  baseVersion: number | null;
  baseDocumentDigest: string | null;
  change: LifeModelV2UserChange;
};

export type DraftLifeModelV2RollbackRequest = {
  baseVersion: number;
  baseDocumentDigest: string;
  targetVersion: number;
  targetDocumentDigest: string;
};

export type DraftLifeModelV2ExportRequest = {
  modelVersion: number;
  documentDigest: string;
  projectionDigest: string | null;
  format: "yaml" | "json";
  targetPath: string;
};

export type LifeModelV2ProposalReceipt = {
  proposalId: string;
  status: "review_required";
  baseVersion: number | null;
  baseDocumentDigest: string | null;
  resultDocumentDigest: string | null;
  operationCount: number;
};

export type LegacyLifeModelMigrationCandidateV2 = {
  candidateId: string;
  itemId: string;
  sourcePaths: string[];
  targetSection: LifeModelSectionV2;
  proposedValue: LegacyLifeModelMigrationCandidateValueV2;
  sensitive: boolean;
};

export type LegacyLifeModelMigrationSelectionV2 = {
  candidateId: string;
  decision: "include" | "exclude";
  editedValue: LegacyLifeModelMigrationCandidateValueV2 | null;
};

export type DraftLegacyLifeModelMigrationRequest = {
  sourceDigest: string;
  selections: LegacyLifeModelMigrationSelectionV2[];
  nonLifemodelItemsAcknowledged: boolean;
};

export type DraftLegacyLifeModelMigrationReceipt = {
  proposalId: string;
  status: "review_required";
  sourceDigest: string;
  includedCount: number;
  excludedCount: number;
  nonLifemodelItemCount: number;
};

export type LifeModelTrustQualityState = {
  readiness: "not_built" | "limited" | "usable_with_limits" | "ready" | "stale" | "unknown";
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

export type LifeModelLearningCandidate = {
  id: string;
  workspaceRef: string;
  summary: string;
  section: LifeModelSectionV2;
  value: LifeModelUserValueV2;
  targetKey: string;
  suggestionClass: string;
  supportCount: number;
  oppositionCount: number;
  independentSupportCount: number;
  status:
    | "accumulating"
    | "reviewable"
    | "conflicted"
    | "proposed"
    | "rejected"
    | "materialized"
    | "expired";
  explicitness: "explicit_user_request" | "passive_inference";
  sensitivity: "internal";
  observationIds: string[];
  sourceRefs: string[];
  sourceKinds: Array<
    | "explicit_user_message"
    | "task_outcome"
    | "agent_reflection"
    | "user_feedback"
    | "user_correction"
    | "model_extraction"
  >;
  confirmedAt?: string;
  proposalId?: string;
  decidedAt?: string;
  materializedVersion?: number;
  materializedDocumentDigest?: string;
  createdAt: string;
  updatedAt: string;
  expiresAt: string;
};

export type LifeModelLearningSummary = {
  available: boolean;
  activeCount: number;
  candidates: LifeModelLearningCandidate[];
};

export type DeleteLifeModelLearningCandidateReceipt = {
  candidateId: string;
  deleted: boolean;
  proposalDeleted: false;
  canonicalLifeModelChanged: false;
};

export type ConfirmLifeModelLearningCandidateReceipt = {
  candidateId: string;
  status: "reviewable";
  sourceKind: "user_feedback";
  proposalCreated: false;
  canonicalLifeModelChanged: false;
};

export type StageLifeModelLearningCandidateReceipt = {
  candidateId: string;
  proposalId: string;
  status: "review_required";
  baseVersion: number | null;
  baseDocumentDigest: string | null;
  resultDocumentDigest: string;
  canonicalLifeModelChanged: false;
};

export type LifeModelLearningDecisionReceipt = {
  candidateId: string;
  changed: boolean;
  status: "rejected" | "expired";
  suppressionKind: "exact_candidate" | "suggestion_class" | null;
  contentScrubbed: boolean;
  proposalChanged: false;
  canonicalLifeModelChanged: false;
};

export type LifeModelLearningReviewDecisionReceipt = {
  candidateId: string;
  proposalId: string;
  changed: boolean;
  status: "proposed" | "rejected" | "materialized";
  contentScrubbed: boolean;
  correctionObservationId?: string;
  cooldownUntil?: string;
  materializedVersion?: number;
  materializedDocumentDigest?: string;
  canonicalLifeModelChanged: boolean;
};

export type LifeModelViewModel = {
  truthMode: LifeModelTruthMode;
  canonicalSummary: LifeModelCanonicalSummary | null;
  versionHistory: LifeModelVersionHistoryEntryV2[];
  legacyMigrationPreview: LegacyLifeModelMigrationPreviewV2 | null;
  trustQualityState: LifeModelTrustQualityState;
  pendingUpdateCounts: LifeModelPendingUpdateCounts;
  provenanceRefs: EvidenceRef[];
  candidateChanges: LifeModelCandidateChange[];
  materializedChanges: LifeModelMaterializedChange[];
  manualOverrideState: LifeModelManualOverrideState | null;
  relatedReviewItemRefs: LifeModelReviewItemRef[];
  memoryLinkage: LifeModelMemoryLinkageSummary;
  learning: LifeModelLearningSummary;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

// Backend-owned task and workspace read-model contracts.
// Canonical Rust owner: openlife-core/src/agent/tasks_view_model.rs.
export type TaskLifecycleStatus =
  | "running"
  | "waiting_review"
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

export type CanonicalTaskItemKind =
  | "instruction"
  | "plan"
  | "steering"
  | "tool_call"
  | "observation"
  | "provider_generation"
  | "artifact_draft"
  | "review_checkpoint"
  | "artifact_materialized"
  | "verification"
  | "final_result";

export type CanonicalTaskItemStatus =
  | "waiting"
  | "running"
  | "completed"
  | "blocked"
  | "failed"
  | "cancelled"
  | "interrupted"
  | "effect_unknown";

export type CanonicalArtifactStatus =
  | "draft"
  | "waiting_review"
  | "materialized"
  | "failed"
  | "effect_unknown";

export type TaskItemViewModel = {
  id: string;
  runId: string;
  sequence: number;
  kind: CanonicalTaskItemKind;
  status: CanonicalTaskItemStatus;
  summaryCode: string;
  evidenceRefs: EvidenceRef[];
};

export type TaskArtifactViewModel = {
  artifactId: string;
  version: number;
  status: CanonicalArtifactStatus;
  mediaType: string;
  contentDigest: string;
  targetReferenceDigest: string;
  materializedReference?: string;
  observedContentDigest?: string;
  proposalRef?: BackendEntityRef;
  sourceItemRef: BackendEntityRef;
  evidenceRefs: EvidenceRef[];
  change: {
    kind: "create" | "replace" | "unknown";
    status: CanonicalArtifactStatus;
    targetReference?: string;
    expectedPriorDigest?: string;
  };
  preview: {
    status: "available" | "truncated" | "unavailable";
    content?: string;
    reasonCode?: string;
  };
  verification: {
    status: "pending" | "verified" | "failed" | "unknown";
    expectedContentDigest: string;
    observedContentDigest?: string;
    verificationItemPresent: boolean;
    reasonCode?: string;
  };
  undo: {
    available: boolean;
    status?: string;
    proposalRef?: BackendEntityRef;
    reasonCode?: string;
  };
};

export type TaskViewModelItem = {
  canonicalTaskId: string;
  relatedRunIds: string[];
  conversationId?: string;
  title: string;
  lifecycleStatus: TaskLifecycleStatus;
  terminalDeliveryStatus: TaskTerminalDeliveryStatus;
  finalDeliveryEvidencePresent: boolean;
  items: TaskItemViewModel[];
  workPlan?: {
    revision: number;
    steps: Array<{
      id: string;
      kind:
        | "analyze"
        | "read_imported_document"
        | "read_workspace_file"
        | "web_search"
        | "web_fetch"
        | "use_selected_skill"
        | "read_mcp"
        | "draft_artifact"
        | "verify"
        | "deliver_result";
      required: boolean;
      dependsOn: string[];
    }>;
    completion: {
      resultKind: "answer" | "artifact";
      requiresVerification: boolean;
    };
    budgetPolicy: {
      maxPlanAttempts: number;
      maxProviderAttempts: number;
      maxToolAttempts: number;
      maxTotalItems: number;
    };
  };
  artifacts: TaskArtifactViewModel[];
  pendingBlockers: string[];
  needsAttention?: boolean;
  attentionReasonCodes?: string[];
  pendingReviewItemRefs: BackendEntityRef[];
  allowedControls: TaskControl[];
  nextRecommendedControl: string;
  latestResultPreview?: TaskLatestResultPreview;
  evidenceRefs: EvidenceRef[];
  updatedAt?: string;
};

export type TasksViewModelSummary = {
  total: number;
  needsAttentionCount: number;
  activeCount: number;
  waitingReviewCount: number;
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
  selectedConversationId?: string;
  tasks: TaskViewModelItem[];
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

export interface ConversationTurnViewModel {
  turnId: string;
  status: "running" | "completed" | "failed" | "cancelled" | "interrupted";
  providerProfileId: string;
  providerId: string;
  modelId: string;
  endpointClass: string;
  errorCode?: string;
}

export interface ProviderProfileViewModel {
  profileId: string;
  providerId: string;
  modelId: string;
  endpointClass: string;
  selected: boolean;
}

export interface ProjectRecord {
  id: string;
  name: string;
  workspaceRoot?: string;
  revision: number;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationViewModel {
  status: "ready" | "empty";
  conversations: ChatSession[];
  projects: ProjectRecord[];
  selectedProjectId: string | null;
  selectedConversationId: string | null;
  messages: ChatMessage[];
  latestTurn: ConversationTurnViewModel | null;
  providerStatus: "ready" | "unavailable";
  providerProfiles: ProviderProfileViewModel[];
  selectedProviderProfileId: string | null;
  providerErrorCode: string | null;
  workStatus: "available" | "unavailable";
}

export interface WorkbenchViewModel {
  capturedAt: string;
  conversation: ViewModelEnvelope<ConversationViewModel>;
  workspace: ViewModelEnvelope<WorkspaceViewModel>;
  tasks: ViewModelEnvelope<TasksViewModel>;
  review: ViewModelEnvelope<ReviewCenterViewModel>;
  providerBoundary: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
}

export async function getConversationViewModel(
  conversationId?: string
): Promise<ConversationViewModel> {
  return safeInvoke<ConversationViewModel>("get_conversation_view_model", {
    conversationId,
  });
}

export async function getWorkbenchViewModel(
  conversationId?: string | null
): Promise<WorkbenchViewModel> {
  return safeInvoke<WorkbenchViewModel>("get_workbench_view_model", {
    ...(conversationId == null || conversationId === "" ? {} : { conversationId }),
  });
}

export async function createChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("create_chat_session", { ...sessionArgs(sessionId), title });
}

export async function createProject(projectId: string, name: string): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("create_project", {
    projectId,
    name,
  });
}

export async function assignConversationProject(
  conversationId: string,
  projectId: string | null
): Promise<void> {
  return safeInvoke("assign_conversation_project", {
    conversationId,
    projectId,
  });
}

export async function renameChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("rename_chat_session", { ...sessionArgs(sessionId), title });
}

export async function deleteChatSession(sessionId: string): Promise<void> {
  return safeInvoke("delete_chat_session", sessionArgs(sessionId));
}

// ── Canonical Memory retrieval / access-tier telemetry ──
export interface CanonicalMemoryOwner {
  ownerKind: string;
  ownerId: string;
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

export async function restoreArchivedMemory(
  owner: CanonicalMemoryOwner
): Promise<MemoryRetrievalMutationResult> {
  return safeInvoke<MemoryRetrievalMutationResult>("restore_archived_chunks", { owner });
}

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

export async function getReviewCenterViewModel(): Promise<
  ViewModelEnvelope<ReviewCenterViewModel>
> {
  return safeInvoke<ViewModelEnvelope<ReviewCenterViewModel>>("get_review_center_view_model");
}

export async function getLifeModelViewModel(): Promise<ViewModelEnvelope<LifeModelViewModel>> {
  return safeInvoke<ViewModelEnvelope<LifeModelViewModel>>("get_life_model_view_model");
}

export async function deleteLifeModelLearningCandidate(
  candidateId: string
): Promise<DeleteLifeModelLearningCandidateReceipt> {
  return safeInvoke<DeleteLifeModelLearningCandidateReceipt>(
    "delete_lifemodel_learning_candidate",
    { candidateId }
  );
}

export async function confirmLifeModelLearningCandidate(
  candidateId: string
): Promise<ConfirmLifeModelLearningCandidateReceipt> {
  return safeInvoke<ConfirmLifeModelLearningCandidateReceipt>(
    "confirm_lifemodel_learning_candidate",
    { candidateId }
  );
}

export async function stageLifeModelLearningCandidate(
  candidateId: string
): Promise<StageLifeModelLearningCandidateReceipt> {
  return safeInvoke<StageLifeModelLearningCandidateReceipt>("stage_lifemodel_learning_candidate", {
    candidateId,
  });
}

export async function editLifeModelLearningProposal(
  proposalId: string,
  statement: string
): Promise<{
  proposalId: string;
  status: "edited_pending_review";
  resultDocumentDigest: string;
  durableWriteExecuted: false;
  learning: LifeModelLearningReviewDecisionReceipt;
}> {
  return safeInvoke("edit_lifemodel_learning_proposal", {
    request: { proposalId, statement },
  });
}

export async function rejectLifeModelLearningCandidate(
  candidateId: string
): Promise<LifeModelLearningDecisionReceipt> {
  return safeInvoke<LifeModelLearningDecisionReceipt>("reject_lifemodel_learning_candidate", {
    candidateId,
  });
}

export async function pauseLifeModelLearningSuggestionClass(
  candidateId: string
): Promise<LifeModelLearningDecisionReceipt> {
  return safeInvoke<LifeModelLearningDecisionReceipt>("pause_lifemodel_learning_suggestion_class", {
    candidateId,
  });
}

export async function draftLegacyLifeModelMigration(
  request: DraftLegacyLifeModelMigrationRequest
): Promise<DraftLegacyLifeModelMigrationReceipt> {
  return safeInvoke<DraftLegacyLifeModelMigrationReceipt>("draft_legacy_lifemodel_migration", {
    request,
  });
}

export async function draftLifeModelV2Change(
  request: DraftLifeModelV2ChangeRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke<LifeModelV2ProposalReceipt>("draft_lifemodel_v2_change", { request });
}

export async function draftLifeModelV2Rollback(
  request: DraftLifeModelV2RollbackRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke<LifeModelV2ProposalReceipt>("draft_lifemodel_v2_rollback", { request });
}

export async function draftLifeModelV2Export(
  request: DraftLifeModelV2ExportRequest
): Promise<LifeModelV2ProposalReceipt> {
  return safeInvoke<LifeModelV2ProposalReceipt>("draft_lifemodel_v2_export", { request });
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

export interface MemoryLifecycleRecord {
  memoryId: string;
  proposalId: string;
  sourceTaskId?: string;
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
  canonicalTaskRuntimeProjectionStatus?: "confirmed" | "reconciliation_required" | "not_applicable";
  proposalId?: string;
  warnings: string[];
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
  lifeModelLearning?:
    | LifeModelLearningReviewDecisionReceipt
    | {
        proposalId: string;
        status: "reconciliation_required";
        canonicalLifeModelChanged: true;
      };
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
  return safeInvoke("accept_proposal", { proposalId });
}

export async function rollbackMemoryAsset(
  memoryId: string,
  reason: string
): Promise<MemoryRollbackReport> {
  return safeInvoke("rollback_memory_asset", { memoryId, reason });
}

export async function draftMemoryCorrectionProposal(
  memoryId: string,
  content: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_correction_proposal", {
    memoryId,
    content,
  });
}

export async function draftMemoryArchiveProposal(
  memoryId: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_archive_proposal", { memoryId });
}

export async function draftMemoryStopRecallProposal(
  memoryId: string
): Promise<MemoryActionProposalReceipt> {
  return safeInvoke("draft_memory_stop_recall_proposal", { memoryId });
}

export async function privacyEraseMemoryAsset(memoryId: string): Promise<MemoryPrivacyEraseReport> {
  return safeInvoke("privacy_erase_memory_asset", { memoryId });
}

export async function rejectProposal(proposalId: string): Promise<void> {
  return safeInvoke("reject_proposal", { proposalId });
}

export async function requestArtifactUndo(artifactId: string): Promise<{
  artifactId: string;
  proposalId: string;
  status: "waiting_review";
}> {
  return safeInvoke("request_artifact_undo", {
    artifactId,
  });
}

export async function postponeProposal(proposalId: string): Promise<void> {
  return safeInvoke("postpone_proposal", { proposalId });
}
