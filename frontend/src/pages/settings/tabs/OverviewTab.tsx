import { Link } from "react-router-dom";
import {
  runMemoryTierMaintenance,
  rebuildMemoryIndex,
  type SystemDiagnostics,
} from "../../../tauri";
import { buildSafeModeBlockedMessage } from "../../../utils/runtimeMessages";

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

interface OverviewTabProps {
  diagnostics: SystemDiagnostics | null;
  safeMode: boolean;
  exportLoading: boolean;
  handleExport: () => Promise<void>;
  refreshAllDiagnostics: () => Promise<SystemDiagnostics | null>;
  tierLoading: boolean;
  setTierLoading: (v: boolean) => void;
  setTierResult: (v: string | null) => void;
  rebuildLoading: boolean;
  setRebuildLoading: (v: boolean) => void;
  rebuildResult: string | null;
  setRebuildResult: (v: string | null) => void;
  onNavigateTab: (tabId: string, anchorId?: string) => void;
}

type NavTarget = { tab: string; anchor?: string };

function NavButton({
  nav,
  label,
  onNavigateTab,
  className,
}: {
  nav: NavTarget;
  label: string;
  onNavigateTab: (tabId: string, anchorId?: string) => void;
  className?: string;
}) {
  return (
    <button
      onClick={e => {
        e.preventDefault();
        onNavigateTab(nav.tab, nav.anchor);
      }}
      className={
        className ??
        "shrink-0 rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50 cursor-pointer"
      }
    >
      {label}
    </button>
  );
}

export default function OverviewTab({
  diagnostics,
  safeMode,
  exportLoading,
  handleExport,
  refreshAllDiagnostics,
  tierLoading,
  setTierLoading,
  setTierResult,
  rebuildLoading,
  setRebuildLoading,
  rebuildResult,
  setRebuildResult,
  onNavigateTab,
}: OverviewTabProps) {
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
      nav: { tab: "provider", anchor: "llm-settings" } as NavTarget,
    },
    {
      label: "本地模型",
      ok: diagnostics?.ollama_online ?? false,
      detail: diagnostics?.ollama_online
        ? `${diagnostics?.resolved_local_model || diagnostics?.local_model} 在线`
        : "Ollama 离线，若走本地模型需要先启动",
      action: "查看本地配置",
      nav: { tab: "provider", anchor: "local-model-settings" } as NavTarget,
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
      href: "/" + (diagnostics?.model_empty ? "builder" : ""),
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
      nav: { tab: "data", anchor: "data-health" } as NavTarget,
    },
    {
      label: "对话验证",
      ok: (diagnostics?.chat_session_count ?? 0) > 0,
      detail:
        (diagnostics?.chat_session_count ?? 0) > 0
          ? `${diagnostics?.chat_session_count} 个会话`
          : "还没有完成过一轮对话",
      action: "去对话",
      href: "/chat",
    },
  ];

  // ---- Beta Readiness Checklist (P11-2) ----
  const readinessState = diagnostics?.beta_ready
    ? "ready"
    : safeMode
      ? "safe-mode"
      : diagnostics?.chat_ready
        ? "partial"
        : "blocked";

  const readinessItems = [
    {
      id: "model-provider",
      label: "模型/Provider 就绪",
      ok: diagnostics?.chat_ready ?? false,
      detail: diagnostics?.chat_ready
        ? `${diagnostics?.cloud_provider ?? "本地"} 可用`
        : (diagnostics?.readiness_issues[0] ?? "未配置可用模型"),
      action: "配置模型",
      nav: { tab: "provider" } as NavTarget,
    },
    {
      id: "life-model",
      label: "LifeModel 状态",
      ok: Boolean(diagnostics?.life_model_ready && !diagnostics?.model_empty),
      detail: diagnostics?.model_empty ? "人生模型为空，建议先构建" : "LifeModel 可读取且非空",
      action: diagnostics?.model_empty ? "去构建" : "查看模型",
      href: "/" + (diagnostics?.model_empty ? "builder" : ""),
    },
    {
      id: "data-health",
      label: "数据健康",
      ok: !(safeMode || diagnostics?.database_status === "degraded"),
      detail: safeMode
        ? "Safe Mode: 数据环境存在风险"
        : diagnostics?.database_status === "degraded"
          ? "数据库降级运行"
          : "数据文件正常",
      action: safeMode ? "查看恢复" : "查看数据",
      nav: { tab: "data", anchor: "data-health" } as NavTarget,
    },
    {
      id: "pending-proposals",
      label: "待处理提案",
      ok: (diagnostics?.pending_proposal_count ?? 0) === 0,
      detail:
        (diagnostics?.pending_proposal_count ?? 0) > 0
          ? `${diagnostics?.pending_proposal_count} 个待处理${
              (diagnostics?.high_risk_pending_proposal_count ?? 0) > 0
                ? `（含 ${diagnostics?.high_risk_pending_proposal_count} 个高风险）`
                : ""
            }`
          : "无待处理提案",
      action: (diagnostics?.pending_proposal_count ?? 0) > 0 ? "去审阅" : "查看",
      href: "/review",
    },
    {
      id: "agent-runs",
      label: "AgentRun 记录",
      ok: (diagnostics?.agent_run_count ?? 0) > 0,
      detail:
        (diagnostics?.agent_run_count ?? 0) > 0
          ? `${diagnostics?.agent_run_count} 条运行记录`
          : "暂无运行记录",
      action: "查看 Runs",
      href: "/runs",
    },
    {
      id: "backup-snapshot",
      label: "备份/快照可用性",
      ok: (diagnostics?.snapshot_count ?? 0) > 0,
      detail:
        (diagnostics?.snapshot_count ?? 0) > 0
          ? `${diagnostics?.snapshot_count} 个快照已创建`
          : "尚未创建快照，建议先导出备份",
      action: (diagnostics?.snapshot_count ?? 0) > 0 ? "查看版本" : "去导出",
      href: (diagnostics?.snapshot_count ?? 0) > 0 ? "/versions" : undefined,
      nav: (diagnostics?.snapshot_count ?? 0) > 0 ? undefined : ({ tab: "data" } as NavTarget),
    },
    {
      id: "diagnostic-export",
      label: "诊断导出",
      ok: true,
      detail: "可随时导出诊断报告（不含 API Key 和私人内容）",
      action: "导出诊断",
      nav: { tab: "data" } as NavTarget,
    },
  ];

  const betaFlow = [
    {
      title: "1. 完成设置与诊断",
      done: Boolean(
        diagnostics?.chat_ready || diagnostics?.cloud_api_configured || diagnostics?.ollama_online
      ),
      detail: diagnostics?.chat_ready
        ? "模型后端已经可用，基础运行环境通过。"
        : "先把本地或云端模型跑通，避免进入聊天页后才发现不能用。",
      nav: { tab: "provider", anchor: "llm-settings" } as NavTarget,
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
      href: "/builder",
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
      href: "/chat",
      action: "去对话",
    },
    {
      title: "4. 查看校准或版本回滚",
      done: Boolean((diagnostics?.snapshot_count ?? 0) > 0),
      detail:
        (diagnostics?.snapshot_count ?? 0) > 0
          ? `已经有 ${diagnostics?.snapshot_count} 个快照，版本安全网已建立。`
          : "至少确认一次快照/回滚路径，Beta 试用才算具备可恢复能力。",
      href: "/versions",
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

  return (
    <>
      {/* Beta Readiness State Banner */}
      <section className="space-y-4">
        <div
          className={classNames(
            "rounded-2xl border p-4",
            readinessState === "ready"
              ? "border-emerald-200 bg-emerald-50/60"
              : readinessState === "safe-mode"
                ? "border-red-200 bg-red-50/60"
                : readinessState === "partial"
                  ? "border-amber-200 bg-amber-50/60"
                  : "border-rose-200 bg-rose-50/60"
          )}
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-900">Beta Readiness 状态</div>
              <div className="mt-1 text-xs text-stone-500">
                {readinessState === "ready"
                  ? "所有核心检查项通过，可以开始完整试用。"
                  : readinessState === "safe-mode"
                    ? "Safe Mode: 数据环境存在风险，请先导出备份并修复。"
                    : readinessState === "partial"
                      ? "核心链路可用，但仍有项待完善。"
                      : "存在阻塞项，请先修复以下问题。"}
              </div>
            </div>
            <span
              className={classNames(
                "rounded-full px-2 py-1 text-xs font-medium shrink-0",
                readinessState === "ready"
                  ? "bg-emerald-100 text-emerald-700"
                  : readinessState === "safe-mode"
                    ? "bg-red-100 text-red-700"
                    : readinessState === "partial"
                      ? "bg-amber-100 text-amber-700"
                      : "bg-rose-100 text-rose-700"
              )}
            >
              {readinessState === "ready"
                ? "就绪"
                : readinessState === "safe-mode"
                  ? "Safe Mode"
                  : readinessState === "partial"
                    ? "部分就绪"
                    : "阻塞"}
            </span>
          </div>

          {/* Readiness Items */}
          <div className="mt-4 space-y-2">
            {readinessItems.map(item => (
              <div
                key={item.id}
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
                {"nav" in item && item.nav ? (
                  <NavButton nav={item.nav} label={item.action} onNavigateTab={onNavigateTab} />
                ) : item.href ? (
                  <Link
                    to={item.href}
                    className="shrink-0 rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
                  >
                    {item.action}
                  </Link>
                ) : null}
              </div>
            ))}
          </div>

          {/* Beta Readiness Issues */}
          {diagnostics && diagnostics.beta_readiness_issues.length > 0 && (
            <div className="mt-3 rounded-lg bg-white/70 p-3">
              <div className="text-xs font-medium text-amber-800">试用就绪建议：</div>
              <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-700">
                {diagnostics.beta_readiness_issues.map(issue => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </section>

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
                {"nav" in item && item.nav ? (
                  <NavButton nav={item.nav} label={item.action} onNavigateTab={onNavigateTab} />
                ) : item.href ? (
                  <Link
                    to={item.href}
                    className="shrink-0 rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
                  >
                    {item.action}
                  </Link>
                ) : null}
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
                      step.done ? "bg-emerald-100 text-emerald-700" : "bg-amber-100 text-amber-700"
                    )}
                  >
                    {step.done ? "完成" : "待完成"}
                  </span>
                </div>
                <div className="mt-3">
                  {"nav" in step && step.nav ? (
                    <NavButton nav={step.nav} label={step.action} onNavigateTab={onNavigateTab} />
                  ) : step.href ? (
                    <Link
                      to={step.href}
                      className="inline-flex rounded-full border border-stone-200 bg-white px-3 py-1 text-[11px] font-medium text-stone-700 hover:bg-stone-50"
                    >
                      {step.action}
                    </Link>
                  ) : null}
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
              <NavButton
                nav={{ tab: "provider", anchor: "llm-settings" }}
                label="1. 配置 API Key"
                onNavigateTab={onNavigateTab}
                className="rounded-md bg-indigo-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-700"
              />
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
                  const recovered = refreshed && !safeMode;
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
    </>
  );
}
