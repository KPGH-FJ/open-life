import { useEffect, useState } from "react";
import type {
  AppConfig,
  HotMemoryCache,
  PrivacyPolicy,
  ProviderTransmissionHistoryItem,
  ProviderTransmissionStatus,
  SystemDiagnostics,
  ToolManifest,
  ToolPermissionRecord,
} from "../../../tauri";
import { listProviderTransmissionHistory } from "../../../tauri";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

const SECRET_LIKE_RE =
  /(^|[^a-z0-9])sk-|bearer\s|authorization:|api_key=|api key:|apikey=|token=|secret=|password=/i;

function safeTransmissionText(value?: string | null): string {
  const text = value?.trim();
  if (!text) return "-";
  if (SECRET_LIKE_RE.test(text)) return "redacted_sensitive";
  return text.length > 96 ? `${text.slice(0, 93)}...` : text;
}

function transmissionStatusView(status: ProviderTransmissionStatus): {
  label: string;
  className: string;
} {
  if (status === "sent") {
    return {
      label: "sent · 已外发",
      className: "border-amber-200 bg-amber-50 text-amber-800",
    };
  }
  if (status === "not_sent") {
    return {
      label: "not_sent · 未外发",
      className: "border-emerald-200 bg-emerald-50 text-emerald-800",
    };
  }
  if (status === "blocked") {
    return {
      label: "blocked · 已阻断",
      className: "border-red-200 bg-red-50 text-red-800",
    };
  }
  if (status === "not_instrumented") {
    return {
      label: "not_instrumented · 旧 run 未接入",
      className: "border-slate-200 bg-slate-50 text-slate-700",
    };
  }
  return {
    label: "unknown · 证据不足",
    className: "border-slate-200 bg-white text-slate-700",
  };
}

function sourceRefSummary(item: ProviderTransmissionHistoryItem): string {
  if (!item.source_refs.length) return "-";
  return item.source_refs
    .slice(0, 3)
    .map(ref => {
      const source = safeTransmissionText(ref.source);
      const status = ref.status ? `:${safeTransmissionText(ref.status)}` : "";
      return `${source}${status}`;
    })
    .join(" / ");
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
  const [transmissionHistory, setTransmissionHistory] = useState<
    ProviderTransmissionHistoryItem[]
  >([]);
  const [transmissionLoading, setTransmissionLoading] = useState(false);
  const [transmissionError, setTransmissionError] = useState<string | null>(null);

  const refreshProviderTransmissionHistory = async () => {
    setTransmissionLoading(true);
    setTransmissionError(null);
    try {
      const history = await listProviderTransmissionHistory(20);
      setTransmissionHistory(Array.isArray(history) ? history : []);
    } catch (error) {
      setTransmissionHistory([]);
      setTransmissionError(error instanceof Error ? error.message : String(error));
    } finally {
      setTransmissionLoading(false);
    }
  };

  useEffect(() => {
    void refreshProviderTransmissionHistory();
  }, []);

  const handleRefresh = async () => {
    await Promise.all([refreshSecurityState(), refreshProviderTransmissionHistory()]);
  };

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
            onClick={handleRefresh}
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
              <div className="text-sm font-semibold text-slate-800">最近 provider 外发历史</div>
              <div className="mt-1 text-xs text-slate-500">
                来源：AgentRun route evidence。配置态不会当作 sent。
              </div>
            </div>
            <div className="text-xs text-slate-500">
              {transmissionLoading ? "读取中..." : `${transmissionHistory.length} 条`}
            </div>
          </div>

          {transmissionError && (
            <div className="mt-3 rounded-md border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-700">
              {safeTransmissionText(transmissionError)}
            </div>
          )}

          {!transmissionLoading && transmissionHistory.length === 0 ? (
            <div className="mt-3 rounded-lg border border-dashed border-slate-200 bg-slate-50 px-3 py-4 text-xs text-slate-500">
              暂无 provider transmission history；这不等于未外发，旧 run 可能未接入
              not_instrumented 证据。
            </div>
          ) : (
            <div className="mt-3 space-y-2">
              {transmissionHistory.map(item => {
                const status = transmissionStatusView(item.status);
                return (
                  <div
                    key={`${item.run_id}:${item.evidence_id}`}
                    className="rounded-lg border border-slate-100 bg-slate-50 px-3 py-3 text-xs text-slate-700"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span
                        className={classNames(
                          "rounded-full border px-2 py-0.5 font-semibold",
                          status.className
                        )}
                      >
                        {status.label}
                      </span>
                      <span className="text-slate-500">
                        {safeTransmissionText(item.started_at)}
                      </span>
                    </div>
                    <div className="mt-2 grid gap-1 md:grid-cols-2">
                      <div>run_id：{safeTransmissionText(item.run_id)}</div>
                      <div>task_session_id：{safeTransmissionText(item.task_session_id)}</div>
                      <div>provider：{safeTransmissionText(item.provider)}</div>
                      <div>model：{safeTransmissionText(item.model)}</div>
                      <div>route_type：{safeTransmissionText(item.route_type)}</div>
                      <div>truth_confidence：{safeTransmissionText(item.truth_confidence)}</div>
                      <div>evidence_id：{safeTransmissionText(item.evidence_id)}</div>
                      <div>data_category：{safeTransmissionText(item.data_category)}</div>
                    </div>
                    <div className="mt-2 text-slate-600">
                      reason：{safeTransmissionText(item.reason)}
                    </div>
                    <div className="mt-1 text-slate-500">
                      source_refs：{sourceRefSummary(item)}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
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
