import { useEffect, useState } from "react";
import {
  getConfig,
  saveConfig,
  type AppConfig,
  exportAllData,
  importAllData,
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
  listToolManifests,
  type ToolPermissionRecord,
  type PluginRecord,
  type ToolManifest,
} from "../tauri";
import { LayoutDashboard, Cpu, Shield, Database, Puzzle } from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import LoadingSpinner from "../components/LoadingSpinner";
import { isSafeMode } from "../utils/safeMode";
import { readableError } from "../utils/error";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";
import PluginSection from "./settings/PluginSection";
import OverviewTab from "./settings/tabs/OverviewTab";
import ProviderTab from "./settings/tabs/ProviderTab";
import PrivacyTab from "./settings/tabs/PrivacyTab";
import DataTab from "./settings/tabs/DataTab";

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
  const [toolManifests, setToolManifests] = useState<ToolManifest[]>([]);
  const [securityLoading, setSecurityLoading] = useState(false);
  const [securityMessage, setSecurityMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<string>("overview");

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
    refreshAllDiagnostics();
  }, []);

  const refreshAllDiagnostics = async () => {
    const [router, modelRouter, diag, cache, policy, permissions, pluginRecords, manifests] =
      await Promise.all([
        getRouterStatus().catch(() => null),
        getModelRouterStatus().catch(() => null),
        getSystemDiagnostics().catch(() => null),
        getHotCache().catch(() => null),
        getPrivacyPolicy().catch(() => null),
        listToolPermissions().catch(() => []),
        listPlugins().catch(() => []),
        listToolManifests().catch(() => []),
      ]);
    setRouterStatus(router);
    setModelRouterStatus(modelRouter);
    setDiagnostics(diag);
    setHotCache(cache);
    setPrivacyPolicyState(policy);
    setToolPermissions(permissions);
    setPlugins(pluginRecords);
    setToolManifests(manifests);
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

  const handleExportDiagnostics = async () => {
    setExportLoading(true);
    setMessage(null);
    try {
      const diag = await getSystemDiagnostics();
      const safeDiagnostics = buildSafeDiagnosticExportPayload(diag);

      const report = {
        timestamp: new Date().toISOString(),
        app_version: diag?.app_version || "unknown",
        platform: navigator.platform,
        userAgent: navigator.userAgent,
        diagnostics: safeDiagnostics,
        config_summary: {
          provider: config.llm?.provider,
          prefer_local: config.prefer_local_model,
          local_model: config.local_model,
          chat_proposal_enabled: config.chat_proposal?.enabled,
          use_agent_loop: config.use_agent_loop,
        },
        screen_size: `${window.screen.width}x${window.screen.height}`,
        language: navigator.language,
        privacy_manifest: {
          export_strategy: "explicit-whitelist",
          includes_raw_life_model: false,
          includes_raw_messages: false,
          includes_raw_memory: false,
          includes_raw_tool_output: false,
          includes_raw_prompts: false,
          includes_api_keys: false,
          includes_raw_config: false,
          includes_local_paths: false,
          retained_summary_fields:
            "Boolean readiness flags, numeric counts, provider/model names (non-sensitive), database/file health booleans, startup warnings / readiness issues (local paths redacted to [local-path]), builder completion percentages, userAgent, platform",
          purpose: "Beta trial feedback and issue diagnosis",
          auto_upload: false,
        },
        feedback_guidance: {
          what_to_include:
            "Describe what you were doing, what you expected, and what actually happened. Include run IDs, proposal IDs, or plan IDs if visible.",
          what_not_to_include:
            "Do not paste raw LifeModel content, chat messages, memory content, or tool output unless explicitly requested.",
          how_to_report:
            "Export this diagnostic file and attach it to your issue report. The file contains system state and config summary only.",
        },
      };
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `openlife-diagnostics-${new Date().toISOString().slice(0, 10)}.json`,
      });
      if (!path) {
        return;
      }
      await writeTextFile(path, JSON.stringify(report, null, 2));
      setMessage("诊断报告导出成功（已排除原始敏感内容）");
    } catch (e: any) {
      setMessage("诊断报告导出失败: " + readableError(e));
    } finally {
      setExportLoading(false);
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

  const safeMode = isSafeMode(diagnostics);

  const handleNavigateToTab = (tabId: string, anchorId?: string) => {
    setActiveTab(tabId);
    if (anchorId) {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const el = document.getElementById(anchorId);
          if (el) {
            el.scrollIntoView({ behavior: "smooth", block: "start" });
          }
        });
      });
    }
  };

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

        {/* Tab Navigation */}
        <div className="flex gap-1 border-b border-gray-200 pb-1 overflow-x-auto">
          {[
            { id: "overview", label: "概览", icon: LayoutDashboard },
            { id: "provider", label: "模型", icon: Cpu },
            { id: "privacy", label: "隐私安全", icon: Shield },
            { id: "data", label: "数据", icon: Database },
            { id: "plugins", label: "插件", icon: Puzzle },
          ].map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={classNames(
                "flex items-center gap-1.5 px-3 py-2 text-sm font-medium rounded-t-lg transition",
                activeTab === tab.id
                  ? "text-indigo-600 border-b-2 border-indigo-600 bg-indigo-50/50"
                  : "text-gray-600 hover:text-gray-900 hover:bg-gray-50"
              )}
            >
              <tab.icon size={14} />
              {tab.label}
            </button>
          ))}
        </div>

        {/* Overview Tab */}
        {activeTab === "overview" && (
          <OverviewTab
            diagnostics={diagnostics}
            safeMode={safeMode}
            exportLoading={exportLoading}
            handleExport={handleExport}
            refreshAllDiagnostics={refreshAllDiagnostics}
            tierLoading={tierLoading}
            setTierLoading={setTierLoading}
            setTierResult={setTierResult}
            rebuildLoading={rebuildLoading}
            setRebuildLoading={setRebuildLoading}
            rebuildResult={rebuildResult}
            setRebuildResult={setRebuildResult}
            onNavigateTab={handleNavigateToTab}
          />
        )}

        {/* Provider Tab */}
        {activeTab === "provider" && (
          <ProviderTab
            config={config}
            setConfig={setConfig}
            diagnostics={diagnostics}
            routerStatus={routerStatus}
            modelRouterStatus={modelRouterStatus}
          />
        )}

        {/* Privacy & Security Tab */}
        {activeTab === "privacy" && (
          <PrivacyTab
            diagnostics={diagnostics}
            hotCache={hotCache}
            privacyPolicy={privacyPolicy}
            setPrivacyPolicyState={setPrivacyPolicyState}
            securityLoading={securityLoading}
            securityMessage={securityMessage}
            handleExportAudit={handleExportAudit}
            handleCleanupAudit={handleCleanupAudit}
            handleRotateAuditKey={handleRotateAuditKey}
            toolPermissions={toolPermissions}
            revokeToolPermission={revokeToolPermission}
            refreshAllDiagnostics={refreshAllDiagnostics}
            config={config}
            setConfig={setConfig}
            refreshSecurityState={refreshSecurityState}
            toolManifests={toolManifests}
            safeMode={safeMode}
            handleSavePrivacyPolicy={handleSavePrivacyPolicy}
          />
        )}

        {/* Data Tab */}
        {activeTab === "data" && (
          <DataTab
            handleExport={handleExport}
            handleImport={handleImport}
            exportLoading={exportLoading}
            importLoading={importLoading}
            safeMode={safeMode}
            diagnostics={diagnostics}
            evolutionLoading={evolutionLoading}
            evolutionResult={evolutionResult}
            setEvolutionLoading={setEvolutionLoading}
            setEvolutionResult={setEvolutionResult}
            tierLoading={tierLoading}
            tierResult={tierResult}
            setTierLoading={setTierLoading}
            setTierResult={setTierResult}
            handleExportDiagnostics={handleExportDiagnostics}
          />
        )}

        {/* Plugins Tab */}
        {activeTab === "plugins" && (
          <PluginSection
            plugins={plugins}
            onPluginsChange={setPlugins}
            onRefreshDiagnostics={refreshAllDiagnostics}
          />
        )}

        {/* Save button - always visible */}
        <div className="flex justify-end pt-4 border-t">
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

function redactLocalPathsFromText(text: string): string {
  if (!text) return text;

  return text
    .replace(
      /(^|[\s"'=(:：])file:\/\/\/[^,;'"()\[\]<>]+/gi,
      (_match, prefix) => `${prefix}[local-file-url]`
    )
    .replace(
      /(^|[\s"'=(:：])\\\\[a-zA-Z0-9_.-]+\\[^,;'"()\[\]<>]+/g,
      (_match, prefix) => `${prefix}[local-path]`
    )
    .replace(
      /(^|[\s"'=(:：])[A-Za-z]:\\[^,;'"()\[\]<>]+/g,
      (_match, prefix) => `${prefix}[local-path]`
    )
    .replace(
      /(^|[\s"'=(:：])\/[a-zA-Z][^,;'"()\[\]<>]*(?:\/[^,;'"()\[\]<>]+)+/g,
      (_match, prefix) => `${prefix}[local-path]`
    );
}

function redactLocalPathsFromList(values?: string[] | null): string[] {
  if (!values || values.length === 0) return [];
  return values.map(v => redactLocalPathsFromText(v));
}

function buildSafeDiagnosticExportPayload(
  diag: SystemDiagnostics | null
): Record<string, unknown> | null {
  if (!diag) return null;

  return {
    beta_ready: diag.beta_ready,
    chat_ready: diag.chat_ready,
    onboarding_completed: diag.onboarding_completed,
    cloud_api_configured: diag.cloud_api_configured,
    cloud_provider: diag.cloud_provider ?? null,
    config_source: diag.config_source,
    memory_chunk_count: diag.memory_chunk_count,
    vector_corrupt_embedding_count: diag.vector_corrupt_embedding_count ?? 0,
    startup_warnings: redactLocalPathsFromList(diag.startup_warnings),
    database_status: diag.database_status ?? "unknown",
    chat_session_count: diag.chat_session_count,
    agent_run_count: diag.agent_run_count,
    agent_run_store_status: diag.agent_run_store_status,
    pending_proposal_count: diag.pending_proposal_count,
    high_risk_pending_proposal_count: diag.high_risk_pending_proposal_count,
    proposal_store_status: diag.proposal_store_status,
    snapshot_count: diag.snapshot_count,
    life_model_ready: diag.life_model_ready,
    model_empty: diag.model_empty,
    ollama_online: diag.ollama_online,
    local_model: diag.local_model,
    resolved_local_model: diag.resolved_local_model ?? null,
    prefer_local_model: diag.prefer_local_model,
    mcp_server_count: diag.mcp_server_count,
    mcp_tool_count: diag.mcp_tool_count,
    mcp_recent_audit_count: diag.mcp_recent_audit_count,
    mcp_recent_pii_count: diag.mcp_recent_pii_count,
    unfinished_builder_sessions: diag.unfinished_builder_sessions,
    pending_builder_review_sessions: diag.pending_builder_review_sessions ?? 0,
    app_version: diag.app_version,
    readiness_issues: redactLocalPathsFromList(diag.readiness_issues),
    beta_readiness_issues: redactLocalPathsFromList(diag.beta_readiness_issues),
    router: {
      onnx_available: diag.router.onnx_available,
      active_backend: diag.router.active_backend,
    },
    builder_completion: diag.builder_completion
      ? {
          identity: diag.builder_completion.identity,
          goals: diag.builder_completion.goals,
          capabilities: diag.builder_completion.capabilities,
          state: diag.builder_completion.state,
          overall: diag.builder_completion.overall,
          lowest_dimension: diag.builder_completion.lowest_dimension,
        }
      : null,
    data_files: diag.data_files
      ? {
          messages_db_exists: diag.data_files.messages_db_exists,
          messages_db_size_mb: diag.data_files.messages_db_size_mb,
          vectors_db_exists: diag.data_files.vectors_db_exists,
          vectors_db_size_mb: diag.data_files.vectors_db_size_mb,
          mcp_audit_db_exists: diag.data_files.mcp_audit_db_exists,
          mcp_audit_db_size_mb: diag.data_files.mcp_audit_db_size_mb,
          config_yaml_exists: diag.data_files.config_yaml_exists,
          life_model_yaml_exists: diag.data_files.life_model_yaml_exists,
        }
      : null,
    ollama_models: (diag.ollama_models ?? []).map(m => ({
      name: m.name,
      size_mb: m.size_mb,
    })),
  };
}
