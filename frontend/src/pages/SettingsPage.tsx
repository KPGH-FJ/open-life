import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  getConfig,
  saveConfig,
  type AppConfig,
  generateEvolutionReport,
  runMemoryTierMaintenance,
  rebuildMemoryIndex,
  exportAllData,
  importAllData,
  testLlmConnection,
  checkOllamaStatus,
  getRouterStatus,
  getModelRouterStatus,
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
  type ModelRouterStatus,
  type SystemDiagnostics,
  listToolPermissions,
  revokeToolPermission,
  listPlugins,
  reloadPlugins,
  enablePlugin,
  disablePlugin,
  type ToolPermissionRecord,
  type PluginRecord,
} from "../tauri";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import LoadingSpinner from "../components/LoadingSpinner";
import { isSafeMode } from "../utils/safeMode";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";
import ProviderConfigSection from "./settings/ProviderConfigSection";
import PluginSection from "./settings/PluginSection";

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
    chat_proposal: {
      enabled: true,
      confidence_threshold: 0.6,
      min_message_length: 10,
      cooldown_seconds: 300,
    },
  };
}

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
  const [rebuildLoading, setRebuildLoading] = useState(false);
  const [rebuildResult, setRebuildResult] = useState<string | null>(null);
  const [exportLoading, setExportLoading] = useState(false);
  const [importLoading, setImportLoading] = useState(false);
  const [routerStatus, setRouterStatus] = useState<RouterStatus | null>(null);
  const [modelRouterStatus, setModelRouterStatus] = useState<ModelRouterStatus | null>(null);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [hotCache, setHotCache] = useState<HotMemoryCache | null>(null);
  const [privacyPolicy, setPrivacyPolicyState] = useState<PrivacyPolicy | null>(null);
  const [toolPermissions, setToolPermissions] = useState<ToolPermissionRecord[]>([]);
  const [plugins, setPlugins] = useState<PluginRecord[]>([]);
  const [securityLoading, setSecurityLoading] = useState(false);
  const [securityMessage, setSecurityMessage] = useState<string | null>(null);

  useEffect(() => {
    getConfig()
      .then(cfg => {
        setConfig(normalizeConfig(cfg));
        setLoading(false);
      })
      .catch(e => {
        setMessage("加载配置失败: " + readableError(e));
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    if (!config.prefer_local_model) return;
    checkOllamaStatus()
      .then(setOllamaOnline)
      .catch(() => setOllamaOnline(false));
  }, [config.local_model, config.prefer_local_model]);

  useEffect(() => {
    refreshAllDiagnostics();
  }, []);

  const refreshAllDiagnostics = async () => {
    const [router, modelRouter, diag, cache, policy, permissions, pluginRecords] = await Promise.all([
      getRouterStatus().catch(() => null),
      getModelRouterStatus().catch(() => null),
      getSystemDiagnostics().catch(() => null),
      getHotCache().catch(() => null),
      getPrivacyPolicy().catch(() => null),
      listToolPermissions().catch(() => []),
      listPlugins().catch(() => []),
    ]);
    setRouterStatus(router);
    setModelRouterStatus(modelRouter);
    setDiagnostics(diag);
    setHotCache(cache);
    setPrivacyPolicyState(policy);
    setToolPermissions(permissions);
    setPlugins(pluginRecords);
    return diag;
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
      setMessage(
        `导出成功（格式版本 ${data.version}${data.app_version ? "，应用版本 " + data.app_version : ""}）`
      );
    } catch (e: any) {
      setMessage("导出失败: " + readableError(e));
    } finally {
      setExportLoading(false);
    }
  };

  const handleImport = async () => {
    if (safeMode) {
      setMessage(buildSafeModeBlockedMessage("导入覆盖", diagnostics));
      return;
    }
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
      setMessage(buildRuntimeActionError("导入数据", e, "data"));
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
    if (safeMode) {
      setSecurityMessage(buildSafeModeBlockedMessage("审计日志清理", diagnostics));
      return;
    }
    if (!confirm("确定清理 90 天前的 MCP 审计日志吗？此操作不可撤销。")) return;
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      const removed = await cleanupMcpAuditLogs(90);
      setSecurityMessage(`已清理 ${removed} 条旧 MCP 审计日志`);
      await refreshSecurityState();
    } catch (e: any) {
      setSecurityMessage(buildRuntimeActionError("清理审计日志", e, "data"));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleRotateAuditKey = async () => {
    if (safeMode) {
      setSecurityMessage(buildSafeModeBlockedMessage("审计密钥轮换", diagnostics));
      return;
    }
    if (!confirm("确定轮换 MCP 审计密钥吗？系统会保留本地 keyring，以便历史日志继续可读。")) return;
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      await rotateMcpAuditKey();
      setSecurityMessage("审计密钥已轮换，历史日志会继续按原 key epoch 解密");
    } catch (e: any) {
      setSecurityMessage(buildRuntimeActionError("轮换审计密钥", e, "data"));
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

  const allDataFilesOk = df
    ? df.messages_db_exists && df.vectors_db_exists && df.config_yaml_exists
    : null;

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
      detail: diagnostics?.model_empty
        ? (diagnostics?.pending_builder_review_sessions ?? 0) > 0
          ? `有 ${diagnostics?.pending_builder_review_sessions} 个待确认的 Builder Review`
          : (diagnostics?.unfinished_builder_sessions ?? 0) > 0
            ? `有 ${diagnostics?.unfinished_builder_sessions} 个待继续的 Builder 会话`
            : "尚未完成初始构建"
        : diagnostics?.life_model_ready
          ? "可读取"
          : "读取失败",
      action: diagnostics?.model_empty
        ? (diagnostics?.pending_builder_review_sessions ?? 0) > 0
          ? "去审阅"
          : (diagnostics?.unfinished_builder_sessions ?? 0) > 0
            ? "继续 Builder"
            : "去构建"
        : "查看模型",
      href: diagnostics?.model_empty ? "#/builder" : "#/",
    },
    {
      label: "数据文件",
      ok: allDataFilesOk ?? false,
      detail:
        allDataFilesOk === true
          ? "数据目录健康"
          : allDataFilesOk === false
            ? "部分数据文件缺失"
            : "等待诊断",
      action: "查看数据",
      href: "#data-health",
    },
    {
      label: "对话验证",
      ok: (diagnostics?.chat_session_count ?? 0) > 0,
      detail:
        (diagnostics?.chat_session_count ?? 0) > 0
          ? `${diagnostics?.chat_session_count} 个会话`
          : "还没有完成过一轮对话",
      action: "去对话",
      href: "#/chat",
    },
  ];

  const safeMode = isSafeMode(diagnostics);
  const betaFlow = [
    {
      title: "1. 完成设置与诊断",
      done: Boolean(
        diagnostics?.chat_ready || diagnostics?.cloud_api_configured || diagnostics?.ollama_online
      ),
      detail: diagnostics?.chat_ready
        ? "模型后端已经可用，基础运行环境通过。"
        : "先把本地或云端模型跑通，避免进入聊天页后才发现不能用。",
      to: "#llm-settings",
      action: "检查模型配置",
    },
    {
      title: "2. 完成人生模型构建",
      done: Boolean(diagnostics && !diagnostics.model_empty && diagnostics.life_model_ready),
      detail: diagnostics?.model_empty
        ? (diagnostics?.pending_builder_review_sessions ?? 0) > 0
          ? `Builder 里还有 ${diagnostics?.pending_builder_review_sessions} 个待确认 Review。先把这些建议应用掉，比重新开始更合适。`
          : (diagnostics?.unfinished_builder_sessions ?? 0) > 0
            ? `Builder 里还有 ${diagnostics?.unfinished_builder_sessions} 个待继续或待确认的会话。先把 Review 应用掉，比重新开始更合适。`
            : "Builder 还没形成最小模型，当前很多建议仍会偏通用。"
        : "人生模型已可读取，个性化能力开始成立。",
      to: "#/builder",
      action: diagnostics?.model_empty
        ? (diagnostics?.pending_builder_review_sessions ?? 0) > 0
          ? "去审阅"
          : (diagnostics?.unfinished_builder_sessions ?? 0) > 0
            ? "继续 Builder"
            : "去构建"
        : "去构建",
    },
    {
      title: "3. 跑通第一次对话",
      done: Boolean((diagnostics?.chat_session_count ?? 0) > 0),
      detail:
        (diagnostics?.chat_session_count ?? 0) > 0
          ? `已经完成 ${diagnostics?.chat_session_count ?? 0} 次对话验证。`
          : "至少完成一轮真实对话，才能确认主链路不是只在设置页看起来正常。",
      to: "#/chat",
      action: "去对话",
    },
    {
      title: "4. 查看校准或版本回滚",
      done: Boolean((diagnostics?.snapshot_count ?? 0) > 0),
      detail:
        (diagnostics?.snapshot_count ?? 0) > 0
          ? `已经有 ${diagnostics?.snapshot_count} 个快照，版本安全网已建立。`
          : "至少确认一次快照/回滚路径，Beta 试用才算具备可恢复能力。",
      to: "#/versions",
      action: "看版本控制",
    },
  ];
  const recoveryIssues = [
    ...(diagnostics?.startup_warnings?.map(warning => ({
      title: "启动降级",
      detail: warning,
      tone: "error" as const,
    })) ?? []),
    ...((diagnostics?.vector_corrupt_embedding_count ?? 0) > 0
      ? [
          {
            title: "向量索引损坏",
            detail: `检测到 ${diagnostics?.vector_corrupt_embedding_count} 条向量 embedding 记录损坏，长期记忆检索可能不完整。`,
            tone: "warning" as const,
          },
        ]
      : []),
    ...((diagnostics?.pending_builder_review_sessions ?? 0) > 0
      ? [
          {
            title: "Builder 待确认 Review",
            detail: `当前还有 ${diagnostics?.pending_builder_review_sessions} 个待确认 Review。建议先回到 Builder 审阅并应用，再验证对话与仪表盘。`,
            tone: "warning" as const,
          },
        ]
      : []),
    ...((diagnostics?.chat_session_count ?? 0) > 0 && (diagnostics?.memory_chunk_count ?? 0) === 0
      ? [
          {
            title: "聊天已有记录，但语义记忆为空",
            detail:
              "说明主聊天链路跑过，但长期记忆还没真正建立。建议先重建向量索引，再验证校准与长期记忆。",
            tone: "warning" as const,
          },
        ]
      : []),
    ...(diagnostics?.database_status === "degraded"
      ? [
          {
            title: "数据库模式已降级",
            detail: "当前应用没有运行在完全健康的数据模式下，继续试用前建议先导出备份。",
            tone: "warning" as const,
          },
        ]
      : []),
  ];

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
              {trialChecks.map(item => (
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
                  {diagnostics.readiness_issues.map(issue => (
                    <li key={issue}>{issue}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </section>

        <section className="space-y-4">
          <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-stone-900">试用闭环定义</div>
                <div className="mt-1 text-xs text-stone-500">
                  下面这 4 步都跑通，才算真正完成了一次 OpenLife Beta
                  试用，而不是只停留在配置或单页体验。
                </div>
              </div>
              <span
                className={classNames(
                  "rounded-full px-2 py-1 text-xs font-medium",
                  diagnostics?.beta_ready
                    ? "bg-emerald-100 text-emerald-700"
                    : "bg-blue-100 text-blue-700"
                )}
              >
                {diagnostics?.beta_ready ? "已闭环" : "闭环中"}
              </span>
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-2">
              {betaFlow.map(step => (
                <div
                  key={step.title}
                  className="rounded-xl border border-white bg-white/80 px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-sm font-medium text-stone-900">{step.title}</div>
                      <div className="mt-1 text-xs leading-5 text-stone-600">{step.detail}</div>
                    </div>
                    <span
                      className={classNames(
                        "shrink-0 rounded-full px-2 py-1 text-[11px] font-medium",
                        step.done
                          ? "bg-emerald-100 text-emerald-700"
                          : "bg-amber-100 text-amber-700"
                      )}
                    >
                      {step.done ? "完成" : "待完成"}
                    </span>
                  </div>
                  <div className="mt-3">
                    <a
                      href={step.to}
                      className="inline-flex rounded-full border border-stone-200 bg-white px-3 py-1 text-[11px] font-medium text-stone-700 hover:bg-stone-50"
                    >
                      {step.action}
                    </a>
                  </div>
                </div>
              ))}
            </div>
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

        {safeMode && (
          <section className="space-y-4 border border-amber-200 bg-amber-50/60 rounded-2xl p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h3 className="text-sm font-semibold text-amber-950">恢复控制台</h3>
                <p className="mt-1 text-xs text-amber-800">
                  当前检测到启动降级、数据库异常或记忆索引损坏。建议先备份，再继续试用 Builder /
                  Chat。
                </p>
              </div>
              <span className="rounded-full bg-amber-100 px-2 py-1 text-xs font-medium text-amber-800">
                Safe Mode
              </span>
            </div>
            <div className="space-y-2">
              {recoveryIssues.map(issue => (
                <div
                  key={`${issue.title}-${issue.detail}`}
                  className={classNames(
                    "rounded-xl border px-3 py-3",
                    issue.tone === "error"
                      ? "border-rose-200 bg-white text-rose-900"
                      : "border-amber-200 bg-white text-amber-900"
                  )}
                >
                  <div className="text-sm font-medium">{issue.title}</div>
                  <div className="mt-1 text-xs opacity-80">{issue.detail}</div>
                </div>
              ))}
            </div>
            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-xl border border-white bg-white/80 px-3 py-3">
                <div className="text-xs font-medium text-stone-700">活跃数据目录</div>
                <div className="mt-1 break-all text-xs text-stone-500">
                  {diagnostics?.active_data_dir ?? diagnostics?.data_dir ?? "-"}
                </div>
              </div>
              <div className="rounded-xl border border-white bg-white/80 px-3 py-3">
                <div className="text-xs font-medium text-stone-700">兼容旧目录</div>
                <div className="mt-1 break-all text-xs text-stone-500">
                  {diagnostics?.legacy_data_dir ?? "未检测到旧目录"}
                </div>
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              <button
                onClick={handleExport}
                disabled={exportLoading}
                className="rounded-md bg-amber-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-amber-950 disabled:opacity-50"
              >
                {exportLoading ? "导出中..." : "先导出完整备份"}
              </button>
              <button
                onClick={refreshAllDiagnostics}
                className="rounded-md border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-900 hover:bg-amber-100"
              >
                重新检查数据状态
              </button>
              <button
                onClick={async () => {
                  if (safeMode) {
                    setTierResult(buildSafeModeBlockedMessage("记忆层级维护", diagnostics));
                    return;
                  }
                  setTierLoading(true);
                  setTierResult(null);
                  try {
                    const res = await runMemoryTierMaintenance();
                    setTierResult(
                      `记忆层级维护已完成：晋升 ${res.promoted} 条，降级 ${res.demoted} 条。`
                    );
                    await refreshAllDiagnostics();
                  } catch (e) {
                    setTierResult(`记忆层级维护失败：${readableError(e)}`);
                  } finally {
                    setTierLoading(false);
                  }
                }}
                disabled={tierLoading || safeMode}
                className="rounded-md border border-stone-300 bg-white px-3 py-1.5 text-xs font-medium text-stone-800 hover:bg-stone-50 disabled:opacity-50"
              >
                {tierLoading ? "检查中..." : "运行记忆层级维护"}
              </button>
              <button
                onClick={async () => {
                  if (!confirm("确定重建向量索引吗？系统会基于现有聊天消息重新生成记忆向量。"))
                    return;
                  setRebuildLoading(true);
                  setRebuildResult(null);
                  try {
                    const res = await rebuildMemoryIndex();
                    const refreshed = await refreshAllDiagnostics();
                    const recovered = refreshed && !isSafeMode(refreshed);
                    setRebuildResult(
                      `向量索引重建完成：共处理 ${res.processed} 条消息，重建 ${res.indexed} 条，跳过 ${res.skipped} 条。${
                        recovered
                          ? " 当前数据环境已恢复，可继续试用。"
                          : " 已刷新诊断，请继续确认数据环境是否恢复。"
                      }`
                    );
                  } catch (e) {
                    setRebuildResult(`向量索引重建失败：${readableError(e)}`);
                  } finally {
                    setRebuildLoading(false);
                  }
                }}
                disabled={rebuildLoading}
                className="rounded-md border border-emerald-300 bg-white px-3 py-1.5 text-xs font-medium text-emerald-800 hover:bg-emerald-50 disabled:opacity-50"
              >
                {rebuildLoading ? "重建中..." : "重建向量索引"}
              </button>
            </div>
            {rebuildResult && (
              <div className="rounded-lg bg-white/80 px-3 py-2 text-xs text-stone-700">
                {rebuildResult}
              </div>
            )}
            <div className="rounded-lg bg-white/80 px-3 py-3 text-xs text-stone-600">
              如果这里持续提示向量损坏，建议顺序是：先导出备份，再点击“重建向量索引”，最后刷新状态确认损坏计数是否下降。
            </div>
          </section>
        )}

        {/* Data File Health */}
        <section id="data-health" className="space-y-4">
          <h3 className="text-sm font-medium text-gray-700">数据文件健康</h3>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
            {dataFileItems.map(item => (
              <div
                key={item.label}
                className={classNames(
                  "rounded-lg border p-3",
                  item.exists
                    ? "border-emerald-200 bg-emerald-50/40"
                    : "border-amber-200 bg-amber-50/40"
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
        <ProviderConfigSection
          config={config}
          onConfigChange={setConfig}
          diagnostics={diagnostics}
        />

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
                  <div className="text-slate-600">
                    {provider.enabled ? "enabled" : "disabled"}
                  </div>
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
                checked={config.experimental_model_router ?? false}
                onChange={e =>
                  setConfig(prev => ({
                    ...prev,
                    experimental_model_router: e.target.checked,
                  }))
                }
                className="rounded border-gray-300"
              />
              <span className="text-sm text-gray-700">启用 ModelRouter（灰度测试）</span>
            </label>

            <div className="text-xs text-gray-500 bg-gray-50 p-3 rounded-lg">
              <p>⚠️ 实验性功能可能导致不稳定行为。</p>
              <p>开启后会同时使用新旧实现，用于对比测试。</p>
              <p>如遇到问题，请关闭后反馈。</p>
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
                securityMessage.includes("失败")
                  ? "bg-red-50 text-red-700"
                  : "bg-blue-50 text-blue-700"
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
                <div>
                  最近刷新：
                  {hotCache?.last_refreshed
                    ? new Date(hotCache.last_refreshed).toLocaleString()
                    : "-"}
                </div>
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
                  disabled={securityLoading || safeMode}
                  className="rounded-md border border-slate-200 bg-white px-3 py-1.5 text-xs text-slate-700 hover:bg-slate-50 disabled:opacity-50"
                >
                  清理旧日志
                </button>
                <button
                  onClick={handleRotateAuditKey}
                  disabled={securityLoading || safeMode}
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
                <div className="mt-1 text-xs text-slate-500">
                  保存后会写入本地 privacy_policy.yaml，重启后继续生效。
                </div>
              </div>
              <label className="flex items-center gap-2 text-xs text-slate-600">
                <input
                  type="checkbox"
                  checked={privacyPolicy?.enabled ?? true}
                  onChange={e =>
                    setPrivacyPolicyState(prev => ({
                      ...(prev ?? { rules: [] }),
                      enabled: e.target.checked,
                    }))
                  }
                />
                启用隐私处理
              </label>
            </div>
            <div className="mt-3 grid gap-2 md:grid-cols-3">
              {(privacyPolicy?.rules ?? []).map((rule, index) => (
                <div
                  key={`${rule.ptype}-${index}`}
                  className="rounded-lg border border-slate-100 bg-slate-50 px-3 py-2 text-xs"
                >
                  <div className="font-medium text-slate-700">{rule.ptype}</div>
                  <div className="mt-1 flex items-center justify-between gap-2">
                    <label className="flex items-center gap-1 text-slate-600">
                      <input
                        type="checkbox"
                        checked={rule.enabled}
                        onChange={e =>
                          setPrivacyPolicyState(prev => {
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
                      onChange={e =>
                        setPrivacyPolicyState(prev => {
                          if (!prev) return prev;
                          const next = [...prev.rules];
                          next[index] = {
                            ...next[index],
                            action: e.target.value as "Mask" | "Block" | "Allow",
                          };
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
              disabled={importLoading || safeMode}
              className="px-3 py-2 bg-white border border-gray-300 text-gray-700 rounded-md text-sm font-medium hover:bg-gray-50 disabled:opacity-50"
            >
              {importLoading ? "导入中..." : "导入全部数据"}
            </button>
          </div>
          <p className="text-xs text-gray-500">
            导出将包含 LifeModel、聊天记录与向量记忆数据，格式为
            JSON（带版本号与主版本校验）。导入会覆盖当前数据，跨主版本导入会被拒绝，请谨慎操作。
          </p>
        </section>

        {/* Agent Execution Governance */}
        <section className="space-y-4 border-t pt-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <h3 className="text-sm font-medium text-gray-700">Agent 执行权限</h3>
              <p className="mt-1 text-xs text-gray-500">
                高风险工具和写操作默认进入确认流；这里展示已经授予或拒绝的后端权限策略。
              </p>
            </div>
            <button
              onClick={refreshAllDiagnostics}
              className="rounded-md border border-gray-300 bg-white px-3 py-1.5 text-xs font-medium text-gray-700 hover:bg-gray-50"
            >
              刷新
            </button>
          </div>
          <div className="space-y-2">
            {toolPermissions.length === 0 ? (
              <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50 px-3 py-3 text-xs text-gray-500">
                暂无工具权限策略。高风险工具会在执行前请求确认。
              </div>
            ) : (
              toolPermissions.map(permission => (
                <div
                  key={permission.id}
                  className="flex items-center justify-between gap-3 rounded-lg border border-gray-200 bg-white px-3 py-3"
                >
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-gray-900">{permission.toolName}</div>
                    <div className="mt-1 text-xs text-gray-500">
                      {permission.policy} · {permission.source} · {permission.riskLevel} ·{" "}
                      {permission.actionType}
                    </div>
                  </div>
                  <button
                    onClick={async () => {
                      await revokeToolPermission(permission.id);
                      await refreshAllDiagnostics();
                    }}
                    className="shrink-0 rounded-md border border-rose-200 bg-white px-3 py-1.5 text-xs font-medium text-rose-700 hover:bg-rose-50"
                  >
                    撤销
                  </button>
                </div>
              ))
            )}
          </div>
        </section>

        <PluginSection
          plugins={plugins}
          diagnostics={diagnostics}
          onPluginsChange={setPlugins}
          onRefreshDiagnostics={refreshAllDiagnostics}
        />

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
                if (safeMode) {
                  setTierResult(buildSafeModeBlockedMessage("记忆层级维护", diagnostics));
                  return;
                }
                setTierLoading(true);
                setTierResult(null);
                try {
                  const res = await runMemoryTierMaintenance();
                  setTierResult(
                    `记忆层级维护完成：晋升 ${res.promoted} 条，降级 ${res.demoted} 条`
                  );
                } catch (e: any) {
                  setTierResult("维护失败: " + readableError(e));
                } finally {
                  setTierLoading(false);
                }
              }}
              disabled={tierLoading || safeMode}
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
