import type { AgentProposal } from "../tauri";

export type ProposalDisplayModel = {
  title: string;
  domain: string;
  typeLabel: string;
  impact: string;
  confidenceLabel: string;
  intent: string;
  plainImpact: string;
  beforeSummary: string;
  afterSummary: string;
  diffRows: ProposalDisplayDiffRow[];
  evidenceSummary: string;
  technicalRows: Array<{ label: string; value: string; href?: string }>;
};

export type ProposalDisplayDiffRow = {
  field: string;
  before: string;
  after: string;
  redacted: boolean;
};

const TYPE_LABELS: Record<string, string> = {
  life_model_update: "Life Model 调整",
  goal_update: "目标更新",
  state_update: "状态更新",
  preference_update: "偏好记录",
  capability_update: "能力更新",
  memory_write: "记住偏好",
  memory_archive: "整理记忆",
  tool_permission: "工具权限",
  plugin_permission: "插件权限",
  schedule_checkin: "提醒确认",
  scheduled_task: "计划任务",
  external_write_action: "外部写入",
  model_policy_change: "模型策略",
  data_export: "数据导出",
  unsupported: "暂不支持",
};

const SOURCE_LABELS: Record<string, string> = {
  builder_review: "构建",
  calibration_run: "校准",
  feedback_evolution: "反馈",
  memory_governance: "记忆整理",
  skill_runtime: "技能候选",
  plugin: "插件",
  manual: "手动调整",
  chat_conversation: "对话",
  proactive_agent: "OpenLife 主动提醒",
  planning_session: "规划",
};

const SENSITIVE_KEY_RE =
  /(token|secret|password|credential|authorization|api[_-]?key|payload|raw|content|hash|digest|body|email|import|export)/i;
const TECHNICAL_PATH_KEY_RE = /(^|[_.\]-])path($|[_.\]-])/i;
const SENSITIVE_VALUE_RE =
  /(api[_-]?key|bearer\s+|token|secret|password|credential|authorization|raw[_-]?|payload|should[_-]?not[_-]?render)/i;

export function sourceLabel(source: string): string {
  return SOURCE_LABELS[source] ?? "OpenLife";
}

export function proposalDomainLabel(proposal: AgentProposal): string {
  const path = proposal.affectedPath.toLowerCase();
  if (path.startsWith("identity.voice_style")) return "陪伴语气";
  if (path.startsWith("identity.name")) return "称呼";
  if (path.startsWith("identity.")) return "身份信息";
  if (path.startsWith("preferences.")) return "偏好";
  if (path.startsWith("state.current_focus")) return "当前焦点";
  if (path.startsWith("state.focus_areas")) return "关注领域";
  if (path.startsWith("state.")) return "当前状态";
  if (path.startsWith("capabilities.")) return "能力";
  if (path.startsWith("goals.")) return "目标";
  if (path.startsWith("memory.")) return "记忆";
  if (path.startsWith("plugins.") || path.startsWith("tools.")) return "外部能力";
  return TYPE_LABELS[proposal.proposalType] ?? proposal.proposalType.replace(/_/g, " ");
}

export function proposalTypeLabel(proposal: AgentProposal): string {
  const domain = proposalDomainLabel(proposal);
  const raw = TYPE_LABELS[proposal.proposalType] ?? proposal.proposalType.replace(/_/g, " ");
  return domain === raw ? raw : `${domain} · ${raw}`;
}

function isEmptyValue(value: unknown): boolean {
  if (value == null) return true;
  if (typeof value === "string") return value.trim().length === 0;
  if (Array.isArray(value)) return value.length === 0;
  if (typeof value === "object") return Object.keys(value as Record<string, unknown>).length === 0;
  return false;
}

function verbFor(proposal: AgentProposal): string {
  if (
    proposal.proposalType === "tool_permission" ||
    proposal.proposalType === "plugin_permission" ||
    proposal.proposalType === "scheduled_task" ||
    proposal.proposalType === "external_write_action" ||
    proposal.proposalType === "data_export"
  ) {
    return "确认";
  }
  if (proposal.proposalType === "memory_archive") return "整理";
  if (isEmptyValue(proposal.before) && !isEmptyValue(proposal.after)) return "新增";
  if (!isEmptyValue(proposal.before) && isEmptyValue(proposal.after)) return "移除";
  return "更新";
}

export function proposalSubject(proposal: AgentProposal): string {
  return `${verbFor(proposal)}${proposalDomainLabel(proposal)}`;
}

export function metadataValueSummary(value: unknown): string {
  if (value === null || value === undefined) return "空";
  if (typeof value === "string") return value.trim() ? `文本 ${value.trim().length} 字` : "空文本";
  if (typeof value === "number" || typeof value === "boolean") return `${typeof value}: ${value}`;
  if (Array.isArray(value)) return `数组 ${value.length} 项`;
  if (typeof value === "object") {
    const keys = Object.keys(value as Record<string, unknown>).sort();
    return keys.length > 0 ? `对象字段：${keys.slice(0, 6).join(", ")}` : "空对象";
  }
  return typeof value;
}

function pathFieldLabel(path: string): string {
  const match = path.match(/([A-Za-z0-9_]+)(?:\]|\))?$/);
  if (!match) return path || "值";
  return match[1].replace(/_/g, " ");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function shortPreview(value: string, maxLength = 72): string {
  const compact = value.replace(/\s+/g, " ").trim();
  if (!compact) return "空文本";
  return compact.length > maxLength ? `「${compact.slice(0, maxLength - 1)}…」` : `「${compact}」`;
}

function safeDisplayValue(value: unknown, fieldPath: string): { text: string; redacted: boolean } {
  const keySensitive = SENSITIVE_KEY_RE.test(fieldPath) || TECHNICAL_PATH_KEY_RE.test(fieldPath);
  if (typeof value === "string") {
    const sensitive = keySensitive || SENSITIVE_VALUE_RE.test(value);
    return {
      text: sensitive ? metadataValueSummary(value) : shortPreview(value),
      redacted: sensitive,
    };
  }
  if (value === null || value === undefined) {
    return { text: "空", redacted: false };
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return { text: `${value}`, redacted: false };
  }
  if (Array.isArray(value)) {
    return { text: `数组 ${value.length} 项`, redacted: keySensitive };
  }
  if (typeof value === "object") {
    return { text: metadataValueSummary(value), redacted: keySensitive };
  }
  return { text: typeof value, redacted: keySensitive };
}

function candidateDiffKeys(before: unknown, after: unknown): string[] {
  if (!isRecord(before) && !isRecord(after)) return [];
  const keys = new Set<string>();
  if (isRecord(before)) Object.keys(before).forEach(key => keys.add(key));
  if (isRecord(after)) Object.keys(after).forEach(key => keys.add(key));
  return Array.from(keys).sort().slice(0, 8);
}

function buildDiffRows(proposal: AgentProposal): ProposalDisplayDiffRow[] {
  const keys = candidateDiffKeys(proposal.before, proposal.after);
  if (keys.length === 0) {
    const field = pathFieldLabel(proposal.affectedPath);
    const before = safeDisplayValue(proposal.before, proposal.affectedPath);
    const after = safeDisplayValue(proposal.after, proposal.affectedPath);
    return [{ field, before: before.text, after: after.text, redacted: before.redacted || after.redacted }];
  }

  return keys.map(key => {
    const fieldPath = `${proposal.affectedPath}.${key}`;
    const beforeValue = isRecord(proposal.before) ? proposal.before[key] : undefined;
    const afterValue = isRecord(proposal.after) ? proposal.after[key] : undefined;
    const before = safeDisplayValue(beforeValue, fieldPath);
    const after = safeDisplayValue(afterValue, fieldPath);
    return {
      field: key.replace(/_/g, " "),
      before: before.text,
      after: after.text,
      redacted: before.redacted || after.redacted,
    };
  });
}

function evidenceSummary(proposal: AgentProposal): string {
  if (proposal.whyOpenLifeThinksThis?.trim()) return proposal.whyOpenLifeThinksThis.trim();
  const evidenceCount = proposal.evidenceSummaries?.length ?? 0;
  const behaviorCount = proposal.behaviorChecks?.length ?? 0;
  if (evidenceCount > 0 || behaviorCount > 0) {
    return `有 ${evidenceCount} 条依据摘要和 ${behaviorCount} 条行为检查可展开查看。`;
  }
  return "暂无足够依据摘要；不确定时可以先选择“稍后再说”或“改一下”。";
}

function impactText(proposal: AgentProposal): string {
  const domain = proposalDomainLabel(proposal);
  if (proposal.riskLevel === "high" || proposal.riskLevel === "critical") {
    return `会改变 OpenLife 对「${domain}」的核心理解，请确认它确实稳定。`;
  }
  if (
    proposal.proposalType === "tool_permission" ||
    proposal.proposalType === "external_write_action" ||
    proposal.proposalType === "plugin_permission"
  ) {
    return "这会影响外部工具或写入权限；未同意前不会执行。";
  }
  return `会影响 OpenLife 后续如何理解和使用你的「${domain}」信息。`;
}

function technicalString(value: unknown): string | null {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return null;
}

function appendExternalWriteTechnicalRows(
  rows: Array<{ label: string; value: string; href?: string }>,
  proposal: AgentProposal
) {
  if (proposal.proposalType !== "external_write_action" || !isRecord(proposal.after)) return;
  const after = proposal.after;
  const path = technicalString(after.path);
  const operation = technicalString(after.operation);
  const sizeBytes = technicalString(after.size_bytes);
  const contentHash = technicalString(after.content_hash);
  if (path) rows.push({ label: "外部路径", value: path });
  if (operation) rows.push({ label: "外部操作", value: operation });
  if (sizeBytes) rows.push({ label: "内容大小", value: `${sizeBytes} bytes` });
  if (contentHash) rows.push({ label: "内容摘要", value: contentHash });
}

export function buildProposalDisplayModel(proposal: AgentProposal): ProposalDisplayModel {
  const domain = proposalDomainLabel(proposal);
  const type = proposalTypeLabel(proposal);
  const technicalRows: Array<{ label: string; value: string; href?: string }> = [
    { label: "位置", value: proposal.affectedPath },
    { label: "类型", value: type },
    { label: "来源", value: sourceLabel(proposal.source) },
    { label: "状态", value: proposal.status },
    { label: "原值摘要", value: metadataValueSummary(proposal.before) },
    { label: "新值摘要", value: metadataValueSummary(proposal.after) },
  ];
  if (proposal.sourceDetail) {
    technicalRows.push({ label: "来源详情", value: metadataValueSummary(proposal.sourceDetail) });
  }
  if (proposal.runId) {
    technicalRows.push({ label: "Run", value: proposal.runId, href: `#/runs/${proposal.runId}` });
  }
  appendExternalWriteTechnicalRows(technicalRows, proposal);

  return {
    title: proposalSubject(proposal),
    domain,
    typeLabel: type,
    impact: proposal.riskLevel,
    confidenceLabel: `${Math.round(proposal.confidence * 100)}%`,
    intent: `${verbFor(proposal)}一条${domain}信息。`,
    plainImpact: impactText(proposal),
    beforeSummary: metadataValueSummary(proposal.before),
    afterSummary: metadataValueSummary(proposal.after),
    diffRows: buildDiffRows(proposal),
    evidenceSummary: evidenceSummary(proposal),
    technicalRows,
  };
}
