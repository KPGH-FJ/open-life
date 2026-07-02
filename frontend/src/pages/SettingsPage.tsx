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
  rebuildMemoryIndex,
  getPrivacyPolicy,
  setPrivacyPolicy,
  getDangerActionPreflight,
  buildDangerActionConfirmationEvidence,
  type ExportPayload,
  type HotMemoryCache,
  type PrivacyPolicy,
  type RouterStatus,
  type ModelRouterStatus,
  type SystemDiagnostics,
  type DangerActionPreflightView,
  type DangerActionType,
  type DangerActionConfirmationEvidence,
  listToolPermissions,
  revokeToolPermission,
  listPlugins,
  listToolManifests,
  type ToolPermissionRecord,
  type PluginRecord,
  type ToolManifest,
} from "../tauri";
import { Cpu, Shield, Wrench, Inbox, SlidersHorizontal } from "lucide-react";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import LoadingSpinner from "../components/LoadingSpinner";
import { isSafeMode } from "../utils/safeMode";
import { isInternalDebugSurfaceEnabled } from "../utils/internalDebug";
import { buildRuntimeActionError } from "../utils/runtimeMessages";
import PluginSection from "./settings/PluginSection";
import OverviewTab from "./settings/tabs/OverviewTab";
import ProviderTab from "./settings/tabs/ProviderTab";
import PrivacyTab from "./settings/tabs/PrivacyTab";
import DataTab from "./settings/tabs/DataTab";
import ToolsPermissionsTab from "./settings/tabs/ToolsPermissionsTab";
import ReviewMemoryTab from "./settings/tabs/ReviewMemoryTab";
import AdvancedTab from "./settings/tabs/AdvancedTab";
import MultiStrategyPreviewSection from "./settings/MultiStrategyPreviewSection";
import ConfirmDangerDialog from "../components/ConfirmDangerDialog";
import DangerActionPreflightDetails from "../components/DangerActionPreflightDetails";

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
    runtime_mode: "local_first_default",
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

const DANGER_ACTION_LABELS: Record<DangerActionType, string> = {
  data_export: "导出全部数据",
  data_import_overwrite: "导入覆盖备份",
  mcp_audit_export: "导出审计",
  mcp_audit_cleanup: "清理旧日志",
  mcp_audit_key_rotation: "轮换密钥",
  agent_run_delete: "删除运行记录",
  agent_run_bulk_delete: "批量删除运行记录",
  vector_rebuild: "重建向量索引",
};

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
  const [pendingImport, setPendingImport] = useState<{
    payload: ExportPayload;
    path: string;
    confirmationEvidence: DangerActionConfirmationEvidence;
  } | null>(null);
  const [dangerPreflight, setDangerPreflight] = useState<DangerActionPreflightView | null>(null);
  const [dangerPreflightAction, setDangerPreflightAction] = useState<DangerActionType | null>(null);
  const [dangerPreflightLoading, setDangerPreflightLoading] = useState<DangerActionType | null>(
    null
  );
  const showInternalDebug = isInternalDebugSurfaceEnabled();

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

  useEffect(() => {
    if (!showInternalDebug && activeTab === "experimental") {
      setActiveTab("advanced");
    }
  }, [activeTab, showInternalDebug]);

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
          runtime_mode: config.runtime_mode,
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

  const openDangerActionPreflight = async (
    actionType: DangerActionType,
    channel: "data" | "security"
  ) => {
    setDangerPreflightLoading(actionType);
    if (channel === "security") {
      setSecurityMessage(null);
    } else {
      setMessage(null);
    }
    try {
      const view = await getDangerActionPreflight(actionType, safeMode);
      setDangerPreflight(view);
      setDangerPreflightAction(actionType);
    } catch (e: any) {
      const errorMessage = "动作预检失败: " + readableError(e);
      if (channel === "security") {
        setSecurityMessage(errorMessage);
      } else {
        setMessage(errorMessage);
      }
    } finally {
      setDangerPreflightLoading(current => (current === actionType ? null : current));
    }
  };

  const handleExport = async () => openDangerActionPreflight("data_export", "data");

  const executeExport = async () => {
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

  const handleImport = async () => openDangerActionPreflight("data_import_overwrite", "data");

  const executeImportFileSelection = async (
    confirmationEvidence: DangerActionConfirmationEvidence
  ) => {
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
      setPendingImport({ payload, path, confirmationEvidence });
      setMessage("已读取导入文件，请确认覆盖导入。");
    } catch (e: any) {
      setMessage(buildRuntimeActionError("导入数据", e, "data"));
    } finally {
      setImportLoading(false);
    }
  };

  const confirmImport = async () => {
    if (!pendingImport) return;
    setImportLoading(true);
    setMessage(null);
    try {
      await importAllData(pendingImport.payload, pendingImport.confirmationEvidence);
      setPendingImport(null);
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

  const handleExportAudit = async () => openDangerActionPreflight("mcp_audit_export", "security");

  const executeExportAudit = async () => {
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

  const handleCleanupAudit = async () => openDangerActionPreflight("mcp_audit_cleanup", "security");

  const executeCleanupAudit = async (confirmationEvidence: DangerActionConfirmationEvidence) => {
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      const removed = await cleanupMcpAuditLogs(90, confirmationEvidence);
      setSecurityMessage(`已清理 ${removed} 条旧 MCP 审计日志`);
      await refreshSecurityState();
    } catch (e: any) {
      setSecurityMessage(buildRuntimeActionError("清理审计日志", e, "data"));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleRotateAuditKey = async () =>
    openDangerActionPreflight("mcp_audit_key_rotation", "security");

  const executeRotateAuditKey = async (confirmationEvidence: DangerActionConfirmationEvidence) => {
    setSecurityLoading(true);
    setSecurityMessage(null);
    try {
      await rotateMcpAuditKey(confirmationEvidence);
      setSecurityMessage("审计密钥已轮换，历史日志会继续按原 key epoch 解密");
    } catch (e: any) {
      setSecurityMessage(buildRuntimeActionError("轮换审计密钥", e, "data"));
    } finally {
      setSecurityLoading(false);
    }
  };

  const handleVectorRebuild = async () => openDangerActionPreflight("vector_rebuild", "data");

  const executeVectorRebuild = async (confirmationEvidence: DangerActionConfirmationEvidence) => {
    setRebuildLoading(true);
    setRebuildResult(null);
    try {
      const res = await rebuildMemoryIndex(confirmationEvidence);
      const refreshed = await refreshAllDiagnostics();
      const recovered = refreshed && !isSafeMode(refreshed);
      setRebuildResult(
        `向量索引重建完成：共处理 ${res.processed} 条消息，重建 ${res.indexed} 条，跳过 ${res.skipped} 条。${
          recovered
            ? " 当前数据环境已恢复，可继续使用。"
            : " 已刷新诊断，请继续确认数据环境是否恢复。"
        }`
      );
    } catch (e) {
      setRebuildResult(`向量索引重建失败：${readableError(e)}`);
    } finally {
      setRebuildLoading(false);
    }
  };

  const continueDangerAction = async () => {
    if (!dangerPreflight || !dangerPreflight.finalActionEnabled) return;
    const actionType = dangerPreflight.actionType;
    const confirmationEvidence = buildDangerActionConfirmationEvidence(dangerPreflight);
    setDangerPreflight(null);
    setDangerPreflightAction(null);

    switch (actionType) {
      case "data_export":
        await executeExport();
        break;
      case "data_import_overwrite":
        await executeImportFileSelection(confirmationEvidence);
        break;
      case "mcp_audit_export":
        await executeExportAudit();
        break;
      case "mcp_audit_cleanup":
        await executeCleanupAudit(confirmationEvidence);
        break;
      case "mcp_audit_key_rotation":
        await executeRotateAuditKey(confirmationEvidence);
        break;
      case "vector_rebuild":
        await executeVectorRebuild(confirmationEvidence);
        break;
      default:
        setMessage("动作预检失败: unsupported danger action preflight action type");
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
      {dangerPreflight && (
        <ConfirmDangerDialog
          open={Boolean(dangerPreflight)}
          title={`动作预检：${DANGER_ACTION_LABELS[dangerPreflight.actionType] ?? "危险动作"}`}
          description={<DangerActionPreflightDetails view={dangerPreflight} />}
          confirmLabel={dangerPreflight.finalActionEnabled ? "继续执行" : "Safe Mode 已阻断"}
          cancelLabel="返回"
          severity={dangerPreflight.riskTier === "critical" ? "danger" : "warning"}
          confirmationText={
            dangerPreflight.confirmationRequired && dangerPreflight.confirmationPhrase
              ? dangerPreflight.confirmationPhrase
              : undefined
          }
          confirmDisabled={!dangerPreflight.finalActionEnabled}
          busy={dangerPreflightLoading === dangerPreflightAction}
          onConfirm={() => void continueDangerAction()}
          onCancel={() => {
            setDangerPreflight(null);
            setDangerPreflightAction(null);
          }}
        />
      )}
      <ConfirmDangerDialog
        open={Boolean(pendingImport)}
        title="确认覆盖导入全部数据"
        description={
          <div className="space-y-1">
            <div>导入会覆盖当前 LifeModel、聊天记录与记忆数据。</div>
            <div>文件：{pendingImport?.path ?? ""}</div>
            <div>
              版本：{pendingImport?.payload.version ?? "unknown"}
              {pendingImport?.payload.app_version
                ? ` / 应用 ${pendingImport.payload.app_version}`
                : ""}
            </div>
          </div>
        }
        confirmationText={pendingImport?.confirmationEvidence.confirmationPhrase || "IMPORT"}
        confirmLabel="覆盖导入"
        busy={importLoading}
        onConfirm={() => void confirmImport()}
        onCancel={() => setPendingImport(null)}
      />
      <div className="max-w-5xl mx-auto bg-white rounded-xl shadow p-6 space-y-6">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold text-gray-800">Settings</h2>
            <p className="mt-1 text-xs leading-5 text-gray-500">
              管理模型路线、隐私边界、工具权限和本地数据。诊断和连接细节默认放在高级区。
            </p>
          </div>
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
            { id: "overview", label: "General", icon: SlidersHorizontal },
            { id: "provider", label: "Models", icon: Cpu },
            { id: "privacy_data", label: "Privacy & Data", icon: Shield },
            { id: "tools", label: "Tools & Permissions", icon: Wrench },
            { id: "review_memory", label: "Mailbox & Memory", icon: Inbox },
            { id: "advanced", label: "Advanced", icon: SlidersHorizontal },
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
            handleVectorRebuild={handleVectorRebuild}
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
            showInternalDebug={showInternalDebug}
            onProviderValidationChanged={refreshAllDiagnostics}
          />
        )}

        {/* Privacy & Data Tab */}
        {activeTab === "privacy_data" && (
          <div className="space-y-6">
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
              handleSavePrivacyPolicy={handleSavePrivacyPolicy}
            />
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
          </div>
        )}

        {activeTab === "tools" && (
          <ToolsPermissionsTab
            diagnostics={diagnostics}
            config={config}
            setConfig={setConfig}
            toolPermissions={toolPermissions}
            revokeToolPermission={revokeToolPermission}
            refreshAllDiagnostics={refreshAllDiagnostics}
            refreshSecurityState={refreshSecurityState}
            toolManifests={toolManifests}
          />
        )}

        {activeTab === "review_memory" && (
          <ReviewMemoryTab config={config} setConfig={setConfig} diagnostics={diagnostics} />
        )}

        {activeTab === "advanced" && (
          <AdvancedTab
            config={config}
            setConfig={setConfig}
            diagnostics={diagnostics}
            routerStatus={routerStatus}
            modelRouterStatus={modelRouterStatus}
            showInternalDebug={showInternalDebug}
            pluginSection={
              <PluginSection
                plugins={plugins}
                onPluginsChange={setPlugins}
                onRefreshDiagnostics={refreshAllDiagnostics}
              />
            }
            experimentalSection={showInternalDebug ? <MultiStrategyPreviewSection /> : undefined}
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
