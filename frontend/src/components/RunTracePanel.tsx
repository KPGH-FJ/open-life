import { CheckCircle2, CircleAlert, ShieldCheck, Sparkles, Wrench } from "lucide-react";
import { Link } from "react-router-dom";
import type {
  ProductAgentRun,
  ProductHSBehaviorCheckSummary,
  ProductHSSelectionAudit,
  ProductReactActionTrace,
} from "../tauri";
import { mailboxRoute } from "../productShellContract";

interface Props {
  run?: ProductAgentRun | null;
}

function selectedPolicies(audit?: ProductHSSelectionAudit): string[] {
  return audit?.selectedPolicyIds ?? [];
}

function selectedStyles(audit?: ProductHSSelectionAudit): string[] {
  return audit?.selectedHeuristicIds ?? [];
}

function tokenText(audit?: ProductHSSelectionAudit): string | null {
  if (!audit) return null;
  const estimated = audit.estimatedTokens;
  const budget = audit.tokenBudget;
  if (estimated === undefined || budget === undefined) return null;
  return `${estimated}/${budget} tokens`;
}

function policyLabel(id: string): string {
  if (id.includes("external_writes.proposal_first")) return "Confirm before external writes";
  if (id.includes("sensitive_topics.local_only")) return "Local-only for sensitive topics";
  return id
    .replace(/^policy\./, "")
    .replace(/[._-]+/g, " ")
    .replace(/\b\w/g, char => char.toUpperCase());
}

function styleLabel(id: string): string {
  if (id.includes("low_energy_planning")) return "Low-energy planning style";
  if (id.includes("rejected_reminder_delay")) return "Delay repeated reminders after rejection";
  return id
    .replace(/^heuristic\./, "")
    .replace(/[._-]+/g, " ")
    .replace(/\b\w/g, char => char.toUpperCase());
}

function collectBehaviorChecks(run?: ProductAgentRun | null): ProductHSBehaviorCheckSummary[] {
  return run?.behaviorChecks ?? [];
}

function collectReactActionTraces(run?: ProductAgentRun | null): ProductReactActionTrace[] {
  const byAction = new Map<string, ProductReactActionTrace>();
  for (const action of run?.actions ?? []) {
    if (action.reactTrace?.metadataSafe) {
      byAction.set(action.reactTrace.actionId, action.reactTrace);
    }
  }
  for (const observation of run?.observations ?? []) {
    if (observation.reactTrace?.metadataSafe && !byAction.has(observation.reactTrace.actionId)) {
      byAction.set(observation.reactTrace.actionId, observation.reactTrace);
    }
  }
  return [...byAction.values()].sort((a, b) => a.toolCallIndex - b.toolCallIndex);
}

export default function RunTracePanel({ run }: Props) {
  if (run?.legacyPayloadUnverified) {
    return (
      <section className="rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm text-amber-900">
        Legacy collaboration, tool, and strategy metadata is unverified and is not shown as runtime
        fact.
      </section>
    );
  }
  const audit = run?.hsSelectionAudit;
  const policies = selectedPolicies(audit);
  const styles = selectedStyles(audit);
  const checks = collectBehaviorChecks(run);
  const reactActionTraces = collectReactActionTraces(run);
  const hasCollaborationContent = policies.length > 0 || styles.length > 0 || checks.length > 0;
  const hasReactActionContent = reactActionTraces.length > 0;
  const hasContent = hasCollaborationContent || hasReactActionContent;

  if (!hasContent) {
    return (
      <section className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-600">
        Verified runtime trace is unavailable; execution details remain unknown.
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-emerald-100 bg-emerald-50 p-3 text-sm text-emerald-950">
      {hasReactActionContent && (
        <div className="rounded-lg bg-white/80 p-3 text-stone-800">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2 font-semibold text-stone-900">
              <Wrench size={16} />
              ReAct action lifecycle
            </div>
            <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] text-emerald-700">
              metadata-safe
            </span>
          </div>

          <div className="mt-2 space-y-2">
            {reactActionTraces.map(trace => (
              <div
                key={`${trace.actionId}-${trace.observationId ?? "no-observation"}`}
                className="rounded border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-900">{trace.toolName}</span>
                  <span>Status: {trace.status}</span>
                  <span>Source: {trace.toolSource}</span>
                  <span>Risk: {trace.riskLevel}</span>
                  <span>Category: {trace.actionCategory}</span>
                </div>
                <div className="mt-1 flex flex-wrap gap-2">
                  <span>Step: {trace.stepIndex}</span>
                  <span>Tool call: {trace.toolCallIndex}</span>
                  {trace.permissionDecision && <span>Permission: {trace.permissionDecision}</span>}
                  {trace.outputPreview && <span>{trace.outputPreview}</span>}
                  {trace.outputReceipt ? (
                    <span>
                      Output receipt: {trace.outputReceipt.verified ? "verified" : "unverified"} ·{" "}
                      {trace.outputReceipt.kind} · {trace.outputReceipt.byteCount} bytes ·{" "}
                      {trace.outputReceipt.digest}
                    </span>
                  ) : (
                    <span>Output receipt: unknown</span>
                  )}
                  {trace.proposalId && (
                    <Link
                      to={mailboxRoute({ proposalId: trace.proposalId })}
                      className="text-blue-700 hover:text-blue-900"
                    >
                      Proposal: {trace.proposalId}
                    </Link>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {hasCollaborationContent && (
        <>
          <div
            className={`flex flex-wrap items-center justify-between gap-2 ${
              hasReactActionContent ? "mt-3" : ""
            }`}
          >
            <div className="flex items-center gap-2 font-semibold">
              <ShieldCheck size={16} />
              AI collaboration rules used
            </div>
            {tokenText(audit) && (
              <span className="rounded-full bg-white px-2 py-0.5 text-[10px] text-emerald-700">
                {tokenText(audit)}
              </span>
            )}
          </div>

          <div className="mt-3 grid gap-3 md:grid-cols-2">
            {policies.length > 0 && (
              <div className="rounded-lg bg-white/80 p-3">
                <div className="text-[11px] font-medium text-emerald-700">collaboration rule</div>
                <div className="mt-2 flex flex-wrap gap-2">
                  {policies.map(id => (
                    <span
                      key={id}
                      className="rounded-full border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-xs text-emerald-800"
                    >
                      {policyLabel(id)}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {styles.length > 0 && (
              <div className="rounded-lg bg-white/80 p-3">
                <div className="flex items-center gap-1 text-[11px] font-medium text-teal-700">
                  <Sparkles size={12} />
                  AI collaboration style
                </div>
                <div className="mt-2 flex flex-wrap gap-2">
                  {styles.map(id => (
                    <span
                      key={id}
                      className="rounded-full border border-teal-100 bg-teal-50 px-2 py-0.5 text-xs text-teal-800"
                    >
                      {styleLabel(id)}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>

          {checks.length > 0 && (
            <div className="mt-3 rounded-lg bg-white/80 p-3">
              <div className="text-[11px] font-medium text-stone-700">behavior check</div>
              <div className="mt-2 space-y-2">
                {checks.map(check => (
                  <div key={check.id} className="flex items-start gap-2 text-xs text-stone-700">
                    {check.passed ? (
                      <CheckCircle2 size={14} className="mt-0.5 shrink-0 text-emerald-600" />
                    ) : (
                      <CircleAlert size={14} className="mt-0.5 shrink-0 text-amber-600" />
                    )}
                    <div>
                      <div className="font-medium">{check.label}</div>
                      {check.summary && (
                        <div className="mt-0.5 text-stone-500">{check.summary}</div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}
