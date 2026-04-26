import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { listAgentRuns, type AgentRun } from "../tauri";
import { Activity, Clock, AlertTriangle, CheckCircle, XCircle, Trash2, RotateCcw } from "lucide-react";

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

export default function RunsPage() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    loadRuns();
  }, []);

  async function loadRuns() {
    try {
      setLoading(true);
      const data = await listAgentRuns(50, 0);
      setRuns(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

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
        <div className="text-red-500">{error}</div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto p-6">
      <div className="max-w-5xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold text-stone-900">Runs</h1>
          <div className="text-sm text-stone-500">共 {runs.length} 条记录</div>
        </div>

        {runs.length === 0 ? (
          <div className="text-center py-12 text-stone-400">
            <Activity size={48} className="mx-auto mb-4 opacity-30" />
            <p>暂无运行记录</p>
            <p className="text-sm mt-1">开始对话或构建 LifeModel 后将在此显示</p>
          </div>
        ) : (
          <div className="space-y-3">
            {runs.map((run) => (
              <div
                key={run.id}
                onClick={() => navigate(`/runs/${run.id}`)}
                className="bg-white rounded-xl border border-stone-200 p-4 cursor-pointer hover:shadow-md transition-shadow"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    {statusIcon(run.status)}
                    <div>
                      <div className="font-medium text-stone-900">
                        {kindLabel(run.kind)}
                      </div>
                      <div className="text-xs text-stone-500 mt-0.5">
                        {run.user_input
                          ? run.user_input.slice(0, 60) + "..."
                          : "No user input"}
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
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
