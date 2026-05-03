import { Link } from "react-router-dom";
import ProviderConfigSection from "../ProviderConfigSection";
import type { AppConfig, SystemDiagnostics, RouterStatus, ModelRouterStatus } from "../../../tauri";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface ProviderTabProps {
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  diagnostics: SystemDiagnostics | null;
  routerStatus: RouterStatus | null;
  modelRouterStatus: ModelRouterStatus | null;
}

export default function ProviderTab({
  config,
  setConfig,
  diagnostics,
  routerStatus,
  modelRouterStatus,
}: ProviderTabProps) {
  return (
    <>
      <ProviderConfigSection config={config} onConfigChange={setConfig} diagnostics={diagnostics} />

      {/* Router */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">Layer 1 路由状态</h3>
        <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 space-y-2">
          <div className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
            ModelRouter 仍为灰度能力；AgentRun trace 会记录 privacy、retry、fallback 与 provider
            health 是否为估算值。
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

      {/* Model Router Provider Health */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">ModelRouter Provider Health</h3>
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
                  <div className="md:col-span-5 rounded bg-rose-50 px-2 py-1 text-xs text-rose-700">
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

      {/* Chat Proposal Settings */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">Chat Proposal 设置</h3>
        <div className="grid gap-4">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={config.chat_proposal?.enabled ?? true}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  chat_proposal: {
                    ...prev.chat_proposal,
                    enabled: e.target.checked,
                  },
                }))
              }
              className="rounded border-gray-300"
            />
            <span className="text-sm text-gray-700">启用 Chat Proposal 自动提取</span>
          </label>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-gray-500 mb-1">置信度阈值</label>
              <input
                type="number"
                min="0"
                max="1"
                step="0.1"
                value={config.chat_proposal?.confidence_threshold ?? 0.6}
                onChange={e =>
                  setConfig(prev => ({
                    ...prev,
                    chat_proposal: {
                      ...prev.chat_proposal,
                      confidence_threshold: parseFloat(e.target.value),
                    },
                  }))
                }
                className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-500 mb-1">最小消息长度</label>
              <input
                type="number"
                min="5"
                max="100"
                value={config.chat_proposal?.min_message_length ?? 10}
                onChange={e =>
                  setConfig(prev => ({
                    ...prev,
                    chat_proposal: {
                      ...prev.chat_proposal,
                      min_message_length: parseInt(e.target.value),
                    },
                  }))
                }
                className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs text-gray-500 mb-1">提取冷却时间（秒）</label>
            <input
              type="number"
              min="0"
              max="3600"
              step="60"
              value={config.chat_proposal?.cooldown_seconds ?? 300}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  chat_proposal: {
                    ...prev.chat_proposal,
                    cooldown_seconds: parseInt(e.target.value),
                  },
                }))
              }
              className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
            />
          </div>
        </div>
      </section>

      {/* Experimental Features */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">实验性功能</h3>
        <div className="grid gap-4">
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
            <span className="text-sm text-gray-700">启用 AgentLoop（Beta 双轨）</span>
          </label>

          <div className="text-xs text-gray-500 bg-gray-50 p-3 rounded-lg">
            <p>AgentLoop 启用后，Chat 将使用新的 ReAct 执行循环。</p>
            <p>如遇问题可关闭，会自动回退到旧路径。</p>
          </div>
        </div>
      </section>

      {/* Beta readiness */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">Beta 发布准备检查</h3>
        <div
          className={classNames(
            "rounded-xl border p-4",
            diagnostics?.beta_ready
              ? "border-blue-100 bg-blue-50 text-blue-900"
              : "border-amber-100 bg-amber-50 text-amber-900"
          )}
        >
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold">Beta 就绪状态</div>
              <div className="mt-1 text-xs">
                {diagnostics?.beta_ready
                  ? "Beta 就绪：核心链路、人生模型、对话验证、云端 API 均已通过。"
                  : "距离 Beta 发布还有以下事项需要完善。"}
              </div>
            </div>
            <span className="shrink-0 rounded-full bg-white/70 px-2 py-1 text-xs font-medium">
              {diagnostics?.beta_ready ? "Beta 就绪" : "待完善"}
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
            <div>首次引导：{diagnostics?.onboarding_completed ? "已完成" : "未完成"}</div>
            {!diagnostics?.model_empty && diagnostics?.builder_completion && (
              <div>构建完成度：{Math.round(diagnostics.builder_completion.overall)}%</div>
            )}
          </div>
          {diagnostics &&
            diagnostics.beta_readiness_issues &&
            diagnostics.beta_readiness_issues.length > 0 && (
              <div className="mt-3 rounded-lg bg-white/70 p-3">
                <div className="text-xs font-medium">Beta 前建议处理：</div>
                <ul className="mt-1 list-disc space-y-1 pl-4 text-xs">
                  {diagnostics.beta_readiness_issues.map(issue => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              </div>
            )}
          <div className="mt-3 flex flex-wrap gap-2">
            <Link
              to="/builder"
              className="rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
            >
              去 Builder
            </Link>
            <Link
              to="/chat"
              className="rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
            >
              去 Chat
            </Link>
            <Link
              to="/dashboard"
              className="rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
            >
              去 Dashboard
            </Link>
            <Link
              to="/versions"
              className="rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
            >
              看版本控制
            </Link>
          </div>
        </div>
      </section>
    </>
  );
}
