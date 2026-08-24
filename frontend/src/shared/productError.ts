type ErrorRecord = Record<string, unknown>;

function asRecord(value: unknown): ErrorRecord | null {
  return value !== null && typeof value === "object" ? (value as ErrorRecord) : null;
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

export function productErrorCode(error: unknown): string {
  if (error instanceof Error) return error.message || error.name || "unknown_error";
  if (typeof error === "string") return error.trim() || "unknown_error";

  const record = asRecord(error);
  if (!record) return "unknown_error";

  const detail = asRecord(record.detail);
  const kind = nonEmptyString(record.kind);
  const code = nonEmptyString(detail?.code) ?? nonEmptyString(record.code);
  const hint = nonEmptyString(detail?.hint) ?? nonEmptyString(record.hint);

  if (kind && hint) return `${kind}:${hint}`;
  if (code) return code;
  if (hint) return hint;
  if (kind) return `${kind}:command_failed`;
  return "structured_command_error";
}

const PRODUCT_ERROR_MESSAGES: ReadonlyArray<[RegExp, string]> = [
  [
    /web_search_challenge_detected/i,
    "当前搜索服务要求完成人机验证，OpenLife 无法代替你完成。你可以稍后重试，或在设置中改用已配置的搜索服务。",
  ],
  [
    /web_search_no_structured_results/i,
    "当前搜索服务没有返回可核验的结果。本轮没有被标记为完成，你可以重试。",
  ],
  [/provider_reasoning_without_final_content/i, "模型没有返回完整结果。你可以重试本轮工作。"],
  [
    /provider_quota_exhausted/i,
    "当前模型额度不足，本轮工作已安全停止。额度恢复后可以重试；OpenLife 不会静默切换到其他模型。",
  ],
  [
    /work_semantic_verification_(needs_more_evidence|stalled)/i,
    "现有来源仍不足以支持交付要求，因此 OpenLife 停止了任务，没有把未经支持的内容标记为完成。你可以补充来源或重试。",
  ],
  [
    /work_source_grounding_(provider_failed|result_invalid|candidate_invalid|context_too_large|artifact_identity_changed|review_weakened|user_missing)/i,
    "OpenLife 无法确认结果中的陈述都受到当前来源支持，因此没有交付这份结果。你可以缩小任务范围或重试。",
  ],
  [
    /web_artifact_citation_not_in_body/i,
    "来源标记没有放在它所支持的正文陈述附近，因此没有交付。OpenLife 可以重新整理引用后继续。",
  ],
  [
    /web_artifact_(source_validation_failed|citation_missing|citation_unknown|citation_run_mismatch|url_not_observed)|web_fetch_distinct_search_result_missing/i,
    "当前读取的来源不足以支持这份结果，因此没有交付。OpenLife 可以继续读取所需来源后重试。",
  ],
  [
    /work_plan_(json_invalid|required_step_incomplete|invalid|missing)/i,
    "OpenLife 没能形成可靠的执行计划。本轮没有被标记为完成，请调整要求后重试。",
  ],
  [
    /provider_request_preparation_failed|provider_.*unavailable/i,
    "当前模型连接尚未准备好。请在设置中检查模型后重试。",
  ],
  [
    /work_run_budget_exhausted/i,
    "这项工作达到本轮执行预算；已完成的执行记录仍会保留。你可以重试这项工作。",
  ],
  [
    /artifact_target_precondition_changed/i,
    "目标文件在确认前发生了变化。为避免覆盖新内容，本次写入已停止。",
  ],
  [
    /artifact_revision_(base_changed|base_not_verified_current|target_reference_mismatch)/i,
    "当前产物版本已经变化或无法通过完整性核验，因此没有开始修订。请刷新后从最新已核验版本重新发起。",
  ],
  [
    /artifact_revision_(target_or_media_changed|requires_single_artifact|produced_no_change)/i,
    "模型返回的修订没有遵守当前产物边界，或没有产生实际变化，因此 OpenLife 没有覆盖现有文件。",
  ],
  [
    /artifact_revision_(run_identity_mismatch|receipt_unverified)/i,
    "OpenLife 无法确认这次修订绑定到了新运行，因此没有把它作为已开始的修订展示。",
  ],
  [/artifact_file_changed/i, "目标文件已经变化，当前版本不再通过摘要核验，因此没有打开或导出。"],
  [
    /artifact_file_unavailable|artifact_materialized_reference_missing/i,
    "已记录的产物文件当前不可访问；OpenLife 没有把缺失文件当作可用结果。",
  ],
  [
    /artifact_export_(failed|unverified|digest_mismatch|target_invalid|picker_failed)/i,
    "另存操作或写后摘要核验没有完成；原产物没有被修改。",
  ],
  [/artifact_open_failed/i, "产物通过核验，但系统没有成功打开对应文件。"],
  [
    /safe_paths|artifact_safe_path|artifact_output.*unavailable/i,
    "尚未选择可保存文件的位置。请先选择一个文件夹。",
  ],
  [
    /canonical_state_unknown|read_only_degraded|database/i,
    "本地数据状态暂时无法确认。请重新读取；在恢复前不会执行写入。",
  ],
  [
    /network.*(denied|blocked|consent|required)|provider_network/i,
    "当前网络范围不允许这次访问。请在设置中检查联网范围。",
  ],
  [/cancel/i, "取消请求没有完成；当前工作仍按运行中处理。"],
];

export function productErrorMessage(
  error: unknown,
  fallback = "操作没有完成。你可以重试，或在详情中查看技术信息。"
): string {
  const code = productErrorCode(error);
  const mapped = PRODUCT_ERROR_MESSAGES.find(([pattern]) => pattern.test(code));
  if (mapped) return mapped[1];
  const looksInternal = /^[a-z0-9_.:-]+$/i.test(code) || code.includes("_");
  return looksInternal ? fallback : code;
}
