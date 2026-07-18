import { useState } from "react";
import { testLlmConnection } from "../../tauri";
import type {
  AgentRuntimeMode,
  AppConfig,
  CloudApiValidationStatus,
  SystemDiagnostics,
} from "../../tauri";

const PROVIDER_PRESETS: Record<
  string,
  { label: string; base: string; model: string; embed: boolean; test_url: string }
> = {
  deepseek: {
    label: "DeepSeek",
    base: "https://api.deepseek.com",
    model: "deepseek-chat",
    embed: false,
    test_url: "https://api.deepseek.com/chat/completions",
  },
  openai: {
    label: "OpenAI",
    base: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    embed: true,
    test_url: "https://api.openai.com/v1/chat/completions",
  },
  openrouter: {
    label: "OpenRouter",
    base: "https://openrouter.ai/api/v1",
    model: "openai/gpt-4o-mini",
    embed: true,
    test_url: "https://openrouter.ai/api/v1/chat/completions",
  },
  siliconflow: {
    label: "SiliconFlow",
    base: "https://api.siliconflow.cn/v1",
    model: "Qwen/Qwen2.5-72B-Instruct",
    embed: false,
    test_url: "https://api.siliconflow.cn/v1/chat/completions",
  },
  moonshot: {
    label: "Moonshot/Kimi",
    base: "https://api.moonshot.cn/v1",
    model: "moonshot-v1-8k",
    embed: false,
    test_url: "https://api.moonshot.cn/v1/chat/completions",
  },
};

const LOCAL_MODEL_OPTIONS = [
  { value: "llama2", label: "Llama 2" },
  { value: "llama3", label: "Llama 3" },
  { value: "llama3.1", label: "Llama 3.1" },
  { value: "mistral", label: "Mistral" },
  { value: "qwen2.5", label: "Qwen 2.5" },
  { value: "gemma2", label: "Gemma 2" },
  { value: "phi4", label: "Phi-4" },
  { value: "deepseek-r1:14b", label: "DeepSeek R1 14B" },
  { value: "deepseek-r1:8b", label: "DeepSeek R1 8B" },
];

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface ProviderConfigSectionProps {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
  diagnostics: SystemDiagnostics | null;
  onProviderValidationChanged?: () => Promise<unknown> | unknown;
}

export default function ProviderConfigSection({
  config,
  onConfigChange,
  diagnostics,
  onProviderValidationChanged,
}: ProviderConfigSectionProps) {
  const [apiTestLoading, setApiTestLoading] = useState(false);
  const [apiTestResult, setApiTestResult] = useState<{ ok: boolean; text: string } | null>(null);

  const provider = config.llm.provider ?? "deepseek";
  const preset = PROVIDER_PRESETS[provider];
  const runtimeMode = config.runtime_mode ?? "local_first_default";
  const providerValidation = providerValidationView(diagnostics);
  const isDeepSeekReasoner = config.llm.chat_model === "deepseek-reasoner";
  const ollamaServiceOnline = diagnostics?.ollama_service_online ?? null;
  const resolvedLocalModel = diagnostics?.resolved_local_model ?? null;
  const selectedLocalModel = config.local_model || diagnostics?.local_model || "本地模型";
  const presetLocalModelValues = new Set(LOCAL_MODEL_OPTIONS.map(opt => opt.value));
  const detectedLocalModelOptions = (diagnostics?.ollama_models ?? []).filter(
    (model, index, models) =>
      model.name.trim() &&
      !presetLocalModelValues.has(model.name) &&
      models.findIndex(candidate => candidate.name === model.name) === index
  );
  const selectLocalModelValues = new Set([
    ...LOCAL_MODEL_OPTIONS.map(opt => opt.value),
    ...detectedLocalModelOptions.map(model => model.name),
  ]);
  const shouldShowCurrentLocalModelOption =
    config.local_model.trim() !== "" && !selectLocalModelValues.has(config.local_model);

  const updateLlm = (field: keyof AppConfig["llm"], value: string) => {
    onConfigChange({
      ...config,
      llm: { ...config.llm, [field]: value },
    });
  };

  const updateProvider = (p: NonNullable<AppConfig["llm"]["provider"]>) => {
    const pr = PROVIDER_PRESETS[p];
    onConfigChange({
      ...config,
      llm: {
        ...config.llm,
        provider: p,
        openai_base: pr.base,
        chat_model: pr.model,
        embedding_enabled: pr.embed,
      },
    });
  };

  const updateRuntimeMode = (mode: AgentRuntimeMode) => {
    onConfigChange({
      ...config,
      runtime_mode: mode,
    });
  };

  const handleTestApiKey = async () => {
    setApiTestLoading(true);
    try {
      const res = await testLlmConnection(config);
      setApiTestResult({ ok: res.ok, text: `${res.provider}: ${res.message}` });
      await onProviderValidationChanged?.();
    } catch (e: any) {
      setApiTestResult({ ok: false, text: e.message || "测试失败" });
      await onProviderValidationChanged?.();
    } finally {
      setApiTestLoading(false);
    }
  };

  return (
    <>
      <section id="llm-settings" className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">LLM 配置</h3>
        <div className="grid gap-4">
          <div>
            <label className="block text-xs text-gray-500 mb-1">Agent Runtime Mode</label>
            <div className="grid gap-2 rounded-lg border border-stone-200 bg-stone-50 p-1 sm:grid-cols-2">
              {[
                {
                  id: "local_first_default" as const,
                  label: "Local-first default",
                  desc: "保持现有用户行为和隐私边界。",
                },
                {
                  id: "capability_first" as const,
                  label: "Capability-first",
                  desc: "普通上下文更完整，credentials 仍会被拦截。",
                },
              ].map(item => (
                <button
                  key={item.id}
                  type="button"
                  onClick={() => updateRuntimeMode(item.id)}
                  className={classNames(
                    "rounded-md px-3 py-2 text-left transition",
                    runtimeMode === item.id
                      ? "bg-stone-900 text-white shadow-sm"
                      : "bg-white text-stone-700 hover:bg-stone-100"
                  )}
                >
                  <div className="text-sm font-semibold">{item.label}</div>
                  <div
                    className={
                      runtimeMode === item.id
                        ? "mt-1 text-xs leading-4 text-stone-200"
                        : "mt-1 text-xs leading-4 text-stone-500"
                    }
                  >
                    {item.desc}
                  </div>
                </button>
              ))}
            </div>
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">云端模型 Provider</label>
            <div className="flex flex-wrap gap-2">
              {Object.entries(PROVIDER_PRESETS).map(([key, p]) => (
                <button
                  key={key}
                  onClick={() => updateProvider(key as NonNullable<AppConfig["llm"]["provider"]>)}
                  className={classNames(
                    "rounded-md px-3 py-1.5 text-xs font-medium border transition",
                    provider === key
                      ? "bg-stone-900 text-amber-50 border-stone-900"
                      : "bg-white text-gray-700 border-gray-200 hover:bg-gray-50"
                  )}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">API Base URL</label>
            <input
              type="text"
              value={config.llm.openai_base}
              onChange={e => updateLlm("openai_base", e.target.value)}
              className="w-full border rounded-md px-3 py-2 text-sm"
              placeholder={preset.base || "https://api.example.com/v1"}
            />
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">API Key</label>
            <div className="flex gap-2">
              <input
                type="password"
                value={config.llm.openai_key}
                onChange={e => updateLlm("openai_key", e.target.value)}
                className="flex-1 border rounded-md px-3 py-2 text-sm"
                placeholder="sk-..."
              />
              <button
                onClick={handleTestApiKey}
                disabled={apiTestLoading}
                className="px-3 py-2 bg-slate-600 text-white rounded-md text-sm font-medium hover:bg-slate-700 disabled:opacity-50"
              >
                {apiTestLoading ? "测试中..." : "测试连接"}
              </button>
            </div>
            {apiTestResult && (
              <div
                className={classNames(
                  "mt-1 text-xs",
                  apiTestResult.ok ? "text-emerald-600" : "text-red-600"
                )}
              >
                {apiTestResult.text}
              </div>
            )}
            <div
              className={classNames(
                "mt-2 rounded-md border px-3 py-2 text-xs",
                providerValidation.tone === "ready" &&
                  "border-emerald-200 bg-emerald-50 text-emerald-800",
                providerValidation.tone === "warning" &&
                  "border-amber-200 bg-amber-50 text-amber-800",
                providerValidation.tone === "error" && "border-red-200 bg-red-50 text-red-700",
                providerValidation.tone === "neutral" && "border-gray-200 bg-gray-50 text-gray-600"
              )}
            >
              <div className="font-medium">{providerValidation.label}</div>
              {providerValidation.detail && (
                <div className="mt-1 leading-5">{providerValidation.detail}</div>
              )}
            </div>
            {diagnostics?.config_source === "env_var" && (
              <div className="mt-1 text-xs text-blue-600">
                检测到 API Key 来自环境变量，配置文件中无需填写
              </div>
            )}
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-gray-500 mb-1">Chat Model</label>
              <input
                type="text"
                value={config.llm.chat_model}
                onChange={e => updateLlm("chat_model", e.target.value)}
                className="w-full border rounded-md px-3 py-2 text-sm"
                placeholder={preset.model || "model-name"}
              />
              {isDeepSeekReasoner && (
                <div className="mt-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
                  <div className="font-medium">当前选择的是 DeepSeek 推理模型</div>
                  <div className="mt-1">
                    `deepseek-reasoner`
                    更适合长推理，不适合作为桌面端主聊天模型。当前聊天流会自动兜底为
                    `deepseek-chat`，但为了减少等待和排障成本，建议你直接改成 `deepseek-chat`。
                  </div>
                  <button
                    type="button"
                    onClick={() => updateLlm("chat_model", "deepseek-chat")}
                    className="mt-2 rounded-md border border-amber-300 bg-white px-2 py-1 text-xs font-medium text-amber-900 hover:bg-amber-100"
                  >
                    一键改为 deepseek-chat
                  </button>
                </div>
              )}
            </div>
            <div>
              <label className="block text-xs text-gray-500 mb-1">Embedding Model</label>
              <input
                type="text"
                value={config.llm.embedding_model}
                onChange={e => updateLlm("embedding_model", e.target.value)}
                className="w-full border rounded-md px-3 py-2 text-sm"
                disabled={config.llm.embedding_enabled === false}
                placeholder="text-embedding-3-small"
              />
              <label className="mt-2 flex items-center gap-2 text-xs text-gray-600">
                <input
                  type="checkbox"
                  checked={config.llm.embedding_enabled !== false}
                  onChange={e =>
                    onConfigChange({
                      ...config,
                      llm: { ...config.llm, embedding_enabled: e.target.checked },
                    })
                  }
                />
                启用远端 embedding（DeepSeek 默认关闭）
              </label>
              {provider === "deepseek" && (
                <div className="mt-2 rounded-md border border-blue-200 bg-blue-50 px-3 py-2 text-xs text-blue-800">
                  DeepSeek 主要用于聊天，不建议把它当作长期记忆的远端 embedding 服务。 OpenLife
                  会优先使用本地/Ollama 或哈希向量回退；如果历史聊天很多但记忆仍为空，
                  请先保存当前设置，再去恢复控制台重建向量索引。
                </div>
              )}
            </div>
          </div>
        </div>
      </section>

      <section id="local-model-settings" className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">本地模型（Ollama）</h3>
        <div className="flex items-center gap-3">
          <input
            id="prefer_local"
            type="checkbox"
            checked={config.prefer_local_model}
            onChange={e => onConfigChange({ ...config, prefer_local_model: e.target.checked })}
            className="h-4 w-4"
          />
          <label htmlFor="prefer_local" className="text-sm text-gray-700">
            优先使用本地模型（Ollama）
          </label>
        </div>
        <div className="grid grid-cols-2 gap-4 items-end">
          <div>
            <label className="block text-xs text-gray-500 mb-1">本地模型名称</label>
            <select
              value={config.local_model}
              onChange={e => onConfigChange({ ...config, local_model: e.target.value })}
              className="w-full border rounded-md px-3 py-2 text-sm bg-white"
            >
              {shouldShowCurrentLocalModelOption && (
                <option value={config.local_model}>当前：{config.local_model}</option>
              )}
              {LOCAL_MODEL_OPTIONS.map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
              {detectedLocalModelOptions.length > 0 && (
                <optgroup label="已安装模型">
                  {detectedLocalModelOptions.map(model => (
                    <option key={model.name} value={model.name}>
                      {model.name}
                    </option>
                  ))}
                </optgroup>
              )}
            </select>
            <input
              type="text"
              value={config.local_model}
              onChange={e => onConfigChange({ ...config, local_model: e.target.value })}
              className="mt-2 w-full border rounded-md px-3 py-2 text-sm"
              placeholder="例如 qwen3:8b、llama3.2:latest"
            />
          </div>
          <div className="text-sm">
            {ollamaServiceOnline === null ? (
              <span className="text-gray-400">Ollama 状态读取中...</span>
            ) : ollamaServiceOnline ? (
              <span className="text-emerald-600">
                ● Ollama 在线{resolvedLocalModel ? ` · ${resolvedLocalModel}` : ""}
              </span>
            ) : (
              <span className="text-red-600">● Ollama 离线</span>
            )}
          </div>
        </div>
        {diagnostics && diagnostics.ollama_models && diagnostics.ollama_models.length > 0 && (
          <div className="rounded-lg border border-emerald-200 bg-emerald-50/40 p-3">
            <div className="text-xs font-medium text-emerald-800 mb-2">
              检测到以下 Ollama 模型：
            </div>
            <div className="flex flex-wrap gap-2">
              {diagnostics.ollama_models.map(m => (
                <button
                  key={m.name}
                  onClick={() => onConfigChange({ ...config, local_model: m.name })}
                  className={classNames(
                    "rounded-full px-2.5 py-1 text-xs border transition",
                    config.local_model === m.name
                      ? "bg-emerald-600 text-white border-emerald-600"
                      : "bg-white text-gray-700 border-gray-200 hover:bg-gray-50"
                  )}
                  title={`${m.size_mb} MB`}
                >
                  {m.name}
                </button>
              ))}
            </div>
          </div>
        )}
        {ollamaServiceOnline === false && (
          <div className="rounded-lg bg-amber-50 border border-amber-200 p-3 text-xs text-amber-800 space-y-1">
            <div className="font-medium">Ollama 未检测到，可能的原因：</div>
            <ul className="list-disc pl-4 space-y-0.5">
              <li>Ollama 尚未安装：访问 ollama.com 下载安装</li>
              <li>Ollama 未启动：在终端运行 ollama serve</li>
              <li>
                使用了非默认端口：当前默认检测 127.0.0.1/localhost:11434，也可通过
                OPENLIFE_OLLAMA_BASE_URL 或 OLLAMA_HOST 指定
              </li>
            </ul>
          </div>
        )}
        {ollamaServiceOnline === true && diagnostics && !diagnostics.ollama_online && (
          <div className="rounded-lg bg-blue-50 border border-blue-200 p-3 text-xs text-blue-800 space-y-1">
            <div className="font-medium">Ollama 已启动，但当前模型未匹配：{selectedLocalModel}</div>
            <ul className="list-disc pl-4 space-y-0.5">
              <li>如果已下载 llama3.1，请选择 Llama 3.1，或点击上方检测到的模型。</li>
              <li>如果想继续使用当前名称，请先运行 ollama pull {selectedLocalModel}。</li>
            </ul>
          </div>
        )}
        {ollamaServiceOnline === true && diagnostics && !diagnostics.ollama_online && (
          <div className="rounded-lg bg-blue-50 border border-blue-200 p-3 text-xs text-blue-800 space-y-1">
            <div className="font-medium">Ollama 已启动，但当前模型未匹配：{selectedLocalModel}</div>
            <ul className="list-disc pl-4 space-y-0.5">
              <li>如果已下载 llama3.1，请选择 Llama 3.1，或点击上方检测到的模型。</li>
              <li>如果想继续使用当前名称，请先运行 ollama pull {selectedLocalModel}。</li>
            </ul>
          </div>
        )}
      </section>
    </>
  );
}

function providerValidationView(diagnostics: SystemDiagnostics | null): {
  label: string;
  detail?: string;
  tone: "ready" | "warning" | "error" | "neutral";
} {
  if (!diagnostics) {
    return {
      label: "Provider 状态读取中",
      detail: "正在读取配置和最近一次连接验证记录。",
      tone: "neutral",
    };
  }
  if (!diagnostics.cloud_api_configured) {
    return {
      label: "Provider 未配置",
      detail: "需要 provider、base URL、chat model 和 API key 后才能测试连接。",
      tone: "neutral",
    };
  }
  const status: CloudApiValidationStatus = diagnostics.cloud_api_validation_status ?? "unvalidated";
  switch (status) {
    case "validated":
      return {
        label: "Validated",
        detail: diagnostics.cloud_api_validated_at
          ? `Provider 已验证；最近验证：${diagnostics.cloud_api_validated_at}`
          : "Provider 已验证；最近一次真实连接验证成功。",
        tone: "ready",
      };
    case "failed":
      return {
        label: "Failed validation",
        detail: diagnostics.cloud_api_last_error
          ? `安全错误标签：${diagnostics.cloud_api_last_error}`
          : "最近一次真实连接验证失败。",
        tone: "error",
      };
    case "stale":
      return {
        label: "Validation stale",
        detail:
          "Provider 验证已失效：provider、base URL、model、credential generation、network policy 或 24h TTL 已变化。",
        tone: "warning",
      };
    case "remote_unknown":
      return {
        label: "Remote state unknown",
        detail: "本地已停止等待，但无法确认远端是否完成；不会把它显示为成功或远端已取消。",
        tone: "warning",
      };
    case "runtime_generation_incoherent":
      return {
        label: "Runtime generation incoherent",
        detail: "Provider 配置和执行适配器不属于同一运行代，系统已在外发前失败关闭。",
        tone: "error",
      };
    case "validation_record_corrupt":
      return {
        label: "Validation record corrupt",
        detail: "最近验证记录无法通过结构与语义校验；需要重新执行真实连接验证。",
        tone: "error",
      };
    case "validation_record_io_error":
      return {
        label: "Validation record unreadable",
        detail: "无法读取最近验证记录；当前状态保持 unknown，不能当作 cloud-ready。",
        tone: "error",
      };
    case "unknown":
      return {
        label: "Provider state unknown",
        detail: "现有证据不足以确认 Provider 可用；当前不会显示为已验证。",
        tone: "neutral",
      };
    case "scripted_provider_probe":
    case "scripted_dogfood":
      return {
        label: "Local test proof only",
        detail:
          "Scripted proof 只证明本地测试链，不是 external cloud ready；实际云端传输状态仍为 unknown。",
        tone: "warning",
      };
    case "unconfigured":
      return {
        label: "Provider 未配置",
        detail: "当前验证状态与配置事实均显示 Provider 未配置。",
        tone: "neutral",
      };
    case "unvalidated":
      return {
        label: "Configured, not validated",
        detail:
          "Provider 已配置，尚未验证；填写 key 只代表 configured，点击“测试连接”成功后才会显示 validated。",
        tone: "warning",
      };
  }
}
