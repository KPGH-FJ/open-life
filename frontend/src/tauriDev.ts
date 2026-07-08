import { invoke } from "@tauri-apps/api/core";

type DevPayload = Record<string, any>;
type MultiStrategyAgentPreviewInput = DevPayload;
type MultiStrategyAgentPreviewOutput = DevPayload;
type MultiStrategyRuntimeMaturityReport = DevPayload;
type RuntimeMigrationGateCheckInput = DevPayload;
type RuntimeMigrationGateReport = DevPayload;
type ControlledChatPilotEligibilityCheckInput = DevPayload;
type ControlledChatPilotEligibilityReport = DevPayload;
type ControlledPilotPromotionEvidenceInput = DevPayload;
type ControlledPilotPromotionEvidenceResult = DevPayload;
type ControlledPilotPromotionEvidenceSummary = DevPayload;
type ControlledPilotPromotionReadinessCheckInput = DevPayload;
type ControlledPilotPromotionReadinessReport = DevPayload;
type ControlledChatMigrationPlanDraftInput = DevPayload;
type ControlledChatMigrationPlanDraft = DevPayload;
type ControlledChatMigrationReviewDecisionInput = DevPayload;
type ControlledChatMigrationReviewDecisionResult = DevPayload;
type ControlledChatMigrationReviewDecisionSummary = DevPayload;
type ControlledChatMigrationImplementationGateInput = DevPayload;
type ControlledChatMigrationImplementationGateReport = DevPayload;
type ControlledChatMigrationShadowRunInput = DevPayload;
type ControlledChatMigrationShadowRunOutput = DevPayload;
type ControlledChatMigrationShadowReviewDecisionInput = DevPayload;
type ControlledChatMigrationShadowReviewDecisionResult = DevPayload;
type ControlledChatMigrationShadowReviewSummary = DevPayload;
type ControlledChatCutoverReadinessInput = DevPayload;
type ControlledChatCutoverReadinessReport = DevPayload;
type ControlledChatCutoverCandidateInput = DevPayload;
type ControlledChatCutoverCandidateOutput = DevPayload;
type ControlledChatCutoverCandidateReviewDecisionInput = DevPayload;
type ControlledChatCutoverCandidateReviewDecisionResult = DevPayload;
type ControlledChatCutoverCandidateReviewSummary = DevPayload;
type ControlledChatCutoverCandidatePromotionReadinessInput = DevPayload;
type ControlledChatCutoverCandidatePromotionReadinessReport = DevPayload;

function isTauriEnv(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

function safeDevInvoke<T>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (!isTauriEnv()) {
    return Promise.reject(
      new Error("当前不在 OpenLife 桌面应用环境中，无法调用原生开发功能。请在桌面窗口内操作。")
    );
  }
  return invoke<T>(cmd, args);
}

export async function runMultiStrategyAgentPreview(
  input: MultiStrategyAgentPreviewInput
): Promise<MultiStrategyAgentPreviewOutput> {
  return safeDevInvoke<MultiStrategyAgentPreviewOutput>("run_multi_strategy_agent_preview", {
    input,
  });
}

export async function getRuntimeStrategyRegistryStatus(): Promise<MultiStrategyRuntimeMaturityReport> {
  return safeDevInvoke<MultiStrategyRuntimeMaturityReport>("get_runtime_strategy_registry_status");
}

export async function getReactBetaExecutionStatus(): Promise<any> {
  return safeDevInvoke("get_react_beta_execution_status");
}

export async function runMainChatAgentExecutionV1EvalGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_execution_v1_eval_gate");
}

export async function runMainChatAgentProductizationV1Gate(): Promise<any> {
  return safeDevInvoke("run_main_chat_runtime_contract_gate");
}

export async function runMainChatStage3ExecutionUxReport(): Promise<any> {
  return safeDevInvoke("run_main_chat_stage3_execution_ux_report");
}

export async function runMainChatExternalLiveProductizationGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_external_live_productization_gate");
}

export async function runMainChatAgentProductMaturityV2EventGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_product_maturity_v2_event_gate");
}

export async function runMainChatAgentProductMaturityV2PlanGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_product_maturity_v2_plan_gate");
}

export async function runMainChatAgentProductMaturityV2SkillsGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_product_maturity_v2_skills_gate");
}

export async function runMainChatAgentProductMaturityV2FinalReadinessGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_product_maturity_v2_final_readiness_gate");
}

export async function runMainChatAgentBetaV1ReadinessGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_beta_v1_readiness_gate");
}

export async function runMainChatAgentStage2ReadinessGate(): Promise<any> {
  return safeDevInvoke("run_main_chat_agent_stage2_readiness_gate");
}

export async function validateMainChatAgentStage2ManualDogfoodArtifact(): Promise<any> {
  return safeDevInvoke("validate_main_chat_agent_stage2_manual_dogfood_artifact");
}

export async function checkRuntimeMigrationGate(
  input: RuntimeMigrationGateCheckInput = {}
): Promise<RuntimeMigrationGateReport> {
  return safeDevInvoke<RuntimeMigrationGateReport>("check_runtime_migration_gate", { input });
}

export async function checkControlledChatPilotEligibility(
  input: ControlledChatPilotEligibilityCheckInput = {}
): Promise<ControlledChatPilotEligibilityReport> {
  return safeDevInvoke<ControlledChatPilotEligibilityReport>(
    "check_controlled_chat_pilot_eligibility",
    { input }
  );
}

export async function recordControlledPilotPromotionEvidence(
  input: ControlledPilotPromotionEvidenceInput
): Promise<ControlledPilotPromotionEvidenceResult> {
  return safeDevInvoke<ControlledPilotPromotionEvidenceResult>(
    "record_controlled_pilot_promotion_evidence",
    { input }
  );
}

export async function getControlledPilotPromotionEvidenceSummary(): Promise<ControlledPilotPromotionEvidenceSummary> {
  return safeDevInvoke<ControlledPilotPromotionEvidenceSummary>(
    "get_controlled_pilot_promotion_evidence_summary"
  );
}

export async function checkControlledPilotPromotionReadiness(
  input: ControlledPilotPromotionReadinessCheckInput = {}
): Promise<ControlledPilotPromotionReadinessReport> {
  return safeDevInvoke<ControlledPilotPromotionReadinessReport>(
    "check_controlled_pilot_promotion_readiness",
    { input }
  );
}

export async function draftControlledChatMigrationPlan(
  input: ControlledChatMigrationPlanDraftInput = {}
): Promise<ControlledChatMigrationPlanDraft> {
  return safeDevInvoke<ControlledChatMigrationPlanDraft>("draft_controlled_chat_migration_plan", {
    input,
  });
}

export async function recordControlledChatMigrationReviewDecision(
  input: ControlledChatMigrationReviewDecisionInput
): Promise<ControlledChatMigrationReviewDecisionResult> {
  return safeDevInvoke<ControlledChatMigrationReviewDecisionResult>(
    "record_controlled_chat_migration_review_decision",
    { input }
  );
}

export async function getControlledChatMigrationReviewDecisionSummary(): Promise<ControlledChatMigrationReviewDecisionSummary> {
  return safeDevInvoke<ControlledChatMigrationReviewDecisionSummary>(
    "get_controlled_chat_migration_review_decision_summary"
  );
}

export async function checkControlledChatMigrationImplementationGate(
  input: ControlledChatMigrationImplementationGateInput = {}
): Promise<ControlledChatMigrationImplementationGateReport> {
  return safeDevInvoke<ControlledChatMigrationImplementationGateReport>(
    "check_controlled_chat_migration_implementation_gate",
    { input }
  );
}

export async function runControlledChatMigrationShadowRun(
  input: ControlledChatMigrationShadowRunInput
): Promise<ControlledChatMigrationShadowRunOutput> {
  return safeDevInvoke<ControlledChatMigrationShadowRunOutput>(
    "run_controlled_chat_migration_shadow_run",
    { input }
  );
}

export async function recordControlledChatMigrationShadowReviewDecision(
  input: ControlledChatMigrationShadowReviewDecisionInput
): Promise<ControlledChatMigrationShadowReviewDecisionResult> {
  return safeDevInvoke<ControlledChatMigrationShadowReviewDecisionResult>(
    "record_controlled_chat_migration_shadow_review_decision",
    { input }
  );
}

export async function getControlledChatMigrationShadowReviewSummary(): Promise<ControlledChatMigrationShadowReviewSummary> {
  return safeDevInvoke<ControlledChatMigrationShadowReviewSummary>(
    "get_controlled_chat_migration_shadow_review_summary"
  );
}

export async function checkControlledChatCutoverReadiness(
  input: ControlledChatCutoverReadinessInput = {}
): Promise<ControlledChatCutoverReadinessReport> {
  return safeDevInvoke<ControlledChatCutoverReadinessReport>(
    "check_controlled_chat_cutover_readiness",
    { input }
  );
}

export async function runControlledChatCutoverCandidate(
  input: ControlledChatCutoverCandidateInput
): Promise<ControlledChatCutoverCandidateOutput> {
  return safeDevInvoke<ControlledChatCutoverCandidateOutput>(
    "run_controlled_chat_cutover_candidate",
    { input }
  );
}

export async function recordControlledChatCutoverCandidateReviewDecision(
  input: ControlledChatCutoverCandidateReviewDecisionInput
): Promise<ControlledChatCutoverCandidateReviewDecisionResult> {
  return safeDevInvoke<ControlledChatCutoverCandidateReviewDecisionResult>(
    "record_controlled_chat_cutover_candidate_review_decision",
    { input }
  );
}

export async function getControlledChatCutoverCandidateReviewSummary(): Promise<ControlledChatCutoverCandidateReviewSummary> {
  return safeDevInvoke<ControlledChatCutoverCandidateReviewSummary>(
    "get_controlled_chat_cutover_candidate_review_summary"
  );
}

export async function checkControlledChatCutoverCandidatePromotionReadiness(
  input: ControlledChatCutoverCandidatePromotionReadinessInput = {}
): Promise<ControlledChatCutoverCandidatePromotionReadinessReport> {
  return safeDevInvoke<ControlledChatCutoverCandidatePromotionReadinessReport>(
    "check_controlled_chat_cutover_candidate_promotion_readiness",
    { input }
  );
}

export async function listStage4KnowledgeAssetInventory(selectedSkillId?: string): Promise<any> {
  return safeDevInvoke("list_stage4_knowledge_asset_inventory", {
    selectedSkillId,
    selected_skill_id: selectedSkillId,
  });
}

export async function createManagedKnowledgeWriteDraft(
  targetPath: string,
  afterContent: string,
  sourceProposalId?: string,
  linkedMemoryIds: string[] = []
): Promise<any> {
  return safeDevInvoke("create_managed_knowledge_write_draft", {
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

export async function confirmManagedKnowledgeWrite(proposalId: string): Promise<any> {
  return safeDevInvoke("confirm_managed_knowledge_write", {
    proposalId,
    proposal_id: proposalId,
  });
}

export async function rollbackManagedKnowledgeWrite(versionId: string): Promise<any> {
  return safeDevInvoke("rollback_managed_knowledge_write", {
    versionId,
    version_id: versionId,
  });
}

export async function runMainChatStage4MemoryKnowledgeReport(): Promise<any> {
  return safeDevInvoke("run_main_chat_stage4_memory_knowledge_report");
}

export async function evaluateMainChatStage5ReleaseDebugPreflight(): Promise<any> {
  return safeDevInvoke("evaluate_main_chat_stage5_release_debug_preflight");
}

export async function exportMainChatAgentDebugBundle(
  taskSessionId: string,
  options: {
    scenarioId?: string;
    reviewerId?: string;
    uiEvidence?: Record<string, unknown>;
  } = {}
): Promise<any> {
  return safeDevInvoke("export_main_chat_agent_debug_bundle", {
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
  input: Record<string, unknown>
): Promise<any> {
  return safeDevInvoke("create_main_chat_internal_issue_report", { input });
}

export async function listMainChatDebugBundles(): Promise<any[]> {
  return safeDevInvoke("list_main_chat_debug_bundles");
}

export async function getMainChatDebugBundle(bundleId: string): Promise<any> {
  return safeDevInvoke("get_main_chat_debug_bundle", { bundleId, bundle_id: bundleId });
}

export async function deleteMainChatDebugBundle(bundleId: string): Promise<boolean> {
  return safeDevInvoke("delete_main_chat_debug_bundle", { bundleId, bundle_id: bundleId });
}

export async function listMainChatInternalIssueReports(): Promise<any[]> {
  return safeDevInvoke("list_main_chat_internal_issue_reports");
}

export async function getMainChatInternalIssueReport(reportId: string): Promise<any> {
  return safeDevInvoke("get_main_chat_internal_issue_report", { reportId, report_id: reportId });
}

export async function deleteMainChatInternalIssueReport(reportId: string): Promise<boolean> {
  return safeDevInvoke("delete_main_chat_internal_issue_report", { reportId, report_id: reportId });
}

export async function runMainChatStage5ReleaseDebugReport(): Promise<any> {
  return safeDevInvoke("run_main_chat_stage5_release_debug_report");
}
