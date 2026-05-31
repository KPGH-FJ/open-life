import { type FormEvent, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Play,
  RefreshCw,
  ShieldCheck,
  XCircle,
} from "lucide-react";
import {
  checkControlledChatCutoverCandidatePromotionReadiness,
  checkControlledChatCutoverReadiness,
  checkControlledChatPilotEligibility,
  checkControlledChatMigrationImplementationGate,
  checkControlledPilotPromotionReadiness,
  checkDefaultChatAdapterActivationImplementationGate,
  checkDefaultChatAdapterContractHarness,
  checkDefaultChatAdapterImplementationReadiness,
  checkRuntimeMigrationGate,
  draftDefaultChatAdapterActivationPlan,
  draftControlledChatMigrationPlan,
  getDefaultChatAdapterActivationReviewSummary,
  getDefaultChatAdapterDryRunReviewSummary,
  getDefaultChatAdapterRoutingStatus,
  getControlledChatCutoverCandidateReviewSummary,
  getControlledChatMigrationReviewDecisionSummary,
  getControlledChatMigrationShadowReviewSummary,
  getControlledPilotPromotionEvidenceSummary,
  getDefaultChatRuntimeBoundaryStatus,
  recordDefaultChatAdapterActivationReviewDecision,
  recordDefaultChatAdapterDryRunReviewDecision,
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
  DefaultChatAdapterControlledPreviewReport,
  DefaultChatAdapterDryRunReport,
  DefaultChatAdapterDryRunReviewDecisionKind,
  DefaultChatAdapterDryRunReviewDecisionResult,
  DefaultChatAdapterDryRunReviewSummary,
  DefaultChatAdapterImplementationReadinessReport,
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

const NO_TOOLS_PROMPT = "No developer tools catalog supplied for this preview.";
const SAFE_SUMMARY_KEYS = [
  "runtimeBoundary",
  "defaultChatAdapterRouting",
  "contractHarness",
  "adapterDryRun",
  "dryRunReview",
  "implementationReadiness",
  "adapterPreview",
  "activationPlan",
  "activationReview",
  "activationImplementationGate",
  "cutoverReadinessGate",
  "promotionReadinessGate",
  "taskKind",
  "reasonCode",
  "riskLevel",
  "hasHsPacket",
  "policyReasonCode",
  "descriptorKind",
  "candidateAdapter",
  "contractShape",
  "candidateReady",
  "previewReady",
  "nonDefault",
  "allowWrites",
  "maxToolCalls",
  "metadataSafe",
  "planningOnly",
  "requiredEvidenceReady",
  "defaultChatUnchanged",
  "defaultChatPathUnchanged",
  "readOnly",
  "contractHarnessReady",
  "dryRunReady",
  "blocked",
  "adapterPath",
  "adapterDisabled",
  "chatMessageSaved",
  "agentRunRecorded",
  "reviewerNoteStorage",
  "humanReviewOnly",
  "draftReady",
  "manualReviewRequired",
  "notAutomaticMigration",
  "requiresSeparateImplementation",
  "requiresSeparateCutoverImplementation",
  "candidatePromotionReady",
  "currentMode",
  "routingMode",
  "adapterScaffoldPresent",
  "controlledAdapterEnabled",
  "defaultSendPath",
  "startStreamPath",
  "controlledCandidateAvailable",
  "activationImplementationGateEligible",
  "candidatePromotionReadinessRequired",
  "automaticMigrationEnabled",
  "ready",
  "cutoverReadinessEligible",
  "requiredApprovedCandidates",
  "approvedCandidateCount",
  "verifiedCandidateCount",
  "latestDecisionKind",
  "blockingReasonCount",
  "activationSectionCount",
  "preconditionSectionCount",
  "adapterContractCheckCount",
  "fallbackPlanCount",
  "rollbackPlanCount",
  "observabilityPlanCount",
  "testPlanCount",
  "latestDecisionPresent",
  "implementationGateEligible",
  "activationPlanDigestMatched",
  "approvedCount",
  "rejectOrReworkCount",
  "implementationEligible",
  "latestShadowReviewDecisionKind",
  "shadowRunReady",
  "blockedBeforeRuntime",
  "userOutputPresent",
  "outputDigestPresent",
  "contentStorage",
  "toolStorage",
  "chatHistoryStorage",
  "proposalStorage",
  "lifeModelPatchStorage",
  "memoryStorage",
  "evidenceStorage",
  "mcpAuditStorage",
  "transcriptStorage",
  "agentRunStorage",
  "runtimeCallStorage",
  "modelCallStorage",
  "externalWriteStorage",
];
const GATE_FIELDS: Array<keyof Omit<RuntimeMigrationGateReport, "blockingReasons">> = [
  "defaultChatUnchanged",
  "previewPathHealthy",
  "metadataSafeTraceReady",
  "fallbackAvailable",
  "noExternalWrites",
  "proposalFirstPreserved",
];

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

function readableError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    if ("message" in error && typeof (error as any).message === "string") {
      return (error as any).message;
    }
    if ("error" in error && typeof (error as any).error === "string") {
      return (error as any).error;
    }
  }
  return String(error);
}

function safeSummaryEntries(summary: Record<string, unknown>): Array<[string, string]> {
  return SAFE_SUMMARY_KEYS.flatMap(key => {
    const value = summary[key];
    if (value === undefined || value === null) return [];
    if (!["string", "number", "boolean"].includes(typeof value)) return [];
    return [[key, String(value)]];
  });
}

function PlanList({ title, items }: { title: string; items: string[] }) {
  return (
    <div>
      <div className="text-xs font-medium text-stone-700">{title}</div>
      <div className="mt-1 space-y-1">
        {items.map(item => (
          <div
            key={item}
            className="rounded-md border border-stone-100 bg-stone-50 px-2 py-1 text-xs leading-5 text-stone-700"
          >
            {item}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function MultiStrategyPreviewSection() {
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

  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-amber-200 bg-amber-50 p-4 text-sm text-amber-900">
        <div className="flex items-start gap-2">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" />
          <div>
            <div className="font-semibold">Runtime preview / beta</div>
            <div className="mt-1 text-xs leading-5">
              This entry calls the preview command only. It is separate from Chat, forces
              write-disabled execution, and omits raw tools prompts, raw memory context, PII, mail
              bodies, and file content from the result view.
            </div>
          </div>
        </div>
      </div>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Runtime Boundary
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only boundary status for the current default Chat runtime. It reports the legacy
              stream path and does not start a candidate, readiness gate, migration, tool call,
              model call, or write path.
            </div>
          </div>
          <button
            type="button"
            onClick={handleDefaultChatBoundaryRefresh}
            disabled={boundaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              boundaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={boundaryChecking ? "animate-spin" : undefined} />
            {boundaryChecking ? "Refreshing..." : "Refresh Default Chat Boundary"}
          </button>
        </div>

        {boundaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {boundaryError}
          </div>
        )}

        {boundaryStatus && (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-2">
              {[
                ["currentMode", boundaryStatus.currentMode],
                ["defaultChatUnchanged", String(boundaryStatus.defaultChatUnchanged)],
                ["automaticMigrationEnabled", String(boundaryStatus.automaticMigrationEnabled)],
                [
                  "candidatePromotionReadinessRequired",
                  String(boundaryStatus.candidatePromotionReadinessRequired),
                ],
                [
                  "controlledCandidateAvailable",
                  String(boundaryStatus.controlledCandidateAvailable),
                ],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {boundaryStatus.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {boundaryStatus.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No boundary blockers returned.</div>
              )}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Metadata-safe summary</div>
              <div className="mt-1 flex flex-wrap gap-1.5">
                {safeSummaryEntries(boundaryStatus.metadataSafeSummary).map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-stone-200 bg-stone-50 px-2 py-1 font-mono text-xs text-stone-700"
                  >
                    summary.{key}: {value}
                  </span>
                ))}
              </div>
            </div>
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Runtime Migration Gate</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only migration diagnostic for existing MultiStrategy preview audit evidence. This
              is not a Chat switching control and does not run preview, ReAct, PlanExecute, tools,
              or writes.
            </div>
          </div>
          <button
            type="button"
            onClick={handleGateCheck}
            disabled={gateChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              gateChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={gateChecking ? "animate-spin" : undefined} />
            {gateChecking ? "Checking..." : "Check Runtime Migration Gate"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          {result?.runId
            ? `Checking explicit preview run: ${result.runId}`
            : "No preview will be started. The command may read the latest existing preview run."}
        </div>

        {gateError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {gateError}
          </div>
        )}

        {gateReport && (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-2">
              {GATE_FIELDS.map(field => {
                const passed = gateReport[field];
                return (
                  <div
                    key={field}
                    className="flex items-center justify-between gap-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs"
                  >
                    <span className="font-mono text-stone-700">{field}</span>
                    <span
                      className={classNames(
                        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
                        passed ? "bg-emerald-100 text-emerald-800" : "bg-red-100 text-red-700"
                      )}
                    >
                      {passed ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
                      {passed ? "Pass" : "Block"}
                    </span>
                  </div>
                );
              })}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {gateReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {gateReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No blocking reasons returned.</div>
              )}
            </div>
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Pilot eligibility</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only qualification check for a controlled Chat migration pilot. This is not a
              Chat switching control. Even when eligible, default Chat is not replaced automatically
              and the normal Chat path keeps its fallback.
            </div>
          </div>
          <button
            type="button"
            onClick={handlePilotEligibilityCheck}
            disabled={pilotChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              pilotChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={pilotChecking ? "animate-spin" : undefined} />
            {pilotChecking ? "Checking..." : "Check Pilot Eligibility"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          Reads recent existing MultiStrategy preview AgentRun evidence only. It does not run
          preview, ReAct, PlanExecute, tools, proposal apply, LifeModel/Memory writes, or audit
          writes.
        </div>

        {pilotError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {pilotError}
          </div>
        )}

        {pilotReport && (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  pilotReport.eligible
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {pilotReport.eligible ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
                {pilotReport.eligible ? "Eligible" : "Blocked"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {pilotReport.cleanRunCount} / {pilotReport.requiredCleanRuns} clean runs
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  pilotReport.defaultChatUnchanged
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                defaultChatUnchanged: {pilotReport.defaultChatUnchanged ? "true" : "false"}
              </span>
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Checked run ids</div>
              {pilotReport.checkedRunIds.length > 0 ? (
                <div className="mt-1 flex flex-wrap gap-1.5">
                  {pilotReport.checkedRunIds.map(runId => (
                    <span
                      key={runId}
                      className="rounded-md border border-stone-200 bg-stone-50 px-2 py-1 font-mono text-xs text-stone-700"
                    >
                      {runId}
                    </span>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No preview runs checked.</div>
              )}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {pilotReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {pilotReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No pilot eligibility blockers returned.
                </div>
              )}
            </div>
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Promotion evidence summary</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only metadata-safe evidence recorded after reviewed promotion. It shows counts,
              run ids, and timestamps only; raw prompts and raw pilot responses are not displayed.
            </div>
          </div>
          <button
            type="button"
            onClick={handlePromotionSummaryRefresh}
            disabled={promotionSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              promotionSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={promotionSummaryChecking ? "animate-spin" : undefined}
            />
            {promotionSummaryChecking ? "Refreshing..." : "Refresh Promotion Evidence"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          This summary reads existing EvidenceStore records only. It does not run preview, promote a
          message, or write LifeModel, Memory, Proposal, Action, Observation, or tool results.
        </div>

        {promotionSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {promotionSummaryError}
          </div>
        )}

        {promotionSummary ? (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Promoted count</div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {promotionSummary.promotedCount}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">
                  Latest promotion timestamp
                </div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {promotionSummary.latestPromotionTimestamp ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">
                  Source/target mismatch blocks
                </div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {promotionSummary.sourceTargetMismatchBlockCount}
                </div>
              </div>
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">
                Recent promoted pilot run ids
              </div>
              {promotionSummary.recentPromotedPilotRunIds.length > 0 ? (
                <div className="mt-1 flex flex-wrap gap-1.5">
                  {promotionSummary.recentPromotedPilotRunIds.map(runId => (
                    <span
                      key={runId}
                      className="rounded-md border border-stone-200 bg-stone-50 px-2 py-1 font-mono text-xs text-stone-700"
                    >
                      {runId}
                    </span>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No promotion evidence recorded.</div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No promotion evidence summary loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Promotion readiness gate</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only gate based on existing promotion evidence only. Pass means ready to discuss
              the next Chat migration step; it is not automatic migration permission.
            </div>
          </div>
          <button
            type="button"
            onClick={handlePromotionReadinessCheck}
            disabled={promotionReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              promotionReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={promotionReadinessChecking ? "animate-spin" : undefined}
            />
            {promotionReadinessChecking ? "Checking..." : "Check Promotion Readiness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          Session filtering is reserved for future EvidenceStore support; this check currently reads
          the global metadata-safe promotion summary and never reads raw pilot responses or raw user
          input.
        </div>

        {promotionReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {promotionReadinessError}
          </div>
        )}

        {promotionReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  promotionReadinessReport.ready
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {promotionReadinessReport.ready ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {promotionReadinessReport.ready ? "Ready" : "Blocked"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {promotionReadinessReport.promotedCount} /{" "}
                {promotionReadinessReport.requiredPromotions} promotions
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  promotionReadinessReport.metadataSafeEvidenceReady
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                metadataSafeEvidenceReady:{" "}
                {promotionReadinessReport.metadataSafeEvidenceReady ? "true" : "false"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  promotionReadinessReport.defaultChatUnchanged
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                defaultChatUnchanged:{" "}
                {promotionReadinessReport.defaultChatUnchanged ? "true" : "false"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                Mismatch blocks: {promotionReadinessReport.sourceTargetMismatchBlockCount}
              </span>
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">
                  Latest promotion timestamp
                </div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {promotionReadinessReport.latestPromotionTimestamp ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Gate meaning</div>
                <div className="mt-1 text-xs text-stone-700">discussion only</div>
              </div>
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">
                Recent promoted pilot run ids
              </div>
              {promotionReadinessReport.recentPromotedPilotRunIds.length > 0 ? (
                <div className="mt-1 flex flex-wrap gap-1.5">
                  {promotionReadinessReport.recentPromotedPilotRunIds.map(runId => (
                    <span
                      key={runId}
                      className="rounded-md border border-stone-200 bg-stone-50 px-2 py-1 font-mono text-xs text-stone-700"
                    >
                      {runId}
                    </span>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No promotion evidence recorded.</div>
              )}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {promotionReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {promotionReadinessReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No promotion readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No promotion readiness report loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Migration plan draft</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only human review draft generated from the promotion readiness gate. It will not
              switch default Chat, does not change feature flags, and does not create migration
              evidence.
            </div>
          </div>
          <button
            type="button"
            onClick={handleMigrationDraft}
            disabled={migrationDraftChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              migrationDraftChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={migrationDraftChecking ? "animate-spin" : undefined} />
            {migrationDraftChecking ? "Drafting..." : "Draft Migration Plan"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          This command reuses W24 readiness output only. Readiness pass is not migration permission,
          and this panel cannot replace default Chat.
        </div>

        {migrationDraftError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {migrationDraftError}
          </div>
        )}

        {migrationDraft ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  migrationDraft.draftReady
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {migrationDraft.draftReady ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
                {migrationDraft.draftReady ? "Draft ready" : "Draft blocked"}
              </span>
              {migrationDraft.manualReviewRequired && (
                <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                  Manual review required
                </span>
              )}
              {migrationDraft.notAutomaticMigration && (
                <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                  Not automatic migration
                </span>
              )}
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {migrationDraft.readinessReport.promotedCount} /{" "}
                {migrationDraft.readinessReport.requiredPromotions} promotions
              </span>
            </div>

            {migrationDraft.blockingReasons.length > 0 && (
              <div>
                <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
                <div className="mt-1 space-y-1">
                  {migrationDraft.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {migrationDraft.draftReady ? (
              <div className="grid gap-3 md:grid-cols-2">
                <PlanList title="Migration scope" items={migrationDraft.migrationScope} />
                <PlanList
                  title="Required preconditions"
                  items={migrationDraft.requiredPreconditions}
                />
                <PlanList title="Rollback plan" items={migrationDraft.rollbackPlan} />
                <PlanList title="Fallback plan" items={migrationDraft.fallbackPlan} />
                <div className="md:col-span-2">
                  <PlanList title="Test plan" items={migrationDraft.testPlan} />
                </div>
              </div>
            ) : (
              <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-xs leading-5 text-red-700">
                No executable migration plan is generated until promotion readiness passes.
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No migration plan draft loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Migration Review Decision</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Approval only allows next-stage implementation discussion. It is not Chat migration,
              does not replace default Chat, and records metadata-safe decision evidence only.
            </div>
          </div>
          <button
            type="button"
            onClick={handleReviewDecisionSummaryRefresh}
            disabled={reviewDecisionSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              reviewDecisionSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={reviewDecisionSummaryChecking ? "animate-spin" : undefined}
            />
            {reviewDecisionSummaryChecking ? "Refreshing..." : "Refresh Decision Summary"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          The record command first rechecks the W25 draft. Reviewer notes are sent for backend
          checksum categorization only; raw note text is not evidence.
        </div>

        {reviewDecisionSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {reviewDecisionSummaryError}
          </div>
        )}

        {reviewDecisionSummary ? (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest decision</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {reviewDecisionSummary.latestDecision?.decisionKind ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Approved count</div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {reviewDecisionSummary.approvedCount}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Rework/reject count</div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {reviewDecisionSummary.reworkRejectCount}
                </div>
              </div>
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest timestamp</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {reviewDecisionSummary.latestTimestamp ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Draft hash</div>
                <div className="mt-1 break-all font-mono text-xs text-stone-900">
                  {reviewDecisionSummary.latestDecision?.draftHash ?? "none"}
                </div>
              </div>
            </div>

            {reviewDecisionSummary.blockingReasons.length > 0 && (
              <div>
                <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
                <div className="mt-1 space-y-1">
                  {reviewDecisionSummary.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No review decision summary loaded.</div>
        )}

        <div className="mt-4 space-y-3">
          {migrationDraft?.draftReady ? (
            <>
              <label className="block">
                <span className="text-xs font-medium text-stone-700">Reviewer note</span>
                <textarea
                  value={reviewerNote}
                  onChange={event => setReviewerNote(event.target.value)}
                  rows={2}
                  className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                  placeholder="Optional note. Stored as length, checksum, and category only."
                />
              </label>

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => handleRecordReviewDecision("approve")}
                  disabled={reviewDecisionRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    reviewDecisionRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-emerald-700 text-white hover:bg-emerald-800"
                  )}
                >
                  <CheckCircle2 size={13} />
                  Approve Review Decision
                </button>
                <button
                  type="button"
                  onClick={() => handleRecordReviewDecision("reject")}
                  disabled={reviewDecisionRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    reviewDecisionRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-red-700 text-white hover:bg-red-800"
                  )}
                >
                  <XCircle size={13} />
                  Reject Review Decision
                </button>
                <button
                  type="button"
                  onClick={() => handleRecordReviewDecision("request_rework")}
                  disabled={reviewDecisionRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    reviewDecisionRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-stone-900 text-amber-50 hover:bg-stone-800"
                  )}
                >
                  <RefreshCw size={13} />
                  Request Rework Review Decision
                </button>
              </div>
            </>
          ) : migrationDraft ? (
            <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-xs leading-5 text-red-700">
              <div>Review decision recording is blocked until draftReady=true.</div>
              {migrationDraft.blockingReasons.length > 0 && (
                <div className="mt-1 space-y-1">
                  {migrationDraft.blockingReasons.map(reason => (
                    <div key={reason}>Review decision blocker: {reason}</div>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
              Load a ready migration plan draft before recording a review decision.
            </div>
          )}

          {reviewDecisionError && (
            <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
              {reviewDecisionError}
            </div>
          )}

          {reviewDecisionResult && (
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-xs leading-5",
                reviewDecisionResult.recorded
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              <div className="font-medium">
                {reviewDecisionResult.recorded ? "Decision recorded" : "Decision blocked"}
              </div>
              <div className="mt-1">
                {reviewDecisionResult.decisionKind} · draftReady:{" "}
                {reviewDecisionResult.draftReady ? "true" : "false"} ·{" "}
                {reviewDecisionResult.evidenceId ?? "no evidence"}
              </div>
              {reviewDecisionResult.blockingReasons.length > 0 && (
                <div className="mt-1 space-y-1">
                  {reviewDecisionResult.blockingReasons.map(reason => (
                    <div key={reason}>{reason}</div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Implementation Gate</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only gate for entering controlled Chat migration implementation discussion. It
              reads W24 readiness, the current W25 draft hash, and latest W26 metadata-safe review
              decision evidence; current Send remains untouched.
            </div>
          </div>
          <button
            type="button"
            onClick={handleImplementationGateCheck}
            disabled={implementationGateChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              implementationGateChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={implementationGateChecking ? "animate-spin" : undefined}
            />
            {implementationGateChecking ? "Checking..." : "Check Implementation Gate"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
          Approval here only means implementation development can be discussed. Even when eligible,
          default Chat will not switch.
        </div>

        {implementationGateError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {implementationGateError}
          </div>
        )}

        {implementationGateReport ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  implementationGateReport.implementationEligible
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {implementationGateReport.implementationEligible ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {implementationGateReport.implementationEligible ? "Eligible" : "Blocked"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {implementationGateReport.readinessReport.promotedCount} /{" "}
                {implementationGateReport.readinessReport.requiredPromotions} promotions
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  implementationGateReport.draftHashMatched
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                draftHashMatched: {implementationGateReport.draftHashMatched ? "true" : "false"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  implementationGateReport.approvedAfterLatestDraft
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                approvedAfterLatestDraft:{" "}
                {implementationGateReport.approvedAfterLatestDraft ? "true" : "false"}
              </span>
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest decision</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {implementationGateReport.latestDecision?.decisionKind ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Readiness</div>
                <div className="mt-1 text-xs text-stone-700">
                  {implementationGateReport.readinessReport.ready ? "ready" : "blocked"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Mismatch blocks</div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {implementationGateReport.readinessReport.sourceTargetMismatchBlockCount}
                </div>
              </div>
            </div>

            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
              Even when eligible, default Chat will not switch.
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {implementationGateReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {implementationGateReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No implementation gate blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No implementation gate report loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Shadow Run</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Non-default controlled migration shadow run. It first checks the implementation gate,
              then runs a bounded metadata-safe runtime probe with writes disabled. It does not save
              to Chat, does not change feature flags, and cannot switch default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleShadowRun}
            disabled={shadowRunChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              shadowRunChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <Play size={13} />
            {shadowRunChecking ? "Running..." : "Run Shadow Run"}
          </button>
        </div>

        <div className="mt-3 grid gap-3 md:grid-cols-[1fr_auto]">
          <label className="block">
            <span className="text-xs font-medium text-stone-700">Shadow prompt descriptor</span>
            <select
              value={shadowDescriptor}
              onChange={event =>
                setShadowDescriptor(
                  event.target.value as ControlledChatMigrationShadowRunDescriptor
                )
              }
              className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
            >
              <option value="default_readiness_probe">default_readiness_probe</option>
              <option value="planning_readiness_probe">planning_readiness_probe</option>
              <option value="sensitive_local_only_probe">sensitive_local_only_probe</option>
            </select>
          </label>
          <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
            Uses a bounded descriptor instead of raw prompt text.
          </div>
        </div>

        <div className="mt-3 rounded-md border border-emerald-100 bg-emerald-50 px-3 py-2 text-xs leading-5 text-emerald-900">
          Not saved to Chat history and does not switch default Chat.
        </div>

        {shadowRunError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {shadowRunError}
          </div>
        )}

        {shadowRunResult ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  shadowRunResult.shadowRunReady
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {shadowRunResult.shadowRunReady ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {shadowRunResult.shadowRunReady ? "Shadow ready" : "Shadow blocked"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  shadowRunResult.implementationGateReport.implementationEligible
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                implementationGateEligible:{" "}
                {shadowRunResult.implementationGateReport.implementationEligible ? "true" : "false"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {shadowRunResult.shadowRunId ?? "no shadow audit"}
              </span>
            </div>

            <div className="grid gap-2 text-xs md:grid-cols-2">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-stone-700">
                Strategy: {shadowRunResult.strategyKind}
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-stone-700">
                Payload: {shadowRunResult.payloadKind}
              </div>
            </div>

            {safeSummaryEntries(shadowRunResult.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(shadowRunResult.metadataSafeSummary).map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                  >
                    {key}: {value}
                  </span>
                ))}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Warnings</div>
              {shadowRunResult.warnings.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {shadowRunResult.warnings.map(warning => (
                    <div
                      key={warning}
                      className="rounded-md border border-amber-100 bg-amber-50 px-2 py-1 text-xs text-amber-800"
                    >
                      {warning}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No shadow warnings returned.</div>
              )}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {shadowRunResult.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {shadowRunResult.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No shadow run blockers returned.</div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No shadow run loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Shadow Review</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Manual review evidence for ready shadow runs. Evidence stores only the shadow run id,
              decision, note checksum metadata, readiness digest, and timestamp.
            </div>
          </div>
          <button
            type="button"
            onClick={handleShadowReviewSummaryRefresh}
            disabled={shadowReviewSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              shadowReviewSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={shadowReviewSummaryChecking ? "animate-spin" : undefined}
            />
            {shadowReviewSummaryChecking ? "Refreshing..." : "Refresh Shadow Review Summary"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This area never starts a shadow run and never promotes output to Chat.
        </div>

        {shadowReviewSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {shadowReviewSummaryError}
          </div>
        )}

        {shadowReviewSummary ? (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest shadow decision</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {shadowReviewSummary.latestDecision?.decisionKind ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Approved shadow reviews</div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {shadowReviewSummary.approvedCount}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">
                  Shadow rework/reject count
                </div>
                <div className="mt-1 text-sm font-semibold text-stone-900">
                  {shadowReviewSummary.reworkRejectCount}
                </div>
              </div>
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Shadow run id</div>
                <div className="mt-1 break-all font-mono text-xs text-stone-900">
                  {shadowReviewSummary.latestDecision?.shadowRunId ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Readiness summary digest</div>
                <div className="mt-1 break-all font-mono text-xs text-stone-900">
                  {shadowReviewSummary.latestDecision?.readinessSummaryDigest ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest timestamp</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {shadowReviewSummary.latestTimestamp ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Reviewer note category</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {shadowReviewSummary.latestDecision?.reviewerNoteCategory ?? "none"}
                </div>
              </div>
            </div>

            {shadowReviewSummary.blockingReasons.length > 0 && (
              <div>
                <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
                <div className="mt-1 space-y-1">
                  {shadowReviewSummary.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No shadow review summary loaded.</div>
        )}

        <div className="mt-4 space-y-3">
          {shadowRunResult?.shadowRunReady && shadowRunResult.shadowRunId ? (
            <>
              <label className="block">
                <span className="text-xs font-medium text-stone-700">Shadow reviewer note</span>
                <textarea
                  value={shadowReviewNote}
                  onChange={event => setShadowReviewNote(event.target.value)}
                  rows={2}
                  className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                  placeholder="Optional note. Stored as checksum metadata only."
                />
              </label>

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => handleRecordShadowReviewDecision("approve")}
                  disabled={shadowReviewRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    shadowReviewRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-emerald-700 text-white hover:bg-emerald-800"
                  )}
                >
                  <CheckCircle2 size={13} />
                  Approve Shadow Review
                </button>
                <button
                  type="button"
                  onClick={() => handleRecordShadowReviewDecision("reject")}
                  disabled={shadowReviewRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    shadowReviewRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-red-700 text-white hover:bg-red-800"
                  )}
                >
                  <XCircle size={13} />
                  Reject Shadow Review
                </button>
                <button
                  type="button"
                  onClick={() => handleRecordShadowReviewDecision("request_rework")}
                  disabled={shadowReviewRecording}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    shadowReviewRecording
                      ? "bg-stone-100 text-stone-400"
                      : "bg-stone-900 text-amber-50 hover:bg-stone-800"
                  )}
                >
                  <RefreshCw size={13} />
                  Request Rework Shadow Review
                </button>
              </div>
            </>
          ) : (
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
              A ready shadow run is required before recording shadow review evidence.
            </div>
          )}

          {shadowReviewError && (
            <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
              {shadowReviewError}
            </div>
          )}

          {shadowReviewResult && (
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-xs leading-5",
                shadowReviewResult.recorded
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              <div className="font-medium">
                {shadowReviewResult.recorded ? "Shadow review recorded" : "Shadow review blocked"}
              </div>
              <div className="mt-1">
                {shadowReviewResult.decisionKind} · {shadowReviewResult.shadowRunId} ·{" "}
                {shadowReviewResult.evidenceId ?? "no evidence"}
              </div>
              <div className="mt-1 break-all font-mono">
                {shadowReviewResult.readinessSummaryDigest}
              </div>
              {shadowReviewResult.blockingReasons.length > 0 && (
                <div className="mt-1 space-y-1">
                  {shadowReviewResult.blockingReasons.map(reason => (
                    <div key={reason}>{reason}</div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Cutover Readiness</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only cutover planning readiness check. It verifies W27, latest W29 approval, and
              the approved shadow run audit; it does not start runtime, write evidence, save Chat,
              or migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleCutoverReadinessCheck}
            disabled={cutoverReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              cutoverReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={cutoverReadinessChecking ? "animate-spin" : undefined}
            />
            {cutoverReadinessChecking ? "Checking..." : "Check Cutover Readiness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          W30 is cutover planning readiness only. A pass means the team can discuss default Chat
          migration implementation; it is not a default Chat migration or feature flag change.
        </div>

        {cutoverReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {cutoverReadinessError}
          </div>
        )}

        {cutoverReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  cutoverReadinessReport.cutoverPlanningEligible
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {cutoverReadinessReport.cutoverPlanningEligible ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {cutoverReadinessReport.cutoverPlanningEligible
                  ? "Cutover Planning Eligible"
                  : "Cutover Blocked"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  cutoverReadinessReport.requiredEvidenceReady
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                requiredEvidenceReady:{" "}
                {cutoverReadinessReport.requiredEvidenceReady ? "true" : "false"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  cutoverReadinessReport.defaultChatUnchanged
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                defaultChatUnchanged:{" "}
                {cutoverReadinessReport.defaultChatUnchanged ? "true" : "false"}
              </span>
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Implementation gate</div>
                <div className="mt-1 text-xs text-stone-700">
                  {cutoverReadinessReport.implementationGateReport.implementationEligible
                    ? "eligible"
                    : "blocked"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest shadow decision</div>
                <div className="mt-1 font-mono text-xs text-stone-900">
                  {cutoverReadinessReport.latestShadowReviewDecision?.decisionKind ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Verified shadow run</div>
                <div className="mt-1 break-all font-mono text-xs text-stone-900">
                  {cutoverReadinessReport.verifiedShadowRunId ?? "none"}
                </div>
              </div>
            </div>

            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Readiness summary digest</div>
              <div className="mt-1 break-all font-mono text-xs text-stone-900">
                {cutoverReadinessReport.readinessSummaryDigest ?? "none"}
              </div>
            </div>

            {safeSummaryEntries(cutoverReadinessReport.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(cutoverReadinessReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {cutoverReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {cutoverReadinessReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No cutover readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No cutover readiness report loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Cutover Candidate</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Non-default controlled Chat cutover candidate. It first checks W30 readiness, then
              runs one write-disabled runtime probe for contract-shape validation only. It cannot
              save to Chat, promote output, or switch default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleCutoverCandidateRun}
            disabled={cutoverCandidateChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              cutoverCandidateChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <Play size={13} />
            {cutoverCandidateChecking ? "Running..." : "Run Cutover Candidate"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          Uses a bounded descriptor instead of raw prompt text. The backend forces allowWrites=false
          and maxToolCalls=0, and only a metadata-safe AgentRun audit may be created.
        </div>

        {cutoverCandidateError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {cutoverCandidateError}
          </div>
        )}

        {cutoverCandidateResult ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  cutoverCandidateResult.candidateReady
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {cutoverCandidateResult.candidateReady ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {cutoverCandidateResult.candidateReady ? "Candidate ready" : "Candidate blocked"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  cutoverCandidateResult.contractShape === "send_message_compatible"
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                {cutoverCandidateResult.contractShape}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 font-mono text-xs font-medium text-stone-700">
                {cutoverCandidateResult.candidateRunId ?? "no candidate audit"}
              </span>
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Output preview</div>
                <div className="mt-1 text-xs text-stone-700">
                  {cutoverCandidateResult.outputPreview ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">User output</div>
                <div className="mt-1 text-xs text-stone-700">
                  {cutoverCandidateResult.userOutput ? "returned to UI only" : "none"}
                </div>
              </div>
            </div>

            {safeSummaryEntries(cutoverCandidateResult.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(cutoverCandidateResult.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Warnings</div>
              {cutoverCandidateResult.warnings.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {cutoverCandidateResult.warnings.map(warning => (
                    <div
                      key={warning}
                      className="rounded-md border border-amber-100 bg-amber-50 px-2 py-1 text-xs text-amber-800"
                    >
                      {warning}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">No cutover candidate warnings.</div>
              )}
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {cutoverCandidateResult.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {cutoverCandidateResult.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No cutover candidate blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">No cutover candidate result loaded.</div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Cutover Candidate Review</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Manual candidate review evidence for W32. Decisions are recorded as metadata-only
              evidence and do not save candidate output to Chat, promote output, migrate default
              Chat, or change feature flags.
            </div>
          </div>
          <button
            type="button"
            onClick={handleCutoverCandidateReviewSummaryRefresh}
            disabled={cutoverCandidateReviewSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              cutoverCandidateReviewSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={cutoverCandidateReviewSummaryChecking ? "animate-spin" : undefined}
            />
            {cutoverCandidateReviewSummaryChecking
              ? "Refreshing..."
              : "Refresh Candidate Review Summary"}
          </button>
        </div>

        {cutoverCandidateReviewSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {cutoverCandidateReviewSummaryError}
          </div>
        )}

        {cutoverCandidateReviewSummary ? (
          <div className="mt-4 grid gap-2 md:grid-cols-2">
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Latest decision</div>
              <div className="mt-1 text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestDecision?.decisionKind ?? "none"}
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Counts</div>
              <div className="mt-1 flex flex-wrap gap-2 text-xs text-stone-800">
                <span>Approved: {cutoverCandidateReviewSummary.approvedCount}</span>
                <span>Rework / Reject: {cutoverCandidateReviewSummary.reworkRejectCount}</span>
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Candidate run</div>
              <div className="mt-1 font-mono text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestDecision?.candidateRunId ?? "none"}
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Contract shape</div>
              <div className="mt-1 text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestDecision?.contractShape ?? "none"}
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Summary digest</div>
              <div className="mt-1 break-all font-mono text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestDecision?.candidateSummaryDigest ?? "none"}
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Latest timestamp</div>
              <div className="mt-1 text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestTimestamp ?? "none"}
              </div>
            </div>
            <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
              <div className="text-[10px] uppercase text-stone-400">Reviewer note category</div>
              <div className="mt-1 text-xs text-stone-800">
                {cutoverCandidateReviewSummary.latestDecision?.reviewerNoteCategory ?? "none"}
              </div>
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No cutover candidate review summary loaded.
          </div>
        )}

        {cutoverCandidateReviewSummary?.blockingReasons.length ? (
          <div className="mt-3 space-y-1">
            {cutoverCandidateReviewSummary.blockingReasons.map(reason => (
              <div
                key={reason}
                className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
              >
                {reason}
              </div>
            ))}
          </div>
        ) : null}

        <div className="mt-4 border-t border-stone-100 pt-4">
          <label className="block">
            <span className="text-xs font-medium text-stone-700">candidate review note</span>
            <textarea
              value={cutoverCandidateReviewNote}
              onChange={event => setCutoverCandidateReviewNote(event.target.value)}
              rows={3}
              className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
              placeholder="Optional note; backend stores only length, checksum, and category."
            />
          </label>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => handleRecordCutoverCandidateReviewDecision("approve")}
              disabled={cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId}
              className={classNames(
                "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId
                  ? "bg-stone-100 text-stone-400"
                  : "bg-emerald-700 text-white hover:bg-emerald-800"
              )}
            >
              <CheckCircle2 size={13} />
              Approve Candidate
            </button>
            <button
              type="button"
              onClick={() => handleRecordCutoverCandidateReviewDecision("reject")}
              disabled={cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId}
              className={classNames(
                "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId
                  ? "bg-stone-100 text-stone-400"
                  : "bg-red-700 text-white hover:bg-red-800"
              )}
            >
              <XCircle size={13} />
              Reject Candidate
            </button>
            <button
              type="button"
              onClick={() => handleRecordCutoverCandidateReviewDecision("request_rework")}
              disabled={cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId}
              className={classNames(
                "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                cutoverCandidateReviewRecording || !cutoverCandidateResult?.candidateRunId
                  ? "bg-stone-100 text-stone-400"
                  : "bg-amber-700 text-white hover:bg-amber-800"
              )}
            >
              <ShieldCheck size={13} />
              Request Candidate Rework
            </button>
          </div>
          <div className="mt-2 text-xs text-stone-500">
            {cutoverCandidateResult?.candidateRunId
              ? `Review target: ${cutoverCandidateResult.candidateRunId}`
              : "Run a cutover candidate before recording review evidence."}
          </div>
        </div>

        {cutoverCandidateReviewError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {cutoverCandidateReviewError}
          </div>
        )}

        {cutoverCandidateReviewResult && (
          <div className="mt-4 rounded-md border border-stone-200 bg-stone-50 px-3 py-2">
            <div
              className={classNames(
                "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                cutoverCandidateReviewResult.recorded
                  ? "bg-emerald-100 text-emerald-800"
                  : "bg-red-100 text-red-700"
              )}
            >
              {cutoverCandidateReviewResult.recorded ? (
                <CheckCircle2 size={13} />
              ) : (
                <XCircle size={13} />
              )}
              {cutoverCandidateReviewResult.recorded
                ? "Candidate review recorded"
                : "Candidate review blocked"}
            </div>
            <div className="mt-2 text-xs text-stone-700">
              {cutoverCandidateReviewResult.decisionKind} ·{" "}
              {cutoverCandidateReviewResult.candidateRunId} ·{" "}
              {cutoverCandidateReviewResult.evidenceId ?? "no evidence"}
            </div>
            <div className="mt-1 break-all font-mono text-xs text-stone-600">
              {cutoverCandidateReviewResult.candidateSummaryDigest}
            </div>
            {cutoverCandidateReviewResult.blockingReasons.length > 0 && (
              <div className="mt-2 space-y-1">
                {cutoverCandidateReviewResult.blockingReasons.map(reason => (
                  <div
                    key={reason}
                    className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                  >
                    {reason}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Candidate Promotion Readiness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only gate for W33. It checks W30 cutover readiness, metadata-safe candidate
              review approvals, current candidate AgentRun safety, and default Chat isolation. It
              does not run runtime or switch default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleCandidatePromotionReadinessRefresh}
            disabled={candidatePromotionReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              candidatePromotionReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={candidatePromotionReadinessChecking ? "animate-spin" : undefined}
            />
            {candidatePromotionReadinessChecking
              ? "Refreshing..."
              : "Refresh Candidate Promotion Readiness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          Approval evidence is still only permission to discuss migration implementation. The
          default Send path remains unchanged, and this panel has no Chat migration action.
        </div>

        {candidatePromotionReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {candidatePromotionReadinessError}
          </div>
        )}

        {candidatePromotionReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  candidatePromotionReadinessReport.ready
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {candidatePromotionReadinessReport.ready ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {candidatePromotionReadinessReport.ready ? "Promotion ready" : "Promotion blocked"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                {candidatePromotionReadinessReport.approvedCandidateCount} /{" "}
                {candidatePromotionReadinessReport.requiredApprovedCandidates} approved candidates
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  candidatePromotionReadinessReport.cutoverReadinessEligible
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                cutoverReadinessEligible:{" "}
                {candidatePromotionReadinessReport.cutoverReadinessEligible ? "true" : "false"}
              </span>
              <span
                className={classNames(
                  "rounded-full px-2.5 py-1 text-xs font-medium",
                  candidatePromotionReadinessReport.defaultChatUnchanged
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-red-50 text-red-700"
                )}
              >
                defaultChatUnchanged:{" "}
                {candidatePromotionReadinessReport.defaultChatUnchanged ? "true" : "false"}
              </span>
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Latest decision</div>
                <div className="mt-1 text-xs text-stone-800">
                  Latest decision:{" "}
                  {candidatePromotionReadinessReport.latestDecision?.decisionKind ?? "none"}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Checked at</div>
                <div className="mt-1 font-mono text-xs text-stone-800">
                  {candidatePromotionReadinessReport.checkedAt}
                </div>
              </div>
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2">
                <div className="text-[10px] uppercase text-stone-400">Gate meaning</div>
                <div className="mt-1 text-xs text-stone-700">discussion only</div>
              </div>
            </div>

            <div>
              <div className="text-xs font-medium text-stone-700">Approved candidates</div>
              {candidatePromotionReadinessReport.approvedCandidates.length > 0 ? (
                <div className="mt-1 space-y-2">
                  {candidatePromotionReadinessReport.approvedCandidates.map(candidate => (
                    <div
                      key={`${candidate.evidenceId}-${candidate.candidateRunId}`}
                      className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700"
                    >
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-mono text-stone-900">{candidate.candidateRunId}</span>
                        <span
                          className={classNames(
                            "rounded-full px-2 py-0.5 text-[11px] font-medium",
                            candidate.ready
                              ? "bg-emerald-50 text-emerald-700"
                              : "bg-red-50 text-red-700"
                          )}
                        >
                          {candidate.ready ? "ready" : "blocked"}
                        </span>
                        <span>{candidate.contractShape}</span>
                      </div>
                      <div className="mt-1 break-all font-mono text-stone-500">
                        {candidate.runReadinessDigest}
                      </div>
                      {candidate.blockingReasons.length > 0 && (
                        <div className="mt-2 space-y-1">
                          {candidate.blockingReasons.map(reason => (
                            <div
                              key={reason}
                              className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-red-700"
                            >
                              {reason}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No approved candidate review evidence loaded.
                </div>
              )}
            </div>

            {safeSummaryEntries(candidatePromotionReadinessReport.metadataSafeSummary).length >
              0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(candidatePromotionReadinessReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {candidatePromotionReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {candidatePromotionReadinessReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No candidate promotion readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No candidate promotion readiness report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Activation Plan
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W35 draft for future adapter activation review. It combines W33 candidate
              promotion readiness with W34 default Chat boundary status and does not provide any
              switch, migrate, or enable action.
            </div>
          </div>
          <button
            type="button"
            onClick={handleActivationPlanRefresh}
            disabled={activationPlanChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              activationPlanChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={activationPlanChecking ? "animate-spin" : undefined} />
            {activationPlanChecking ? "Refreshing..." : "Refresh Activation Plan Draft"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This panel is human-review-only. Default Chat remains on the legacy stream path, and any
          adapter activation would require separate implementation work.
        </div>

        {activationPlanError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {activationPlanError}
          </div>
        )}

        {activationPlanDraft ? (
          <div className="mt-4 space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={classNames(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium",
                  activationPlanDraft.draftReady
                    ? "bg-emerald-100 text-emerald-800"
                    : "bg-red-100 text-red-700"
                )}
              >
                {activationPlanDraft.draftReady ? (
                  <CheckCircle2 size={13} />
                ) : (
                  <XCircle size={13} />
                )}
                {activationPlanDraft.draftReady
                  ? "Activation draft ready"
                  : "Activation draft blocked"}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                currentMode: {activationPlanDraft.runtimeBoundaryStatus.currentMode}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                automaticMigrationEnabled:{" "}
                {String(activationPlanDraft.runtimeBoundaryStatus.automaticMigrationEnabled)}
              </span>
              <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">
                W33 ready: {String(activationPlanDraft.candidatePromotionReadinessReport.ready)}
              </span>
            </div>

            {safeSummaryEntries(activationPlanDraft.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(activationPlanDraft.metadataSafeSummary).map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                  >
                    {key}: {value}
                  </span>
                ))}
              </div>
            )}

            {activationPlanDraft.draftReady && (
              <div className="grid gap-3 md:grid-cols-2">
                <PlanList title="Activation Scope" items={activationPlanDraft.activationScope} />
                <PlanList
                  title="Required Preconditions"
                  items={activationPlanDraft.requiredPreconditions}
                />
                <PlanList
                  title="Adapter Contract Checks"
                  items={activationPlanDraft.adapterContractChecks}
                />
                <PlanList title="Fallback Plan" items={activationPlanDraft.fallbackPlan} />
                <PlanList title="Rollback Plan" items={activationPlanDraft.rollbackPlan} />
                <PlanList
                  title="Observability Plan"
                  items={activationPlanDraft.observabilityPlan}
                />
                <PlanList title="Test Plan" items={activationPlanDraft.testPlan} />
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {activationPlanDraft.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {activationPlanDraft.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No activation plan blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter activation draft loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Activation Review Decision
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Explicit W36 human review evidence for the W35 activation plan draft. It records
              approve, reject, or request rework metadata only; it does not activate or migrate
              default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleActivationReviewSummaryRefresh}
            disabled={activationReviewSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              activationReviewSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={activationReviewSummaryChecking ? "animate-spin" : undefined}
            />
            {activationReviewSummaryChecking
              ? "Refreshing..."
              : "Refresh Activation Review Summary"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          Reviewer notes are never stored as raw text. Only checksum, length, and bounded category
          metadata are persisted in EvidenceStore.
        </div>

        <div className="mt-4 space-y-3">
          <label className="block">
            <span className="text-xs font-medium text-stone-700">Activation reviewer note</span>
            <textarea
              value={activationReviewNote}
              onChange={event => setActivationReviewNote(event.target.value)}
              rows={3}
              className="mt-1 w-full rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-800 outline-none focus:border-stone-500"
              placeholder="Optional private note; only metadata is stored."
            />
          </label>

          <div className="flex flex-wrap gap-2">
            {[
              ["approve", "Approve"],
              ["reject", "Reject"],
              ["request_rework", "Request Rework"],
            ].map(([decisionKind, label]) => (
              <button
                key={decisionKind}
                type="button"
                onClick={() =>
                  handleRecordActivationReviewDecision(
                    decisionKind as DefaultChatAdapterActivationReviewDecisionKind
                  )
                }
                disabled={activationReviewRecording}
                className={classNames(
                  "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                  activationReviewRecording
                    ? "bg-stone-100 text-stone-400"
                    : decisionKind === "approve"
                      ? "bg-emerald-700 text-white hover:bg-emerald-800"
                      : "bg-stone-900 text-amber-50 hover:bg-stone-800"
                )}
              >
                {decisionKind === "approve" ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
                {label}
              </button>
            ))}
          </div>
        </div>

        {activationReviewError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {activationReviewError}
          </div>
        )}

        {activationReviewSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {activationReviewSummaryError}
          </div>
        )}

        {activationReviewResult && (
          <div className="mt-4 space-y-2 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
            <div className="font-medium text-stone-900">
              {activationReviewResult.recorded
                ? "Activation review decision recorded"
                : "Activation review decision blocked"}
            </div>
            <div>decisionKind: {activationReviewResult.decisionKind}</div>
            <div>draftReady: {String(activationReviewResult.draftReady)}</div>
            <div className="break-all">
              activationPlanDigest: {activationReviewResult.activationPlanDigest}
            </div>
            {activationReviewResult.evidenceId && (
              <div>evidenceId: {activationReviewResult.evidenceId}</div>
            )}
            {activationReviewResult.blockingReasons.length > 0 && (
              <div className="space-y-1">
                {activationReviewResult.blockingReasons.map(reason => (
                  <div
                    key={reason}
                    className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-red-700"
                  >
                    {reason}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activationReviewSummary ? (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["approvedCount", String(activationReviewSummary.approvedCount)],
                ["rejectOrReworkCount", String(activationReviewSummary.rejectOrReworkCount)],
                ["latestTimestamp", activationReviewSummary.latestTimestamp ?? "none"],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {activationReviewSummary.latestDecision && (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                <div className="font-medium text-stone-900">Latest decision</div>
                <div className="mt-1">
                  decisionKind: {activationReviewSummary.latestDecision.decisionKind}
                </div>
                <div>draftReady: {String(activationReviewSummary.latestDecision.draftReady)}</div>
                <div>
                  candidatePromotionReady:{" "}
                  {String(activationReviewSummary.latestDecision.candidatePromotionReady)}
                </div>
                <div>currentMode: {activationReviewSummary.latestDecision.currentMode}</div>
                <div>
                  automaticMigrationEnabled:{" "}
                  {String(activationReviewSummary.latestDecision.automaticMigrationEnabled)}
                </div>
                <div className="break-all">
                  activationPlanDigest:{" "}
                  {activationReviewSummary.latestDecision.activationPlanDigest}
                </div>
                <div>
                  reviewerNoteCategory:{" "}
                  {activationReviewSummary.latestDecision.reviewerNoteCategory}
                </div>
                <div>
                  reviewerNoteLength: {activationReviewSummary.latestDecision.reviewerNoteLength}
                </div>
              </div>
            )}

            {safeSummaryEntries(activationReviewSummary.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(activationReviewSummary.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {activationReviewSummary.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {activationReviewSummary.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No activation review blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter activation review summary loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Activation Implementation Gate
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W37 gate over the current W35 activation plan draft and W36 approval
              evidence. Eligible means implementation discussion can continue; it does not activate
              or migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleActivationImplementationGateCheck}
            disabled={activationImplementationGateChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              activationImplementationGateChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={activationImplementationGateChecking ? "animate-spin" : undefined}
            />
            {activationImplementationGateChecking
              ? "Checking..."
              : "Check Activation Implementation Gate"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This gate reads existing metadata-safe evidence only. It does not write Evidence, run
          runtime, call tools, create Chat messages, or change the default Chat path.
        </div>

        {activationImplementationGateError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {activationImplementationGateError}
          </div>
        )}

        {activationImplementationGateReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                activationImplementationGateReport.implementationGateEligible
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {activationImplementationGateReport.implementationGateEligible
                ? "Activation implementation gate eligible"
                : "Activation implementation gate blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["draftReady", String(activationImplementationGateReport.draftReady)],
                ["currentMode", activationImplementationGateReport.currentMode],
                [
                  "automaticMigrationEnabled",
                  String(activationImplementationGateReport.automaticMigrationEnabled),
                ],
                [
                  "defaultChatUnchanged",
                  String(activationImplementationGateReport.defaultChatUnchanged),
                ],
                [
                  "activationPlanDigestMatched",
                  String(activationImplementationGateReport.activationPlanDigestMatched),
                ],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            <div className="break-all rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700">
              currentActivationPlanDigest:{" "}
              {activationImplementationGateReport.currentActivationPlanDigest}
            </div>

            {activationImplementationGateReport.latestDecision && (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                <div className="font-medium text-stone-900">Latest activation review decision</div>
                <div className="mt-1">
                  decisionKind: {activationImplementationGateReport.latestDecision.decisionKind}
                </div>
                <div>
                  draftReady: {String(activationImplementationGateReport.latestDecision.draftReady)}
                </div>
                <div>
                  candidatePromotionReady:{" "}
                  {String(
                    activationImplementationGateReport.latestDecision.candidatePromotionReady
                  )}
                </div>
                <div>
                  currentMode: {activationImplementationGateReport.latestDecision.currentMode}
                </div>
                <div>
                  automaticMigrationEnabled:{" "}
                  {String(
                    activationImplementationGateReport.latestDecision.automaticMigrationEnabled
                  )}
                </div>
                <div className="break-all">
                  activationPlanDigest:{" "}
                  {activationImplementationGateReport.latestDecision.activationPlanDigest}
                </div>
              </div>
            )}

            {safeSummaryEntries(activationImplementationGateReport.metadataSafeSummary).length >
              0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(activationImplementationGateReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {activationImplementationGateReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {activationImplementationGateReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No activation implementation gate blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter activation implementation gate report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Routing Status
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W38 scaffold status. It verifies the adapter boundary is present but still
              disabled, with both default Send and streaming pinned to the legacy path.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterRoutingRefresh}
            disabled={adapterRoutingChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterRoutingChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={adapterRoutingChecking ? "animate-spin" : undefined} />
            {adapterRoutingChecking ? "Refreshing..." : "Refresh Adapter Routing Status"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This status command only reads the W37 implementation gate. It does not create AgentRuns,
          Evidence, Chat messages, proposals, LifeModel patches, memory writes, MCP audit rows, or
          model calls.
        </div>

        {adapterRoutingError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterRoutingError}
          </div>
        )}

        {adapterRoutingStatus ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterRoutingStatus.controlledAdapterEnabled
                  ? "border-red-100 bg-red-50 text-red-700"
                  : "border-emerald-100 bg-emerald-50 text-emerald-800"
              )}
            >
              {adapterRoutingStatus.controlledAdapterEnabled
                ? "Controlled adapter enabled"
                : "Controlled adapter disabled"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["currentMode", adapterRoutingStatus.currentMode],
                ["adapterScaffoldPresent", String(adapterRoutingStatus.adapterScaffoldPresent)],
                ["controlledAdapterEnabled", String(adapterRoutingStatus.controlledAdapterEnabled)],
                ["defaultSendPath", adapterRoutingStatus.defaultSendPath],
                ["startStreamPath", adapterRoutingStatus.startStreamPath],
                [
                  "activationImplementationGateEligible",
                  String(adapterRoutingStatus.activationImplementationGateEligible),
                ],
                [
                  "requiresSeparateCutoverImplementation",
                  String(adapterRoutingStatus.requiresSeparateCutoverImplementation),
                ],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(adapterRoutingStatus.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterRoutingStatus.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {adapterRoutingStatus.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterRoutingStatus.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No adapter routing blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter routing status loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Contract Harness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W39 contract harness over W38 routing status. It checks that the future
              adapter contract still maps both send paths to legacy stream while the controlled
              adapter remains disabled.
            </div>
          </div>
          <button
            type="button"
            onClick={handleContractHarnessCheck}
            disabled={contractHarnessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              contractHarnessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw size={13} className={contractHarnessChecking ? "animate-spin" : undefined} />
            {contractHarnessChecking ? "Checking..." : "Check Adapter Contract Harness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          The harness calls only the routing status command. It does not route Chat traffic, run
          runtime, call tools or models, create evidence, or persist any transcript.
        </div>

        {contractHarnessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {contractHarnessError}
          </div>
        )}

        {contractHarnessReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                contractHarnessReport.contractHarnessReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {contractHarnessReport.contractHarnessReady
                ? "Adapter contract harness ready"
                : "Adapter contract harness blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["contractShape", contractHarnessReport.contractShape],
                ["adapterDisabled", String(contractHarnessReport.adapterDisabled)],
                [
                  "activationImplementationGateEligible",
                  String(contractHarnessReport.activationImplementationGateEligible),
                ],
                ["currentMode", contractHarnessReport.routingStatus.currentMode],
                [
                  "controlledAdapterEnabled",
                  String(contractHarnessReport.routingStatus.controlledAdapterEnabled),
                ],
                ["defaultSendPath", contractHarnessReport.routingStatus.defaultSendPath],
                ["startStreamPath", contractHarnessReport.routingStatus.startStreamPath],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            <div className="grid gap-2 md:grid-cols-2">
              {[
                contractHarnessReport.sendMessageContract,
                contractHarnessReport.streamMessageContract,
              ].map(contract => (
                <div
                  key={contract.name}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700"
                >
                  <div className="font-medium text-stone-900">{contract.name}</div>
                  <div className="mt-1 font-mono">ready: {String(contract.ready)}</div>
                  <div className="font-mono">expectedPath: {contract.expectedPath}</div>
                  <div className="font-mono">actualPath: {contract.actualPath}</div>
                  {contract.blockingReasons.length > 0 && (
                    <div className="mt-1 space-y-1">
                      {contract.blockingReasons.map(reason => (
                        <div
                          key={reason}
                          className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-red-700"
                        >
                          {reason}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>

            {safeSummaryEntries(contractHarnessReport.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(contractHarnessReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {contractHarnessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {contractHarnessReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No adapter contract blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter contract harness report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">Default Chat Adapter Dry Run</div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Explicit W40 dry-run boundary for the future adapter invocation contract. It requires
              the W39 harness, stays write-disabled, and keeps default Chat on legacy stream.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterDryRun}
            disabled={adapterDryRunChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterDryRunChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <Play size={13} />
            {adapterDryRunChecking ? "Running..." : "Run Adapter Dry Run"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          The dry run sends only a bounded probe descriptor to the adapter boundary. It does not
          save chat messages, call tools or models, create evidence, or switch routing.
        </div>

        {adapterDryRunError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterDryRunError}
          </div>
        )}

        {adapterDryRunReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterDryRunReport.dryRunReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterDryRunReport.dryRunReady
                ? "Adapter dry run ready"
                : "Adapter dry run blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["contractShape", adapterDryRunReport.contractShape],
                ["sourceSessionId", adapterDryRunReport.sourceSessionId],
                ["adapterPath", adapterDryRunReport.adapterPath],
                ["allowWrites", String(adapterDryRunReport.allowWrites)],
                ["maxToolCalls", String(adapterDryRunReport.maxToolCalls)],
                ["defaultChatPathUnchanged", String(adapterDryRunReport.defaultChatPathUnchanged)],
                ["chatMessageSaved", String(adapterDryRunReport.chatMessageSaved)],
                ["agentRunRecorded", String(adapterDryRunReport.agentRunRecorded)],
                ["contractHarnessReady", String(adapterDryRunReport.contractHarnessReady)],
                ["inputMessageLength", String(adapterDryRunReport.inputMessageLength)],
                ["inputMessageHash", adapterDryRunReport.inputMessageHash],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(adapterDryRunReport.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterDryRunReport.metadataSafeSummary).map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                  >
                    {key}: {value}
                  </span>
                ))}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {adapterDryRunReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterDryRunReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No adapter dry run blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter dry run report loaded.
          </div>
        )}

        <div className="mt-5 border-t border-stone-100 pt-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-900">
                Default Chat Adapter Dry Run Review
              </div>
              <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
                Explicit W41 human review evidence for the dry-run result. Approve requires a ready
                dry run; all notes are stored as checksum, length, and bounded category only.
              </div>
            </div>
            <button
              type="button"
              onClick={handleAdapterDryRunReviewSummaryRefresh}
              disabled={adapterDryRunReviewSummaryChecking}
              className={classNames(
                "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                adapterDryRunReviewSummaryChecking
                  ? "bg-stone-100 text-stone-400"
                  : "bg-stone-900 text-amber-50 hover:bg-stone-800"
              )}
            >
              <RefreshCw
                size={13}
                className={adapterDryRunReviewSummaryChecking ? "animate-spin" : undefined}
              />
              {adapterDryRunReviewSummaryChecking
                ? "Refreshing..."
                : "Refresh Dry Run Review Summary"}
            </button>
          </div>

          <label className="mt-4 block">
            <span className="text-xs font-medium text-stone-700">Dry-run reviewer note</span>
            <textarea
              value={adapterDryRunReviewNote}
              onChange={event => setAdapterDryRunReviewNote(event.target.value)}
              rows={3}
              className="mt-1 w-full rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-800 outline-none focus:border-stone-500"
              placeholder="Optional dry-run review note; only metadata is stored."
            />
          </label>

          <div className="mt-3 flex flex-wrap gap-2">
            {[
              ["approve", "Approve Dry Run Review"],
              ["reject", "Reject Dry Run Review"],
              ["request_rework", "Request Dry Run Rework"],
            ].map(([decisionKind, label]) => (
              <button
                key={decisionKind}
                type="button"
                onClick={() =>
                  handleRecordAdapterDryRunReviewDecision(
                    decisionKind as DefaultChatAdapterDryRunReviewDecisionKind
                  )
                }
                disabled={adapterDryRunReviewRecording || !adapterDryRunReport}
                className={classNames(
                  "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                  adapterDryRunReviewRecording || !adapterDryRunReport
                    ? "bg-stone-100 text-stone-400"
                    : decisionKind === "approve"
                      ? "bg-emerald-700 text-white hover:bg-emerald-800"
                      : "bg-stone-900 text-amber-50 hover:bg-stone-800"
                )}
              >
                {decisionKind === "approve" ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
                {label}
              </button>
            ))}
          </div>

          {adapterDryRunReviewError && (
            <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
              {adapterDryRunReviewError}
            </div>
          )}

          {adapterDryRunReviewSummaryError && (
            <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
              {adapterDryRunReviewSummaryError}
            </div>
          )}

          {adapterDryRunReviewResult && (
            <div className="mt-4 space-y-2 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
              <div className="font-medium text-stone-900">
                {adapterDryRunReviewResult.recorded
                  ? "Dry-run review evidence recorded"
                  : "Dry-run review not recorded"}
              </div>
              <div>decisionKind: {adapterDryRunReviewResult.decisionKind}</div>
              <div>sourceSessionId: {adapterDryRunReviewResult.sourceSessionId}</div>
              <div>contractShape: {adapterDryRunReviewResult.contractShape}</div>
              <div>reviewDryRunReady: {String(adapterDryRunReviewResult.dryRunReady)}</div>
              <div className="break-all">
                dryRunSummaryDigest: {adapterDryRunReviewResult.dryRunSummaryDigest}
              </div>
              {adapterDryRunReviewResult.evidenceId && (
                <div>evidenceId: {adapterDryRunReviewResult.evidenceId}</div>
              )}
              {adapterDryRunReviewResult.blockingReasons.length > 0 && (
                <div className="space-y-1">
                  {adapterDryRunReviewResult.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {adapterDryRunReviewSummary ? (
            <div className="mt-4 space-y-3">
              <div className="grid gap-2 md:grid-cols-3">
                {[
                  ["approvedCount", String(adapterDryRunReviewSummary.approvedCount)],
                  ["rejectOrReworkCount", String(adapterDryRunReviewSummary.rejectOrReworkCount)],
                  ["latestTimestamp", adapterDryRunReviewSummary.latestTimestamp ?? "none"],
                ].map(([label, value]) => (
                  <div
                    key={label}
                    className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                  >
                    {label}: {value}
                  </div>
                ))}
              </div>

              {adapterDryRunReviewSummary.latestDecision && (
                <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                  <div className="font-medium text-stone-900">Latest dry-run review decision</div>
                  <div className="mt-1">
                    latestDecisionKind: {adapterDryRunReviewSummary.latestDecision.decisionKind}
                  </div>
                  <div>
                    sourceSessionId: {adapterDryRunReviewSummary.latestDecision.sourceSessionId}
                  </div>
                  <div>
                    contractShape: {adapterDryRunReviewSummary.latestDecision.contractShape}
                  </div>
                  <div>
                    latestDryRunReady:{" "}
                    {String(adapterDryRunReviewSummary.latestDecision.dryRunReady)}
                  </div>
                  <div className="break-all">
                    dryRunSummaryDigest:{" "}
                    {adapterDryRunReviewSummary.latestDecision.dryRunSummaryDigest}
                  </div>
                  <div>
                    reviewerNoteCategory:{" "}
                    {adapterDryRunReviewSummary.latestDecision.reviewerNoteCategory}
                  </div>
                  <div>
                    reviewerNoteLength:{" "}
                    {adapterDryRunReviewSummary.latestDecision.reviewerNoteLength}
                  </div>
                </div>
              )}

              {safeSummaryEntries(adapterDryRunReviewSummary.metadataSafeSummary).length > 0 && (
                <div className="flex flex-wrap gap-2 text-xs">
                  {safeSummaryEntries(adapterDryRunReviewSummary.metadataSafeSummary).map(
                    ([key, value]) => (
                      <span
                        key={key}
                        className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                      >
                        {key}: {value}
                      </span>
                    )
                  )}
                </div>
              )}

              <div>
                <div className="text-xs font-medium text-stone-700">Review blockers</div>
                {adapterDryRunReviewSummary.blockingReasons.length > 0 ? (
                  <div className="mt-1 space-y-1">
                    {adapterDryRunReviewSummary.blockingReasons.map(reason => (
                      <div
                        key={reason}
                        className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                      >
                        {reason}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="mt-1 text-xs text-stone-500">
                    No dry-run review blockers returned.
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="mt-3 text-xs text-stone-500">
              No default Chat adapter dry-run review summary loaded.
            </div>
          )}
        </div>
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Implementation Readiness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W42 gate over activation implementation eligibility, adapter contract
              harness, dry-run boundary, and latest dry-run review evidence. Ready means
              implementation discussion can proceed; it does not switch, enable, activate, or
              migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterImplementationReadinessCheck}
            disabled={adapterImplementationReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterImplementationReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={adapterImplementationReadinessChecking ? "animate-spin" : undefined}
            />
            {adapterImplementationReadinessChecking
              ? "Checking..."
              : "Check Adapter Implementation Readiness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          The gate re-checks the same bounded dry-run probe and compares its metadata-safe digest
          with the latest approved dry-run review. It does not write Evidence, Chat messages,
          AgentRuns, proposals, memory, LifeModel patches, MCP audit, external writes, or model
          calls.
        </div>

        {adapterImplementationReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterImplementationReadinessError}
          </div>
        )}

        {adapterImplementationReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterImplementationReadinessReport.implementationReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterImplementationReadinessReport.implementationReady
                ? "Implementation readiness ready"
                : "Implementation readiness blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                [
                  "implementationReady",
                  String(adapterImplementationReadinessReport.implementationReady),
                ],
                [
                  "activationImplementationGateEligible",
                  String(adapterImplementationReadinessReport.activationImplementationGateEligible),
                ],
                [
                  "contractHarnessReady",
                  String(adapterImplementationReadinessReport.contractHarnessReady),
                ],
                ["dryRunReady", String(adapterImplementationReadinessReport.dryRunReady)],
                [
                  "dryRunReviewApproved",
                  String(adapterImplementationReadinessReport.dryRunReviewApproved),
                ],
                [
                  "dryRunDigestMatched",
                  String(adapterImplementationReadinessReport.dryRunDigestMatched),
                ],
                [
                  "defaultChatUnchanged",
                  String(adapterImplementationReadinessReport.defaultChatUnchanged),
                ],
                [
                  "controlledAdapterEnabled",
                  String(adapterImplementationReadinessReport.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(adapterImplementationReadinessReport.automaticMigrationEnabled),
                ],
                ["defaultSendPath", adapterImplementationReadinessReport.defaultSendPath],
                ["startStreamPath", adapterImplementationReadinessReport.startStreamPath],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {adapterImplementationReadinessReport.latestDryRunReviewDecision && (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                <div className="font-medium text-stone-900">Latest dry-run review decision</div>
                <div className="mt-1">
                  latestDryRunReviewDecisionKind:{" "}
                  {adapterImplementationReadinessReport.latestDryRunReviewDecision.decisionKind}
                </div>
                <div>
                  latestDryRunReady:{" "}
                  {String(
                    adapterImplementationReadinessReport.latestDryRunReviewDecision.dryRunReady
                  )}
                </div>
                <div>
                  sourceSessionId:{" "}
                  {adapterImplementationReadinessReport.latestDryRunReviewDecision.sourceSessionId}
                </div>
                <div className="break-all">
                  dryRunSummaryDigest:{" "}
                  {
                    adapterImplementationReadinessReport.latestDryRunReviewDecision
                      .dryRunSummaryDigest
                  }
                </div>
              </div>
            )}

            {safeSummaryEntries(adapterImplementationReadinessReport.metadataSafeSummary).length >
              0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterImplementationReadinessReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {adapterImplementationReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterImplementationReadinessReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No adapter implementation readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter implementation readiness report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Controlled Preview
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Explicit W43 controlled implementation preview. It first checks implementation
              readiness, then runs only a non-default, write-disabled, zero-tool preview. It does
              not save to Chat, promote output, enable the adapter, or migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterControlledPreview}
            disabled={adapterControlledPreviewChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterControlledPreviewChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <Play size={13} />
            {adapterControlledPreviewChecking ? "Running..." : "Run Adapter Controlled Preview"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          The preview returns a Send-compatible shape for inspection only. It keeps
          allowWrites=false, maxToolCalls=0, defaultSendPath=legacy_stream, and
          startStreamPath=legacy_stream.
        </div>

        {adapterControlledPreviewError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterControlledPreviewError}
          </div>
        )}

        {adapterControlledPreviewReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterControlledPreviewReport.previewReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterControlledPreviewReport.previewReady
                ? "Controlled preview ready"
                : "Controlled preview blocked"}
            </div>

            {adapterControlledPreviewReport.reply && (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-sm leading-6 text-stone-800">
                {adapterControlledPreviewReport.reply}
              </div>
            )}

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["previewReady", String(adapterControlledPreviewReport.previewReady)],
                ["blocked", String(adapterControlledPreviewReport.blocked)],
                ["contractShape", adapterControlledPreviewReport.contractShape],
                ["adapterPath", adapterControlledPreviewReport.adapterPath],
                ["sourceSessionId", adapterControlledPreviewReport.sourceSessionId],
                ["runId", adapterControlledPreviewReport.runId ?? "none"],
                ["allowWrites", String(adapterControlledPreviewReport.allowWrites)],
                ["maxToolCalls", String(adapterControlledPreviewReport.maxToolCalls)],
                [
                  "defaultChatPathUnchanged",
                  String(adapterControlledPreviewReport.defaultChatPathUnchanged),
                ],
                ["chatMessageSaved", String(adapterControlledPreviewReport.chatMessageSaved)],
                ["agentRunRecorded", String(adapterControlledPreviewReport.agentRunRecorded)],
                ["implementationReady", String(adapterControlledPreviewReport.implementationReady)],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(adapterControlledPreviewReport.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterControlledPreviewReport.metadataSafeSummary).map(
                  ([key, value]) => (
                    <span
                      key={key}
                      className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                    >
                      {key}: {value}
                    </span>
                  )
                )}
              </div>
            )}

            {adapterControlledPreviewReport.warnings.length > 0 && (
              <div>
                <div className="text-xs font-medium text-stone-700">Warnings</div>
                <div className="mt-1 space-y-1">
                  {adapterControlledPreviewReport.warnings.map(warning => (
                    <div
                      key={warning}
                      className="rounded-md border border-amber-100 bg-amber-50 px-2 py-1 text-xs text-amber-800"
                    >
                      {warning}
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div>
              <div className="text-xs font-medium text-stone-700">Blocking reasons</div>
              {adapterControlledPreviewReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterControlledPreviewReport.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-1 text-xs text-stone-500">
                  No adapter controlled preview blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No default Chat adapter controlled preview report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white">
        <button
          type="button"
          onClick={() => setOpen(value => !value)}
          aria-expanded={open}
          className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left"
        >
          <span className="flex min-w-0 items-center gap-2">
            {open ? (
              <ChevronDown size={16} className="shrink-0 text-stone-500" />
            ) : (
              <ChevronRight size={16} className="shrink-0 text-stone-500" />
            )}
            <span>
              <span className="block text-sm font-semibold text-stone-900">
                MultiStrategy Preview
              </span>
              <span className="block text-xs text-stone-500">Non-default debug runtime entry</span>
            </span>
          </span>
          <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-medium text-amber-800">
            Preview/Beta
          </span>
        </button>

        {open && (
          <div className="space-y-4 border-t border-stone-100 p-4">
            <form onSubmit={handleSubmit} className="space-y-4">
              <label className="block">
                <span className="text-xs font-medium text-stone-700">userText</span>
                <textarea
                  value={userText}
                  onChange={event => setUserText(event.target.value)}
                  rows={4}
                  className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                  placeholder="Describe a runtime preview task..."
                />
              </label>

              <div className="grid gap-3 md:grid-cols-3">
                <label className="block">
                  <span className="text-xs font-medium text-stone-700">layer</span>
                  <select
                    value={layer}
                    onChange={event =>
                      setLayer(event.target.value as MultiStrategyAgentPreviewLayer)
                    }
                    className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                  >
                    <option value="L1">L1</option>
                    <option value="L2">L2</option>
                    <option value="L3">L3</option>
                  </select>
                </label>

                <label className="flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-700">
                  <input
                    type="checkbox"
                    checked={allowPlanning}
                    onChange={event => setAllowPlanning(event.target.checked)}
                    className="rounded border-stone-300"
                  />
                  <span>allowPlanning</span>
                </label>

                <label className="flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-700">
                  <input
                    type="checkbox"
                    checked={localModelAvailable}
                    onChange={event => setLocalModelAvailable(event.target.checked)}
                    className="rounded border-stone-300"
                  />
                  <span>localModelAvailable</span>
                </label>
              </div>

              <div className="rounded-md border border-stone-200">
                <button
                  type="button"
                  onClick={() => setAdvancedOpen(value => !value)}
                  aria-expanded={advancedOpen}
                  className="flex w-full items-center justify-between px-3 py-2 text-left text-xs font-medium text-stone-700"
                >
                  <span>Advanced toolsPrompt</span>
                  {advancedOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                {advancedOpen && (
                  <div className="border-t border-stone-100 p-3">
                    <label className="block">
                      <span className="text-xs font-medium text-stone-700">toolsPrompt</span>
                      <textarea
                        value={toolsPrompt}
                        onChange={event => setToolsPrompt(event.target.value)}
                        rows={3}
                        className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 focus:border-stone-900 focus:outline-none focus:ring-1 focus:ring-stone-900"
                        placeholder="Optional developer-supplied tool summary"
                      />
                    </label>
                  </div>
                )}
              </div>

              <div className="rounded-md border border-emerald-100 bg-emerald-50 p-3 text-xs text-emerald-900">
                <div className="flex items-center gap-2 font-medium">
                  <ShieldCheck size={14} />
                  <span>Preview guardrails</span>
                </div>
                <div className="mt-1 leading-5">
                  No LifeModel, Memory, Proposal, email, calendar, or file write executor is invoked
                  from this panel. Empty toolsPrompt is sent as a no-catalog marker.
                </div>
              </div>

              <div className="flex justify-end">
                <button
                  type="submit"
                  disabled={submitting}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium",
                    submitting
                      ? "bg-stone-200 text-stone-500"
                      : "bg-stone-900 text-amber-50 hover:bg-stone-800"
                  )}
                >
                  <Play size={14} />
                  {submitting ? "Running..." : "Run Preview"}
                </button>
              </div>
            </form>

            {error && (
              <div className="rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
                {error}
              </div>
            )}

            {result && (
              <div className="space-y-3 rounded-lg border border-stone-200 bg-stone-50 p-4">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <div className="text-sm font-semibold text-stone-900">Preview result</div>
                    <div className="mt-1 text-xs text-stone-500">
                      Metadata-safe summary only. Review the persisted trace in Runs.
                    </div>
                  </div>
                  {result.runId && (
                    <button
                      type="button"
                      onClick={() => navigate(`/runs/${result.runId}`)}
                      className="inline-flex items-center gap-1.5 rounded-md bg-white px-3 py-1.5 text-xs font-medium text-stone-700 ring-1 ring-stone-200 hover:bg-stone-100"
                    >
                      <ExternalLink size={13} />
                      View Run Trace
                    </button>
                  )}
                </div>

                <div className="grid gap-2 text-xs md:grid-cols-2">
                  <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                    <div className="text-[10px] uppercase text-stone-400">runId</div>
                    <div className="mt-1 font-mono text-stone-900">
                      {result.runId ?? "not returned"}
                    </div>
                  </div>
                  <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                    Strategy: {result.strategyKind}
                  </div>
                  <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                    Payload: {result.payloadKind}
                  </div>
                  <div className="rounded-md bg-white px-3 py-2 text-stone-700 ring-1 ring-stone-100">
                    Governance: {result.governanceDecisionKind ?? "unknown"}
                  </div>
                </div>

                {summaryEntries.length > 0 && (
                  <div className="flex flex-wrap gap-2 text-xs">
                    {summaryEntries.map(([key, value]) => (
                      <span
                        key={key}
                        className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                      >
                        {key}: {value}
                      </span>
                    ))}
                  </div>
                )}

                <div>
                  <div className="text-xs font-medium text-stone-700">Warnings</div>
                  {result.warnings.length > 0 ? (
                    <div className="mt-1 space-y-1">
                      {result.warnings.map(warning => (
                        <div
                          key={warning}
                          className="rounded-md border border-amber-100 bg-amber-50 px-2 py-1 text-xs text-amber-800"
                        >
                          {warning}
                        </div>
                      ))}
                    </div>
                  ) : (
                    <div className="mt-1 text-xs text-stone-500">No warnings returned.</div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
