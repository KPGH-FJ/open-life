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
  const lines = [
    proposal.whyOpenLifeThinksThis?.trim(),
    ...(proposal.evidenceSummaries ?? []).map(evidence => evidence.summary?.trim()),
    ...(proposal.behaviorChecks ?? []).map(check => check.summary?.trim() || check.label?.trim()),
  ].filter((line): line is string => Boolean(line));
  return lines.length ? lines.slice(0, 5) : [fallback];
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

export function buildReviewDecisionView(proposal: AgentProposal): ReviewDecisionView {
  const display = buildProposalDisplayModel(proposal);
  const group = groupForProposal(proposal);
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
    why: proposal.reason || display.intent,
    evidence: evidenceLines(proposal, fallbackEvidence),
    impactScope: impactScope(proposal),
    riskLabel: riskLabel(proposal),
    riskTone: riskTone(proposal),
    confidenceLabel: display.confidenceLabel,
    sourceLabel: sourceLabel(proposal.source),
    technicalRows: display.technicalRows,
  };
}

export function reviewGroupLabel(group: ReviewDecisionGroup): string {
  return GROUP_LABELS[group];
}
