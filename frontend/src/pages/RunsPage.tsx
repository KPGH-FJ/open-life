import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { listAgentRuns, deleteAgentRun, type AgentRun } from "../tauri";
import {
  Activity,
  Clock,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Trash2,
  RotateCcw,
  Search,
  Filter,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
} from "lucide-react";

function statusIcon(status: string) {
  switch (status) {
    case "running":
      return <Activity size={16} className="text-blue-500 animate-pulse" />;
    case "completed":
      return <CheckCircle size={16} className="text-emerald-500" />;
    case "failed":
      return <XCircle size={16} className="text-red-500" />;
    case "cancelled":
      return <AlertTriangle size={16} className="text-amber-500" />;
    default:
      return <Activity size={16} className="text-stone-400" />;
  }
}

function kindLabel(kind: string): string {
  const labels: Record<string, string> = {
    conversation: "Chat",
    builder: "Life Model Building",
    calibration: "Calibration",
    evolution: "Evolution",
    tool_execution: "Tool",
    proactive: "Proactive",
    planning: "Planning",
    review: "Review",
    writing: "Writing",
    memory_governance: "Memory",
  };
  return labels[kind] || kind;
}

const PAGE_SIZE = 20;

export default function RunsPage() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<string>("all");
  const [kindFilter, setKindFilter] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedRuns, setSelectedRuns] = useState<Set<string>>(new Set());
  const [showTrash, setShowTrash] = useState(false);
  const [page, setPage] = useState(0);
  const navigate = useNavigate();

  useEffect(() => {
    loadRuns();
  }, [showTrash]);

  async function loadRuns() {
    try {
      setLoading(true);
      const data = await listAgentRuns(100, 0);
      setRuns(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  const filteredRuns = runs.filter(run => {
    // Trash filter
    if (showTrash) {
      return !!run.deletedAt;
    } else {
      if (run.deletedAt) return false;
    }

    // Status filter
    if (statusFilter !== "all" && run.status !== statusFilter) return false;

    // Kind filter
    if (kindFilter !== "all" && run.kind !== kindFilter) return false;

    // Search
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      const text = `${run.userInput ?? ""} ${run.outputPreview ?? ""} ${run.kind}`.toLowerCase();
      if (!text.includes(query)) return false;
    }

    return true;
  });

  const paginatedRuns = filteredRuns.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  const totalPages = Math.ceil(filteredRuns.length / PAGE_SIZE);

  function toggleSelect(runId: string) {
    const newSet = new Set(selectedRuns);
    if (newSet.has(runId)) {
      newSet.delete(runId);
    } else {
      newSet.add(runId);
    }
    setSelectedRuns(newSet);
  }

  function selectAll() {
    if (selectedRuns.size === paginatedRuns.length) {
      setSelectedRuns(new Set());
    } else {
      setSelectedRuns(new Set(paginatedRuns.map(r => r.id)));
    }
  }

  async function handleBatchDelete() {
    if (!confirm(`确定要删除选中的 ${selectedRuns.size} 条记录吗？`)) return;
    try {
      for (const runId of selectedRuns) {
        await deleteAgentRun(runId);
      }
      setSelectedRuns(new Set());
      await loadRuns();
    } catch (e) {
      setError(String(e));
    }
  }

  const statusOptions = [
    { value: "all", label: "全部状态" },
    { value: "running", label: "运行中" },
    { value: "completed", label: "已完成" },
    { value: "failed", label: "失败" },
    { value: "cancelled", label: "已取消" },
  ];

  const kindOptions = [
    { value: "all", label: "全部类型" },
    { value: "conversation", label: "Chat" },
    { value: "builder", label: "Builder" },
    { value: "calibration", label: "Calibration" },
  ];

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-stone-500">加载中...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <div className="text-red-500 mb-3">{error}</div>
          <button
            onClick={() => loadRuns()}
            className="text-sm px-3 py-1.5 rounded-md bg-red-50 text-red-700 hover:bg-red-100 border border-red-200"
          >
            重试
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-5xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-bold text-stone-900">{showTrash ? "已删除" : "Runs"}</h1>
            <div className="text-sm text-stone-500">
              共 {filteredRuns.length} 条记录
              {showTrash && " (当前版本不可恢复)"}
            </div>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => {
                setShowTrash(!showTrash);
                setPage(0);
                setSelectedRuns(new Set());
              }}
              className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition ${
                showTrash
                  ? "bg-stone-900 text-amber-50"
                  : "bg-white border border-stone-200 text-stone-700 hover:bg-stone-50"
              }`}
            >
              {showTrash ? <RotateCcw size={16} /> : <Trash2 size={16} />}
              {showTrash ? "返回列表" : "已删除"}
            </button>
            <button
              onClick={loadRuns}
              className="flex items-center gap-2 px-4 py-2 bg-white border border-stone-200 text-stone-700 rounded-lg text-sm font-medium hover:bg-stone-50 transition"
            >
              <RefreshCw size={16} />
              刷新
            </button>
          </div>
        </div>

        {/* Filters */}
        <div className="bg-white rounded-xl border border-stone-200 p-4 mb-4 space-y-3">
          <div className="flex flex-wrap gap-3">
            {/* Search */}
            <div className="flex-1 min-w-[200px]">
              <div className="relative">
                <Search
                  size={16}
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-stone-400"
                />
                <input
                  type="text"
                  placeholder="搜索输入内容或输出..."
                  value={searchQuery}
                  onChange={e => {
                    setSearchQuery(e.target.value);
                    setPage(0);
                  }}
                  className="w-full pl-9 pr-4 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
                />
              </div>
            </div>

            {/* Status filter */}
            <div className="flex items-center gap-2">
              <Filter size={16} className="text-stone-400" />
              <select
                value={statusFilter}
                onChange={e => {
                  setStatusFilter(e.target.value);
                  setPage(0);
                }}
                className="px-3 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
              >
                {statusOptions.map(opt => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>

            {/* Kind filter */}
            <select
              value={kindFilter}
              onChange={e => {
                setKindFilter(e.target.value);
                setPage(0);
              }}
              className="px-3 py-2 border border-stone-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-stone-900"
            >
              {kindOptions.map(opt => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          {/* Batch actions */}
          {selectedRuns.size > 0 && (
            <div className="flex items-center gap-3 pt-3 border-t border-stone-100">
              <span className="text-sm text-stone-600">已选择 {selectedRuns.size} 条</span>
              {showTrash ? (
                <span className="text-xs text-stone-400">已删除记录不可恢复</span>
              ) : (
                <button
                  onClick={handleBatchDelete}
                  className="flex items-center gap-1 px-3 py-1.5 bg-red-600 text-white rounded-lg text-sm hover:bg-red-700 transition"
                >
                  <Trash2 size={14} />
                  删除
                </button>
              )}
              <button
                onClick={() => setSelectedRuns(new Set())}
                className="px-3 py-1.5 text-stone-600 text-sm hover:bg-stone-100 rounded-lg transition"
              >
                取消选择
              </button>
            </div>
          )}
        </div>

        {/* Runs List */}
        {paginatedRuns.length === 0 ? (
          <div className="text-center py-12 text-stone-400">
            <Activity size={48} className="mx-auto mb-4 opacity-30" />
            <p>{showTrash ? "暂无已删除记录" : "暂无运行记录"}</p>
            <p className="text-sm mt-1">
              {showTrash
                ? "已删除的 Run 在当前版本中不可恢复"
                : "开始对话或构建 LifeModel 后将在此显示"}
            </p>
          </div>
        ) : (
          <>
            <div className="space-y-3">
              {/* Select all header */}
              <div className="flex items-center gap-3 px-4 py-2 bg-stone-50 rounded-lg">
                <input
                  type="checkbox"
                  checked={
                    paginatedRuns.length > 0 && paginatedRuns.every(r => selectedRuns.has(r.id))
                  }
                  onChange={selectAll}
                  className="rounded border-stone-300"
                />
                <span className="text-xs text-stone-500">全选本页</span>
              </div>

              {paginatedRuns.map(run => (
                <div
                  key={run.id}
                  className={`bg-white rounded-xl border p-4 cursor-pointer hover:shadow-md transition-shadow ${
                    selectedRuns.has(run.id)
                      ? "border-stone-900 ring-1 ring-stone-900"
                      : "border-stone-200"
                  }`}
                >
                  <div className="flex items-start gap-3">
                    <input
                      type="checkbox"
                      checked={selectedRuns.has(run.id)}
                      onChange={e => {
                        e.stopPropagation();
                        toggleSelect(run.id);
                      }}
                      className="mt-1 rounded border-stone-300"
                    />
                    <div className="flex-1" onClick={() => navigate(`/runs/${run.id}`)}>
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-3">
                          {statusIcon(run.status)}
                          <div>
                            <div className="font-medium text-stone-900">{kindLabel(run.kind)}</div>
                            <div className="text-xs text-stone-500 mt-0.5">
                              {run.userInput ? run.userInput.slice(0, 60) + "..." : "No user input"}
                            </div>
                          </div>
                        </div>
                        <div className="text-right">
                          <div className="text-xs text-stone-400 flex items-center gap-1">
                            <Clock size={12} />
                            {new Date(run.startedAt).toLocaleString()}
                          </div>
                          {run.outputPreview && (
                            <div className="text-xs text-stone-500 mt-1 max-w-xs truncate">
                              {run.outputPreview}
                            </div>
                          )}
                        </div>
                      </div>
                      {run.error && (
                        <div className="mt-2 text-xs text-red-500 bg-red-50 rounded px-2 py-1">
                          {run.error.message}
                        </div>
                      )}
                      {run.generatedProposals.length > 0 && (
                        <div className="mt-2 text-xs text-blue-600 bg-blue-50 rounded px-2 py-1">
                          {run.generatedProposals.length} 个提案
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
              <div className="flex items-center justify-between mt-6">
                <div className="text-sm text-stone-500">
                  第 {page + 1} / {totalPages} 页
                </div>
                <div className="flex gap-2">
                  <button
                    onClick={() => setPage(Math.max(0, page - 1))}
                    disabled={page === 0}
                    className="flex items-center gap-1 px-3 py-2 bg-white border border-stone-200 rounded-lg text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-stone-50 transition"
                  >
                    <ChevronLeft size={16} />
                    上一页
                  </button>
                  <button
                    onClick={() => setPage(Math.min(totalPages - 1, page + 1))}
                    disabled={page >= totalPages - 1}
                    className="flex items-center gap-1 px-3 py-2 bg-white border border-stone-200 rounded-lg text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-stone-50 transition"
                  >
                    下一页
                    <ChevronRight size={16} />
                  </button>
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
