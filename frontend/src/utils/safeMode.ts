import type { SystemDiagnostics } from "../tauri";

export function isSafeMode(diagnostics: SystemDiagnostics | null | undefined): boolean {
  return (
    Boolean(diagnostics) &&
    ((diagnostics?.startup_warnings?.length ?? 0) > 0 ||
      (diagnostics?.vector_corrupt_embedding_count ?? 0) > 0 ||
      diagnostics?.database_status === "degraded")
  );
}

export function getSafeModeReason(diagnostics: SystemDiagnostics | null | undefined): string {
  if (!diagnostics) {
    return "系统当前处于 Safe Mode。";
  }
  return (
    diagnostics.startup_warnings?.[0] ??
    ((diagnostics.vector_corrupt_embedding_count ?? 0) > 0
      ? `检测到 ${diagnostics.vector_corrupt_embedding_count} 条损坏向量索引记录。`
      : diagnostics.database_status === "degraded"
        ? "当前数据库处于降级模式，暂不建议继续高风险写入。"
        : "系统当前处于 Safe Mode。")
  );
}
