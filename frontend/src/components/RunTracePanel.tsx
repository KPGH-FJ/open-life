import { CheckCircle2, CircleAlert, ListChecks, ShieldCheck, Sparkles, Wrench } from "lucide-react";
import { Link } from "react-router-dom";
import type {
  AgentRun,
  HSBehaviorCheckSummary,
  HSSelectionAudit,
  ReactActionTraceEnvelope,
  ReasoningTrace,
} from "../tauri";
import { mailboxRoute, productRoutePath } from "../productShellContract";
import { getPlanExecuteProductTrace } from "../utils/planExecuteProduct";

interface Props {
  run?: AgentRun | null;
  trace?: ReasoningTrace | null;
}

interface SkillRuntimeTraceEnvelope {
  traceKind: "skill_runtime";
  skillId?: string;
  skillSourceKind?: string;
  executionStatus?: string;
  parseStatus?: string;
  validationStatus?: string;
  warningCount?: number;
  proposalCandidateCount?: number;
  acceptedProposalCandidateCount?: number;
  skippedProposalCandidateCount?: number;
  generatedProposalIds?: string[];
  metadataSafe?: boolean;
  containsRawContent?: boolean;
  guidanceConsumptionMode?: string;
  contextReport?: {
    requiredContextCount?: number;
    availableContextCount?: number;
    promptContextDigest?: string;
    items?: Array<{
      contextId?: string;
      available?: boolean;
      itemCount?: number;
      digest?: string;
    }>;
  };
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

function collectBehaviorChecks(
  run?: AgentRun | null,
  trace?: ReasoningTrace | null
): HSBehaviorCheckSummary[] {
  return run?.behaviorChecks ?? run?.reasoningTrace?.behaviorChecks ?? trace?.behaviorChecks ?? [];
}

function collectReactActionTraces(run?: AgentRun | null): ReactActionTraceEnvelope[] {
  const byAction = new Map<string, ReactActionTraceEnvelope>();
  for (const action of run?.actions ?? []) {
    if (action.reactTrace) byAction.set(action.reactTrace.actionId, action.reactTrace);
  }
  for (const observation of run?.observations ?? []) {
    if (observation.reactTrace && !byAction.has(observation.reactTrace.actionId)) {
      byAction.set(observation.reactTrace.actionId, observation.reactTrace);
    }
  }
  return [...byAction.values()].sort((a, b) => a.toolCallIndex - b.toolCallIndex);
}

function extractSkillTrace(value: any): SkillRuntimeTraceEnvelope | null {
  const trace = value?.skillTrace;
  if (!trace || trace.traceKind !== "skill_runtime") return null;
  return trace as SkillRuntimeTraceEnvelope;
}

function collectSkillRuntimeTraces(run?: AgentRun | null): SkillRuntimeTraceEnvelope[] {
  const bySkill = new Map<string, SkillRuntimeTraceEnvelope>();
  for (const action of run?.actions ?? []) {
    const trace = extractSkillTrace(action.output);
    if (trace) bySkill.set(`${action.id}:${trace.skillId ?? "skill"}`, trace);
  }
  for (const observation of run?.observations ?? []) {
    const trace = extractSkillTrace(observation.structuredResult);
    if (trace) bySkill.set(`${observation.id}:${trace.skillId ?? "skill"}`, trace);
  }
  return [...bySkill.values()];
}

export default function RunTracePanel({ run, trace }: Props) {
  const audit =
    run?.hsSelectionAudit ?? run?.reasoningTrace?.hsSelectionAudit ?? trace?.hsSelectionAudit;
  const policies = selectedPolicies(audit);
  const styles = selectedStyles(audit);
  const checks = collectBehaviorChecks(run, trace);
  const productTrace = getPlanExecuteProductTrace(run);
  const reactActionTraces = collectReactActionTraces(run);
  const skillRuntimeTraces = collectSkillRuntimeTraces(run);
  const hasCollaborationContent = policies.length > 0 || styles.length > 0 || checks.length > 0;
  const hasStrategyContent = !!productTrace;
  const hasReactActionContent = reactActionTraces.length > 0;
  const hasSkillRuntimeContent = skillRuntimeTraces.length > 0;
  const hasContent =
    hasCollaborationContent ||
    hasStrategyContent ||
    hasReactActionContent ||
    hasSkillRuntimeContent;

  if (!hasContent) {
    return (
      <section className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-600">
        No collaboration rules affected this run.
      </section>
    );
  }

  return (
    <section className="rounded-lg border border-emerald-100 bg-emerald-50 p-3 text-sm text-emerald-950">
      {hasSkillRuntimeContent && (
        <div className="rounded-lg bg-white/80 p-3 text-stone-800">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2 font-semibold text-stone-900">
              <Sparkles size={16} />
              Skill Runtime trace
            </div>
            <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[10px] text-emerald-700">
              metadata-safe
            </span>
          </div>

          <div className="mt-2 space-y-2">
            {skillRuntimeTraces.map((trace, index) => (
              <div
                key={`${trace.skillId ?? "skill"}-${index}`}
                className="rounded border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-700"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-900">
                    {trace.skillId ?? "unknown_skill"}
                  </span>
                  {trace.parseStatus && <span>Parse: {trace.parseStatus}</span>}
                  {trace.validationStatus && <span>Validation: {trace.validationStatus}</span>}
                  {trace.executionStatus && <span>Status: {trace.executionStatus}</span>}
                  {trace.guidanceConsumptionMode && (
                    <span>Guidance: {trace.guidanceConsumptionMode}</span>
                  )}
                </div>
                <div className="mt-1 flex flex-wrap gap-2">
                  {trace.proposalCandidateCount !== undefined && (
                    <span>Candidates: {trace.proposalCandidateCount}</span>
                  )}
                  {trace.acceptedProposalCandidateCount !== undefined && (
                    <span>Accepted: {trace.acceptedProposalCandidateCount}</span>
                  )}
                  {trace.skippedProposalCandidateCount !== undefined && (
                    <span>Skipped: {trace.skippedProposalCandidateCount}</span>
                  )}
                  {trace.warningCount !== undefined && <span>Warnings: {trace.warningCount}</span>}
                  {trace.contextReport?.availableContextCount !== undefined && (
                    <span>
                      Context: {trace.contextReport.availableContextCount}/
                      {trace.contextReport.requiredContextCount ?? "?"}
                    </span>
                  )}
                  {trace.contextReport?.promptContextDigest && (
                    <span>{trace.contextReport.promptContextDigest}</span>
                  )}
                </div>
                {trace.generatedProposalIds && trace.generatedProposalIds.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-2">
                    {trace.generatedProposalIds.map(proposalId => (
                      <Link
                        key={proposalId}
                        to={mailboxRoute({ proposalId })}
                        className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700 hover:bg-blue-100"
                      >
                        Proposal: {proposalId}
                      </Link>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {hasReactActionContent && (
        <div
          className={`rounded-lg bg-white/80 p-3 text-stone-800 ${hasSkillRuntimeContent ? "mt-3" : ""}`}
        >
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
                  {trace.outputHash && <span>{trace.outputHash}</span>}
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

      {productTrace && (
        <div
          className={`rounded-lg bg-white/80 p-3 text-stone-800 ${hasReactActionContent ? "mt-3" : ""}`}
        >
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
            {productTrace.strategyDescriptorId && (
              <span className="rounded border border-violet-100 bg-violet-50 px-2 py-0.5 text-violet-700">
                Descriptor: {productTrace.strategyDescriptorId}
              </span>
            )}
            {productTrace.registryReady !== undefined && (
              <span className="rounded border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-emerald-700">
                Registry: {productTrace.registryReady ? "ready" : "blocked"}
              </span>
            )}
            {productTrace.scenarioId && (
              <span className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700">
                Scenario: {productTrace.scenarioId}
              </span>
            )}
            {productTrace.planSessionId && (
              <Link
                to={productRoutePath("Today")}
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
                  to={mailboxRoute({ proposalId })}
                  className="rounded border border-blue-100 bg-blue-50 px-2 py-0.5 text-blue-700 hover:bg-blue-100"
                >
                  {proposalId}
                </Link>
              ))}
            </div>
          )}
        </div>
      )}

      {hasCollaborationContent && (
        <>
          <div
            className={`flex flex-wrap items-center justify-between gap-2 ${
              hasStrategyContent || hasReactActionContent || hasSkillRuntimeContent ? "mt-3" : ""
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
