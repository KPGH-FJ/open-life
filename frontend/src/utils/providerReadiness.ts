import type { ProductTone } from "../components/product/ProductPrimitives";
import type { RouteIdentity, RuntimeRouteEvidence, SystemDiagnostics } from "../tauri";

export type ProviderReadinessView = {
  providerLabel: string;
  status: string;
  statusLabel: string;
  detail: string;
  tone: ProductTone;
  cloudReady: boolean;
  configured: boolean;
  actualRouteLabel: string;
  actualRouteTone: ProductTone;
  externalTransmissionLabel: string;
  externalTransmissionTone: ProductTone;
};

function providerName(diagnostics: SystemDiagnostics | null): string {
  return diagnostics?.cloud_provider || "云端模型";
}

function normalizedStatus(evidence: RuntimeRouteEvidence | null | undefined): string | null {
  return evidence?.provider_readiness?.validation_status || null;
}

function fallbackStatus(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics?.cloud_api_configured) return "unconfigured";
  if (diagnostics.cloud_api_validation_status) return diagnostics.cloud_api_validation_status;
  if (diagnostics.cloud_api_validated === true) return "validated";
  return "unvalidated";
}

function isScriptedProofStatus(status: string | null | undefined): boolean {
  return status === "scripted_provider_probe" || status === "scripted_dogfood";
}

function routeDisplay(route?: RouteIdentity | null): { label: string; tone: ProductTone } {
  if (!route) return { label: "实际路线未验证", tone: "neutral" };
  const type = route.route_type;
  if (type === "local") return { label: `最近实际本地 · ${route.provider}`, tone: "ready" };
  if (type === "cloud") return { label: `最近实际云端 · ${route.provider}`, tone: "warning" };
  if (type === "agent_runtime") return { label: "运行时事实路径 · 未调用模型", tone: "info" };
  if (type === "scripted") return { label: "脚本化开发 proof", tone: "info" };
  return { label: "实际路线未验证", tone: "neutral" };
}

function externalTransmissionDisplay(status?: RuntimeRouteEvidence["external_transmission"]): {
  label: string;
  tone: ProductTone;
} {
  if (status === "sent") return { label: "运行证据显示已外发", tone: "warning" };
  if (status === "not_sent") return { label: "运行证据显示未外发", tone: "ready" };
  if (status === "unknown") return { label: "当前证据无法判断是否外发", tone: "neutral" };
  return { label: "外发记录未接入", tone: "neutral" };
}

export function buildProviderReadinessView(
  diagnostics: SystemDiagnostics | null
): ProviderReadinessView {
  const evidence = diagnostics?.runtime_route_evidence ?? null;
  const status = normalizedStatus(evidence) ?? fallbackStatus(diagnostics);
  const providerLabel = evidence?.provider_readiness?.preferred || providerName(diagnostics);
  const configured =
    evidence?.provider_readiness?.configured ?? Boolean(diagnostics?.cloud_api_configured);
  const actualRoute = evidence?.actual_route ?? evidence?.last_completed_route ?? null;
  const actual = routeDisplay(actualRoute);
  const external = externalTransmissionDisplay(evidence?.external_transmission);

  if (!configured || status === "unconfigured") {
    return {
      providerLabel,
      status,
      statusLabel: "Not configured",
      detail: "没有可证明可用的云端 provider。",
      tone: "neutral",
      cloudReady: false,
      configured: false,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "validated") {
    return {
      providerLabel,
      status,
      statusLabel: "Validated",
      detail: "配置和最近验证记录匹配；这只表示 cloud 可用，不代表最近任务实际使用。",
      tone: "ready",
      cloudReady: true,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "failed") {
    return {
      providerLabel,
      status,
      statusLabel: "Failed validation",
      detail: diagnostics?.cloud_api_last_error
        ? `最近连接验证失败：${diagnostics.cloud_api_last_error}`
        : "最近连接验证失败，不能当作 cloud-ready。",
      tone: "danger",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "stale") {
    return {
      providerLabel,
      status,
      statusLabel: "Validation stale",
      detail: "配置或网络策略已变化，或验证已过期；需要重新验证后才能当作可用。",
      tone: "warning",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (isScriptedProofStatus(status)) {
    return {
      providerLabel,
      status,
      statusLabel: "Local test proof only",
      detail: "当前只有脚本化开发 proof，不是 external cloud ready。",
      tone: "info",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "remote_unknown") {
    return {
      providerLabel,
      status,
      statusLabel: "Remote state unknown",
      detail: "本地无法确认远端终态；不能当作成功、失败或已远端取消。",
      tone: "warning",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "runtime_generation_incoherent") {
    return {
      providerLabel,
      status,
      statusLabel: "Runtime generation incoherent",
      detail: "Provider 配置与执行适配器不属于同一运行代；系统已失败关闭。",
      tone: "danger",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "validation_record_corrupt" || status === "validation_record_io_error") {
    return {
      providerLabel,
      status,
      statusLabel:
        status === "validation_record_corrupt"
          ? "Validation record corrupt"
          : "Validation record unreadable",
      detail: "持久化验证证据不可用；当前状态保持 unknown，不能当作 cloud-ready。",
      tone: "danger",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  if (status === "unknown") {
    return {
      providerLabel,
      status,
      statusLabel: "Provider state unknown",
      detail: "现有证据不足以确认 Provider 可用。",
      tone: "neutral",
      cloudReady: false,
      configured: true,
      actualRouteLabel: actual.label,
      actualRouteTone: actual.tone,
      externalTransmissionLabel: external.label,
      externalTransmissionTone: external.tone,
    };
  }

  return {
    providerLabel,
    status,
    statusLabel: "Configured, not validated",
    detail: "配置存在，但没有通过连接验证；不能当作 cloud-ready。",
    tone: "warning",
    cloudReady: false,
    configured: true,
    actualRouteLabel: actual.label,
    actualRouteTone: actual.tone,
    externalTransmissionLabel: external.label,
    externalTransmissionTone: external.tone,
  };
}
