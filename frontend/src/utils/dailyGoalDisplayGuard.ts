import type { DailyGoal } from "../types";

export type TodayCardType =
  | "goal"
  | "task"
  | "suggestion"
  | "state_signal"
  | "pending_proposal"
  | "blocker";

export type DailyGoalDisplayGuard = {
  valid: boolean;
  cardType: Extract<TodayCardType, "goal" | "suggestion" | "state_signal" | "blocker">;
  reason?: string;
  recoveryAction?: string;
};

const STATE_METRIC_KEYS = [
  "qapressure",
  "qa_pressure",
  "qa pressure",
  "energy",
  "mood",
  "pressure",
  "confidence",
  "能量",
  "精力",
  "心情",
  "情绪",
  "压力",
  "信心",
  "置信度",
];

const STATE_RECEIPT_PREFIXES = [
  "已记录状态",
  "状态已记录",
  "记录状态",
  "recorded state",
  "state recorded",
];

const SYSTEM_FEEDBACK_PREFIXES = [
  "已添加今日目标",
  "今日目标",
  "用法：/state",
  "用法:/state",
  "/state ",
  "格式不正确",
  "无法识别 /goal",
  "没有添加今日目标",
  "没有保存为今日目标",
  "暂时无法发送普通对话",
  "这次没有执行工具调用",
  "这次没有读取网页",
  "这次没有调用 mcp",
  "治理策略阻止了这次操作",
  "deepseek 鉴权失败",
  "本地 ollama 不可用",
  "云端模型服务暂时不可用",
  "网络连接异常",
];

const SUGGESTION_PREFIXES = ["建议", "可以考虑", "可以先", "推荐", "try ", "consider "];

const GOVERNANCE_BLOCKER_TOKENS = [
  "blocked by governance",
  "model_selected_disallowed_tool",
  "model_selected_tool_policy_blocked",
  "web_network_policy_blocked",
  "mcp_missing_read_target",
  "tool_permission_required",
];

function stripLeadingMarks(value: string): string {
  return value.replace(/^[\s✅✔︎✓\-[\]().:：]+/, "").trim();
}

function hasStateAssignmentShape(value: string): boolean {
  const compact = value.trim();
  if (!compact.includes("=")) return false;
  return /^[A-Za-z0-9_.\-\u4e00-\u9fff\s]+=\s*[-+]?\d+(\.\d+)?\s*\S+$/u.test(compact);
}

function hasMetricSampleShape(value: string): boolean {
  return /\b[A-Za-z][A-Za-z0-9_.-]{1,40}\s*=\s*[-+]?\d+(\.\d+)?\s*\S*/.test(value);
}

function hasStateMetricToken(value: string): boolean {
  const lower = value.toLowerCase();
  return STATE_METRIC_KEYS.some(key => {
    const normalizedKey = key.toLowerCase();
    if (/^[a-z0-9_ ]+$/i.test(normalizedKey)) {
      return new RegExp(
        `(^|[^a-z0-9_])${normalizedKey.replace(/\s+/g, "\\s+")}([^a-z0-9_]|$)`,
        "i"
      ).test(lower);
    }
    return lower.includes(normalizedKey);
  });
}

function hasStateMetricShape(value: string): boolean {
  if (!hasStateMetricToken(value)) return false;
  return /[:=：]\s*\S+/.test(value) || /[-+]?\d+(\.\d+)?\s*(points?|分|\/10|级|%)/i.test(value);
}

function hasGovernanceBlockerToken(value: string): boolean {
  const lower = value.toLowerCase();
  return (
    GOVERNANCE_BLOCKER_TOKENS.some(token => lower.includes(token)) ||
    /\bmodel_selected_[a-z0-9_]+\b/.test(lower)
  );
}

export function inspectDailyGoalName(name: string): DailyGoalDisplayGuard {
  const trimmed = name.trim();
  const normalized = stripLeadingMarks(trimmed).toLowerCase();

  if (!trimmed) {
    return {
      valid: false,
      cardType: "blocker",
      reason: "目标为空。",
      recoveryAction: "请输入一个可以执行的行动或目标。",
    };
  }

  if (STATE_RECEIPT_PREFIXES.some(prefix => normalized.startsWith(prefix.toLowerCase()))) {
    return {
      valid: false,
      cardType: "state_signal",
      reason: "这看起来像状态记录回执，不是今日目标。",
      recoveryAction: "请把压力、精力等数值保留在状态记录里，再单独写一个可执行目标。",
    };
  }

  if (hasStateMetricShape(trimmed)) {
    return {
      valid: false,
      cardType: "state_signal",
      reason: "这看起来像状态指标，不是目标或任务。",
      recoveryAction: "状态指标应留在状态信号里；目标应描述一个用户确认过的行动结果。",
    };
  }

  if (SUGGESTION_PREFIXES.some(prefix => normalized.startsWith(prefix.toLowerCase()))) {
    return {
      valid: false,
      cardType: "suggestion",
      reason: "这看起来像建议，不是用户已确认目标。",
      recoveryAction: "建议需要用户确认后，才应进入今日目标。",
    };
  }

  if (
    SYSTEM_FEEDBACK_PREFIXES.some(prefix => normalized.startsWith(prefix.toLowerCase())) ||
    trimmed.includes("暂无今日目标")
  ) {
    return {
      valid: false,
      cardType: "blocker",
      reason: "这看起来像系统反馈文本，不是用户目标。",
      recoveryAction: "请改成一句你今天真的要推进的事情。",
    };
  }

  if (hasGovernanceBlockerToken(trimmed)) {
    return {
      valid: false,
      cardType: "blocker",
      reason: "这看起来像系统或治理阻断说明，不是用户目标。",
      recoveryAction: "请先处理能力设置、权限或工具问题，再单独写一个可执行目标。",
    };
  }

  if (hasStateAssignmentShape(trimmed) || hasMetricSampleShape(trimmed)) {
    return {
      valid: false,
      cardType: "state_signal",
      reason: "这看起来像 key = value 状态样本，不是目标。",
      recoveryAction: "状态样本应留在状态视图；目标应描述一个行动结果。",
    };
  }

  return { valid: true, cardType: "goal" };
}

export function isDisplayableDailyGoal(goal: DailyGoal): boolean {
  return inspectDailyGoalName(goal.name).valid;
}

export function splitDailyGoalsByDisplayQuality(goals: DailyGoal[]): {
  displayable: DailyGoal[];
  suspicious: Array<{ goal: DailyGoal; guard: DailyGoalDisplayGuard; originalIndex: number }>;
} {
  const displayable: DailyGoal[] = [];
  const suspicious: Array<{
    goal: DailyGoal;
    guard: DailyGoalDisplayGuard;
    originalIndex: number;
  }> = [];

  goals.forEach((goal, originalIndex) => {
    const guard = inspectDailyGoalName(goal.name);
    if (guard.valid) {
      displayable.push(goal);
    } else {
      suspicious.push({ goal, guard, originalIndex });
    }
  });

  return { displayable, suspicious };
}
