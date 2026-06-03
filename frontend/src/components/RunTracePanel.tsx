import { CheckCircle2, CircleAlert, ListChecks, ShieldCheck, Sparkles } from "lucide-react";
import { Link } from "react-router-dom";
import type { AgentRun, HSBehaviorCheckSummary, HSSelectionAudit, ReasoningTrace } from "../tauri";
import { getPlanExecuteProductTrace } from "../utils/planExecuteProduct";
import { getMultiStrategyPreviewAudit } from "../utils/previewAudit";

interface Props {
  run?: AgentRun | null;
  trace?: ReasoningTrace | null;
}

function selectedPolicies(audit?: HSSelectionAudit): string[] {
  const raw = audit as any;
  return audit?.selectedPolicyIds ?? raw?.selected_policy_ids ?? [];
}

function selectedStyles(audit?: HSSelectionAudit): string[] {
  const raw = audit as any;
  return audit?.selectedHeuristicIds ?? raw?.selected_heuristic_ids ?? [];
}

function tokenText(audit?: HSSelectionAudit): string | null {
  if (!audit) return null;
  const raw = audit as any;
  const estimated = audit.estimatedTokens ?? raw?.estimated_tokens;
  const budget = audit.tokenBudget ?? raw?.token_budget;
  if (estimated === undefined || budget === undefined) return null;
  return `${estimated}/${budget} tokens`;
}

function policyLabel(id: string): string {
  if (id.includes("external_writes.proposal_first")) return "Review before external writes";
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

function collectBehaviorChecks(
  run?: AgentRun | null,
  trace?: ReasoningTrace | null
): HSBehaviorCheckSummary[] {
  return run?.behaviorChecks ?? run?.reasoningTrace?.behaviorChecks ?? trace?.behaviorChecks ?? [];
}

export default function RunTracePanel({ run, trace }: Props) {
  const audit =
    run?.hsSelectionAudit ?? run?.reasoningTrace?.hsSelectionAudit ?? trace?.hsSelectionAudit;
  const policies = selectedPolicies(audit);
  const styles = selectedStyles(audit);
  const checks = collectBehaviorChecks(run, trace);
  const previewAudit = getMultiStrategyPreviewAudit(run);
  const productTrace = getPlanExecuteProductTrace(run);
  const hasCollaborationContent = policies.length > 0 || styles.length > 0 || checks.length > 0;
  const hasStrategyContent = !!previewAudit || !!productTrace;
  const hasContent = hasCollaborationContent || hasStrategyContent;

  if (!hasContent) {
    return (
      <section className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-600">
        No collaboration rules affected this run.
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-emerald-100 bg-emerald-50 p-3 text-sm text-emerald-950">
      {productTrace && (
        <div className="rounded-lg bg-white/80 p-3 text-stone-800">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2 font-semibold text-stone-900">
              <ListChecks size={16} />
              Plan-Execute product trace
            </div>
            {productTrace.metadataSafe && (
              <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] text-emerald-700">
                metadata-safe
              </span>
            )}
          </div>

          <div className="mt-2 flex flex-wrap gap-2 text-xs">
            {productTrace.scenarioId && (
              <span className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700">
                Scenario: {productTrace.scenarioId}
              </span>
            )}
            {productTrace.planSessionId && (
              <Link
                to="/workspace"
                className="rounded border border-stone-200 bg-stone-50 px-2 py-0.5 text-stone-700 hover:bg-stone-100"
              >
                Session: {productTrace.planSessionId}
              </Link>
            )}
            {productTrace.status && (
              <span className="rounded border border-teal-100 bg-teal-50 px-2 py-0.5 text-teal-700">
                Status: {productTrace.status}
              </span>
            )}
            {productTrace.stepCount !== undefined && (
              <span className="rounded border border-stone-200 bg-stone-50 px-2 py-0.5 text-stone-700">
                Steps: {productTrace.stepCount}
              </span>
            )}
            {productTrace.generatedProposalCount !== undefined && (
              <span className="rounded border border-amber-100 bg-amber-50 px-2 py-0.5 text-amber-700">
                Proposals: {productTrace.generatedProposalCount}
              </span>
            )}
          </div>

          {productTrace.stepStatusCounts && (
            <div className="mt-2 flex flex-wrap gap-2 text-xs">
              {productTrace.stepStatusCounts.planned !== undefined && (
                <span className="rounded border border-stone-200 bg-stone-50 px-2 py-0.5 text-stone-700">
                  planned: {productTrace.stepStatusCounts.planned}
                </span>
              )}
              {productTrace.stepStatusCounts.executed !== undefined && (
                <span className="rounded border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-emerald-700">
                  executed: {productTrace.stepStatusCounts.executed}
                </span>
              )}
              {productTrace.stepStatusCounts.requiresProposal !== undefined && (
                <span className="rounded border border-amber-100 bg-amber-50 px-2 py-0.5 text-amber-700">
                  requires proposal: {productTrace.stepStatusCounts.requiresProposal}
                </span>
              )}
              {productTrace.stepStatusCounts.blocked !== undefined && (
                <span className="rounded border border-red-100 bg-red-50 px-2 py-0.5 text-red-700">
                  blocked: {productTrace.stepStatusCounts.blocked}
                </span>
              )}
            </div>
          )}

          <div className="mt-2 flex flex-wrap gap-2 text-xs">
            <span className="rounded border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-emerald-700">
              Direct writes: {productTrace.directLifeModelWrites ? "detected" : "none"}
            </span>
            <span className="rounded border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-emerald-700">
              External writes: {productTrace.externalWritesExecuted ? "detected" : "none"}
            </span>
            {productTrace.warningCount !== undefined && productTrace.warningCount > 0 && (
              <span className="rounded border border-amber-100 bg-amber-50 px-2 py-0.5 text-amber-700">
                Warnings: {productTrace.warningCount}
              </span>
            )}
          </div>

          {productTrace.generatedProposalIds && productTrace.generatedProposalIds.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2 text-xs">
              {productTrace.generatedProposalIds.map(proposalId => (
                <Link
                  key={proposalId}
                  to="/review"
                  className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700 hover:bg-blue-100"
                >
                  {proposalId}
                </Link>
              ))}
            </div>
          )}
        </div>
      )}

      {previewAudit && (
        <div className={`rounded-lg bg-white/80 p-3 text-stone-800 ${productTrace ? "mt-3" : ""}`}>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="font-semibold text-stone-900">Multi-strategy preview trace</div>
            {previewAudit.metadataSafe && (
              <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] text-emerald-700">
                metadata-safe
              </span>
            )}
          </div>
          <div className="mt-2 flex flex-wrap gap-2 text-xs">
            {previewAudit.strategyKind && (
              <span className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700">
                Strategy: {previewAudit.strategyKind}
              </span>
            )}
            {previewAudit.payloadKind && (
              <span className="rounded border border-teal-100 bg-teal-50 px-2 py-0.5 text-teal-700">
                Payload: {previewAudit.payloadKind}
              </span>
            )}
            {previewAudit.governanceDecisionKind && (
              <span className="rounded border border-amber-100 bg-amber-50 px-2 py-0.5 text-amber-700">
                Governance: {previewAudit.governanceDecisionKind}
              </span>
            )}
            {previewAudit.reasonCode && (
              <span className="rounded border border-stone-200 bg-stone-50 px-2 py-0.5 text-stone-700">
                {previewAudit.reasonCode}
              </span>
            )}
          </div>
          {previewAudit.planStepStatuses && previewAudit.planStepStatuses.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2 text-xs">
              {previewAudit.planStepStatuses.map(status => (
                <span
                  key={status}
                  className="rounded border border-stone-200 bg-stone-50 px-2 py-0.5 text-stone-700"
                >
                  {status}
                </span>
              ))}
            </div>
          )}
          {previewAudit.warnings && previewAudit.warnings.length > 0 && (
            <div className="mt-2 space-y-1 text-xs text-amber-700">
              {previewAudit.warnings.map(warning => (
                <div key={warning}>{warning}</div>
              ))}
            </div>
          )}
        </div>
      )}

      {hasCollaborationContent && (
        <>
          <div
            className={`flex flex-wrap items-center justify-between gap-2 ${
              hasStrategyContent ? "mt-3" : ""
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
