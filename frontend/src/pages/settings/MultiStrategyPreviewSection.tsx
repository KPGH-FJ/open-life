import { type FormEvent, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Play,
  ShieldCheck,
} from "lucide-react";
import { runMultiStrategyAgentPreview } from "../../tauri";
import type { MultiStrategyAgentPreviewLayer, MultiStrategyAgentPreviewOutput } from "../../types";

const NO_TOOLS_PROMPT = "No developer tools catalog supplied for this preview.";
const SAFE_SUMMARY_KEYS = [
  "taskKind",
  "reasonCode",
  "riskLevel",
  "hasHsPacket",
  "policyReasonCode",
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
