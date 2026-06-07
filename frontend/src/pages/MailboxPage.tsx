import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  Archive,
  Check,
  Clock,
  Edit2,
  Inbox,
  MailOpen,
  RefreshCw,
  ShieldCheck,
  X,
} from "lucide-react";
import {
  acceptProposal,
  editProposal,
  getConfig,
  getSystemDiagnostics,
  listProposals,
  postponeProposal,
  rejectProposal,
  type AgentProposal,
  type AppConfig,
  type ProposalStatus,
} from "../tauri";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";

type FolderId = "pending" | "accepted" | "archived" | "needs_edit";
type QuickAction = "accept" | "reject" | "postpone";

const FOLDERS: Array<{ id: FolderId; label: string; icon: typeof Inbox }> = [
  { id: "pending", label: "待处理", icon: Inbox },
  { id: "accepted", label: "已接受", icon: Check },
  { id: "archived", label: "已拒绝/已归档", icon: Archive },
  { id: "needs_edit", label: "需要修改", icon: Edit2 },
];

const TYPE_LABELS: Record<string, string> = {
  life_model_update: "Life Model update",
  goal_update: "Goal update",
  state_update: "State update",
  preference_update: "Preference update",
  capability_update: "Capability update",
  memory_write: "Memory write",
  memory_archive: "Memory archive",
  tool_permission: "Tool permission",
  plugin_permission: "Plugin permission",
  schedule_checkin: "Schedule check-in",
  scheduled_task: "Scheduled task",
  external_write_action: "External write",
  model_policy_change: "Model policy",
  data_export: "Data export",
  unsupported: "Unsupported proposal",
};

function isUnsupportedType(type: string): boolean {
  return ["plugin_permission", "model_policy_change", "schedule_checkin", "unsupported"].includes(
    type
  );
}

function typeLabel(type: string): string {
  return TYPE_LABELS[type] ?? type.replace(/_/g, " ");
}

function proposalSubject(proposal: AgentProposal): string {
  return `${typeLabel(proposal.proposalType)} · ${proposal.affectedPath || "未指定路径"}`;
}

function folderMatches(proposal: AgentProposal, folder: FolderId): boolean {
  if (folder === "pending") return proposal.status === "pending";
  if (folder === "accepted") return proposal.status === "accepted";
  if (folder === "archived") {
    return ["rejected", "postponed", "expired"].includes(proposal.status);
  }
  return proposal.status === "edited";
}

function senderFor(proposal: AgentProposal): string {
  if (proposal.proposalType.includes("memory")) return "Memory";
  if (
    proposal.proposalType.includes("tool") ||
    proposal.proposalType.includes("plugin") ||
    proposal.proposalType === "scheduled_task" ||
    proposal.proposalType === "external_write_action" ||
    proposal.proposalType === "data_export" ||
    proposal.proposalType === "model_policy_change"
  ) {
    return "Tool";
  }
  if (
    proposal.source === "skill_runtime" ||
    proposal.source === "chat_conversation" ||
    proposal.source === "planning_session" ||
    proposal.source === "proactive_agent"
  ) {
    return "Agent";
  }
  if (
    proposal.proposalType === "life_model_update" ||
    proposal.proposalType === "goal_update" ||
    proposal.proposalType === "state_update" ||
    proposal.proposalType === "preference_update" ||
    proposal.proposalType === "capability_update" ||
    proposal.source === "builder_review" ||
    proposal.source === "calibration_run"
  ) {
    return "Life Model";
  }
  return "OpenLife";
}

function riskClass(risk: AgentProposal["riskLevel"]): string {
  if (risk === "high" || risk === "critical") return "border-rose-200 bg-rose-50 text-rose-800";
  if (risk === "medium") return "border-amber-200 bg-amber-50 text-amber-800";
  return "border-emerald-200 bg-emerald-50 text-emerald-800";
}

function statusClass(status: ProposalStatus): string {
  if (status === "accepted") return "border-emerald-200 bg-emerald-50 text-emerald-800";
  if (status === "rejected" || status === "expired") return "border-stone-200 bg-stone-100 text-stone-600";
  if (status === "edited" || status === "postponed") return "border-amber-200 bg-amber-50 text-amber-800";
  return "border-blue-200 bg-blue-50 text-blue-800";
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function truncate(value: string, length = 88): string {
  return value.length > length ? `${value.slice(0, length)}...` : value;
}

function shortDigest(value?: string): string | null {
  if (!value) return null;
  return value.length > 18 ? `${value.slice(0, 18)}...` : value;
}

function stableHash(value: string): string {
  let hash = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return `fnv1a:${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

function metadataValueSummary(value: unknown): string {
  if (value === null || value === undefined) return "空";
  if (typeof value === "string") return `文本 ${value.length} 字 · ${stableHash(value)}`;
  if (typeof value === "number" || typeof value === "boolean") return `${typeof value}: ${value}`;
  if (Array.isArray(value)) return `数组 ${value.length} 项`;
  if (typeof value === "object") {
    const keys = Object.keys(value as Record<string, unknown>).sort();
    return keys.length > 0 ? `对象字段：${keys.slice(0, 8).join(", ")}` : "空对象";
  }
  return typeof value;
}

function isPathInSafePaths(path: string | undefined, safePaths: string[]): boolean {
  if (!path || safePaths.length === 0) return false;
  const normalized = path.replace(/\\/g, "/");
  return safePaths.some(safe => {
    const safeNorm = safe.replace(/\\/g, "/");
    return normalized === safeNorm || normalized.startsWith(`${safeNorm}/`);
  });
}

function externalWritePath(proposal: AgentProposal): string | undefined {
  return proposal.proposalType === "external_write_action" ? proposal.after?.path : undefined;
}

function canAccept(proposal: AgentProposal, safeMode: boolean, safePaths: string[]): boolean {
  if (safeMode) return false;
  if (proposal.status !== "pending") return false;
  if (isUnsupportedType(proposal.proposalType)) return false;
  const path = externalWritePath(proposal);
  if (proposal.proposalType === "external_write_action" && !isPathInSafePaths(path, safePaths)) {
    return false;
  }
  return true;
}

function actionBlockedReason(
  proposal: AgentProposal,
  safeModeActive: boolean,
  safePaths: string[]
): string | null {
  if (safeModeActive) return "Safe Mode 下无法应用或编辑 Proposal。";
  if (proposal.status !== "pending") return "只有 pending Proposal 可以被应用。";
  if (isUnsupportedType(proposal.proposalType)) {
    return "该 Proposal 类型当前尚未接入应用器，不能接受。";
  }
  const path = externalWritePath(proposal);
  if (proposal.proposalType === "external_write_action" && !isPathInSafePaths(path, safePaths)) {
    return "目标路径不在 Safe Paths 内，不能应用。";
  }
  return null;
}

function appliedNotice(proposal: AgentProposal): string {
  if (proposal.proposalType === "tool_permission") {
    return `已更新工具权限：${proposal.affectedPath}`;
  }
  if (proposal.proposalType === "memory_write" || proposal.proposalType === "memory_archive") {
    return `已应用到记忆治理：${proposal.affectedPath}`;
  }
  if (
    proposal.proposalType === "plugin_permission" ||
    proposal.proposalType === "scheduled_task" ||
    proposal.proposalType === "external_write_action" ||
    proposal.proposalType === "model_policy_change" ||
    proposal.proposalType === "data_export"
  ) {
    return `已处理 Proposal：${proposal.affectedPath}`;
  }
  return `已应用到人生模型：${proposal.affectedPath}`;
}

function statusLabel(status: ProposalStatus): string {
  const labels: Record<ProposalStatus, string> = {
    pending: "pending",
    accepted: "accepted",
    rejected: "rejected",
    edited: "edited",
    postponed: "postponed",
    expired: "expired",
  };
  return labels[status];
}

export default function MailboxPage() {
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [folder, setFolder] = useState<FolderId>("pending");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [actingId, setActingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<any>(null);
  const [safePaths, setSafePaths] = useState<string[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  const safeModeActive = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const [data, diag, config] = await Promise.all([
        listProposals(undefined, undefined, undefined, 100),
        getSystemDiagnostics().catch(() => null),
        getConfig().catch(() => null),
      ]);
      setProposals(data);
      setDiagnostics(diag);
      setSafePaths((config as AppConfig | null)?.system?.safe_paths ?? []);
    } catch (err) {
      setError(`加载 Proposal 失败：${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const folderCounts = useMemo(() => {
    return FOLDERS.reduce<Record<FolderId, number>>(
      (acc, item) => {
        acc[item.id] = proposals.filter(proposal => folderMatches(proposal, item.id)).length;
        return acc;
      },
      { pending: 0, accepted: 0, archived: 0, needs_edit: 0 }
    );
  }, [proposals]);

  const visibleProposals = useMemo(
    () => proposals.filter(proposal => folderMatches(proposal, folder)),
    [folder, proposals]
  );

  useEffect(() => {
    if (visibleProposals.length === 0) {
      setSelectedId(null);
      return;
    }
    if (!selectedId || !visibleProposals.some(proposal => proposal.id === selectedId)) {
      setSelectedId(visibleProposals[0].id);
    }
  }, [selectedId, visibleProposals]);

  const selectedProposal =
    visibleProposals.find(proposal => proposal.id === selectedId) ?? visibleProposals[0] ?? null;

  const runAction = async (proposal: AgentProposal, action: QuickAction) => {
    setActingId(proposal.id);
    setError(null);
    setNotice(null);

    if (action === "accept") {
      const blocker = actionBlockedReason(proposal, safeModeActive, safePaths);
      if (blocker) {
        setError(blocker);
        setActingId(null);
        return;
      }
    }

    try {
      if (action === "accept") {
        await acceptProposal(proposal.id);
        setNotice(appliedNotice(proposal));
      } else if (action === "reject") {
        await rejectProposal(proposal.id);
        setNotice(`已拒绝：${proposal.affectedPath}`);
      } else {
        await postponeProposal(proposal.id);
        setNotice(`已稍后处理：${proposal.affectedPath}`);
      }
      await load();
    } catch (err) {
      const message = String(err);
      if (message.includes("no_such_field") || message.includes("不包含字段路径")) {
        setError(`应用失败：字段路径 "${proposal.affectedPath}" 不存在于当前 LifeModel。`);
      } else if (message.includes("无法转换")) {
        setError(`应用失败：值类型与字段 "${proposal.affectedPath}" 不匹配。`);
      } else if (message.includes("尚未接入应用器") || message.includes("not supported")) {
        setError("应用失败：该 Proposal 类型在当前版本中尚未支持。Proposal 将保持 pending 状态。");
      } else {
        setError(`处理 Proposal 失败：${message}`);
      }
    } finally {
      setActingId(null);
    }
  };

  const startEdit = (proposal: AgentProposal) => {
    setEditingId(proposal.id);
    setEditValue("");
    setError(null);
    setNotice(null);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditValue("");
  };

  const saveEdit = async (proposal: AgentProposal) => {
    if (safeModeActive) {
      setError("Safe Mode 下无法应用或编辑 Proposal。");
      return;
    }

    setActingId(proposal.id);
    setError(null);
    setNotice(null);
    try {
      let parsed: unknown;
      try {
        parsed = JSON.parse(editValue);
      } catch {
        parsed = editValue;
      }
      await editProposal(proposal.id, parsed);
      setNotice(`已编辑并应用：${proposal.affectedPath}`);
      setEditingId(null);
      setEditValue("");
      await load();
    } catch (err) {
      const message = String(err);
      if (message.includes("无法转换") || message.includes("JSON")) {
        setError(`编辑失败：值无法应用到字段 "${proposal.affectedPath}"，请检查 JSON 格式。`);
      } else {
        setError(`编辑失败：${message}`);
      }
    } finally {
      setActingId(null);
    }
  };

  return (
    <section
      data-testid="mailbox-page"
      aria-label="邮箱"
      className="h-full min-h-0 overflow-hidden overflow-x-hidden bg-[#f5f6f2] px-3 py-3 sm:px-4"
    >
      <div className="mx-auto flex h-full min-h-0 w-full max-w-[1500px] flex-col gap-3">
        <div className="flex shrink-0 items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-stone-950 text-white shadow-sm">
              <MailOpen size={18} aria-hidden="true" />
            </div>
            <h2 className="text-xl font-bold tracking-normal text-stone-950">邮箱</h2>
            <span className="rounded-md border border-stone-200 bg-white px-2.5 py-1 text-xs text-stone-600">
              {folderCounts.pending} pending
            </span>
          </div>
          <button
            type="button"
            onClick={load}
            className="inline-flex h-9 items-center gap-2 rounded-md border border-stone-200 bg-white px-3 text-sm font-medium text-stone-700 shadow-sm hover:bg-stone-50"
          >
            <RefreshCw size={15} aria-hidden="true" />
            刷新
          </button>
        </div>

        {safeModeActive && (
          <div className="shrink-0 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <div className="font-semibold">系统处于 Safe Mode</div>
            <div className="mt-1 text-xs text-amber-800">
              {safeModeReason} 当前仅可查看、拒绝或稍后处理 Proposal，无法接受或编辑。
            </div>
          </div>
        )}

        {notice && (
          <div className="shrink-0 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
            {notice}
          </div>
        )}
        {error && (
          <div className="shrink-0 rounded-lg border border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-800">
            <div className="font-medium">处理失败</div>
            <div className="mt-1">{error}</div>
          </div>
        )}

        <div className="grid min-h-0 flex-1 gap-3 lg:grid-cols-[380px_minmax(0,1fr)]">
          <aside className="flex min-h-0 flex-col overflow-hidden rounded-lg border border-stone-200 bg-white shadow-sm">
            <div className="shrink-0 border-b border-stone-100 p-2">
              <div className="grid grid-cols-2 gap-1">
                {FOLDERS.map(item => {
                  const Icon = item.icon;
                  const active = folder === item.id;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => setFolder(item.id)}
                      className={[
                        "flex h-9 min-w-0 items-center justify-between gap-2 rounded-md px-2 text-xs font-medium transition",
                        active
                          ? "bg-stone-900 text-white"
                          : "text-stone-600 hover:bg-stone-100 hover:text-stone-950",
                      ].join(" ")}
                    >
                      <span className="flex min-w-0 items-center gap-1.5">
                        <Icon size={13} aria-hidden="true" className="shrink-0" />
                        <span className="truncate">{item.label}</span>
                      </span>
                      <span
                        className={[
                          "rounded px-1.5 py-0.5 text-[10px]",
                          active ? "bg-white/15 text-white" : "bg-stone-100 text-stone-500",
                        ].join(" ")}
                      >
                        {folderCounts[item.id]}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-auto">
              {loading ? (
                <div className="p-4 text-sm text-stone-500">正在加载 Proposal...</div>
              ) : visibleProposals.length === 0 ? (
                <div className="flex h-full min-h-[260px] flex-col items-center justify-center p-8 text-center">
                  <Inbox size={36} className="text-stone-300" aria-hidden="true" />
                  <div className="mt-3 text-sm font-semibold text-stone-800">没有邮件</div>
                  <div className="mt-1 text-xs text-stone-500">当前文件夹没有 Proposal。</div>
                </div>
              ) : (
                <div className="divide-y divide-stone-100">
                  {visibleProposals.map(proposal => {
                    const active = selectedProposal?.id === proposal.id;
                    return (
                      <button
                        key={proposal.id}
                        type="button"
                        aria-pressed={active}
                        onClick={() => setSelectedId(proposal.id)}
                        className={[
                          "block w-full px-4 py-3 text-left transition",
                          active ? "bg-stone-100" : "bg-white hover:bg-stone-50",
                        ].join(" ")}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <div className="flex min-w-0 items-center gap-2">
                              <span className="shrink-0 text-xs font-semibold text-stone-950">
                                {senderFor(proposal)}
                              </span>
                              <span className="truncate text-xs text-stone-400">
                                {formatDate(proposal.createdAt)}
                              </span>
                            </div>
                            <div className="mt-1 truncate text-sm font-semibold text-stone-900">
                              {proposalSubject(proposal)}
                            </div>
                          </div>
                          <span
                            className={`shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-medium ${riskClass(
                              proposal.riskLevel
                            )}`}
                          >
                            {proposal.riskLevel}
                          </span>
                        </div>
                        <div className="mt-1 line-clamp-2 text-xs leading-5 text-stone-500">
                          {truncate(proposal.reason)}
                        </div>
                        <div className="mt-2 flex flex-wrap items-center gap-1.5">
                          <span
                            className={`rounded-full border px-2 py-0.5 text-[10px] font-medium ${statusClass(
                              proposal.status
                            )}`}
                          >
                            {statusLabel(proposal.status)}
                          </span>
                          {isUnsupportedType(proposal.proposalType) && (
                            <span className="rounded-full border border-amber-200 bg-amber-50 px-2 py-0.5 text-[10px] font-medium text-amber-800">
                              暂不支持
                            </span>
                          )}
                        </div>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </aside>

          <main
            data-testid="mail-reader"
            className="min-h-0 overflow-auto rounded-lg border border-stone-200 bg-white shadow-sm"
          >
            {selectedProposal ? (
              <article className="min-h-full">
                <header className="border-b border-stone-100 px-5 py-4">
                  <div className="flex flex-wrap items-center gap-2 text-xs text-stone-500">
                    <span className="font-semibold text-stone-950">{senderFor(selectedProposal)}</span>
                    <span>to You</span>
                    <span>{formatDate(selectedProposal.createdAt)}</span>
                  </div>
                  <h3 className="mt-2 text-lg font-bold tracking-normal text-stone-950">
                    主题：{proposalSubject(selectedProposal)}
                  </h3>
                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    <span
                      className={`rounded-full border px-2.5 py-1 text-[11px] font-medium ${riskClass(
                        selectedProposal.riskLevel
                      )}`}
                    >
                      risk {selectedProposal.riskLevel}
                    </span>
                    <span
                      className={`rounded-full border px-2.5 py-1 text-[11px] font-medium ${statusClass(
                        selectedProposal.status
                      )}`}
                    >
                      status {statusLabel(selectedProposal.status)}
                    </span>
                    <span className="rounded-full bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">
                      confidence {Math.round(selectedProposal.confidence * 100)}%
                    </span>
                  </div>
                </header>

                <div className="space-y-4 px-5 py-4">
                  <section>
                    <div className="text-xs font-semibold uppercase tracking-normal text-stone-400">
                      reason
                    </div>
                    <p className="mt-2 text-sm leading-6 text-stone-800">{selectedProposal.reason}</p>
                  </section>

                  <section className="rounded-lg border border-stone-200 bg-stone-50 p-4">
                    <div className="text-xs font-semibold uppercase tracking-normal text-stone-500">
                      impact / boundary
                    </div>
                    <div className="mt-3 grid gap-2 text-sm text-stone-700 md:grid-cols-2">
                      <div>目标：{selectedProposal.affectedPath}</div>
                      <div>变更摘要：{metadataValueSummary(selectedProposal.after)}</div>
                      <div>来源：{selectedProposal.source}</div>
                      <div>状态：{selectedProposal.status}</div>
                      {selectedProposal.sourceDetail && (
                        <div className="md:col-span-2">
                          来源详情：{metadataValueSummary(selectedProposal.sourceDetail)}
                        </div>
                      )}
                      {selectedProposal.runId && (
                        <div className="md:col-span-2">
                          Run：<a className="text-stone-900 underline" href={`#/runs/${selectedProposal.runId}`}>{selectedProposal.runId}</a>
                        </div>
                      )}
                    </div>

                    {selectedProposal.proposalType === "external_write_action" && (
                      <div className="mt-3 rounded-md border border-stone-200 bg-white p-3 text-xs text-stone-600">
                        <div className="font-semibold text-stone-700">External write boundary</div>
                        <div className="mt-2 grid gap-1.5 md:grid-cols-2">
                          <div>路径：{selectedProposal.after?.path || "未提供"}</div>
                          <div>操作：{selectedProposal.after?.operation || "unknown"}</div>
                          <div>
                            大小：
                            {selectedProposal.after?.size_bytes != null
                              ? `${selectedProposal.after.size_bytes} bytes`
                              : "unknown"}
                          </div>
                          <div>
                            Safe Paths：
                            {isPathInSafePaths(selectedProposal.after?.path, safePaths)
                              ? "允许范围内"
                              : "不在允许范围内"}
                          </div>
                          {selectedProposal.after?.content_hash && (
                            <div className="md:col-span-2">
                              digest {shortDigest(selectedProposal.after.content_hash)}
                            </div>
                          )}
                        </div>
                      </div>
                    )}

                    {(selectedProposal.riskLevel === "high" ||
                      selectedProposal.riskLevel === "critical" ||
                      selectedProposal.proposalType === "external_write_action" ||
                      isUnsupportedType(selectedProposal.proposalType)) && (
                      <div className="mt-3 flex items-start gap-2 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-900">
                        <AlertTriangle size={14} className="mt-0.5 shrink-0" aria-hidden="true" />
                        <span>
                          该 Proposal 仍必须通过确认中心处理；不支持的类型和 Safe Paths
                          之外的写入不会被接受。
                        </span>
                      </div>
                    )}
                  </section>

                  <section className="rounded-lg border border-sky-100 bg-sky-50 p-4">
                    <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-normal text-sky-800">
                      <ShieldCheck size={14} aria-hidden="true" />
                      metadata-safe evidence
                    </div>
                    {selectedProposal.whyOpenLifeThinksThis && (
                      <p className="mt-2 text-sm leading-6 text-sky-950">
                        {selectedProposal.whyOpenLifeThinksThis}
                      </p>
                    )}
                    {(selectedProposal.evidenceSummaries?.length ?? 0) > 0 && (
                      <div className="mt-3 space-y-2">
                        {selectedProposal.evidenceSummaries?.map(summary => (
                          <div key={summary.id} className="rounded-md bg-white/80 p-3 text-sm">
                            <div className="font-medium text-stone-800">{summary.summary}</div>
                            <div className="mt-2 flex flex-wrap gap-2 text-[10px] text-stone-500">
                              {summary.sourceAssetIds?.slice(0, 3).map(sourceId => (
                                <span key={sourceId} className="rounded bg-stone-100 px-2 py-0.5">
                                  source {sourceId}
                                </span>
                              ))}
                              {shortDigest(summary.contentDigest) && (
                                <span className="rounded bg-stone-100 px-2 py-0.5 font-mono">
                                  digest {shortDigest(summary.contentDigest)}
                                </span>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                    {(selectedProposal.behaviorChecks?.length ?? 0) > 0 && (
                      <div className="mt-3 space-y-2">
                        {selectedProposal.behaviorChecks?.map(check => (
                          <div key={check.id} className="rounded-md bg-white/80 p-3 text-sm">
                            <div className="font-medium text-stone-800">{check.label}</div>
                            {check.summary && (
                              <div className="mt-1 text-xs text-stone-500">{check.summary}</div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                    {!selectedProposal.whyOpenLifeThinksThis &&
                      (selectedProposal.evidenceSummaries?.length ?? 0) === 0 &&
                      (selectedProposal.behaviorChecks?.length ?? 0) === 0 && (
                        <div className="mt-2 text-sm text-sky-900">
                          暂无可展示的 metadata-safe evidence summary。
                        </div>
                      )}
                  </section>

                  <section className="rounded-lg border border-stone-200 p-4">
                    <div className="text-xs font-semibold uppercase tracking-normal text-stone-500">
                      quick replies
                    </div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "accept")}
                        disabled={!canAccept(selectedProposal, safeModeActive, safePaths) || actingId === selectedProposal.id}
                        title={
                          actionBlockedReason(selectedProposal, safeModeActive, safePaths) ??
                          undefined
                        }
                        className="inline-flex h-9 items-center gap-1.5 rounded-md bg-stone-900 px-3 text-sm font-medium text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <Check size={15} aria-hidden="true" />
                        {safeModeActive
                          ? "接受（Safe Mode）"
                          : isUnsupportedType(selectedProposal.proposalType)
                            ? "暂不支持"
                            : "接受"}
                      </button>
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "reject")}
                        disabled={actingId === selectedProposal.id}
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-rose-200 bg-white px-3 text-sm font-medium text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                      >
                        <X size={15} aria-hidden="true" />
                        拒绝
                      </button>
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "postpone")}
                        disabled={actingId === selectedProposal.id}
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50"
                      >
                        <Clock size={15} aria-hidden="true" />
                        稍后
                      </button>
                      <button
                        type="button"
                        onClick={() => startEdit(selectedProposal)}
                        disabled={safeModeActive || actingId === selectedProposal.id || editingId === selectedProposal.id}
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <Edit2 size={15} aria-hidden="true" />
                        {safeModeActive ? "修改（Safe Mode）" : "修改"}
                      </button>
                    </div>

                    {editingId === selectedProposal.id && (
                      <div className="mt-4 rounded-lg border border-amber-200 bg-amber-50 p-3">
                        <label className="text-xs font-medium text-amber-900" htmlFor="mailbox-edit-after">
                          新 after 值
                        </label>
                        <textarea
                          id="mailbox-edit-after"
                          value={editValue}
                          onChange={event => setEditValue(event.target.value)}
                          rows={5}
                          className="mt-2 w-full rounded-md border border-amber-200 bg-white p-3 text-sm leading-6 text-stone-800 outline-none focus:border-amber-500"
                          placeholder="输入 JSON 或文本。原始 payload 不会在这里自动展开。"
                        />
                        <div className="mt-2 flex gap-2">
                          <button
                            type="button"
                            onClick={() => saveEdit(selectedProposal)}
                            disabled={actingId === selectedProposal.id || editValue.trim().length === 0}
                            className="rounded-md bg-stone-900 px-3 py-1.5 text-xs font-medium text-white hover:bg-stone-800 disabled:opacity-50"
                          >
                            保存修改
                          </button>
                          <button
                            type="button"
                            onClick={cancelEdit}
                            className="rounded-md border border-stone-200 bg-white px-3 py-1.5 text-xs font-medium text-stone-700 hover:bg-stone-50"
                          >
                            取消
                          </button>
                        </div>
                      </div>
                    )}
                  </section>
                </div>
              </article>
            ) : (
              <div className="flex h-full min-h-[420px] flex-col items-center justify-center p-8 text-center">
                <Inbox size={42} className="text-stone-300" aria-hidden="true" />
                <div className="mt-4 text-base font-semibold text-stone-800">选择一封邮件</div>
                <div className="mt-1 text-sm text-stone-500">左侧列表中没有可阅读的 Proposal。</div>
              </div>
            )}
          </main>
        </div>
      </div>
    </section>
  );
}
