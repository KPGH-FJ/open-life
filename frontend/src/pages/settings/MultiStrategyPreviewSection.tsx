import { type FormEvent, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  checkControlledChatCutoverCandidatePromotionReadiness,
  checkControlledChatCutoverReadiness,
  checkControlledChatPilotEligibility,
  checkControlledChatMigrationImplementationGate,
  checkControlledPilotPromotionReadiness,
  checkDefaultChatAdapterActivationImplementationGate,
  checkDefaultChatAdapterContractHarness,
  checkDefaultChatAdapterControlledPreviewApprovalReadiness,
  checkDefaultChatAdapterCutoverPlanApprovalReadiness,
  checkDefaultChatAdapterImplementationReadiness,
  checkDefaultChatAdapterNarrowImplementationDiscussionGate,
  checkDefaultChatAdapterNarrowImplementationPlanApprovalReadiness,
  checkRuntimeMigrationGate,
  draftDefaultChatAdapterCutoverImplementationPlan,
  draftDefaultChatAdapterNarrowImplementationPlan,
  draftDefaultChatAdapterActivationPlan,
  draftControlledChatMigrationPlan,
  getDefaultChatAdapterNarrowImplementationPlanReviewSummary,
  getDefaultChatAdapterActivationReviewSummary,
  getDefaultChatAdapterControlledPreviewReviewSummary,
  getDefaultChatAdapterCutoverPlanReviewSummary,
  getDefaultChatAdapterDryRunReviewSummary,
  getDefaultChatAdapterOrdinaryEntryPreflightStatus,
  getDefaultChatAdapterRoutingStatus,
  getControlledChatCutoverCandidateReviewSummary,
  getControlledChatMigrationReviewDecisionSummary,
  getControlledChatMigrationShadowReviewSummary,
  getControlledPilotPromotionEvidenceSummary,
  getDefaultChatRuntimeBoundaryStatus,
  recordDefaultChatAdapterActivationReviewDecision,
  recordDefaultChatAdapterControlledPreviewReviewDecision,
  recordDefaultChatAdapterCutoverPlanReviewDecision,
  recordDefaultChatAdapterDryRunReviewDecision,
  recordDefaultChatAdapterNarrowImplementationPlanReviewDecision,
  recordControlledChatCutoverCandidateReviewDecision,
  recordControlledChatMigrationReviewDecision,
  recordControlledChatMigrationShadowReviewDecision,
  runControlledChatCutoverCandidate,
  runControlledChatMigrationShadowRun,
  runDefaultChatAdapterControlledPreview,
  runDefaultChatAdapterDryRun,
  runMultiStrategyAgentPreview,
} from "../../tauri";
import type {
  ControlledChatCutoverCandidateOutput,
  ControlledChatCutoverCandidateReviewDecisionKind,
  ControlledChatCutoverCandidateReviewDecisionResult,
  ControlledChatCutoverCandidateReviewSummary,
  ControlledChatCutoverCandidatePromotionReadinessReport,
  ControlledChatCutoverReadinessReport,
  DefaultChatAdapterActivationPlanDraft,
  DefaultChatAdapterActivationImplementationGateReport,
  DefaultChatAdapterContractHarnessReport,
  DefaultChatAdapterControlledPreviewApprovalReadinessReport,
  DefaultChatAdapterCutoverImplementationPlanDraft,
  DefaultChatAdapterCutoverPlanApprovalReadinessReport,
  DefaultChatAdapterCutoverPlanReviewDecisionKind,
  DefaultChatAdapterCutoverPlanReviewDecisionResult,
  DefaultChatAdapterCutoverPlanReviewSummary,
  DefaultChatAdapterControlledPreviewReport,
  DefaultChatAdapterControlledPreviewReviewDecisionKind,
  DefaultChatAdapterControlledPreviewReviewDecisionResult,
  DefaultChatAdapterControlledPreviewReviewSummary,
  DefaultChatAdapterDryRunReport,
  DefaultChatAdapterDryRunReviewDecisionKind,
  DefaultChatAdapterDryRunReviewDecisionResult,
  DefaultChatAdapterDryRunReviewSummary,
  DefaultChatAdapterImplementationReadinessReport,
  DefaultChatAdapterNarrowImplementationDiscussionGateReport,
  DefaultChatAdapterNarrowImplementationPlanDraft,
  DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport,
  DefaultChatAdapterNarrowImplementationPlanReviewDecisionKind,
  DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult,
  DefaultChatAdapterNarrowImplementationPlanReviewSummary,
  DefaultChatAdapterOrdinaryEntryPreflightStatus,
  DefaultChatAdapterRoutingStatus,
  DefaultChatAdapterActivationReviewDecisionKind,
  DefaultChatAdapterActivationReviewDecisionResult,
  DefaultChatAdapterActivationReviewSummary,
  ControlledChatMigrationImplementationGateReport,
  ControlledChatMigrationPlanDraft,
  ControlledChatMigrationReviewDecisionKind,
  ControlledChatMigrationReviewDecisionResult,
  ControlledChatMigrationReviewDecisionSummary,
  ControlledChatMigrationShadowRunDescriptor,
  ControlledChatMigrationShadowRunOutput,
  ControlledChatMigrationShadowReviewDecisionKind,
  ControlledChatMigrationShadowReviewDecisionResult,
  ControlledChatMigrationShadowReviewSummary,
  ControlledChatPilotEligibilityReport,
  ControlledPilotPromotionEvidenceSummary,
  ControlledPilotPromotionReadinessReport,
  DefaultChatRuntimeBoundaryStatus,
  MultiStrategyAgentPreviewLayer,
  MultiStrategyAgentPreviewOutput,
  RuntimeMigrationGateReport,
} from "../../types";
import {
  ControlledPilotPanel,
  CutoverCandidatePanel,
  DefaultChatActivationPanel,
  DefaultChatCutoverPanel,
  DefaultChatImplementationPanel,
  MigrationPlanningPanel,
  NarrowImplementationPanel,
  RuntimePreviewPanel,
  type MultiStrategyPanelProps,
} from "./multiStrategy/panels";
import { readableError, safeSummaryEntries } from "./multiStrategy/shared";
import { isInternalDebugSurfaceEnabled } from "../../utils/internalDebug";

const NO_TOOLS_PROMPT = "No developer tools catalog supplied for this preview.";
export default function MultiStrategyPreviewSection() {
  if (!isInternalDebugSurfaceEnabled()) {
    return null;
  }
  return <MultiStrategyPreviewSectionInner />;
}

function MultiStrategyPreviewSectionInner() {
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [userText, setUserText] = useState("");
  const [allowPlanning, setAllowPlanning] = useState(false);
  const [localModelAvailable, setLocalModelAvailable] = useState(false);
  const [layer, setLayer] = useState<MultiStrategyAgentPreviewLayer>("L2");
  const [toolsPrompt, setToolsPrompt] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<MultiStrategyAgentPreviewOutput | null>(null);
  const [boundaryChecking, setBoundaryChecking] = useState(false);
  const [boundaryError, setBoundaryError] = useState<string | null>(null);
  const [boundaryStatus, setBoundaryStatus] = useState<DefaultChatRuntimeBoundaryStatus | null>(
    null
  );
  const [gateChecking, setGateChecking] = useState(false);
  const [gateError, setGateError] = useState<string | null>(null);
  const [gateReport, setGateReport] = useState<RuntimeMigrationGateReport | null>(null);
  const [pilotChecking, setPilotChecking] = useState(false);
  const [pilotError, setPilotError] = useState<string | null>(null);
  const [pilotReport, setPilotReport] = useState<ControlledChatPilotEligibilityReport | null>(null);
  const [promotionSummaryChecking, setPromotionSummaryChecking] = useState(false);
  const [promotionSummaryError, setPromotionSummaryError] = useState<string | null>(null);
  const [promotionSummary, setPromotionSummary] =
    useState<ControlledPilotPromotionEvidenceSummary | null>(null);
  const [promotionReadinessChecking, setPromotionReadinessChecking] = useState(false);
  const [promotionReadinessError, setPromotionReadinessError] = useState<string | null>(null);
  const [promotionReadinessReport, setPromotionReadinessReport] =
    useState<ControlledPilotPromotionReadinessReport | null>(null);
  const [migrationDraftChecking, setMigrationDraftChecking] = useState(false);
  const [migrationDraftError, setMigrationDraftError] = useState<string | null>(null);
  const [migrationDraft, setMigrationDraft] = useState<ControlledChatMigrationPlanDraft | null>(
    null
  );
  const [reviewerNote, setReviewerNote] = useState("");
  const [reviewDecisionRecording, setReviewDecisionRecording] = useState(false);
  const [reviewDecisionError, setReviewDecisionError] = useState<string | null>(null);
  const [reviewDecisionResult, setReviewDecisionResult] =
    useState<ControlledChatMigrationReviewDecisionResult | null>(null);
  const [reviewDecisionSummaryChecking, setReviewDecisionSummaryChecking] = useState(false);
  const [reviewDecisionSummaryError, setReviewDecisionSummaryError] = useState<string | null>(null);
  const [reviewDecisionSummary, setReviewDecisionSummary] =
    useState<ControlledChatMigrationReviewDecisionSummary | null>(null);
  const [implementationGateChecking, setImplementationGateChecking] = useState(false);
  const [implementationGateError, setImplementationGateError] = useState<string | null>(null);
  const [implementationGateReport, setImplementationGateReport] =
    useState<ControlledChatMigrationImplementationGateReport | null>(null);
  const [shadowDescriptor, setShadowDescriptor] =
    useState<ControlledChatMigrationShadowRunDescriptor>("default_readiness_probe");
  const [shadowRunChecking, setShadowRunChecking] = useState(false);
  const [shadowRunError, setShadowRunError] = useState<string | null>(null);
  const [shadowRunResult, setShadowRunResult] =
    useState<ControlledChatMigrationShadowRunOutput | null>(null);
  const [shadowReviewNote, setShadowReviewNote] = useState("");
  const [shadowReviewRecording, setShadowReviewRecording] = useState(false);
  const [shadowReviewError, setShadowReviewError] = useState<string | null>(null);
  const [shadowReviewResult, setShadowReviewResult] =
    useState<ControlledChatMigrationShadowReviewDecisionResult | null>(null);
  const [shadowReviewSummaryChecking, setShadowReviewSummaryChecking] = useState(false);
  const [shadowReviewSummaryError, setShadowReviewSummaryError] = useState<string | null>(null);
  const [shadowReviewSummary, setShadowReviewSummary] =
    useState<ControlledChatMigrationShadowReviewSummary | null>(null);
  const [cutoverReadinessChecking, setCutoverReadinessChecking] = useState(false);
  const [cutoverReadinessError, setCutoverReadinessError] = useState<string | null>(null);
  const [cutoverReadinessReport, setCutoverReadinessReport] =
    useState<ControlledChatCutoverReadinessReport | null>(null);
  const [cutoverCandidateChecking, setCutoverCandidateChecking] = useState(false);
  const [cutoverCandidateError, setCutoverCandidateError] = useState<string | null>(null);
  const [cutoverCandidateResult, setCutoverCandidateResult] =
    useState<ControlledChatCutoverCandidateOutput | null>(null);
  const [cutoverCandidateReviewNote, setCutoverCandidateReviewNote] = useState("");
  const [cutoverCandidateReviewRecording, setCutoverCandidateReviewRecording] = useState(false);
  const [cutoverCandidateReviewError, setCutoverCandidateReviewError] = useState<string | null>(
    null
  );
  const [cutoverCandidateReviewResult, setCutoverCandidateReviewResult] =
    useState<ControlledChatCutoverCandidateReviewDecisionResult | null>(null);
  const [cutoverCandidateReviewSummaryChecking, setCutoverCandidateReviewSummaryChecking] =
    useState(false);
  const [cutoverCandidateReviewSummaryError, setCutoverCandidateReviewSummaryError] = useState<
    string | null
  >(null);
  const [cutoverCandidateReviewSummary, setCutoverCandidateReviewSummary] =
    useState<ControlledChatCutoverCandidateReviewSummary | null>(null);
  const [candidatePromotionReadinessChecking, setCandidatePromotionReadinessChecking] =
    useState(false);
  const [candidatePromotionReadinessError, setCandidatePromotionReadinessError] = useState<
    string | null
  >(null);
  const [candidatePromotionReadinessReport, setCandidatePromotionReadinessReport] =
    useState<ControlledChatCutoverCandidatePromotionReadinessReport | null>(null);
  const [activationPlanChecking, setActivationPlanChecking] = useState(false);
  const [activationPlanError, setActivationPlanError] = useState<string | null>(null);
  const [activationPlanDraft, setActivationPlanDraft] =
    useState<DefaultChatAdapterActivationPlanDraft | null>(null);
  const [activationReviewNote, setActivationReviewNote] = useState("");
  const [activationReviewRecording, setActivationReviewRecording] = useState(false);
  const [activationReviewError, setActivationReviewError] = useState<string | null>(null);
  const [activationReviewResult, setActivationReviewResult] =
    useState<DefaultChatAdapterActivationReviewDecisionResult | null>(null);
  const [activationReviewSummaryChecking, setActivationReviewSummaryChecking] = useState(false);
  const [activationReviewSummaryError, setActivationReviewSummaryError] = useState<string | null>(
    null
  );
  const [activationReviewSummary, setActivationReviewSummary] =
    useState<DefaultChatAdapterActivationReviewSummary | null>(null);
  const [activationImplementationGateChecking, setActivationImplementationGateChecking] =
    useState(false);
  const [activationImplementationGateError, setActivationImplementationGateError] = useState<
    string | null
  >(null);
  const [activationImplementationGateReport, setActivationImplementationGateReport] =
    useState<DefaultChatAdapterActivationImplementationGateReport | null>(null);
  const [adapterRoutingChecking, setAdapterRoutingChecking] = useState(false);
  const [adapterRoutingError, setAdapterRoutingError] = useState<string | null>(null);
  const [adapterRoutingStatus, setAdapterRoutingStatus] =
    useState<DefaultChatAdapterRoutingStatus | null>(null);
  const [ordinaryEntryPreflightChecking, setOrdinaryEntryPreflightChecking] = useState(false);
  const [ordinaryEntryPreflightError, setOrdinaryEntryPreflightError] = useState<string | null>(
    null
  );
  const [ordinaryEntryPreflightStatus, setOrdinaryEntryPreflightStatus] =
    useState<DefaultChatAdapterOrdinaryEntryPreflightStatus | null>(null);
  const [contractHarnessChecking, setContractHarnessChecking] = useState(false);
  const [contractHarnessError, setContractHarnessError] = useState<string | null>(null);
  const [contractHarnessReport, setContractHarnessReport] =
    useState<DefaultChatAdapterContractHarnessReport | null>(null);
  const [adapterDryRunChecking, setAdapterDryRunChecking] = useState(false);
  const [adapterDryRunError, setAdapterDryRunError] = useState<string | null>(null);
  const [adapterDryRunReport, setAdapterDryRunReport] =
    useState<DefaultChatAdapterDryRunReport | null>(null);
  const [adapterDryRunReviewNote, setAdapterDryRunReviewNote] = useState("");
  const [adapterDryRunReviewRecording, setAdapterDryRunReviewRecording] = useState(false);
  const [adapterDryRunReviewError, setAdapterDryRunReviewError] = useState<string | null>(null);
  const [adapterDryRunReviewResult, setAdapterDryRunReviewResult] =
    useState<DefaultChatAdapterDryRunReviewDecisionResult | null>(null);
  const [adapterDryRunReviewSummaryChecking, setAdapterDryRunReviewSummaryChecking] =
    useState(false);
  const [adapterDryRunReviewSummaryError, setAdapterDryRunReviewSummaryError] = useState<
    string | null
  >(null);
  const [adapterDryRunReviewSummary, setAdapterDryRunReviewSummary] =
    useState<DefaultChatAdapterDryRunReviewSummary | null>(null);
  const [adapterImplementationReadinessChecking, setAdapterImplementationReadinessChecking] =
    useState(false);
  const [adapterImplementationReadinessError, setAdapterImplementationReadinessError] = useState<
    string | null
  >(null);
  const [adapterImplementationReadinessReport, setAdapterImplementationReadinessReport] =
    useState<DefaultChatAdapterImplementationReadinessReport | null>(null);
  const [adapterControlledPreviewChecking, setAdapterControlledPreviewChecking] = useState(false);
  const [adapterControlledPreviewError, setAdapterControlledPreviewError] = useState<string | null>(
    null
  );
  const [adapterControlledPreviewReport, setAdapterControlledPreviewReport] =
    useState<DefaultChatAdapterControlledPreviewReport | null>(null);
  const [adapterControlledPreviewReviewNote, setAdapterControlledPreviewReviewNote] = useState("");
  const [adapterControlledPreviewReviewRecording, setAdapterControlledPreviewReviewRecording] =
    useState(false);
  const [adapterControlledPreviewReviewError, setAdapterControlledPreviewReviewError] = useState<
    string | null
  >(null);
  const [adapterControlledPreviewReviewResult, setAdapterControlledPreviewReviewResult] =
    useState<DefaultChatAdapterControlledPreviewReviewDecisionResult | null>(null);
  const [
    adapterControlledPreviewReviewSummaryChecking,
    setAdapterControlledPreviewReviewSummaryChecking,
  ] = useState(false);
  const [
    adapterControlledPreviewReviewSummaryError,
    setAdapterControlledPreviewReviewSummaryError,
  ] = useState<string | null>(null);
  const [adapterControlledPreviewReviewSummary, setAdapterControlledPreviewReviewSummary] =
    useState<DefaultChatAdapterControlledPreviewReviewSummary | null>(null);
  const [
    adapterControlledPreviewApprovalReadinessChecking,
    setAdapterControlledPreviewApprovalReadinessChecking,
  ] = useState(false);
  const [
    adapterControlledPreviewApprovalReadinessError,
    setAdapterControlledPreviewApprovalReadinessError,
  ] = useState<string | null>(null);
  const [
    adapterControlledPreviewApprovalReadinessReport,
    setAdapterControlledPreviewApprovalReadinessReport,
  ] = useState<DefaultChatAdapterControlledPreviewApprovalReadinessReport | null>(null);
  const [adapterCutoverPlanDrafting, setAdapterCutoverPlanDrafting] = useState(false);
  const [adapterCutoverPlanError, setAdapterCutoverPlanError] = useState<string | null>(null);
  const [adapterCutoverPlanDraft, setAdapterCutoverPlanDraft] =
    useState<DefaultChatAdapterCutoverImplementationPlanDraft | null>(null);
  const [adapterCutoverPlanReviewNote, setAdapterCutoverPlanReviewNote] = useState("");
  const [adapterCutoverPlanReviewRecording, setAdapterCutoverPlanReviewRecording] = useState(false);
  const [adapterCutoverPlanReviewError, setAdapterCutoverPlanReviewError] = useState<string | null>(
    null
  );
  const [adapterCutoverPlanReviewResult, setAdapterCutoverPlanReviewResult] =
    useState<DefaultChatAdapterCutoverPlanReviewDecisionResult | null>(null);
  const [adapterCutoverPlanReviewSummaryChecking, setAdapterCutoverPlanReviewSummaryChecking] =
    useState(false);
  const [adapterCutoverPlanReviewSummaryError, setAdapterCutoverPlanReviewSummaryError] = useState<
    string | null
  >(null);
  const [adapterCutoverPlanReviewSummary, setAdapterCutoverPlanReviewSummary] =
    useState<DefaultChatAdapterCutoverPlanReviewSummary | null>(null);
  const [
    adapterCutoverPlanApprovalReadinessChecking,
    setAdapterCutoverPlanApprovalReadinessChecking,
  ] = useState(false);
  const [adapterCutoverPlanApprovalReadinessError, setAdapterCutoverPlanApprovalReadinessError] =
    useState<string | null>(null);
  const [adapterCutoverPlanApprovalReadinessReport, setAdapterCutoverPlanApprovalReadinessReport] =
    useState<DefaultChatAdapterCutoverPlanApprovalReadinessReport | null>(null);
  const [narrowImplementationGateChecking, setNarrowImplementationGateChecking] = useState(false);
  const [narrowImplementationGateError, setNarrowImplementationGateError] = useState<string | null>(
    null
  );
  const [narrowImplementationGateReport, setNarrowImplementationGateReport] =
    useState<DefaultChatAdapterNarrowImplementationDiscussionGateReport | null>(null);
  const [narrowImplementationPlanDrafting, setNarrowImplementationPlanDrafting] = useState(false);
  const [narrowImplementationPlanError, setNarrowImplementationPlanError] = useState<string | null>(
    null
  );
  const [narrowImplementationPlanDraft, setNarrowImplementationPlanDraft] =
    useState<DefaultChatAdapterNarrowImplementationPlanDraft | null>(null);
  const [narrowImplementationPlanReviewNote, setNarrowImplementationPlanReviewNote] = useState("");
  const [narrowImplementationPlanReviewRecording, setNarrowImplementationPlanReviewRecording] =
    useState(false);
  const [narrowImplementationPlanReviewError, setNarrowImplementationPlanReviewError] = useState<
    string | null
  >(null);
  const [narrowImplementationPlanReviewResult, setNarrowImplementationPlanReviewResult] =
    useState<DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult | null>(null);
  const [
    narrowImplementationPlanReviewSummaryChecking,
    setNarrowImplementationPlanReviewSummaryChecking,
  ] = useState(false);
  const [
    narrowImplementationPlanReviewSummaryError,
    setNarrowImplementationPlanReviewSummaryError,
  ] = useState<string | null>(null);
  const [narrowImplementationPlanReviewSummary, setNarrowImplementationPlanReviewSummary] =
    useState<DefaultChatAdapterNarrowImplementationPlanReviewSummary | null>(null);
  const [
    narrowImplementationPlanApprovalReadinessChecking,
    setNarrowImplementationPlanApprovalReadinessChecking,
  ] = useState(false);
  const [
    narrowImplementationPlanApprovalReadinessError,
    setNarrowImplementationPlanApprovalReadinessError,
  ] = useState<string | null>(null);
  const [
    narrowImplementationPlanApprovalReadinessReport,
    setNarrowImplementationPlanApprovalReadinessReport,
  ] = useState<DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport | null>(null);

  const summaryEntries = useMemo(
    () => safeSummaryEntries(result?.metadataSafeSummary ?? {}),
    [result]
  );

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const trimmedUserText = userText.trim();
    if (!trimmedUserText) {
      setError("userText is required for preview.");
      return;
    }

    setSubmitting(true);
    setError(null);
    setResult(null);
    setGateError(null);
    setGateReport(null);
    setPilotError(null);
    setPilotReport(null);

    try {
      const output = await runMultiStrategyAgentPreview({
        sessionId: `runtime-preview-${Date.now()}`,
        userText: trimmedUserText,
        toolsPrompt: toolsPrompt.trim() || NO_TOOLS_PROMPT,
        allowPlanning,
        localModelAvailable,
        layer,
        executionBudget: {
          allowWrites: false,
        },
      });
      setResult(output);
      setUserText("");
      setToolsPrompt("");
    } catch (e) {
      setError(`Preview failed: ${readableError(e)}`);
    } finally {
      setSubmitting(false);
    }
  };

  const handleDefaultChatBoundaryRefresh = async () => {
    setBoundaryChecking(true);
    setBoundaryError(null);
    setBoundaryStatus(null);
    try {
      const status = await getDefaultChatRuntimeBoundaryStatus();
      setBoundaryStatus(status);
    } catch (e) {
      setBoundaryError(`Default Chat boundary refresh failed: ${readableError(e)}`);
    } finally {
      setBoundaryChecking(false);
    }
  };

  const handleGateCheck = async () => {
    setGateChecking(true);
    setGateError(null);
    setGateReport(null);
    try {
      const input = result?.runId ? { previewRunId: result.runId } : {};
      const report = await checkRuntimeMigrationGate(input);
      setGateReport(report);
    } catch (e) {
      setGateError(`Gate check failed: ${readableError(e)}`);
    } finally {
      setGateChecking(false);
    }
  };

  const handlePilotEligibilityCheck = async () => {
    setPilotChecking(true);
    setPilotError(null);
    setPilotReport(null);
    try {
      const report = await checkControlledChatPilotEligibility();
      setPilotReport(report);
    } catch (e) {
      setPilotError(`Pilot eligibility check failed: ${readableError(e)}`);
    } finally {
      setPilotChecking(false);
    }
  };

  const handlePromotionSummaryRefresh = async () => {
    setPromotionSummaryChecking(true);
    setPromotionSummaryError(null);
    try {
      const summary = await getControlledPilotPromotionEvidenceSummary();
      setPromotionSummary(summary);
    } catch (e) {
      setPromotionSummaryError(`Promotion evidence summary failed: ${readableError(e)}`);
    } finally {
      setPromotionSummaryChecking(false);
    }
  };

  const handlePromotionReadinessCheck = async () => {
    setPromotionReadinessChecking(true);
    setPromotionReadinessError(null);
    setPromotionReadinessReport(null);
    try {
      const report = await checkControlledPilotPromotionReadiness();
      setPromotionReadinessReport(report);
    } catch (e) {
      setPromotionReadinessError(`Promotion readiness check failed: ${readableError(e)}`);
    } finally {
      setPromotionReadinessChecking(false);
    }
  };

  const handleMigrationDraft = async () => {
    setMigrationDraftChecking(true);
    setMigrationDraftError(null);
    setMigrationDraft(null);
    setReviewDecisionResult(null);
    setReviewDecisionError(null);
    try {
      const draft = await draftControlledChatMigrationPlan();
      setMigrationDraft(draft);
    } catch (e) {
      setMigrationDraftError(`Migration plan draft failed: ${readableError(e)}`);
    } finally {
      setMigrationDraftChecking(false);
    }
  };

  const handleReviewDecisionSummaryRefresh = async () => {
    setReviewDecisionSummaryChecking(true);
    setReviewDecisionSummaryError(null);
    try {
      const summary = await getControlledChatMigrationReviewDecisionSummary();
      setReviewDecisionSummary(summary);
    } catch (e) {
      setReviewDecisionSummaryError(`Review decision summary failed: ${readableError(e)}`);
    } finally {
      setReviewDecisionSummaryChecking(false);
    }
  };

  const handleRecordReviewDecision = async (
    decisionKind: ControlledChatMigrationReviewDecisionKind
  ) => {
    setReviewDecisionRecording(true);
    setReviewDecisionError(null);
    setReviewDecisionResult(null);
    try {
      const trimmedNote = reviewerNote.trim();
      const result = await recordControlledChatMigrationReviewDecision({
        decisionKind,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setReviewDecisionResult(result);
      if (result.recorded) {
        setReviewerNote("");
      }
    } catch (e) {
      setReviewDecisionError(`Review decision recording failed: ${readableError(e)}`);
    } finally {
      setReviewDecisionRecording(false);
    }
  };

  const handleImplementationGateCheck = async () => {
    setImplementationGateChecking(true);
    setImplementationGateError(null);
    setImplementationGateReport(null);
    try {
      const report = await checkControlledChatMigrationImplementationGate();
      setImplementationGateReport(report);
    } catch (e) {
      setImplementationGateError(`Implementation gate check failed: ${readableError(e)}`);
    } finally {
      setImplementationGateChecking(false);
    }
  };

  const handleShadowRun = async () => {
    setShadowRunChecking(true);
    setShadowRunError(null);
    setShadowRunResult(null);
    setShadowReviewError(null);
    setShadowReviewResult(null);
    try {
      const result = await runControlledChatMigrationShadowRun({
        sessionId: `settings-shadow-run-${Date.now()}`,
        boundedTestPromptDescriptor: shadowDescriptor,
        requiredPromotions: 3,
      });
      setShadowRunResult(result);
    } catch (e) {
      setShadowRunError(`Shadow run failed: ${readableError(e)}`);
    } finally {
      setShadowRunChecking(false);
    }
  };

  const handleShadowReviewSummaryRefresh = async () => {
    setShadowReviewSummaryChecking(true);
    setShadowReviewSummaryError(null);
    try {
      const summary = await getControlledChatMigrationShadowReviewSummary();
      setShadowReviewSummary(summary);
    } catch (e) {
      setShadowReviewSummaryError(`Shadow review summary failed: ${readableError(e)}`);
    } finally {
      setShadowReviewSummaryChecking(false);
    }
  };

  const handleRecordShadowReviewDecision = async (
    decisionKind: ControlledChatMigrationShadowReviewDecisionKind
  ) => {
    const shadowRunId = shadowRunResult?.shadowRunReady ? shadowRunResult.shadowRunId : null;
    if (!shadowRunId) {
      setShadowReviewError("Shadow review recording requires a ready shadow run id.");
      return;
    }

    setShadowReviewRecording(true);
    setShadowReviewError(null);
    setShadowReviewResult(null);
    try {
      const trimmedNote = shadowReviewNote.trim();
      const result = await recordControlledChatMigrationShadowReviewDecision({
        shadowRunId,
        decisionKind,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setShadowReviewResult(result);
      if (result.recorded) {
        setShadowReviewNote("");
      }
    } catch (e) {
      setShadowReviewError(`Shadow review recording failed: ${readableError(e)}`);
    } finally {
      setShadowReviewRecording(false);
    }
  };

  const handleCutoverReadinessCheck = async () => {
    setCutoverReadinessChecking(true);
    setCutoverReadinessError(null);
    setCutoverReadinessReport(null);
    try {
      const report = await checkControlledChatCutoverReadiness();
      setCutoverReadinessReport(report);
    } catch (e) {
      setCutoverReadinessError(`Cutover readiness check failed: ${readableError(e)}`);
    } finally {
      setCutoverReadinessChecking(false);
    }
  };

  const handleCutoverCandidateRun = async () => {
    setCutoverCandidateChecking(true);
    setCutoverCandidateError(null);
    setCutoverCandidateResult(null);
    setCutoverCandidateReviewError(null);
    setCutoverCandidateReviewResult(null);
    try {
      const result = await runControlledChatCutoverCandidate({
        sessionId: `settings-cutover-candidate-${Date.now()}`,
        boundedTestPromptDescriptor: "default_contract_probe",
        requiredPromotions: 3,
      });
      setCutoverCandidateResult(result);
    } catch (e) {
      setCutoverCandidateError(`Cutover candidate failed: ${readableError(e)}`);
    } finally {
      setCutoverCandidateChecking(false);
    }
  };

  const handleCutoverCandidateReviewSummaryRefresh = async () => {
    setCutoverCandidateReviewSummaryChecking(true);
    setCutoverCandidateReviewSummaryError(null);
    try {
      const summary = await getControlledChatCutoverCandidateReviewSummary();
      setCutoverCandidateReviewSummary(summary);
    } catch (e) {
      setCutoverCandidateReviewSummaryError(`Candidate review summary failed: ${readableError(e)}`);
    } finally {
      setCutoverCandidateReviewSummaryChecking(false);
    }
  };

  const handleCandidatePromotionReadinessRefresh = async () => {
    setCandidatePromotionReadinessChecking(true);
    setCandidatePromotionReadinessError(null);
    setCandidatePromotionReadinessReport(null);
    try {
      const report = await checkControlledChatCutoverCandidatePromotionReadiness({
        requiredApprovedCandidates: 1,
      });
      setCandidatePromotionReadinessReport(report);
    } catch (e) {
      setCandidatePromotionReadinessError(
        `Candidate promotion readiness failed: ${readableError(e)}`
      );
    } finally {
      setCandidatePromotionReadinessChecking(false);
    }
  };

  const handleActivationPlanRefresh = async () => {
    setActivationPlanChecking(true);
    setActivationPlanError(null);
    setActivationPlanDraft(null);
    try {
      const draft = await draftDefaultChatAdapterActivationPlan({
        requiredApprovedCandidates: 1,
      });
      setActivationPlanDraft(draft);
    } catch (e) {
      setActivationPlanError(`Activation plan draft failed: ${readableError(e)}`);
    } finally {
      setActivationPlanChecking(false);
    }
  };

  const handleActivationReviewSummaryRefresh = async () => {
    setActivationReviewSummaryChecking(true);
    setActivationReviewSummaryError(null);
    try {
      const summary = await getDefaultChatAdapterActivationReviewSummary();
      setActivationReviewSummary(summary);
    } catch (e) {
      setActivationReviewSummaryError(`Activation review summary failed: ${readableError(e)}`);
    } finally {
      setActivationReviewSummaryChecking(false);
    }
  };

  const handleRecordActivationReviewDecision = async (
    decisionKind: DefaultChatAdapterActivationReviewDecisionKind
  ) => {
    setActivationReviewRecording(true);
    setActivationReviewError(null);
    setActivationReviewResult(null);
    try {
      const trimmedNote = activationReviewNote.trim();
      const result = await recordDefaultChatAdapterActivationReviewDecision({
        decisionKind,
        requiredApprovedCandidates: 1,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setActivationReviewResult(result);
      if (result.recorded) {
        setActivationReviewNote("");
        await handleActivationReviewSummaryRefresh();
      }
    } catch (e) {
      setActivationReviewError(`Activation review recording failed: ${readableError(e)}`);
    } finally {
      setActivationReviewRecording(false);
    }
  };

  const handleActivationImplementationGateCheck = async () => {
    setActivationImplementationGateChecking(true);
    setActivationImplementationGateError(null);
    setActivationImplementationGateReport(null);
    try {
      const report = await checkDefaultChatAdapterActivationImplementationGate({
        requiredApprovedCandidates: 1,
      });
      setActivationImplementationGateReport(report);
    } catch (e) {
      setActivationImplementationGateError(
        `Activation implementation gate failed: ${readableError(e)}`
      );
    } finally {
      setActivationImplementationGateChecking(false);
    }
  };

  const handleAdapterRoutingRefresh = async () => {
    setAdapterRoutingChecking(true);
    setAdapterRoutingError(null);
    setAdapterRoutingStatus(null);
    try {
      const status = await getDefaultChatAdapterRoutingStatus({
        requiredApprovedCandidates: 1,
      });
      setAdapterRoutingStatus(status);
    } catch (e) {
      setAdapterRoutingError(`Adapter routing status failed: ${readableError(e)}`);
    } finally {
      setAdapterRoutingChecking(false);
    }
  };

  const handleOrdinaryEntryPreflightRefresh = async () => {
    setOrdinaryEntryPreflightChecking(true);
    setOrdinaryEntryPreflightError(null);
    setOrdinaryEntryPreflightStatus(null);
    try {
      const status = await getDefaultChatAdapterOrdinaryEntryPreflightStatus();
      setOrdinaryEntryPreflightStatus(status);
    } catch (e) {
      setOrdinaryEntryPreflightError(`Ordinary entry preflight status failed: ${readableError(e)}`);
    } finally {
      setOrdinaryEntryPreflightChecking(false);
    }
  };

  const handleContractHarnessCheck = async () => {
    setContractHarnessChecking(true);
    setContractHarnessError(null);
    setContractHarnessReport(null);
    try {
      const report = await checkDefaultChatAdapterContractHarness({
        requiredApprovedCandidates: 1,
      });
      setContractHarnessReport(report);
    } catch (e) {
      setContractHarnessError(`Adapter contract harness failed: ${readableError(e)}`);
    } finally {
      setContractHarnessChecking(false);
    }
  };

  const handleAdapterDryRun = async () => {
    setAdapterDryRunChecking(true);
    setAdapterDryRunError(null);
    setAdapterDryRunReport(null);
    setAdapterDryRunReviewError(null);
    setAdapterDryRunReviewResult(null);
    try {
      const report = await runDefaultChatAdapterDryRun({
        sessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedCandidates: 1,
      });
      setAdapterDryRunReport(report);
    } catch (e) {
      setAdapterDryRunError(`Adapter dry run failed: ${readableError(e)}`);
    } finally {
      setAdapterDryRunChecking(false);
    }
  };

  const handleAdapterDryRunReviewSummaryRefresh = async () => {
    setAdapterDryRunReviewSummaryChecking(true);
    setAdapterDryRunReviewSummaryError(null);
    try {
      const summary = await getDefaultChatAdapterDryRunReviewSummary();
      setAdapterDryRunReviewSummary(summary);
    } catch (e) {
      setAdapterDryRunReviewSummaryError(
        `Adapter dry-run review summary failed: ${readableError(e)}`
      );
    } finally {
      setAdapterDryRunReviewSummaryChecking(false);
    }
  };

  const handleRecordAdapterDryRunReviewDecision = async (
    decisionKind: DefaultChatAdapterDryRunReviewDecisionKind
  ) => {
    setAdapterDryRunReviewRecording(true);
    setAdapterDryRunReviewError(null);
    setAdapterDryRunReviewResult(null);
    try {
      const trimmedNote = adapterDryRunReviewNote.trim();
      const result = await recordDefaultChatAdapterDryRunReviewDecision({
        decisionKind,
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedCandidates: 1,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setAdapterDryRunReviewResult(result);
      if (result.recorded) {
        setAdapterDryRunReviewNote("");
        await handleAdapterDryRunReviewSummaryRefresh();
      }
    } catch (e) {
      setAdapterDryRunReviewError(`Adapter dry-run review recording failed: ${readableError(e)}`);
    } finally {
      setAdapterDryRunReviewRecording(false);
    }
  };

  const handleAdapterImplementationReadinessCheck = async () => {
    setAdapterImplementationReadinessChecking(true);
    setAdapterImplementationReadinessError(null);
    setAdapterImplementationReadinessReport(null);
    try {
      const report = await checkDefaultChatAdapterImplementationReadiness({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedCandidates: 1,
      });
      setAdapterImplementationReadinessReport(report);
    } catch (e) {
      setAdapterImplementationReadinessError(
        `Adapter implementation readiness failed: ${readableError(e)}`
      );
    } finally {
      setAdapterImplementationReadinessChecking(false);
    }
  };

  const handleAdapterControlledPreview = async () => {
    setAdapterControlledPreviewChecking(true);
    setAdapterControlledPreviewError(null);
    setAdapterControlledPreviewReport(null);
    setAdapterControlledPreviewReviewError(null);
    setAdapterControlledPreviewReviewResult(null);
    try {
      const report = await runDefaultChatAdapterControlledPreview({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedCandidates: 1,
      });
      setAdapterControlledPreviewReport(report);
    } catch (e) {
      setAdapterControlledPreviewError(`Adapter controlled preview failed: ${readableError(e)}`);
    } finally {
      setAdapterControlledPreviewChecking(false);
    }
  };

  const handleAdapterControlledPreviewReviewSummaryRefresh = async () => {
    setAdapterControlledPreviewReviewSummaryChecking(true);
    setAdapterControlledPreviewReviewSummaryError(null);
    try {
      const summary = await getDefaultChatAdapterControlledPreviewReviewSummary();
      setAdapterControlledPreviewReviewSummary(summary);
    } catch (e) {
      setAdapterControlledPreviewReviewSummaryError(
        `Adapter controlled preview review summary failed: ${readableError(e)}`
      );
    } finally {
      setAdapterControlledPreviewReviewSummaryChecking(false);
    }
  };

  const handleRecordAdapterControlledPreviewReviewDecision = async (
    decisionKind: DefaultChatAdapterControlledPreviewReviewDecisionKind
  ) => {
    const previewRunId = adapterControlledPreviewReport?.runId;
    if (!previewRunId) {
      setAdapterControlledPreviewReviewError(
        "Controlled preview review recording requires a preview AgentRun id."
      );
      return;
    }

    setAdapterControlledPreviewReviewRecording(true);
    setAdapterControlledPreviewReviewError(null);
    setAdapterControlledPreviewReviewResult(null);
    try {
      const trimmedNote = adapterControlledPreviewReviewNote.trim();
      const result = await recordDefaultChatAdapterControlledPreviewReviewDecision({
        previewRunId,
        decisionKind,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setAdapterControlledPreviewReviewResult(result);
      if (result.recorded) {
        setAdapterControlledPreviewReviewNote("");
        await handleAdapterControlledPreviewReviewSummaryRefresh();
      }
    } catch (e) {
      setAdapterControlledPreviewReviewError(
        `Adapter controlled preview review recording failed: ${readableError(e)}`
      );
    } finally {
      setAdapterControlledPreviewReviewRecording(false);
    }
  };

  const handleAdapterControlledPreviewApprovalReadinessCheck = async () => {
    setAdapterControlledPreviewApprovalReadinessChecking(true);
    setAdapterControlledPreviewApprovalReadinessError(null);
    setAdapterControlledPreviewApprovalReadinessReport(null);
    try {
      const report = await checkDefaultChatAdapterControlledPreviewApprovalReadiness({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setAdapterControlledPreviewApprovalReadinessReport(report);
    } catch (e) {
      setAdapterControlledPreviewApprovalReadinessError(
        `Adapter controlled preview approval readiness failed: ${readableError(e)}`
      );
    } finally {
      setAdapterControlledPreviewApprovalReadinessChecking(false);
    }
  };

  const handleAdapterCutoverPlanDraft = async () => {
    setAdapterCutoverPlanDrafting(true);
    setAdapterCutoverPlanError(null);
    setAdapterCutoverPlanDraft(null);
    try {
      const draft = await draftDefaultChatAdapterCutoverImplementationPlan({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setAdapterCutoverPlanDraft(draft);
    } catch (e) {
      setAdapterCutoverPlanError(`Adapter cutover implementation plan failed: ${readableError(e)}`);
    } finally {
      setAdapterCutoverPlanDrafting(false);
    }
  };

  const handleAdapterCutoverPlanReviewSummary = async () => {
    setAdapterCutoverPlanReviewSummaryChecking(true);
    setAdapterCutoverPlanReviewSummaryError(null);
    try {
      const summary = await getDefaultChatAdapterCutoverPlanReviewSummary();
      setAdapterCutoverPlanReviewSummary(summary);
    } catch (e) {
      setAdapterCutoverPlanReviewSummaryError(
        `Adapter cutover plan review summary failed: ${readableError(e)}`
      );
    } finally {
      setAdapterCutoverPlanReviewSummaryChecking(false);
    }
  };

  const handleAdapterCutoverPlanReviewDecision = async (
    decisionKind: DefaultChatAdapterCutoverPlanReviewDecisionKind
  ) => {
    setAdapterCutoverPlanReviewRecording(true);
    setAdapterCutoverPlanReviewError(null);
    setAdapterCutoverPlanReviewResult(null);
    try {
      const trimmedNote = adapterCutoverPlanReviewNote.trim();
      const result = await recordDefaultChatAdapterCutoverPlanReviewDecision({
        decisionKind,
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setAdapterCutoverPlanReviewResult(result);
      if (result.recorded) {
        setAdapterCutoverPlanReviewNote("");
        void handleAdapterCutoverPlanReviewSummary();
      }
    } catch (e) {
      setAdapterCutoverPlanReviewError(
        `Adapter cutover plan review recording failed: ${readableError(e)}`
      );
    } finally {
      setAdapterCutoverPlanReviewRecording(false);
    }
  };

  const handleAdapterCutoverPlanApprovalReadinessCheck = async () => {
    setAdapterCutoverPlanApprovalReadinessChecking(true);
    setAdapterCutoverPlanApprovalReadinessError(null);
    setAdapterCutoverPlanApprovalReadinessReport(null);
    try {
      const report = await checkDefaultChatAdapterCutoverPlanApprovalReadiness({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setAdapterCutoverPlanApprovalReadinessReport(report);
    } catch (e) {
      setAdapterCutoverPlanApprovalReadinessError(
        `Adapter cutover plan approval readiness failed: ${readableError(e)}`
      );
    } finally {
      setAdapterCutoverPlanApprovalReadinessChecking(false);
    }
  };

  const handleNarrowImplementationGateCheck = async () => {
    setNarrowImplementationGateChecking(true);
    setNarrowImplementationGateError(null);
    setNarrowImplementationGateReport(null);
    try {
      const report = await checkDefaultChatAdapterNarrowImplementationDiscussionGate({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setNarrowImplementationGateReport(report);
    } catch (e) {
      setNarrowImplementationGateError(`Narrow implementation gate failed: ${readableError(e)}`);
    } finally {
      setNarrowImplementationGateChecking(false);
    }
  };

  const handleNarrowImplementationPlanDraft = async () => {
    setNarrowImplementationPlanDrafting(true);
    setNarrowImplementationPlanError(null);
    setNarrowImplementationPlanDraft(null);
    setNarrowImplementationPlanReviewError(null);
    setNarrowImplementationPlanReviewResult(null);
    try {
      const draft = await draftDefaultChatAdapterNarrowImplementationPlan({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setNarrowImplementationPlanDraft(draft);
    } catch (e) {
      setNarrowImplementationPlanError(
        `Narrow implementation plan draft failed: ${readableError(e)}`
      );
    } finally {
      setNarrowImplementationPlanDrafting(false);
    }
  };

  const handleNarrowImplementationPlanReviewSummaryRefresh = async () => {
    setNarrowImplementationPlanReviewSummaryChecking(true);
    setNarrowImplementationPlanReviewSummaryError(null);
    try {
      const summary = await getDefaultChatAdapterNarrowImplementationPlanReviewSummary();
      setNarrowImplementationPlanReviewSummary(summary);
    } catch (e) {
      setNarrowImplementationPlanReviewSummaryError(
        `Narrow implementation plan review summary failed: ${readableError(e)}`
      );
    } finally {
      setNarrowImplementationPlanReviewSummaryChecking(false);
    }
  };

  const handleRecordNarrowImplementationPlanReviewDecision = async (
    decisionKind: DefaultChatAdapterNarrowImplementationPlanReviewDecisionKind
  ) => {
    setNarrowImplementationPlanReviewRecording(true);
    setNarrowImplementationPlanReviewError(null);
    setNarrowImplementationPlanReviewResult(null);
    try {
      const trimmedNote = narrowImplementationPlanReviewNote.trim();
      const result = await recordDefaultChatAdapterNarrowImplementationPlanReviewDecision({
        decisionKind,
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setNarrowImplementationPlanReviewResult(result);
      if (result.recorded) {
        setNarrowImplementationPlanReviewNote("");
        await handleNarrowImplementationPlanReviewSummaryRefresh();
      }
    } catch (e) {
      setNarrowImplementationPlanReviewError(
        `Narrow implementation plan review recording failed: ${readableError(e)}`
      );
    } finally {
      setNarrowImplementationPlanReviewRecording(false);
    }
  };

  const handleNarrowImplementationPlanApprovalReadinessCheck = async () => {
    setNarrowImplementationPlanApprovalReadinessChecking(true);
    setNarrowImplementationPlanApprovalReadinessError(null);
    setNarrowImplementationPlanApprovalReadinessReport(null);
    try {
      const report = await checkDefaultChatAdapterNarrowImplementationPlanApprovalReadiness({
        sourceSessionId: "settings-dry-run",
        message: "Settings adapter dry-run probe.",
        requiredApprovedPreviews: 1,
        requiredApprovedCandidates: 1,
      });
      setNarrowImplementationPlanApprovalReadinessReport(report);
    } catch (e) {
      setNarrowImplementationPlanApprovalReadinessError(
        `Narrow implementation plan approval readiness failed: ${readableError(e)}`
      );
    } finally {
      setNarrowImplementationPlanApprovalReadinessChecking(false);
    }
  };

  const handleRecordCutoverCandidateReviewDecision = async (
    decisionKind: ControlledChatCutoverCandidateReviewDecisionKind
  ) => {
    const candidateRunId = cutoverCandidateResult?.candidateRunId;
    if (!candidateRunId) {
      setCutoverCandidateReviewError(
        "Candidate review recording requires a candidate AgentRun id."
      );
      return;
    }

    setCutoverCandidateReviewRecording(true);
    setCutoverCandidateReviewError(null);
    setCutoverCandidateReviewResult(null);
    try {
      const trimmedNote = cutoverCandidateReviewNote.trim();
      const result = await recordControlledChatCutoverCandidateReviewDecision({
        candidateRunId,
        decisionKind,
        ...(trimmedNote ? { optionalReviewerNote: trimmedNote } : {}),
      });
      setCutoverCandidateReviewResult(result);
      if (result.recorded) {
        setCutoverCandidateReviewNote("");
      }
    } catch (e) {
      setCutoverCandidateReviewError(`Candidate review recording failed: ${readableError(e)}`);
    } finally {
      setCutoverCandidateReviewRecording(false);
    }
  };

  const panelProps: MultiStrategyPanelProps = {
    activationImplementationGateChecking,
    activationImplementationGateError,
    activationImplementationGateReport,
    activationPlanChecking,
    activationPlanDraft,
    activationPlanError,
    activationReviewError,
    activationReviewNote,
    activationReviewRecording,
    activationReviewResult,
    activationReviewSummary,
    activationReviewSummaryChecking,
    activationReviewSummaryError,
    adapterControlledPreviewApprovalReadinessChecking,
    adapterControlledPreviewApprovalReadinessError,
    adapterControlledPreviewApprovalReadinessReport,
    adapterControlledPreviewChecking,
    adapterControlledPreviewError,
    adapterControlledPreviewReport,
    adapterControlledPreviewReviewError,
    adapterControlledPreviewReviewNote,
    adapterControlledPreviewReviewRecording,
    adapterControlledPreviewReviewResult,
    adapterControlledPreviewReviewSummary,
    adapterControlledPreviewReviewSummaryChecking,
    adapterControlledPreviewReviewSummaryError,
    adapterCutoverPlanApprovalReadinessChecking,
    adapterCutoverPlanApprovalReadinessError,
    adapterCutoverPlanApprovalReadinessReport,
    adapterCutoverPlanDraft,
    adapterCutoverPlanDrafting,
    adapterCutoverPlanError,
    adapterCutoverPlanReviewError,
    adapterCutoverPlanReviewNote,
    adapterCutoverPlanReviewRecording,
    adapterCutoverPlanReviewResult,
    adapterCutoverPlanReviewSummary,
    adapterCutoverPlanReviewSummaryChecking,
    adapterCutoverPlanReviewSummaryError,
    adapterDryRunChecking,
    adapterDryRunError,
    adapterDryRunReport,
    adapterDryRunReviewError,
    adapterDryRunReviewNote,
    adapterDryRunReviewRecording,
    adapterDryRunReviewResult,
    adapterDryRunReviewSummary,
    adapterDryRunReviewSummaryChecking,
    adapterDryRunReviewSummaryError,
    adapterImplementationReadinessChecking,
    adapterImplementationReadinessError,
    adapterImplementationReadinessReport,
    adapterRoutingChecking,
    adapterRoutingError,
    adapterRoutingStatus,
    advancedOpen,
    allowPlanning,
    boundaryChecking,
    boundaryError,
    boundaryStatus,
    candidatePromotionReadinessChecking,
    candidatePromotionReadinessError,
    candidatePromotionReadinessReport,
    contractHarnessChecking,
    contractHarnessError,
    contractHarnessReport,
    cutoverCandidateChecking,
    cutoverCandidateError,
    cutoverCandidateResult,
    cutoverCandidateReviewError,
    cutoverCandidateReviewNote,
    cutoverCandidateReviewRecording,
    cutoverCandidateReviewResult,
    cutoverCandidateReviewSummary,
    cutoverCandidateReviewSummaryChecking,
    cutoverCandidateReviewSummaryError,
    cutoverReadinessChecking,
    cutoverReadinessError,
    cutoverReadinessReport,
    error,
    gateChecking,
    gateError,
    gateReport,
    handleActivationImplementationGateCheck,
    handleActivationPlanRefresh,
    handleActivationReviewSummaryRefresh,
    handleAdapterControlledPreview,
    handleAdapterControlledPreviewApprovalReadinessCheck,
    handleAdapterControlledPreviewReviewSummaryRefresh,
    handleAdapterCutoverPlanApprovalReadinessCheck,
    handleAdapterCutoverPlanDraft,
    handleAdapterCutoverPlanReviewDecision,
    handleAdapterCutoverPlanReviewSummary,
    handleAdapterDryRun,
    handleAdapterDryRunReviewSummaryRefresh,
    handleAdapterImplementationReadinessCheck,
    handleAdapterRoutingRefresh,
    handleCandidatePromotionReadinessRefresh,
    handleContractHarnessCheck,
    handleCutoverCandidateReviewSummaryRefresh,
    handleCutoverCandidateRun,
    handleCutoverReadinessCheck,
    handleDefaultChatBoundaryRefresh,
    handleGateCheck,
    handleImplementationGateCheck,
    handleMigrationDraft,
    handleNarrowImplementationGateCheck,
    handleNarrowImplementationPlanApprovalReadinessCheck,
    handleNarrowImplementationPlanDraft,
    handleNarrowImplementationPlanReviewSummaryRefresh,
    handleOrdinaryEntryPreflightRefresh,
    handlePilotEligibilityCheck,
    handlePromotionReadinessCheck,
    handlePromotionSummaryRefresh,
    handleRecordActivationReviewDecision,
    handleRecordAdapterControlledPreviewReviewDecision,
    handleRecordAdapterDryRunReviewDecision,
    handleRecordCutoverCandidateReviewDecision,
    handleRecordNarrowImplementationPlanReviewDecision,
    handleRecordReviewDecision,
    handleRecordShadowReviewDecision,
    handleReviewDecisionSummaryRefresh,
    handleShadowReviewSummaryRefresh,
    handleShadowRun,
    handleSubmit,
    implementationGateChecking,
    implementationGateError,
    implementationGateReport,
    layer,
    localModelAvailable,
    migrationDraft,
    migrationDraftChecking,
    migrationDraftError,
    narrowImplementationGateChecking,
    narrowImplementationGateError,
    narrowImplementationGateReport,
    narrowImplementationPlanApprovalReadinessChecking,
    narrowImplementationPlanApprovalReadinessError,
    narrowImplementationPlanApprovalReadinessReport,
    narrowImplementationPlanDraft,
    narrowImplementationPlanDrafting,
    narrowImplementationPlanError,
    narrowImplementationPlanReviewError,
    narrowImplementationPlanReviewNote,
    narrowImplementationPlanReviewRecording,
    narrowImplementationPlanReviewResult,
    narrowImplementationPlanReviewSummary,
    narrowImplementationPlanReviewSummaryChecking,
    narrowImplementationPlanReviewSummaryError,
    navigate,
    open,
    ordinaryEntryPreflightChecking,
    ordinaryEntryPreflightError,
    ordinaryEntryPreflightStatus,
    pilotChecking,
    pilotError,
    pilotReport,
    promotionReadinessChecking,
    promotionReadinessError,
    promotionReadinessReport,
    promotionSummary,
    promotionSummaryChecking,
    promotionSummaryError,
    result,
    reviewDecisionError,
    reviewDecisionRecording,
    reviewDecisionResult,
    reviewDecisionSummary,
    reviewDecisionSummaryChecking,
    reviewDecisionSummaryError,
    reviewerNote,
    setActivationReviewNote,
    setAdapterControlledPreviewReviewNote,
    setAdapterCutoverPlanReviewNote,
    setAdapterDryRunReviewNote,
    setAdvancedOpen,
    setAllowPlanning,
    setCutoverCandidateReviewNote,
    setLayer,
    setLocalModelAvailable,
    setNarrowImplementationPlanReviewNote,
    setOpen,
    setReviewerNote,
    setShadowDescriptor,
    setShadowReviewNote,
    setToolsPrompt,
    setUserText,
    shadowDescriptor,
    shadowReviewError,
    shadowReviewNote,
    shadowReviewRecording,
    shadowReviewResult,
    shadowReviewSummary,
    shadowReviewSummaryChecking,
    shadowReviewSummaryError,
    shadowRunChecking,
    shadowRunError,
    shadowRunResult,
    submitting,
    summaryEntries,
    toolsPrompt,
    userText,
  };

  return (
    <div className="space-y-4">
      <RuntimePreviewPanel {...panelProps} />
      <ControlledPilotPanel {...panelProps} />
      <MigrationPlanningPanel {...panelProps} />
      <CutoverCandidatePanel {...panelProps} />
      <DefaultChatActivationPanel {...panelProps} />
      <DefaultChatImplementationPanel {...panelProps} />
      <DefaultChatCutoverPanel {...panelProps} />
      <NarrowImplementationPanel {...panelProps} />
    </div>
  );
}
