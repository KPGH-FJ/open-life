import type { Dispatch, FormEvent, SetStateAction } from "react";
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
import { runDetailRoute } from "../../../productShellContract";
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
} from "../../../types";
import { GATE_FIELDS, PlanList, classNames, safeSummaryEntries } from "./shared";

type Setter<T> = Dispatch<SetStateAction<T>>;
type DecisionHandler<T> = (decisionKind: T) => void;

export interface MultiStrategyPanelProps {
  activationImplementationGateChecking: boolean;
  activationImplementationGateError: string | null;
  activationImplementationGateReport: DefaultChatAdapterActivationImplementationGateReport | null;
  activationPlanChecking: boolean;
  activationPlanDraft: DefaultChatAdapterActivationPlanDraft | null;
  activationPlanError: string | null;
  activationReviewError: string | null;
  activationReviewNote: string;
  activationReviewRecording: boolean;
  activationReviewResult: DefaultChatAdapterActivationReviewDecisionResult | null;
  activationReviewSummary: DefaultChatAdapterActivationReviewSummary | null;
  activationReviewSummaryChecking: boolean;
  activationReviewSummaryError: string | null;
  adapterControlledPreviewApprovalReadinessChecking: boolean;
  adapterControlledPreviewApprovalReadinessError: string | null;
  adapterControlledPreviewApprovalReadinessReport: DefaultChatAdapterControlledPreviewApprovalReadinessReport | null;
  adapterControlledPreviewChecking: boolean;
  adapterControlledPreviewError: string | null;
  adapterControlledPreviewReport: DefaultChatAdapterControlledPreviewReport | null;
  adapterControlledPreviewReviewError: string | null;
  adapterControlledPreviewReviewNote: string;
  adapterControlledPreviewReviewRecording: boolean;
  adapterControlledPreviewReviewResult: DefaultChatAdapterControlledPreviewReviewDecisionResult | null;
  adapterControlledPreviewReviewSummary: DefaultChatAdapterControlledPreviewReviewSummary | null;
  adapterControlledPreviewReviewSummaryChecking: boolean;
  adapterControlledPreviewReviewSummaryError: string | null;
  adapterCutoverPlanApprovalReadinessChecking: boolean;
  adapterCutoverPlanApprovalReadinessError: string | null;
  adapterCutoverPlanApprovalReadinessReport: DefaultChatAdapterCutoverPlanApprovalReadinessReport | null;
  adapterCutoverPlanDraft: DefaultChatAdapterCutoverImplementationPlanDraft | null;
  adapterCutoverPlanDrafting: boolean;
  adapterCutoverPlanError: string | null;
  adapterCutoverPlanReviewError: string | null;
  adapterCutoverPlanReviewNote: string;
  adapterCutoverPlanReviewRecording: boolean;
  adapterCutoverPlanReviewResult: DefaultChatAdapterCutoverPlanReviewDecisionResult | null;
  adapterCutoverPlanReviewSummary: DefaultChatAdapterCutoverPlanReviewSummary | null;
  adapterCutoverPlanReviewSummaryChecking: boolean;
  adapterCutoverPlanReviewSummaryError: string | null;
  adapterDryRunChecking: boolean;
  adapterDryRunError: string | null;
  adapterDryRunReport: DefaultChatAdapterDryRunReport | null;
  adapterDryRunReviewError: string | null;
  adapterDryRunReviewNote: string;
  adapterDryRunReviewRecording: boolean;
  adapterDryRunReviewResult: DefaultChatAdapterDryRunReviewDecisionResult | null;
  adapterDryRunReviewSummary: DefaultChatAdapterDryRunReviewSummary | null;
  adapterDryRunReviewSummaryChecking: boolean;
  adapterDryRunReviewSummaryError: string | null;
  adapterImplementationReadinessChecking: boolean;
  adapterImplementationReadinessError: string | null;
  adapterImplementationReadinessReport: DefaultChatAdapterImplementationReadinessReport | null;
  adapterRoutingChecking: boolean;
  adapterRoutingError: string | null;
  adapterRoutingStatus: DefaultChatAdapterRoutingStatus | null;
  advancedOpen: boolean;
  allowPlanning: boolean;
  boundaryChecking: boolean;
  boundaryError: string | null;
  boundaryStatus: DefaultChatRuntimeBoundaryStatus | null;
  candidatePromotionReadinessChecking: boolean;
  candidatePromotionReadinessError: string | null;
  candidatePromotionReadinessReport: ControlledChatCutoverCandidatePromotionReadinessReport | null;
  contractHarnessChecking: boolean;
  contractHarnessError: string | null;
  contractHarnessReport: DefaultChatAdapterContractHarnessReport | null;
  cutoverCandidateChecking: boolean;
  cutoverCandidateError: string | null;
  cutoverCandidateResult: ControlledChatCutoverCandidateOutput | null;
  cutoverCandidateReviewError: string | null;
  cutoverCandidateReviewNote: string;
  cutoverCandidateReviewRecording: boolean;
  cutoverCandidateReviewResult: ControlledChatCutoverCandidateReviewDecisionResult | null;
  cutoverCandidateReviewSummary: ControlledChatCutoverCandidateReviewSummary | null;
  cutoverCandidateReviewSummaryChecking: boolean;
  cutoverCandidateReviewSummaryError: string | null;
  cutoverReadinessChecking: boolean;
  cutoverReadinessError: string | null;
  cutoverReadinessReport: ControlledChatCutoverReadinessReport | null;
  error: string | null;
  gateChecking: boolean;
  gateError: string | null;
  gateReport: RuntimeMigrationGateReport | null;
  handleActivationImplementationGateCheck: () => void;
  handleActivationPlanRefresh: () => void;
  handleActivationReviewSummaryRefresh: () => void;
  handleAdapterControlledPreview: () => void;
  handleAdapterControlledPreviewApprovalReadinessCheck: () => void;
  handleAdapterControlledPreviewReviewSummaryRefresh: () => void;
  handleAdapterCutoverPlanApprovalReadinessCheck: () => void;
  handleAdapterCutoverPlanDraft: () => void;
  handleAdapterCutoverPlanReviewDecision: DecisionHandler<DefaultChatAdapterCutoverPlanReviewDecisionKind>;
  handleAdapterCutoverPlanReviewSummary: () => void;
  handleAdapterDryRun: () => void;
  handleAdapterDryRunReviewSummaryRefresh: () => void;
  handleAdapterImplementationReadinessCheck: () => void;
  handleAdapterRoutingRefresh: () => void;
  handleCandidatePromotionReadinessRefresh: () => void;
  handleContractHarnessCheck: () => void;
  handleCutoverCandidateReviewSummaryRefresh: () => void;
  handleCutoverCandidateRun: () => void;
  handleCutoverReadinessCheck: () => void;
  handleDefaultChatBoundaryRefresh: () => void;
  handleGateCheck: () => void;
  handleImplementationGateCheck: () => void;
  handleMigrationDraft: () => void;
  handleNarrowImplementationGateCheck: () => void;
  handleNarrowImplementationPlanApprovalReadinessCheck: () => void;
  handleNarrowImplementationPlanDraft: () => void;
  handleNarrowImplementationPlanReviewSummaryRefresh: () => void;
  handleOrdinaryEntryPreflightRefresh: () => void;
  handlePilotEligibilityCheck: () => void;
  handlePromotionReadinessCheck: () => void;
  handlePromotionSummaryRefresh: () => void;
  handleRecordActivationReviewDecision: DecisionHandler<DefaultChatAdapterActivationReviewDecisionKind>;
  handleRecordAdapterControlledPreviewReviewDecision: DecisionHandler<DefaultChatAdapterControlledPreviewReviewDecisionKind>;
  handleRecordAdapterDryRunReviewDecision: DecisionHandler<DefaultChatAdapterDryRunReviewDecisionKind>;
  handleRecordCutoverCandidateReviewDecision: DecisionHandler<ControlledChatCutoverCandidateReviewDecisionKind>;
  handleRecordNarrowImplementationPlanReviewDecision: DecisionHandler<DefaultChatAdapterNarrowImplementationPlanReviewDecisionKind>;
  handleRecordReviewDecision: DecisionHandler<ControlledChatMigrationReviewDecisionKind>;
  handleRecordShadowReviewDecision: DecisionHandler<ControlledChatMigrationShadowReviewDecisionKind>;
  handleReviewDecisionSummaryRefresh: () => void;
  handleShadowReviewSummaryRefresh: () => void;
  handleShadowRun: () => void;
  handleSubmit: (event: FormEvent<HTMLFormElement>) => void;
  implementationGateChecking: boolean;
  implementationGateError: string | null;
  implementationGateReport: ControlledChatMigrationImplementationGateReport | null;
  layer: MultiStrategyAgentPreviewLayer;
  localModelAvailable: boolean;
  migrationDraft: ControlledChatMigrationPlanDraft | null;
  migrationDraftChecking: boolean;
  migrationDraftError: string | null;
  narrowImplementationGateChecking: boolean;
  narrowImplementationGateError: string | null;
  narrowImplementationGateReport: DefaultChatAdapterNarrowImplementationDiscussionGateReport | null;
  narrowImplementationPlanApprovalReadinessChecking: boolean;
  narrowImplementationPlanApprovalReadinessError: string | null;
  narrowImplementationPlanApprovalReadinessReport: DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport | null;
  narrowImplementationPlanDraft: DefaultChatAdapterNarrowImplementationPlanDraft | null;
  narrowImplementationPlanDrafting: boolean;
  narrowImplementationPlanError: string | null;
  narrowImplementationPlanReviewError: string | null;
  narrowImplementationPlanReviewNote: string;
  narrowImplementationPlanReviewRecording: boolean;
  narrowImplementationPlanReviewResult: DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult | null;
  narrowImplementationPlanReviewSummary: DefaultChatAdapterNarrowImplementationPlanReviewSummary | null;
  narrowImplementationPlanReviewSummaryChecking: boolean;
  narrowImplementationPlanReviewSummaryError: string | null;
  navigate: (to: string) => void;
  open: boolean;
  ordinaryEntryPreflightChecking: boolean;
  ordinaryEntryPreflightError: string | null;
  ordinaryEntryPreflightStatus: DefaultChatAdapterOrdinaryEntryPreflightStatus | null;
  pilotChecking: boolean;
  pilotError: string | null;
  pilotReport: ControlledChatPilotEligibilityReport | null;
  promotionReadinessChecking: boolean;
  promotionReadinessError: string | null;
  promotionReadinessReport: ControlledPilotPromotionReadinessReport | null;
  promotionSummary: ControlledPilotPromotionEvidenceSummary | null;
  promotionSummaryChecking: boolean;
  promotionSummaryError: string | null;
  result: MultiStrategyAgentPreviewOutput | null;
  reviewDecisionError: string | null;
  reviewDecisionRecording: boolean;
  reviewDecisionResult: ControlledChatMigrationReviewDecisionResult | null;
  reviewDecisionSummary: ControlledChatMigrationReviewDecisionSummary | null;
  reviewDecisionSummaryChecking: boolean;
  reviewDecisionSummaryError: string | null;
  reviewerNote: string;
  setActivationReviewNote: Setter<string>;
  setAdapterControlledPreviewReviewNote: Setter<string>;
  setAdapterCutoverPlanReviewNote: Setter<string>;
  setAdapterDryRunReviewNote: Setter<string>;
  setAdvancedOpen: Setter<boolean>;
  setAllowPlanning: Setter<boolean>;
  setCutoverCandidateReviewNote: Setter<string>;
  setLayer: Setter<MultiStrategyAgentPreviewLayer>;
  setLocalModelAvailable: Setter<boolean>;
  setNarrowImplementationPlanReviewNote: Setter<string>;
  setOpen: Setter<boolean>;
  setReviewerNote: Setter<string>;
  setShadowDescriptor: Setter<ControlledChatMigrationShadowRunDescriptor>;
  setShadowReviewNote: Setter<string>;
  setToolsPrompt: Setter<string>;
  setUserText: Setter<string>;
  shadowDescriptor: ControlledChatMigrationShadowRunDescriptor;
  shadowReviewError: string | null;
  shadowReviewNote: string;
  shadowReviewRecording: boolean;
  shadowReviewResult: ControlledChatMigrationShadowReviewDecisionResult | null;
  shadowReviewSummary: ControlledChatMigrationShadowReviewSummary | null;
  shadowReviewSummaryChecking: boolean;
  shadowReviewSummaryError: string | null;
  shadowRunChecking: boolean;
  shadowRunError: string | null;
  shadowRunResult: ControlledChatMigrationShadowRunOutput | null;
  submitting: boolean;
  summaryEntries: Array<[string, string]>;
  toolsPrompt: string;
  userText: string;
}

export function RuntimePreviewPanel(props: MultiStrategyPanelProps) {
  const {
    advancedOpen,
    allowPlanning,
    boundaryChecking,
    boundaryError,
    boundaryStatus,
    error,
    gateChecking,
    gateError,
    gateReport,
    handleDefaultChatBoundaryRefresh,
    handleGateCheck,
    handleOrdinaryEntryPreflightRefresh,
    handleSubmit,
    layer,
    localModelAvailable,
    navigate,
    open,
    ordinaryEntryPreflightChecking,
    ordinaryEntryPreflightError,
    ordinaryEntryPreflightStatus,
    result,
    setAdvancedOpen,
    setAllowPlanning,
    setLayer,
    setLocalModelAvailable,
    setOpen,
    setToolsPrompt,
    setUserText,
    submitting,
    summaryEntries,
    toolsPrompt,
    userText,
  } = props;

  return (
    <>
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
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Ordinary Entry Preflight
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W56 status over the W55 ordinary-entry preflight. It checks that ordinary
              send and stream entries remain legacy-only, side-effect locked, and non-migrating.
            </div>
          </div>
          <button
            type="button"
            onClick={handleOrdinaryEntryPreflightRefresh}
            disabled={ordinaryEntryPreflightChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              ordinaryEntryPreflightChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={ordinaryEntryPreflightChecking ? "animate-spin" : undefined}
            />
            {ordinaryEntryPreflightChecking ? "Checking..." : "Refresh Ordinary Entry Preflight"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This status command does not call runtime, tools, models, migration gates, preview
          commands, or evidence recorders. It only reads the current default Chat adapter route and
          W55 preflight guard.
        </div>

        {ordinaryEntryPreflightError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {ordinaryEntryPreflightError}
          </div>
        )}

        {ordinaryEntryPreflightStatus ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                ordinaryEntryPreflightStatus.statusReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {ordinaryEntryPreflightStatus.statusReady
                ? "Ordinary entry preflight ready"
                : "Ordinary entry preflight blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["statusReady", String(ordinaryEntryPreflightStatus.statusReady)],
                ["defaultChatUnchanged", String(ordinaryEntryPreflightStatus.defaultChatUnchanged)],
                ["currentMode", ordinaryEntryPreflightStatus.currentMode],
                [
                  "controlledAdapterEnabled",
                  String(ordinaryEntryPreflightStatus.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(ordinaryEntryPreflightStatus.automaticMigrationEnabled),
                ],
                ["defaultSendPath", ordinaryEntryPreflightStatus.defaultSendPath],
                ["startStreamPath", ordinaryEntryPreflightStatus.startStreamPath],
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
                ordinaryEntryPreflightStatus.sendMessagePreflight,
                ordinaryEntryPreflightStatus.streamMessagePreflight,
              ].map(preflight => (
                <div
                  key={preflight.callsite}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700"
                >
                  <div className="font-medium text-stone-900">{preflight.callsite}</div>
                  <div className="mt-1 font-mono">
                    preflightReady: {String(preflight.preflightReady)}
                  </div>
                  <div className="font-mono">
                    legacyEntryAllowed: {String(preflight.legacyEntryAllowed)}
                  </div>
                  <div className="font-mono">contractShape: {preflight.contractShape}</div>
                  <div className="font-mono">ordinaryEntryPath: {preflight.ordinaryEntryPath}</div>
                  <div className="font-mono">
                    sideEffectLockEngaged: {String(preflight.sideEffectLockEngaged)}
                  </div>
                  <div className="font-mono">allowWrites: {String(preflight.allowWrites)}</div>
                  <div className="font-mono">maxToolCalls: {preflight.maxToolCalls}</div>
                  <div className="font-mono">
                    defaultChatMigrationAllowed: {String(preflight.defaultChatMigrationAllowed)}
                  </div>
                  {preflight.blockingReasons.length > 0 && (
                    <div className="mt-1 space-y-1">
                      {preflight.blockingReasons.map(reason => (
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

            {safeSummaryEntries(ordinaryEntryPreflightStatus.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(ordinaryEntryPreflightStatus.metadataSafeSummary).map(
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
              {ordinaryEntryPreflightStatus.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {ordinaryEntryPreflightStatus.blockingReasons.map(reason => (
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
                  No ordinary-entry preflight blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No ordinary-entry preflight status loaded.
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
            内部预览
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
                      onClick={() => navigate(runDetailRoute(result.runId!))}
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
    </>
  );
}

export function ControlledPilotPanel(props: MultiStrategyPanelProps) {
  const {
    handlePilotEligibilityCheck,
    handlePromotionReadinessCheck,
    handlePromotionSummaryRefresh,
    pilotChecking,
    pilotError,
    pilotReport,
    promotionReadinessChecking,
    promotionReadinessError,
    promotionReadinessReport,
    promotionSummary,
    promotionSummaryChecking,
    promotionSummaryError,
  } = props;

  return (
    <>
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
    </>
  );
}

export function MigrationPlanningPanel(props: MultiStrategyPanelProps) {
  const {
    cutoverReadinessChecking,
    cutoverReadinessError,
    cutoverReadinessReport,
    handleCutoverReadinessCheck,
    handleImplementationGateCheck,
    handleMigrationDraft,
    handleRecordReviewDecision,
    handleRecordShadowReviewDecision,
    handleReviewDecisionSummaryRefresh,
    handleShadowReviewSummaryRefresh,
    handleShadowRun,
    implementationGateChecking,
    implementationGateError,
    implementationGateReport,
    migrationDraft,
    migrationDraftChecking,
    migrationDraftError,
    reviewDecisionError,
    reviewDecisionRecording,
    reviewDecisionResult,
    reviewDecisionSummary,
    reviewDecisionSummaryChecking,
    reviewDecisionSummaryError,
    reviewerNote,
    setReviewerNote,
    setShadowDescriptor,
    setShadowReviewNote,
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
  } = props;

  return (
    <>
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
    </>
  );
}

export function CutoverCandidatePanel(props: MultiStrategyPanelProps) {
  const {
    candidatePromotionReadinessChecking,
    candidatePromotionReadinessError,
    candidatePromotionReadinessReport,
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
    handleCandidatePromotionReadinessRefresh,
    handleCutoverCandidateReviewSummaryRefresh,
    handleCutoverCandidateRun,
    handleRecordCutoverCandidateReviewDecision,
    setCutoverCandidateReviewNote,
  } = props;

  return (
    <>
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
    </>
  );
}

export function DefaultChatActivationPanel(props: MultiStrategyPanelProps) {
  const {
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
    adapterRoutingChecking,
    adapterRoutingError,
    adapterRoutingStatus,
    handleActivationImplementationGateCheck,
    handleActivationPlanRefresh,
    handleActivationReviewSummaryRefresh,
    handleAdapterRoutingRefresh,
    handleRecordActivationReviewDecision,
    setActivationReviewNote,
  } = props;

  return (
    <>
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
    </>
  );
}

export function DefaultChatImplementationPanel(props: MultiStrategyPanelProps) {
  const {
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
    contractHarnessChecking,
    contractHarnessError,
    contractHarnessReport,
    handleAdapterControlledPreview,
    handleAdapterControlledPreviewApprovalReadinessCheck,
    handleAdapterControlledPreviewReviewSummaryRefresh,
    handleAdapterDryRun,
    handleAdapterDryRunReviewSummaryRefresh,
    handleAdapterImplementationReadinessCheck,
    handleContractHarnessCheck,
    handleRecordAdapterControlledPreviewReviewDecision,
    handleRecordAdapterDryRunReviewDecision,
    setAdapterControlledPreviewReviewNote,
    setAdapterDryRunReviewNote,
  } = props;

  return (
    <>
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

            <div className="rounded-md border border-stone-100 bg-white p-3">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <div className="text-xs font-medium text-stone-800">
                    Controlled Preview Review Evidence
                  </div>
                  <div className="mt-1 text-xs leading-5 text-stone-500">
                    Records metadata-safe human review evidence for the preview AgentRun only.
                  </div>
                </div>
                <button
                  type="button"
                  onClick={handleAdapterControlledPreviewReviewSummaryRefresh}
                  disabled={adapterControlledPreviewReviewSummaryChecking}
                  className={classNames(
                    "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                    adapterControlledPreviewReviewSummaryChecking
                      ? "bg-stone-100 text-stone-400"
                      : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
                  )}
                >
                  <RefreshCw size={13} />
                  {adapterControlledPreviewReviewSummaryChecking
                    ? "Refreshing..."
                    : "Refresh Controlled Preview Review Summary"}
                </button>
              </div>

              <textarea
                value={adapterControlledPreviewReviewNote}
                onChange={event => setAdapterControlledPreviewReviewNote(event.target.value)}
                placeholder="Optional preview review note"
                className="mt-3 min-h-[72px] w-full rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-800 outline-none focus:border-stone-400"
              />

              <div className="mt-3 flex flex-wrap gap-2">
                {(
                  [
                    ["approve", "Approve Controlled Preview"],
                    ["reject", "Reject Controlled Preview"],
                    ["request_rework", "Request Controlled Preview Rework"],
                  ] as const
                ).map(([decisionKind, label]) => (
                  <button
                    key={decisionKind}
                    type="button"
                    onClick={() => handleRecordAdapterControlledPreviewReviewDecision(decisionKind)}
                    disabled={
                      adapterControlledPreviewReviewRecording ||
                      !adapterControlledPreviewReport.runId
                    }
                    className={classNames(
                      "rounded-md px-3 py-2 text-xs font-medium",
                      adapterControlledPreviewReviewRecording ||
                        !adapterControlledPreviewReport.runId
                        ? "bg-stone-100 text-stone-400"
                        : decisionKind === "approve"
                          ? "bg-emerald-700 text-white hover:bg-emerald-800"
                          : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
                    )}
                  >
                    {label}
                  </button>
                ))}
              </div>

              {adapterControlledPreviewReviewError && (
                <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
                  {adapterControlledPreviewReviewError}
                </div>
              )}
              {adapterControlledPreviewReviewSummaryError && (
                <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
                  {adapterControlledPreviewReviewSummaryError}
                </div>
              )}

              {adapterControlledPreviewReviewResult && (
                <div className="mt-3 grid gap-2 md:grid-cols-3">
                  {[
                    ["recorded", String(adapterControlledPreviewReviewResult.recorded)],
                    ["evidenceId", adapterControlledPreviewReviewResult.evidenceId ?? "none"],
                    ["previewRunId", adapterControlledPreviewReviewResult.previewRunId],
                    ["decisionKind", adapterControlledPreviewReviewResult.decisionKind],
                    ["contractShape", adapterControlledPreviewReviewResult.contractShape],
                    [
                      "previewSummaryDigest",
                      adapterControlledPreviewReviewResult.previewSummaryDigest,
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
              )}

              {adapterControlledPreviewReviewResult?.blockingReasons.length ? (
                <div className="mt-3 space-y-1">
                  {adapterControlledPreviewReviewResult.blockingReasons.map(reason => (
                    <div
                      key={reason}
                      className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                    >
                      {reason}
                    </div>
                  ))}
                </div>
              ) : null}

              {adapterControlledPreviewReviewSummary && (
                <div className="mt-3 space-y-2">
                  <div className="grid gap-2 md:grid-cols-3">
                    {[
                      [
                        "approvedCount",
                        String(adapterControlledPreviewReviewSummary.approvedCount),
                      ],
                      [
                        "rejectOrReworkCount",
                        String(adapterControlledPreviewReviewSummary.rejectOrReworkCount),
                      ],
                      [
                        "latestTimestamp",
                        adapterControlledPreviewReviewSummary.latestTimestamp ?? "none",
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

                  {adapterControlledPreviewReviewSummary.latestDecision && (
                    <div className="grid gap-2 md:grid-cols-3">
                      {[
                        [
                          "latestEvidenceId",
                          adapterControlledPreviewReviewSummary.latestDecision.evidenceId,
                        ],
                        [
                          "previewRunId",
                          adapterControlledPreviewReviewSummary.latestDecision.previewRunId,
                        ],
                        [
                          "latestDecisionKind",
                          adapterControlledPreviewReviewSummary.latestDecision.decisionKind,
                        ],
                        [
                          "latestContractShape",
                          adapterControlledPreviewReviewSummary.latestDecision.contractShape,
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
                  )}

                  {safeSummaryEntries(adapterControlledPreviewReviewSummary.metadataSafeSummary)
                    .length > 0 && (
                    <div className="flex flex-wrap gap-2 text-xs">
                      {safeSummaryEntries(
                        adapterControlledPreviewReviewSummary.metadataSafeSummary
                      ).map(([key, value]) => (
                        <span
                          key={key}
                          className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                        >
                          {key}: {value}
                        </span>
                      ))}
                    </div>
                  )}
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

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Controlled Preview Approval Readiness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              W45 read-only gate over implementation readiness, human preview review approval, and
              the approved controlled preview AgentRun safety state.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterControlledPreviewApprovalReadinessCheck}
            disabled={adapterControlledPreviewApprovalReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterControlledPreviewApprovalReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
            )}
          >
            <ShieldCheck size={13} />
            {adapterControlledPreviewApprovalReadinessChecking
              ? "Checking..."
              : "Check Controlled Preview Approval Readiness"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This check does not run the controlled preview, save Chat messages, create review
          evidence, enable the adapter, or migrate default Chat.
        </div>

        {adapterControlledPreviewApprovalReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterControlledPreviewApprovalReadinessError}
          </div>
        )}

        {adapterControlledPreviewApprovalReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterControlledPreviewApprovalReadinessReport.ready
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterControlledPreviewApprovalReadinessReport.ready
                ? "Controlled preview approval readiness ready"
                : "Controlled preview approval readiness blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["ready", String(adapterControlledPreviewApprovalReadinessReport.ready)],
                [
                  "requiredApprovedPreviews",
                  String(adapterControlledPreviewApprovalReadinessReport.requiredApprovedPreviews),
                ],
                [
                  "approvedPreviewCount",
                  String(adapterControlledPreviewApprovalReadinessReport.approvedPreviewCount),
                ],
                [
                  "implementationReadinessReady",
                  String(
                    adapterControlledPreviewApprovalReadinessReport.implementationReadinessReady
                  ),
                ],
                [
                  "previewReviewApproved",
                  String(adapterControlledPreviewApprovalReadinessReport.previewReviewApproved),
                ],
                [
                  "previewDigestMatched",
                  String(adapterControlledPreviewApprovalReadinessReport.previewDigestMatched),
                ],
                [
                  "defaultChatUnchanged",
                  String(adapterControlledPreviewApprovalReadinessReport.defaultChatUnchanged),
                ],
                [
                  "controlledAdapterEnabled",
                  String(adapterControlledPreviewApprovalReadinessReport.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(adapterControlledPreviewApprovalReadinessReport.automaticMigrationEnabled),
                ],
                [
                  "defaultSendPath",
                  adapterControlledPreviewApprovalReadinessReport.defaultSendPath,
                ],
                [
                  "startStreamPath",
                  adapterControlledPreviewApprovalReadinessReport.startStreamPath,
                ],
                [
                  "verifiedPreviewRunIds",
                  adapterControlledPreviewApprovalReadinessReport.verifiedPreviewRunIds.length
                    ? adapterControlledPreviewApprovalReadinessReport.verifiedPreviewRunIds.join(
                        ", "
                      )
                    : "none",
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

            {adapterControlledPreviewApprovalReadinessReport.latestDecision && (
              <div className="grid gap-2 md:grid-cols-3">
                {[
                  [
                    "latestEvidenceId",
                    adapterControlledPreviewApprovalReadinessReport.latestDecision.evidenceId,
                  ],
                  [
                    "latestPreviewRunId",
                    adapterControlledPreviewApprovalReadinessReport.latestDecision.previewRunId,
                  ],
                  [
                    "latestDecisionKind",
                    adapterControlledPreviewApprovalReadinessReport.latestDecision.decisionKind,
                  ],
                  [
                    "latestContractShape",
                    adapterControlledPreviewApprovalReadinessReport.latestDecision.contractShape,
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
            )}

            {safeSummaryEntries(adapterControlledPreviewApprovalReadinessReport.metadataSafeSummary)
              .length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(
                  adapterControlledPreviewApprovalReadinessReport.metadataSafeSummary
                ).map(([key, value]) => (
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
              {adapterControlledPreviewApprovalReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterControlledPreviewApprovalReadinessReport.blockingReasons.map(reason => (
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
                  No controlled preview approval readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No controlled preview approval readiness report loaded.
          </div>
        )}
      </section>
    </>
  );
}

export function DefaultChatCutoverPanel(props: MultiStrategyPanelProps) {
  const {
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
    handleAdapterCutoverPlanApprovalReadinessCheck,
    handleAdapterCutoverPlanDraft,
    handleAdapterCutoverPlanReviewDecision,
    handleAdapterCutoverPlanReviewSummary,
    setAdapterCutoverPlanReviewNote,
  } = props;

  return (
    <>
      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Cutover Implementation Plan
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              W46 read-only draft over W45 approval readiness. It produces human-review planning
              material only and keeps default Chat on legacy stream.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterCutoverPlanDraft}
            disabled={adapterCutoverPlanDrafting}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterCutoverPlanDrafting
                ? "bg-stone-100 text-stone-400"
                : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
            )}
          >
            <ShieldCheck size={13} />
            {adapterCutoverPlanDrafting ? "Drafting..." : "Draft Cutover Implementation Plan"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This draft does not run controlled preview, runtime, tools, or model calls, and it does
          not save Chat messages, create evidence, enable routing, or change feature flags.
        </div>

        {adapterCutoverPlanError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterCutoverPlanError}
          </div>
        )}

        {adapterCutoverPlanDraft ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterCutoverPlanDraft.draftReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterCutoverPlanDraft.draftReady
                ? "Cutover implementation plan ready"
                : "Cutover implementation plan blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["draftReady", String(adapterCutoverPlanDraft.draftReady)],
                ["manualReviewRequired", String(adapterCutoverPlanDraft.manualReviewRequired)],
                ["notAutomaticMigration", String(adapterCutoverPlanDraft.notAutomaticMigration)],
                [
                  "requiresSeparateImplementation",
                  String(adapterCutoverPlanDraft.requiresSeparateImplementation),
                ],
                [
                  "requiresSeparateCutoverReview",
                  String(adapterCutoverPlanDraft.requiresSeparateCutoverReview),
                ],
                ["sourceSessionId", adapterCutoverPlanDraft.sourceSessionId],
                ["inputMessageLength", String(adapterCutoverPlanDraft.inputMessageLength)],
                ["inputMessageHash", adapterCutoverPlanDraft.inputMessageHash],
                ["stablePlanDigest", adapterCutoverPlanDraft.stablePlanDigest ?? "none"],
                [
                  "controlledPreviewApprovalReady",
                  String(adapterCutoverPlanDraft.controlledPreviewApprovalReadiness.ready),
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

            {adapterCutoverPlanDraft.planSections.length > 0 && (
              <div className="space-y-2">
                {adapterCutoverPlanDraft.planSections.map(section => (
                  <div
                    key={section.sectionKey}
                    className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2"
                  >
                    <div className="font-mono text-xs text-stone-500">{section.sectionKey}</div>
                    <div className="mt-1 text-sm font-medium text-stone-800">{section.title}</div>
                    <ul className="mt-2 list-disc space-y-1 pl-5 text-xs leading-5 text-stone-600">
                      {section.items.map(item => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  </div>
                ))}
              </div>
            )}

            {safeSummaryEntries(adapterCutoverPlanDraft.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterCutoverPlanDraft.metadataSafeSummary).map(
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
              {adapterCutoverPlanDraft.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {adapterCutoverPlanDraft.blockingReasons.map(reason => (
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
                  No cutover implementation plan blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 text-xs text-stone-500">
            No cutover implementation plan draft loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Cutover Plan Review
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              W47 records human review evidence over the W46 cutover implementation plan. It does
              not implement, enable, or route default Chat through the adapter.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterCutoverPlanReviewSummary}
            disabled={adapterCutoverPlanReviewSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterCutoverPlanReviewSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
            )}
          >
            <RefreshCw
              size={13}
              className={adapterCutoverPlanReviewSummaryChecking ? "animate-spin" : undefined}
            />
            {adapterCutoverPlanReviewSummaryChecking
              ? "Refreshing..."
              : "Refresh Cutover Plan Review"}
          </button>
        </div>

        <label className="mt-3 block text-xs font-medium text-stone-700">
          Cutover plan reviewer note
          <textarea
            value={adapterCutoverPlanReviewNote}
            onChange={event => setAdapterCutoverPlanReviewNote(event.target.value)}
            rows={2}
            className="mt-1 w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-800 focus:border-stone-400 focus:outline-none"
            placeholder="Optional note stored only as length/checksum/category metadata."
          />
        </label>

        <div className="mt-3 flex flex-wrap gap-2">
          {[
            ["approve", "Approve Cutover Plan"],
            ["reject", "Reject Cutover Plan"],
            ["request_rework", "Request Cutover Plan Rework"],
          ].map(([decisionKind, label]) => (
            <button
              key={decisionKind}
              type="button"
              onClick={() =>
                handleAdapterCutoverPlanReviewDecision(
                  decisionKind as DefaultChatAdapterCutoverPlanReviewDecisionKind
                )
              }
              disabled={adapterCutoverPlanReviewRecording}
              className={classNames(
                "rounded-md px-3 py-2 text-xs font-medium",
                adapterCutoverPlanReviewRecording
                  ? "bg-stone-100 text-stone-400"
                  : decisionKind === "approve"
                    ? "bg-stone-900 text-amber-50 hover:bg-stone-800"
                    : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
              )}
            >
              {label}
            </button>
          ))}
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          Approve is blocked unless the current W46 draft is ready. Reject and request rework can
          record metadata-safe review evidence for a blocked draft.
        </div>

        {adapterCutoverPlanReviewError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterCutoverPlanReviewError}
          </div>
        )}

        {adapterCutoverPlanReviewSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterCutoverPlanReviewSummaryError}
          </div>
        )}

        {adapterCutoverPlanReviewResult && (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterCutoverPlanReviewResult.recorded
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterCutoverPlanReviewResult.recorded
                ? "Cutover plan review recorded"
                : "Cutover plan review blocked"}
            </div>
            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["recorded", String(adapterCutoverPlanReviewResult.recorded)],
                ["decisionKind", adapterCutoverPlanReviewResult.decisionKind],
                ["sourceSessionId", adapterCutoverPlanReviewResult.sourceSessionId],
                ["draftReady", String(adapterCutoverPlanReviewResult.draftReady)],
                ["cutoverPlanDigest", adapterCutoverPlanReviewResult.cutoverPlanDigest ?? "none"],
                ["planSectionCount", String(adapterCutoverPlanReviewResult.planSectionCount)],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>
            {adapterCutoverPlanReviewResult.blockingReasons.length > 0 && (
              <div className="space-y-1">
                {adapterCutoverPlanReviewResult.blockingReasons.map(reason => (
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

        {adapterCutoverPlanReviewSummary && (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["approvedCount", String(adapterCutoverPlanReviewSummary.approvedCount)],
                ["rejectedCount", String(adapterCutoverPlanReviewSummary.rejectedCount)],
                ["requestReworkCount", String(adapterCutoverPlanReviewSummary.requestReworkCount)],
                [
                  "latestApprovedPlanDigest",
                  adapterCutoverPlanReviewSummary.latestApprovedPlanDigest ?? "none",
                ],
                [
                  "latestDecisionKind",
                  adapterCutoverPlanReviewSummary.latestDecision?.decisionKind ?? "none",
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

            {safeSummaryEntries(adapterCutoverPlanReviewSummary.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(adapterCutoverPlanReviewSummary.metadataSafeSummary).map(
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

            {adapterCutoverPlanReviewSummary.blockingReasons.length > 0 && (
              <div className="space-y-1">
                {adapterCutoverPlanReviewSummary.blockingReasons.map(reason => (
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
              Default Chat Adapter Cutover Plan Approval Readiness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              W48 is a read-only gate over the current W46 plan draft and W47 human review evidence.
              Ready means later adapter implementation discussion only.
            </div>
          </div>
          <button
            type="button"
            onClick={handleAdapterCutoverPlanApprovalReadinessCheck}
            disabled={adapterCutoverPlanApprovalReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              adapterCutoverPlanApprovalReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "border border-stone-200 bg-white text-stone-700 hover:bg-stone-50"
            )}
          >
            <RefreshCw
              size={13}
              className={adapterCutoverPlanApprovalReadinessChecking ? "animate-spin" : undefined}
            />
            {adapterCutoverPlanApprovalReadinessChecking
              ? "Checking..."
              : "Check Cutover Plan Approval"}
          </button>
        </div>

        {adapterCutoverPlanApprovalReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {adapterCutoverPlanApprovalReadinessError}
          </div>
        )}

        {adapterCutoverPlanApprovalReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                adapterCutoverPlanApprovalReadinessReport.ready
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {adapterCutoverPlanApprovalReadinessReport.ready
                ? "Cutover plan approval ready"
                : "Cutover plan approval blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["ready", String(adapterCutoverPlanApprovalReadinessReport.ready)],
                ["draftReady", String(adapterCutoverPlanApprovalReadinessReport.draftReady)],
                ["w45Ready", String(adapterCutoverPlanApprovalReadinessReport.w45Ready)],
                [
                  "cutoverPlanReviewApproved",
                  String(adapterCutoverPlanApprovalReadinessReport.cutoverPlanReviewApproved),
                ],
                [
                  "cutoverPlanDigestMatched",
                  String(adapterCutoverPlanApprovalReadinessReport.cutoverPlanDigestMatched),
                ],
                [
                  "currentPlanDigest",
                  adapterCutoverPlanApprovalReadinessReport.currentPlanDigest ?? "none",
                ],
                [
                  "latestApprovedPlanDigest",
                  adapterCutoverPlanApprovalReadinessReport.latestApprovedPlanDigest ?? "none",
                ],
                [
                  "latestDecisionKind",
                  adapterCutoverPlanApprovalReadinessReport.latestDecision?.decisionKind ?? "none",
                ],
                [
                  "defaultChatUnchanged",
                  String(adapterCutoverPlanApprovalReadinessReport.defaultChatUnchanged),
                ],
                [
                  "controlledAdapterEnabled",
                  String(adapterCutoverPlanApprovalReadinessReport.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(adapterCutoverPlanApprovalReadinessReport.automaticMigrationEnabled),
                ],
                ["defaultSendPath", adapterCutoverPlanApprovalReadinessReport.defaultSendPath],
                ["startStreamPath", adapterCutoverPlanApprovalReadinessReport.startStreamPath],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(adapterCutoverPlanApprovalReadinessReport.metadataSafeSummary)
              .length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(
                  adapterCutoverPlanApprovalReadinessReport.metadataSafeSummary
                ).map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-stone-200 bg-white px-2 py-1 text-stone-700"
                  >
                    {key}: {value}
                  </span>
                ))}
              </div>
            )}

            {adapterCutoverPlanApprovalReadinessReport.blockingReasons.length > 0 ? (
              <div className="space-y-1">
                {adapterCutoverPlanApprovalReadinessReport.blockingReasons.map(reason => (
                  <div
                    key={reason}
                    className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                  >
                    {reason}
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                No cutover plan approval blockers returned.
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
            No cutover plan approval readiness report loaded.
          </div>
        )}
      </section>
    </>
  );
}

export function NarrowImplementationPanel(props: MultiStrategyPanelProps) {
  const {
    handleNarrowImplementationGateCheck,
    handleNarrowImplementationPlanApprovalReadinessCheck,
    handleNarrowImplementationPlanDraft,
    handleNarrowImplementationPlanReviewSummaryRefresh,
    handleRecordNarrowImplementationPlanReviewDecision,
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
    setNarrowImplementationPlanReviewNote,
  } = props;

  return (
    <>
      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Narrow Implementation Discussion Gate
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W57 gate over W48 cutover plan approval and W56 ordinary-entry preflight
              status. Eligible means discussion-ready only, not a routing change.
            </div>
          </div>
          <button
            type="button"
            onClick={handleNarrowImplementationGateCheck}
            disabled={narrowImplementationGateChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              narrowImplementationGateChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={narrowImplementationGateChecking ? "animate-spin" : undefined}
            />
            {narrowImplementationGateChecking ? "Checking..." : "Check Narrow Implementation Gate"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This gate does not call runtime, tools, models, previews, evidence recorders, or routing
          toggles. It only combines existing readiness and preflight status.
        </div>

        {narrowImplementationGateError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {narrowImplementationGateError}
          </div>
        )}

        {narrowImplementationGateReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                narrowImplementationGateReport.eligible
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {narrowImplementationGateReport.eligible
                ? "Narrow implementation discussion eligible"
                : "Narrow implementation discussion blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["eligible", String(narrowImplementationGateReport.eligible)],
                [
                  "defaultChatUnchanged",
                  String(narrowImplementationGateReport.defaultChatUnchanged),
                ],
                [
                  "cutoverPlanApprovalReady",
                  String(narrowImplementationGateReport.cutoverPlanApprovalReady),
                ],
                [
                  "ordinaryEntryPreflightStatusReady",
                  String(narrowImplementationGateReport.ordinaryEntryPreflightStatusReady),
                ],
                ["sendPreflightReady", String(narrowImplementationGateReport.sendPreflightReady)],
                [
                  "streamPreflightReady",
                  String(narrowImplementationGateReport.streamPreflightReady),
                ],
                [
                  "controlledAdapterEnabled",
                  String(narrowImplementationGateReport.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(narrowImplementationGateReport.automaticMigrationEnabled),
                ],
                ["defaultSendPath", narrowImplementationGateReport.defaultSendPath],
                ["startStreamPath", narrowImplementationGateReport.startStreamPath],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(narrowImplementationGateReport.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(narrowImplementationGateReport.metadataSafeSummary).map(
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

            {narrowImplementationGateReport.blockingReasons.length > 0 ? (
              <div className="space-y-1">
                {narrowImplementationGateReport.blockingReasons.map(reason => (
                  <div
                    key={reason}
                    className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                  >
                    {reason}
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                No narrow implementation gate blockers returned.
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
            No narrow implementation gate report loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Narrow Implementation Plan
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W58 draft over the W57 discussion gate. Ready means human-review planning
              material only; default Chat remains on legacy_stream.
            </div>
          </div>
          <button
            type="button"
            onClick={handleNarrowImplementationPlanDraft}
            disabled={narrowImplementationPlanDrafting}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              narrowImplementationPlanDrafting
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={narrowImplementationPlanDrafting ? "animate-spin" : undefined}
            />
            {narrowImplementationPlanDrafting ? "Drafting..." : "Draft Narrow Implementation Plan"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          This draft creates no records, calls no runtime, tools, models, previews, evidence
          recorders, or routing toggles. Blocked drafts return no plan sections.
        </div>

        {narrowImplementationPlanError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {narrowImplementationPlanError}
          </div>
        )}

        {narrowImplementationPlanDraft ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                narrowImplementationPlanDraft.draftReady
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-red-100 bg-red-50 text-red-700"
              )}
            >
              {narrowImplementationPlanDraft.draftReady
                ? "Narrow implementation plan ready for human review"
                : "Narrow implementation plan blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["draftReady", String(narrowImplementationPlanDraft.draftReady)],
                [
                  "discussionGateEligible",
                  String(narrowImplementationPlanDraft.discussionGate.eligible),
                ],
                ["sourceSessionId", narrowImplementationPlanDraft.sourceSessionId],
                ["inputMessageLength", String(narrowImplementationPlanDraft.inputMessageLength)],
                ["inputMessageHash", narrowImplementationPlanDraft.inputMessageHash],
                ["stablePlanDigest", narrowImplementationPlanDraft.stablePlanDigest ?? "none"],
                [
                  "manualReviewRequired",
                  String(narrowImplementationPlanDraft.manualReviewRequired),
                ],
                [
                  "notAutomaticMigration",
                  String(narrowImplementationPlanDraft.notAutomaticMigration),
                ],
                [
                  "requiresSeparateImplementation",
                  String(narrowImplementationPlanDraft.requiresSeparateImplementation),
                ],
                [
                  "requiresSeparateCutoverReview",
                  String(narrowImplementationPlanDraft.requiresSeparateCutoverReview),
                ],
                [
                  "defaultChatUnchanged",
                  String(narrowImplementationPlanDraft.discussionGate.defaultChatUnchanged),
                ],
                ["defaultSendPath", narrowImplementationPlanDraft.discussionGate.defaultSendPath],
                ["startStreamPath", narrowImplementationPlanDraft.discussionGate.startStreamPath],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700"
                >
                  {label}: {value}
                </div>
              ))}
            </div>

            {safeSummaryEntries(narrowImplementationPlanDraft.metadataSafeSummary).length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(narrowImplementationPlanDraft.metadataSafeSummary).map(
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

            {narrowImplementationPlanDraft.planSections.length > 0 ? (
              <div className="grid gap-3 md:grid-cols-2">
                {narrowImplementationPlanDraft.planSections.map(section => (
                  <div
                    key={section.sectionKey}
                    className="rounded-md border border-stone-100 bg-stone-50 p-3"
                  >
                    <div className="text-xs font-semibold text-stone-900">{section.title}</div>
                    <ul className="mt-2 space-y-1 text-xs leading-5 text-stone-600">
                      {section.items.map(item => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                No narrow implementation plan sections returned.
              </div>
            )}

            {narrowImplementationPlanDraft.blockingReasons.length > 0 ? (
              <div className="space-y-1">
                {narrowImplementationPlanDraft.blockingReasons.map(reason => (
                  <div
                    key={reason}
                    className="rounded-md border border-red-100 bg-red-50 px-2 py-1 text-xs text-red-700"
                  >
                    {reason}
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
                No narrow implementation plan blockers returned.
              </div>
            )}
          </div>
        ) : (
          <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
            No narrow implementation plan draft loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Narrow Implementation Plan Review
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Explicit W59 human review evidence for the W58 narrow implementation plan draft. It
              records approve, reject, or request rework metadata only; it does not implement,
              route, activate, or migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleNarrowImplementationPlanReviewSummaryRefresh}
            disabled={narrowImplementationPlanReviewSummaryChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              narrowImplementationPlanReviewSummaryChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={narrowImplementationPlanReviewSummaryChecking ? "animate-spin" : undefined}
            />
            {narrowImplementationPlanReviewSummaryChecking
              ? "Refreshing..."
              : "Refresh Narrow Plan Review"}
          </button>
        </div>

        <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs leading-5 text-stone-600">
          Reviewer notes are never stored as raw text. Only checksum, length, and bounded category
          metadata are persisted when a decision is recorded.
        </div>

        <div className="mt-4 space-y-3">
          <label className="block">
            <span className="text-xs font-medium text-stone-700">
              Narrow implementation reviewer note
            </span>
            <textarea
              value={narrowImplementationPlanReviewNote}
              onChange={event => setNarrowImplementationPlanReviewNote(event.target.value)}
              rows={3}
              className="mt-1 w-full rounded-md border border-stone-200 bg-white px-3 py-2 text-sm text-stone-800 outline-none focus:border-stone-500"
              placeholder="Optional narrow plan reviewer note; only metadata is stored."
            />
          </label>

          <div className="flex flex-wrap gap-2">
            {[
              ["approve", "Approve Narrow Plan"],
              ["reject", "Reject Narrow Plan"],
              ["request_rework", "Request Narrow Plan Rework"],
            ].map(([decisionKind, label]) => (
              <button
                key={decisionKind}
                type="button"
                onClick={() =>
                  handleRecordNarrowImplementationPlanReviewDecision(
                    decisionKind as DefaultChatAdapterNarrowImplementationPlanReviewDecisionKind
                  )
                }
                disabled={narrowImplementationPlanReviewRecording}
                className={classNames(
                  "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
                  narrowImplementationPlanReviewRecording
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

        {narrowImplementationPlanReviewError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {narrowImplementationPlanReviewError}
          </div>
        )}

        {narrowImplementationPlanReviewSummaryError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {narrowImplementationPlanReviewSummaryError}
          </div>
        )}

        {narrowImplementationPlanReviewResult && (
          <div className="mt-4 space-y-2 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
            <div className="font-medium text-stone-900">
              {narrowImplementationPlanReviewResult.recorded
                ? "Narrow implementation plan review recorded"
                : "Narrow implementation plan review blocked"}
            </div>
            <div>recorded: {String(narrowImplementationPlanReviewResult.recorded)}</div>
            <div>decisionKind: {narrowImplementationPlanReviewResult.decisionKind}</div>
            <div>sourceSessionId: {narrowImplementationPlanReviewResult.sourceSessionId}</div>
            <div>draftReady: {String(narrowImplementationPlanReviewResult.draftReady)}</div>
            <div>planSectionCount: {narrowImplementationPlanReviewResult.planSectionCount}</div>
            <div className="break-all">
              narrowPlanDigest: {narrowImplementationPlanReviewResult.narrowPlanDigest ?? "none"}
            </div>
            {narrowImplementationPlanReviewResult.evidenceId && (
              <div>evidenceId: {narrowImplementationPlanReviewResult.evidenceId}</div>
            )}
            {narrowImplementationPlanReviewResult.blockingReasons.length > 0 && (
              <div className="space-y-1">
                {narrowImplementationPlanReviewResult.blockingReasons.map(reason => (
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

        {narrowImplementationPlanReviewSummary ? (
          <div className="mt-4 space-y-3">
            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["approvedCount", String(narrowImplementationPlanReviewSummary.approvedCount)],
                ["rejectedCount", String(narrowImplementationPlanReviewSummary.rejectedCount)],
                [
                  "requestReworkCount",
                  String(narrowImplementationPlanReviewSummary.requestReworkCount),
                ],
                [
                  "latestApprovedPlanDigest",
                  narrowImplementationPlanReviewSummary.latestApprovedPlanDigest ?? "none",
                ],
                [
                  "latestTimestamp",
                  narrowImplementationPlanReviewSummary.latestTimestamp ?? "none",
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

            {narrowImplementationPlanReviewSummary.latestDecision && (
              <div className="rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-700">
                <div className="font-medium text-stone-900">Latest decision</div>
                <div className="mt-1">
                  latestDecisionKind:{" "}
                  {narrowImplementationPlanReviewSummary.latestDecision.decisionKind}
                </div>
                <div>
                  draftReady:{" "}
                  {String(narrowImplementationPlanReviewSummary.latestDecision.draftReady)}
                </div>
                <div>
                  w57Eligible:{" "}
                  {String(narrowImplementationPlanReviewSummary.latestDecision.w57Eligible)}
                </div>
                <div className="break-all">
                  latestNarrowPlanDigest:{" "}
                  {narrowImplementationPlanReviewSummary.latestDecision.narrowPlanDigest ?? "none"}
                </div>
                <div>
                  reviewerNoteCategory:{" "}
                  {narrowImplementationPlanReviewSummary.latestDecision.reviewerNoteCategory}
                </div>
                <div>
                  reviewerNoteLength:{" "}
                  {narrowImplementationPlanReviewSummary.latestDecision.reviewerNoteLength}
                </div>
              </div>
            )}

            {safeSummaryEntries(narrowImplementationPlanReviewSummary.metadataSafeSummary).length >
              0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(narrowImplementationPlanReviewSummary.metadataSafeSummary).map(
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
              {narrowImplementationPlanReviewSummary.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {narrowImplementationPlanReviewSummary.blockingReasons.map(reason => (
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
                  No narrow implementation plan review blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
            No narrow implementation plan review summary loaded.
          </div>
        )}
      </section>

      <section className="rounded-lg border border-stone-200 bg-white p-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="text-sm font-semibold text-stone-900">
              Default Chat Adapter Narrow Implementation Plan Approval Readiness
            </div>
            <div className="mt-1 max-w-xl text-xs leading-5 text-stone-600">
              Read-only W60 gate over the current W58 narrow implementation plan draft and W59
              review approval. A pass only supports separate narrow implementation discussion; it
              does not route, activate, or migrate default Chat.
            </div>
          </div>
          <button
            type="button"
            onClick={handleNarrowImplementationPlanApprovalReadinessCheck}
            disabled={narrowImplementationPlanApprovalReadinessChecking}
            className={classNames(
              "inline-flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium",
              narrowImplementationPlanApprovalReadinessChecking
                ? "bg-stone-100 text-stone-400"
                : "bg-stone-900 text-amber-50 hover:bg-stone-800"
            )}
          >
            <RefreshCw
              size={13}
              className={
                narrowImplementationPlanApprovalReadinessChecking ? "animate-spin" : undefined
              }
            />
            {narrowImplementationPlanApprovalReadinessChecking
              ? "Checking..."
              : "Check Narrow Plan Approval Readiness"}
          </button>
        </div>

        {narrowImplementationPlanApprovalReadinessError && (
          <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
            {narrowImplementationPlanApprovalReadinessError}
          </div>
        )}

        {narrowImplementationPlanApprovalReadinessReport ? (
          <div className="mt-4 space-y-3">
            <div
              className={classNames(
                "rounded-md border px-3 py-2 text-sm font-medium",
                narrowImplementationPlanApprovalReadinessReport.ready
                  ? "border-emerald-100 bg-emerald-50 text-emerald-800"
                  : "border-amber-100 bg-amber-50 text-amber-800"
              )}
            >
              {narrowImplementationPlanApprovalReadinessReport.ready
                ? "Narrow plan approval readiness passed"
                : "Narrow plan approval readiness blocked"}
            </div>

            <div className="grid gap-2 md:grid-cols-3">
              {[
                ["ready", String(narrowImplementationPlanApprovalReadinessReport.ready)],
                ["draftReady", String(narrowImplementationPlanApprovalReadinessReport.draftReady)],
                [
                  "discussionGateEligible",
                  String(narrowImplementationPlanApprovalReadinessReport.discussionGateEligible),
                ],
                [
                  "narrowPlanReviewApproved",
                  String(narrowImplementationPlanApprovalReadinessReport.narrowPlanReviewApproved),
                ],
                [
                  "narrowPlanDigestMatched",
                  String(narrowImplementationPlanApprovalReadinessReport.narrowPlanDigestMatched),
                ],
                [
                  "defaultChatUnchanged",
                  String(narrowImplementationPlanApprovalReadinessReport.defaultChatUnchanged),
                ],
                [
                  "controlledAdapterEnabled",
                  String(narrowImplementationPlanApprovalReadinessReport.controlledAdapterEnabled),
                ],
                [
                  "automaticMigrationEnabled",
                  String(narrowImplementationPlanApprovalReadinessReport.automaticMigrationEnabled),
                ],
                [
                  "defaultSendPath",
                  narrowImplementationPlanApprovalReadinessReport.defaultSendPath,
                ],
                [
                  "startStreamPath",
                  narrowImplementationPlanApprovalReadinessReport.startStreamPath,
                ],
                [
                  "latestDecisionKind",
                  narrowImplementationPlanApprovalReadinessReport.latestDecision?.decisionKind ??
                    "none",
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

            <div className="space-y-2">
              <div className="break-all rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700">
                currentPlanDigest:{" "}
                {narrowImplementationPlanApprovalReadinessReport.currentPlanDigest ?? "none"}
              </div>
              <div className="break-all rounded-md border border-stone-100 bg-stone-50 px-3 py-2 font-mono text-xs text-stone-700">
                latestApprovedPlanDigest:{" "}
                {narrowImplementationPlanApprovalReadinessReport.latestApprovedPlanDigest ?? "none"}
              </div>
            </div>

            {safeSummaryEntries(narrowImplementationPlanApprovalReadinessReport.metadataSafeSummary)
              .length > 0 && (
              <div className="flex flex-wrap gap-2 text-xs">
                {safeSummaryEntries(
                  narrowImplementationPlanApprovalReadinessReport.metadataSafeSummary
                ).map(([key, value]) => (
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
              {narrowImplementationPlanApprovalReadinessReport.blockingReasons.length > 0 ? (
                <div className="mt-1 space-y-1">
                  {narrowImplementationPlanApprovalReadinessReport.blockingReasons.map(reason => (
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
                  No narrow implementation plan approval readiness blockers returned.
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="mt-3 rounded-md border border-stone-100 bg-stone-50 px-3 py-2 text-xs text-stone-600">
            No narrow implementation plan approval readiness report loaded.
          </div>
        )}
      </section>
    </>
  );
}
