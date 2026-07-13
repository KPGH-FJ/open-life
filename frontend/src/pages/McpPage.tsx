import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Wrench,
  Plus,
  Trash2,
  Server,
  RefreshCw,
  LayoutTemplate,
  ChevronRight,
  CheckCircle,
  Sparkles,
  Shield,
  Activity,
  Eye,
  Lock,
  FileText,
  AlertTriangle,
  Check,
  X,
  ChevronDown,
  ChevronUp,
  Info,
} from "lucide-react";
import {
  registerMcpServer,
  unregisterMcpServer,
  listMcpServers,
  listMcpTools,
  listMcpTemplates,
  recommendMcpManifests,
  listMcpAuditLogs,
  getPrivacyPolicy,
  getRuntimeBuildInfo,
  type McpServerInfo,
  type McpTemplate,
  type ToolManifest,
  type McpAuditLogEntry,
  type PrivacyRule,
  type RuntimeBuildInfo,
} from "../tauri";
import EmptyState from "../components/EmptyState";
import ErrorBanner from "../components/ErrorBanner";

type WizardStep = "select" | "preview" | "done";

function resolvePlaceholders(arr: string[], inputs: Record<string, string>): string[] {
  return arr.map(s => s.replace(/\{\{(\w+)\}\}/g, (_, key) => inputs[key] ?? `{{${key}}}`));
}

function resolveEnv(
  env: Record<string, string> | undefined,
  inputs: Record<string, string>
): Record<string, string> {
  if (!env) return {};
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(env)) {
    out[k] = v.replace(/\{\{(\w+)\}\}/g, (_, key) => inputs[key] ?? `{{${key}}}`);
  }
  return out;
}

function parseTypedMcpManifests(value: string, serverName: string): ToolManifest[] {
  const parsed: unknown = JSON.parse(value);
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error("至少需要一个 typed manifest contract");
  }
  for (const manifest of parsed) {
    if (typeof manifest !== "object" || manifest === null) {
      throw new Error("typed manifest 必须是对象");
    }
    const candidate = manifest as ToolManifest;
    const source = candidate.source;
    if (
      typeof candidate.id !== "string" ||
      !candidate.id.trim() ||
      typeof candidate.name !== "string" ||
      !candidate.name.trim() ||
      typeof candidate.parameters !== "object" ||
      !Array.isArray(candidate.capabilities) ||
      candidate.capabilities.length === 0 ||
      candidate.capabilities.some(capability => !capability.trim()) ||
      source?.type !== "Mcp" ||
      source.server_name !== serverName ||
      !["read", "write", "network", "external_side_effect", "proposal_only_write"].includes(
        candidate.action_type
      ) ||
      !["low", "medium", "high", "critical"].includes(candidate.risk_level) ||
      !["low", "medium", "high", "critical"].includes(candidate.permission_level) ||
      candidate.enabled !== true ||
      candidate.declarative_only !== false ||
      candidate.idempotency_contract === "unspecified"
    ) {
      throw new Error("typed manifest 必须完整并绑定当前 MCP Server 名称");
    }
  }
  return parsed as ToolManifest[];
}

export default function McpPage() {
  const [runtimeBuildInfo, setRuntimeBuildInfo] = useState<RuntimeBuildInfo | null>(null);
  const [runtimeBuildInfoLoaded, setRuntimeBuildInfoLoaded] = useState(false);
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [tools, setTools] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [pageError, setPageError] = useState<string>("");

  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [argsText, setArgsText] = useState("");
  const [manifestsText, setManifestsText] = useState("");
  const [registering, setRegistering] = useState(false);

  const [templates, setTemplates] = useState<McpTemplate[]>([]);
  const [recommended, setRecommended] = useState<ToolManifest[]>([]);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardStep, setWizardStep] = useState<WizardStep>("select");
  const [selectedTemplate, setSelectedTemplate] = useState<McpTemplate | null>(null);
  const [templateInputs, setTemplateInputs] = useState<Record<string, string>>({});
  const [wizardRegistering, setWizardRegistering] = useState(false);
  const [auditLogs, setAuditLogs] = useState<McpAuditLogEntry[]>([]);
  const [privacyRules, setPrivacyRules] = useState<PrivacyRule[]>([]);
  const [expandedLog, setExpandedLog] = useState<number | null>(null);
  const arbitraryRegistrationEnabled =
    runtimeBuildInfo?.devExtensionsEnabled === true &&
    runtimeBuildInfo.arbitraryMcpRegistrationEnabled === true;
  const registrationStatus = !runtimeBuildInfoLoaded
    ? "checking"
    : arbitraryRegistrationEnabled
      ? "dev_only_enabled"
      : runtimeBuildInfo?.devExtensionsEnabled === false
        ? "disabled_by_build"
        : "unavailable";
  const typedTemplates = templates.filter(template => (template.manifests?.length ?? 0) > 0);

  const load = async () => {
    setLoading(true);
    setPageError("");
    try {
      const [s, t, tpls, logs, policy] = await Promise.all([
        listMcpServers(),
        listMcpTools(),
        listMcpTemplates(),
        listMcpAuditLogs(20),
        getPrivacyPolicy(),
      ]);
      setServers(s);
      setTools(t);
      setTemplates(tpls);
      setAuditLogs(logs);
      setPrivacyRules(policy.rules);
      const rec = await recommendMcpManifests(5);
      setRecommended(rec);
    } catch (e) {
      setPageError("加载失败: " + String(e));
    } finally {
      setLoading(false);
    }
  };

  const toggleLogExpand = (logId: number) => {
    setExpandedLog(prev => (prev === logId ? null : logId));
  };

  // Calculate audit stats
  const auditStats = {
    total: auditLogs.length,
    success: auditLogs.filter(l => l.success).length,
    failed: auditLogs.filter(l => !l.success).length,
    piiHits: auditLogs.filter(l => l.pii_found).length,
    uniqueTools: new Set(auditLogs.map(l => l.tool_name)).size,
  };

  useEffect(() => {
    load();
  }, []);

  useEffect(() => {
    let active = true;
    getRuntimeBuildInfo()
      .then(info => {
        if (active) setRuntimeBuildInfo(info);
      })
      .catch(() => {
        if (active) setRuntimeBuildInfo(null);
      })
      .finally(() => {
        if (active) setRuntimeBuildInfoLoaded(true);
      });
    return () => {
      active = false;
    };
  }, []);

  const handleRegister = async () => {
    if (!arbitraryRegistrationEnabled || !name.trim() || !command.trim() || !manifestsText.trim())
      return;
    const args = argsText.trim().split(/\s+/).filter(Boolean);
    setRegistering(true);
    setPageError("");
    try {
      const manifests = parseTypedMcpManifests(manifestsText, name.trim());
      await registerMcpServer(name.trim(), command.trim(), args, manifests);
      setName("");
      setCommand("");
      setArgsText("");
      setManifestsText("");
      await load();
    } catch (e) {
      setPageError("注册失败: " + String(e));
    } finally {
      setRegistering(false);
    }
  };

  const handleRemove = async (n: string) => {
    if (!arbitraryRegistrationEnabled) return;
    if (!confirm(`确定要删除 MCP Server "${n}" 吗？`)) return;
    setPageError("");
    try {
      await unregisterMcpServer(n);
      await load();
    } catch (e) {
      setPageError("删除失败: " + String(e));
    }
  };

  const openWizard = () => {
    if (!arbitraryRegistrationEnabled || !templates.some(template => template.manifests?.length))
      return;
    setWizardOpen(true);
    setWizardStep("select");
    setSelectedTemplate(null);
    setTemplateInputs({});
  };

  const closeWizard = () => {
    setWizardOpen(false);
    setWizardStep("select");
    setSelectedTemplate(null);
    setTemplateInputs({});
  };

  const selectTemplate = (tpl: McpTemplate) => {
    setSelectedTemplate(tpl);
    const initial: Record<string, string> = {};
    for (const key of tpl.required_args) {
      initial[key] = "";
    }
    setTemplateInputs(initial);
    setWizardStep("preview");
  };

  const installRecommended = (manifest: ToolManifest) => {
    if (!arbitraryRegistrationEnabled) return;
    const matched = templates.find(tpl => {
      if (!tpl.manifests?.length) return false;
      if (tpl.id === manifest.name) return true;
      const tags = tpl.tags ?? [];
      return manifest.tags.some(tag => tags.includes(tag));
    });
    if (matched) {
      openWizard();
      setSelectedTemplate(matched);
      const initial: Record<string, string> = {};
      for (const key of matched.required_args) initial[key] = "";
      setTemplateInputs(initial);
      setWizardStep("preview");
      return;
    }
    setPageError("当前推荐工具没有对应模板，可先手动注册。");
  };

  const hasTypedTemplateForManifest = (manifest: ToolManifest) =>
    typedTemplates.some(template => {
      if (template.id === manifest.name) return true;
      const tags = template.tags ?? [];
      return manifest.tags.some(tag => tags.includes(tag));
    });

  const previewArgs = selectedTemplate
    ? resolvePlaceholders(selectedTemplate.args, templateInputs)
    : [];
  const previewEnv = selectedTemplate ? resolveEnv(selectedTemplate.env, templateInputs) : {};
  const canRegisterTemplate =
    selectedTemplate !== null &&
    selectedTemplate.required_args.every(k => templateInputs[k]?.trim() !== "");

  const handleRegisterTemplate = async () => {
    if (!arbitraryRegistrationEnabled || !selectedTemplate?.manifests?.length) return;
    setWizardRegistering(true);
    setPageError("");
    try {
      const resolvedArgs = resolvePlaceholders(selectedTemplate.args, templateInputs);
      const env = resolveEnv(selectedTemplate.env, templateInputs);
      await registerMcpServer(
        selectedTemplate.id,
        selectedTemplate.command,
        resolvedArgs,
        selectedTemplate.manifests,
        env
      );
      setWizardStep("done");
      await load();
    } catch (e) {
      setPageError("注册失败: " + String(e));
    } finally {
      setWizardRegistering(false);
    }
  };

  return (
    <div className="h-full overflow-auto bg-white p-6">
      <div className="max-w-4xl mx-auto space-y-8">
        <ErrorBanner message={pageError} onClose={() => setPageError("")} />
        <h2 className="text-xl font-bold text-gray-900 flex items-center gap-2">
          <Server className="text-indigo-600" size={22} />
          MCP 管理
        </h2>

        <section
          className={`rounded-xl border p-4 ${arbitraryRegistrationEnabled ? "border-indigo-100 bg-indigo-50/70" : "border-amber-200 bg-amber-50"}`}
        >
          <div className="flex items-start gap-3">
            <Lock
              className={`mt-0.5 shrink-0 ${arbitraryRegistrationEnabled ? "text-indigo-700" : "text-amber-700"}`}
              size={16}
              aria-hidden="true"
            />
            <div>
              <div className="text-sm font-semibold text-gray-900">任意 MCP 注册能力</div>
              <div className="mt-1 text-sm text-gray-700">
                {arbitraryRegistrationEnabled
                  ? "仅当前开发构建允许注册和移除本地 MCP Server。"
                  : "当前构建只展示后端可读取的 Server、工具和审计事实，不开放注册或移除入口。"}
              </div>
              <div className="mt-2 font-mono text-xs font-semibold text-gray-700">
                {registrationStatus}
              </div>
            </div>
          </div>
        </section>

        <section className="rounded-xl border border-indigo-100 bg-indigo-50/70 p-5">
          <div className="flex items-start gap-3">
            <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white text-indigo-700">
              <Shield size={16} aria-hidden="true" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-indigo-950">Chat 外部能力如何生效</h3>
              <p className="mt-1 text-sm leading-6 text-indigo-900">
                {arbitraryRegistrationEnabled
                  ? "开发构建可在这里接入 MCP；Chat 每次调用仍会经过 allowlist、隐私规则和必要确认。"
                  : "这里仅展示后端已报告的 MCP 只读事实。当前构建不能从此页接入新工具，推荐项也不代表能力已经可用。"}
              </p>
              <div className="mt-3 flex flex-wrap gap-2 text-xs">
                <span className="rounded-md bg-white px-2.5 py-1 font-medium text-indigo-800">
                  已注册 Server：{servers.length}
                </span>
                <span className="rounded-md bg-white px-2.5 py-1 font-medium text-indigo-800">
                  当前工具：{tools.length}
                </span>
                <span className="rounded-md bg-white px-2.5 py-1 font-medium text-indigo-800">
                  隐私规则：{privacyRules.length}
                </span>
              </div>
            </div>
          </div>
        </section>

        {arbitraryRegistrationEnabled && (
          <section className="space-y-4 border rounded-xl p-5 bg-gray-50">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
                <Plus size={16} /> 注册新 MCP Server
              </h3>
              <button
                onClick={openWizard}
                disabled={typedTemplates.length === 0}
                className="inline-flex items-center gap-2 text-sm bg-white border border-gray-200 px-3 py-1.5 rounded-lg hover:bg-gray-100 disabled:cursor-not-allowed disabled:opacity-50"
              >
                <LayoutTemplate size={14} />
                {typedTemplates.length > 0 ? "安装 typed 模板" : "暂无 typed 模板"}
              </button>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div className="space-y-1">
                <label className="text-xs text-gray-500">名称</label>
                <input
                  className="w-full border rounded-md px-3 py-2 text-sm"
                  placeholder="例如: filesystem"
                  value={name}
                  onChange={e => setName(e.target.value)}
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs text-gray-500">启动命令</label>
                <input
                  className="w-full border rounded-md px-3 py-2 text-sm"
                  placeholder="例如: npx"
                  value={command}
                  onChange={e => setCommand(e.target.value)}
                />
              </div>
            </div>
            <div className="space-y-1">
              <label className="text-xs text-gray-500">参数（空格分隔）</label>
              <input
                className="w-full border rounded-md px-3 py-2 text-sm"
                placeholder="例如: -y @modelcontextprotocol/server-filesystem /tmp"
                value={argsText}
                onChange={e => setArgsText(e.target.value)}
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-gray-500">
                Typed manifests JSON（必须覆盖服务发现的全部工具）
              </label>
              <textarea
                className="min-h-36 w-full rounded-md border px-3 py-2 font-mono text-xs"
                placeholder='[{"id":"mcp:server:tool","name":"tool",...}]'
                value={manifestsText}
                onChange={event => setManifestsText(event.target.value)}
              />
              <p className="text-xs leading-5 text-gray-500">
                OpenLife 不会根据工具名称猜测权限、风险、能力或幂等性；后端会把这里的契约与真实
                tools/list 结果逐项核对。
              </p>
            </div>
            <div className="flex gap-3">
              <button
                onClick={handleRegister}
                disabled={registering || !name.trim() || !command.trim() || !manifestsText.trim()}
                className="inline-flex items-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-indigo-700 disabled:opacity-50"
              >
                {registering ? "注册中..." : "注册"}
              </button>
            </div>
          </section>
        )}

        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
              <Sparkles size={16} /> 推荐工具
            </h3>
            <div className="text-xs text-gray-500">
              {arbitraryRegistrationEnabled
                ? "根据当前目标与能力缺口自动推荐"
                : "候选清单，仅供查看"}
            </div>
          </div>
          {recommended.length === 0 ? (
            <EmptyState
              title="暂无推荐"
              description="先补充目标和能力数据后，这里会给出更贴近当前阶段的工具建议。"
              className="py-4"
            />
          ) : (
            <div className="grid grid-cols-1 gap-3">
              {recommended.map(manifest => (
                <div
                  key={manifest.name}
                  className="border rounded-lg p-4 bg-white flex items-start justify-between gap-4"
                >
                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <div className="font-medium text-gray-900">{manifest.name}</div>
                      <span
                        className={`text-[10px] px-2 py-0.5 rounded-full ${manifest.permission_level === "high" ? "bg-rose-100 text-rose-700" : manifest.permission_level === "medium" ? "bg-amber-100 text-amber-700" : "bg-emerald-100 text-emerald-700"}`}
                      >
                        {manifest.permission_level}
                      </span>
                      <span className="text-[10px] px-2 py-0.5 rounded-full bg-slate-100 text-slate-600">
                        {manifest.source.type === "Mcp" ? "MCP" : "Built-in"}
                      </span>
                    </div>
                    <div className="text-sm text-gray-600">{manifest.description}</div>
                    {manifest.tags.length > 0 && (
                      <div className="flex flex-wrap gap-2">
                        {manifest.tags.map(tag => (
                          <span
                            key={tag}
                            className="rounded-full border border-gray-200 bg-gray-50 px-2 py-0.5 text-[10px] text-gray-600"
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                  {manifest.source.type === "BuiltIn" ? (
                    <div className="inline-flex items-center gap-1 text-xs text-emerald-700 bg-emerald-50 px-3 py-2 rounded-lg">
                      <Shield size={14} /> 已内置
                    </div>
                  ) : arbitraryRegistrationEnabled && hasTypedTemplateForManifest(manifest) ? (
                    <button
                      onClick={() => installRecommended(manifest)}
                      className="shrink-0 inline-flex items-center gap-2 bg-indigo-600 text-white px-3 py-2 rounded-lg text-sm hover:bg-indigo-700"
                    >
                      安装 typed 模板
                    </button>
                  ) : (
                    <div className="shrink-0 rounded-lg bg-gray-100 px-3 py-2 text-xs font-medium text-gray-600">
                      无可执行 typed 契约
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>

        {/* Safety & Transparency Section */}
        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
              <Shield size={16} className="text-indigo-500" />
              安全审计中心
            </h3>
            <div className="flex items-center gap-2">
              <Link
                to="/settings"
                className="text-xs border border-gray-200 rounded-lg px-3 py-1.5 hover:bg-gray-50"
              >
                在隐私设置中管理审计保留
              </Link>
            </div>
          </div>

          {/* Stats Cards */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <div className="rounded-lg border border-gray-200 bg-white p-3">
              <div className="flex items-center gap-1.5 text-xs text-gray-500">
                <Activity size={12} />
                总调用次数
              </div>
              <div className="mt-1 text-xl font-semibold text-gray-900">{auditStats.total}</div>
            </div>
            <div className="rounded-lg border border-emerald-100 bg-emerald-50 p-3">
              <div className="flex items-center gap-1.5 text-xs text-emerald-600">
                <Check size={12} />
                成功
              </div>
              <div className="mt-1 text-xl font-semibold text-emerald-700">
                {auditStats.success}
              </div>
            </div>
            <div className="rounded-lg border border-rose-100 bg-rose-50 p-3">
              <div className="flex items-center gap-1.5 text-xs text-rose-600">
                <X size={12} />
                失败
              </div>
              <div className="mt-1 text-xl font-semibold text-rose-700">{auditStats.failed}</div>
            </div>
            <div className="rounded-lg border border-amber-100 bg-amber-50 p-3">
              <div className="flex items-center gap-1.5 text-xs text-amber-600">
                <AlertTriangle size={12} />
                PII 拦截
              </div>
              <div className="mt-1 text-xl font-semibold text-amber-700">{auditStats.piiHits}</div>
            </div>
          </div>

          {/* Privacy Rules */}
          <div className="rounded-lg border border-indigo-100 bg-indigo-50/50 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-indigo-700 mb-3">
              <Lock size={16} />
              隐私保护规则
            </div>
            <div className="space-y-2">
              {privacyRules.length === 0 ? (
                <div className="text-xs text-gray-500">暂无隐私规则配置</div>
              ) : (
                privacyRules.map(rule => (
                  <div
                    key={rule.ptype}
                    className={`flex items-center justify-between rounded-lg border px-3 py-2 text-xs ${
                      rule.enabled
                        ? "bg-white border-gray-200"
                        : "bg-gray-50 border-gray-100 opacity-60"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      {rule.enabled ? (
                        <Shield size={14} className="text-emerald-500" />
                      ) : (
                        <Eye size={14} className="text-gray-400" />
                      )}
                      <span className="font-medium text-gray-700">{rule.ptype}</span>
                      {rule.custom_pattern && (
                        <span className="text-[10px] text-gray-400">({rule.custom_pattern})</span>
                      )}
                    </div>
                    <span
                      className={`text-[10px] px-2 py-0.5 rounded-full ${
                        rule.action === "Block"
                          ? "bg-rose-100 text-rose-700"
                          : rule.action === "Mask"
                            ? "bg-amber-100 text-amber-700"
                            : "bg-slate-100 text-slate-600"
                      }`}
                    >
                      {rule.action === "Block" ? "阻止" : rule.action === "Mask" ? "脱敏" : "允许"}
                    </span>
                  </div>
                ))
              )}
            </div>
            <div className="mt-3 text-[11px] text-indigo-600/80">
              <Info size={12} className="inline mr-1" />
              所有敏感数据在发送到外部 MCP 服务器前都会经过上述规则检查。
            </div>
          </div>

          {/* Audit Logs */}
          {auditLogs.length === 0 ? (
            <EmptyState
              title="暂无审计记录"
              description="一旦触发 MCP 工具调用，这里会显示最近的执行记录和隐私命中情况。"
              className="py-4"
            />
          ) : (
            <div className="space-y-3">
              {auditLogs.map(log => {
                const isExpanded = expandedLog === log.id;
                return (
                  <div
                    key={log.id}
                    className="rounded-lg border border-gray-200 bg-white overflow-hidden"
                  >
                    {/* Header - always visible */}
                    <div
                      onClick={() => toggleLogExpand(log.id)}
                      className="p-4 cursor-pointer hover:bg-gray-50 transition"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div className="flex items-center gap-3">
                          <div className="font-medium text-gray-900">{log.tool_name}</div>
                          <span className="text-xs text-gray-400">
                            {new Date(log.created_at).toLocaleString("zh-CN")}
                          </span>
                        </div>
                        <div className="flex items-center gap-2">
                          <span
                            className={`text-[10px] px-2 py-0.5 rounded-full ${
                              log.success
                                ? "bg-emerald-100 text-emerald-700"
                                : "bg-rose-100 text-rose-700"
                            }`}
                          >
                            {log.success ? "成功" : "失败"}
                          </span>
                          {log.pii_found && (
                            <span className="text-[10px] px-2 py-0.5 rounded-full bg-amber-100 text-amber-700">
                              敏感数据已脱敏
                            </span>
                          )}
                          {isExpanded ? (
                            <ChevronUp size={16} className="text-gray-400" />
                          ) : (
                            <ChevronDown size={16} className="text-gray-400" />
                          )}
                        </div>
                      </div>
                      {/* Preview line */}
                      <div className="mt-2 text-xs text-gray-500 truncate">
                        <span className="text-gray-400">参数预览：</span>
                        {log.arguments.substring(0, 100)}
                        {log.arguments.length > 100 && "..."}
                      </div>
                    </div>

                    {/* Expanded details */}
                    {isExpanded && (
                      <div className="border-t border-gray-100 px-4 py-4 bg-gray-50/50">
                        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                          <div className="space-y-2">
                            <div className="flex items-center gap-2 text-xs font-medium text-gray-600">
                              <FileText size={14} />
                              请求参数
                            </div>
                            <div className="rounded-md bg-slate-50 border border-slate-100 p-3">
                              <pre className="text-xs text-gray-700 whitespace-pre-wrap break-all max-h-40 overflow-auto">
                                {log.arguments}
                              </pre>
                            </div>
                          </div>
                          <div className="space-y-2">
                            <div className="flex items-center gap-2 text-xs font-medium text-gray-600">
                              <CheckCircle size={14} />
                              执行结果
                            </div>
                            <div className="rounded-md bg-slate-50 border border-slate-100 p-3">
                              <pre className="text-xs text-gray-700 whitespace-pre-wrap break-all max-h-40 overflow-auto">
                                {log.result}
                              </pre>
                            </div>
                          </div>
                        </div>
                        <div className="mt-3 flex items-center gap-4 text-[11px] text-gray-400">
                          <span>调用 ID: {log.id}</span>
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
              <Server size={16} /> 已注册服务器
            </h3>
            <button
              onClick={load}
              disabled={loading}
              className="inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-700 disabled:opacity-50"
            >
              <RefreshCw size={14} /> 刷新
            </button>
          </div>
          {servers.length === 0 ? (
            <EmptyState
              title="暂无注册的 MCP Server"
              description={
                arbitraryRegistrationEnabled
                  ? "点击上方按钮注册服务器或使用模板安装。"
                  : "当前构建未报告可读取的 MCP Server。"
              }
              className="py-4"
            />
          ) : (
            <div className="grid grid-cols-1 gap-3">
              {servers.map(s => (
                <div
                  key={s.name}
                  className="border rounded-lg p-4 bg-white flex items-center justify-between"
                >
                  <div>
                    <div className="font-medium text-gray-900">{s.name}</div>
                    <div className="text-xs text-gray-500 font-mono mt-1">
                      {s.command} {s.args.join(" ")}
                    </div>
                    <div className="text-xs text-gray-400 mt-1">工具数量: {s.tool_count}</div>
                  </div>
                  {arbitraryRegistrationEnabled ? (
                    <button
                      onClick={() => handleRemove(s.name)}
                      className="text-red-500 hover:text-red-700 p-2"
                      title="删除"
                    >
                      <Trash2 size={18} />
                    </button>
                  ) : (
                    <span className="rounded bg-gray-100 px-2 py-1 text-xs text-gray-500">
                      只读
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="space-y-4">
          <h3 className="text-sm font-semibold text-gray-700 flex items-center gap-2">
            <Wrench size={16} /> 可用工具列表
          </h3>
          {tools.length === 0 ? (
            <EmptyState
              title="暂无可用工具"
              description="注册 MCP Server 后将显示可用工具列表。"
              className="py-4"
            />
          ) : (
            <div className="grid grid-cols-1 gap-3">
              {tools.map((t, idx) => (
                <div key={idx} className="border rounded-lg p-4 bg-white">
                  <div className="font-medium text-gray-900">{t.name}</div>
                  <div className="text-sm text-gray-600 mt-1">{t.description}</div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>

      {arbitraryRegistrationEnabled && wizardOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <div className="w-full max-w-lg bg-white rounded-xl shadow-xl p-6 space-y-5">
            <div className="flex items-center justify-between">
              <h3 className="text-base font-semibold text-gray-900 flex items-center gap-2">
                <LayoutTemplate size={18} className="text-indigo-600" />
                {wizardStep === "select" && "选择模板"}
                {wizardStep === "preview" && "预览参数"}
                {wizardStep === "done" && "安装完成"}
              </h3>
              <button onClick={closeWizard} className="text-gray-400 hover:text-gray-600 text-sm">
                关闭
              </button>
            </div>

            {wizardStep === "select" && (
              <div className="space-y-3 max-h-96 overflow-auto pr-1">
                {typedTemplates.map(tpl => (
                  <button
                    key={tpl.id}
                    onClick={() => selectTemplate(tpl)}
                    className="w-full text-left border rounded-lg p-4 hover:border-indigo-500 hover:bg-indigo-50 transition"
                  >
                    <div className="flex items-center justify-between">
                      <div className="font-medium text-gray-900">{tpl.name}</div>
                      <ChevronRight size={16} className="text-gray-400" />
                    </div>
                    <div className="text-sm text-gray-600 mt-1">{tpl.description}</div>
                    <div className="text-xs text-gray-400 mt-2 font-mono">
                      {tpl.command} {tpl.args.join(" ")}
                    </div>
                  </button>
                ))}
              </div>
            )}

            {wizardStep === "preview" && selectedTemplate && (
              <div className="space-y-4">
                <div className="text-sm text-gray-700">
                  <span className="font-medium">{selectedTemplate.name}</span>
                  <span className="text-gray-500"> — {selectedTemplate.description}</span>
                </div>

                {selectedTemplate.required_args.length > 0 ? (
                  <div className="space-y-3">
                    {selectedTemplate.required_args.map(key => (
                      <div key={key} className="space-y-1">
                        <label className="text-xs text-gray-500">
                          {selectedTemplate.arg_labels?.[key] || key}
                          <span className="text-red-500 ml-0.5">*</span>
                        </label>
                        <input
                          type="text"
                          className="w-full border rounded-md px-3 py-2 text-sm"
                          value={templateInputs[key] || ""}
                          onChange={e =>
                            setTemplateInputs(prev => ({ ...prev, [key]: e.target.value }))
                          }
                          placeholder={`输入 ${selectedTemplate.arg_labels?.[key] || key}`}
                        />
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-sm text-gray-500">该模板无需额外参数。</div>
                )}

                <div className="bg-gray-50 border rounded-lg p-3 space-y-2">
                  <div className="text-xs font-medium text-gray-600">预览</div>
                  <div className="text-xs font-mono text-gray-700">
                    {selectedTemplate.command} {previewArgs.join(" ")}
                  </div>
                  {Object.keys(previewEnv).length > 0 && (
                    <div className="text-xs font-mono text-gray-500">
                      env: {JSON.stringify(previewEnv)}
                    </div>
                  )}
                </div>

                <div className="flex gap-3 pt-1">
                  <button
                    onClick={() => setWizardStep("select")}
                    className="px-4 py-2 rounded-lg text-sm border hover:bg-gray-50"
                  >
                    上一步
                  </button>
                  <button
                    onClick={handleRegisterTemplate}
                    disabled={!canRegisterTemplate || wizardRegistering}
                    className="flex-1 inline-flex items-center justify-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-indigo-700 disabled:opacity-50"
                  >
                    {wizardRegistering ? "注册中..." : "确认注册"}
                  </button>
                </div>
              </div>
            )}

            {wizardStep === "done" && (
              <div className="space-y-4 text-center py-4">
                <CheckCircle size={48} className="text-green-500 mx-auto" />
                <div className="text-base font-medium text-gray-900">模板安装成功</div>
                <div className="text-sm text-gray-500">
                  {selectedTemplate?.name} 已注册为 MCP Server，可在工具列表中查看。
                </div>
                <button
                  onClick={closeWizard}
                  className="inline-flex items-center gap-2 bg-indigo-600 text-white px-5 py-2 rounded-lg text-sm font-medium hover:bg-indigo-700"
                >
                  完成
                </button>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
