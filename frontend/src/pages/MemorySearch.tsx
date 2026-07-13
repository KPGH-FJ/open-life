import { useEffect, useState, useCallback, useRef } from "react";
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
  createKnowledgeNote,
  getLowAccessMemoryCandidates,
  restoreArchivedMemory,
  listArchivedChunks,
  getMemoryViewModel,
  getSystemDiagnostics,
  type ArchivedCanonicalMemoryView,
  type LowAccessCanonicalMemoryCandidate,
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
  const [searchReadStatus, setSearchReadStatus] = useState<"idle" | "loading" | "ready" | "error">(
    "idle"
  );
  const [searchStatus, setSearchStatus] = useState("");
  const [showLowConfidenceResults, setShowLowConfidenceResults] = useState(false);
  const [expandedResults, setExpandedResults] = useState<Set<number>>(new Set());

  const [content, setContent] = useState("");
  const [source, setSource] = useState("manual");
  const [indexing, setIndexing] = useState(false);
  const [indexMsg, setIndexMsg] = useState("");
  const [memoryViewModel, setMemoryViewModel] = useState<MemoryViewModel | null>(null);
  const [memoryViewModelStatus, setMemoryViewModelStatus] = useState("loading");
  const [memoryViewModelWarnings, setMemoryViewModelWarnings] = useState<string[]>([]);
  const [archived, setArchived] = useState<ArchivedCanonicalMemoryView[]>([]);
  const [archiveReadStatus, setArchiveReadStatus] = useState<"loading" | "ready" | "error">(
    "loading"
  );
  const [lowAccessCandidates, setLowAccessCandidates] = useState<
    LowAccessCanonicalMemoryCandidate[]
  >([]);
  const [archiveMsg, setArchiveMsg] = useState("");
  const [archiveLoading, setArchiveLoading] = useState(false);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const indexOperationsRef = useRef<Map<string, string>>(new Map());
  const archiveLoadGenerationRef = useRef(0);
  const archiveLoadAbortRef = useRef<AbortController | null>(null);
  const searchGenerationRef = useRef(0);
  const loading = searchReadStatus === "loading";

  const loadArchiveState = useCallback(async () => {
    const generation = archiveLoadGenerationRef.current + 1;
    archiveLoadGenerationRef.current = generation;
    archiveLoadAbortRef.current?.abort();
    const controller = new AbortController();
    archiveLoadAbortRef.current = controller;
    const isCurrentGeneration = () =>
      !controller.signal.aborted && archiveLoadGenerationRef.current === generation;
    setArchiveReadStatus("loading");
    try {
      const [memoryEnvelope, archivedList, diag] = await Promise.all([
        getMemoryViewModel(),
        listArchivedChunks(20),
        getSystemDiagnostics().catch(() => null),
      ]);
      if (!isCurrentGeneration()) return;
      if (
        memoryEnvelope.data === null ||
        (memoryEnvelope.status !== "ready" && memoryEnvelope.status !== "empty")
      ) {
        throw new Error(`memory_view_model_${memoryEnvelope.status}`);
      }
      setMemoryViewModel(memoryEnvelope.data);
      setMemoryViewModelStatus(memoryEnvelope.status);
      setMemoryViewModelWarnings(
        (memoryEnvelope.warnings ?? []).map(warning => `${warning.code}: ${warning.message}`)
      );
      setArchived(archivedList);
      setArchiveReadStatus("ready");
      setDiagnostics(diag);
    } catch (error) {
      if (!isCurrentGeneration()) return;
      setMemoryViewModel(null);
      setMemoryViewModelStatus("unknown");
      setMemoryViewModelWarnings([]);
      setArchived([]);
      setArchiveReadStatus("error");
      setArchiveMsg("加载记忆层级失败: " + String(error));
      throw error;
    } finally {
      if (archiveLoadAbortRef.current === controller) {
        archiveLoadAbortRef.current = null;
      }
    }
  }, []);

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  useEffect(() => {
    loadArchiveState().catch(() => undefined);
    return () => {
      archiveLoadGenerationRef.current += 1;
      archiveLoadAbortRef.current?.abort();
      archiveLoadAbortRef.current = null;
    };
  }, [loadArchiveState]);

  const handleSearch = async () => {
    if (!query.trim()) return;
    const generation = searchGenerationRef.current + 1;
    searchGenerationRef.current = generation;
    setSearchReadStatus("loading");
    setResults([]);
    setSearchStatus("");
    try {
      const res = await searchMemory(query.trim(), 5);
      if (searchGenerationRef.current !== generation) return;
      setResults(res.hits);
      if (res.routeQuality === "identity_unknown") {
        setSearchStatus(
          "Embedding 已返回结果，但模型版本身份无法验证；当前不会缓存、写入或使用该向量进行检索。请先配置可验证的模型 revision，而不是直接重建索引。"
        );
      } else if (res.vectorStatus === "rebuild_required") {
        setSearchStatus(
          "向量索引存在未知或不兼容的 embedding profile；当前仅使用兼容向量和关键词结果，请在恢复控制台重建索引。"
        );
      } else if (res.vectorStatus === "embedding_failed") {
        setSearchStatus(
          "Embedding 服务本次调用失败；关键词结果仍可用，未将失败显示为完整语义检索。"
        );
      } else if (res.vectorStatus === "vector_search_failed") {
        setSearchStatus("向量索引本次读取失败；关键词结果仍可用，请检查恢复控制台中的存储状态。");
      } else if (res.routeQuality === "deterministic_hash_approximation") {
        setSearchStatus(
          "当前使用本地确定性哈希近似检索；它不会调用语义模型，结果质量更接近词形匹配，不能视为完整语义检索。"
        );
      }
      setShowLowConfidenceResults(false);
      setExpandedResults(new Set());
      setSearchReadStatus("ready");
    } catch (e) {
      if (searchGenerationRef.current !== generation) return;
      console.error("记忆搜索失败", e);
      setResults([]);
      setSearchStatus(buildRuntimeActionError("检索记忆", e, "data"));
      setSearchReadStatus("error");
    }
  };

  const handleIndex = async () => {
    if (!content.trim()) return;
    if (safeMode) {
      setIndexMsg(buildSafeModeBlockedMessage("收录知识笔记", diagnostics));
      return;
    }
    setIndexing(true);
    setIndexMsg("");
    const normalizedContent = content.trim();
    const payloadKey = `${source}\u0000${normalizedContent}`;
    const operationId = indexOperationsRef.current.get(payloadKey) ?? crypto.randomUUID();
    indexOperationsRef.current.set(payloadKey, operationId);
    try {
      const receipt = await createKnowledgeNote("default", normalizedContent, source, operationId);
      if (!receipt.canonicalCommitted) {
        setIndexMsg("知识笔记尚未确认提交；请使用同一内容重试，系统会复用原操作编号。");
        return;
      }
      if (indexOperationsRef.current.get(payloadKey) === operationId) {
        indexOperationsRef.current.delete(payloadKey);
      }
      setContent(current => (current.trim() === normalizedContent ? "" : current));
      const commitTruth =
        receipt.projectionState === "applied"
          ? "知识笔记已提交，检索索引已生效。"
          : receipt.projectionState === "pending"
            ? "知识笔记已提交，检索索引正在后台处理。"
            : receipt.projectionState === "degraded"
              ? "知识笔记已提交，但检索索引当前处于降级状态。"
              : receipt.projectionState === "superseded"
                ? "知识笔记已提交，但该索引投影已被更新版本取代。"
                : "知识笔记已提交，但该索引投影已进入补偿状态。";
      setIndexMsg(commitTruth);
      try {
        await loadArchiveState();
      } catch (refreshError) {
        console.error("知识笔记提交后视图刷新失败", refreshError);
        setIndexMsg(`${commitTruth} 当前视图刷新失败，可稍后重试刷新；提交事实不受影响。`);
      }
    } catch (e) {
      setIndexMsg(
        `尚未确认知识笔记是否提交；再次提交相同内容会复用原操作编号。${buildRuntimeActionError("提交知识笔记", e, "data")}`
      );
    } finally {
      setIndexing(false);
    }
  };

  const handleArchiveLowAccess = async () => {
    if (safeMode) {
      setArchiveMsg(buildSafeModeBlockedMessage("记忆归档", diagnostics));
      return;
    }
    setArchiveLoading(true);
    setArchiveMsg("");
    try {
      const candidates = await getLowAccessMemoryCandidates();
      setLowAccessCandidates(candidates);
      setArchiveMsg(
        candidates.length === 0
          ? "当前没有经过 canonical owner 验证的低访问候选；未执行任何归档。"
          : `发现 ${candidates.length} 条低访问候选；这只是本地访问指标建议，尚未归档，也没有改变 canonical Memory。请从对话或审阅收件箱发起精确归档。`
      );
    } catch (e) {
      setArchiveMsg(buildRuntimeActionError("归档低访问记忆", e, "data"));
    } finally {
      setArchiveLoading(false);
    }
  };

  const handleRestore = useCallback(
    async (chunk: ArchivedCanonicalMemoryView) => {
      if (safeMode) {
        setArchiveMsg(buildSafeModeBlockedMessage("归档记忆恢复", diagnostics));
        return;
      }
      if (!confirm(`确定恢复这条归档记忆吗？\n\n${chunk.owner.ownerKind}:${chunk.owner.ownerId}`))
        return;
      setArchiveLoading(true);
      setArchiveMsg("");
      try {
        const receipt = await restoreArchivedMemory(chunk.owner);
        if (!receipt.canonicalCommitted) {
          setArchiveMsg("恢复请求尚未确认写入 canonical Memory；当前状态保持未知。请稍后重试。");
          return;
        }
        const projectionMessage =
          receipt.projectionState === "applied"
            ? "检索投影已生效。"
            : receipt.projectionState === "pending"
              ? "检索投影正在后台处理。"
              : receipt.projectionState === "degraded"
                ? "canonical 状态已恢复，但检索投影当前降级。"
                : receipt.projectionState === "superseded"
                  ? "该投影已被更新版本取代。"
                  : "投影已按当前 canonical 状态完成补偿。";
        setArchiveMsg(
          receipt.changed
            ? `归档记忆已恢复；${projectionMessage}`
            : `记忆原本已处于可检索状态；${projectionMessage}`
        );
        await loadArchiveState();
      } catch (e) {
        setArchiveMsg(buildRuntimeActionError("恢复归档记忆", e, "data"));
      } finally {
        setArchiveLoading(false);
      }
    },
    [safeMode, diagnostics, loadArchiveState]
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
  const memoryTruthKnown =
    archiveReadStatus === "ready" &&
    memoryViewModel !== null &&
    (memoryViewModelStatus === "ready" || memoryViewModelStatus === "empty");
  const knownCount = (value: number | null | undefined) =>
    memoryTruthKnown && typeof value === "number" ? value : "—";

  return (
    <div className="h-full overflow-auto bg-white">
      <div className="max-w-4xl mx-auto p-6 space-y-8">
        {safeMode && (
          <section className="rounded-2xl border border-amber-200 bg-amber-50 px-4 py-4">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div>
                <div className="text-sm font-semibold text-amber-900">
                  Safe Mode：记忆写入操作已暂停
                </div>
                <div className="mt-1 text-xs leading-5 text-amber-800">{safeModeReason}</div>
                <div className="mt-1 text-xs text-amber-700">
                  搜索和查看仍可继续，但知识笔记收录、低访问归档等写操作已暂停。
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
            这个页面从后台 MemoryViewModel 读取生命周期、审阅
            和物化状态；向量层级只是存储遥测，不代表长期记忆已经生效。
          </div>
          <div className="mt-3 grid gap-3 md:grid-cols-3">
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">ReadModel</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {archiveReadStatus === "loading"
                  ? "loading"
                  : memoryTruthKnown
                    ? memoryViewModelStatus
                    : "unknown"}
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">已物化记忆</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {knownCount(memorySummary?.materializedCount)}
              </div>
            </div>
            <div className="rounded-xl border border-white bg-white px-3 py-3">
              <div className="text-[11px] font-medium text-slate-500">待确认/待物化</div>
              <div className="mt-1 text-lg font-semibold text-slate-900">
                {knownCount(memorySummary?.reviewRequiredCount)} /{" "}
                {knownCount(memorySummary?.pendingMaterializationCount)}
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
            <Database size={18} /> 手动收录知识笔记
          </h2>
          <p className="text-xs leading-5 text-slate-500">
            KnowledgeNote 是独立的可检索资料，不等于已接受的长期用户事实，也不会直接修改
            LifeModel。需要 OpenLife 记住的个人事实请从对话发起，并按回执或审阅收件箱的状态确认。
          </p>
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
              placeholder="输入要收录的知识笔记..."
              className="flex-1 border rounded-lg px-3 py-2 text-sm"
            />
            <button
              onClick={handleIndex}
              disabled={indexing || !content.trim() || safeMode}
              className="bg-indigo-600 text-white px-4 py-2 rounded-lg text-sm hover:bg-indigo-700 disabled:opacity-50"
            >
              {indexing ? <Loader2 size={16} className="animate-spin" /> : "收录"}
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
              {archiveLoading ? "扫描中..." : "查看低访问候选"}
            </button>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            {[
              ["候选", knownCount(lifecycleSummary?.candidateCount)],
              ["待审阅", knownCount(lifecycleSummary?.pendingReviewCount)],
              ["已确认", knownCount(lifecycleSummary?.confirmedCount)],
              ["已回滚", knownCount(lifecycleSummary?.rolledBackCount)],
              ["物化失败", knownCount(lifecycleSummary?.materializationFailedCount)],
            ].map(([label, value]) => (
              <div key={label} className="rounded-xl border border-slate-200 bg-white p-3">
                <div className="text-xs text-slate-500">{label}</div>
                <div className="mt-1 text-lg font-semibold text-slate-900">{value}</div>
              </div>
            ))}
          </div>
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            {[
              ["向量总数", knownCount(tierStats?.total)],
              ["Tier 1 热记忆", knownCount(tierStats?.tier1)],
              ["Tier 2 检索记忆", knownCount(tierStats?.tier2)],
              ["Tier 3 冷记忆", knownCount(tierStats?.tier3)],
              ["已归档", knownCount(tierStats?.archived)],
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
          {lowAccessCandidates.length > 0 && (
            <div className="space-y-2" aria-label="低访问候选">
              {lowAccessCandidates.map(candidate => (
                <div
                  key={`${candidate.owner.ownerKind}:${candidate.owner.ownerId}`}
                  className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3"
                >
                  <div className="text-xs font-medium text-amber-900">
                    {candidate.owner.ownerKind}:{candidate.owner.ownerId}
                  </div>
                  <div className="mt-1 text-xs text-amber-800">
                    Tier {candidate.tier} · 访问 {candidate.accessCount} 次 · 重要度{` `}
                    {candidate.importanceScore.toFixed(2)} · 仅候选，未归档
                  </div>
                </div>
              ))}
            </div>
          )}
          <div className="space-y-2">
            {archiveReadStatus === "loading" ? (
              <EmptyState
                title="正在读取归档事实"
                description="正在向 canonical Memory owner 核对归档状态。"
                className="py-4"
              />
            ) : archiveReadStatus === "error" ? (
              <EmptyState
                title="归档状态未知"
                description="canonical Memory owner 当前不可用；这里不会把未知状态显示为没有归档。"
                className="py-4"
              />
            ) : archived.length === 0 ? (
              <EmptyState
                title="暂无归档记忆"
                description="只有 canonical Memory 的明确归档事实会显示在这里；访问指标候选不会自动进入归档。"
                className="py-4"
              />
            ) : (
              archived.map(chunk => (
                <div
                  key={`${chunk.owner.ownerKind}:${chunk.owner.ownerId}`}
                  className="rounded-lg border border-slate-200 bg-white p-4"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
                      <div className="text-xs font-medium text-slate-500">
                        {chunk.owner.ownerKind} · canonical revision {chunk.revision}
                      </div>
                      <div className="mt-1 text-sm text-slate-800 whitespace-pre-wrap">
                        {chunk.owner.ownerId}
                      </div>
                      <div className="mt-2 text-xs text-slate-400">
                        归档于 {new Date(chunk.changedAt).toLocaleString()} ·{` `}
                        {chunk.canonicalDisposition}
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
              onChange={e => {
                searchGenerationRef.current += 1;
                setQuery(e.target.value);
                setResults([]);
                setSearchStatus("");
                setSearchReadStatus("idle");
                setShowLowConfidenceResults(false);
                setExpandedResults(new Set());
              }}
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
          {searchStatus && (
            <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              {searchStatus}
            </p>
          )}

          <div className="space-y-3">
            {searchReadStatus === "idle" && query.trim() && (
              <EmptyState
                title="等待搜索"
                description="提交查询后，才会显示后端返回的检索事实。"
                className="py-4"
              />
            )}
            {searchReadStatus === "loading" && (
              <EmptyState
                title="正在检索记忆"
                description="结果尚未返回，当前命中状态未知。"
                className="py-4"
              />
            )}
            {searchReadStatus === "error" && (
              <EmptyState
                title="检索状态未知"
                description="后端未返回可验证结果；失败不能解释为零命中。"
                className="py-4"
              />
            )}
            {searchReadStatus === "ready" && results.length === 0 && (
              <EmptyState
                title="未找到相关记忆"
                description="尝试换一组关键词再次搜索。"
                className="py-4"
              />
            )}
            {searchReadStatus === "ready" && hiddenLowConfidenceCount > 0 && (
              <button
                type="button"
                onClick={() => setShowLowConfidenceResults(true)}
                className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-800 hover:bg-amber-100"
              >
                显示 {hiddenLowConfidenceCount} 条低相关结果
              </button>
            )}
            {searchReadStatus === "ready" &&
              visibleResults.map(r => {
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
