import { useEffect, useState } from "react";
import { listSnapshots, restoreSnapshot, diffSnapshots, createSnapshot } from "../tauri";
import type { LifeModelVersion } from "../types";
import LoadingSpinner from "../components/LoadingSpinner";
import EmptyState from "../components/EmptyState";

type DiffLine = {
  sign: "+" | "-" | " ";
  text: string;
  dim?: "identity" | "goals" | "capabilities" | "state" | "other";
};

type DiffSummary = {
  totalChanges: number;
  adds: number;
  removes: number;
  dims: Array<{ dim: NonNullable<DiffLine["dim"]>; count: number }>;
  highlights: string[];
};

function parseStructuredDiff(diffText: string): DiffLine[] {
  const lines = diffText.split("\n");
  const result: DiffLine[] = [];
  let currentDim: DiffLine["dim"] = "other";
  let indentStack: { indent: number; dim: DiffLine["dim"] }[] = [];

  for (const raw of lines) {
    const sign: DiffLine["sign"] = raw.startsWith("+") ? "+" : raw.startsWith("-") ? "-" : " ";
    const text = raw.slice(1);
    const leadingSpaces = text.match(/^(\s*)/)?.[1].length ?? 0;

    // Detect top-level keys to determine dimension
    const trimmed = text.trimStart();
    if (trimmed.startsWith("identity:") || trimmed.startsWith("'identity':") || trimmed.startsWith('"identity":')) {
      currentDim = "identity";
      indentStack = [{ indent: leadingSpaces, dim: "identity" }];
    } else if (trimmed.startsWith("goals:") || trimmed.startsWith("'goals':") || trimmed.startsWith('"goals":')) {
      currentDim = "goals";
      indentStack = [{ indent: leadingSpaces, dim: "goals" }];
    } else if (trimmed.startsWith("capabilities:") || trimmed.startsWith("'capabilities':") || trimmed.startsWith('"capabilities":')) {
      currentDim = "capabilities";
      indentStack = [{ indent: leadingSpaces, dim: "capabilities" }];
    } else if (trimmed.startsWith("state:") || trimmed.startsWith("'state':") || trimmed.startsWith('"state":')) {
      currentDim = "state";
      indentStack = [{ indent: leadingSpaces, dim: "state" }];
    } else if (trimmed.startsWith("metadata:") || trimmed.startsWith("evolution_rules:") || trimmed.startsWith("health_status:")) {
      currentDim = "other";
      indentStack = [{ indent: leadingSpaces, dim: "other" }];
    } else {
      // Pop stack if we dedented
      while (indentStack.length > 0 && leadingSpaces <= indentStack[indentStack.length - 1].indent) {
        indentStack.pop();
      }
      if (indentStack.length > 0) {
        currentDim = indentStack[indentStack.length - 1].dim;
      }
      if (trimmed.length > 0) {
        indentStack.push({ indent: leadingSpaces, dim: currentDim });
      }
    }

    result.push({ sign, text: raw, dim: currentDim });
  }
  return result;
}

function dimBadgeClass(dim?: DiffLine["dim"]) {
  switch (dim) {
    case "identity":
      return "border-l-4 border-pink-400 bg-pink-50/30";
    case "goals":
      return "border-l-4 border-blue-400 bg-blue-50/30";
    case "capabilities":
      return "border-l-4 border-amber-400 bg-amber-50/30";
    case "state":
      return "border-l-4 border-emerald-400 bg-emerald-50/30";
    default:
      return "border-l-4 border-gray-300";
  }
}

function tagBadgeClass(tag: string) {
  const t = tag.toLowerCase();
  if (t.includes("evolution")) return "bg-purple-100 text-purple-700";
  if (t.includes("calibration")) return "bg-indigo-100 text-indigo-700";
  if (t.includes("builder") || t.includes("quick") || t.includes("socratic")) return "bg-emerald-100 text-emerald-700";
  if (t.includes("progressive") || t.includes("incremental")) return "bg-amber-100 text-amber-700";
  if (t.includes("auto")) return "bg-gray-100 text-gray-700";
  return "bg-slate-100 text-slate-700";
}

function tagLabel(tag: string) {
  const t = tag.toLowerCase();
  if (t.includes("evolution")) return "进化";
  if (t.includes("calibration")) return "校准";
  if (t.includes("quick")) return "快速构建";
  if (t.includes("socratic")) return "苏格拉底";
  if (t.includes("progressive") || t.includes("incremental")) return "渐进构建";
  if (t.includes("builder")) return "构建";
  if (t.includes("auto")) return "自动";
  return tag || "手动";
}

function summarizeStructuredDiff(lines: DiffLine[]): DiffSummary {
  const changed = lines.filter((line) => line.sign !== " ");
  const dimsMap = new Map<NonNullable<DiffLine["dim"]>, number>();
  for (const line of changed) {
    const dim = line.dim ?? "other";
    dimsMap.set(dim, (dimsMap.get(dim) ?? 0) + 1);
  }
  return {
    totalChanges: changed.length,
    adds: changed.filter((line) => line.sign === "+").length,
    removes: changed.filter((line) => line.sign === "-").length,
    dims: Array.from(dimsMap.entries())
      .map(([dim, count]) => ({ dim, count }))
      .sort((a, b) => b.count - a.count),
    highlights: changed
      .map((line) => line.text.replace(/^[+-]\s*/, "").trim())
      .filter((text) => text.length > 0 && !text.endsWith(":"))
      .slice(0, 4),
  };
}

function dimLabel(dim: NonNullable<DiffLine["dim"]>) {
  switch (dim) {
    case "identity":
      return "身份";
    case "goals":
      return "目标";
    case "capabilities":
      return "能力";
    case "state":
      return "状态";
    default:
      return "其他";
  }
}

export default function VersionControl() {
  const [snapshots, setSnapshots] = useState<LifeModelVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [tag, setTag] = useState("");
  const [note, setNote] = useState("");
  const [creating, setCreating] = useState(false);
  const [notice, setNotice] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [diffText, setDiffText] = useState<string>("");
  const [diffing, setDiffing] = useState(false);
  const [structuredDiff, setStructuredDiff] = useState<DiffLine[]>([]);
  const diffSummary = summarizeStructuredDiff(structuredDiff);

  const load = async () => {
    setLoading(true);
    try {
      const data = await listSnapshots();
      setSnapshots(data);
    } catch (e) {
      setNotice("加载失败: " + String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleCreate = async () => {
    setCreating(true);
    try {
      await createSnapshot(tag || "手动快照", note || "");
      setTag("");
      setNote("");
      setNotice("快照创建成功");
      await load();
    } catch (e) {
      setNotice("快照创建失败: " + String(e));
    } finally {
      setCreating(false);
    }
  };

  const handleRestore = async (version: string) => {
    if (!confirm(`确定要回滚到版本 ${version} 吗？\n\n系统会先自动创建 pre-restore 备份快照，再恢复目标版本。`)) return;
    try {
      await restoreSnapshot(version);
      setNotice(`回滚成功，系统已先自动备份当前版本，再恢复到 ${version}`);
    } catch (e) {
      setNotice("回滚失败: " + String(e));
    }
  };

  const toggleSelect = (version: string) => {
    setSelected((prev) => {
      if (prev.includes(version)) {
        return prev.filter((v) => v !== version);
      }
      if (prev.length >= 2) {
        return [prev[1], version];
      }
      return [...prev, version];
    });
  };

  const handleDiff = async () => {
    if (selected.length !== 2) return;
    setDiffing(true);
    try {
      const text = await diffSnapshots(selected[0], selected[1]);
      setDiffText(text);
      setStructuredDiff(parseStructuredDiff(text));
    } catch (e) {
      setNotice("对比失败: " + String(e));
    } finally {
      setDiffing(false);
    }
  };

  const dimLegend = (
    <div className="flex flex-wrap gap-3 text-xs mb-3">
      <span className="px-2 py-1 rounded border-l-4 border-pink-400 bg-pink-50 text-pink-700">Identity</span>
      <span className="px-2 py-1 rounded border-l-4 border-blue-400 bg-blue-50 text-blue-700">Goals</span>
      <span className="px-2 py-1 rounded border-l-4 border-amber-400 bg-amber-50 text-amber-700">Capabilities</span>
      <span className="px-2 py-1 rounded border-l-4 border-emerald-400 bg-emerald-50 text-emerald-700">State</span>
      <span className="px-2 py-1 rounded border-l-4 border-gray-300 bg-gray-50 text-gray-600">Other</span>
    </div>
  );

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-4xl mx-auto bg-white rounded-xl shadow p-6 space-y-6">
        <h2 className="text-2xl font-bold text-gray-800">版本控制</h2>
        {notice && (
          <div className="text-sm text-green-600 bg-green-50 px-3 py-2 rounded">
            {notice}
          </div>
        )}

        <section className="space-y-3">
          <h3 className="text-lg font-semibold text-gray-700">创建快照</h3>
          <div className="flex gap-3">
            <input
              placeholder="标签 (如: 里程碑 v0.2)"
              value={tag}
              onChange={(e) => setTag(e.target.value)}
              className="flex-1 border rounded-md px-3 py-2"
            />
            <input
              placeholder="备注"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              className="flex-[2] border rounded-md px-3 py-2"
            />
            <button
              onClick={handleCreate}
              disabled={creating}
              className="bg-indigo-600 text-white px-4 py-2 rounded-md hover:bg-indigo-700 disabled:opacity-50"
            >
              {creating ? "创建中..." : "快照"}
            </button>
          </div>
        </section>

        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold text-gray-700">历史版本</h3>
            <div className="flex gap-2">
              <button
                onClick={handleDiff}
                disabled={selected.length !== 2 || diffing}
                className="px-3 py-1.5 rounded-md text-sm border hover:bg-gray-50 disabled:opacity-50"
              >
                {diffing ? "对比中..." : "对比选中版本"}
              </button>
              <button
                onClick={load}
                className="px-3 py-1.5 rounded-md text-sm border hover:bg-gray-50"
              >
                刷新
              </button>
            </div>
          </div>

          {loading ? (
            <LoadingSpinner text="加载中..." className="py-6" />
          ) : snapshots.length === 0 ? (
            <EmptyState title="暂无历史快照" description="你还没有保存过任何版本快照。" className="py-6" />
          ) : (
            <div className="divide-y border rounded-md">
              {snapshots.map((s) => (
                <div
                  key={s.version}
                  className="flex items-center justify-between px-4 py-3 hover:bg-gray-50"
                >
                  <div className="flex items-center gap-3">
                    <input
                      type="checkbox"
                      checked={selected.includes(s.version)}
                      onChange={() => toggleSelect(s.version)}
                      className="h-4 w-4"
                    />
                    <div>
                      <div className="font-medium text-gray-800 flex items-center gap-2">
                        <span className={`text-xs px-2 py-0.5 rounded ${tagBadgeClass(s.tag)}`}>
                          {tagLabel(s.tag)}
                        </span>
                        <span>{s.version}</span>
                      </div>
                      <div className="text-xs text-gray-500">
                        {new Date(s.timestamp).toLocaleString()}
                        {s.note ? ` · ${s.note}` : ""}
                      </div>
                    </div>
                  </div>
                  <button
                    onClick={() => handleRestore(s.version)}
                    className="text-sm text-indigo-600 hover:text-indigo-800"
                  >
                    回滚
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>

        {diffText && (
          <section className="space-y-2">
            <h3 className="text-lg font-semibold text-gray-700">差异对比</h3>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div className="rounded-lg border bg-indigo-50/70 px-4 py-3">
                <div className="text-xs text-indigo-500">总变更</div>
                <div className="text-xl font-semibold text-indigo-900">{diffSummary.totalChanges}</div>
              </div>
              <div className="rounded-lg border bg-emerald-50/70 px-4 py-3">
                <div className="text-xs text-emerald-500">新增</div>
                <div className="text-xl font-semibold text-emerald-900">{diffSummary.adds}</div>
              </div>
              <div className="rounded-lg border bg-rose-50/70 px-4 py-3">
                <div className="text-xs text-rose-500">删除</div>
                <div className="text-xl font-semibold text-rose-900">{diffSummary.removes}</div>
              </div>
            </div>
            <div className="rounded-lg border bg-gray-50 px-4 py-3 space-y-3">
              <div>
                <div className="text-xs font-medium text-gray-700 mb-2">差异摘要</div>
                {diffSummary.dims.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {diffSummary.dims.map((item) => (
                      <span key={item.dim} className="rounded-full border bg-white px-3 py-1 text-xs text-gray-700">
                        {dimLabel(item.dim)} · {item.count} 处
                      </span>
                    ))}
                  </div>
                ) : (
                  <div className="text-sm text-gray-500">当前没有结构化差异。</div>
                )}
              </div>
              {diffSummary.highlights.length > 0 && (
                <div>
                  <div className="text-xs font-medium text-gray-700 mb-2">关键变化</div>
                  <div className="space-y-1">
                    {diffSummary.highlights.map((item, index) => (
                      <div key={`${item}-${index}`} className="text-sm text-gray-600">
                        {item}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
            {dimLegend}
            <pre className="text-xs bg-gray-900 text-gray-100 p-4 rounded-md overflow-auto max-h-96">
              {structuredDiff.map((line, idx) => {
                const base = line.sign === "+" ? "text-green-300" : line.sign === "-" ? "text-red-300" : "text-gray-300";
                return (
                  <div key={idx} className={`${dimBadgeClass(line.dim)} ${base} px-1`}>
                    {line.text}
                  </div>
                );
              })}
            </pre>
          </section>
        )}
      </div>
    </div>
  );
}
