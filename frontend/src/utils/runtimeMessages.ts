import type { SystemDiagnostics } from "../tauri";
import { getSafeModeReason } from "./safeMode";

export type RuntimeIssueKind = "configuration" | "data" | "model" | "review";

function issueTail(kind: RuntimeIssueKind): string {
  switch (kind) {
    case "configuration":
      return "建议先去设置页完成“试用就绪检查”，再继续当前操作。";
    case "data":
      return "建议先去 Settings 的恢复控制台处理数据风险，再继续当前操作。";
    case "model":
      return "建议先确认模型服务与连接测试通过，再继续当前操作。";
    case "review":
      return "建议先回到 Builder 审阅待确认内容，再继续当前操作。";
    default:
      return "建议先检查当前环境状态，再继续当前操作。";
  }
}

export function buildSafeModeBlockedMessage(
  action: string,
  diagnostics: SystemDiagnostics | null | undefined
): string {
  const reason = getSafeModeReason(diagnostics);
  return `当前处于 Safe Mode，已暂停${action}。原因：${reason} 风险：继续写入可能让当前降级数据状态更难恢复。${issueTail("data")}`;
}

export function buildRuntimeActionError(
  action: string,
  error: unknown,
  kind: RuntimeIssueKind
): string {
  const detail = error instanceof Error ? error.message : String(error);
  return `${action}失败：${detail} ${issueTail(kind)}`;
}
