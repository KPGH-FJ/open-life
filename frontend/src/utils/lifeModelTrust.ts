import type { LifeModel } from "../types";
import type { AgentProposal, Model4DCompletion, SystemDiagnostics } from "../tauri";
import { inspectDailyGoalName } from "./dailyGoalDisplayGuard";
import { sourceLabel } from "./proposalDisplay";

export type LifeModelDimensionKey = "identity" | "goals" | "capabilities" | "state";

export type LifeModelDisplayQualityIssue = {
  value: string;
  reason: string;
  recoveryAction: string;
};

export type LifeModelTrustView = {
  key: LifeModelDimensionKey;
  title: string;
  statusLabel: string;
  confidenceLabel: string;
  updatedAtLabel: string;
  pendingProposalCount: number;
  evidenceSummary: string;
  sourceSummary: string;
  suppressedIssues: LifeModelDisplayQualityIssue[];
};

const DIMENSION_TITLES: Record<LifeModelDimensionKey, string> = {
  identity: "Identity",
  goals: "Goals",
  capabilities: "Capabilities",
  state: "State",
};

const DIMENSION_PREFIXES: Record<LifeModelDimensionKey, string[]> = {
  identity: ["identity."],
  goals: ["goals."],
  capabilities: ["capabilities."],
  state: ["state."],
};

function percentLabel(value: number | undefined | null): string {
  if (typeof value !== "number" || Number.isNaN(value)) return "置信度未读取";
  const normalized = value > 0 && value <= 1 ? value * 100 : value;
  return `约 ${Math.round(Math.max(0, Math.min(100, normalized)))}%`;
}

function completionFor(
  key: LifeModelDimensionKey,
  completion: Model4DCompletion | null
): number | undefined | null {
  if (!completion) return null;
  return completion[key];
}

function pendingForDimension(
  key: LifeModelDimensionKey,
  proposals: AgentProposal[]
): AgentProposal[] {
  const prefixes = DIMENSION_PREFIXES[key];
  return proposals.filter(proposal =>
    prefixes.some(prefix => proposal.affectedPath.toLowerCase().startsWith(prefix))
  );
}

function formatUpdatedAt(model: LifeModel | null): string {
  const value = model?.metadata.updated_at;
  if (!value) return "最近更新未记录";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "最近更新未记录";
  return `最近更新 ${date.toLocaleDateString("zh-CN")}`;
}

export function splitLifeModelItemsByDisplayQuality(items: string[]): {
  displayable: string[];
  suppressed: LifeModelDisplayQualityIssue[];
} {
  const displayable: string[] = [];
  const suppressed: LifeModelDisplayQualityIssue[] = [];

  items.forEach(item => {
    const guard = inspectDailyGoalName(item);
    if (guard.valid) {
      displayable.push(item);
    } else {
      suppressed.push({
        value: item,
        reason: guard.reason ?? "这看起来像原始抽取文本。",
        recoveryAction: guard.recoveryAction ?? "请在 Mailbox 中确认后再进入正式摘要。",
      });
    }
  });

  return { displayable, suppressed };
}

export function buildLifeModelTrustViews({
  lifeModel,
  diagnostics,
  completion,
  pendingProposals,
  suppressedByDimension,
}: {
  lifeModel: LifeModel | null;
  diagnostics: SystemDiagnostics | null;
  completion: Model4DCompletion | null;
  pendingProposals: AgentProposal[];
  suppressedByDimension: Record<LifeModelDimensionKey, LifeModelDisplayQualityIssue[]>;
}): Record<LifeModelDimensionKey, LifeModelTrustView> {
  const output = {} as Record<LifeModelDimensionKey, LifeModelTrustView>;

  (Object.keys(DIMENSION_TITLES) as LifeModelDimensionKey[]).forEach(key => {
    const proposals = pendingForDimension(key, pendingProposals);
    const evidenceSources = proposals.map(proposal => sourceLabel(proposal.source));
    const sourceSummary = evidenceSources.length
      ? Array.from(new Set(evidenceSources)).slice(0, 3).join(" / ")
      : "当前没有待确认来源";
    const evidenceSummary =
      proposals[0]?.whyOpenLifeThinksThis?.trim() ||
      proposals[0]?.evidenceSummaries?.[0]?.summary ||
      (suppressedByDimension[key].length
        ? `${suppressedByDimension[key].length} 条原始抽取文本已降级为待确认证据。`
        : "正式摘要来自已确认的 Life Model 视图。");
    const confidence = completionFor(key, completion);
    const pendingCount = proposals.length;

    output[key] = {
      key,
      title: DIMENSION_TITLES[key],
      statusLabel:
        pendingCount > 0 ? "有待确认更新" : diagnostics?.model_empty ? "待构建" : "已确认视图",
      confidenceLabel: percentLabel(confidence),
      updatedAtLabel: formatUpdatedAt(lifeModel),
      pendingProposalCount: pendingCount,
      evidenceSummary,
      sourceSummary,
      suppressedIssues: suppressedByDimension[key],
    };
  });

  return output;
}
