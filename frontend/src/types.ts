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

export interface DefaultChatRuntimeBoundaryStatus {
  currentMode: "legacy_stream";
  controlledCandidateAvailable: boolean;
  defaultChatUnchanged: boolean;
  candidatePromotionReadinessRequired: boolean;
  automaticMigrationEnabled: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
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

export interface DefaultChatAdapterActivationPlanDraftInput {
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
}

export interface DefaultChatAdapterActivationPlanDraft {
  draftReady: boolean;
  candidatePromotionReadinessReport: ControlledChatCutoverCandidatePromotionReadinessReport;
  runtimeBoundaryStatus: DefaultChatRuntimeBoundaryStatus;
  activationScope: string[];
  requiredPreconditions: string[];
  adapterContractChecks: string[];
  fallbackPlan: string[];
  rollbackPlan: string[];
  observabilityPlan: string[];
  testPlan: string[];
  manualReviewRequired: boolean;
  notAutomaticMigration: boolean;
  requiresSeparateImplementation: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export type DefaultChatAdapterActivationReviewDecisionKind =
  | "approve"
  | "reject"
  | "request_rework";

export interface DefaultChatAdapterActivationReviewDecisionInput {
  decisionKind: DefaultChatAdapterActivationReviewDecisionKind;
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
  optionalReviewerNote?: string;
}

export interface DefaultChatAdapterActivationReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  decisionKind: DefaultChatAdapterActivationReviewDecisionKind;
  draftReady: boolean;
  activationPlanDigest: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface DefaultChatAdapterActivationReviewLatestDecision {
  evidenceId: string;
  decisionKind: DefaultChatAdapterActivationReviewDecisionKind;
  draftReady: boolean;
  activationPlanDigest: string;
  candidatePromotionReady: boolean;
  currentMode: string;
  automaticMigrationEnabled: boolean;
  reviewerNoteChecksum?: string | null;
  reviewerNoteLength: number;
  reviewerNoteCategory: string;
  createdAt: string;
}

export interface DefaultChatAdapterActivationReviewSummary {
  latestDecision?: DefaultChatAdapterActivationReviewLatestDecision | null;
  approvedCount: number;
  rejectOrReworkCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterActivationImplementationGateInput {
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
}

export interface DefaultChatAdapterActivationImplementationGateReport {
  implementationGateEligible: boolean;
  draftReady: boolean;
  latestDecision?: DefaultChatAdapterActivationReviewLatestDecision | null;
  currentActivationPlanDigest: string;
  activationPlanDigestMatched: boolean;
  defaultChatUnchanged: boolean;
  automaticMigrationEnabled: boolean;
  currentMode: string;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterRoutingStatusInput {
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
}

export interface DefaultChatAdapterRoutingStatus {
  currentMode: string;
  adapterScaffoldPresent: boolean;
  controlledAdapterEnabled: boolean;
  defaultSendPath: string;
  startStreamPath: string;
  activationImplementationGateEligible: boolean;
  requiresSeparateCutoverImplementation: boolean;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterContractHarnessInput {
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  sessionId?: string;
}

export interface DefaultChatAdapterContractCheck {
  name: string;
  ready: boolean;
  expectedPath: string;
  actualPath: string;
  blockingReasons: string[];
}

export interface DefaultChatAdapterContractHarnessReport {
  contractHarnessReady: boolean;
  contractShape: string;
  adapterDisabled: boolean;
  activationImplementationGateEligible: boolean;
  routingStatus: DefaultChatAdapterRoutingStatus;
  sendMessageContract: DefaultChatAdapterContractCheck;
  streamMessageContract: DefaultChatAdapterContractCheck;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterDryRunInput {
  sessionId: string;
  message: string;
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
}

export interface DefaultChatAdapterDryRunReport {
  dryRunReady: boolean;
  blocked: boolean;
  contractShape: string;
  sourceSessionId: string;
  adapterPath: string;
  allowWrites: boolean;
  maxToolCalls: number;
  defaultChatPathUnchanged: boolean;
  chatMessageSaved: boolean;
  agentRunRecorded: boolean;
  contractHarnessReady: boolean;
  inputMessageLength: number;
  inputMessageHash: string;
  userOutputPreview?: string | null;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export type DefaultChatAdapterDryRunReviewDecisionKind = "approve" | "reject" | "request_rework";

export interface DefaultChatAdapterDryRunReviewDecisionInput {
  decisionKind: DefaultChatAdapterDryRunReviewDecisionKind;
  sourceSessionId: string;
  message: string;
  dryRunSummaryDigest?: string;
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
  optionalReviewerNote?: string;
}

export interface DefaultChatAdapterDryRunReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  decisionKind: DefaultChatAdapterDryRunReviewDecisionKind;
  sourceSessionId: string;
  contractShape: string;
  dryRunReady: boolean;
  dryRunSummaryDigest: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface DefaultChatAdapterDryRunReviewLatestDecision {
  evidenceId: string;
  decisionKind: DefaultChatAdapterDryRunReviewDecisionKind;
  sourceSessionId: string;
  contractShape: string;
  dryRunReady: boolean;
  dryRunSummaryDigest: string;
  reviewerNoteChecksum?: string | null;
  reviewerNoteLength: number;
  reviewerNoteCategory: string;
  createdAt: string;
}

export interface DefaultChatAdapterDryRunReviewSummary {
  latestDecision?: DefaultChatAdapterDryRunReviewLatestDecision | null;
  approvedCount: number;
  rejectOrReworkCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterImplementationReadinessInput {
  sourceSessionId: string;
  message: string;
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
}

export interface DefaultChatAdapterImplementationReadinessReport {
  implementationReady: boolean;
  latestDryRunReviewDecision?: DefaultChatAdapterDryRunReviewLatestDecision | null;
  activationImplementationGateEligible: boolean;
  contractHarnessReady: boolean;
  dryRunReady: boolean;
  dryRunReviewApproved: boolean;
  dryRunDigestMatched: boolean;
  defaultChatUnchanged: boolean;
  controlledAdapterEnabled: boolean;
  automaticMigrationEnabled: boolean;
  defaultSendPath: string;
  startStreamPath: string;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface DefaultChatAdapterControlledPreviewInput {
  sourceSessionId: string;
  message: string;
  requiredApprovedCandidates?: number;
  requiredPromotions?: number;
}

export interface DefaultChatAdapterControlledPreviewReport {
  previewReady: boolean;
  blocked: boolean;
  contractShape: string;
  sourceSessionId: string;
  adapterPath: string;
  reply?: string | null;
  reasoningTrace: Record<string, unknown>;
  toolCalls: unknown[];
  runId?: string | null;
  allowWrites: boolean;
  maxToolCalls: number;
  defaultChatPathUnchanged: boolean;
  chatMessageSaved: boolean;
  agentRunRecorded: boolean;
  implementationReady: boolean;
  warnings: string[];
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export type DefaultChatAdapterControlledPreviewReviewDecisionKind =
  | "approve"
  | "reject"
  | "request_rework";

export interface DefaultChatAdapterControlledPreviewReviewDecisionInput {
  previewRunId: string;
  decisionKind: DefaultChatAdapterControlledPreviewReviewDecisionKind;
  optionalReviewerNote?: string;
}

export interface DefaultChatAdapterControlledPreviewReviewDecisionResult {
  recorded: boolean;
  evidenceId?: string | null;
  previewRunId: string;
  decisionKind: DefaultChatAdapterControlledPreviewReviewDecisionKind;
  contractShape: string;
  previewSummaryDigest: string;
  createdAt: string;
  blockingReasons: string[];
}

export interface DefaultChatAdapterControlledPreviewReviewLatestDecision {
  evidenceId: string;
  previewRunId: string;
  decisionKind: DefaultChatAdapterControlledPreviewReviewDecisionKind;
  contractShape: string;
  previewSummaryDigest: string;
  reviewerNoteChecksum?: string | null;
  reviewerNoteLength: number;
  reviewerNoteCategory: string;
  createdAt: string;
}

export interface DefaultChatAdapterControlledPreviewReviewSummary {
  latestDecision?: DefaultChatAdapterControlledPreviewReviewLatestDecision | null;
  approvedCount: number;
  rejectOrReworkCount: number;
  latestTimestamp?: string | null;
  blockingReasons: string[];
  metadataSafeSummary: Record<string, unknown>;
}

export interface LifeModelVersion {
  version: string;
  timestamp: string;
  tag: string;
  note: string;
  yaml_content: string;
}
