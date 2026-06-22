import type {
  AppConfig,
  SystemDiagnostics,
  ToolManifest,
  ToolPermissionRecord,
} from "../../../tauri";
import { CapabilityCard, StatusChip } from "../../../components/product/ProductPrimitives";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface ToolsPermissionsTabProps {
  diagnostics: SystemDiagnostics | null;
  config: AppConfig;
  setConfig: React.Dispatch<React.SetStateAction<AppConfig>>;
  toolPermissions: ToolPermissionRecord[];
  revokeToolPermission: (id: string) => Promise<boolean>;
  refreshAllDiagnostics: () => Promise<SystemDiagnostics | null>;
  refreshSecurityState: () => Promise<void>;
  toolManifests: ToolManifest[];
}

export default function ToolsPermissionsTab({
  diagnostics,
  config,
  setConfig,
  toolPermissions,
  revokeToolPermission,
  refreshAllDiagnostics,
  refreshSecurityState,
  toolManifests,
}: ToolsPermissionsTabProps) {
  const networkEnabled = config.system?.network_policy?.enabled ?? true;
  const safePathCount = config.system?.safe_paths?.length ?? 0;
  const grantedCount = toolPermissions.filter(permission =>
    ["allow", "allow_once", "allow_until_revoked"].includes(permission.policy)
  ).length;
  const executableManifestCount = toolManifests.filter(
    manifest => manifest.enabled && !manifest.declarative_only
  ).length;

  return (
    <>
      <section className="grid gap-3 md:grid-cols-4">
        <CapabilityCard
          title="Web"
          description="控制 web.fetch / web.search 何时能离开本机。"
          tone={networkEnabled ? "info" : "warning"}
          meta={networkEnabled ? "策略启用" : "策略关闭"}
        >
          <StatusChip
            label={`默认 ${config.system?.network_policy?.default_decision ?? "ask"}`}
            tone="info"
          />
        </CapabilityCard>
        <CapabilityCard
          title="File Access"
          description="Agent 只能读取文件访问范围内的路径。"
          tone={safePathCount > 0 ? "ready" : "warning"}
          meta={`${safePathCount} 条路径`}
        />
        <CapabilityCard
          title="Tool Permissions"
          description="已授予、拒绝或需要每次确认的工具权限。"
          tone={toolPermissions.length ? "ready" : "neutral"}
          meta={`${grantedCount} 已授予`}
        />
        <CapabilityCard
          title="MCP / A2A Tools"
          description="高级连接暴露出的工具会先进入治理清单。"
          tone={executableManifestCount > 0 ? "ready" : "neutral"}
          meta={`${executableManifestCount} 可执行`}
        />
      </section>

      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium text-gray-700">Web 与网络权限</h3>
            <p className="mt-1 text-xs text-gray-500">
              未知域名默认应进入确认或阻断；Chat 和 Activity 会解释具体 blocker。
            </p>
          </div>
          <label className="flex items-center gap-2 text-sm text-gray-700">
            <input
              type="checkbox"
              checked={networkEnabled}
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
          <label className="mb-1 block text-xs text-gray-500">默认决策</label>
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

        <DomainList
          title="域名白名单"
          inputId="new-allow-domain"
          domains={config.system?.network_policy?.domain_allowlist ?? []}
          onAdd={domain =>
            setConfig(prev => ({
              ...prev,
              system: {
                ...prev.system,
                network_policy: {
                  ...prev.system?.network_policy,
                  domain_allowlist: [
                    ...(prev.system?.network_policy?.domain_allowlist ?? []),
                    domain,
                  ],
                },
              },
            }))
          }
          onRemove={index =>
            setConfig(prev => ({
              ...prev,
              system: {
                ...prev.system,
                network_policy: {
                  ...prev.system?.network_policy,
                  domain_allowlist: (prev.system?.network_policy?.domain_allowlist ?? []).filter(
                    (_, i) => i !== index
                  ),
                },
              },
            }))
          }
        />

        <DomainList
          title="域名黑名单"
          inputId="new-deny-domain"
          domains={config.system?.network_policy?.domain_denylist ?? []}
          onAdd={domain =>
            setConfig(prev => ({
              ...prev,
              system: {
                ...prev.system,
                network_policy: {
                  ...prev.system?.network_policy,
                  domain_denylist: [
                    ...(prev.system?.network_policy?.domain_denylist ?? []),
                    domain,
                  ],
                },
              },
            }))
          }
          onRemove={index =>
            setConfig(prev => ({
              ...prev,
              system: {
                ...prev.system,
                network_policy: {
                  ...prev.system?.network_policy,
                  domain_denylist: (prev.system?.network_policy?.domain_denylist ?? []).filter(
                    (_, i) => i !== index
                  ),
                },
              },
            }))
          }
        />
      </section>

      <section className="space-y-4 border-t pt-4">
        <div>
          <h3 className="text-sm font-medium text-gray-700">文件访问</h3>
          <p className="mt-1 text-xs text-gray-500">
            这些路径定义 Agent 可读取的本地文件范围；不在范围内的文件读取会进入 blocker。
          </p>
        </div>
        <div className="space-y-2">
          {(config.system?.safe_paths ?? []).map((path, idx) => (
            <div key={path} className="flex items-center gap-2">
              <input
                type="text"
                value={path}
                readOnly
                className="flex-1 rounded border px-3 py-1.5 text-sm bg-gray-50"
              />
              <button
                onClick={() =>
                  setConfig(prev => ({
                    ...prev,
                    system: {
                      ...prev.system,
                      safe_paths: (prev.system?.safe_paths ?? []).filter((_, i) => i !== idx),
                    },
                  }))
                }
                className="rounded px-2 py-1 text-sm text-red-600 hover:bg-red-50"
              >
                删除
              </button>
            </div>
          ))}
          <div className="flex items-center gap-2">
            <input
              type="text"
              placeholder="添加绝对路径..."
              id="new-safe-path"
              className="flex-1 rounded border px-3 py-1.5 text-sm"
            />
            <button
              onClick={() => {
                const input = document.getElementById("new-safe-path") as HTMLInputElement;
                const path = input.value.trim();
                if (!path) {
                  alert("路径不能为空");
                  return;
                }
                const isAbsolute = path.startsWith("/") || /^[A-Za-z]:[\\\/]/.test(path);
                if (!isAbsolute) {
                  alert("路径必须是绝对路径。");
                  return;
                }
                const existing = config.system?.safe_paths ?? [];
                if (existing.includes(path)) {
                  alert("路径已存在");
                  return;
                }
                setConfig(prev => ({
                  ...prev,
                  system: {
                    ...prev.system,
                    safe_paths: [...(prev.system?.safe_paths ?? []), path],
                  },
                }));
                input.value = "";
              }}
              className="rounded bg-stone-900 px-3 py-1.5 text-sm text-white hover:bg-stone-800"
            >
              添加
            </button>
          </div>
        </div>
      </section>

      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium text-gray-700">工具权限与确认</h3>
            <p className="mt-1 text-xs text-gray-500">
              高风险工具和写操作默认进入确认流；这里展示已经授予或拒绝的工具权限。
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

      <section className="space-y-4 border-t pt-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-gray-700">工具能力清单（高级）</h3>
            <p className="mt-1 text-xs text-gray-500">
              面向诊断的注册表视图；普通用户只需要关注上方权限是否已授予。
            </p>
          </div>
          <button
            onClick={refreshSecurityState}
            className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-50"
          >
            刷新
          </button>
        </div>
        <div className="max-h-64 space-y-2 overflow-auto">
          {toolManifests.length === 0 ? (
            <div className="rounded-lg border border-dashed border-gray-200 bg-gray-50 px-3 py-3 text-xs text-gray-500">
              暂无工具注册
            </div>
          ) : (
            toolManifests.map(manifest => {
              const isDeclarative = manifest.declarative_only;
              const isDisabled = !manifest.enabled;
              const isReal = !isDeclarative && !isDisabled;
              return (
                <div
                  key={manifest.id}
                  className={classNames(
                    "flex items-center justify-between gap-3 rounded-lg border px-3 py-2 text-xs",
                    isReal
                      ? "border-green-200 bg-green-50"
                      : isDeclarative
                        ? "border-amber-200 bg-amber-50"
                        : "border-red-200 bg-red-50"
                  )}
                >
                  <div className="min-w-0">
                    <div className="font-medium text-gray-900">{manifest.name}</div>
                    <div className="mt-0.5 text-gray-500">
                      {manifest.risk_level} · {manifest.action_type || "—"}
                    </div>
                    <div className="mt-0.5 text-gray-400">
                      {manifest.capabilities.join(", ") || "none"}
                    </div>
                  </div>
                  <span className="shrink-0 rounded-full bg-white/70 px-2 py-0.5 text-[10px] font-medium text-gray-700">
                    {isReal ? "可执行" : isDeclarative ? "声明-only" : "禁用"}
                  </span>
                </div>
              );
            })
          )}
        </div>
      </section>
      <div className="rounded-lg border border-stone-200 bg-stone-50 px-3 py-2 text-xs text-stone-600">
        当前 MCP server {diagnostics?.mcp_server_count ?? 0} 个，工具{" "}
        {diagnostics?.mcp_tool_count ?? 0} 个。
      </div>
    </>
  );
}

function DomainList({
  title,
  inputId,
  domains,
  onAdd,
  onRemove,
}: {
  title: string;
  inputId: string;
  domains: string[];
  onAdd: (domain: string) => void;
  onRemove: (index: number) => void;
}) {
  return (
    <div>
      <h4 className="mb-2 text-sm font-medium text-gray-900">{title}</h4>
      <div className="space-y-2">
        {domains.map((domain, idx) => (
          <div key={`${domain}-${idx}`} className="flex items-center gap-2">
            <input
              type="text"
              value={domain}
              readOnly
              className="flex-1 rounded border px-3 py-1.5 text-sm bg-gray-50"
            />
            <button
              onClick={() => onRemove(idx)}
              className="rounded px-2 py-1 text-sm text-red-600 hover:bg-red-50"
            >
              删除
            </button>
          </div>
        ))}
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="添加域名..."
            id={inputId}
            className="flex-1 rounded border px-3 py-1.5 text-sm"
          />
          <button
            onClick={() => {
              const input = document.getElementById(inputId) as HTMLInputElement;
              const domain = input.value.trim();
              if (!domain) {
                alert("域名不能为空");
                return;
              }
              if (domains.includes(domain)) {
                alert("域名已存在");
                return;
              }
              onAdd(domain);
              input.value = "";
            }}
            className="rounded bg-stone-900 px-3 py-1.5 text-sm text-white hover:bg-stone-800"
          >
            添加
          </button>
        </div>
      </div>
    </div>
  );
}
