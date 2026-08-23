import type { ChatMessage } from "./types";
export { redactInvokeArgs } from "./ipc/invoke";
export {
  assignConversationProject,
  cancelChatTurn,
  clearMainChatSkill,
  createChatSession,
  createProject,
  deleteChatSession,
  detachResourceFromTurn,
  getConversationViewModel,
  listMainChatSkills,
  listMainChatToolCandidates,
  pickAndImportResources,
  renameChatSession,
  selectMainChatSkill,
  setConversationMemoryMode,
  startStreamMessage,
  submitMainChatTaskSteering,
} from "./ipc/conversation";
export {
  cancelWorkTask,
  exportArtifactResult,
  getWorkbenchViewModel,
  openArtifactResult,
  openExternalHttpsSource,
  requestArtifactUndo,
  retryWorkTask,
} from "./ipc/work";
export {
  getConfig,
  getLifeStateProjection,
  getProductDiagnosticsViewModel,
  getProviderPrivacyBoundarySummary,
  recoverRequiredCredentialAccess,
  saveConfig,
  selectArtifactOutputDirectory,
  testLlmConnection,
} from "./ipc/settings";
export {
  acceptProposal,
  getReviewCenterViewModel,
  postponeProposal,
  rejectProposal,
} from "./ipc/review";
export {
  archiveMemory,
  confirmLifeModelLearningCandidate,
  correctMemory,
  deleteLifeModelLearningCandidate,
  draftLifeModelV2Change,
  draftLifeModelV2Export,
  draftLifeModelV2Rollback,
  editLifeModelLearningProposal,
  getLifeModelViewModel,
  getMemoryViewModel,
  pauseLifeModelLearningSuggestionClass,
  privacyEraseMemoryAsset,
  rejectLifeModelLearningCandidate,
  restoreMemory,
  rollbackMemoryAsset,
  stageLifeModelLearningCandidate,
} from "./ipc/personalIntelligence";

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
    agent_memory_enabled?: boolean;
    safe_paths?: string[];
    search_provider?: "auto" | "duckduckgo" | "brave" | "deepseek" | "searxng";
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

export interface ArtifactOutputDirectorySelection {
  cancelled: boolean;
  selectedPath: string | null;
}

export interface CredentialRecoveryItem {
  purpose: "canonical_task_receipts" | "mcp_audit" | "provider_api_key" | "search_provider_api_key";
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

export interface MainChatMessageOptions {
  operationId: string;
  selectedSkillId?: string;
  mode?: "chat" | "work";
  taskId?: string;
  runId?: string;
}

export interface SendMessageResult {
  reply: string;
  status?: MainChatTurnStatus;
  blockers?: string[];
  run_id?: string;
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

export type ExportArtifactResult = {
  cancelled: boolean;
  savedPath?: string;
  contentDigest?: string;
};

export interface CancelChatTurnResult {
  conversationId: string;
  turnId: string;
  status: "running" | "completed" | "failed" | "cancelled" | "interrupted";
  activeTurnFound: boolean;
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
  sourceRefs: string[];
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
  | "memory_write"
  | "memory_archive"
  | "tool_permission"
  | "external_write_action"
  | "life_model_update";

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

export type LifeModelUserValueV2 =
  | { kind: "statement"; value: { statement: string } }
  | { kind: "long_term_goal"; value: { direction: string; meaning: string } }
  | {
      kind: "relationship";
      value: { person_label: string; relationship: string; significance: string };
    }
  | { kind: "capability"; value: { name: string; description: string } }
  | { kind: "resource"; value: { name: string; description: string } };

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
  | "interrupted"
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
  completionDisposition?: "complete" | "complete_with_disclosed_limitations";
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
  activity: WorkspaceActivityItem[];
  activityRedactionState: string;
  sourceRefs: EvidenceRef[];
  contractLimitations: string[];
};

export type MemoryViewModelSummary = {
  totalMemoryCount: number;
  activeMemoryCount: number;
  archivedMemoryCount: number;
  historicalMemoryCount: number;
};

export type MemoryItemView = {
  memoryId: string;
  content?: string;
  scope: string;
  category: string;
  recallState: "active" | "paused" | "archived" | "historical" | "erased" | "unavailable";
  whyRemembered: string;
  recallExplanation: string;
  acceptedAt?: string;
  sourceRefs: EvidenceRef[];
  privacyErased: boolean;
  canCorrect: boolean;
  canArchive: boolean;
  canRestore: boolean;
  canRollback: boolean;
  canPrivacyErase: boolean;
};

export type MemoryViewModel = {
  summary: MemoryViewModelSummary;
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
  globalMemoryEnabled: boolean;
  selectedMemoryMode: ConversationMemoryMode;
  messages: ChatMessage[];
  latestTurn: ConversationTurnViewModel | null;
  providerStatus: "ready" | "unavailable";
  providerProfiles: ProviderProfileViewModel[];
  selectedProviderProfileId: string | null;
  providerErrorCode: string | null;
  workStatus: "available" | "unavailable";
}

export type ConversationMemoryMode = "use_and_learn" | "use_only" | "off";

export interface WorkbenchViewModel {
  capturedAt: string;
  workspace: ViewModelEnvelope<WorkspaceViewModel>;
  tasks: ViewModelEnvelope<TasksViewModel>;
  review: ViewModelEnvelope<ReviewCenterViewModel>;
  providerBoundary: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
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

export interface MemoryCorrectionResult {
  memoryId: string;
  replacedMemoryId: string;
  canonicalCommitted: boolean;
  projectionState: "pending" | "degraded" | "applied" | "superseded" | "compensated";
  projectionErrorDigest?: string;
  undoAvailable: boolean;
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
