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
  checkControlledChatPilotEligibility,
  checkControlledChatMigrationImplementationGate,
  checkControlledPilotPromotionReadiness,
  checkRuntimeMigrationGate,
  draftControlledChatMigrationPlan,
  getControlledChatMigrationReviewDecisionSummary,
  getControlledPilotPromotionEvidenceSummary,
  recordControlledChatMigrationReviewDecision,
  runMultiStrategyAgentPreview,
} from "../../tauri";
import type {
  ControlledChatMigrationImplementationGateReport,
  ControlledChatMigrationPlanDraft,
  ControlledChatMigrationReviewDecisionKind,
  ControlledChatMigrationReviewDecisionResult,
  ControlledChatMigrationReviewDecisionSummary,
  ControlledChatPilotEligibilityReport,
  ControlledPilotPromotionEvidenceSummary,
  ControlledPilotPromotionReadinessReport,
  MultiStrategyAgentPreviewLayer,
  MultiStrategyAgentPreviewOutput,
  RuntimeMigrationGateReport,
} from "../../types";

const NO_TOOLS_PROMPT = "No developer tools catalog supplied for this preview.";
const SAFE_SUMMARY_KEYS = [
  "taskKind",
  "reasonCode",
  "riskLevel",
  "hasHsPacket",
  "policyReasonCode",
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
