import { useState } from "react";
import { Link } from "react-router-dom";
import {
  recoverRequiredCredentialAccess,
  runMemoryTierMaintenance,
  type CredentialRecoveryItem,
  type CredentialRecoveryReport,
  type LifeStateProjection,
  type ProviderPrivacyBoundarySummary,
  type SystemDiagnostics,
} from "../../../tauri";
import { buildSafeModeBlockedMessage } from "../../../utils/runtimeMessages";
import {
  advancedRoutePath,
  productRoutePath,
  secondaryRoutePath,
} from "../../../productShellContract";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

function readableError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as any).message === "string") return (e as any).message;
    if ("error" in e && typeof (e as any).error === "string") return (e as any).error;
  }
  return String(e);
}

const CREDENTIAL_PURPOSE_LABELS: Record<CredentialRecoveryItem["purpose"], string> = {
  agent_run_receipts: "Agent 运行回执",
  main_chat_events: "主聊天事件",
  action_queue: "动作队列",
  task_store: "任务存储",
};

const CREDENTIAL_STATUS_LABELS: Record<CredentialRecoveryItem["status"], string> = {
  available: "可访问",
  created: "已安全初始化",
  missing_existing_data: "已有数据但密钥缺失",
  invalid: "密钥格式无效",
  unavailable: "系统凭据库不可用",
};

interface OverviewTabProps {
  diagnostics: SystemDiagnostics | null;
  providerPrivacyBoundary?: ProviderPrivacyBoundarySummary | null;
  projection?: LifeStateProjection | null;
  safeMode: boolean;
  exportLoading: boolean;
  handleExport: () => Promise<void>;
  refreshAllDiagnostics: () => Promise<LifeStateProjection | null>;
  tierLoading: boolean;
  setTierLoading: (v: boolean) => void;
  setTierResult: (v: string | null) => void;
  rebuildLoading: boolean;
  setRebuildLoading: (v: boolean) => void;
  rebuildResult: string | null;
  setRebuildResult: (v: string | null) => void;
  handleVectorRebuild: () => Promise<void>;
}

export default function OverviewTab({
  diagnostics,
  providerPrivacyBoundary = null,
  projection,
  safeMode,
  exportLoading,
  handleExport,
  refreshAllDiagnostics,
  tierLoading,
  setTierLoading,
  setTierResult,
  rebuildLoading,
  rebuildResult,
  handleVectorRebuild,
}: OverviewTabProps) {
  const [credentialRecoveryLoading, setCredentialRecoveryLoading] = useState(false);
  const [credentialRecoveryReport, setCredentialRecoveryReport] =
    useState<CredentialRecoveryReport | null>(null);
  const [credentialRecoveryError, setCredentialRecoveryError] = useState<string | null>(null);
  const runtime = diagnostics?.runtime_build_info;
  const readiness = projection?.readiness;
  const usageReady = readiness?.usageReady ?? false;
  const modelEmpty = readiness?.modelEmpty ?? true;
  const lifeModelReady = readiness?.lifeModelReady ?? false;
  const chatReady = readiness?.chatReady ?? false;
  const pendingBuilderReviewCount = readiness?.pendingBuilderReviewSessions ?? 0;
  const unfinishedBuilderSessionCount = readiness?.unfinishedBuilderSessions ?? 0;
  const readinessIssues = readiness?.readinessIssues ?? [];
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
      label: "模型边界",
      ok: Boolean(
        providerPrivacyBoundary &&
        !providerPrivacyBoundary.blockedReason &&
        providerPrivacyBoundary.risk !== "unknown"
      ),
      detail: providerPrivacyBoundary
        ? `${providerPrivacyBoundary.providerLabel} · ${
            providerPrivacyBoundary.blockedReason ?? providerPrivacyBoundary.privacyLabel
          } · ${providerPrivacyBoundary.externalTransmission}`
        : "等待 ProviderPrivacyBoundarySummary",
      action: "配置模型",
      href: "#llm-settings",
    },
    {
      label: "本地模型",
      ok: diagnostics?.ollama_online ?? diagnostics?.ollama_service_online ?? false,
      detail: diagnostics?.ollama_online
        ? `${diagnostics?.resolved_local_model || diagnostics?.local_model} 在线`
        : diagnostics?.ollama_service_online
          ? `Ollama 已启动，但未找到可用模型：${diagnostics?.local_model || "本地模型"}`
          : "Ollama 离线，若走本地模型需要先启动",
      action: "查看本地配置",
      href: "#local-model-settings",
    },
    {
      label: "人生模型",
      ok: Boolean(lifeModelReady && !modelEmpty),
      detail: modelEmpty
        ? pendingBuilderReviewCount > 0
          ? `有 ${pendingBuilderReviewCount} 个 Builder 待确认项`
          : unfinishedBuilderSessionCount > 0
            ? `有 ${unfinishedBuilderSessionCount} 个待继续的 Builder 会话`
            : "尚未完成初始构建"
        : lifeModelReady
          ? "可读取"
          : "读取失败",
      action: modelEmpty
        ? pendingBuilderReviewCount > 0
          ? "去审阅"
          : unfinishedBuilderSessionCount > 0
            ? "继续 Builder"
            : "去构建"
        : "查看模型",
      href: modelEmpty
        ? `#${secondaryRoutePath("LifeModelBuild")}`
        : `#${productRoutePath("Today")}`,
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
      href: `#${productRoutePath("Companion")}`,
    },
  ];

  const usageFlow = [
    {
      title: "1. 完成设置与诊断",
      done: Boolean(chatReady || diagnostics?.cloud_api_configured || diagnostics?.ollama_online),
      detail: chatReady
        ? "模型后端已经可用，基础运行环境通过。"
        : "先把本地或云端模型跑通，避免进入聊天页后才发现不能用。",
      to: "#llm-settings",
      action: "检查模型配置",
    },
    {
      title: "2. 完成人生模型构建",
      done: Boolean(!modelEmpty && lifeModelReady),
      detail: modelEmpty
        ? pendingBuilderReviewCount > 0
          ? `Builder 里还有 ${pendingBuilderReviewCount} 个待确认项。先处理这些建议，比重新开始更合适。`
          : unfinishedBuilderSessionCount > 0
            ? `Builder 里还有 ${unfinishedBuilderSessionCount} 个待继续或待确认的会话。先处理确认建议，比重新开始更合适。`
            : "Builder 还没形成最小模型，当前很多建议仍会偏通用。"
        : "人生模型已可读取，个性化能力开始成立。",
      to: `#${secondaryRoutePath("LifeModelBuild")}`,
      action: modelEmpty
        ? pendingBuilderReviewCount > 0
          ? "去审阅"
          : unfinishedBuilderSessionCount > 0
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
      to: `#${productRoutePath("Companion")}`,
      action: "去对话",
    },
    {
      title: "4. 查看校准或版本回滚",
      done: Boolean((diagnostics?.snapshot_count ?? 0) > 0),
      detail:
        (diagnostics?.snapshot_count ?? 0) > 0
          ? `已经有 ${diagnostics?.snapshot_count} 个快照，版本安全网已建立。`
          : "至少确认一次快照/回滚路径，使用闭环才算具备可恢复能力。",
      to: `#${advancedRoutePath("Versions")}`,
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
    ...(pendingBuilderReviewCount > 0
      ? [
          {
            title: "Builder 待确认项",
            detail: `当前还有 ${pendingBuilderReviewCount} 个待确认项。建议先回到 Builder 审阅并应用，再验证对话与今日页。`,
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
            detail: "当前应用没有运行在完全健康的数据模式下，继续使用前建议先导出备份。",
            tone: "warning" as const,
          },
        ]
      : []),
  ];

  const handleCredentialRecovery = async () => {
    setCredentialRecoveryLoading(true);
    setCredentialRecoveryReport(null);
    setCredentialRecoveryError(null);
    try {
      setCredentialRecoveryReport(await recoverRequiredCredentialAccess());
    } catch (error) {
      setCredentialRecoveryError(readableError(error));
    } finally {
      setCredentialRecoveryLoading(false);
    }
  };

  return (
    <>
      {/* Readiness Checklist */}
      <section className="space-y-4">
        <div
          className={classNames(
            "rounded-2xl border p-4",
            chatReady ? "border-emerald-200 bg-emerald-50/60" : "border-amber-200 bg-amber-50/60"
          )}
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-900">启动检查清单</div>
              <div className="mt-1 text-xs text-stone-500">
                {chatReady
                  ? "核心链路已就绪，可以开始使用 Chat / Builder / Calibration。"
                  : "按这些项逐个修复，桌面端使用会稳定很多。"}
              </div>
            </div>
            <span
              className={classNames(
                "rounded-full px-2 py-1 text-xs font-medium shrink-0",
                chatReady ? "bg-emerald-100 text-emerald-700" : "bg-amber-100 text-amber-700"
              )}
            >
              {chatReady ? "可开始使用" : "还有阻塞"}
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
          {readinessIssues.length > 0 && (
            <div className="mt-3 rounded-lg bg-white/70 p-3">
              <div className="text-xs font-medium text-amber-800">建议先处理：</div>
              <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-700">
                {readinessIssues.map(issue => (
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
              <div className="text-sm font-semibold text-stone-900">使用闭环定义</div>
              <div className="mt-1 text-xs text-stone-500">
                下面这 4 步都跑通，才算真正形成一次可恢复的 OpenLife
                使用闭环，而不是只停留在配置或单页体验。
              </div>
            </div>
            <span
              className={classNames(
                "rounded-full px-2 py-1 text-xs font-medium",
                usageReady ? "bg-emerald-100 text-emerald-700" : "bg-blue-100 text-blue-700"
              )}
            >
              {usageReady ? "已闭环" : "闭环中"}
            </span>
          </div>
          <div className="mt-4 grid gap-3 md:grid-cols-2">
            {usageFlow.map(step => (
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
                      step.done ? "bg-emerald-100 text-emerald-700" : "bg-amber-100 text-amber-700"
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
      {diagnostics && !chatReady && (
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
            {modelEmpty && (
              <Link
                to={secondaryRoutePath("LifeModelBuild")}
                className="rounded-md bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-emerald-700"
              >
                2. 构建人生模型
              </Link>
            )}
            {!modelEmpty && diagnostics.chat_session_count === 0 && (
              <Link
                to={productRoutePath("Companion")}
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
                当前检测到启动降级、数据库异常或记忆索引损坏。建议先备份，再继续使用 Builder /
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
          <div className="grid gap-2">
            <div className="rounded-xl border border-white bg-white/80 px-3 py-3">
              <div className="text-xs font-medium text-stone-700">活跃数据目录</div>
              <div className="mt-1 break-all text-xs text-stone-500">
                {diagnostics?.active_data_dir ?? diagnostics?.data_dir ?? "-"}
              </div>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => void handleCredentialRecovery()}
              disabled={credentialRecoveryLoading}
              className="rounded-md bg-indigo-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-800 disabled:opacity-50"
            >
              {credentialRecoveryLoading ? "等待系统授权..." : "解锁或初始化系统密钥"}
            </button>
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
              onClick={() => void handleVectorRebuild()}
              disabled={rebuildLoading}
              className="rounded-md border border-emerald-300 bg-white px-3 py-1.5 text-xs font-medium text-emerald-800 hover:bg-emerald-50 disabled:opacity-50"
            >
              {rebuildLoading ? "重建中..." : "重建向量索引"}
            </button>
          </div>
          <div className="rounded-lg border border-indigo-100 bg-indigo-50/80 px-3 py-3 text-xs text-indigo-900">
            此操作先经过 OpenLife 原生确认，再由系统凭据库授权；前端不会读取或显示密钥。macOS
            可能再次要求你确认访问。全部就绪后需要重启应用，当前 Safe Mode 不会被页面自行改写。
          </div>
          {credentialRecoveryReport && (
            <div
              className={classNames(
                "rounded-lg px-3 py-3 text-xs",
                credentialRecoveryReport.allRequiredCredentialsReady
                  ? "bg-emerald-50 text-emerald-900"
                  : "bg-rose-50 text-rose-900"
              )}
            >
              <div className="font-medium">
                {credentialRecoveryReport.allRequiredCredentialsReady
                  ? "系统密钥已可访问，请完全退出并重启 OpenLife。"
                  : "仍有系统密钥不可用；没有生成替代密钥，也没有覆盖已有数据。"}
              </div>
              <div className="mt-2 space-y-1 font-mono">
                {credentialRecoveryReport.items.map(item => (
                  <div key={item.purpose}>
                    {CREDENTIAL_PURPOSE_LABELS[item.purpose]}：
                    {CREDENTIAL_STATUS_LABELS[item.status]} ({item.status})
                  </div>
                ))}
              </div>
            </div>
          )}
          {credentialRecoveryError && (
            <div className="rounded-lg bg-rose-50 px-3 py-3 text-xs text-rose-900">
              系统密钥恢复失败：{credentialRecoveryError}
            </div>
          )}
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

      {runtime && (
        <section id="runtime-build-info" className="space-y-4">
          <h3 className="text-sm font-medium text-gray-700">运行来源</h3>
          <div className="grid gap-3 md:grid-cols-2">
            {[
              ["Profile", runtime.profile],
              ["Frontend", runtime.frontendMode],
              ["Binary", runtime.binaryKind],
              ["Git", runtime.gitSha],
              ["Build", runtime.buildTime],
              ["Dev URL", runtime.devUrl || "-"],
              ["Frontend dist", runtime.frontendDist],
              ["Executable", runtime.currentExe],
              ["Data dir", runtime.dataDir],
              ["A2A", `${runtime.a2aStatus} · ${runtime.a2aPort}`],
              ["Bundle", runtime.bundleIdentifier],
              ["Product", runtime.productName],
            ].map(([label, value]) => (
              <div key={label} className="rounded-lg border border-stone-200 bg-stone-50 p-3">
                <div className="text-xs font-medium text-stone-500">{label}</div>
                <div className="mt-1 break-all text-xs text-stone-800">{value}</div>
              </div>
            ))}
          </div>
        </section>
      )}
    </>
  );
}
