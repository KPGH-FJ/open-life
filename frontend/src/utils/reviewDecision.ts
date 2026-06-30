import type { AgentProposal } from "../tauri";
import {
  buildProposalDisplayModel,
  metadataValueSummary,
  proposalDomainLabel,
  proposalSubject,
  sourceLabel,
  type ProposalDisplayDiffRow,
} from "./proposalDisplay";

export type ReviewDecisionGroup =
  | "memory"
  | "life_model"
  | "tool_permission"
  | "external_action"
  | "model_policy"
  | "other";

export type ReviewDecisionView = {
  id: string;
  group: ReviewDecisionGroup;
  groupLabel: string;
  title: string;
  subtitle: string;
  beforeAfter: ProposalDisplayDiffRow[];
  why: string;
  evidence: string[];
  sourceSummary: string;
  sourceDetails: Array<{ label: string; value: string }>;
  impactScope: string;
  riskLabel: string;
  riskTone: "neutral" | "warning" | "danger";
  confidenceLabel: string;
  sourceLabel: string;
  technicalRows: Array<{ label: string; value: string; href?: string }>;
};

const GROUP_LABELS: Record<ReviewDecisionGroup, string> = {
  memory: "记忆",
  life_model: "Life Model",
  tool_permission: "工具权限",
  external_action: "外部操作",
  model_policy: "模型策略",
  other: "其他确认",
};

function groupForProposal(proposal: AgentProposal): ReviewDecisionGroup {
  if (proposal.proposalType === "memory_write" || proposal.proposalType === "memory_archive") {
    return "memory";
  }
  if (
    proposal.proposalType === "tool_permission" ||
    proposal.proposalType === "plugin_permission"
  ) {
    return "tool_permission";
  }
  if (
    proposal.proposalType === "external_write_action" ||
    proposal.proposalType === "scheduled_task" ||
    proposal.proposalType === "data_export"
  ) {
    return "external_action";
  }
  if (proposal.proposalType === "model_policy_change") return "model_policy";
  if (
    proposal.proposalType === "life_model_update" ||
    proposal.proposalType === "goal_update" ||
    proposal.proposalType === "state_update" ||
    proposal.proposalType === "preference_update" ||
    proposal.proposalType === "capability_update"
  ) {
    return "life_model";
  }
  return "other";
}

function riskLabel(proposal: AgentProposal): string {
  const labels: Record<AgentProposal["riskLevel"], string> = {
    low: "低风险",
    medium: "中风险",
    high: "高风险",
    critical: "严重风险",
  };
  return labels[proposal.riskLevel] ?? String(proposal.riskLevel);
}

function riskTone(proposal: AgentProposal): ReviewDecisionView["riskTone"] {
  if (proposal.riskLevel === "high" || proposal.riskLevel === "critical") return "danger";
  if (proposal.riskLevel === "medium") return "warning";
  return "neutral";
}

function evidenceLines(proposal: AgentProposal, fallback: string): string[] {
  const evidenceCount = proposal.evidenceSummaries?.length ?? 0;
  const behaviorCount = proposal.behaviorChecks?.length ?? 0;
  const lines = [`${sourceLabel(proposal.source)}形成了这条候选更新，确认前不会写入。`];

  if (evidenceCount > 0 || behaviorCount > 0) {
    lines.push(`包含 ${evidenceCount} 条依据摘要和 ${behaviorCount} 条行为检查，可展开查看来源。`);
  } else {
    lines.push(fallback);
  }

  return [...lines, ...runtimeFactSummaries(proposal)].slice(0, 5);
}

function impactScope(proposal: AgentProposal): string {
  const domain = proposalDomainLabel(proposal);
  if (groupForProposal(proposal) === "tool_permission") {
    return "会改变工具是否能在确认范围内执行；未同意前不会授予权限。";
  }
  if (groupForProposal(proposal) === "external_action") {
    return "会触发外部动作或本地数据覆盖；未同意前不会执行。";
  }
  if (groupForProposal(proposal) === "model_policy") {
    return "会改变模型路线或隐私边界；请确认它符合你的使用预期。";
  }
  if (groupForProposal(proposal) === "memory") {
    return "会影响 OpenLife 之后如何记住、检索和使用这条长期记忆。";
  }
  return `会影响 OpenLife 对「${domain}」的理解和后续建议。`;
}

const INTERNAL_COPY_RE =
  /\b(governed|governance|proposal|draft|routeType|transcriptId|taskSessionId|fallback|metadata|builder review|agent loop|toolpermission|planexecute|kernel|directanswer|blocker|incomplete)\b/i;
const ENGLISH_SENTENCE_RE = /[A-Za-z]{4,}[\s,:;._-]+[A-Za-z]{4,}/;
const CJK_RE = /[\u3400-\u9fff]/;

function compactUserText(value: string | undefined | null, maxLength = 180): string | null {
  const compact = value?.replace(/\s+/g, " ").trim();
  if (!compact) return null;
  return compact.length > maxLength ? `${compact.slice(0, maxLength - 1)}…` : compact;
}

function isUnexplainedInternalCopy(value: string): boolean {
  if (INTERNAL_COPY_RE.test(value)) return true;
  return ENGLISH_SENTENCE_RE.test(value) && !CJK_RE.test(value);
}

function readableReason(proposal: AgentProposal, fallback: string): string {
  const reason = compactUserText(proposal.reason);
  if (reason && !isUnexplainedInternalCopy(reason)) return reason;

  const group = groupForProposal(proposal);
  if (group === "external_action") {
    return "这是一个需要你确认的外部操作；确认前不会执行。";
  }
  if (group === "tool_permission") {
    return "这是一个工具或插件权限请求；确认前不会授予权限。";
  }
  if (group === "model_policy") {
    return "这是一个模型路线或隐私边界调整；需要你确认后才会生效。";
  }
  if (group === "memory") {
    return "OpenLife 发现一条可能值得记住的内容，需要你确认后才会进入长期记忆。";
  }
  return `${fallback}确认前不会改变 Life Model。`;
}

function sourceSummary(proposal: AgentProposal): string {
  const evidenceCount = proposal.evidenceSummaries?.length ?? 0;
  const behaviorCount = proposal.behaviorChecks?.length ?? 0;
  if (evidenceCount > 0 || behaviorCount > 0) {
    return `${sourceLabel(proposal.source)}提供了 ${evidenceCount} 条依据摘要和 ${behaviorCount} 条检查记录。`;
  }
  return `${sourceLabel(proposal.source)}提供了低信息量来源记录；原始来源只在展开后显示。`;
}

function rawSourceDetails(proposal: AgentProposal): Array<{ label: string; value: string }> {
  const details: Array<{ label: string; value: string }> = [];
  const reason = compactUserText(proposal.reason, 800);
  const why = compactUserText(proposal.whyOpenLifeThinksThis, 800);
  if (reason) details.push({ label: "原因原文", value: reason });
  if (why && why !== reason) details.push({ label: "判断原文", value: why });
  (proposal.evidenceSummaries ?? []).slice(0, 4).forEach((evidence, index) => {
    const summary = compactUserText(evidence.summary, 800);
    if (summary) details.push({ label: `依据 ${index + 1}`, value: summary });
  });
  (proposal.behaviorChecks ?? []).slice(0, 4).forEach((check, index) => {
    const summary = compactUserText(check.summary || check.label, 800);
    if (summary) details.push({ label: `检查 ${index + 1}`, value: summary });
  });
  return details;
}

function readableSourceDetailValue(value: string): string {
  if (!isUnexplainedInternalCopy(value)) return value;
  if (/\b(fallback|degraded)\b|降级|回退/.test(value)) {
    return "这条来源包含降级或恢复原因，原始文本在技术详情中。";
  }
  if (/\b(blocker|blocked)\b|阻断/.test(value)) {
    return "这条来源包含阻断原因，原始文本在技术详情中。";
  }
  if (/\b(governed draft|planexecute|draft)\b|草稿/.test(value)) {
    return "这条来源指向待确认计划草稿，原始文本在技术详情中。";
  }
  return "这条来源包含内部流程说明，原始文本在技术详情中。";
}

function sourceDetails(proposal: AgentProposal): Array<{ label: string; value: string }> {
  return rawSourceDetails(proposal).map(detail => ({
    ...detail,
    value: readableSourceDetailValue(detail.value),
  }));
}

function runtimeFactSummaries(proposal: AgentProposal): string[] {
  const text = [
    proposal.reason,
    proposal.whyOpenLifeThinksThis,
    proposal.sourceDetail,
    ...(proposal.evidenceSummaries ?? []).map(evidence => evidence.summary),
    ...(proposal.behaviorChecks ?? []).map(check => `${check.label ?? ""} ${check.summary ?? ""}`),
  ]
    .filter(Boolean)
    .join(" ");
  const facts: string[] = [];
  if (/\b(blocker|blocked)\b|阻断/.test(text)) {
    facts.push("存在阻断原因：需要先处理后才能继续。");
  }
  if (/\b(fallback|degraded)\b|降级|回退/.test(text)) {
    facts.push("本次包含降级或恢复路线，完整原因在技术详情中。");
  }
  if (/\b(local|local-only)\b|本地/.test(text)) {
    facts.push("本次只使用本地路线或本地证据。");
  }
  if (/\b(incomplete|not completed)\b|未完成/.test(text)) {
    facts.push("本次结果尚未完整完成。");
  }
  if (/\b(governed draft|planexecute|draft)\b|草稿/.test(text)) {
    facts.push("这是待确认计划草稿，确认前不会执行。");
  }
  return Array.from(new Set(facts));
}

export function buildReviewDecisionView(proposal: AgentProposal): ReviewDecisionView {
  const display = buildProposalDisplayModel(proposal);
  const group = groupForProposal(proposal);
  const rawDetails = rawSourceDetails(proposal);
  const fallbackEvidence = `${sourceLabel(proposal.source)} 提供了 ${metadataValueSummary(
    proposal.after
  )} 的候选更新。`;

  return {
    id: proposal.id,
    group,
    groupLabel: GROUP_LABELS[group],
    title: proposalSubject(proposal),
    subtitle: display.typeLabel,
    beforeAfter: display.diffRows,
    why: readableReason(proposal, display.intent),
    evidence: evidenceLines(proposal, fallbackEvidence),
    sourceSummary: sourceSummary(proposal),
    sourceDetails: sourceDetails(proposal),
    impactScope: impactScope(proposal),
    riskLabel: riskLabel(proposal),
    riskTone: riskTone(proposal),
    confidenceLabel: display.confidenceLabel,
    sourceLabel: sourceLabel(proposal.source),
    technicalRows: [
      ...display.technicalRows,
      ...rawDetails.map(detail => ({
        label: `来源${detail.label}`,
        value: detail.value,
      })),
    ],
  };
}

export function reviewGroupLabel(group: ReviewDecisionGroup): string {
  return GROUP_LABELS[group];
}
