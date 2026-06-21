import type { DailyGoal, LifeModel } from "../types";
import { inspectDailyGoalName } from "./dailyGoalDisplayGuard";

export type LifeModelQualityIssue = {
  id: string;
  dimension: "identity" | "goals" | "capabilities" | "state";
  label: string;
  detail: string;
  actionLabel: string;
  route: string;
};

function compact(value: string | null | undefined): string {
  return value?.trim() ?? "";
}

function looksLikeFragment(value: string): boolean {
  const trimmed = compact(value);
  if (!trimmed) return false;
  if (trimmed.length <= 2) return true;
  return /^(我有|做过|固定时间|unknown|n\/a|null)$/i.test(trimmed);
}

function duplicateLabels(values: string[]): string[] {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const value of values) {
    const normalized = compact(value).toLowerCase();
    if (!normalized) continue;
    if (seen.has(normalized)) duplicates.add(compact(value));
    seen.add(normalized);
  }
  return Array.from(duplicates);
}

function collectGoalIssues(goals: DailyGoal[]): LifeModelQualityIssue[] {
  return goals
    .map(goal => ({ goal, guard: inspectDailyGoalName(goal.name) }))
    .filter(item => !item.guard.valid)
    .slice(0, 2)
    .map(item => ({
      id: `goals:${item.goal.name}`,
      dimension: "goals" as const,
      label: "目标里混入了状态或系统回执",
      detail: `「${item.goal.name}」${item.guard.reason ?? "不像一个可执行目标"}`,
      actionLabel: "去邮箱修正",
      route: "/mailbox",
    }));
}

export function getLifeModelQualityIssues(model: LifeModel | null): LifeModelQualityIssue[] {
  if (!model) return [];
  const issues: LifeModelQualityIssue[] = [];

  const identityName = compact(model.identity.name);
  if (looksLikeFragment(identityName)) {
    issues.push({
      id: "identity:name_too_short",
      dimension: "identity",
      label: "身份摘要过短",
      detail: "当前身份字段像占位符，可能不是稳定的用户画像。",
      actionLabel: "去构建补全",
      route: "/builder",
    });
  }

  issues.push(
    ...collectGoalIssues([
      ...model.goals.daily,
      ...model.goals.short_term.map(goal => ({
        name: goal.name,
        done: goal.status === "completed",
      })),
      ...model.goals.medium_term.map(goal => ({
        name: goal.name,
        done: goal.status === "completed",
      })),
      ...model.goals.long_term.map(goal => ({
        name: goal.name,
        done: goal.status === "completed",
      })),
      ...model.goals.life_goals.map(goal => ({
        name: goal.name,
        done: goal.status === "completed",
      })),
    ])
  );

  const capabilityFragments = [
    ...model.capabilities.skills.map(skill => skill.name),
    ...model.capabilities.resources.map(resource => resource.name),
    ...model.capabilities.knowledge_domains.map(domain => domain.domain),
  ].filter(looksLikeFragment);
  if (capabilityFragments.length > 0) {
    issues.push({
      id: `capabilities:fragment:${capabilityFragments[0]}`,
      dimension: "capabilities",
      label: "能力字段像碎片句",
      detail: `例如「${capabilityFragments[0]}」缺少明确能力名或上下文。`,
      actionLabel: "去邮箱确认",
      route: "/mailbox",
    });
  }

  const duplicateFocusAreas = duplicateLabels(model.state.focus_areas);
  if (duplicateFocusAreas.length > 0) {
    issues.push({
      id: `state:duplicate_focus:${duplicateFocusAreas[0]}`,
      dimension: "state",
      label: "状态标签重复",
      detail: `「${duplicateFocusAreas[0]}」出现了重复，建议合并后再作为画像依据。`,
      actionLabel: "去邮箱整理",
      route: "/mailbox",
    });
  }

  return issues.slice(0, 5);
}

export function issuesForLifeModelDimension(
  issues: LifeModelQualityIssue[],
  dimension: LifeModelQualityIssue["dimension"]
): LifeModelQualityIssue[] {
  return issues.filter(issue => issue.dimension === dimension);
}
