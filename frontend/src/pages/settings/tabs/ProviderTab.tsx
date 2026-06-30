import ProviderConfigSection from "../ProviderConfigSection";
import type { AppConfig, ModelRouterStatus, RouterStatus, SystemDiagnostics } from "../../../tauri";
import { CapabilityCard, StatusChip } from "../../../components/product/ProductPrimitives";
import { buildProviderReadinessView } from "../../../utils/providerReadiness";

interface ProviderTabProps {
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  diagnostics: SystemDiagnostics | null;
  routerStatus: RouterStatus | null;
  modelRouterStatus: ModelRouterStatus | null;
  showInternalDebug?: boolean;
  onProviderValidationChanged?: () => Promise<unknown> | unknown;
}

type ModelMode = "local" | "auto" | "cloud";

function activeMode(config: AppConfig, diagnostics: SystemDiagnostics | null): ModelMode {
  if (config.prefer_local_model ?? diagnostics?.prefer_local_model) return "local";
  if (diagnostics?.cloud_api_configured) return "cloud";
  return "auto";
}

export default function ProviderTab({
  config,
  setConfig,
  diagnostics,
  routerStatus,
  modelRouterStatus,
  onProviderValidationChanged,
}: ProviderTabProps) {
  const mode = activeMode(config, diagnostics);
  const providerReadiness = buildProviderReadinessView(diagnostics);
  const localAvailable = Boolean(diagnostics?.ollama_online);

  function setMode(nextMode: ModelMode) {
    setConfig(prev => ({
      ...prev,
      prefer_local_model: nextMode === "local",
    }));
  }

  return (
    <>
      <section className="space-y-3">
        <div>
          <h3 className="text-sm font-medium text-gray-700">模型路线</h3>
          <p className="mt-1 text-xs leading-5 text-gray-500">
            每次 Chat 回复会在运行条里说明实际路线。本地优先命中 LocalOnly 时不会调用云端。
          </p>
        </div>
        <div className="grid gap-2 rounded-lg border border-stone-200 bg-stone-50 p-1 md:grid-cols-3">
          {[
            {
              id: "local" as const,
              label: "Local only",
              desc: "隐私敏感内容只走本地。",
            },
            {
              id: "auto" as const,
              label: "Auto",
              desc: "由 ModelRouter 按隐私和可用性选择。",
            },
            {
              id: "cloud" as const,
              label: "Cloud",
              desc: "适合需要更强模型能力的任务。",
            },
          ].map(item => (
            <button
              key={item.id}
              type="button"
              onClick={() => setMode(item.id)}
              className={[
                "rounded-md px-3 py-3 text-left transition",
                mode === item.id
                  ? "bg-stone-900 text-white shadow-sm"
                  : "bg-white text-stone-700 hover:bg-stone-100",
              ].join(" ")}
            >
              <div className="text-sm font-semibold">{item.label}</div>
              <div
                className={
                  mode === item.id ? "mt-1 text-xs text-stone-200" : "mt-1 text-xs text-stone-500"
                }
              >
                {item.desc}
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="grid gap-3 md:grid-cols-3">
        <CapabilityCard
          title="本地模型"
          description="LocalOnly 和隐私敏感任务优先使用本地模型。"
          tone={localAvailable ? "ready" : "warning"}
          meta={localAvailable ? "在线" : "未就绪"}
        >
          <StatusChip label={diagnostics?.resolved_local_model || config.local_model || "local"} />
        </CapabilityCard>
        <CapabilityCard
          title="自动路由"
          description="路线选择会写入 AgentRun，并在 Chat / Runs 里展示。"
          tone={modelRouterStatus?.enabled ? "ready" : "info"}
          meta={modelRouterStatus?.enabled ? "已启用" : "灰度"}
        >
          <StatusChip label={routerStatus?.active_backend ?? "router"} tone="info" />
        </CapabilityCard>
        <CapabilityCard
          title="云端模型"
          description={providerReadiness.detail}
          tone={providerReadiness.tone}
          meta={providerReadiness.statusLabel}
        >
          <div className="flex flex-wrap gap-1.5">
            <StatusChip
              label={providerReadiness.providerLabel || config.llm?.provider || "cloud"}
              tone={providerReadiness.tone}
            />
            <StatusChip
              label={providerReadiness.actualRouteLabel}
              tone={providerReadiness.actualRouteTone}
            />
            <StatusChip
              label={providerReadiness.externalTransmissionLabel}
              tone={providerReadiness.externalTransmissionTone}
            />
          </div>
        </CapabilityCard>
      </section>

      <ProviderConfigSection
        config={config}
        onConfigChange={setConfig}
        diagnostics={diagnostics}
        onProviderValidationChanged={onProviderValidationChanged}
      />
    </>
  );
}
