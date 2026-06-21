import {
  generateEvolutionReport,
  runMemoryTierMaintenance,
  type SystemDiagnostics,
} from "../../../tauri";
import { buildSafeModeBlockedMessage } from "../../../utils/runtimeMessages";
import { useState } from "react";
import ConfirmDangerDialog from "../../../components/ConfirmDangerDialog";

function readableError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    if ("message" in e && typeof (e as any).message === "string") return (e as any).message;
    if ("error" in e && typeof (e as any).error === "string") return (e as any).error;
  }
  return String(e);
}

interface DataTabProps {
  handleExport: () => Promise<void>;
  handleImport: () => Promise<void>;
  exportLoading: boolean;
  importLoading: boolean;
  safeMode: boolean;
  diagnostics: SystemDiagnostics | null;
  evolutionLoading: boolean;
  evolutionResult: string | null;
  setEvolutionLoading: (v: boolean) => void;
  setEvolutionResult: (v: string | null) => void;
  tierLoading: boolean;
  tierResult: string | null;
  setTierLoading: (v: boolean) => void;
  setTierResult: (v: string | null) => void;
  handleExportDiagnostics: () => Promise<void>;
}

export default function DataTab({
  handleExport,
  handleImport,
  exportLoading,
  importLoading,
  safeMode,
  diagnostics,
  evolutionLoading,
  evolutionResult,
  setEvolutionLoading,
  setEvolutionResult,
  tierLoading,
  tierResult,
  setTierLoading,
  setTierResult,
  handleExportDiagnostics,
}: DataTabProps) {
  const [confirmTierMaintenance, setConfirmTierMaintenance] = useState(false);

  const runTierMaintenance = async () => {
    setConfirmTierMaintenance(false);
    if (safeMode) {
      setTierResult(buildSafeModeBlockedMessage("记忆层级维护", diagnostics));
      return;
    }
    setTierLoading(true);
    setTierResult(null);
    try {
      const res = await runMemoryTierMaintenance();
      setTierResult(`记忆层级维护完成：晋升 ${res.promoted} 条，降级 ${res.demoted} 条`);
    } catch (e: any) {
      setTierResult("维护失败: " + readableError(e));
    } finally {
      setTierLoading(false);
    }
  };

  return (
    <>
      <ConfirmDangerDialog
        open={confirmTierMaintenance}
        title="确认运行记忆层级维护"
        description="这个操作会调整本地记忆的冷热层级，影响后续检索优先级。运行前请确认当前数据环境稳定。"
        confirmLabel="运行维护"
        severity="warning"
        busy={tierLoading}
        onConfirm={() => void runTierMaintenance()}
        onCancel={() => setConfirmTierMaintenance(false)}
      />
      {/* Data Migration */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">数字遗产 / 数据迁移</h3>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={handleExport}
            disabled={exportLoading}
            className="px-3 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 disabled:opacity-50"
          >
            {exportLoading ? "导出中..." : "导出全部数据"}
          </button>
          <button
            onClick={handleImport}
            disabled={importLoading || safeMode}
            className="px-3 py-2 bg-white border border-gray-300 text-gray-700 rounded-md text-sm font-medium hover:bg-gray-50 disabled:opacity-50"
          >
            {importLoading ? "导入中..." : "导入全部数据"}
          </button>
        </div>
        <p className="text-xs text-gray-500">
          导出将包含 LifeModel、聊天记录与向量记忆数据，格式为
          JSON（带版本号与主版本校验）。导入会覆盖当前数据，跨主版本导入会被拒绝，请谨慎操作。
        </p>
      </section>

      {/* Maintenance */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">系统维护</h3>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={async () => {
              setEvolutionLoading(true);
              setEvolutionResult(null);
              try {
                const res = await generateEvolutionReport();
                setEvolutionResult(
                  `只读进化报告：候选 ${res.proposal_candidate_count} 条，已应用 ${res.applied_rule_count} 条\n${res.summary}`
                );
              } catch (e: any) {
                setEvolutionResult("生成失败: " + readableError(e));
              } finally {
                setEvolutionLoading(false);
              }
            }}
            disabled={evolutionLoading}
            className="px-3 py-2 bg-emerald-600 text-white rounded-md text-sm font-medium hover:bg-emerald-700 disabled:opacity-50"
          >
            {evolutionLoading ? "生成中..." : "生成进化报告"}
          </button>
          <button
            onClick={() => setConfirmTierMaintenance(true)}
            disabled={tierLoading || safeMode}
            className="px-3 py-2 bg-amber-600 text-white rounded-md text-sm font-medium hover:bg-amber-700 disabled:opacity-50"
          >
            {tierLoading ? "维护中..." : "运行记忆层级维护"}
          </button>
          <button
            onClick={handleExportDiagnostics}
            disabled={exportLoading}
            className="px-3 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 disabled:opacity-50"
          >
            {exportLoading ? "导出中..." : "导出诊断报告"}
          </button>
        </div>
        {evolutionResult && (
          <div className="text-sm whitespace-pre-line bg-emerald-50 text-emerald-800 rounded px-3 py-2">
            {evolutionResult}
          </div>
        )}
        {tierResult && (
          <div className="text-sm whitespace-pre-line bg-amber-50 text-amber-800 rounded px-3 py-2">
            {tierResult}
          </div>
        )}
      </section>
    </>
  );
}
