import { useEffect, useState, useCallback } from "react";
import { Link } from "react-router-dom";
import {
  Search,
  Database,
  Loader2,
  Archive,
  RotateCcw,
  MessageSquare,
  User,
  Bot,
  Tag,
} from "lucide-react";
import {
  searchMemory,
  indexMemoryChunk,
  archiveLowAccessMemories,
  restoreArchivedChunks,
  listArchivedChunks,
  getMemoryViewModel,
  getSystemDiagnostics,
  type ArchivedChunkSummary,
  type MemoryViewModel,
  type SystemDiagnostics,
} from "../tauri";
import EmptyState from "../components/EmptyState";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";
import { productRoutePath } from "../productShellContract";

interface MemoryResult {
  chunk: {
    id: number;
    session_id: string;
    content: string;
    source: string;
    created_at: string;
  };
  score: number;
}

export default function MemorySearch() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MemoryResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [showLowConfidenceResults, setShowLowConfidenceResults] = useState(false);
  const [expandedResults, setExpandedResults] = useState<Set<number>>(new Set());

  const [content, setContent] = useState("");
  const [source, setSource] = useState("manual");
  const [indexing, setIndexing] = useState(false);
  const [indexMsg, setIndexMsg] = useState("");
  const [memoryViewModel, setMemoryViewModel] = useState<MemoryViewModel | null>(null);
  const [memoryViewModelStatus, setMemoryViewModelStatus] = useState("loading");
  const [memoryViewModelWarnings, setMemoryViewModelWarnings] = useState<string[]>([]);
  const [archived, setArchived] = useState<ArchivedChunkSummary[]>([]);
  const [archiveMsg, setArchiveMsg] = useState("");
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);

  const loadArchiveState = async () => {
    const [memoryEnvelope, archivedList, diag] = await Promise.all([
      getMemoryViewModel(),
      listArchivedChunks(20),
      getSystemDiagnostics().catch(() => null),
    ]);
    setMemoryViewModel(memoryEnvelope.data);
    setMemoryViewModelStatus(memoryEnvelope.status);
    setMemoryViewModelWarnings(
      (memoryEnvelope.warnings ?? []).map(warning => `${warning.code}: ${warning.message}`)
    );
    setArchived(archivedList);
    setDiagnostics(diag);
  };

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  useEffect(() => {
    loadArchiveState().catch(e => setArchiveMsg("加载记忆层级失败: " + String(e)));
  }, []);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    try {
      const res = await searchMemory(query.trim(), 5);
      setResults(res);
      setShowLowConfidenceResults(false);
      setExpandedResults(new Set());
    } catch (e) {
      console.error("记忆搜索失败", e);
    } finally {
      setLoading(false);
    }
  };

  const handleIndex = async () => {
    if (!content.trim()) return;
    if (safeMode) {
      setIndexMsg(buildSafeModeBlockedMessage("手动索引", diagnostics));
      return;
    }
    setIndexing(true);
    setIndexMsg("");
    try {
      await indexMemoryChunk("default", content.trim(), source);
      setIndexMsg("索引成功");
      setContent("");
      await loadArchiveState();
    } catch (e) {
      setIndexMsg(buildRuntimeActionError("写入记忆索引", e, "data"));
    } finally {
      setIndexing(false);
    }
  };

  const handleArchiveLowAccess = async () => {
    if (safeMode) {
      setArchiveMsg(buildSafeModeBlockedMessage("记忆归档", diagnostics));
      return;
    }
    if (!confirm("确定归档低访问、低重要性的旧记忆吗？归档后仍可在下方恢复。")) return;
    setArchiveLoading(true);
    setArchiveMsg("");
    try {
      const count = await archiveLowAccessMemories();
      setArchiveMsg(`已归档 ${count} 条低访问记忆`);
      await loadArchiveState();
    } catch (e) {
      setArchiveMsg(buildRuntimeActionError("归档低访问记忆", e, "data"));
    } finally {
      setArchiveLoading(false);
    }
  };

  const handleRestore = useCallback(
    async (chunk: ArchivedChunkSummary) => {
      if (safeMode) {
        setArchiveMsg(buildSafeModeBlockedMessage("归档记忆恢复", diagnostics));
        return;
      }
      if (!confirm(`确定恢复这条归档记忆吗？\n\n${chunk.summary || chunk.content.slice(0, 80)}`))
        return;
      setArchiveLoading(true);
      setArchiveMsg("");
      try {
        const count = await restoreArchivedChunks([chunk.id]);
        setArchiveMsg(`已恢复 ${count} 条记忆`);
        await loadArchiveState();
      } catch (e) {
        setArchiveMsg(buildRuntimeActionError("恢复归档记忆", e, "data"));
      } finally {
        setArchiveLoading(false);
      }
    },
    [safeMode, diagnostics]
  );

  const normalizedQuery = query.trim().toLowerCase();
  const sortedResults = [...results].sort((a, b) => {
    const aExact =
      normalizedQuery.length > 0 && a.chunk.content.toLowerCase().includes(normalizedQuery);
    const bExact =
      normalizedQuery.length > 0 && b.chunk.content.toLowerCase().includes(normalizedQuery);
    if (aExact !== bExact) return aExact ? -1 : 1;
    return b.score - a.score;
  });
  const visibleResults = sortedResults.filter(
    result => showLowConfidenceResults || result.score >= 0.3
  );
  const hiddenLowConfidenceCount = sortedResults.length - visibleResults.length;
  const tierStats = memoryViewModel?.summary.tierSummary ?? null;
  const lifecycleSummary = memoryViewModel?.lifecycleSummary ?? null;
  const memorySummary = memoryViewModel?.summary ?? null;

  return (
    <div className="h-full overflow-auto bg-white">
      <div className="max-w-4xl mx-auto p-6 space-y-8">
        {safeMode && (
          <section className="rounded-2xl border border-amber-200 bg-amber-50 px-4 py-4">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <div className="text-sm font-semibold text-amber-900">
                  Safe Mode：记忆写入操作已建议暂停
                </div>
                <div className="mt-1 text-xs leading-5 text-amber-800">{safeModeReason}</div>
                <div className="mt-1 text-xs text-amber-700">
                  搜索和查看仍可继续，但手动索引、低访问归档等写操作建议先暂停。
                </div>
              </div>
              <Link
                to={productRoutePath("Settings")}
                className="inline-flex shrink-0 items-center justify-center rounded-full bg-amber-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-amber-950"
              >
                打开恢复控制台
              </Link>
            </div>
          </section>
        )}

        <section className="rounded-2xl border border-slate-200 bg-slate-50/80 px-4 py-4">
          <div className="text-sm font-semibold text-slate-900">记忆治理说明</div>
          <div className="mt-1 text-xs leading-5 text-slate-600">
            这个页面从后台 MemoryViewModel 读取生命周期、Review
            和物化状态；向量层级只是存储遥测，不代表长期记忆已经生效。
          </div>
          <div className="mt-3 grid gap-3 md:grid-cols-3">
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">ReadModel</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {memoryViewModelStatus}
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">已物化记忆</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {memorySummary?.materializedCount ?? 0}
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">待确认/待物化</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {memorySummary?.reviewRequiredCount ?? 0} /{" "}
                {memorySummary?.pendingMaterializationCount ?? 0}
              </div>
            </div>
          </div>
          {memoryViewModelWarnings.length > 0 && (
            <div className="mt-3 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs leading-5 text-amber-900">
              {memoryViewModelWarnings.slice(0, 2).map(warning => (
                <div key={warning}>{warning}</div>
              ))}
            </div>
          )}
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <Database size={18} /> 手动索引记忆
          </h2>
          <div className="flex gap-3">
            <input
              value={source}
              onChange={e => setSource(e.target.value)}
              placeholder="来源标签"
              className="border rounded-lg px-3 py-2 text-sm w-40"
            />
            <input
              value={content}
              onChange={e => setContent(e.target.value)}
              placeholder="输入要索引的记忆内容..."
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleIndex}
              disabled={indexing || !content.trim() || safeMode}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50"
            >
              {indexing ? <Loader2 size={16} className="animate-spin" /> : "索引"}
            </button>
          </div>
          {indexMsg && <p className="text-sm text-gray-600">{indexMsg}</p>}
        </section>

        <section className="space-y-3">
          <div className="flex items-center justify-between gap-3">
            <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
              <Archive size={18} /> 记忆层级与归档
            </h2>
            <button
              onClick={handleArchiveLowAccess}
              disabled={archiveLoading || safeMode}
              className="rounded-lg bg-slate-800 px-3 py-2 text-sm text-white hover:bg-slate-900 disabled:opacity-50"
            >
              {archiveLoading ? "处理中..." : "归档低访问记忆"}
            </button>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            {[
              ["候选", lifecycleSummary?.candidateCount ?? 0],
              ["待审阅", lifecycleSummary?.pendingReviewCount ?? 0],
              ["已确认", lifecycleSummary?.confirmedCount ?? 0],
              ["已回滚", lifecycleSummary?.rolledBackCount ?? 0],
              ["物化失败", lifecycleSummary?.materializationFailedCount ?? 0],
            ].map(([label, value]) => (
              <div key={label} className="rounded-xl border border-slate-200 bg-white p-3">
                <div className="text-xs text-slate-500">{label}</div>
                <div className="mt-1 text-lg font-semibold text-slate-900">{value}</div>
              </div>
            ))}
          </div>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            {[
              ["向量总数", tierStats?.total ?? 0],
              ["Tier 1 热记忆", tierStats?.tier1 ?? 0],
              ["Tier 2 检索记忆", tierStats?.tier2 ?? 0],
              ["Tier 3 冷记忆", tierStats?.tier3 ?? 0],
              ["已归档", tierStats?.archived ?? 0],
            ].map(([label, value]) => (
              <div key={label} className="rounded-xl border border-slate-200 bg-slate-50 p-3">
                <div className="text-xs text-slate-500">{label}</div>
                <div className="mt-1 text-lg font-semibold text-slate-900">{value}</div>
              </div>
            ))}
          </div>
          <div className="rounded-lg border border-indigo-100 bg-indigo-50 px-4 py-3 text-xs leading-5 text-indigo-800">
            读取策略可以简单理解为：热记忆更容易被优先检索，冷记忆保留但不总是优先出现，归档记忆则需要你显式恢复后再重新进入主检索层。
          </div>
          {archiveMsg && (
            <p className="text-sm text-slate-600" data-testid="archive-msg">
              {archiveMsg}
            </p>
          )}
          <div className="space-y-2">
            {archived.length === 0 ? (
              <EmptyState
                title="暂无归档记忆"
                description="低访问记忆归档后会显示在这里，可随时恢复。"
                className="py-4"
              />
            ) : (
              archived.map(chunk => (
                <div key={chunk.id} className="rounded-lg border border-slate-200 bg-white p-4">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-xs font-medium text-slate-500">
                        {chunk.source} · 访问 {chunk.access_count} 次 · 重要度{" "}
                        {chunk.importance_score.toFixed(2)}
                      </div>
                      <div className="mt-1 text-sm text-slate-800 whitespace-pre-wrap">
                        {chunk.summary || chunk.content.slice(0, 200)}
                      </div>
                      <div className="mt-2 text-xs text-slate-400">
                        归档于 {new Date(chunk.archived_at).toLocaleString()}
                      </div>
                    </div>
                    <button
                      onClick={() => handleRestore(chunk)}
                      disabled={archiveLoading}
                      className="inline-flex shrink-0 items-center gap-1 rounded-md border border-indigo-200 bg-indigo-50 px-3 py-1.5 text-xs text-indigo-700 hover:bg-indigo-100 disabled:opacity-50"
                    >
                      <RotateCcw size={12} /> 恢复
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <Search size={18} /> 语义检索记忆
          </h2>
          <div className="flex gap-3">
            <input
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={e => {
                if (e.key === "Enter") handleSearch();
              }}
              placeholder="输入查询语义..."
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleSearch}
              disabled={loading || !query.trim()}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50"
            >
              {loading ? <Loader2 size={16} className="animate-spin" /> : "搜索"}
            </button>
          </div>

          <div className="space-y-3">
            {results.length === 0 && !loading && query && (
              <EmptyState
                title="未找到相关记忆"
                description="尝试换一组关键词再次搜索。"
                className="py-4"
              />
            )}
            {hiddenLowConfidenceCount > 0 && (
              <button
                type="button"
                onClick={() => setShowLowConfidenceResults(true)}
                className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-800 hover:bg-amber-100"
              >
                显示 {hiddenLowConfidenceCount} 条低相关结果
              </button>
            )}
            {visibleResults.map(r => {
              const sourceIcon =
                r.chunk.source === "chat" || r.chunk.source === "assistant" ? (
                  <Bot size={12} />
                ) : r.chunk.source === "user" ? (
                  <User size={12} />
                ) : r.chunk.source === "builder" ? (
                  <Tag size={12} />
                ) : (
                  <Database size={12} />
                );
              const sourceLabel =
                r.chunk.source === "chat" || r.chunk.source === "assistant"
                  ? "AI 回复"
                  : r.chunk.source === "user"
                    ? "用户输入"
                    : r.chunk.source === "builder"
                      ? "构建过程"
                      : r.chunk.source === "manual"
                        ? "手动添加"
                        : r.chunk.source;
              const expanded = expandedResults.has(r.chunk.id);
              const contentPreview =
                expanded || r.chunk.content.length <= 240
                  ? r.chunk.content
                  : `${r.chunk.content.slice(0, 240).trimEnd()}...`;
              const scoreBand = r.score >= 0.7 ? "高相关" : r.score >= 0.3 ? "中相关" : "低相关";
              const exactMatch =
                normalizedQuery.length > 0 &&
                r.chunk.content.toLowerCase().includes(normalizedQuery);
              return (
                <div
                  key={r.chunk.id}
                  className="border rounded-lg p-4 bg-gray-50 hover:bg-gray-100 transition"
                >
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <span className="inline-flex items-center gap-1 rounded-full bg-indigo-50 px-2 py-0.5 text-[10px] font-medium text-indigo-700 border border-indigo-100">
                        {sourceIcon}
                        {sourceLabel}
                      </span>
                      {r.chunk.session_id && r.chunk.session_id !== "default" && (
                        <span className="text-[10px] text-gray-400 flex items-center gap-1">
                          <MessageSquare size={10} />
                          会话 {r.chunk.session_id.slice(0, 8)}...
                        </span>
                      )}
                    </div>
                    <span className="text-xs text-gray-500">
                      {scoreBand} · {Math.round(r.score * 100)}%
                    </span>
                  </div>
                  {exactMatch && (
                    <div className="mb-2 inline-flex rounded-full border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-[10px] font-medium text-emerald-700">
                      包含精确查询文本
                    </div>
                  )}
                  <p className="text-sm text-gray-800 whitespace-pre-wrap">{contentPreview}</p>
                  {r.chunk.content.length > 240 && (
                    <button
                      type="button"
                      onClick={() =>
                        setExpandedResults(prev => {
                          const next = new Set(prev);
                          if (next.has(r.chunk.id)) {
                            next.delete(r.chunk.id);
                          } else {
                            next.add(r.chunk.id);
                          }
                          return next;
                        })
                      }
                      className="mt-2 text-xs font-medium text-indigo-600 hover:text-indigo-800"
                    >
                      {expanded ? "收起" : "展开完整内容"}
                    </button>
                  )}
                  <p className="text-xs text-gray-400 mt-2">
                    {new Date(r.chunk.created_at).toLocaleString("zh-CN", {
                      month: "short",
                      day: "numeric",
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </p>
                </div>
              );
            })}
          </div>
        </section>
      </div>
    </div>
  );
}
