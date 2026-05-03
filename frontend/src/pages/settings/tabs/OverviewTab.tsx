import { Link } from "react-router-dom";
import type { SystemDiagnostics } from "../../../tauri";

function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

interface OverviewTabProps {
  diagnostics: SystemDiagnostics | null;
  safeMode: boolean;
  exportLoading: boolean;
  handleExport: () => Promise<void>;
}

export default function OverviewTab({
  diagnostics,
  safeMode,
  exportLoading,
  handleExport,
}: OverviewTabProps) {
  // Trial checks data
  const trialChecks = diagnostics
    ? [
        {
          label: "模型后端",
          ok: diagnostics.chat_ready,
          detail: diagnostics.ollama_online
            ? `本地模型 ${diagnostics.resolved_local_model || diagnostics.local_model} 在线`
            : diagnostics.cloud_api_configured
              ? `云端 ${diagnostics.cloud_provider} 已配置`
              : "未配置模型后端",
          action: diagnostics.chat_ready ? "已就绪" : "去配置",
          href: diagnostics.chat_ready ? "#" : "#/settings",
        },
        {
          label: "人生模型",
          ok: !diagnostics.model_empty,
          detail: diagnostics.model_empty ? "未构建" : "已构建",
          action: diagnostics.model_empty ? "去构建" : "查看",
          href: diagnostics.model_empty ? "#/builder" : "#/life",
        },
        {
          label: "对话验证",
          ok: (diagnostics.chat_session_count ?? 0) > 0,
          detail:
            (diagnostics.chat_session_count ?? 0) > 0
              ? `已验证（${diagnostics.chat_session_count} 个会话）`
              : "未验证",
          action: (diagnostics.chat_session_count ?? 0) > 0 ? "已验证" : "去对话",
          href: "#/chat",
        },
        {
          label: "首次引导",
          ok: diagnostics.onboarding_completed,
          detail: diagnostics.onboarding_completed ? "已完成" : "未完成",
          action: diagnostics.onboarding_completed ? "已完成" : "查看",
          href: "#/settings",
        },
      ]
    : [];

  const betaFlow = diagnostics
    ? [
        {
          title: "1. 完成设置与诊断",
          detail: "配置模型后端，确认系统诊断无阻塞项。",
          done: diagnostics.chat_ready,
          action: "去设置",
          to: "#/settings",
        },
        {
          title: "2. 完成人生模型构建",
          detail: "在 Builder 中完成 Identity、Goals、Capabilities、State 的构建。",
          done: !diagnostics.model_empty,
          action: "去构建",
          to: "#/builder",
        },
        {
          title: "3. 跑通第一次对话",
          detail: "在 Chat 中发送消息，确认 AgentLoop 正常执行。",
          done: (diagnostics.chat_session_count ?? 0) > 0,
          action: "去对话",
          to: "#/chat",
        },
        {
          title: "4. 查看校准或版本回滚",
          detail: "了解 Calibration 和 VersionControl 的使用。",
          done: diagnostics.onboarding_completed,
          action: "去校准",
          to: "#/calibration",
        },
      ]
    : [];

  const recoveryIssues = diagnostics
    ? [
        ...((diagnostics.vector_corrupt_embedding_count ?? 0) > 0
          ? [
              {
                title: "向量索引损坏",
                detail: `检测到 ${diagnostics.vector_corrupt_embedding_count} 条损坏的向量嵌入，建议重建向量索引。`,
                tone: "error" as const,
              },
            ]
          : []),
        ...((diagnostics.unfinished_builder_sessions ?? 0) > 0
          ? [
              {
                title: "Builder 待确认 Review",
                detail: `有 ${diagnostics.unfinished_builder_sessions} 个待继续的 Builder 会话，建议先应用 Review。`,
                tone: "warning" as const,
              },
            ]
          : []),
        ...((diagnostics.memory_chunk_count ?? 0) === 0
          ? [
              {
                title: "语义记忆为空",
                detail: "当前没有向量记忆数据，对话上下文可能受限。",
                tone: "warning" as const,
              },
            ]
          : []),
      ]
    : [];

  const dataFileItems = diagnostics
    ? [
        {
          label: "messages.db",
          exists: diagnostics.data_files?.messages_db_exists,
          size: diagnostics.data_files?.messages_db_size_mb,
        },
        {
          label: "vectors.db",
          exists: diagnostics.data_files?.vectors_db_exists,
          size: diagnostics.data_files?.vectors_db_size_mb,
        },
        {
          label: "mcp_audit.db",
          exists: diagnostics.data_files?.mcp_audit_db_exists,
          size: diagnostics.data_files?.mcp_audit_db_size_mb,
        },
        { label: "config.yaml", exists: diagnostics.data_files?.config_yaml_exists },
        { label: "life_model.yaml", exists: diagnostics.data_files?.life_model_yaml_exists },
      ]
    : [];

  return (
    <>
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
                <a
                  href={item.href}
                  className="shrink-0 rounded-full border border-stone-200 bg-white px-3 py-1 text-xs font-medium text-stone-700 hover:bg-stone-50"
                >
                  {item.action}
                </a>
              </div>
            ))}
          </div>
          {diagnostics && diagnostics.readiness_issues.length > 0 && (
            <div className="mt-3 rounded-lg bg-white/70 p-3">
              <div className="text-xs font-medium text-amber-800">建议先处理：</div>
              <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-700">
                {diagnostics.readiness_issues.map((issue: string) => (
                  <li key={issue}>{issue}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </section>

      {/* Beta Flow */}
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
      {diagnostics && !diagnostics.chat_ready && (
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

      {/* Safe Mode */}
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
