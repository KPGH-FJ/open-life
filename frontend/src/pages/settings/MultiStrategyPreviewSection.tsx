import { type FormEvent, useState } from "react";
import {
  checkControlledChatCutoverCandidatePromotionReadiness,
  checkControlledChatCutoverReadiness,
  checkControlledChatMigrationImplementationGate,
  checkControlledChatPilotEligibility,
  checkControlledPilotPromotionReadiness,
  checkRuntimeMigrationGate,
  getMainChatRuntimeStatus,
  runMultiStrategyAgentPreview,
  type MainChatRuntimeStatus,
} from "../../tauri";
import type {
  ControlledChatCutoverCandidatePromotionReadinessReport,
  ControlledChatCutoverReadinessReport,
  ControlledChatMigrationImplementationGateReport,
  ControlledChatPilotEligibilityReport,
  ControlledPilotPromotionReadinessReport,
  MultiStrategyAgentPreviewLayer,
  MultiStrategyAgentPreviewOutput,
  RuntimeMigrationGateReport,
} from "../../types";
import { isInternalDebugSurfaceEnabled } from "../../utils/internalDebug";

const NO_TOOLS_PROMPT = "No developer tools catalog supplied for this preview.";

type RuntimeDebugReport =
  | MainChatRuntimeStatus
  | RuntimeMigrationGateReport
  | ControlledChatPilotEligibilityReport
  | ControlledPilotPromotionReadinessReport
  | ControlledChatMigrationImplementationGateReport
  | ControlledChatCutoverReadinessReport
  | ControlledChatCutoverCandidatePromotionReadinessReport
  | MultiStrategyAgentPreviewOutput;

function readableError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  return JSON.stringify(error);
}

function ReportBlock({ title, report }: { title: string; report: RuntimeDebugReport | null }) {
  if (!report) {
    return null;
  }

  return (
    <section className="rounded-lg border border-stone-200 bg-white p-3">
      <h4 className="text-xs font-semibold uppercase tracking-wide text-stone-500">{title}</h4>
      <pre className="mt-2 max-h-72 overflow-auto rounded-md bg-stone-950 p-3 text-xs leading-relaxed text-stone-100">
        {JSON.stringify(report, null, 2)}
      </pre>
    </section>
  );
}

export default function MultiStrategyPreviewSection() {
  if (!isInternalDebugSurfaceEnabled()) {
    return null;
  }

  return <MultiStrategyPreviewSectionInner />;
}

function MultiStrategyPreviewSectionInner() {
  const [open, setOpen] = useState(false);
  const [userText, setUserText] = useState("");
  const [allowPlanning, setAllowPlanning] = useState(false);
  const [localModelAvailable, setLocalModelAvailable] = useState(false);
  const [layer, setLayer] = useState<MultiStrategyAgentPreviewLayer>("L2");
  const [toolsPrompt, setToolsPrompt] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [actionRunning, setActionRunning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<MultiStrategyAgentPreviewOutput | null>(null);
  const [runtimeStatus, setRuntimeStatus] = useState<MainChatRuntimeStatus | null>(null);
  const [migrationGate, setMigrationGate] = useState<RuntimeMigrationGateReport | null>(null);
  const [pilotEligibility, setPilotEligibility] =
    useState<ControlledChatPilotEligibilityReport | null>(null);
  const [promotionReadiness, setPromotionReadiness] =
    useState<ControlledPilotPromotionReadinessReport | null>(null);
  const [implementationGate, setImplementationGate] =
    useState<ControlledChatMigrationImplementationGateReport | null>(null);
  const [cutoverReadiness, setCutoverReadiness] =
    useState<ControlledChatCutoverReadinessReport | null>(null);
  const [candidatePromotionReadiness, setCandidatePromotionReadiness] =
    useState<ControlledChatCutoverCandidatePromotionReadinessReport | null>(null);

  const handlePreviewSubmit = async (event: FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      setPreview(
        await runMultiStrategyAgentPreview({
          sessionId: "settings-runtime-debug-preview",
          userText,
          layer,
          allowPlanning,
          localModelAvailable,
          toolsPrompt: toolsPrompt.trim() || NO_TOOLS_PROMPT,
        })
      );
    } catch (err) {
      setError(readableError(err));
    } finally {
      setSubmitting(false);
    }
  };

  const runAction = async (label: string, action: () => Promise<void>) => {
    setActionRunning(label);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(readableError(err));
    } finally {
      setActionRunning(null);
    }
  };

  return (
    <section className="rounded-xl border border-stone-200 bg-stone-50 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold text-stone-900">Main Chat Runtime Debug</h3>
          <p className="mt-1 text-xs text-stone-500">
            Current kernel-backed route evidence and controlled migration gates.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setOpen(value => !value)}
          className="rounded-md border border-stone-300 bg-white px-3 py-1.5 text-xs font-medium text-stone-700 hover:bg-stone-100"
        >
          {open ? "Hide" : "Show"}
        </button>
      </div>

      {open ? (
        <div className="mt-4 space-y-4">
          <form className="space-y-3" onSubmit={handlePreviewSubmit}>
            <label
              className="block text-xs font-medium text-stone-600"
              htmlFor="runtime-preview-input"
            >
              Preview input
            </label>
            <textarea
              id="runtime-preview-input"
              value={userText}
              onChange={event => setUserText(event.target.value)}
              rows={4}
              className="w-full rounded-md border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 shadow-sm focus:border-stone-500 focus:outline-none"
              placeholder="Ask a representative Main Chat question"
            />
            <div className="grid gap-3 md:grid-cols-3">
              <label className="text-xs font-medium text-stone-600">
                Layer
                <select
                  value={layer}
                  onChange={event => setLayer(event.target.value as MultiStrategyAgentPreviewLayer)}
                  className="mt-1 w-full rounded-md border border-stone-300 bg-white px-2 py-1.5 text-sm"
                >
                  <option value="L1">L1</option>
                  <option value="L2">L2</option>
                  <option value="L3">L3</option>
                </select>
              </label>
              <label className="flex items-center gap-2 text-xs font-medium text-stone-600">
                <input
                  type="checkbox"
                  checked={allowPlanning}
                  onChange={event => setAllowPlanning(event.target.checked)}
                />
                Allow planning
              </label>
              <label className="flex items-center gap-2 text-xs font-medium text-stone-600">
                <input
                  type="checkbox"
                  checked={localModelAvailable}
                  onChange={event => setLocalModelAvailable(event.target.checked)}
                />
                Local model available
              </label>
            </div>
            <textarea
              value={toolsPrompt}
              onChange={event => setToolsPrompt(event.target.value)}
              rows={2}
              className="w-full rounded-md border border-stone-300 bg-white px-3 py-2 text-sm text-stone-900 shadow-sm focus:border-stone-500 focus:outline-none"
              placeholder={NO_TOOLS_PROMPT}
            />
            <button
              type="submit"
              disabled={submitting}
              className="rounded-md bg-stone-900 px-3 py-2 text-xs font-semibold text-white disabled:opacity-50"
            >
              {submitting ? "Running preview" : "Run preview"}
            </button>
          </form>

          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() =>
                runAction("runtime", async () => setRuntimeStatus(await getMainChatRuntimeStatus()))
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Runtime status
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("migration", async () =>
                  setMigrationGate(await checkRuntimeMigrationGate())
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Migration gate
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("pilot", async () =>
                  setPilotEligibility(await checkControlledChatPilotEligibility())
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Pilot eligibility
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("promotion", async () =>
                  setPromotionReadiness(await checkControlledPilotPromotionReadiness())
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Promotion readiness
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("implementation", async () =>
                  setImplementationGate(await checkControlledChatMigrationImplementationGate())
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Implementation gate
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("cutover", async () =>
                  setCutoverReadiness(await checkControlledChatCutoverReadiness())
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Cutover readiness
            </button>
            <button
              type="button"
              onClick={() =>
                runAction("candidate", async () =>
                  setCandidatePromotionReadiness(
                    await checkControlledChatCutoverCandidatePromotionReadiness()
                  )
                )
              }
              className="rounded-md border border-stone-300 bg-white px-3 py-2 text-xs font-medium"
            >
              Candidate promotion
            </button>
          </div>

          {actionRunning ? (
            <p className="text-xs text-stone-500">Running {actionRunning}...</p>
          ) : null}
          {error ? (
            <div className="rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700">
              {error}
            </div>
          ) : null}

          <ReportBlock title="Preview" report={preview} />
          <ReportBlock title="Runtime Status" report={runtimeStatus} />
          <ReportBlock title="Migration Gate" report={migrationGate} />
          <ReportBlock title="Pilot Eligibility" report={pilotEligibility} />
          <ReportBlock title="Promotion Readiness" report={promotionReadiness} />
          <ReportBlock title="Implementation Gate" report={implementationGate} />
          <ReportBlock title="Cutover Readiness" report={cutoverReadiness} />
          <ReportBlock title="Candidate Promotion" report={candidatePromotionReadiness} />
        </div>
      ) : null}
    </section>
  );
}
