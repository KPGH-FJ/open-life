import type { AgentRun, MainChatTaskSummary } from "../tauri";
import { safePreviewText } from "./safePreview";

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
  review: "Review",
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
    completed: "已完成",
    failed: "失败",
    cancelled: "已取消",
  };
  return labels[status] ?? status;
}

function routeLabel(run: AgentRun): string {
  const route = run.modelRoute;
  if (!route) return "模型路线未记录";
  const type =
    route.routeType === "local" ? "本地" : route.routeType === "cloud" ? "云端" : route.routeType;
  return `${type || "未知"} · ${route.provider || "unknown"} · ${route.model || "unknown"}`;
}

export function buildRunDisplaySummary(
  run: AgentRun,
  taskSummary?: MainChatTaskSummary
): RunDisplaySummary {
  const title = kindLabel(run.kind);
  const taskPart = taskSummary ? `Task ${taskSummary.status.replace(/_/g, " ")}` : null;
  const inputPart = run.userInput ? safePreviewText(run.userInput, 96) : "无用户输入正文";
  const subtitle = [taskPart, inputPart].filter(Boolean).join(" · ");
  const actionCount = run.actions?.length ?? 0;
  const observationCount = run.observations?.length ?? 0;
  const proposalCount = run.generatedProposals?.length ?? 0;
  const route = routeLabel(run);
  const outcome = run.error?.message
    ? `${statusLabel(run.status)} · ${run.error.phase || "unknown"}`
    : `${statusLabel(run.status)} · ${actionCount} 个 action · ${observationCount} 个 observation`;
  const tools =
    actionCount > 0
      ? `工具 ${actionCount}`
      : run.contextSummary?.usedToolsPrompt
        ? "工具提示已注入"
        : "未调用工具";
  const proposals = proposalCount > 0 ? `待确认 ${proposalCount}` : "无新提案";

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
