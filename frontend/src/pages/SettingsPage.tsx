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
      const report = {
        timestamp: new Date().toISOString(),
        app_version: diag?.app_version || "unknown",
        platform: navigator.platform,
        userAgent: navigator.userAgent,
        diagnostics: diag,
        config_summary: {
          provider: config.llm?.provider,
          prefer_local: config.prefer_local_model,
          local_model: config.local_model,
          chat_proposal_enabled: config.chat_proposal?.enabled,
          use_agent_loop: config.use_agent_loop,
        },
        screen_size: `${window.screen.width}x${window.screen.height}`,
        language: navigator.language,
      };
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `openlife-diagnostics-${new Date().toISOString().slice(0, 10)}.json`,
      });
      if (!path) {
        setExportLoading(false);
        return;
      }
      await writeTextFile(path, JSON.stringify(report, null, 2));
      setMessage("诊断报告导出成功");
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

  const safeMode = isSafeMode(diagnostics);

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

function readableError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as any).message === "string") return (e as any).message;
    if ("error" in e && typeof (e as any).error === "string") return (e as any).error;
  }
  return String(e);
}
