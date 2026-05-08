import {
  generateEvolutionReport,
  runMemoryTierMaintenance,
  type SystemDiagnostics,
} from "../../../tauri";
import { buildSafeModeBlockedMessage } from "../../../utils/runtimeMessages";
import { readableError } from "../../../utils/error";

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
  return (
    <>
      {/* Data Migration */}
      <section id="data-health" className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">数字遗产 / 数据迁移</h3>
        <div id="backup-snapshot" className="flex flex-wrap gap-3">
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
        <p className="text-xs text-gray-500 mt-1">
          诊断报告导出（下方"导出诊断报告"）默认不包含原始敏感内容（如原始提示词、记忆内容、工具输出），仅包含系统状态和配置摘要，适合用于问题反馈。
        </p>
      </section>

      {/* Feedback Guidance */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">反馈与试用报告</h3>
        <div className="rounded-xl border border-indigo-100 bg-indigo-50/40 p-4 space-y-3">
          <div className="text-xs text-indigo-900 space-y-2">
            <p className="font-medium">提交反馈时建议包含：</p>
            <ul className="list-disc pl-4 space-y-1">
              <li>你正在执行的操作步骤（如"在 Chat 页面发送消息"）</li>
              <li>预期结果与实际结果</li>
              <li>Run ID、Proposal ID 或 Plan ID（如果有）</li>
              <li>导出的诊断报告文件（下方按钮）</li>
            </ul>
            <p className="font-medium mt-2">请勿在反馈中包含：</p>
            <ul className="list-disc pl-4 space-y-1">
              <li>原始 LifeModel 内容</li>
              <li>聊天消息原文</li>
              <li>记忆内容或工具输出</li>
              <li>API Key 或密码</li>
            </ul>
            <p className="text-indigo-700 mt-2">
              诊断报告默认排除上述敏感内容，仅包含系统状态和配置摘要。
            </p>
          </div>
        </div>
      </section>

      {/* Recovery Guidance */}
      <section className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">恢复指引</h3>
        <div className="rounded-xl border border-stone-200 bg-stone-50/70 p-4 space-y-3">
          <div className="text-xs text-stone-600 space-y-2">
            <p className="font-medium text-stone-800">常见数据问题与恢复路径：</p>
            <div className="grid gap-2 md:grid-cols-2">
              <div className="rounded-lg border border-white bg-white/80 p-3">
                <div className="text-xs font-medium text-stone-700">Safe Mode</div>
                <div className="mt-1 text-xs text-stone-500">
                  当检测到启动降级、数据库异常或向量损坏时进入。建议先导出备份，再使用恢复控制台中的重建工具。
                </div>
              </div>
              <div className="rounded-lg border border-white bg-white/80 p-3">
                <div className="text-xs font-medium text-stone-700">Proposal 应用失败</div>
                <div className="mt-1 text-xs text-stone-500">
                  失败的 Proposal 会保持 pending
                  状态。检查错误信息后，可编辑值重新应用，或拒绝后重新生成。
                </div>
              </div>
              <div className="rounded-lg border border-white bg-white/80 p-3">
                <div className="text-xs font-medium text-stone-700">备份与快照</div>
                <div className="mt-1 text-xs text-stone-500">
                  定期导出完整数据备份。版本控制中的快照可用于回滚 LifeModel 到之前状态。
                </div>
              </div>
              <div className="rounded-lg border border-white bg-white/80 p-3">
                <div className="text-xs font-medium text-stone-700">Safe Path 写入</div>
                <div className="mt-1 text-xs text-stone-500">
                  外部文件写入受 safe_paths 限制。若写入失败，请检查目标路径是否在 Settings →
                  Privacy 的 safe_paths 列表中。
                </div>
              </div>
            </div>
            <p className="text-stone-500 mt-2">
              恢复顺序建议：1) 导出诊断报告 2) 导出完整备份 3) 执行针对性修复 4) 验证后继续使用。
            </p>
          </div>
        </div>
      </section>

      {/* Maintenance */}
      <section id="diagnostic-export" className="space-y-4 border-t pt-4">
        <h3 className="text-sm font-medium text-gray-700">系统维护</h3>
        <div className="flex flex-wrap gap-3">
          <button
            onClick={async () => {
              setEvolutionLoading(true);
              setEvolutionResult(null);
              try {
                const res = await generateEvolutionReport();
                setEvolutionResult(`已应用规则 ${res.applied_rules.length} 条\n${res.summary}`);
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
            onClick={async () => {
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
            }}
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
