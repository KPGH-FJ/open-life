import { Link } from "react-router-dom";
import {
  Ban,
  Brain,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  FileCheck2,
  LockKeyhole,
  ShieldAlert,
  Wrench,
} from "lucide-react";
import type { ReactNode } from "react";
import type {
  MainChatAgentStateSnapshot,
  MainChatAgentTaskState,
  MainChatKernelEvent,
  ToolCallResult,
} from "../tauri";
import { mailboxLinkTarget, productRoutePath } from "../productShellContract";

type EvidenceTone = "active" | "success" | "warning" | "blocked" | "neutral";

type Props = {
  state: MainChatAgentStateSnapshot | null;
  taskState: MainChatAgentTaskState | null;
  kernelEvents: MainChatKernelEvent[];
  toolCalls: ToolCallResult[];
  sending: boolean;
  diagnosticsOpen: boolean;
  hasDiagnostics: boolean;
  canCancel: boolean;
  cancelBusy: boolean;
  cancelError?: string | null;
  onCancel?: () => void;
  onToggleDiagnostics: () => void;
};

function cleanLabel(value: unknown, fallback = "unknown"): string {
  if (typeof value !== "string" && typeof value !== "number" && typeof value !== "boolean") {
    return fallback;
  }
  const text = String(value)
    .replace(/[\u0000-\u001f\u007f]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return text ? text.slice(0, 180) : fallback;
}

function statusClass(tone: EvidenceTone): string {
  switch (tone) {
    case "active":
      return "border-sky-200 bg-sky-50 text-sky-900";
    case "success":
      return "border-emerald-200 bg-emerald-50 text-emerald-900";
    case "warning":
      return "border-amber-200 bg-amber-50 text-amber-950";
    case "blocked":
      return "border-rose-200 bg-rose-50 text-rose-900";
    default:
      return "border-stone-200 bg-white text-stone-800";
  }
}

function eventType(event: MainChatKernelEvent): string {
  return event.type;
}

function latestEvent<T extends MainChatKernelEvent["type"]>(
  events: MainChatKernelEvent[],
  type: T
): Extract<MainChatKernelEvent, { type: T }> | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (event.type === type) return event as Extract<MainChatKernelEvent, { type: T }>;
  }
  return null;
}

function latestToolDecisionWithoutObservation(events: MainChatKernelEvent[]) {
  const lastDecisionIndex = [...events]
    .reverse()
    .findIndex(event => eventType(event) === "tool_decision");
  if (lastDecisionIndex < 0) return null;
  const decisionIndex = events.length - 1 - lastDecisionIndex;
  const decision = events[decisionIndex];
  if (!decision || decision.type !== "tool_decision") return null;
  const hasLaterObservation = events
    .slice(decisionIndex + 1)
    .some(
      event =>
        event.type === "tool_observation" &&
        cleanLabel(event.tool_name) === cleanLabel(decision.tool_name)
    );
  return hasLaterObservation ? null : decision;
}

function isActionActive(status: string): boolean {
  return ["running", "executing", "queued", "planned", "retrying", "pending_permission"].includes(
    status
  );
}

function isPermissionState(
  state: MainChatAgentStateSnapshot | null,
  taskState: MainChatAgentTaskState | null
) {
  return Boolean(
    state?.task.status === "waiting_permission" ||
    state?.actions.some(action => action.status === "pending_permission") ||
    state?.blockers.some(blocker =>
      [...(blocker.controls ?? []), blocker.reasonCode].some(value =>
        /permission|confirmation|approve/i.test(value)
      )
    ) ||
    state?.proposals.some(
      proposal =>
        proposal.proposalType === "tool_permission" && proposal.status === "pending_review"
    ) ||
    (taskState?.pendingApprovalCount ?? 0) > 0
  );
}

function stateStatus(
  state: MainChatAgentStateSnapshot | null,
  taskState: MainChatAgentTaskState | null
) {
  return taskState?.session?.status ?? state?.task.status ?? "running";
}

function statusTone(status: string): EvidenceTone {
  if (["completed", "delivered", "succeeded"].includes(status)) return "success";
  if (["blocked", "waiting_permission", "pending_permission"].includes(status)) return "warning";
  if (["failed", "cancelled"].includes(status)) return "blocked";
  if (["running", "executing", "planning", "queued"].includes(status)) return "active";
  return "neutral";
}

function EvidenceRow({
  icon,
  label,
  tone,
  primary,
  secondary,
  children,
}: {
  icon: ReactNode;
  label: string;
  tone: EvidenceTone;
  primary: string;
  secondary?: string | null;
  children?: ReactNode;
}) {
  return (
    <div className={`min-w-0 border-l px-3 py-2 ${statusClass(tone)}`}>
      <div className="flex flex-wrap items-center gap-2">
        <span className="shrink-0">{icon}</span>
        <span className="font-semibold">{label}</span>
        <span className="min-w-0 truncate text-stone-900">{primary}</span>
      </div>
      {secondary && <div className="mt-1 line-clamp-2 text-xs opacity-80">{secondary}</div>}
      {children}
    </div>
  );
}

export default function MainChatExecutionEvidence({
  state,
  taskState,
  kernelEvents,
  toolCalls,
  sending,
  diagnosticsOpen,
  hasDiagnostics,
  canCancel,
  cancelBusy,
  cancelError,
  onCancel,
  onToggleDiagnostics,
}: Props) {
  const status = stateStatus(state, taskState);
  const turnStarted = latestEvent(kernelEvents, "turn_started");
  const contextLoaded = latestEvent(kernelEvents, "context_loaded");
  const routeSelected = latestEvent(kernelEvents, "route_selected");
  const finalAnswerEvent = latestEvent(kernelEvents, "final_answer");
  const activeAction = state?.actions.find(action => isActionActive(action.status)) ?? null;
  const activeToolDecision = latestToolDecisionWithoutObservation(kernelEvents);
  const kernelObservations = kernelEvents.filter(
    (event): event is Extract<MainChatKernelEvent, { type: "tool_observation" }> =>
      event.type === "tool_observation"
  );
  const kernelBlockers = kernelEvents.filter(
    (event): event is Extract<MainChatKernelEvent, { type: "blocker" }> => event.type === "blocker"
  );
  const thinkingEvidence =
    sending &&
    !state?.finalDelivery &&
    Boolean(turnStarted || contextLoaded || routeSelected || status === "running");
  const permissionNeeded = isPermissionState(state, taskState);
  const proposalCount = state?.proposals.length ?? 0;
  const blockerCount =
    (state?.blockers.length ?? 0) +
    (state?.blockers.length ? 0 : (taskState?.session?.pendingBlockers.length ?? 0));
  const observationCount =
    (state?.observations.length ?? 0) || kernelObservations.length || toolCalls.length;
  const hasEvidence =
    thinkingEvidence ||
    Boolean(state) ||
    Boolean(taskState?.session) ||
    kernelEvents.length > 0 ||
    toolCalls.length > 0;

  if (!hasEvidence) return null;

  return (
    <section
      data-testid="main-chat-execution-evidence"
      data-task-status={status}
      data-proposal-count={proposalCount}
      data-blocker-count={blockerCount}
      data-observation-count={observationCount}
      className="border-y border-stone-200 bg-white px-4 py-3 text-xs text-stone-700"
      aria-label="Main Chat execution evidence"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="inline-flex h-6 items-center gap-1 rounded-md bg-stone-950 px-2 font-semibold text-white">
              <Brain size={13} />
              Execution evidence
            </span>
            <span
              className={`inline-flex h-6 items-center rounded-md border px-2 font-medium ${statusClass(
                statusTone(status)
              )}`}
            >
              {status.replace(/_/g, " ")}
            </span>
            {state?.route.strategy && (
              <span className="inline-flex h-6 items-center rounded-md border border-stone-200 bg-stone-50 px-2 font-medium text-stone-800">
                {state.route.strategy}
              </span>
            )}
          </div>
          <div className="mt-1 truncate text-sm font-semibold text-stone-950">
            {state?.task.title ?? "Main Chat turn"}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {canCancel && (
            <button
              type="button"
              aria-label="Cancel current Main Chat task"
              title="Cancel current Main Chat task"
              disabled={cancelBusy}
              onClick={onCancel}
              className="inline-flex min-h-7 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-40"
            >
              <Ban size={13} />
              <span>Cancel</span>
            </button>
          )}
          {hasDiagnostics && (
            <button
              type="button"
              aria-expanded={diagnosticsOpen}
              aria-label={
                diagnosticsOpen ? "Hide Main Chat diagnostics" : "Show Main Chat diagnostics"
              }
              title={diagnosticsOpen ? "Hide Main Chat diagnostics" : "Show Main Chat diagnostics"}
              onClick={onToggleDiagnostics}
              className="inline-flex min-h-7 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-800 hover:bg-stone-100"
            >
              {diagnosticsOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
              <span>Diagnostics</span>
            </button>
          )}
        </div>
      </div>

      <div className="mt-3 grid gap-2 md:grid-cols-2">
        {thinkingEvidence && (
          <EvidenceRow
            icon={<Brain size={14} />}
            label="Thinking"
            tone="active"
            primary={cleanLabel(routeSelected?.route_metadata?.reason, "Kernel turn started")}
            secondary={
              contextLoaded
                ? `${contextLoaded.selected_source_count} bounded context sources selected`
                : turnStarted
                  ? `session ${cleanLabel(turnStarted.session_id)}`
                  : null
            }
          />
        )}

        {(activeAction || activeToolDecision) && (
          <EvidenceRow
            icon={<Wrench size={14} />}
            label="Tool running"
            tone={permissionNeeded ? "warning" : "active"}
            primary={cleanLabel(
              activeAction?.label ?? activeToolDecision?.tool_name,
              "Governed action"
            )}
            secondary={cleanLabel(
              activeAction?.target ?? activeToolDecision?.target ?? activeToolDecision?.reason,
              "Current action is backed by kernel action evidence."
            )}
          />
        )}

        {state?.observations.slice(0, 3).map(observation => (
          <EvidenceRow
            key={observation.observationId}
            icon={<FileCheck2 size={14} />}
            label="Tool observation"
            tone="success"
            primary={cleanLabel(observation.sourceLabel, observation.sourceKind)}
            secondary={cleanLabel(observation.preview)}
          />
        ))}

        {!state?.observations.length &&
          kernelObservations
            .slice(-3)
            .map((observation, index) => (
              <EvidenceRow
                key={`kernel-observation-${index}-${observation.tool_name}`}
                icon={<FileCheck2 size={14} />}
                label="Tool observation"
                tone={observation.blocker ? "warning" : "success"}
                primary={`${cleanLabel(observation.tool_name)} · ${cleanLabel(observation.status)}`}
                secondary={cleanLabel(observation.blocker ?? observation.output_preview)}
              />
            ))}

        {!state?.observations.length &&
          kernelObservations.length === 0 &&
          toolCalls
            .slice(0, 3)
            .map((call, index) => (
              <EvidenceRow
                key={`tool-call-${index}-${call.actionRef}`}
                icon={<FileCheck2 size={14} />}
                label="Tool observation"
                tone={call.status === "success" ? "success" : "warning"}
                primary={cleanLabel(
                  call.toolRef.id === "unknown_tool" ? "Governed tool" : call.toolRef.id
                )}
                secondary={cleanLabel(
                  call.failureCode ??
                    (call.outputReceipt
                      ? `${call.outputReceipt.byteCount} bytes · ${call.outputReceipt.digest}`
                      : (call.executionReceipt?.transportStatus ?? null))
                )}
              />
            ))}

        {state?.proposals.slice(0, 3).map(proposal => (
          <EvidenceRow
            key={proposal.proposalId}
            icon={<ExternalLink size={14} />}
            label="Proposal created"
            tone="warning"
            primary={cleanLabel(proposal.title, proposal.proposalType)}
            secondary={cleanLabel(proposal.summary)}
          >
            <div className="mt-2">
              <Link
                {...mailboxLinkTarget({
                  proposalId: proposal.proposalId,
                  mainChatTaskSessionId: state.task.taskId,
                  returnTo: productRoutePath("Companion"),
                })}
                className="inline-flex min-h-6 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-900 hover:bg-stone-100"
              >
                <ExternalLink size={12} />
                <span>Open proposal</span>
              </Link>
            </div>
          </EvidenceRow>
        ))}

        {permissionNeeded && (
          <EvidenceRow
            icon={<LockKeyhole size={14} />}
            label="Permission needed"
            tone="warning"
            primary="User review is required before continuing."
            secondary="Approve, edit, deny, or defer the pending proposal or permission request."
          >
            <div className="mt-2">
              <Link
                {...mailboxLinkTarget({
                  mainChatTaskSessionId: state?.task.taskId ?? taskState?.session?.id,
                  returnTo: productRoutePath("Companion"),
                })}
                className="inline-flex min-h-6 items-center gap-1 rounded-md border border-stone-200 bg-white px-2 font-medium text-stone-900 hover:bg-stone-100"
              >
                <ExternalLink size={12} />
                <span>Open Mailbox</span>
              </Link>
            </div>
          </EvidenceRow>
        )}

        {state?.blockers.slice(0, 3).map(blocker => (
          <EvidenceRow
            key={blocker.blockerId}
            icon={<ShieldAlert size={14} />}
            label="Blocked"
            tone={permissionNeeded ? "warning" : "blocked"}
            primary={cleanLabel(blocker.title, blocker.reasonCode)}
            secondary={`${blocker.detail} Next: ${blocker.controls.length ? blocker.controls.join(", ") : blocker.recoverable ? "retry or edit the request" : "change the request"}`}
          />
        ))}

        {!state?.blockers.length &&
          (taskState?.session?.pendingBlockers ?? [])
            .slice(0, 3)
            .map(blocker => (
              <EvidenceRow
                key={blocker}
                icon={<ShieldAlert size={14} />}
                label="Blocked"
                tone="warning"
                primary={cleanLabel(blocker)}
                secondary="Next: review the task state or edit the request."
              />
            ))}

        {!state?.blockers.length &&
          !(taskState?.session?.pendingBlockers ?? []).length &&
          kernelBlockers
            .slice(-2)
            .map(blocker => (
              <EvidenceRow
                key={blocker.code}
                icon={<ShieldAlert size={14} />}
                label="Blocked"
                tone="blocked"
                primary={cleanLabel(blocker.code)}
                secondary="Next: edit the request or use a supported governed capability."
              />
            ))}

        {status !== "cancelled" && (state?.finalDelivery || finalAnswerEvent) && (
          <EvidenceRow
            icon={<CheckCircle2 size={14} />}
            label="Final answer"
            tone={status === "cancelled" ? "blocked" : "success"}
            primary={cleanLabel(
              state?.finalDelivery?.headline ?? finalAnswerEvent?.content_preview,
              "Final answer delivered"
            )}
            secondary={
              state?.finalDelivery
                ? `${state.finalDelivery.completedActions.length} actions, ${state.finalDelivery.observationsUsed.length} observations, ${state.finalDelivery.proposalsCreated.length} proposals`
                : `${finalAnswerEvent?.content_chars ?? 0} characters generated`
            }
          />
        )}

        {status === "cancelled" && (
          <EvidenceRow
            icon={<Ban size={14} />}
            label="Canceled"
            tone="blocked"
            primary="The task was canceled."
            secondary={
              taskState?.session?.hasFinalSummary
                ? "A terminal summary is recorded in the canonical trace."
                : "Queued non-terminal actions should be stopped."
            }
          />
        )}
      </div>

      {cancelError && (
        <div className="mt-2 border-l border-rose-400 bg-rose-50 px-2 py-1 text-rose-900">
          {cancelError}
        </div>
      )}
    </section>
  );
}
