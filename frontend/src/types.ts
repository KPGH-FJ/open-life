export interface Metadata {
  version: string;
  created_at: string;
  updated_at: string;
  author: string;
}

export interface ValueItem {
  name: string;
  weight: number;
  description: string;
}

export interface PersonalityTrait {
  trait_name: string;
  score: number;
}

export interface RoleDefinition {
  primary_role: string;
  secondary_roles: string[];
  responsibilities: string[];
  boundaries: string[];
}

export type FormalityLevel = "casual" | "neutral" | "formal";
export type EmojiUsage = "never" | "sparingly" | "often";

export interface VoiceStyle {
  formality: FormalityLevel;
  tone_descriptors: string[];
  vocabulary_preference: string;
  emoji_usage: EmojiUsage;
}

export interface Identity {
  name: string;
  birth_date?: string;
  values: ValueItem[];
  personality_traits: PersonalityTrait[];
  life_philosophy: string;
  mission_statement: string;
  role_definition: RoleDefinition;
  voice_style: VoiceStyle;
}

export interface Milestone {
  name: string;
  target_date?: string;
  status: string;
  description: string;
}

export interface GoalItem {
  name: string;
  priority: number;
  status: string;
  milestones: Milestone[];
  description: string;
  progress: number;
  related_memories: string[];
}

export interface DailyGoal {
  name: string;
  done: boolean;
  time_block?: {
    start: string;
    end: string;
  };
}

export interface Goals {
  short_term: GoalItem[];
  medium_term: GoalItem[];
  long_term: GoalItem[];
  life_goals: GoalItem[];
  daily: DailyGoal[];
  progress: number;
  related_memories: string[];
}

export interface Skill {
  name: string;
  proficiency: number;
  description: string;
}

export interface Resource {
  name: string;
  resource_type: string;
  description: string;
  availability: string;
}

export interface ToolCapability {
  name: string;
  proficiency: number;
  description: string;
}

export interface KnowledgeDomain {
  domain: string;
  level: number;
  description: string;
}

export interface Capabilities {
  skills: Skill[];
  resources: Resource[];
  networks: string[];
  tools: ToolCapability[];
  knowledge_domains: KnowledgeDomain[];
}

export interface HealthStatus {
  physical: string;
  mental: string;
  energy_level: number;
}

export interface EmotionalState {
  current_mood: string;
  stress_level: number;
  fulfillment_score: number;
}

export interface Reflection {
  date: string;
  content: string;
  insights: string[];
}

export interface HabitStreak {
  name: string;
  streak_days: number;
}

export interface CustomStateDimension {
  name: string;
  unit: string;
  current_value: number;
  min_threshold?: number;
  max_threshold?: number;
  alert_days: number;
}

export type AlertLevel = "info" | "warning" | "critical";

export interface StateAlert {
  dimension_name: string;
  level: AlertLevel;
  message: string;
  triggered_at: string;
}

export interface State {
  current_focus: string;
  health_status: HealthStatus;
  emotional_state: EmotionalState;
  recent_reflections: Reflection[];
  open_questions: string[];
  focus_areas: string[];
  recent_events: string[];
  habit_streaks: HabitStreak[];
  custom_dimensions: CustomStateDimension[];
  alerts: StateAlert[];
}

export interface StateHistoryEntry {
  id: number;
  dimension_name: string;
  value: number;
  unit: string;
  recorded_at: string;
  note?: string;
}

export interface Relationship {
  name: string;
  relationship_type: string;
  importance: number;
  notes: string;
}

export interface Relationships {
  inner_circle: Relationship[];
  mentors: Relationship[];
  collaborators: Relationship[];
}

export interface WorkHours {
  preferred_start: string;
  preferred_end: string;
  timezone: string;
}

export interface Preferences {
  work_hours: WorkHours;
  peak_energy_time: string;
  communication_style: string;
  learning_style: string;
  decision_making_style: string;
}

export interface LifeModel {
  metadata: Metadata;
  identity: Identity;
  goals: Goals;
  capabilities: Capabilities;
  state: State;
  relationships: Relationships;
  preferences: Preferences;
  evolution_rules: string[];
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

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
  run_id?: string;
}

export type MultiStrategyAgentPreviewLayer = "L1" | "L2" | "L3" | "l1" | "l2" | "l3";

export interface MultiStrategyAgentPreviewExecutionBudget {
  maxSteps?: number;
  maxToolCalls?: number;
  timeoutSeconds?: number;
  allowCloud?: boolean;
  allowWrites?: boolean;
}

export interface MultiStrategyAgentPreviewInput {
  sessionId: string;
  userText: string;
  toolsPrompt?: string;
  allowPlanning: boolean;
  localModelAvailable: boolean;
  layer?: MultiStrategyAgentPreviewLayer;
  executionBudget?: MultiStrategyAgentPreviewExecutionBudget;
}

export type MultiStrategyAgentPreviewStrategyKind = "react" | "planExecute";
export type MultiStrategyAgentPreviewPayloadKind = "react" | "planExecute" | "blocked";
export type MultiStrategyAgentPreviewGovernanceDecisionKind = "allow" | "warn" | "block";

export interface MultiStrategyAgentPreviewOutput {
  runId?: string;
  strategyKind: MultiStrategyAgentPreviewStrategyKind;
  payloadKind: MultiStrategyAgentPreviewPayloadKind;
  userOutput?: string;
  plan?: unknown;
  proposalIds: string[];
  warnings: string[];
  metadataSafeSummary: Record<string, unknown>;
  governanceDecisionKind?: MultiStrategyAgentPreviewGovernanceDecisionKind;
}

export interface RuntimeStrategySideEffectBudget {
  runtimeCalls: number;
  modelCalls: number;
  toolCalls: number;
  storeWrites: number;
  proposalWrites: number;
  memoryWrites: number;
  lifeModelWrites: number;
  mcpAuditWrites: number;
  externalWrites: number;
}

export interface RuntimeStrategyDescriptor {
  strategyKind: string;
  metadataSafeId: string;
  metadataSafeName: string;
  payloadKind: string;
  capabilityIds: string[];
  supportedTaskCategories: string[];
  writePolicy: string;
  sideEffectBudget: RuntimeStrategySideEffectBudget;
  proposalFirstRequired: boolean;
  metadataSafeTraceSupported: boolean;
  defaultChatMigrationPermission: boolean;
  metadataSafe: boolean;
  executable: boolean;
}

export interface RuntimeStrategyDeclarativeDescriptor {
  strategyKind: string;
  metadataSafeId: string;
  metadataSafeName: string;
  capabilityIds: string[];
  supportedTaskCategories: string[];
  writePolicy: string;
  sideEffectBudget: RuntimeStrategySideEffectBudget;
  declarativeOnly: boolean;
  executable: boolean;
  defaultChatMigrationPermission: boolean;
  metadataSafe: boolean;
}

export interface RuntimeStrategyRegistryReadinessReport {
  reportKind: "runtime_strategy_registry_readiness";
  ready: boolean;
  metadataSafe: boolean;
  executableStrategyCount: number;
  executableDescriptors: RuntimeStrategyDescriptor[];
  futureStrategyDescriptors: RuntimeStrategyDeclarativeDescriptor[];
  requiredStrategyKinds: string[];
  blockingReasons: string[];
  defaultChatUnchanged: boolean;
  migrationPermission: boolean;
  noRuntimeModelToolExecution: boolean;
  noBusinessWrites: boolean;
  metadataSafeSummary: Record<string, unknown>;
}

export interface MultiStrategyRuntimeMaturityReport {
  reportKind: "multi_strategy_runtime_maturity";
  maturityReady: boolean;
  registryReadiness: RuntimeStrategyRegistryReadinessReport;
  executableStrategies: RuntimeStrategyDescriptor[];
  futureStrategyDescriptors: RuntimeStrategyDeclarativeDescriptor[];
  defaultChatUnchanged: boolean;
  migrationPermission: boolean;
  noRuntimeModelToolExecution: boolean;
  noBusinessWrites: boolean;
  statusCommandSideEffectBudget: RuntimeStrategySideEffectBudget;
  blockingReasons: string[];
  metadataSafe: boolean;
  metadataSafeSummary: Record<string, unknown>;
}

export interface ToolRegistryBetaToolReport {
  toolId: string;
  requiredState: string;
  actualState: string;
  ready: boolean;
  executable: boolean;
  source?: string;
  riskLevel?: string;
  actionType?: string;
  capabilities: string[];
  proposalType?: string;
  blockingReasons: string[];
}

export interface ToolRegistryBetaReadinessReport {
  reportKind: "tool_registry_beta_readiness";
  ready: boolean;
  metadataSafe: boolean;
  requiredToolIds: string[];
  tools: ToolRegistryBetaToolReport[];
  executableReadTools: string[];
  proposalOnlyTools: string[];
  permissionGatedTools: string[];
  disabledOrDeclarativeOnlyTools: string[];
  unsupportedOrMissingTools: string[];
  unknownToolsBlocked: boolean;
  pluginToolsExecutableWithoutExecutor: string[];
  calendarEmailProposalToolsAvoidExternalWriteFallback: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface ReactBetaExecutionReadinessReport {
  reportKind: "react_beta_execution_readiness";
  ready: boolean;
  reactLoopPresent: boolean;
  actionSchemaReady: boolean;
  toolRegistryReady: boolean;
  actionExecutorManifestAuthorityReady: boolean;
  agentRunTraceReady: boolean;
  permissionReplayReady: boolean;
  proposalFirstWritesReady: boolean;
  runsTraceSurfaceReady: boolean;
  defaultChatUnchanged: boolean;
  migrationPermission: boolean;
  runtimeStrategyReady: boolean;
  blockingReasons: string[];
  metadataSafe: boolean;
  metadataSafeSummary: Record<string, unknown>;
}

export interface ReactBetaExecutionStatusReport {
  reportKind: "react_beta_execution_status";
  readiness: ReactBetaExecutionReadinessReport;
  toolRegistryReadiness: ToolRegistryBetaReadinessReport;
  defaultChatUnchanged: boolean;
  migrationPermission: boolean;
  noRuntimeModelToolExecution: boolean;
  noBusinessWrites: boolean;
  statusCommandSideEffectBudget: RuntimeStrategySideEffectBudget;
  metadataSafe: boolean;
  metadataSafeSummary: Record<string, unknown>;
}

export type PlanExecuteScenario = "weekly_planning";
export type PlanExecuteSessionStatus =
  | "draft"
  | "finalized"
  | "in_progress"
  | "completed"
  | "cancelled";
export type PlanExecuteStepStatus =
  | "planned"
  | "skipped"
  | "blocked"
  | "requires_proposal"
  | "requires_confirmation"
  | "executed"
  | "cancelled";
export type PlanExecuteRiskLevel = "low" | "medium" | "high" | "critical";

export interface PlanExecuteReviewItem {
  stepId: string;
  title: string;
  status: string;
  evidenceIds: string[];
  linkedActionIds: string[];
  linkedObservationIds: string[];
  linkedProposalIds: string[];
  blockerIds: string[];
}

export interface PlanExecuteReviewSummary {
  reviewId: string;
  planId: string;
  planSessionId: string;
  planStatus: string;
  basePlanRevision: number;
  reviewedAt?: string;
  completedSteps: PlanExecuteReviewItem[];
  skippedSteps: PlanExecuteReviewItem[];
  blockedSteps: PlanExecuteReviewItem[];
  proposalsCreated: PlanExecuteReviewItem[];
  observationsUsed: PlanExecuteReviewItem[];
  unresolved: PlanExecuteReviewItem[];
  recommendedNextAction: string[];
  completionClaimed: boolean;
  metadataSafeSummary?: Record<string, any>;
}

export interface PlanExecuteStepRecord {
  planId?: string;
  stepId: string;
  index?: number;
  order: number;
  title: string;
  description?: string;
  kind?: string;
  intent: string;
  toolName?: string | null;
  actionKind: string;
  riskLevel: PlanExecuteRiskLevel;
  declaredWrite: boolean;
  status: PlanExecuteStepStatus;
  revision?: number;
  basePlanRevision?: number;
  linkedProposalId?: string | null;
  linkedActionIds?: string[];
  linkedObservationIds?: string[];
  linkedProposalIds?: string[];
  blockerIds?: string[];
  linkedFinalDeliveryIds?: string[];
  skipReason?: string | null;
  observationSummary?: string | null;
  policyReasonCode?: string | null;
  policyDecisionId?: string | null;
  statusReason?: string | null;
  evidenceIds?: string[];
  metadataSafeSummary?: Record<string, any>;
}

export interface PlanExecuteSession {
  sessionId: string;
  planId?: string;
  sourceAgentRunId?: string | null;
  sourceChatSessionId?: string | null;
  scenario: PlanExecuteScenario;
  status: PlanExecuteSessionStatus;
  revision?: number;
  revisionId?: string;
  createdAt: string;
  updatedAt: string;
  finalizedAt?: string | null;
  confirmedAt?: string | null;
  reviewId?: string | null;
  reviewSummary?: PlanExecuteReviewSummary | null;
  sourceEvidenceIds?: string[];
  supersededByPlanId?: string | null;
  metadataSafeObjective: string;
  stepCount: number;
  completedStepCount: number;
  proposalRequiredStepCount: number;
  linkedProposalIds: string[];
  warnings: string[];
  steps: PlanExecuteStepRecord[];
  metadataSafeSummary?: Record<string, any>;
}

export interface CreatePlanExecuteSessionInput {
  scenarioId?: PlanExecuteScenario;
  sourceChatSessionId?: string;
  maxSteps?: number;
}

export interface PlanExecuteStepEditInput {
  stepId: string;
  title?: string;
  intent?: string;
  actionKind?: string;
  toolName?: string;
  declaredWrite?: boolean;
  riskLevel?: PlanExecuteRiskLevel;
}

export interface UpdatePlanExecuteSessionDraftInput {
  sessionId: string;
  baseRevision?: number;
  steps: PlanExecuteStepEditInput[];
}

export interface ExecutePlanExecuteStepInput {
  sessionId: string;
  stepId?: string;
  baseRevision?: number;
}

export interface PlanExecuteStepExecutionResult {
  sessionId: string;
  planId?: string;
  stepId: string;
  stepStatus: PlanExecuteStepStatus;
  revision?: number;
  basePlanRevision?: number;
  stepKind?: string;
  linkedProposalId?: string | null;
  linkedActionIds?: string[];
  linkedObservationIds?: string[];
  linkedProposalIds?: string[];
  blockerIds?: string[];
  linkedFinalDeliveryIds?: string[];
  skipReason?: string | null;
  observationSummary?: string | null;
  policyDecisionId?: string | null;
  statusReason?: string | null;
  evidenceIds?: string[];
  metadataSafeSummary?: Record<string, any>;
}

export interface ExecutePlanExecuteStepOutput {
  session: PlanExecuteSession;
  executedStep: PlanExecuteStepExecutionResult;
  metadataSafeSummary?: Record<string, any>;
}

export interface SkipPlanExecuteStepInput {
  sessionId: string;
  stepId: string;
  baseRevision: number;
  reason: string;
}

export interface SkipPlanExecuteStepOutput {
  session: PlanExecuteSession;
  skippedStep: PlanExecuteStepExecutionResult;
  metadataSafeSummary?: Record<string, any>;
}

export interface ReviewPlanExecuteSessionOutput {
  session: PlanExecuteSession;
  summary: PlanExecuteReviewSummary;
  metadataSafeSummary?: Record<string, any>;
}

export interface RuntimeMigrationGateCheckInput {
  previewRunId?: string;
  sessionId?: string;
}

export interface RuntimeMigrationGateReport {
  defaultChatUnchanged: boolean;
  previewPathHealthy: boolean;
  metadataSafeTraceReady: boolean;
  fallbackAvailable: boolean;
  noExternalWrites: boolean;
  proposalFirstPreserved: boolean;
  blockingReasons: string[];
}

export interface ControlledChatPilotEligibilityCheckInput {
  requiredCleanRuns?: number;
  sessionId?: string;
}

export interface ControlledChatPilotEligibilityReport {
  eligible: boolean;
  requiredCleanRuns: number;
  cleanRunCount: number;
  checkedRunIds: string[];
  blockingReasons: string[];
  lastGateReport?: RuntimeMigrationGateReport;
  defaultChatUnchanged: boolean;
}

export interface ControlledPilotPromotionEvidenceInput {
  pilotRunId: string;
  sourceSessionId: string;
  targetSessionId: string;
  strategyKind: MultiStrategyAgentPreviewStrategyKind;
  payloadKind: MultiStrategyAgentPreviewPayloadKind;
  governanceDecisionKind: MultiStrategyAgentPreviewGovernanceDecisionKind | "unknown";
  promotedMessageLength: number;
  promotedMessageHash: string;
  promotedAt: string;
}

export interface ControlledPilotPromotionEvidenceResult {
  evidenceId: string;
  created: boolean;
  pilotRunId: string;
  promotedAt: string;
}

export interface ControlledPilotPromotionEvidenceSummary {
  promotedCount: number;
  recentPromotedPilotRunIds: string[];
  latestPromotionTimestamp?: string | null;
  sourceTargetMismatchBlockCount: number;
}

export interface ControlledPilotPromotionReadinessCheckInput {
  requiredPromotions?: number;
  sessionId?: string;
}

export interface ControlledPilotPromotionReadinessReport {
  ready: boolean;
  requiredPromotions: number;
  promotedCount: number;
  recentPromotedPilotRunIds: string[];
  latestPromotionTimestamp?: string | null;
  sourceTargetMismatchBlockCount: number;
  metadataSafeEvidenceReady: boolean;
  defaultChatUnchanged: boolean;
  blockingReasons: string[];
}

export interface ControlledChatMigrationPlanDraftInput {
  requiredPromotions?: number;
  sessionId?: string;
}

export interface ControlledChatMigrationPlanDraft {
  draftReady: boolean;
  readinessReport: ControlledPilotPromotionReadinessReport;
  migrationScope: string[];
  requiredPreconditions: string[];
  rollbackPlan: string[];
  fallbackPlan: string[];
  testPlan: string[];
  manualReviewRequired: boolean;
  notAutomaticMigration: boolean;
  blockingReasons: string[];
}

export type ControlledChatMigrationReviewDecisionKind = "approve" | "reject" | "request_rework";

export interface ControlledChatMigrationReviewDecisionInput {
  decisionKind: ControlledChatMigrationReviewDecisionKind;
  requiredPromotions?: number;
  sessionId?: string;
  optionalReviewerNote?: string;
}

export interface ControlledChatMigrationReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  decisionKind: ControlledChatMigrationReviewDecisionKind;
  draftReady: boolean;
  draftHash: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface ControlledChatMigrationReviewLatestDecision {
  evidenceId: string;
  decisionKind: ControlledChatMigrationReviewDecisionKind;
  draftReady: boolean;
  draftHash: string;
  createdAt: string;
}

export interface ControlledChatMigrationReviewDecisionSummary {
  latestDecision?: ControlledChatMigrationReviewLatestDecision | null;
  approvedCount: number;
  reworkRejectCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
}

export interface ControlledChatMigrationImplementationGateInput {
  requiredPromotions?: number;
  sessionId?: string;
}

export interface ControlledChatMigrationImplementationGateReport {
  implementationEligible: boolean;
  latestDecision?: ControlledChatMigrationReviewLatestDecision | null;
  readinessReport: ControlledPilotPromotionReadinessReport;
  draftHashMatched: boolean;
  approvedAfterLatestDraft: boolean;
  blockingReasons: string[];
}

export type ControlledChatMigrationShadowRunDescriptor =
  | "default_readiness_probe"
  | "planning_readiness_probe"
  | "sensitive_local_only_probe";

export interface ControlledChatMigrationShadowRunInput {
  sessionId: string;
  userInputChecksum?: string;
  boundedTestPromptDescriptor?: ControlledChatMigrationShadowRunDescriptor;
  requiredPromotions?: number;
}

export interface ControlledChatMigrationShadowRunOutput {
  shadowRunReady: boolean;
  shadowRunId?: string | null;
  implementationGateReport: ControlledChatMigrationImplementationGateReport;
  strategyKind: string;
  payloadKind: string;
  metadataSafeSummary: Record<string, unknown>;
  warnings: string[];
  blockingReasons: string[];
}

export type ControlledChatMigrationShadowReviewDecisionKind =
  | "approve"
  | "reject"
  | "request_rework";

export interface ControlledChatMigrationShadowReviewDecisionInput {
  shadowRunId: string;
  decisionKind: ControlledChatMigrationShadowReviewDecisionKind;
  optionalReviewerNote?: string;
}

export interface ControlledChatMigrationShadowReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  shadowRunId: string;
  decisionKind: ControlledChatMigrationShadowReviewDecisionKind;
  readinessSummaryDigest: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface ControlledChatMigrationShadowReviewLatestDecision {
  evidenceId: string;
  shadowRunId: string;
  decisionKind: ControlledChatMigrationShadowReviewDecisionKind;
  reviewerNoteChecksum?: string | null;
  reviewerNoteLength: number;
  reviewerNoteCategory: string;
  readinessSummaryDigest: string;
  createdAt: string;
}

export interface ControlledChatMigrationShadowReviewSummary {
  latestDecision?: ControlledChatMigrationShadowReviewLatestDecision | null;
  approvedCount: number;
  reworkRejectCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
}

export interface ControlledChatCutoverReadinessInput {
  requiredPromotions?: number;
  sessionId?: string;
}

export interface ControlledChatCutoverReadinessReport {
  cutoverPlanningEligible: boolean;
  implementationGateReport: ControlledChatMigrationImplementationGateReport;
  latestShadowReviewDecision?: ControlledChatMigrationShadowReviewLatestDecision | null;
  verifiedShadowRunId?: string | null;
  readinessSummaryDigest?: string | null;
  defaultChatUnchanged: boolean;
  requiredEvidenceReady: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export type ControlledChatCutoverCandidateDescriptor =
  | "default_contract_probe"
  | "concise_response_probe";

export type ControlledChatCutoverCandidateContractShape =
  | "send_message_compatible"
  | "blocked"
  | "failed";

export interface ControlledChatCutoverCandidateInput {
  sessionId: string;
  userInputChecksum?: string;
  boundedTestPromptDescriptor?: ControlledChatCutoverCandidateDescriptor;
  requiredPromotions?: number;
}

export interface ControlledChatCutoverCandidateOutput {
  candidateReady: boolean;
  candidateRunId?: string | null;
  outputPreview?: string | null;
  userOutput?: string | null;
  contractShape: ControlledChatCutoverCandidateContractShape;
  metadataSafeSummary: Record<string, unknown>;
  warnings: string[];
  blockingReasons: string[];
}

export type ControlledChatCutoverCandidateReviewDecisionKind =
  | "approve"
  | "reject"
  | "request_rework";

export interface ControlledChatCutoverCandidateReviewDecisionInput {
  candidateRunId: string;
  decisionKind: ControlledChatCutoverCandidateReviewDecisionKind;
  optionalReviewerNote?: string;
}

export interface ControlledChatCutoverCandidateReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  candidateRunId: string;
  decisionKind: ControlledChatCutoverCandidateReviewDecisionKind;
  contractShape: string;
  candidateSummaryDigest: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface ControlledChatCutoverCandidateReviewLatestDecision {
  evidenceId: string;
  candidateRunId: string;
  decisionKind: ControlledChatCutoverCandidateReviewDecisionKind;
  contractShape: string;
  candidateSummaryDigest: string;
  reviewerNoteChecksum?: string | null;
  reviewerNoteLength: number;
  reviewerNoteCategory: string;
  createdAt: string;
}

export interface ControlledChatCutoverCandidateReviewSummary {
  latestDecision?: ControlledChatCutoverCandidateReviewLatestDecision | null;
  approvedCount: number;
  reworkRejectCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
}

export interface ControlledChatCutoverCandidatePromotionReadinessInput {
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
}

export interface ControlledChatCutoverCandidatePromotionApprovedCandidate {
  evidenceId: string;
  candidateRunId: string;
  contractShape: string;
  candidateSummaryDigest: string;
  runReadinessDigest: string;
  decisionCreatedAt: string;
  ready: boolean;
  blockingReasons: string[];
}

export interface ControlledChatCutoverCandidatePromotionReadinessReport {
  ready: boolean;
  cutoverReadinessEligible: boolean;
  requiredApprovedCandidates: number;
  approvedCandidateCount: number;
  latestDecision?: ControlledChatCutoverCandidateReviewLatestDecision | null;
  approvedCandidates: ControlledChatCutoverCandidatePromotionApprovedCandidate[];
  defaultChatUnchanged: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
  checkedAt: string;
}

export interface LifeModelVersion {
  version: string;
  timestamp: string;
  tag: string;
  note: string;
  yaml_content: string;
}
