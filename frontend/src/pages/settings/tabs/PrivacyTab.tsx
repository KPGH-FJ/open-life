import type {
  AppConfig,
  HotMemoryCache,
  PrivacyPolicy,
  SystemDiagnostics,
  ToolManifest,
  ToolPermissionRecord,
} from "../../../tauri";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface PrivacyTabProps {
  diagnostics: SystemDiagnostics | null;
  hotCache: HotMemoryCache | null;
  privacyPolicy: PrivacyPolicy | null;
  setPrivacyPolicyState: React.Dispatch<React.SetStateAction<PrivacyPolicy | null>>;
  securityLoading: boolean;
  securityMessage: string | null;
  handleExportAudit: () => Promise<void>;
  handleCleanupAudit: () => Promise<void>;
  handleRotateAuditKey: () => Promise<void>;
  toolPermissions: ToolPermissionRecord[];
  revokeToolPermission: (id: string) => Promise<boolean>;
  refreshAllDiagnostics: () => Promise<SystemDiagnostics | null>;
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  refreshSecurityState: () => Promise<void>;
  toolManifests: ToolManifest[];
  safeMode: boolean;
  handleSavePrivacyPolicy: () => Promise<void>;
}

export default function PrivacyTab({
  diagnostics,
  hotCache,
  privacyPolicy,
  setPrivacyPolicyState,
  securityLoading,
  securityMessage,
  handleExportAudit,
  handleCleanupAudit,
  handleRotateAuditKey,
  refreshSecurityState,
  safeMode,
  handleSavePrivacyPolicy,
}: PrivacyTabProps) {
  return (
    <>
      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium text-gray-700">隐私与长期记忆</h3>
            <p className="mt-1 text-xs text-gray-500">
              这里只处理长期数据、PII 策略和本地审计；工具权限已移到 Tools & Permissions。
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
            <div className="text-sm font-semibold text-slate-800">本地审计</div>
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
              <div className="text-sm font-semibold text-slate-800">PII 与隐私策略</div>
              <div className="mt-1 text-xs text-slate-500">
                保存后写入本地 privacy_policy.yaml，重启后继续生效。
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
    </>
  );
}
