import {
  Ban,
  CheckCircle2,
  Clock,
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
import type { MainChatAgentStateSnapshot } from "../tauri";

type ControlTarget = {
  proposalId?: string;
  actionId?: string;
  blockerId?: string;
};

type ControlHandlers = {
  onResume?: () => void;
  onRetry?: (target?: ControlTarget) => void;
  onCancel?: () => void;
  onApproveOnce?: (
    target: Required<Pick<ControlTarget, "proposalId" | "actionId" | "blockerId">>
  ) => void;
  onDeny?: (target: ControlTarget) => void;
  onDefer?: (target: ControlTarget) => void;
  onAcceptProposal?: (proposalId: string) => void;
  onRejectProposal?: (proposalId: string) => void;
  onEditProposal?: (proposalId: string) => void;
  busy?: boolean;
  canResume?: boolean;
  canRetry?: boolean;
  canCancel?: boolean;
};

type Props = ControlHandlers & {
  state: MainChatAgentStateSnapshot;
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
    state.task.controls.length > 0 ||
    state.blockers.some(blocker => blocker.controls.length) ||
    state.proposals.some(proposal => proposal.controls.length)
  );
}

function supportsControl(state: MainChatAgentStateSnapshot, names: string[]): boolean {
  const controls = [
    ...state.task.controls,
    ...state.blockers.flatMap(blocker => blocker.controls),
    ...state.proposals.flatMap(proposal => proposal.controls),
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

function reviewCenterLink(controls: string[], keyPrefix: string, reviewState?: unknown) {
  if (!shouldShowReviewCenterLink(controls)) return null;
  return (
    <div className="mt-2 flex flex-wrap gap-1">
      <Link
        key={`${keyPrefix}-open-review-center`}
        to="/review"
        state={reviewState}
        className="inline-flex min-h-6 items-center rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100"
      >
        Open Review Center
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
    <div className="min-w-0 border-l border-stone-300 bg-white/80 px-2 py-1">
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
  busy = false,
  canResume = false,
  canRetry = false,
  canCancel = false,
}: Props) {
  const resumeSupported = supportsControl(state, ["resume_task", "continue_task", "resume"]);
  const retrySupported = supportsControl(state, ["retry_failed_action", "retry_action", "retry"]);
  const cancelSupported = supportsControl(state, ["cancel_task", "cancel"]);
  const hasControls = hasAnyControl(state);

  return (
    <section
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
            </div>
          )}
        </div>
      )}

      {state.actions.length > 0 && (
        <div className="mt-3">
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
        <div className="mt-3">
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
                </div>
                <div className="mt-1 text-stone-600">{observation.preview}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {state.blockers.length > 0 && (
        <div className="mt-3">
          <div className="mb-1 flex items-center gap-1 font-semibold text-amber-950">
            <ShieldAlert size={13} />
            Blockers
          </div>
          <div className="divide-y divide-amber-200 border-y border-amber-200 bg-amber-50/70">
            {state.blockers.map(blocker => (
              <div key={blocker.blockerId} className="py-2">
                <div className="font-semibold text-amber-950">{blocker.title}</div>
                <div className="mt-1 text-amber-900">{blocker.detail}</div>
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
                        {blocker.controls.includes("retry") &&
                          blocker.affectedActionId &&
                          onRetry &&
                          inlineControlButton({
                            label: "Retry",
                            icon: <RotateCw size={13} />,
                            disabled: busy,
                            onClick: () => onRetry({ actionId: blocker.affectedActionId }),
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
                  returnTo: "/chat",
                })}
              </div>
            ))}
          </div>
        </div>
      )}

      {state.proposals.length > 0 && (
        <div className="mt-3">
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
                  </div>
                  {reviewCenterLink(
                    proposal.controls.includes("open_review_center")
                      ? proposal.controls
                      : [...proposal.controls, "open_review_center"],
                    proposal.proposalId,
                    { mainChatTaskSessionId: state.task.taskId, returnTo: "/chat" }
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {state.finalDelivery && (
        <div className="mt-3 border-l border-emerald-300 bg-emerald-50/70 px-3 py-2">
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
