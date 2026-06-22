import type { AgentRun, SystemDiagnostics } from "../tauri";

export type CapabilityTone = "ready" | "warning" | "error" | "neutral";

export type CapabilityStatusChip = {
  label: string;
  tone: CapabilityTone;
  detail?: string;
};

export type CapabilityStatusViewModel = {
  headline: string;
  detail: string;
  tone: CapabilityTone;
  primaryActionLabel: string;
  primaryActionHref: string;
  chips: CapabilityStatusChip[];
  modelRouteLabel: string;
  cloudApiStatusLabel: string;
  toolAccessLabel: string;
  toolAccessDetail: string;
};

type CloudRouteState = "none" | "configured_unvalidated" | "validated";

type GovernanceBlockerReason =
  | "model_selected_disallowed_tool"
  | "model_selected_tool_policy_blocked"
  | "web_network_policy_blocked"
  | "mcp_missing_read_target"
  | "tool_permission_required"
  | "unknown_governance_blocker";

function providerName(diagnostics: SystemDiagnostics): string {
  return diagnostics.cloud_provider || "云端模型";
}

function localModelName(diagnostics: SystemDiagnostics): string {
  return diagnostics.resolved_local_model || diagnostics.local_model || "本地模型";
}

function cloudRouteState(diagnostics: SystemDiagnostics): CloudRouteState {
  if (diagnostics.cloud_api_validated === true) return "validated";
  if (diagnostics.cloud_api_configured) return "configured_unvalidated";
  return "none";
}

function cloudConfiguredLabel(diagnostics: SystemDiagnostics): string {
  return `${providerName(diagnostics)} 已配置，连接未验证`;
}

export function cloudApiStatusLabel(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "状态读取中";
  const cloud = cloudRouteState(diagnostics);
  if (cloud === "validated") return `${providerName(diagnostics)} 已验证可用`;
  if (cloud === "configured_unvalidated") return cloudConfiguredLabel(diagnostics);
  return "未配置";
}

function routeLabel(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "模型状态读取中";
  const local = diagnostics.ollama_online;
  const cloud = cloudRouteState(diagnostics);
  if (local && cloud === "validated" && diagnostics.prefer_local_model) {
    return `本地优先 · ${localModelName(diagnostics)} · ${providerName(diagnostics)} 备用`;
  }
  if (local && cloud === "validated") return `云端优先 · ${providerName(diagnostics)} · 本地可备用`;
  if (local && cloud === "configured_unvalidated") {
    return `本地模型 · ${localModelName(diagnostics)} · ${cloudConfiguredLabel(diagnostics)}`;
  }
  if (local) return `本地模型 · ${localModelName(diagnostics)}`;
  if (cloud === "validated") return `云端可用 · ${providerName(diagnostics)}`;
  if (cloud === "configured_unvalidated") return cloudConfiguredLabel(diagnostics);
  return "模型未就绪";
}

function runRouteLabel(run: AgentRun | null | undefined): string | null {
  if (!run?.modelRoute) return null;
  const route = run.modelRoute;
  const type =
    route.routeType === "local" ? "本地" : route.routeType === "cloud" ? "云端" : route.routeType;
  return `最近实际路线 · ${type || "未知"} · ${route.provider || "unknown"} · ${
    route.model || "unknown"
  }`;
}

function toolAccess(diagnostics: SystemDiagnostics | null): {
  label: string;
  detail: string;
  tone: CapabilityTone;
} {
  if (!diagnostics) {
    return {
      label: "工具状态读取中",
      detail: "正在检查 MCP 和安全读取工具。",
      tone: "neutral",
    };
  }
  if (diagnostics.mcp_tool_count > 0) {
    return {
      label: `工具候选 ${diagnostics.mcp_tool_count}`,
      detail: "已发现 MCP 工具；每次调用仍要经过候选 allowlist、隐私策略和必要的确认流程。",
      tone: "ready",
    };
  }
  if (diagnostics.mcp_server_count > 0) {
    return {
      label: "MCP 已连接，工具未暴露",
      detail: "已注册 MCP Server，但当前没有可供 Chat 选择的安全工具。",
      tone: "warning",
    };
  }
  return {
    label: "外部读取未接入",
    detail: "天气、网页和第三方数据需要先启用安全 read-only 工具或 MCP Server。",
    tone: "warning",
  };
}

export function buildCapabilityStatusViewModel(
  diagnostics: SystemDiagnostics | null,
  pendingProposalCount: number,
  currentRun?: AgentRun | null
): CapabilityStatusViewModel {
  const tools = toolAccess(diagnostics);
  const lastRunRouteLabel = runRouteLabel(currentRun);
  if (!diagnostics) {
    return {
      headline: "能力状态读取中",
      detail: "正在检查模型、Life Model 和工具权限。",
      tone: "neutral",
      primaryActionLabel: "查看设置",
      primaryActionHref: "/settings",
      chips: [
        { label: "模型检查中", tone: "neutral" },
        { label: "Life Model 检查中", tone: "neutral" },
        { label: tools.label, tone: tools.tone, detail: tools.detail },
      ],
      modelRouteLabel: "模型状态读取中",
      cloudApiStatusLabel: cloudApiStatusLabel(null),
      toolAccessLabel: tools.label,
      toolAccessDetail: tools.detail,
    };
  }

  const modelRouteLabel = lastRunRouteLabel ?? routeLabel(diagnostics);
  const lifeModelReady = diagnostics.life_model_ready && !diagnostics.model_empty;
  const modelReady = diagnostics.chat_ready;
  const headline = modelReady
    ? tools.tone === "ready"
      ? "对话就绪，工具受治理控制"
      : "对话就绪，外部工具受限"
    : "对话需要配置";
  const detail = modelReady
    ? `${modelRouteLabel}。${tools.detail}`
    : diagnostics.readiness_issues?.[0] || "请先完成模型和数据配置。";

  return {
    headline,
    detail,
    tone: modelReady ? (tools.tone === "ready" ? "ready" : "warning") : "error",
    primaryActionLabel: modelReady && tools.tone !== "ready" ? "配置工具" : "查看能力设置",
    primaryActionHref: modelReady && tools.tone !== "ready" ? "/mcp" : "/settings",
    chips: [
      {
        label: modelRouteLabel,
        tone: modelReady ? "ready" : "error",
      },
      {
        label: lifeModelReady ? "Life Model 已加载" : "Life Model 待补全",
        tone: lifeModelReady ? "ready" : "warning",
      },
      {
        label: tools.label,
        tone: tools.tone,
        detail: tools.detail,
      },
      {
        label: pendingProposalCount > 0 ? `待确认 ${pendingProposalCount}` : "无待确认",
        tone: pendingProposalCount > 0 ? "warning" : "neutral",
      },
    ],
    modelRouteLabel,
    cloudApiStatusLabel: cloudApiStatusLabel(diagnostics),
    toolAccessLabel: tools.label,
    toolAccessDetail: tools.detail,
  };
}

export function explainGovernanceBlocker(
  rawText: string,
  diagnostics: SystemDiagnostics | null
): string | null {
  const lower = rawText.toLowerCase();
  const reason = extractGovernanceBlockerReason(lower);
  if (!reason) return null;

  const tools = toolAccess(diagnostics);
  if (reason === "model_selected_disallowed_tool") {
    return [
      "这次没有执行工具调用：本轮选择了未允许的工具或目标。",
      tools.detail,
      "你可以去“能力设置 / MCP”检查可用安全工具，或把问题改成不依赖外部实时数据的形式。",
    ].join("\n");
  }
  if (reason === "model_selected_tool_policy_blocked") {
    return [
      "这次没有执行工具调用：选择的工具未通过本轮执行策略。",
      "这通常表示风险、权限或上下文不满足当前治理规则。",
      "可以查看任务详情，或改成只读、低风险请求后重试。",
    ].join("\n");
  }
  if (reason === "web_network_policy_blocked") {
    return [
      "这次没有读取网页：当前网络或网页读取策略阻止了请求。",
      "请在 Settings 的“Tools & Permissions”里检查网络策略和安全只读工具。",
    ].join("\n");
  }
  if (reason === "mcp_missing_read_target") {
    return [
      "这次没有调用 MCP：当前没有匹配这个请求的安全读取工具。",
      tools.detail,
      "在 MCP 工具页注册对应 read-only Server 后再试。",
    ].join("\n");
  }
  if (reason === "tool_permission_required") {
    return [
      "这次操作需要你先确认权限。",
      "请到 Review 或任务控制里处理待确认项；确认前不会执行外部操作。",
    ].join("\n");
  }

  return [
    "治理策略阻止了这次操作，未执行外部工具或写入。",
    "可以查看任务详情，或在 Settings 中检查模型、工具和隐私策略。",
  ].join("\n");
}

function extractGovernanceBlockerReason(lowerText: string): GovernanceBlockerReason | null {
  if (lowerText.includes("model_selected_disallowed_tool")) return "model_selected_disallowed_tool";
  if (lowerText.includes("model_selected_tool_policy_blocked")) {
    return "model_selected_tool_policy_blocked";
  }
  if (lowerText.includes("web_network_policy_blocked")) return "web_network_policy_blocked";
  if (lowerText.includes("mcp_missing_read_target")) return "mcp_missing_read_target";
  if (lowerText.includes("tool_permission_required")) return "tool_permission_required";
  if (lowerText.includes("blocked by governance")) return "unknown_governance_blocker";
  if (lowerText.includes("main chat agent v1 blocked by governance")) {
    return "unknown_governance_blocker";
  }
  return null;
}

export function userFacingAssistantContent(
  content: string,
  diagnostics: SystemDiagnostics | null
): string {
  return explainGovernanceBlocker(content, diagnostics) ?? content;
}
