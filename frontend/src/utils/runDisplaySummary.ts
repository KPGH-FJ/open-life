import type { AgentRun, MainChatTaskSummary } from "../tauri";
import { safePreviewText } from "./safePreview";
import { buildRuntimeDisclosure } from "./runtimeDisclosure";

export type RunDisplaySummary = {
  title: string;
  subtitle: string;
  outcome: string;
  route: string;
  tools: string;
  proposals: string;
  searchableText: string;
};

const KIND_LABELS: Record<string, string> = {
  conversation: "Chat",
  builder: "Life Model Building",
  calibration: "Calibration",
  evolution: "Evolution",
  tool_execution: "Tool",
  proactive: "Proactive",
  planning: "Planning",
  review: "Mailbox",
  writing: "Writing",
  memory_governance: "Memory",
  skill: "Skill",
  plugin: "Plugin",
};

function kindLabel(kind: string): string {
  return KIND_LABELS[kind] || kind;
}

function statusLabel(status: string): string {
  const labels: Record<string, string> = {
    running: "运行中",
    waiting_permission: "等待确认",
    blocked: "已阻断",
    timed_out: "已超时",
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
  };
  return labels[status] ?? status;
}

export function buildRunDisplaySummary(
  run: AgentRun,
  taskSummary?: MainChatTaskSummary
): RunDisplaySummary {
  const evidenceView = taskSummary?.evidenceView;
  const disclosure = buildRuntimeDisclosure(run, {
    taskSummary,
    evidenceView,
    runtimeRouteEvidence: evidenceView?.routeEvidence ?? taskSummary?.routeEvidence ?? null,
    strictRuntimeRouteEvidence: Boolean(evidenceView),
  });
  const title = kindLabel(run.kind);
  const lifecycle = evidenceView?.lifecycleState ?? taskSummary?.lifecycleState ?? run.status;
  const taskPart = taskSummary ? `Task ${lifecycle.replace(/_/g, " ")}` : null;
  const inputPart = run.userInput ? safePreviewText(run.userInput, 96) : "无用户输入正文";
  const subtitle = [taskPart, inputPart].filter(Boolean).join(" · ");
  const actionCount =
    evidenceView?.actionCount ?? taskSummary?.actionCount ?? run.actions?.length ?? 0;
  const observationCount =
    evidenceView?.observationCount ??
    taskSummary?.observationCount ??
    run.observations?.length ??
    0;
  const route = disclosure.routeLabel;
  const outcome = run.error?.message
    ? `${statusLabel(lifecycle)} · ${run.error.phase || "unknown"}`
    : evidenceView || actionCount > 0 || observationCount > 0
      ? `${statusLabel(lifecycle)} · ${actionCount} 个 action evidence · ${observationCount} 个 observation evidence`
      : `${statusLabel(lifecycle)} · 未记录 task/run evidence`;
  const tools = disclosure.toolsLabel;
  const proposals = disclosure.proposalsLabel;

  return {
    title,
    subtitle,
    outcome,
    route,
    tools,
    proposals,
    searchableText: [title, subtitle, outcome, route, tools, proposals, taskSummary?.title ?? ""]
      .join(" ")
      .toLowerCase(),
  };
}
