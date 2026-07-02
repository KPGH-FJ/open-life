import { useEffect, useMemo, useState } from "react";
import { useLocation } from "react-router-dom";
import {
  AlertTriangle,
  Archive,
  Check,
  Clock,
  Edit2,
  ListChecks,
  RefreshCw,
  ShieldCheck,
  X,
  type LucideIcon,
} from "lucide-react";
import {
  acceptProposal,
  editProposal,
  getConfig,
  getSystemDiagnostics,
  listProposals,
  postponeProposal,
  rejectProposal,
  resumeMainChatAgentTask,
  type AgentProposal,
  type AppConfig,
  type ProposalStatus,
} from "../tauri";
import ReviewDecisionCard from "../components/ReviewDecisionCard";
import type { MailboxRouteState } from "../productShellContract";
import { canonicalLifeModelPath, proposalSubject } from "../utils/proposalDisplay";
import {
  buildReviewDecisionView,
  reviewGroupLabel,
  type ReviewDecisionGroup,
} from "../utils/reviewDecision";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";

type FolderId = "pending" | "accepted" | "archived" | "needs_edit";
type QuickAction = "accept" | "reject" | "postpone";
type ReviewGroupFilter = "all" | ReviewDecisionGroup;

const FOLDERS: Array<{ id: FolderId; label: string; icon: LucideIcon }> = [
  { id: "pending", label: "待确认", icon: ListChecks },
  { id: "accepted", label: "已同意", icon: Check },
  { id: "archived", label: "已处理", icon: Archive },
  { id: "needs_edit", label: "草稿修改", icon: Edit2 },
];

function isUnsupportedType(type: string): boolean {
  return ["plugin_permission", "model_policy_change", "schedule_checkin", "unsupported"].includes(
    type
  );
}

function editableProposalValue(value: unknown): string {
  if (typeof value === "string") return value;
  return JSON.stringify(value ?? null, null, 2);
}

function folderMatches(proposal: AgentProposal, folder: FolderId): boolean {
  if (folder === "pending") return proposal.status === "pending";
  if (folder === "accepted") return proposal.status === "accepted";
  if (folder === "archived") {
    return ["rejected", "postponed", "expired"].includes(proposal.status);
  }
  return proposal.status === "edited";
}

function folderForProposal(proposal: AgentProposal): FolderId {
  if (proposal.status === "accepted") return "accepted";
  if (proposal.status === "edited") return "needs_edit";
  if (["rejected", "postponed", "expired"].includes(proposal.status)) return "archived";
  return "pending";
}

function senderFor(_proposal: AgentProposal): string {
  return "OpenLife";
}

function impactLabel(risk: AgentProposal["riskLevel"]): string {
  const labels: Record<AgentProposal["riskLevel"], string> = {
    low: "低",
    medium: "中",
    high: "高",
    critical: "严重",
  };
  return labels[risk] ?? String(risk);
}

function riskClass(risk: AgentProposal["riskLevel"]): string {
  if (risk === "high" || risk === "critical") return "border-rose-200 bg-rose-50 text-rose-800";
  if (risk === "medium") return "border-amber-200 bg-amber-50 text-amber-800";
  return "border-emerald-200 bg-emerald-50 text-emerald-800";
}

function statusClass(status: ProposalStatus): string {
  if (status === "accepted") return "border-emerald-200 bg-emerald-50 text-emerald-800";
  if (status === "rejected" || status === "expired")
    return "border-stone-200 bg-stone-100 text-stone-600";
  if (status === "edited" || status === "postponed")
    return "border-amber-200 bg-amber-50 text-amber-800";
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
  if (safeModeActive) return "Safe Mode 下无法同意或编辑。";
  if (proposal.status !== "pending") return "只有待确认项可以同意。";
  if (isUnsupportedType(proposal.proposalType)) {
    return "这类确认当前尚未接入应用器，不能同意。";
  }
  const path = externalWritePath(proposal);
  if (proposal.proposalType === "external_write_action" && !isPathInSafePaths(path, safePaths)) {
    return "目标路径不在文件访问范围内，不能同意。";
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
    return `已处理确认：${proposal.affectedPath}`;
  }
  return `已应用到人生模型：${canonicalLifeModelPath(proposal.affectedPath)}`;
}

function statusLabel(status: ProposalStatus): string {
  const labels: Record<ProposalStatus, string> = {
    pending: "待确认",
    accepted: "已同意",
    rejected: "不同意",
    edited: "已修改",
    postponed: "稍后再说",
    expired: "已处理",
  };
  return labels[status];
}

export default function MailboxPage() {
  const location = useLocation();
  const routeState = location.state as MailboxRouteState | null;
  const mainChatTaskSessionId =
    typeof routeState?.mainChatTaskSessionId === "string" ? routeState.mainChatTaskSessionId : null;
  const proposalDeepLinkId = useMemo(() => {
    const proposalId = new URLSearchParams(location.search).get("proposal")?.trim();
    return proposalId || null;
  }, [location.search]);
  const [proposals, setProposals] = useState<AgentProposal[]>([]);
  const [folder, setFolder] = useState<FolderId>("pending");
  const [groupFilter, setGroupFilter] = useState<ReviewGroupFilter>("all");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [deepLinkMissingProposalId, setDeepLinkMissingProposalId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [actingId, setActingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [diagnostics, setDiagnostics] = useState<any>(null);
  const [safePaths, setSafePaths] = useState<string[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const [mainChatResumeTaskId, setMainChatResumeTaskId] = useState<string | null>(null);
  const [mainChatResumeBusy, setMainChatResumeBusy] = useState(false);

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
      setError(`加载 Mailbox 失败：${String(err)}`);
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

  const folderProposals = useMemo(
    () => proposals.filter(proposal => folderMatches(proposal, folder)),
    [folder, proposals]
  );

  const groupCounts = useMemo(() => {
    const counts: Record<ReviewGroupFilter, number> = {
      all: folderProposals.length,
      memory: 0,
      life_model: 0,
      tool_permission: 0,
      external_action: 0,
      model_policy: 0,
      other: 0,
    };
    folderProposals.forEach(proposal => {
      const group = buildReviewDecisionView(proposal).group;
      counts[group] += 1;
    });
    return counts;
  }, [folderProposals]);

  const visibleProposals = useMemo(
    () =>
      folderProposals.filter(proposal => {
        if (groupFilter === "all") return true;
        return buildReviewDecisionView(proposal).group === groupFilter;
      }),
    [folderProposals, groupFilter]
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

  useEffect(() => {
    if (loading) return;
    if (!proposalDeepLinkId) {
      setDeepLinkMissingProposalId(null);
      return;
    }
    const matchedProposal = proposals.find(proposal => proposal.id === proposalDeepLinkId);
    if (!matchedProposal) {
      setDeepLinkMissingProposalId(proposalDeepLinkId);
      return;
    }
    setDeepLinkMissingProposalId(null);
    setFolder(folderForProposal(matchedProposal));
    setGroupFilter("all");
    setSelectedId(matchedProposal.id);
  }, [loading, proposalDeepLinkId, proposals]);

  const selectedProposal =
    visibleProposals.find(proposal => proposal.id === selectedId) ?? visibleProposals[0] ?? null;
  const selectedDecision = selectedProposal ? buildReviewDecisionView(selectedProposal) : null;

  const runAction = async (proposal: AgentProposal, action: QuickAction) => {
    setActingId(proposal.id);
    setError(null);
    setNotice(null);
    const linkedMainChatTaskId =
      action === "accept" &&
      mainChatTaskSessionId &&
      proposal.sourceDetail === mainChatTaskSessionId
        ? mainChatTaskSessionId
        : null;

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
        if (linkedMainChatTaskId) {
          setMainChatResumeTaskId(linkedMainChatTaskId);
        }
      } else if (action === "reject") {
        await rejectProposal(proposal.id);
        setNotice(`已不同意：${proposal.affectedPath}`);
      } else {
        await postponeProposal(proposal.id);
        setNotice(`已标记稍后再说：${proposal.affectedPath}`);
      }
      await load();
    } catch (err) {
      const message = String(err);
      if (message.includes("no_such_field") || message.includes("不包含字段路径")) {
        setError(`应用失败：字段路径 "${proposal.affectedPath}" 不存在于当前 LifeModel。`);
      } else if (message.includes("无法转换")) {
        setError(`应用失败：值类型与字段 "${proposal.affectedPath}" 不匹配。`);
      } else if (message.includes("尚未接入应用器") || message.includes("not supported")) {
        setError("处理失败：这类确认在当前版本中尚未支持，会继续留在待确认列表。");
      } else {
        setError(`处理失败：${message}`);
      }
    } finally {
      setActingId(null);
    }
  };

  const handleResumeMainChatTask = async () => {
    if (!mainChatResumeTaskId) return;
    setMainChatResumeBusy(true);
    setError(null);
    try {
      const state = await resumeMainChatAgentTask(mainChatResumeTaskId);
      const status = state.session?.status?.replace(/_/g, " ") || "running";
      setNotice(`Main Chat task resumed: ${status}`);
    } catch (err) {
      setError(`恢复 Main Chat task 失败：${String(err)}`);
    } finally {
      setMainChatResumeBusy(false);
    }
  };

  const startEdit = (proposal: AgentProposal) => {
    setEditingId(proposal.id);
    setEditValue(editableProposalValue(proposal.after));
    setError(null);
    setNotice(null);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setEditValue("");
  };

  const saveEdit = async (proposal: AgentProposal) => {
    if (safeModeActive) {
      setError("Safe Mode 下无法同意或编辑。");
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
      aria-label="Mailbox"
      className="h-full min-h-0 overflow-hidden overflow-x-hidden bg-[#f5f6f2] px-3 py-3 sm:px-4"
    >
      <div className="mx-auto flex h-full min-h-0 w-full max-w-[1500px] flex-col gap-3">
        <div className="flex shrink-0 items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-stone-950 text-white shadow-sm">
              <ShieldCheck size={18} aria-hidden="true" />
            </div>
            <div>
              <h2 className="text-xl font-bold tracking-normal text-stone-950">Mailbox</h2>
              <div className="text-xs text-stone-500">
                记忆、权限与 Life Model 建议都需要你确认后才会生效。
              </div>
            </div>
            <span className="rounded-md border border-stone-200 bg-white px-2.5 py-1 text-xs text-stone-600">
              {folderCounts.pending} 个待确认
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
              {safeModeReason} 当前仅可查看、不同意或稍后再说，无法同意或编辑。
            </div>
          </div>
        )}

        {notice && (
          <div className="shrink-0 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
            {notice}
          </div>
        )}
        {deepLinkMissingProposalId && (
          <div className="shrink-0 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <div className="font-medium">确认项不存在、已处理或不可见。</div>
            <div className="mt-1 text-xs text-amber-800">
              Mailbox 没有找到 {deepLinkMissingProposalId}，你仍可以继续处理当前列表中的确认项。
            </div>
          </div>
        )}
        {mainChatResumeTaskId && (
          <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-lg border border-stone-200 bg-white px-4 py-3 text-sm text-stone-700 shadow-sm">
            <div>
              <div className="font-semibold text-stone-950">Main Chat task ready to resume</div>
              <div className="mt-1 text-xs text-stone-500">
                Session {mainChatResumeTaskId.slice(-8)}
              </div>
            </div>
            <button
              type="button"
              aria-label="Resume Main Chat task"
              onClick={handleResumeMainChatTask}
              disabled={mainChatResumeBusy}
              className="inline-flex h-9 items-center gap-1.5 rounded-md bg-stone-900 px-3 text-xs font-medium text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {mainChatResumeBusy ? "Resuming..." : "Resume Main Chat task"}
            </button>
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
              <div className="mt-2 flex flex-wrap gap-1">
                {(
                  [
                    "all",
                    "memory",
                    "life_model",
                    "tool_permission",
                    "external_action",
                    "model_policy",
                  ] as ReviewGroupFilter[]
                ).map(group => {
                  const active = groupFilter === group;
                  const label = group === "all" ? "全部" : reviewGroupLabel(group);
                  return (
                    <button
                      key={group}
                      type="button"
                      onClick={() => setGroupFilter(group)}
                      className={[
                        "inline-flex h-7 items-center rounded-md border px-2 text-[11px] font-semibold",
                        active
                          ? "border-stone-900 bg-stone-900 text-white"
                          : "border-stone-200 bg-white text-stone-600 hover:bg-stone-50",
                      ].join(" ")}
                    >
                      {label} {groupCounts[group]}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="min-h-0 flex-1 overflow-auto">
              {loading ? (
                <div className="p-4 text-sm text-stone-500">正在加载确认项...</div>
              ) : visibleProposals.length === 0 ? (
                <div className="flex h-full min-h-[260px] flex-col items-center justify-center p-8 text-center">
                  <ListChecks size={36} className="text-stone-300" aria-hidden="true" />
                  <div className="mt-3 text-sm font-semibold text-stone-800">没有确认项</div>
                  <div className="mt-1 text-xs text-stone-500">当前文件夹没有待确认内容。</div>
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
                            影响：{impactLabel(proposal.riskLevel)}
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
                              暂不支持同意
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
                    <span className="font-semibold text-stone-950">
                      {senderFor(selectedProposal)}
                    </span>
                    <span>给你</span>
                    <span>{formatDate(selectedProposal.createdAt)}</span>
                  </div>
                  <h3 className="mt-2 text-lg font-bold tracking-normal text-stone-950">
                    {selectedDecision?.title}
                  </h3>
                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    <span
                      className={`rounded-full border px-2.5 py-1 text-[11px] font-medium ${riskClass(
                        selectedProposal.riskLevel
                      )}`}
                    >
                      影响：{impactLabel(selectedProposal.riskLevel)}
                    </span>
                    <span
                      className={`rounded-full border px-2.5 py-1 text-[11px] font-medium ${statusClass(
                        selectedProposal.status
                      )}`}
                    >
                      状态：{statusLabel(selectedProposal.status)}
                    </span>
                    <span className="rounded-full bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">
                      把握：{Math.round(selectedProposal.confidence * 100)}%
                    </span>
                    <span className="rounded-full bg-stone-100 px-2.5 py-1 text-[11px] text-stone-600">
                      {selectedDecision?.groupLabel}
                    </span>
                  </div>
                </header>

                <div className="space-y-4 px-5 py-4">
                  {selectedDecision && <ReviewDecisionCard view={selectedDecision} />}

                  <section className="rounded-lg border border-stone-200 bg-stone-50 p-4">
                    {selectedProposal.proposalType === "external_write_action" && (
                      <div className="rounded-md border border-stone-200 bg-white p-3 text-xs text-stone-600">
                        <div className="font-semibold text-stone-700">外部操作边界</div>
                        <div className="mt-2 leading-5">
                          这会请求外部写入；未同意前不会执行。路径、内容摘要和 Run
                          信息只放在技术详情中。
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
                          这个确认项仍必须由你确认；暂不支持的类型和文件访问范围之外的写入不会被同意。
                        </span>
                      </div>
                    )}
                  </section>

                  <section className="rounded-lg border border-stone-200 p-4">
                    <div className="text-xs font-semibold uppercase tracking-normal text-stone-500">
                      你的回复
                    </div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "accept")}
                        disabled={
                          !canAccept(selectedProposal, safeModeActive, safePaths) ||
                          actingId === selectedProposal.id
                        }
                        title={
                          actionBlockedReason(selectedProposal, safeModeActive, safePaths) ??
                          undefined
                        }
                        className="inline-flex h-9 items-center gap-1.5 rounded-md bg-stone-900 px-3 text-sm font-medium text-white hover:bg-stone-800 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <Check size={15} aria-hidden="true" />
                        同意
                      </button>
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "reject")}
                        disabled={actingId === selectedProposal.id}
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-rose-200 bg-white px-3 text-sm font-medium text-rose-700 hover:bg-rose-50 disabled:opacity-50"
                      >
                        <X size={15} aria-hidden="true" />
                        不同意
                      </button>
                      <button
                        type="button"
                        onClick={() => runAction(selectedProposal, "postpone")}
                        disabled={actingId === selectedProposal.id}
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:opacity-50"
                      >
                        <Clock size={15} aria-hidden="true" />
                        稍后再说
                      </button>
                      <button
                        type="button"
                        onClick={() => startEdit(selectedProposal)}
                        disabled={
                          safeModeActive ||
                          actingId === selectedProposal.id ||
                          editingId === selectedProposal.id
                        }
                        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-3 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        <Edit2 size={15} aria-hidden="true" />
                        改一下
                      </button>
                    </div>

                    {editingId === selectedProposal.id && (
                      <div className="mt-4 rounded-lg border border-amber-200 bg-amber-50 p-3">
                        <label
                          className="text-xs font-medium text-amber-900"
                          htmlFor="mailbox-edit-after"
                        >
                          你想改成什么
                        </label>
                        <textarea
                          id="mailbox-edit-after"
                          value={editValue}
                          onChange={event => setEditValue(event.target.value)}
                          rows={5}
                          className="mt-2 w-full rounded-md border border-amber-200 bg-white p-3 text-sm leading-6 text-stone-800 outline-none focus:border-amber-500"
                          placeholder="输入 JSON 或文本。原始内容不会在这里自动展开。"
                        />
                        <div className="mt-2 flex gap-2">
                          <button
                            type="button"
                            onClick={() => saveEdit(selectedProposal)}
                            disabled={
                              actingId === selectedProposal.id || editValue.trim().length === 0
                            }
                            className="rounded-md bg-stone-900 px-3 py-1.5 text-xs font-medium text-white hover:bg-stone-800 disabled:opacity-50"
                          >
                            保存
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
                <ListChecks size={42} className="text-stone-300" aria-hidden="true" />
                <div className="mt-4 text-base font-semibold text-stone-800">选择一个确认项</div>
                <div className="mt-1 text-sm text-stone-500">左侧列表中没有可阅读的确认项。</div>
              </div>
            )}
          </main>
        </div>
      </div>
    </section>
  );
}
