import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface RolloutMetric {
  id: number;
  experiment: string;
  version: string;
  timestamp: string;
  duration_ms: number;
  success: boolean;
  error?: string;
  metadata?: string;
}

interface RolloutSummary {
  total: number;
  v2_count: number;
  v1_count: number;
  success_count: number;
  v2_avg_duration_ms?: number;
  v1_avg_duration_ms?: number;
}

const MetricsPage: React.FC = () => {
  const [metrics, setMetrics] = useState<RolloutMetric[]>([]);
  const [summary, setSummary] = useState<RolloutSummary | null>(null);
  const [errors, setErrors] = useState<RolloutMetric[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [metricsData, summaryData, errorsData] = await Promise.all([
        invoke<RolloutMetric[]>("get_rollout_metrics", {
          experiment: "context_assembler",
          limit: 100,
          offset: 0,
        }),
        invoke<RolloutSummary>("get_rollout_summary", {
          experiment: "context_assembler",
        }),
        invoke<RolloutMetric[]>("get_rollout_errors", {
          experiment: "context_assembler",
          limit: 10,
        }),
      ]);
      setMetrics(metricsData);
      setSummary(summaryData);
      setErrors(errorsData);
    } catch (e) {
      console.error("Failed to load metrics:", e);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-lg text-gray-500">加载中...</div>
      </div>
    );
  }

  const v2Metrics = metrics.filter((m) => m.version === "v2");
  const v1Metrics = metrics.filter((m) => m.version === "v1");

  return (
    <div className="p-6 max-w-7xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-gray-900">灰度监控 Dashboard</h1>
        <button
          onClick={loadData}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
        >
          刷新数据
        </button>
      </div>

      {/* Summary Cards */}
      {summary && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-8">
          <div className="bg-white rounded-lg shadow p-4 border-l-4 border-blue-500">
            <div className="text-sm text-gray-500">总调用次数</div>
            <div className="text-2xl font-bold">{summary.total}</div>
          </div>
          <div className="bg-white rounded-lg shadow p-4 border-l-4 border-green-500">
            <div className="text-sm text-gray-500">V2 调用</div>
            <div className="text-2xl font-bold text-green-600">{summary.v2_count}</div>
          </div>
          <div className="bg-white rounded-lg shadow p-4 border-l-4 border-gray-500">
            <div className="text-sm text-gray-500">V1 调用</div>
            <div className="text-2xl font-bold">{summary.v1_count}</div>
          </div>
          <div className="bg-white rounded-lg shadow p-4 border-l-4 border-purple-500">
            <div className="text-sm text-gray-500">成功率</div>
            <div className="text-2xl font-bold text-purple-600">
              {summary.total > 0
                ? `${((summary.success_count / summary.total) * 100).toFixed(1)}%`
                : "N/A"}
            </div>
          </div>
        </div>
      )}

      {/* Performance Comparison */}
      {summary && (
        <div className="bg-white rounded-lg shadow p-6 mb-8">
          <h2 className="text-lg font-semibold mb-4">性能对比</h2>
          <div className="grid grid-cols-2 gap-8">
            <div>
              <div className="text-sm text-gray-500 mb-1">V2 平均耗时</div>
              <div className="text-xl font-bold">
                {summary.v2_avg_duration_ms
                  ? `${summary.v2_avg_duration_ms.toFixed(0)}ms`
                  : "暂无数据"}
              </div>
            </div>
            <div>
              <div className="text-sm text-gray-500 mb-1">V1 平均耗时</div>
              <div className="text-xl font-bold">
                {summary.v1_avg_duration_ms
                  ? `${summary.v1_avg_duration_ms.toFixed(0)}ms`
                  : "暂无数据"}
              </div>
            </div>
          </div>
          {summary.v2_avg_duration_ms && summary.v1_avg_duration_ms && (
            <div className="mt-4 p-3 bg-gray-50 rounded text-sm">
              V2 相比 V1
              {summary.v2_avg_duration_ms < summary.v1_avg_duration_ms
                ? ` 快 ${(
                    ((summary.v1_avg_duration_ms - summary.v2_avg_duration_ms) /
                      summary.v1_avg_duration_ms) *
                    100
                  ).toFixed(1)}%`
                : ` 慢 ${(
                    ((summary.v2_avg_duration_ms - summary.v1_avg_duration_ms) /
                      summary.v1_avg_duration_ms) *
                    100
                  ).toFixed(1)}%`}
            </div>
          )}
        </div>
      )}

      {/* Recent Metrics */}
      <div className="bg-white rounded-lg shadow p-6 mb-8">
        <h2 className="text-lg font-semibold mb-4">
          最近调用记录 ({metrics.length})
        </h2>
        <div className="overflow-x-auto">
          <table className="min-w-full">
            <thead>
              <tr className="border-b">
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">版本</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">耗时</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">状态</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">时间</th>
                <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">元数据</th>
              </tr>
            </thead>
            <tbody>
              {metrics.slice(0, 20).map((metric) => (
                <tr key={metric.id} className="border-b hover:bg-gray-50">
                  <td className="py-2 px-3">
                    <span
                      className={`inline-flex px-2 py-1 text-xs rounded-full ${
                        metric.version === "v2"
                          ? "bg-green-100 text-green-800"
                          : "bg-gray-100 text-gray-800"
                      }`}
                    >
                      {metric.version}
                    </span>
                  </td>
                  <td className="py-2 px-3">{metric.duration_ms}ms</td>
                  <td className="py-2 px-3">
                    <span
                      className={`inline-flex px-2 py-1 text-xs rounded-full ${
                        metric.success
                          ? "bg-green-100 text-green-800"
                          : "bg-red-100 text-red-800"
                      }`}
                    >
                      {metric.success ? "成功" : "失败"}
                    </span>
                  </td>
                  <td className="py-2 px-3 text-sm text-gray-500">
                    {new Date(metric.timestamp).toLocaleString()}
                  </td>
                  <td className="py-2 px-3 text-sm text-gray-500">
                    {metric.metadata || "-"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Errors */}
      {errors.length > 0 && (
        <div className="bg-white rounded-lg shadow p-6">
          <h2 className="text-lg font-semibold mb-4 text-red-600">
            最近错误 ({errors.length})
          </h2>
          <div className="space-y-3">
            {errors.map((error) => (
              <div
                key={error.id}
                className="p-3 bg-red-50 border border-red-200 rounded text-sm"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="font-medium">{error.version}</span>
                  <span className="text-gray-500">
                    {new Date(error.timestamp).toLocaleString()}
                  </span>
                </div>
                <div className="text-red-700">{error.error}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default MetricsPage;
