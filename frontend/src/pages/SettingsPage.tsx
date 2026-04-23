import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  getConfig,
  saveConfig,
  type AppConfig,
  generateEvolutionReport,
  runMemoryTierMaintenance,
  exportAllData,
  importAllData,
  testLlmConnection,
  checkOllamaStatus,
  getRouterStatus,
  getSystemDiagnostics,
  getHotCache,
  exportMcpAuditLogs,
  cleanupMcpAuditLogs,
  rotateMcpAuditKey,
  getPrivacyPolicy,
  setPrivacyPolicy,
  type ExportPayload,
  type HotMemoryCache,
  type PrivacyPolicy,
  type RouterStatus,
  type SystemDiagnostics,
} from "../tauri";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import LoadingSpinner from "../components/LoadingSpinner";

function defaultConfig(): AppConfig {
  return {
    llm: {
      provider: "deepseek",
      openai_base: "https://api.deepseek.com",
      openai_key: "",
      embedding_model: "text-embedding-3-small",
      chat_model: "deepseek-chat",
      embedding_enabled: false,
    },
    prefer_local_model: false,
    local_model: "llama2",
  };
}

const PROVIDER_PRESETS: Record<string, { label: string; base: string; model: string; embed: boolean; test_url: string }> = {
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
  dashscope: {
    label: "通义千问",
    base: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    model: "qwen-plus",
    embed: false,
    test_url: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
  },
  zhipu: {
    label: "智谱 GLM",
    base: "https://open.bigmodel.cn/api/paas/v4",
    model: "glm-4-flash",
    embed: false,
    test_url: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
  },
  custom: {
    label: "自定义",
    base: "",
    model: "",
    embed: false,
    test_url: "",
  },
};

function normalizeConfig(config?: Partial<AppConfig> | null): AppConfig {
  const fallback = defaultConfig();
  return {
    ...fallback,
    ...config,
    llm: {
      ...fallback.llm,
      ...(config?.llm ?? {}),
      provider: config?.llm?.provider ?? fallback.llm.provider,
      embedding_enabled: config?.llm?.embedding_enabled ?? fallback.llm.embedding_enabled,
    },
  };
}

const LOCAL_MODEL_OPTIONS = [
  { value: "llama2", label: "llama2" },
  { value: "llama3", label: "llama3" },
  { value: "llama3.1", label: "llama3.1" },
  { value: "llama3.2", label: "llama3.2" },
  { value: "qwen2.5", label: "qwen2.5" },
  { value: "mistral", label: "mistral" },
  { value: "gemma2", label: "gemma2" },
  { value: "nomic-embed-text", label: "nomic-embed-text (仅嵌入)" },
];

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

export default function SettingsPage() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [evolutionLoading, setEvolutionLoading] = useState(false);
  const [evolutionResult, setEvolutionResult] = useState<string | null>(null);
  const [tierLoading, setTierLoading] = useState(false);
  const [tierResult, setTierResult] = useState<string | null>(null);

  const [apiTestLoading, setApiTestLoading] = useState(false);
  const [apiTestResult, setApiTestResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [ollamaOnline, setOllamaOnline] = useState<boolean | null>(null);
  const [exportLoading, setExportLoading] = useState(false);
  const [importLoading, setImportLoading] = useState(false);
  const [routerStatus, setRouterStatus] = useState<RouterStatus | null>(null);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [hotCache, setHotCache] = useState<HotMemoryCache | null>(null);
  const [privacyPolicy, setPrivacyPolicyState] = useState<PrivacyPolicy | null>(null);
  const [securityLoading, setSecurityLoading] = useState(false);
  const [securityMessage, setSecurityMessage] = useState<string | null>(null);

  useEffect(() => {
    getConfig()
      .then((cfg) => {
        setConfig(normalizeConfig(cfg));
        setLoading(false);
      })
      .catch((e) => {
        setMessage("加载配置失败: " + readableError(e));
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    if (!config.prefer_local_model) return;
    checkOllamaStatus().then(setOllamaOnline).catch(() => setOllamaOnline(false));
  }, [config.local_model, config.prefer_local_model]);

  useEffect(() => {
    refreshAllDiagnostics();
  }, []);

  const refreshAllDiagnostics = async () => {
    const [router, diag, cache, policy] = await Promise.all([
      getRouterStatus().catch(() => null),
      getSystemDiagnostics().catch(() => null),
      getHotCache().catch(() => null),
      getPrivacyPolicy().catch(() => null),
    ]);
    setRouterStatus(router);
    setDiagnostics(diag);
    setHotCache(cache);
    setPrivacyPolicyState(policy);
  };

  const updateLlm = (field: keyof AppConfig["llm"], value: string) => {
    setConfig((prev) => ({
      ...prev,
      llm: { ...prev.llm, [field]: value },
    }));
    setApiTestResult(null);
  };

  const updateProvider = (provider: NonNullable<AppConfig["llm"]["provider"]>) => {
    const preset = PROVIDER_PRESETS[provider];
    setConfig((prev) => ({
      ...prev,
      llm: {
        ...prev.llm,
        provider,
        openai_base: provider === "custom" ? prev.llm.openai_base : preset.base,
        chat_model: provider === "custom" ? prev.llm.chat_model : preset.model,
        embedding_enabled: preset.embed,
      },
      prefer_local_model: provider === "deepseek" ? false : prev.prefer_local_model,
    }));
    setApiTestResult(null);
  };

  const handleSave = async () => {
    setSaving(true);
    setMessage(null);
    try {
      await saveConfig(config);
      setMessage("保存成功");
      await refreshAllDiagnostics();
    } catch (e: any) {
      setMessage("保存失败: " + readableError(e));
    } finally {
      setSaving(false);
    }
  };

  const handleTestApiKey = async () => {
    setApiTestLoading(true);
    setApiTestResult(null);
    try {
      const result = await testLlmConnection(config);
      setApiTestResult({ ok: result.ok, text: `${result.provider}: ${result.message}` });
      await refreshAllDiagnostics();
    } catch (e: any) {
      setApiTestResult({ ok: false, text: readableError(e) });
    } finally {
      setApiTestLoading(false);
    }
  };

  const handleExport = async () => {
    setExportLoading(true);
    setMessage(null);
    try {
      const data = await exportAllData();
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: "openlife-export.json",
      });
      if (!path) {
        setExportLoading(false);
        return;
      }
      await writeTextFile(path, JSON.stringify(data, null, 2));
      setMessage(`导出成功（格式版本 ${data.version}${data.app_version ? "，应用版本 " + data.app_version : ""}）`);
    } catch (e: any) {
      setMessage("导出失败: " + readableError(e));
    } finally {
      setExportLoading(false);
    }
  };

  const handleImport = async () => {
    setImportLoading(true);
    setMessage(null);
    try {
      const selected = await open({
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path) {
        setImportLoading(false);
        return;
      }
      const text = await readTextFile(path);
      const payload: ExportPayload = JSON.parse(text);
      await importAllData(payload);
      setMessage("导入成功，请刷新页面以查看最新数据");
      await refreshAllDiagnostics();
    } catch (e: any) {
      setMessage("导入失败: " + readableError(e));
    } finally {
      setImportLoading(false);
    }
  };

  const refreshSecurityState = async () => {
    await refreshAllDiagnostics();
  };

  const handleExportAudit = async () => {
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      const audit = await exportMcpAuditLogs(30);
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: "openlife-mcp-audit.json",
      });
      if (!path) return;
      await writeTextFile(path, JSON.stringify(audit, null, 2));
      setSecurityMessage(`已导出近 ${audit.days} 天 MCP 审计日志 ${audit.entry_count} 条`);
    } catch (e: any) {
      setSecurityMessage("审计日志导出失败: " + readableError(e));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleCleanupAudit = async () => {
    if (!confirm("确定清理 90 天前的 MCP 审计日志吗？此操作不可撤销。")) return;
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      const removed = await cleanupMcpAuditLogs(90);
      setSecurityMessage(`已清理 ${removed} 条旧 MCP 审计日志`);
      await refreshSecurityState();
    } catch (e: any) {
      setSecurityMessage("审计日志清理失败: " + readableError(e));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleRotateAuditKey = async () => {
    if (!confirm("确定轮换 MCP 审计密钥吗？系统会保留本地 keyring，以便历史日志继续可读。")) return;
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      await rotateMcpAuditKey();
      setSecurityMessage("审计密钥已轮换，历史日志会继续按原 key epoch 解密");
    } catch (e: any) {
      setSecurityMessage("审计密钥轮换失败: " + readableError(e));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleSavePrivacyPolicy = async () => {
    if (!privacyPolicy) return;
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      await setPrivacyPolicy(privacyPolicy);
      setSecurityMessage("隐私策略已保存，重启后仍会生效");
    } catch (e: any) {
      setSecurityMessage("隐私策略保存失败: " + readableError(e));
    } finally {
      setSecurityLoading(false);
    }
  };

  // ---- Data file health ----
  const df = diagnostics?.data_files;
  const dataFileItems = df
    ? [
        { label: "消息数据库", exists: df.messages_db_exists, size: df.messages_db_size_mb },
        { label: "向量数据库", exists: df.vectors_db_exists, size: df.vectors_db_size_mb },
        { label: "审计数据库", exists: df.mcp_audit_db_exists, size: df.mcp_audit_db_size_mb },
        { label: "配置文件", exists: df.config_yaml_exists, size: undefined },
        { label: "人生模型", exists: df.life_model_yaml_exists, size: undefined },
      ]
    : [];

  const allDataFilesOk = df ? df.messages_db_exists && df.vectors_db_exists && df.config_yaml_exists : null;

  // ---- Trial checklist ----
  const trialChecks = [
    {
      label: "云端模型",
      ok: diagnostics?.cloud_api_configured ?? false,
      detail: diagnostics?.cloud_api_configured
        ? `${diagnostics?.cloud_provider ?? "云端"} 已配置${diagnostics?.config_source === "env_var" ? "（来自环境变量）" : ""}`
        : "还没有可用的云端 API Key",
      action: "配置模型",
      href: "#llm-settings",
    },
    {
      label: "本地模型",
      ok: diagnostics?.ollama_online ?? false,
      detail: diagnostics?.ollama_online
        ? `${diagnostics?.resolved_local_model || diagnostics?.local_model} 在线`
        : "Ollama 离线，若走本地模型需要先启动",
      action: "查看本地配置",
      href: "#local-model-settings",
    },
    {
      label: "人生模型",
      ok: Boolean(diagnostics?.life_model_ready && !diagnostics?.model_empty),
      detail: diagnostics?.model_empty ? "尚未完成初始构建" : diagnostics?.life_model_ready ? "可读取" : "读取失败",
      action: diagnostics?.model_empty ? "去构建" : "查看模型",
      href: diagnostics?.model_empty ? "#/builder" : "#/",
    },
    {
      label: "数据文件",
      ok: allDataFilesOk ?? false,
      detail: allDataFilesOk === true ? "数据目录健康" : allDataFilesOk === false ? "部分数据文件缺失" : "等待诊断",
      action: "查看数据",
      href: "#data-health",
    },
    {
      label: "对话验证",
      ok: (diagnostics?.chat_session_count ?? 0) > 0,
      detail: (diagnostics?.chat_session_count ?? 0) > 0 ? `${diagnostics?.chat_session_count} 个会话` : "还没有完成过一轮对话",
      action: "去对话",
      href: "#/chat",
    },
  ];

  const provider = config.llm.provider ?? "deepseek";
  const preset = PROVIDER_PRESETS[provider];

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <LoadingSpinner text="正在加载设置..." />
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-2xl mx-auto bg-white rounded-xl shadow p-6 space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-gray-800">试用控制台</h2>
          <button
            onClick={refreshAllDiagnostics}
            className="rounded-md border border-gray-200 px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-50"
          >
            刷新状态
          </button>
        </div>

        {message && (
          <div
            className={classNames(
              "text-sm px-3 py-2 rounded",
              message.includes("失败") ? "bg-red-50 text-red-700" : "bg-green-50 text-green-700"
            )}
          >
            {message}
          </div>
        )}

        {/* Trial Checklist */}
        <section className="space-y-4">
          <div
            className={classNames(
              "rounded-2xl border p-4",
              diagnostics?.chat_ready
                ? "border-emerald-200 bg-emerald-50/60"
                : "border-amber-200 bg-amber-50/60"
            )}
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-stone-900">试用路径 Checklist</div>
                <div className="mt-1 text-xs text-stone-500">
                  {diagnostics?.chat_ready
                    ? "核心链路已就绪，可以开始试用 Chat / Builder / Calibration。"
                    : "按这些项逐个修复，桌面端试用会稳定很多。"}
                </div>
              </div>
              <span
                className={classNames(
                  "rounded-full px-2 py-1 text-xs font-medium shrink-0",
                  diagnostics?.chat_ready
                    ? "bg-emerald-100 text-emerald-700"
                    : "bg-amber-100 text-amber-700"
                )}
              >
                {diagnostics?.chat_ready ? "可开始试用" : "还有阻塞"}
              </span>
            </div>
            <div className="mt-4 space-y-2">
              {trialChecks.map((item) => (
                <div
                  key={item.label}
                  className="flex items-center gap-3 rounded-xl border border-white bg-white/75 px-3 py-2"
                >
                  <div
                    className={classNames(
                      "flex h-6 w-6 items-center justify-center rounded-full text-xs font-bold",
                      item.ok ? "bg-emerald-100 text-emerald-700" : "bg-amber-100 text-amber-700"
                    )}
                  >
                    {item.ok ? "✓" : "!"}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-stone-800">{item.label}</div>
                    <div className="truncate text-xs text-stone-500">{item.detail}</div>
                  </div>
                  <a
                    href={item.href}
                    className="shrink-0 rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
                  >
                    {item.action}
                  </a>
                </div>
              ))}
            </div>
            {diagnostics && diagnostics.readiness_issues.length > 0 && (
              <div className="mt-3 rounded-lg bg-white/70 p-3">
                <div className="text-xs font-medium text-amber-800">建议先处理：</div>
                <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-700">
                  {diagnostics.readiness_issues.map((issue) => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </section>

        {/* Quick Actions */}
        {diagnostics && !diagnostics.chat_ready && (
          <section className="space-y-3">
            <div className="text-sm font-medium text-gray-700">快速修复</div>
            <div className="flex flex-wrap gap-2">
              {!diagnostics.cloud_api_configured && (
                <a
                  href="#llm-settings"
                  className="rounded-md bg-indigo-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-700"
                >
                  1. 配置 API Key
                </a>
              )}
              {diagnostics.model_empty && (
                <Link
                  to="/builder"
                  className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-700"
                >
                  2. 构建人生模型
                </Link>
              )}
              {!diagnostics.model_empty && diagnostics.chat_session_count === 0 && (
                <Link
                  to="/chat"
                  className="rounded-md bg-slate-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-800"
                >
                  3. 开始一次对话
                </Link>
              )}
            </div>
          </section>
        )}

        {/* Data File Health */}
        <section id="data-health" className="space-y-4">
          <h3 className="text-sm font-medium text-gray-700">数据文件健康</h3>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
            {dataFileItems.map((item) => (
              <div
                key={item.label}
                className={classNames(
                  "rounded-lg border p-3",
                  item.exists ? "border-emerald-200 bg-emerald-50/40" : "border-amber-200 bg-amber-50/40"
                )}
              >
                <div className="flex items-center gap-1.5">
                  <span
                    className={classNames(
                      "h-2 w-2 rounded-full",
                      item.exists ? "bg-emerald-500" : "bg-amber-400"
                    )}
                  />
                  <span className="text-xs text-gray-500">{item.label}</span>
                </div>
                <div className="mt-1 text-sm font-medium text-gray-800">
                  {item.exists ? "就绪" : "缺失"}
                  {item.size !== undefined && item.size > 0 && (
                    <span className="ml-1 text-xs text-gray-500">({item.size} MB)</span>
                  )}
                </div>
              </div>
            ))}
          </div>
          <p className="text-xs text-gray-500">
            数据目录：{diagnostics?.active_data_dir ?? diagnostics?.data_dir ?? "-"}
          </p>
        </section>

        {/* LLM Settings */}
        <section id="llm-settings" className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">LLM 配置</h3>
          <div className="grid gap-4">
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
                onChange={(e) => updateLlm("openai_base", e.target.value)}
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
                  onChange={(e) => updateLlm("openai_key", e.target.value)}
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
                  onChange={(e) => updateLlm("chat_model", e.target.value)}
                  className="w-full border rounded-md px-3 py-2 text-sm"
                  placeholder={preset.model || "model-name"}
                />
              </div>
              <div>
                <label className="block text-xs text-gray-500 mb-1">Embedding Model</label>
                <input
                  type="text"
                  value={config.llm.embedding_model}
                  onChange={(e) => updateLlm("embedding_model", e.target.value)}
                  className="w-full border rounded-md px-3 py-2 text-sm"
                  disabled={config.llm.embedding_enabled === false}
                  placeholder="text-embedding-3-small"
                />
                <label className="mt-2 flex items-center gap-2 text-xs text-gray-600">
                  <input
                    type="checkbox"
                    checked={config.llm.embedding_enabled !== false}
                    onChange={(e) =>
                      setConfig((prev) => ({
                        ...prev,
                        llm: { ...prev.llm, embedding_enabled: e.target.checked },
                      }))
                    }
                  />
                  启用远端 embedding（DeepSeek 默认关闭）
                </label>
              </div>
            </div>
          </div>
        </section>

        {/* Local Model */}
        <section id="local-model-settings" className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">本地模型（Ollama）</h3>
          <div className="flex items-center gap-3">
            <input
              id="prefer_local"
              type="checkbox"
              checked={config.prefer_local_model}
              onChange={(e) =>
                setConfig((prev) => ({ ...prev, prefer_local_model: e.target.checked }))
              }
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
                onChange={(e) => setConfig((prev) => ({ ...prev, local_model: e.target.value }))}
                className="w-full border rounded-md px-3 py-2 text-sm bg-white"
              >
                {LOCAL_MODEL_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="text-sm">
              {ollamaOnline === null ? (
                <span className="text-gray-400">正在检测 Ollama...</span>
              ) : ollamaOnline ? (
                <span className="text-emerald-600">● Ollama 在线</span>
              ) : (
                <span className="text-red-600">● Ollama 离线</span>
              )}
            </div>
          </div>
          {diagnostics && diagnostics.ollama_models && diagnostics.ollama_models.length > 0 && (
            <div className="rounded-lg border border-emerald-200 bg-emerald-50/40 p-3">
              <div className="text-xs font-medium text-emerald-800 mb-2">检测到以下 Ollama 模型：</div>
              <div className="flex flex-wrap gap-2">
                {diagnostics.ollama_models.map((m) => (
                  <button
                    key={m.name}
                    onClick={() => setConfig((prev) => ({ ...prev, local_model: m.name }))}
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
          {ollamaOnline === false && (
            <div className="rounded-lg bg-amber-50 border border-amber-200 p-3 text-xs text-amber-800 space-y-1">
              <div className="font-medium">Ollama 未检测到，可能的原因：</div>
              <ul className="list-disc pl-4 space-y-0.5">
                <li>Ollama 尚未安装：访问 ollama.com 下载安装</li>
                <li>Ollama 未启动：在终端运行 ollama serve</li>
                <li>使用了非默认端口：当前只检测 localhost:11434</li>
              </ul>
            </div>
          )}
        </section>

        {/* Router */}
        <section className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">Layer 1 路由状态</h3>
          <div className="rounded-lg border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-700 space-y-2">
            <div className="flex items-center justify-between">
              <span>当前后端</span>
              <span className="font-medium uppercase">{routerStatus?.active_backend ?? "unknown"}</span>
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
              <span>{routerStatus ? `${Math.round(routerStatus.latency_threshold_us / 1000)}ms` : "-"}</span>
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
            {diagnostics && diagnostics.beta_readiness_issues && diagnostics.beta_readiness_issues.length > 0 && (
              <div className="mt-3 rounded-lg bg-white/70 p-3">
                <div className="text-xs font-medium">Beta 前建议处理：</div>
                <ul className="mt-1 list-disc space-y-1 pl-4 text-xs">
                  {diagnostics.beta_readiness_issues.map((issue) => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </section>

        {/* Security */}
        <section className="space-y-4 border-t pt-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-sm font-medium text-gray-700">安全治理与长期记忆</h3>
              <p className="mt-1 text-xs text-gray-500">
                查看热记忆摘要、MCP 审计和隐私策略，确保长期数据可控、可导出、可清理。
              </p>
            </div>
            <button
              onClick={refreshSecurityState}
              className="rounded-md border border-gray-200 px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-50"
            >
              刷新
            </button>
          </div>

          {securityMessage && (
            <div
              className={classNames(
                "rounded px-3 py-2 text-sm",
                securityMessage.includes("失败") ? "bg-red-50 text-red-700" : "bg-blue-50 text-blue-700"
              )}
            >
              {securityMessage}
            </div>
          )}

          <div className="grid gap-3 md:grid-cols-2">
            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
              <div className="text-sm font-semibold text-slate-800">热记忆摘要</div>
              <div className="mt-2 space-y-2 text-xs text-slate-600">
                <div>{hotCache?.identity_summary || "暂无热记忆摘要"}</div>
                <div>核心价值观：{hotCache?.top_values?.join("、") || "-"}</div>
                <div>当前目标：{hotCache?.current_goals?.slice(0, 2).join("；") || "-"}</div>
                <div>最近刷新：{hotCache?.last_refreshed ? new Date(hotCache.last_refreshed).toLocaleString() : "-"}</div>
              </div>
            </div>

            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
              <div className="text-sm font-semibold text-slate-800">MCP 审计</div>
              <div className="mt-2 grid grid-cols-2 gap-2 text-xs text-slate-600">
                <div>近期审计：{diagnostics?.mcp_recent_audit_count ?? "-"}</div>
                <div>PII 命中：{diagnostics?.mcp_recent_pii_count ?? "-"}</div>
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  onClick={handleExportAudit}
                  disabled={securityLoading}
                  className="rounded-md bg-slate-800 px-3 py-1.5 text-xs font-medium text-white hover:bg-slate-900 disabled:opacity-50"
                >
                  导出审计
                </button>
                <button
                  onClick={handleCleanupAudit}
                  disabled={securityLoading}
                  className="rounded-md border border-slate-200 bg-white px-3 py-1.5 text-xs text-slate-700 hover:bg-slate-50 disabled:opacity-50"
                >
                  清理旧日志
                </button>
                <button
                  onClick={handleRotateAuditKey}
                  disabled={securityLoading}
                  className="rounded-md border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs text-amber-700 hover:bg-amber-100 disabled:opacity-50"
                >
                  轮换密钥
                </button>
              </div>
            </div>
          </div>

          <div className="rounded-xl border border-slate-200 bg-white p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-slate-800">隐私策略</div>
                <div className="mt-1 text-xs text-slate-500">保存后会写入本地 privacy_policy.yaml，重启后继续生效。</div>
              </div>
              <label className="flex items-center gap-2 text-xs text-slate-600">
                <input
                  type="checkbox"
                  checked={privacyPolicy?.enabled ?? true}
                  onChange={(e) =>
                    setPrivacyPolicyState((prev) => ({ ...(prev ?? { rules: [] }), enabled: e.target.checked }))
                  }
                />
                启用隐私处理
              </label>
            </div>
            <div className="mt-3 grid gap-2 md:grid-cols-3">
              {(privacyPolicy?.rules ?? []).map((rule, index) => (
                <div key={`${rule.ptype}-${index}`} className="rounded-lg border border-slate-100 bg-slate-50 px-3 py-2 text-xs">
                  <div className="font-medium text-slate-700">{rule.ptype}</div>
                  <div className="mt-1 flex items-center justify-between gap-2">
                    <label className="flex items-center gap-1 text-slate-600">
                      <input
                        type="checkbox"
                        checked={rule.enabled}
                        onChange={(e) =>
                          setPrivacyPolicyState((prev) => {
                            if (!prev) return prev;
                            const next = [...prev.rules];
                            next[index] = { ...next[index], enabled: e.target.checked };
                            return { ...prev, rules: next };
                          })
                        }
                      />
                      开启
                    </label>
                    <select
                      value={rule.action}
                      onChange={(e) =>
                        setPrivacyPolicyState((prev) => {
                          if (!prev) return prev;
                          const next = [...prev.rules];
                          next[index] = { ...next[index], action: e.target.value as "Mask" | "Block" | "Allow" };
                          return { ...prev, rules: next };
                        })
                      }
                      className="rounded border border-slate-200 bg-white px-2 py-1"
                    >
                      <option value="Mask">Mask</option>
                      <option value="Block">Block</option>
                      <option value="Allow">Allow</option>
                    </select>
                  </div>
                </div>
              ))}
            </div>
            <button
              onClick={handleSavePrivacyPolicy}
              disabled={securityLoading || !privacyPolicy}
              className="mt-3 rounded-md bg-indigo-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
            >
              保存隐私策略
            </button>
          </div>
        </section>

        {/* Data Migration */}
        <section className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">数字遗产 / 数据迁移</h3>
          <div className="flex flex-wrap gap-3">
            <button
              onClick={handleExport}
              disabled={exportLoading}
              className="px-3 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 disabled:opacity-50"
            >
              {exportLoading ? "导出中..." : "导出全部数据"}
            </button>
            <button
              onClick={handleImport}
              disabled={importLoading}
              className="px-3 py-2 bg-white border border-gray-300 text-gray-700 rounded-md text-sm font-medium hover:bg-gray-50 disabled:opacity-50"
            >
              {importLoading ? "导入中..." : "导入全部数据"}
            </button>
          </div>
          <p className="text-xs text-gray-500">
            导出将包含 LifeModel、聊天记录与向量记忆数据，格式为 JSON（带版本号与主版本校验）。导入会覆盖当前数据，跨主版本导入会被拒绝，请谨慎操作。
          </p>
        </section>

        {/* Maintenance */}
        <section className="space-y-4 border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700">系统维护</h3>
          <div className="flex flex-wrap gap-3">
            <button
              onClick={async () => {
                setEvolutionLoading(true);
                setEvolutionResult(null);
                try {
                  const res = await generateEvolutionReport();
                  setEvolutionResult(`已应用规则 ${res.applied_rules.length} 条\n${res.summary}`);
                } catch (e: any) {
                  setEvolutionResult("生成失败: " + readableError(e));
                } finally {
                  setEvolutionLoading(false);
                }
              }}
              disabled={evolutionLoading}
              className="px-3 py-2 bg-emerald-600 text-white rounded-md text-sm font-medium hover:bg-emerald-700 disabled:opacity-50"
            >
              {evolutionLoading ? "生成中..." : "生成进化报告"}
            </button>
            <button
              onClick={async () => {
                setTierLoading(true);
                setTierResult(null);
                try {
                  const res = await runMemoryTierMaintenance();
                  setTierResult(`记忆层级维护完成：晋升 ${res.promoted} 条，降级 ${res.demoted} 条`);
                } catch (e: any) {
                  setTierResult("维护失败: " + readableError(e));
                } finally {
                  setTierLoading(false);
                }
              }}
              disabled={tierLoading}
              className="px-3 py-2 bg-amber-600 text-white rounded-md text-sm font-medium hover:bg-amber-700 disabled:opacity-50"
            >
              {tierLoading ? "维护中..." : "运行记忆层级维护"}
            </button>
          </div>
          {evolutionResult && (
            <div className="text-sm whitespace-pre-line bg-emerald-50 text-emerald-800 rounded px-3 py-2">
              {evolutionResult}
            </div>
          )}
          {tierResult && (
            <div className="text-sm whitespace-pre-line bg-amber-50 text-amber-800 rounded px-3 py-2">
              {tierResult}
            </div>
          )}
        </section>

        <div className="flex justify-end">
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-2 bg-indigo-600 text-white rounded-md text-sm font-medium hover:bg-indigo-700 disabled:opacity-50"
          >
            {saving ? "保存中..." : "保存设置"}
          </button>
        </div>
      </div>
    </div>
  );
}

function readableError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as any).message === "string") return (e as any).message;
    if ("error" in e && typeof (e as any).error === "string") return (e as any).error;
  }
  return String(e);
}
