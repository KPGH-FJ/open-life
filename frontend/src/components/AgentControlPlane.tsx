import {
  Ban,
  CheckCircle2,
  Clock,
  Copy,
  FileText,
  Pencil,
  Play,
  RotateCw,
  ShieldAlert,
  Wrench,
  XCircle,
} from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import type { MainChatAgentDurableEvent, MainChatAgentStateSnapshot } from "../tauri";
import type { PlanExecuteReviewItem } from "../types";
import { mailboxLinkTarget, productRoutePath } from "../productShellContract";

type ControlTarget = {
  proposalId?: string;
  actionId?: string;
  blockerId?: string;
};

type PlanControlTarget = {
  planSessionId: string;
  baseRevision: number;
  stepId?: string;
  title?: string;
  reason?: string;
};

type ControlHandlers = {
  onResume?: () => void;
  onRetry?: () => void;
  onCancel?: () => void;
  onApproveOnce?: (
    target: Required<Pick<ControlTarget, "proposalId" | "actionId" | "blockerId">>
  ) => void;
  onDeny?: (target: ControlTarget) => void;
  onDefer?: (target: ControlTarget) => void;
  onAcceptProposal?: (proposalId: string) => void;
  onRejectProposal?: (proposalId: string) => void;
  onEditProposal?: (proposalId: string) => void;
  onRollbackMemory?: (memoryId: string) => void;
  onConfirmPlan?: (target: PlanControlTarget) => void;
  onEditPlanStep?: (
    target: Required<Pick<PlanControlTarget, "planSessionId" | "baseRevision" | "stepId" | "title">>
  ) => void;
  onExecutePlanStep?: (
    target: Required<Pick<PlanControlTarget, "planSessionId" | "baseRevision" | "stepId">>
  ) => void;
  onSkipPlanStep?: (
    target: Required<Pick<PlanControlTarget, "planSessionId" | "baseRevision" | "stepId">>
  ) => void;
  onCancelPlan?: (target: PlanControlTarget) => void;
  onReviewPlan?: (target: PlanControlTarget) => void;
  busy?: boolean;
  canResume?: boolean;
  canRetry?: boolean;
  canCancel?: boolean;
};

type Props = ControlHandlers & {
  state: MainChatAgentStateSnapshot;
  eventStream?: {
    status: string;
    taskSessionId?: string;
    lastAppliedSequence: number;
    events: MainChatAgentDurableEvent[];
  };
};

function statusClass(status: string): string {
  switch (status) {
    case "completed":
    case "delivered":
    case "succeeded":
      return "border-emerald-200 bg-emerald-50 text-emerald-800";
    case "blocked":
    case "waiting_permission":
    case "pending_permission":
      return "border-amber-200 bg-amber-50 text-amber-900";
    case "failed":
    case "cancelled":
      return "border-rose-200 bg-rose-50 text-rose-800";
    case "running":
    case "executing":
      return "border-sky-200 bg-sky-50 text-sky-800";
    default:
      return "border-stone-200 bg-white text-stone-700";
  }
}

function shortId(value: string): string {
  if (value.length <= 10) return value;
  return value.slice(-10);
}

function hasAnyControl(state: MainChatAgentStateSnapshot): boolean {
  return (
    (state.task.controls ?? []).length > 0 ||
    (state.blockers ?? []).some(blocker => (blocker.controls ?? []).length) ||
    (state.proposals ?? []).some(proposal => (proposal.controls ?? []).length)
  );
}

function supportsControl(state: MainChatAgentStateSnapshot, names: string[]): boolean {
  const controls = [
    ...(state.task.controls ?? []),
    ...(state.blockers ?? []).flatMap(blocker => blocker.controls ?? []),
    ...(state.proposals ?? []).flatMap(proposal => proposal.controls ?? []),
  ];
  return controls.some(control => names.includes(control));
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function fieldText(value: unknown, keys: string[]): string {
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  const record = asRecord(value);
  if (!record) return "item";
  for (const key of keys) {
    const item = record[key];
    if (typeof item === "string" || typeof item === "number" || typeof item === "boolean") {
      return String(item);
    }
  }
  return "item";
}

function secondaryText(value: unknown, keys: string[]): string | null {
  const record = asRecord(value);
  if (!record) return null;
  const parts = keys.flatMap(key => {
    const item = record[key];
    if (typeof item === "string" || typeof item === "number" || typeof item === "boolean") {
      return [`${key}: ${String(item)}`];
    }
    return [];
  });
  return parts.length ? parts.join(" · ") : null;
}

function readExecutionBadges(
  readExecution?: MainChatAgentStateSnapshot["observations"][number]["readExecution"]
) {
  if (!readExecution) return [];
  return [
    readExecution.kind,
    readExecution.realReadOnlyExecution ? "real read" : null,
    readExecution.fixtureBacked ? "fixture" : null,
    readExecution.networkReadAttempted ? "network attempted" : null,
    readExecution.directWritesExecuted ? "writes recorded" : "no writes",
  ].filter((badge): badge is string => Boolean(badge));
}

function shouldShowReviewCenterLink(controls: string[]): boolean {
  return controls.some(control =>
    [
      "open_review_center",
      "approve_once",
      "deny",
      "defer",
      "accept_proposal",
      "reject_proposal",
      "edit_proposal",
      "rollback",
    ].includes(control)
  );
}

function reviewCenterLink(
  controls: string[],
  keyPrefix: string,
  reviewState?: { proposalId?: string; mainChatTaskSessionId?: string; returnTo?: string }
) {
  if (!shouldShowReviewCenterLink(controls)) return null;
  return (
    <div className="mt-2 flex flex-wrap gap-1">
      <Link
        key={`${keyPrefix}-open-review-center`}
        {...mailboxLinkTarget(reviewState)}
        className="inline-flex min-h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100"
      >
        Open Mailbox
      </Link>
    </div>
  );
}

function inlineControlButton({
  label,
  icon,
  disabled,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="inline-flex min-h-6 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}

function matchingPermissionProposal(
  state: MainChatAgentStateSnapshot,
  blocker: MainChatAgentStateSnapshot["blockers"][number]
) {
  const actionId = blocker.affectedActionId;
  if (!actionId) return null;
  return (
    state.proposals.find(
      proposal =>
        proposal.proposalType === "tool_permission" &&
        proposal.status === "pending_review" &&
        proposal.actionIds.includes(actionId)
    ) ?? null
  );
}

function finalDeliverySection(
  title: string,
  items: unknown[],
  primaryKeys: string[],
  secondaryKeys: string[]
) {
  if (items.length === 0) return null;
  return (
    <div
      data-testid="agent-final-delivery-section"
      data-section-title={title}
      className="min-w-0 border-l border-stone-300 bg-white/80 px-2 py-1"
    >
      <div className="font-semibold text-stone-950">{title}</div>
      <div className="mt-1 space-y-1">
        {items.map((item, index) => {
          const primary = fieldText(item, primaryKeys);
          const secondary = secondaryText(item, secondaryKeys);
          return (
            <div key={`${title}-${index}`} className="min-w-0">
              <div className="truncate text-stone-700">{primary}</div>
              {secondary && <div className="truncate text-stone-500">{secondary}</div>}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function boundedTraceString(value: string | undefined | null, fallback: string | null = null) {
  if (!value) return fallback;
  return value
    .replace(/[\u0000-\u001f\u007f]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 160);
}

function reviewerTraceJson(state: MainChatAgentStateSnapshot, blockerCodes: string[]) {
  return JSON.stringify({
    schemaVersion: "main-chat-reviewer-trace-v1",
    taskId: boundedTraceString(state.task.taskId, "unknown"),
    runId: boundedTraceString(state.task.runId, "unknown"),
    status: boundedTraceString(state.task.status, "unknown"),
    route: boundedTraceString(state.route.strategy, "unknown"),
    blockers: blockerCodes.slice(0, 12).map(code => boundedTraceString(code, "unknown")),
    provider: boundedTraceString(state.provider?.provider),
    model: boundedTraceString(state.provider?.model),
    finalDeliveryStatus: boundedTraceString(state.finalDelivery?.status),
    timestamp: new Date().toISOString(),
  });
}

function idsForAction(
  items: Array<{
    actionId?: string;
    affectedActionId?: string;
    observationId?: string;
    blockerId?: string;
  }>,
  actionId: string,
  idKey: "observationId" | "blockerId"
) {
  return items
    .filter(item => item.actionId === actionId || item.affectedActionId === actionId)
    .map(item => item[idKey])
    .filter((id): id is string => Boolean(id));
}

function isCurrentAction(
  action: MainChatAgentStateSnapshot["actions"][number],
  state: MainChatAgentStateSnapshot
) {
  if (["running", "executing", "pending_permission", "queued"].includes(action.status)) {
    return true;
  }
  if (!["executing", "queued", "observing", "planning"].includes(state.task.status)) return false;
  return (
    state.actions.find(
      item => !["succeeded", "completed", "failed", "blocked", "cancelled"].includes(item.status)
    )?.actionId === action.actionId
  );
}

function reviewSummarySection(title: string, items: PlanExecuteReviewItem[]) {
  return (
    <div className="min-w-0 border-l border-emerald-300 bg-white/80 px-2 py-1">
      <div className="font-semibold text-stone-950">{title}</div>
      <div className="mt-1 space-y-1">
        {items.length > 0 ? (
          items.map(item => (
            <div key={`${title}-${item.stepId}`} className="min-w-0">
              <div className="truncate text-stone-700">{item.title}</div>
              <div className="mt-0.5 flex flex-wrap gap-1 text-stone-500">
                <span>{item.status.replace(/_/g, " ")}</span>
                {[
                  ...item.linkedActionIds,
                  ...item.linkedObservationIds,
                  ...item.linkedProposalIds,
                  ...item.blockerIds,
                ]
                  .slice(0, 3)
                  .map(id => (
                    <span
                      key={`${title}-${item.stepId}-${id}`}
                      className="inline-flex h-5 max-w-full items-center rounded-md border border-stone-200 bg-stone-50 px-1.5"
                    >
                      <span className="truncate">{shortId(id)}</span>
                    </span>
                  ))}
              </div>
            </div>
          ))
        ) : (
          <div className="text-stone-500">none</div>
        )}
      </div>
    </div>
  );
}

export default function AgentControlPlane({
  state,
  onResume,
  onRetry,
  onCancel,
  onApproveOnce,
  onDeny,
  onDefer,
  onAcceptProposal,
  onRejectProposal,
  onEditProposal,
  onRollbackMemory,
  onConfirmPlan,
  onEditPlanStep,
  onExecutePlanStep,
  onSkipPlanStep,
  onCancelPlan,
  onReviewPlan,
  busy = false,
  canResume = false,
  canRetry = false,
  canCancel = false,
  eventStream,
}: Props) {
  const resumeSupported = supportsControl(state, ["resume_task", "continue_task", "resume"]);
  const retrySupported = supportsControl(state, ["retry_failed_action", "retry_action", "retry"]);
  const cancelSupported = supportsControl(state, ["cancel_task", "cancel"]);
  const hasControls = hasAnyControl(state);
  const plan = state.plan ?? null;
  const planSessionId = plan?.planSessionId ?? plan?.planId;
  const planRevision =
    typeof plan?.revision === "number" && Number.isFinite(plan.revision) ? plan.revision : null;
  const planControls = plan?.controls ?? [];
  const planArtifact = plan?.artifactView ?? null;
  const planArtifactControls = planArtifact?.controls ?? [];
  const planCommandReady = Boolean(plan && planSessionId && planRevision !== null);
  const reviewSummary = plan?.reviewSummary ?? null;
  const finalDelivery = state.finalDelivery;
  const finalDeliverySectionTitles = [
    finalDelivery?.completedActions.length ? "Completed actions" : null,
    finalDelivery?.observationsUsed.length ? "Sources used" : null,
    finalDelivery?.proposalsCreated.length ? "Proposals created" : null,
    finalDelivery?.blockers.length ? "Blocked items" : null,
    finalDelivery?.skippedWork?.length ? "Skipped work" : null,
    finalDelivery?.pendingUserActions.length ? "Pending user actions" : null,
    finalDelivery?.durableChanges.length ? "Durable changes" : null,
    finalDelivery?.nextSteps.length ? "Next steps" : null,
  ].filter((title): title is string => Boolean(title));
  const blockerCodes = state.blockers.map(blocker => blocker.reasonCode || blocker.blockerId);
  const reviewerTraceLine = reviewerTraceJson(state, blockerCodes);
  const timelineVisible =
    Boolean(plan?.steps?.length) ||
    state.actions.length > 0 ||
    state.observations.length > 0 ||
    state.blockers.length > 0 ||
    state.proposals.length > 0 ||
    Boolean(state.finalDelivery);

  return (
    <section
      data-testid="agent-control-plane"
      data-task-session-id={state.task.taskId}
      data-run-id={state.task.runId}
      data-route-strategy={state.route.strategy}
      data-task-status={state.task.status}
      data-action-count={state.actions.length}
      data-observation-count={state.observations.length}
      data-blocker-count={state.blockers.length}
      data-proposal-count={state.proposals.length}
      data-final-delivery={state.finalDelivery ? "true" : "false"}
      data-final-delivery-status={state.finalDelivery?.status ?? ""}
      data-final-delivery-section-titles={finalDeliverySectionTitles.join("|")}
      aria-label="Agent Control Plane"
      className="border-y border-stone-200 bg-white px-4 py-3 text-xs text-stone-700"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="inline-flex h-6 items-center gap-1 rounded-md bg-stone-950 px-2 font-semibold text-white">
              <Wrench size={13} />
              Agent Control Plane
            </span>
            <span
              className={`inline-flex h-6 items-center rounded-md border px-2 font-medium ${statusClass(state.task.status)}`}
            >
              {state.task.status.replace(/_/g, " ")}
            </span>
            <span className="inline-flex h-6 items-center rounded-md border border-stone-200 bg-stone-50 px-2 font-semibold text-stone-900">
              {state.route.strategy}
            </span>
            <span className="text-stone-500">Run {shortId(state.task.runId)}</span>
            {state.route.confidence !== undefined && (
              <span className="text-stone-500">{Math.round(state.route.confidence * 100)}%</span>
            )}
          </div>
          <div className="mt-2 min-w-0">
            <div className="truncate text-sm font-semibold text-stone-950">{state.task.title}</div>
            <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-stone-500">
              <span>reason: {state.route.reason}</span>
              <span>sequence: {state.sequence}</span>
              {state.task.traceAvailable && <span>trace available</span>}
            </div>
          </div>
        </div>

        {hasControls && (
          <div className="flex shrink-0 items-center gap-1">
            {resumeSupported && (
              <button
                type="button"
                aria-label="Resume task"
                title="Resume task"
                disabled={!onResume || !canResume || busy}
                onClick={onResume}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Play size={14} />
              </button>
            )}
            {retrySupported && (
              <button
                type="button"
                aria-label="Retry failed action"
                title="Retry failed action"
                disabled={!onRetry || !canRetry || busy}
                onClick={() => onRetry?.()}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RotateCw size={14} />
              </button>
            )}
            {cancelSupported && (
              <button
                type="button"
                aria-label="Cancel task"
                title="Cancel task"
                disabled={!onCancel || !canCancel || busy}
                onClick={onCancel}
                className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-stone-200 bg-white text-stone-700 disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Ban size={14} />
              </button>
            )}
          </div>
        )}
      </div>

      <div
        data-testid="agent-reviewer-trace"
        data-task-session-id={state.task.taskId}
        data-run-id={state.task.runId}
        data-blocker-codes={blockerCodes.join("|")}
        className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 border-l border-stone-300 bg-stone-50/80 px-2 py-1 text-stone-600"
      >
        <span className="font-semibold text-stone-950">Audit trace</span>
        <span className="min-w-0 break-all">
          task <span className="font-mono text-stone-800">{state.task.taskId}</span>
        </span>
        <span className="min-w-0 break-all">
          run <span className="font-mono text-stone-800">{state.task.runId}</span>
        </span>
        {blockerCodes.length > 0 && (
          <span className="min-w-0 break-all">
            blockers <span className="font-mono text-stone-800">{blockerCodes.join(",")}</span>
          </span>
        )}
        <button
          type="button"
          aria-label="Copy reviewer trace"
          title="Copy reviewer trace"
          onClick={() => {
            void navigator.clipboard?.writeText(reviewerTraceLine).catch(() => undefined);
          }}
          className="inline-flex min-h-6 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100"
        >
          <Copy size={12} />
          <span>Copy</span>
        </button>
      </div>

      {eventStream && (
        <div
          data-testid="agent-event-stream"
          data-event-stream-status={eventStream.status}
          data-event-count={eventStream.events.length}
          className="mt-3 border-l border-indigo-300 bg-indigo-50/70 px-2 py-1"
        >
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-semibold text-stone-950">Event stream</span>
            <span className="inline-flex h-5 items-center rounded-md border border-indigo-200 bg-white px-1.5 font-medium text-indigo-800">
              {eventStream.status}
            </span>
            <span className="text-stone-600">
              {eventStream.events.length} {eventStream.events.length === 1 ? "event" : "events"}
            </span>
            <span className="text-stone-500">sequence {eventStream.lastAppliedSequence}</span>
          </div>
          {eventStream.events.length > 0 && (
            <div className="mt-1 flex flex-wrap gap-1 text-stone-600">
              {eventStream.events.slice(-4).map(event => (
                <span
                  key={event.eventId}
                  className="inline-flex h-5 max-w-full items-center rounded-md border border-indigo-100 bg-white px-1.5"
                >
                  <span className="truncate">
                    #{event.sequence} {event.eventType} · {event.source}
                  </span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}

      {(state.context.length > 0 || state.provider || state.plan) && (
        <div className="mt-3 grid gap-2 md:grid-cols-3">
          {state.context.length > 0 && (
            <div className="min-w-0 border-l border-stone-300 bg-stone-50/80 px-2 py-1">
              <div className="font-semibold text-stone-950">Context</div>
              <div className="mt-1 space-y-1">
                {state.context.slice(0, 3).map(context => (
                  <div key={context.contextId} className="truncate text-stone-600">
                    {context.sourceLabel}
                  </div>
                ))}
              </div>
            </div>
          )}
          {state.provider && (
            <div className="min-w-0 border-l border-sky-300 bg-sky-50/70 px-2 py-1">
              <div className="font-semibold text-stone-950">Provider</div>
              <div className="mt-1 truncate text-stone-600">
                {state.provider.provider} · {state.provider.model}
              </div>
            </div>
          )}
          {state.plan && (
            <div className="min-w-0 border-l border-emerald-300 bg-emerald-50/70 px-2 py-1">
              <div className="font-semibold text-stone-950">Plan</div>
              <div className="mt-1 truncate text-stone-600">{state.plan.summary}</div>
              {state.plan.revision !== undefined && state.plan.revision !== null && (
                <div className="mt-1 text-stone-500">
                  revision {state.plan.revisionId ?? state.plan.revision}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {planArtifact && (
        <div
          data-testid="agent-plan-artifact"
          data-plan-id={planArtifact.planId}
          data-plan-session-id={planArtifact.planSessionId}
          data-task-session-id={planArtifact.taskSessionId}
          data-run-id={planArtifact.runId}
          className="mt-3 border-l border-emerald-500 bg-emerald-50/70 px-3 py-2"
        >
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-semibold text-emerald-950">{planArtifact.title}</span>
                <span
                  className={`inline-flex h-5 items-center rounded-md border px-1.5 font-medium ${statusClass(
                    planArtifact.status
                  )}`}
                >
                  {planArtifact.status.replace(/_/g, " ")}
                </span>
              </div>
              <div className="mt-1 min-w-0 break-all font-mono text-stone-700">
                {planArtifact.planId}
              </div>
              <div className="mt-1 text-stone-700">{planArtifact.summary}</div>
            </div>
            <div className="flex shrink-0 flex-wrap gap-1">
              <button
                type="button"
                aria-label="Copy plan artifact"
                title="Copy plan artifact"
                onClick={() => {
                  void navigator.clipboard?.writeText(planArtifact.body).catch(() => undefined);
                }}
                className="inline-flex min-h-6 items-center gap-1 rounded-md border border-emerald-200 bg-white px-2 font-medium text-emerald-950 hover:bg-emerald-100"
              >
                <Copy size={12} />
                <span>Copy</span>
              </button>
              {planCommandReady &&
                planArtifactControls.includes("confirm_plan") &&
                onConfirmPlan &&
                inlineControlButton({
                  label: "Confirm plan",
                  icon: <CheckCircle2 size={13} />,
                  disabled: busy,
                  onClick: () =>
                    onConfirmPlan({
                      planSessionId: planSessionId!,
                      baseRevision: planRevision!,
                    }),
                })}
              {planCommandReady &&
                planArtifactControls.includes("cancel_task") &&
                onCancelPlan &&
                inlineControlButton({
                  label: "Cancel plan",
                  icon: <Ban size={13} />,
                  disabled: busy,
                  onClick: () =>
                    onCancelPlan({
                      planSessionId: planSessionId!,
                      baseRevision: planRevision!,
                    }),
                })}
              {planCommandReady &&
                planArtifactControls.includes("review_plan") &&
                onReviewPlan &&
                inlineControlButton({
                  label: "Confirm plan",
                  icon: <CheckCircle2 size={13} />,
                  disabled: busy,
                  onClick: () =>
                    onReviewPlan({
                      planSessionId: planSessionId!,
                      baseRevision: planRevision!,
                    }),
                })}
            </div>
          </div>
          <div className="mt-2 whitespace-pre-wrap border-y border-emerald-200 bg-white/80 px-2 py-2 text-stone-800">
            {planArtifact.body}
          </div>
          <div className="mt-2 grid gap-2 md:grid-cols-2">
            <div className="min-w-0 border-l border-emerald-300 bg-white/80 px-2 py-1">
              <div className="font-semibold text-stone-950">Assumptions</div>
              <div className="mt-1 space-y-1">
                {planArtifact.assumptions.map(item => (
                  <div key={`assumption-${item.label}`} className="min-w-0">
                    <div className="font-medium text-stone-800">{item.label}</div>
                    <div className="text-stone-600">{item.detail}</div>
                    {item.sourceToolEvidence.length > 0 && (
                      <div className="mt-1 flex flex-wrap gap-1">
                        {item.sourceToolEvidence.map(source => (
                          <span
                            key={`${item.label}-${source.evidenceId}`}
                            className="inline-flex h-5 max-w-full items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 text-stone-600"
                          >
                            <span className="truncate">
                              {source.sourceLabel} · {source.toolName ?? source.sourceKind}
                            </span>
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
            <div className="min-w-0 border-l border-amber-300 bg-white/80 px-2 py-1">
              <div className="font-semibold text-stone-950">Unknowns</div>
              <div className="mt-1 space-y-1">
                {planArtifact.unknowns.length > 0 ? (
                  planArtifact.unknowns.map(item => (
                    <div key={`unknown-${item.label}`} className="min-w-0">
                      <div className="font-medium text-amber-950">{item.label}</div>
                      <div className="text-amber-900">{item.detail}</div>
                    </div>
                  ))
                ) : (
                  <div className="text-stone-500">none</div>
                )}
              </div>
            </div>
          </div>
          <div className="mt-2 flex flex-wrap gap-2 text-stone-600">
            <span>route {planArtifact.routeEvidence.strategy}</span>
            <span>run {shortId(planArtifact.runEvidence.runId)}</span>
            <span>task {shortId(planArtifact.runEvidence.taskSessionId)}</span>
            <span>{planArtifact.runEvidence.actionIds.length} actions</span>
            <span>{planArtifact.runEvidence.observationIds.length} observations</span>
          </div>
        </div>
      )}

      {timelineVisible && (
        <div
          data-testid="agent-execution-timeline"
          className="mt-3 border-l border-stone-900 bg-stone-50/70 px-3 py-2"
        >
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-semibold text-stone-950">Execution timeline</span>
            <span className="text-stone-500">sequence {state.sequence}</span>
          </div>
          <div className="mt-2 space-y-2">
            {plan?.steps?.map(step => (
              <div
                key={`timeline-plan-${step.stepId}`}
                data-testid={`agent-timeline-plan-${step.stepId}`}
                className="min-w-0 border-l border-emerald-300 bg-white px-2 py-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-950">plan {step.index}</span>
                  <span
                    className={`inline-flex h-5 items-center rounded-md border px-1.5 ${statusClass(step.status)}`}
                  >
                    {step.status.replace(/_/g, " ")}
                  </span>
                  <span className="truncate text-stone-700">{step.title}</span>
                </div>
                {[
                  ...step.linkedActionIds.map(id => `action ${id}`),
                  ...step.linkedObservationIds.map(id => `observation ${id}`),
                  ...step.linkedProposalIds.map(id => `proposal ${id}`),
                  ...step.blockerIds.map(id => `blocker ${id}`),
                ].length > 0 && (
                  <div className="mt-1 flex flex-wrap gap-1 text-stone-500">
                    {[
                      ...step.linkedActionIds.map(id => `action ${id}`),
                      ...step.linkedObservationIds.map(id => `observation ${id}`),
                      ...step.linkedProposalIds.map(id => `proposal ${id}`),
                      ...step.blockerIds.map(id => `blocker ${id}`),
                    ].map(label => (
                      <span key={`${step.stepId}-${label}`}>{label}</span>
                    ))}
                  </div>
                )}
              </div>
            ))}
            {state.actions.map(action => {
              const observationIds = idsForAction(
                state.observations,
                action.actionId,
                "observationId"
              );
              const blockerIds = idsForAction(state.blockers, action.actionId, "blockerId");
              const proposalIds = state.proposals
                .filter(proposal => proposal.actionIds.includes(action.actionId))
                .map(proposal => proposal.proposalId);
              const current = isCurrentAction(action, state);
              return (
                <div
                  key={`timeline-action-${action.actionId}`}
                  data-testid={`agent-timeline-action-${action.actionId}`}
                  data-current-action={current ? "true" : "false"}
                  className={`min-w-0 border-l px-2 py-1 ${
                    current
                      ? "border-sky-500 bg-sky-50 ring-1 ring-sky-100"
                      : "border-stone-300 bg-white"
                  }`}
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-semibold text-stone-950">{action.label}</span>
                    <span className="text-stone-500">{action.actionType}</span>
                    <span
                      className={`inline-flex h-5 items-center rounded-md border px-1.5 ${statusClass(action.status)}`}
                    >
                      {action.status.replace(/_/g, " ")}
                    </span>
                    {current && (
                      <span className="inline-flex h-5 items-center rounded-md border border-sky-200 bg-white px-1.5 font-medium text-sky-800">
                        current
                      </span>
                    )}
                  </div>
                  <div className="mt-1 truncate text-stone-600">{action.target}</div>
                  {[...observationIds, ...blockerIds, ...proposalIds].length > 0 && (
                    <div className="mt-1 flex flex-wrap gap-1 text-stone-500">
                      {observationIds.map(id => (
                        <span key={`${action.actionId}-observation-${id}`}>observation {id}</span>
                      ))}
                      {blockerIds.map(id => (
                        <span key={`${action.actionId}-blocker-${id}`}>blocker {id}</span>
                      ))}
                      {proposalIds.map(id => (
                        <span key={`${action.actionId}-proposal-${id}`}>proposal {id}</span>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
            {state.observations.map(observation => (
              <div
                key={`timeline-observation-${observation.observationId}`}
                data-testid={`agent-timeline-observation-${observation.observationId}`}
                className="min-w-0 border-l border-emerald-300 bg-white px-2 py-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-950">observation</span>
                  <span className="text-stone-500">{observation.sourceKind}</span>
                  <span className="truncate text-stone-700">{observation.sourceLabel}</span>
                  <span className="text-stone-500">action {observation.actionId}</span>
                </div>
                <div className="mt-1 line-clamp-2 text-stone-600">{observation.preview}</div>
              </div>
            ))}
            {state.blockers.map(blocker => (
              <div
                key={`timeline-blocker-${blocker.blockerId}`}
                data-testid={`agent-timeline-blocker-${blocker.blockerId}`}
                className="min-w-0 border-l border-amber-400 bg-amber-50 px-2 py-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-amber-950">blocker</span>
                  <span className="text-amber-900">{blocker.reasonCode}</span>
                  {blocker.affectedActionId && (
                    <span className="text-amber-900">action {blocker.affectedActionId}</span>
                  )}
                </div>
                <div className="mt-1 line-clamp-2 text-amber-900">{blocker.detail}</div>
              </div>
            ))}
            {state.proposals.map(proposal => (
              <div
                key={`timeline-proposal-${proposal.proposalId}`}
                data-testid={`agent-timeline-proposal-${proposal.proposalId}`}
                className="min-w-0 border-l border-indigo-300 bg-white px-2 py-1"
              >
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-950">proposal</span>
                  <span className="text-stone-500">{proposal.proposalType}</span>
                  <span
                    className={`inline-flex h-5 items-center rounded-md border px-1.5 ${statusClass(proposal.status)}`}
                  >
                    {proposal.status.replace(/_/g, " ")}
                  </span>
                  <span className="text-stone-500">{proposal.proposalId}</span>
                </div>
                <div className="mt-1 line-clamp-2 text-stone-600">{proposal.summary}</div>
              </div>
            ))}
            {state.finalDelivery && (
              <div className="min-w-0 border-l border-emerald-500 bg-emerald-50 px-2 py-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-emerald-950">final delivery</span>
                  <span
                    className={`inline-flex h-5 items-center rounded-md border px-1.5 ${statusClass(state.finalDelivery.status)}`}
                  >
                    {state.finalDelivery.status.replace(/_/g, " ")}
                  </span>
                  <span className="text-emerald-900">{state.finalDelivery.deliveryId}</span>
                </div>
                <div className="mt-1 line-clamp-2 text-emerald-900">
                  {state.finalDelivery.headline}
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {plan && (plan.steps?.length || planCommandReady) ? (
        <div
          data-testid="agent-plan-interaction"
          className="mt-3 border-l border-emerald-300 bg-emerald-50/70 px-3 py-2"
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="min-w-0">
              <div className="font-semibold text-stone-950">Plan interaction</div>
              <div className="mt-1 truncate text-stone-700">{plan.summary}</div>
            </div>
            {planCommandReady && !planArtifact && (
              <div className="flex flex-wrap gap-1">
                {planControls.includes("confirm_plan") &&
                  onConfirmPlan &&
                  inlineControlButton({
                    label: "Confirm plan",
                    icon: <CheckCircle2 size={13} />,
                    disabled: busy,
                    onClick: () =>
                      onConfirmPlan({
                        planSessionId: planSessionId!,
                        baseRevision: planRevision!,
                      }),
                  })}
                {planControls.includes("cancel_task") &&
                  onCancelPlan &&
                  inlineControlButton({
                    label: "Cancel plan",
                    icon: <Ban size={13} />,
                    disabled: busy,
                    onClick: () =>
                      onCancelPlan({
                        planSessionId: planSessionId!,
                        baseRevision: planRevision!,
                      }),
                  })}
                {planControls.includes("review_plan") &&
                  onReviewPlan &&
                  inlineControlButton({
                    label: "Confirm plan",
                    icon: <CheckCircle2 size={13} />,
                    disabled: busy,
                    onClick: () =>
                      onReviewPlan({
                        planSessionId: planSessionId!,
                        baseRevision: planRevision!,
                      }),
                  })}
              </div>
            )}
          </div>
          {plan.steps?.length ? (
            <div className="mt-2 divide-y divide-emerald-200 border-y border-emerald-200 bg-white/80">
              {plan.steps.map(step => {
                const stepControls = step.controls ?? [];
                const stepCommandReady = planCommandReady;
                return (
                  <div key={step.stepId} className="grid gap-2 py-2 md:grid-cols-[1fr_auto]">
                    <div className="min-w-0 px-2">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-semibold text-stone-950">
                          {step.index}. {step.title}
                        </span>
                        <span
                          className={`inline-flex h-5 items-center rounded-md border px-1.5 font-medium ${statusClass(step.status)}`}
                        >
                          {step.status.replace(/_/g, " ")}
                        </span>
                        <span className="text-stone-500">{step.kind}</span>
                      </div>
                      {step.reason && <div className="mt-1 text-stone-600">{step.reason}</div>}
                      {step.skipReason && (
                        <div className="mt-1 text-amber-900">Skipped: {step.skipReason}</div>
                      )}
                      {[
                        ...step.linkedActionIds,
                        ...step.linkedObservationIds,
                        ...step.linkedProposalIds,
                        ...step.blockerIds,
                      ].length > 0 && (
                        <div className="mt-1 flex flex-wrap gap-1">
                          {[
                            ...step.linkedActionIds,
                            ...step.linkedObservationIds,
                            ...step.linkedProposalIds,
                            ...step.blockerIds,
                          ].map(id => (
                            <span
                              key={`${step.stepId}-${id}`}
                              className="inline-flex h-5 max-w-full items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 text-stone-600"
                            >
                              <span className="truncate">{shortId(id)}</span>
                            </span>
                          ))}
                        </div>
                      )}
                    </div>
                    {stepCommandReady && (
                      <div className="flex flex-wrap items-start justify-end gap-1 px-2">
                        {stepControls.includes("edit_plan") &&
                          onEditPlanStep &&
                          inlineControlButton({
                            label: `Edit step ${step.title}`,
                            icon: <Pencil size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onEditPlanStep({
                                planSessionId: planSessionId!,
                                baseRevision: planRevision!,
                                stepId: step.stepId,
                                title: step.title,
                              }),
                          })}
                        {stepControls.includes("execute_step") &&
                          onExecutePlanStep &&
                          inlineControlButton({
                            label: `Execute step ${step.title}`,
                            icon: <Play size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onExecutePlanStep({
                                planSessionId: planSessionId!,
                                baseRevision: planRevision!,
                                stepId: step.stepId,
                              }),
                          })}
                        {stepControls.includes("skip_step") &&
                          onSkipPlanStep &&
                          inlineControlButton({
                            label: `Skip step ${step.title}`,
                            icon: <Ban size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onSkipPlanStep({
                                planSessionId: planSessionId!,
                                baseRevision: planRevision!,
                                stepId: step.stepId,
                              }),
                          })}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          ) : null}
          {reviewSummary && (
            <div className="mt-3 border-t border-emerald-200 pt-2">
              <div className="flex flex-wrap items-center gap-2">
                <div className="font-semibold text-stone-950">Plan summary</div>
                <span
                  className={`inline-flex h-5 items-center rounded-md border px-1.5 font-medium ${statusClass(
                    reviewSummary.planStatus
                  )}`}
                >
                  {reviewSummary.planStatus.replace(/_/g, " ")}
                </span>
                <span className="text-stone-500">revision {reviewSummary.basePlanRevision}</span>
              </div>
              <div className="mt-2 grid gap-2 md:grid-cols-3">
                {reviewSummarySection("Completed", reviewSummary.completedSteps)}
                {reviewSummarySection("Skipped", reviewSummary.skippedSteps)}
                {reviewSummarySection("Blocked", reviewSummary.blockedSteps)}
                {reviewSummarySection("Proposals created", reviewSummary.proposalsCreated)}
                {reviewSummarySection("Observations used", reviewSummary.observationsUsed)}
                {reviewSummarySection("Unresolved", reviewSummary.unresolved)}
              </div>
              {reviewSummary.recommendedNextAction.length > 0 && (
                <div className="mt-2 border-l border-emerald-300 bg-white/80 px-2 py-1">
                  <div className="font-semibold text-stone-950">Recommended next action</div>
                  <div className="mt-1 space-y-1 text-stone-700">
                    {reviewSummary.recommendedNextAction.map(action => (
                      <div key={action}>{action}</div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      ) : null}

      {state.actions.length > 0 && (
        <div data-testid="agent-actions" className="mt-3">
          <div className="mb-1 flex items-center gap-1 font-semibold text-stone-950">
            <Wrench size={13} />
            Actions
          </div>
          <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white">
            {state.actions.map(action => (
              <div key={action.actionId} className="grid gap-2 py-2 md:grid-cols-[1fr_auto]">
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-semibold text-stone-950">{action.label}</span>
                    <span className="text-stone-500">{action.actionType}</span>
                    <span
                      className={`inline-flex h-5 items-center rounded-md border px-1.5 font-medium ${statusClass(action.status)}`}
                    >
                      {action.status.replace(/_/g, " ")}
                    </span>
                    <span className="text-stone-500">{action.riskLevel} risk</span>
                  </div>
                  <div className="mt-1 truncate text-stone-600">{action.target}</div>
                  {[
                    ...action.observationIds.map(id => `observation ${id}`),
                    ...state.blockers
                      .filter(blocker => blocker.affectedActionId === action.actionId)
                      .map(blocker => `blocker ${blocker.blockerId}`),
                  ].length > 0 && (
                    <div className="mt-1 flex flex-wrap gap-1 text-stone-500">
                      {[
                        ...action.observationIds.map(id => `observation ${id}`),
                        ...state.blockers
                          .filter(blocker => blocker.affectedActionId === action.actionId)
                          .map(blocker => `blocker ${blocker.blockerId}`),
                      ].map(label => (
                        <span key={`${action.actionId}-${label}`}>{label}</span>
                      ))}
                    </div>
                  )}
                </div>
                <div className="text-right text-stone-500">
                  {action.retryable ? "retryable" : shortId(action.policyDecisionId)}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {state.observations.length > 0 && (
        <div data-testid="agent-observations" className="mt-3">
          <div className="mb-1 flex items-center gap-1 font-semibold text-stone-950">
            <FileText size={13} />
            Observations
          </div>
          <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white">
            {state.observations.map(observation => (
              <div key={observation.observationId} className="py-2">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold text-stone-950">
                    {observation.sourceKind}: {observation.sourceLabel}
                  </span>
                  {readExecutionBadges(observation.readExecution).map(badge => (
                    <span
                      key={`${observation.observationId}-${badge}`}
                      className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 text-stone-600"
                    >
                      {badge}
                    </span>
                  ))}
                  {observation.citationAvailable && (
                    <span className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 text-stone-600">
                      citation
                    </span>
                  )}
                  <span className="inline-flex h-5 items-center rounded-md border border-stone-200 bg-stone-50 px-1.5 text-stone-600">
                    action {observation.actionId}
                  </span>
                </div>
                <div className="mt-1 text-stone-600">{observation.preview}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {state.blockers.length > 0 && (
        <div data-testid="agent-blockers" className="mt-3">
          <div className="mb-1 flex items-center gap-1 font-semibold text-amber-950">
            <ShieldAlert size={13} />
            Blockers
          </div>
          <div className="divide-y divide-amber-200 border-y border-amber-200 bg-amber-50/70">
            {state.blockers.map(blocker => (
              <div key={blocker.blockerId} className="py-2">
                <div className="font-semibold text-amber-950">{blocker.title}</div>
                <div className="mt-1 text-amber-900">{blocker.detail}</div>
                <div className="mt-1 flex flex-wrap gap-1 text-amber-900">
                  <span>blocker {blocker.blockerId}</span>
                  {blocker.affectedActionId && <span>action {blocker.affectedActionId}</span>}
                  <span>{blocker.recoverable ? "recoverable" : "terminal explanation"}</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {(() => {
                    const permissionProposal = matchingPermissionProposal(state, blocker);
                    const actionId = blocker.affectedActionId;
                    const canActOnPermission = Boolean(permissionProposal && actionId);
                    return (
                      <>
                        {blocker.controls.includes("approve_once") &&
                          permissionProposal &&
                          actionId &&
                          onApproveOnce &&
                          inlineControlButton({
                            label: "Approve once",
                            icon: <CheckCircle2 size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onApproveOnce({
                                proposalId: permissionProposal.proposalId,
                                actionId,
                                blockerId: blocker.blockerId,
                              }),
                          })}
                        {blocker.controls.includes("deny") &&
                          canActOnPermission &&
                          onDeny &&
                          inlineControlButton({
                            label: "Deny",
                            icon: <XCircle size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onDeny({
                                proposalId: permissionProposal?.proposalId,
                                actionId,
                                blockerId: blocker.blockerId,
                              }),
                          })}
                        {blocker.controls.includes("defer") &&
                          canActOnPermission &&
                          onDefer &&
                          inlineControlButton({
                            label: "Defer",
                            icon: <Clock size={13} />,
                            disabled: busy,
                            onClick: () =>
                              onDefer({
                                proposalId: permissionProposal?.proposalId,
                                actionId,
                                blockerId: blocker.blockerId,
                              }),
                          })}
                        {blocker.controls.includes("cancel") &&
                          onCancel &&
                          inlineControlButton({
                            label: "Cancel",
                            icon: <Ban size={13} />,
                            disabled: busy || !canCancel,
                            onClick: onCancel,
                          })}
                      </>
                    );
                  })()}
                </div>
                {reviewCenterLink(blocker.controls, blocker.blockerId, {
                  mainChatTaskSessionId: state.task.taskId,
                  returnTo: productRoutePath("Companion"),
                })}
              </div>
            ))}
          </div>
        </div>
      )}

      {state.proposals.length > 0 && (
        <div data-testid="agent-proposals" className="mt-3">
          <div className="mb-1 flex items-center gap-1 font-semibold text-stone-950">
            <CheckCircle2 size={13} />
            Proposals
          </div>
          <div className="divide-y divide-stone-200 border-y border-stone-200 bg-white">
            {state.proposals.map(proposal => (
              <div key={proposal.proposalId} className="py-2">
                <div className="min-w-0">
                  <div className="font-semibold text-stone-950">{proposal.title}</div>
                  <div className="mt-1 text-stone-600">{proposal.summary}</div>
                  <div className="mt-1 flex flex-wrap gap-1 text-stone-500">
                    <span>proposal {proposal.proposalId}</span>
                    {proposal.actionIds.map(actionId => (
                      <span key={`${proposal.proposalId}-${actionId}`}>action {actionId}</span>
                    ))}
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {proposal.status === "pending_review" &&
                      proposal.controls.includes("accept_proposal") &&
                      onAcceptProposal &&
                      inlineControlButton({
                        label: "Accept proposal",
                        icon: <CheckCircle2 size={13} />,
                        disabled: busy,
                        onClick: () => onAcceptProposal(proposal.proposalId),
                      })}
                    {proposal.status === "pending_review" &&
                      proposal.controls.includes("reject_proposal") &&
                      onRejectProposal &&
                      inlineControlButton({
                        label: "Reject proposal",
                        icon: <XCircle size={13} />,
                        disabled: busy,
                        onClick: () => onRejectProposal(proposal.proposalId),
                      })}
                    {proposal.status === "pending_review" &&
                      proposal.controls.includes("edit_proposal") &&
                      onEditProposal &&
                      inlineControlButton({
                        label: "Edit proposal",
                        icon: <Pencil size={13} />,
                        disabled: busy,
                        onClick: () => onEditProposal(proposal.proposalId),
                      })}
                    {proposal.status === "pending_review" &&
                      proposal.controls.includes("defer") &&
                      onDefer &&
                      inlineControlButton({
                        label: "Defer",
                        icon: <Clock size={13} />,
                        disabled: busy,
                        onClick: () => onDefer({ proposalId: proposal.proposalId }),
                      })}
                    {proposal.status === "accepted" &&
                      proposal.proposalType === "memory" &&
                      proposal.controls.includes("rollback") &&
                      proposal.memoryLifecycle?.status === "materialized" &&
                      proposal.memoryLifecycle.memoryId &&
                      onRollbackMemory &&
                      inlineControlButton({
                        label: "Rollback memory",
                        icon: <RotateCw size={13} />,
                        disabled: busy,
                        onClick: () => onRollbackMemory(proposal.memoryLifecycle!.memoryId),
                      })}
                  </div>
                  {reviewCenterLink(
                    proposal.controls.includes("open_review_center")
                      ? proposal.controls
                      : [...proposal.controls, "open_review_center"],
                    proposal.proposalId,
                    {
                      proposalId: proposal.proposalId,
                      mainChatTaskSessionId: state.task.taskId,
                      returnTo: productRoutePath("Companion"),
                    }
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {state.finalDelivery && (
        <div
          data-testid="agent-final-delivery"
          className="mt-3 border-l border-emerald-300 bg-emerald-50/70 px-3 py-2"
        >
          <div className="flex flex-wrap items-center gap-2">
            <CheckCircle2 size={14} className="text-emerald-700" />
            <span className="font-semibold text-stone-950">{state.finalDelivery.headline}</span>
            <span
              className={`inline-flex h-5 items-center rounded-md border px-1.5 font-medium ${statusClass(state.finalDelivery.status)}`}
            >
              {state.finalDelivery.status.replace(/_/g, " ")}
            </span>
          </div>
          <div className="mt-1 text-stone-700">{state.finalDelivery.answer}</div>
          {state.finalDelivery.nextSteps.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-1">
              {state.finalDelivery.nextSteps.map(step => (
                <span
                  key={step}
                  className="inline-flex min-h-5 items-center rounded-md border border-emerald-200 bg-white px-1.5 text-emerald-900"
                >
                  {step}
                </span>
              ))}
            </div>
          )}
          {[
            state.finalDelivery.completedActions,
            state.finalDelivery.observationsUsed,
            state.finalDelivery.proposalsCreated,
            state.finalDelivery.blockers,
            state.finalDelivery.skippedWork ?? [],
            state.finalDelivery.pendingUserActions,
            state.finalDelivery.durableChanges,
          ].some(items => items.length > 0) && (
            <div className="mt-3 grid gap-2 md:grid-cols-2">
              {finalDeliverySection(
                "Completed actions",
                state.finalDelivery.completedActions,
                ["actionType", "action_id", "actionId", "target"],
                ["status", "target"]
              )}
              {finalDeliverySection(
                "Sources used",
                state.finalDelivery.observationsUsed,
                ["sourceLabel", "source_label", "observationId", "observation_id"],
                ["sourceKind", "source_kind", "preview"]
              )}
              {finalDeliverySection(
                "Proposals created",
                state.finalDelivery.proposalsCreated,
                ["summary", "proposalType", "proposal_type", "proposalId", "proposal_id"],
                ["status"]
              )}
              {finalDeliverySection(
                "Blocked items",
                state.finalDelivery.blockers,
                ["reasonCode", "reason_code", "blockerId", "blocker_id"],
                ["affectedActionId", "affected_action_id"]
              )}
              {finalDeliverySection(
                "Skipped work",
                state.finalDelivery.skippedWork ?? [],
                ["title", "stepId", "step_id", "kind"],
                ["reason", "skipReason", "skip_reason", "status"]
              )}
              {finalDeliverySection(
                "Pending user actions",
                state.finalDelivery.pendingUserActions,
                ["kind", "pendingId", "pending_id"],
                ["pendingId", "pending_id"]
              )}
              {finalDeliverySection(
                "Durable changes",
                state.finalDelivery.durableChanges,
                ["changeType", "change_type", "target"],
                ["target", "provenanceId", "provenance_id"]
              )}
            </div>
          )}
        </div>
      )}

      {state.diagnostics.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-1">
          {state.diagnostics.map(diagnostic => (
            <span
              key={diagnostic.gapId}
              className="inline-flex min-h-5 items-center rounded-md border border-rose-200 bg-rose-50 px-1.5 text-rose-800"
            >
              {diagnostic.gapCode}
            </span>
          ))}
        </div>
      )}
    </section>
  );
}
