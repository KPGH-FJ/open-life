import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { Search, Database, Loader2, Archive, RotateCcw, MessageSquare, User, Bot, Tag } from "lucide-react";
import {
  searchMemory,
  indexMemoryChunk,
  archiveLowAccessMemories,
  restoreArchivedChunks,
  listArchivedChunks,
  getMemoryTierStats,
  getSystemDiagnostics,
  type ArchivedChunkSummary,
  type SystemDiagnostics,
  type TierStats,
} from "../tauri";
import EmptyState from "../components/EmptyState";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";

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

  const [content, setContent] = useState("");
  const [source, setSource] = useState("manual");
  const [indexing, setIndexing] = useState(false);
  const [indexMsg, setIndexMsg] = useState("");
  const [tierStats, setTierStats] = useState<TierStats | null>(null);
  const [archived, setArchived] = useState<ArchivedChunkSummary[]>([]);
  const [archiveMsg, setArchiveMsg] = useState("");
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);

  const loadArchiveState = async () => {
    const [stats, archivedList, diag] = await Promise.all([
      getMemoryTierStats(),
      listArchivedChunks(20),
      getSystemDiagnostics().catch(() => null),
    ]);
    setTierStats(stats);
    setArchived(archivedList);
    setDiagnostics(diag);
  };

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  useEffect(() => {
    loadArchiveState().catch((e) => setArchiveMsg("加载记忆层级失败: " + String(e)));
  }, []);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    try {
      const res = await searchMemory(query.trim(), 5);
      setResults(res);
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

  const handleRestore = async (chunk: ArchivedChunkSummary) => {
    if (safeMode) {
      setArchiveMsg(buildSafeModeBlockedMessage("归档记忆恢复", diagnostics));
      return;
    }
    if (!confirm(`确定恢复这条归档记忆吗？\n\n${chunk.summary || chunk.content.slice(0, 80)}`)) return;
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
  };

  return (
    <div className="h-full overflow-auto bg-white">
      <div className="max-w-4xl mx-auto p-6 space-y-8">
        {safeMode && (
          <section className="rounded-2xl border border-amber-200 bg-amber-50 px-4 py-4">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <div className="text-sm font-semibold text-amber-900">Safe Mode：记忆写入操作已建议暂停</div>
                <div className="mt-1 text-xs leading-5 text-amber-800">
                  {safeModeReason}
                </div>
                <div className="mt-1 text-xs text-amber-700">
                  搜索和查看仍可继续，但手动索引、低访问归档等写操作建议先暂停。
                </div>
              </div>
              <Link
                to="/settings"
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
            这个页面负责回答三个问题：系统现在记住了什么、哪些记忆已经沉到归档层、以及哪些内容值得重新恢复到活跃层。
          </div>
          <div className="mt-3 grid gap-3 md:grid-cols-3">
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">搜索记忆</div>
              <div className="mt-1 text-xs leading-5 text-slate-700">
                用语义检索确认系统到底记住了什么，避免“以为被记住，其实没有进入长期记忆”。
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">归档与恢复</div>
              <div className="mt-1 text-xs leading-5 text-slate-700">
                低访问记忆会逐渐沉到归档层；如果某段历史又重新重要，可以在这里恢复，而不是重新手工输入。
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">风险边界</div>
              <div className="mt-1 text-xs leading-5 text-slate-700">
                如果当前处于 Safe Mode，写入和层级维护会先暂停；先修复数据环境，再做记忆治理更安全。
              </div>
            </div>
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-lg font-semibold text-gray-800 flex items-center gap-2">
            <Database size={18} /> 手动索引记忆
          </h2>
          <div className="flex gap-3">
            <input
              value={source}
              onChange={(e) => setSource(e.target.value)}
              placeholder="来源标签"
              className="border rounded-lg px-3 py-2 text-sm w-40"
            />
            <input
              value={content}
              onChange={(e) => setContent(e.target.value)}
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
              ["活跃总数", tierStats?.total ?? 0],
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
          {archiveMsg && <p className="text-sm text-slate-600">{archiveMsg}</p>}
          <div className="space-y-2">
            {archived.length === 0 ? (
              <EmptyState title="暂无归档记忆" description="低访问记忆归档后会显示在这里，可随时恢复。" className="py-4" />
            ) : (
              archived.map((chunk) => (
                <div key={chunk.id} className="rounded-lg border border-slate-200 bg-white p-4">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-xs font-medium text-slate-500">
                        {chunk.source} · 访问 {chunk.access_count} 次 · 重要度 {chunk.importance_score.toFixed(2)}
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
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
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
              <EmptyState title="未找到相关记忆" description="尝试换一组关键词再次搜索。" className="py-4" />
            )}
            {results.map((r, idx) => {
              const sourceIcon = r.chunk.source === "chat" || r.chunk.source === "assistant" ? <Bot size={12} /> :
                r.chunk.source === "user" ? <User size={12} /> :
                r.chunk.source === "builder" ? <Tag size={12} /> :
                <Database size={12} />;
              const sourceLabel = r.chunk.source === "chat" || r.chunk.source === "assistant" ? "AI 回复" :
                r.chunk.source === "user" ? "用户输入" :
                r.chunk.source === "builder" ? "构建过程" :
                r.chunk.source === "manual" ? "手动添加" :
                r.chunk.source;
              return (
                <div
                  key={idx}
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
                      相关度 {Math.round(r.score * 100)}%
                    </span>
                  </div>
                  <p className="text-sm text-gray-800 whitespace-pre-wrap">{r.chunk.content}</p>
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
