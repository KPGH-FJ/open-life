import type {
  SystemDiagnostics,
  HotMemoryCache,
  PrivacyPolicy,
  ToolPermissionRecord,
  ToolManifest,
  AppConfig,
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
  toolPermissions,
  revokeToolPermission,
  refreshAllDiagnostics,
  config,
  setConfig,
  refreshSecurityState,
  toolManifests,
  safeMode,
  handleSavePrivacyPolicy,
}: PrivacyTabProps) {
  return (
    <>
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

      {/* Network Policy */}
      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium text-gray-700">网络访问策略</h3>
            <p className="mt-1 text-xs text-gray-500">
              控制 web.fetch 和 web.search 工具的域名访问权限。
            </p>
          </div>
          <label className="flex items-center gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={config.system?.network_policy?.enabled ?? true}
              onChange={e =>
                setConfig(prev => ({
                  ...prev,
                  system: {
                    ...prev.system,
                    network_policy: {
                      ...prev.system?.network_policy,
                      enabled: e.target.checked,
                    },
                  },
                }))
              }
              className="rounded border-gray-300"
            />
            启用网络策略
          </label>
        </div>

        <div>
          <label className="block text-xs text-gray-500 mb-1">默认决策</label>
          <select
            value={config.system?.network_policy?.default_decision ?? "ask"}
            onChange={e =>
              setConfig(prev => ({
                ...prev,
                system: {
                  ...prev.system,
                  network_policy: {
                    ...prev.system?.network_policy,
                    default_decision: e.target.value as "ask" | "allow" | "deny",
                  },
                },
              }))
            }
            className="w-full rounded-lg border border-gray-200 px-3 py-2 text-sm"
          >
            <option value="ask">询问（每次确认）</option>
            <option value="allow">允许</option>
            <option value="deny">拒绝</option>
          </select>
        </div>

        <div>
          <h4 className="text-sm font-medium text-gray-900 mb-2">域名白名单</h4>
          <div className="space-y-2">
            {(config.system?.network_policy?.domain_allowlist ?? []).map((domain, idx) => (
              <div key={idx} className="flex items-center gap-2">
                <input
                  type="text"
                  value={domain}
                  readOnly
                  className="flex-1 px-3 py-1.5 text-sm border rounded bg-gray-50"
                />
                <button
                  onClick={() =>
                    setConfig(prev => ({
                      ...prev,
                      system: {
                        ...prev.system,
                        network_policy: {
                          ...prev.system?.network_policy,
                          domain_allowlist: (
                            prev.system?.network_policy?.domain_allowlist ?? []
                          ).filter((_, i) => i !== idx),
                        },
                      },
                    }))
                  }
                  className="px-2 py-1 text-sm text-red-600 hover:bg-red-50 rounded"
                >
                  删除
                </button>
              </div>
            ))}
            <div className="flex items-center gap-2">
              <input
                type="text"
                placeholder="添加域名..."
                id="new-allow-domain"
                className="flex-1 px-3 py-1.5 text-sm border rounded"
              />
              <button
                onClick={() => {
                  const input = document.getElementById("new-allow-domain") as HTMLInputElement;
                  const domain = input.value.trim();
                  if (!domain) {
                    alert("域名不能为空");
                    return;
                  }
                  const existing = config.system?.network_policy?.domain_allowlist ?? [];
                  if (existing.includes(domain)) {
                    alert("域名已存在");
                    return;
                  }
                  setConfig(prev => ({
                    ...prev,
                    system: {
                      ...prev.system,
                      network_policy: {
                        ...prev.system?.network_policy,
                        domain_allowlist: [...existing, domain],
                      },
                    },
                  }));
                  input.value = "";
                }}
                className="px-3 py-1.5 text-sm bg-stone-900 text-white rounded hover:bg-stone-800"
              >
                添加
              </button>
            </div>
          </div>
        </div>

        <div>
          <h4 className="text-sm font-medium text-gray-900 mb-2">域名黑名单</h4>
          <div className="space-y-2">
            {(config.system?.network_policy?.domain_denylist ?? []).map((domain, idx) => (
              <div key={idx} className="flex items-center gap-2">
                <input
                  type="text"
                  value={domain}
                  readOnly
                  className="flex-1 px-3 py-1.5 text-sm border rounded bg-gray-50"
                />
                <button
                  onClick={() =>
                    setConfig(prev => ({
                      ...prev,
                      system: {
                        ...prev.system,
                        network_policy: {
                          ...prev.system?.network_policy,
                          domain_denylist: (
                            prev.system?.network_policy?.domain_denylist ?? []
                          ).filter((_, i) => i !== idx),
                        },
                      },
                    }))
                  }
                  className="px-2 py-1 text-sm text-red-600 hover:bg-red-50 rounded"
                >
                  删除
                </button>
              </div>
            ))}
            <div className="flex items-center gap-2">
              <input
                type="text"
                placeholder="添加域名..."
                id="new-deny-domain"
                className="flex-1 px-3 py-1.5 text-sm border rounded"
              />
              <button
                onClick={() => {
                  const input = document.getElementById("new-deny-domain") as HTMLInputElement;
                  const domain = input.value.trim();
                  if (!domain) {
                    alert("域名不能为空");
                    return;
                  }
                  const existing = config.system?.network_policy?.domain_denylist ?? [];
                  if (existing.includes(domain)) {
                    alert("域名已存在");
                    return;
                  }
                  setConfig(prev => ({
                    ...prev,
                    system: {
                      ...prev.system,
                      network_policy: {
                        ...prev.system?.network_policy,
                        domain_denylist: [...existing, domain],
                      },
                    },
                  }));
                  input.value = "";
                }}
                className="px-3 py-1.5 text-sm bg-stone-900 text-white rounded hover:bg-stone-800"
              >
                添加
              </button>
            </div>
          </div>
        </div>

        <div className="text-xs text-gray-500 bg-gray-50 p-3 rounded-lg">
          <p>
            网络策略在下次对话时生效。当前策略：
            {(config.system?.network_policy?.enabled ?? true) ? "已启用" : "已禁用"}
          </p>
          <p>白名单优先于黑名单。工具级覆盖优先于域名策略。</p>
        </div>
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

      {/* Tool Registry */}
      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-gray-700">Tool Registry</h3>
          <button
            onClick={refreshAllDiagnostics}
            className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-50"
          >
            刷新
          </button>
        </div>
        <div className="space-y-2 max-h-64 overflow-auto">
          {toolManifests.length === 0 ? (
            <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50 px-3 py-3 text-xs text-gray-500">
              暂无工具注册
            </div>
          ) : (
            toolManifests.map(manifest => {
              const isDeclarative = manifest.declarative_only;
              const isDisabled = !manifest.enabled;
              const isReal = !isDeclarative && !isDisabled;
              const sourceStr =
                typeof manifest.source === "string"
                  ? manifest.source
                  : manifest.source.type === "BuiltIn"
                    ? "builtin"
                    : manifest.source.type === "Mcp"
                      ? `mcp:${manifest.source.server_name}`
                      : manifest.source.type === "A2A"
                        ? `a2a:${manifest.source.agent_name}`
                        : `plugin:${manifest.source.plugin_id}`;
              return (
                <div
                  key={manifest.id}
                  className={`flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-xs ${
                    isReal
                      ? "border-green-200 bg-green-50"
                      : isDeclarative
                        ? "border-amber-200 bg-amber-50"
                        : "border-red-200 bg-red-50"
                  }`}
                >
                  <div className="min-w-0">
                    <div className="font-medium text-gray-900">{manifest.name}</div>
                    <div className="mt-0.5 text-gray-500">
                      {sourceStr} · {manifest.risk_level} · {manifest.action_type || "—"}
                    </div>
                    <div className="mt-0.5 text-gray-400">
                      {manifest.capabilities.join(", ") || "none"}
                    </div>
                  </div>
                  <span
                    className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                      isReal
                        ? "bg-green-100 text-green-700"
                        : isDeclarative
                          ? "bg-amber-100 text-amber-700"
                          : "bg-red-100 text-red-700"
                    }`}
                  >
                    {isReal ? "✅ 可执行" : isDeclarative ? "⚠️ 声明-only" : "❌ 禁用"}
                  </span>
                </div>
              );
            })
          )}
        </div>
      </section>
    </>
  );
}
