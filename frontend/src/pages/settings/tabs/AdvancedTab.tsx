import { Link } from "react-router-dom";
import type { ReactNode } from "react";
import type { AppConfig, ModelRouterStatus, RouterStatus, SystemDiagnostics } from "../../../tauri";
import {
  advancedRoutePath,
  diagnosticsUsageReadinessIssues,
  diagnosticsUsageReady,
} from "../../../productShellContract";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface AdvancedTabProps {
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  diagnostics: SystemDiagnostics | null;
  routerStatus: RouterStatus | null;
  modelRouterStatus: ModelRouterStatus | null;
  showInternalDebug: boolean;
  pluginSection: ReactNode;
  experimentalSection?: ReactNode;
}

function usageReady(diagnostics: SystemDiagnostics | null): boolean {
  if (!diagnostics) return false;
  return diagnosticsUsageReady(diagnostics);
}

function usageReadinessIssues(diagnostics: SystemDiagnostics | null): string[] {
  if (!diagnostics) return [];
  return diagnosticsUsageReadinessIssues(diagnostics);
}

export default function AdvancedTab({
  config,
  setConfig,
  diagnostics,
  routerStatus,
  modelRouterStatus,
  showInternalDebug,
  pluginSection,
  experimentalSection,
}: AdvancedTabProps) {
  return (
    <>
      <section className="space-y-4 border-t pt-4">
        <div>
          <h3 className="text-sm font-medium text-gray-700">高级连接</h3>
          <p className="mt-1 text-xs text-gray-500">
            MCP / A2A / Plugin 是开发者和高级用户入口；普通用户默认只需要在 Tools & Permissions
            看能力和授权。
          </p>
        </div>
        <div className="grid gap-3 md:grid-cols-3">
          <Link
            to={advancedRoutePath("McpTools")}
            className="rounded-lg border border-stone-200 bg-white p-4 text-sm hover:bg-stone-50"
          >
            <div className="font-semibold text-stone-950">MCP / Tools</div>
            <div className="mt-1 text-xs text-stone-500">
              {diagnostics?.mcp_server_count ?? 0} servers · {diagnostics?.mcp_tool_count ?? 0}{" "}
              tools
            </div>
          </Link>
          <Link
            to={advancedRoutePath("A2A")}
            className="rounded-lg border border-stone-200 bg-white p-4 text-sm hover:bg-stone-50"
          >
            <div className="font-semibold text-stone-950">A2A</div>
            <div className="mt-1 text-xs text-stone-500">外部 Agent 连接与发送确认。</div>
          </Link>
          <div className="rounded-lg border border-stone-200 bg-white p-4 text-sm">
            <div className="font-semibold text-stone-950">Plugins</div>
            <div className="mt-1 text-xs text-stone-500">本地插件和 manifest 诊断。</div>
          </div>
        </div>
        {pluginSection}
      </section>

      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">ModelRouter internals</h3>
        <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 space-y-2">
          <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
            这些是诊断字段。普通任务应在 Chat 和 Runs 里看用户语言的路线解释。
          </div>
          <div className="flex items-center justify-between">
            <span>当前后端</span>
            <span className="font-medium uppercase">
              {routerStatus?.active_backend ?? "unknown"}
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span>ONNX 可用</span>
            <span>{routerStatus?.onnx_available ? "是" : "否"}</span>
          </div>
          <div className="flex items-center justify-between">
            <span>已自动降级</span>
            <span>{routerStatus?.onnx_disabled ? "是" : "否"}</span>
          </div>
          <div className="flex items-center justify-between">
            <span>延迟阈值</span>
            <span>
              {routerStatus ? `${Math.round(routerStatus.latency_threshold_us / 1000)}ms` : "-"}
            </span>
          </div>
        </div>
      </section>

      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">Provider 健康状态</h3>
        <div className="rounded-lg border border-slate-200 bg-white">
          <div className="flex items-center justify-between border-b border-slate-100 px-4 py-3 text-sm">
            <span className="font-medium text-slate-700">
              状态：{modelRouterStatus?.enabled ? "已启用" : "灰度未启用"}
            </span>
            <span className="text-xs text-slate-500">
              {modelRouterStatus?.lastCheckAt
                ? new Date(modelRouterStatus.lastCheckAt).toLocaleString()
                : "未检查"}
            </span>
          </div>
          {modelRouterStatus?.message && (
            <div className="border-b border-amber-100 bg-amber-50 px-4 py-2 text-xs text-amber-800">
              {modelRouterStatus.message}
            </div>
          )}
          <div className="divide-y divide-slate-100">
            {(modelRouterStatus?.providers ?? []).map(provider => (
              <div key={provider.name} className="grid gap-2 px-4 py-3 text-sm md:grid-cols-5">
                <div className="font-medium text-slate-800">{provider.name}</div>
                <div className={provider.available ? "text-emerald-700" : "text-rose-700"}>
                  {provider.available ? "available" : "unavailable"}
                </div>
                <div className="text-slate-600">{provider.enabled ? "enabled" : "disabled"}</div>
                <div className="text-slate-600">
                  {provider.latencyMs != null ? `${provider.latencyMs}ms` : "latency n/a"}
                </div>
                <div className="text-xs text-slate-500">
                  {provider.healthIsEstimated ? "estimated" : "probed"}
                </div>
                {provider.lastError && (
                  <div className="rounded bg-rose-50 px-2 py-1 text-xs text-rose-700 md:col-span-5">
                    {provider.lastError}
                  </div>
                )}
              </div>
            ))}
            {(!modelRouterStatus || modelRouterStatus.providers.length === 0) && (
              <div className="px-4 py-3 text-sm text-slate-500">暂无 provider health 数据。</div>
            )}
          </div>
        </div>
      </section>

      {showInternalDebug && (
        <section className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">内部调试功能</h3>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={config.experimental_context_assembler ?? false}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  experimental_context_assembler: e.target.checked,
                }))
              }
              className="rounded border-gray-300"
            />
            <span className="text-sm text-gray-700">启用 ContextAssembler V2（灰度测试）</span>
          </label>

          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={config.use_agent_loop ?? false}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  use_agent_loop: e.target.checked,
                }))
              }
              className="rounded border-gray-300"
            />
            <span className="text-sm text-gray-700">启用 AgentLoop（内部调试）</span>
          </label>
        </section>
      )}

      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">使用准备诊断</h3>
        <div
          className={classNames(
            "rounded-xl border p-4",
            usageReady(diagnostics)
              ? "border-blue-100 bg-blue-50 text-blue-900"
              : "border-amber-100 bg-amber-50 text-amber-900"
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold">使用准备状态</div>
              <div className="mt-1 text-xs">
                {usageReady(diagnostics)
                  ? "使用准备就绪：核心链路、人生模型、对话验证和模型后端均已通过当前检查。"
                  : "继续处理以下事项后，默认体验会更稳定。"}
              </div>
            </div>
            <span className="shrink-0 rounded-full bg-white/70 px-2 py-1 text-xs font-medium">
              {usageReady(diagnostics) ? "已就绪" : "待完善"}
            </span>
          </div>
          <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
            <div>核心链路：{diagnostics?.chat_ready ? "就绪" : "未就绪"}</div>
            <div>人生模型：{diagnostics?.model_empty ? "未构建" : "已构建"}</div>
            <div>
              对话验证：
              {diagnostics?.chat_session_count && diagnostics.chat_session_count > 0
                ? `已验证（${diagnostics.chat_session_count} 个会话）`
                : "未验证"}
            </div>
            <div>云端 API：{diagnostics?.cloud_api_configured ? "已配置" : "未配置"}</div>
          </div>
          {usageReadinessIssues(diagnostics).length ? (
            <div className="mt-3 rounded-lg bg-white/70 p-3">
              <div className="text-xs font-medium">建议处理：</div>
              <ul className="mt-1 list-disc space-y-1 pl-4 text-xs">
                {usageReadinessIssues(diagnostics).map(issue => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      </section>

      {experimentalSection}
    </>
  );
}
