import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  ReviewItem,
  TaskControl,
  ViewModelEnvelope,
} from "@/tauri";
import type {
  WorkbenchContextSummary,
  WorkbenchEvidenceReference,
  WorkbenchInspectorModel,
} from "@/ui/shell";
import {
  taskLifecyclePresentation,
  toWorkbenchEvidence,
} from "@/ui/journeys/readOnly/readOnlySpinePresentation";
import type { GovernedActionSnapshot } from "./governedActionDataSource";

function uniqueEvidence(refs: readonly EvidenceRef[]): WorkbenchEvidenceReference[] {
  const seen = new Set<string>();
  return refs
    .filter(ref => {
      if (seen.has(ref.id)) return false;
      seen.add(ref.id);
      return true;
    })
    .map(toWorkbenchEvidence);
}

function diagnosticsText(snapshot: GovernedActionSnapshot): string {
  return snapshot.diagnostics
    .map(item => `${item.id}:${item.status}${item.message ? ` (${item.message})` : ""}`)
    .join(" | ");
}

export function governedBoundaryEnvelope(
  snapshot: GovernedActionSnapshot | null
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  if (!snapshot) {
    return {
      data: null,
      status: "loading",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
  }
  const workspace = snapshot.workspaceEnvelope;
  const data = workspace.data?.providerPrivacyBoundarySummary ?? null;
  const status =
    workspace.status === "error" || workspace.status === "stale" || workspace.status === "loading"
      ? workspace.status
      : data
        ? "ready"
        : "error";
  return {
    data,
    status,
    lastUpdatedAt: workspace.lastUpdatedAt,
    source: "backend-readmodel",
    evidenceRefs: data?.evidenceRefs ?? [],
    warnings: workspace.warnings ?? [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

export function workspaceContext(snapshot: GovernedActionSnapshot | null): WorkbenchContextSummary {
  if (!snapshot || snapshot.workspaceEnvelope.status === "loading") {
    return {
      eyebrow: "当前执行",
      title: "工作区",
      status: { label: "正在读取", status: "neutral" },
    };
  }
  if (snapshot.workspaceEnvelope.status === "error") {
    return {
      eyebrow: "当前执行",
      title: "工作区",
      status: { label: "状态不可用", status: "error" },
    };
  }
  if (snapshot.workspaceEnvelope.status === "stale") {
    return {
      eyebrow: "当前执行",
      title: "工作区",
      status: { label: "状态已陈旧", status: "stale" },
    };
  }
  if (snapshot.workspaceEnvelope.status === "empty") {
    return {
      eyebrow: "当前执行",
      title: "工作区",
      status: { label: "没有当前任务", status: "neutral" },
    };
  }
  const task = snapshot.workspaceEnvelope.data?.activeTask;
  if (!task) {
    return {
      eyebrow: "当前执行",
      title: "工作区",
      status: { label: "没有当前任务", status: "neutral" },
    };
  }
  const status = taskLifecyclePresentation(task);
  return { eyebrow: "当前执行", title: "工作区", status };
}

export function reviewContext(
  snapshot: GovernedActionSnapshot | null,
  item: ReviewItem | null
): WorkbenchContextSummary {
  if (!snapshot || snapshot.reviewEnvelope.status === "loading") {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "正在读取", status: "neutral" },
    };
  }
  if (snapshot.reviewEnvelope.status === "error") {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "状态不可用", status: "error" },
    };
  }
  if (snapshot.reviewEnvelope.status === "stale") {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "状态已陈旧", status: "stale" },
    };
  }
  if (snapshot.reviewEnvelope.status === "empty") {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "暂无待处理项", status: "neutral" },
    };
  }
  if (!item) {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "暂无待处理项", status: "neutral" },
    };
  }
  if (["pending", "edited", "deferred"].includes(item.status)) {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "等待你的决定", status: "waiting" },
    };
  }
  if (item.status === "approved") {
    const materialization =
      item.materializationStatus === "applying"
        ? { label: "正在应用", status: "waiting" as const }
        : item.materializationStatus === "applied"
          ? { label: "已应用", status: "success" as const, verified: true }
          : item.materializationStatus === "failed"
            ? { label: "应用失败", status: "error" as const }
            : item.materializationStatus === "rolled_back"
              ? { label: "已回滚", status: "waiting" as const }
              : item.materializationStatus === "unknown"
                ? { label: "应用状态未知", status: "unknown" as const }
                : { label: "已批准，尚未应用", status: "neutral" as const };
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: {
        label: item.type === "tool_permission" ? "已允许一次，尚未继续任务" : materialization.label,
        status: item.type === "tool_permission" ? "neutral" : materialization.status,
        verified: item.type === "tool_permission" ? undefined : materialization.verified,
      },
    };
  }
  if (item.status === "rejected") {
    return {
      eyebrow: "建议与权限",
      title: "审核中心",
      status: { label: "已拒绝", status: "neutral" },
    };
  }
  return {
    eyebrow: "建议与权限",
    title: "审核中心",
    status: { label: "决定状态未知", status: "unknown" },
  };
}

export function findExactResumeControl(snapshot: GovernedActionSnapshot): TaskControl | null {
  if (snapshot.workspaceEnvelope.status !== "ready") return null;
  const task = snapshot.workspaceEnvelope.data?.activeTask;
  if (!task?.taskSessionId || task.canonicalTaskId !== task.taskSessionId) return null;
  return (
    task.allowedControls.find(
      control =>
        control.kind === "resume" &&
        control.effect === "task_resume_request" &&
        control.targetTaskId === task.taskSessionId
    ) ?? null
  );
}

export function workspaceInspector(
  snapshot: GovernedActionSnapshot | null,
  selectedEvidence: string
): WorkbenchInspectorModel {
  if (!snapshot) {
    return {
      title: "工作区状态依据",
      conclusion: "正在读取当前任务。",
      risk: "读取完成前不开放审核或任务控制。",
      nextAction: "等待后端读模型返回。",
      evidence: [],
    };
  }
  const envelopeModel = snapshot.workspaceEnvelope.data;
  const model = ["ready", "stale"].includes(snapshot.workspaceEnvelope.status)
    ? envelopeModel
    : null;
  const task = model?.activeTask;
  const evidence = uniqueEvidence([
    ...(model?.sourceRefs ?? []),
    ...(task?.evidenceRefs ?? []),
    ...(model?.activity.flatMap(item => item.evidenceRefs) ?? []),
    ...(model?.pendingReviewItems.flatMap(item => item.evidenceRefs) ?? []),
    ...(envelopeModel?.providerPrivacyBoundarySummary.evidenceRefs ?? []),
  ]);
  const permissionItem =
    task && model?.pendingReviewItems.find(item => item.type === "tool_permission");

  return {
    title: task?.title ?? "工作区状态依据",
    conclusion:
      snapshot.workspaceEnvelope.status === "error"
        ? "WorkspaceViewModel 未能建立，当前没有可执行产品结论。"
        : task?.lifecycleStatus === "waiting_permission"
          ? "任务暂停在一个明确动作之前；被请求的动作尚未执行。"
          : task?.lifecycleStatus === "waiting_review"
            ? "报告产物已经生成，但任务要等你审核后才能确认交付。"
            : task?.lifecycleStatus === "running"
              ? "刷新后的任务读模型确认同一任务正在处理。"
              : task
                ? `后端将当前任务标记为 ${task.lifecycleStatus}。`
                : "后端没有提供当前活动任务。",
    risk: permissionItem
      ? permissionItem.decisionContext.permission?.status === "ready"
        ? "批准只创建一次精确授权；任务恢复和后续结果仍需独立刷新。"
        : "权限范围不完整，批准必须保持禁用。"
      : snapshot.workspaceEnvelope.status === "error" ||
          snapshot.workspaceEnvelope.status === "stale"
        ? "错误或陈旧状态不能授权审核决定或任务恢复。"
        : "当前没有等待决定的精确权限项。",
    nextAction: permissionItem
      ? "进入审核中心核对访问范围并作出决定。"
      : findExactResumeControl(snapshot)?.enabled
        ? "请求继续任务，然后再次刷新同一任务状态。"
        : "查看当前活动与来源；没有后端允许的动作时保持只读。",
    evidence,
    evidenceFeedback: selectedEvidence
      ? `已选择 ${selectedEvidence}；这里只展示引用元数据，不展开敏感正文。`
      : evidence.length === 0
        ? "当前没有可展示的后端证据引用。"
        : undefined,
    technicalDetails: [
      { label: "workspaceStatus", value: snapshot.workspaceEnvelope.status },
      { label: "activeTaskId", value: task?.canonicalTaskId ?? "none" },
      { label: "taskSessionId", value: task?.taskSessionId ?? "none" },
      { label: "lifecycle", value: task?.lifecycleStatus ?? "none" },
      {
        label: "reviewItemIds",
        value: model?.pendingReviewItems.map(item => item.id).join(", ") || "none",
      },
      {
        label: "controlIds",
        value: task?.allowedControls.map(control => control.id).join(", ") || "none",
      },
      { label: "redaction", value: model?.activityRedactionState ?? "unknown" },
      { label: "diagnostics", value: diagnosticsText(snapshot) },
    ],
  };
}

export function reviewInspector(
  snapshot: GovernedActionSnapshot | null,
  item: ReviewItem | null,
  selectedEvidence: string
): WorkbenchInspectorModel {
  if (!snapshot) {
    return {
      title: "审核状态依据",
      conclusion: "正在读取审核项。",
      risk: "读取完成前不开放决定动作。",
      nextAction: "等待后端读模型返回。",
      evidence: [],
    };
  }
  const permission = item?.decisionContext.permission;
  const evidence = uniqueEvidence([
    ...(snapshot.reviewEnvelope.evidenceRefs ?? []),
    ...(item?.evidenceRefs ?? []),
    ...(item?.decisionContext.evidenceRefs ?? []),
    ...(permission?.evidenceRefs ?? []),
    ...(permission?.transmissionBoundary.evidenceRefs ?? []),
  ]);

  return {
    title: item?.decisionContext.title ?? "审核状态依据",
    conclusion: item
      ? item.status === "approved"
        ? item.type === "tool_permission"
          ? "一次性权限决定已经记录，但任务是否继续仍取决于刷新后的 TaskControl。"
          : item.materializationStatus === "applied"
            ? "刷新后的审核读模型确认变更已经应用。"
            : item.materializationStatus === "applying"
              ? "刷新后的审核读模型确认应用过程已经开始，但尚未完成。"
              : item.materializationStatus === "failed"
                ? "审核决定仍为已批准，但应用过程失败。"
                : item.materializationStatus === "rolled_back"
                  ? "刷新后的审核读模型确认此前应用已经回滚。"
                  : item.materializationStatus === "unknown"
                    ? "审核决定已经记录，但应用结果未知。"
                    : "审核决定已经记录，但没有已应用证明。"
        : `审核项当前状态为 ${item.status}；打开详情没有改变该状态。`
      : snapshot.reviewEnvelope.status === "error"
        ? "ReviewCenterViewModel 未能建立。"
        : "当前没有选中的审核项。",
    risk: permission
      ? permission.status === "ready"
        ? `${permission.transmissionBoundary.summary} 授权只匹配一次精确动作。`
        : `权限上下文缺少：${permission.missingFields.join("、") || "未说明字段"}。`
      : item
        ? `风险级别由后端标记为 ${item.risk}；不能从页面文本重新分级。`
        : "没有可用于决定的上下文。",
    nextAction: item
      ? item.status === "approved" && item.taskResumeRelation?.canRequestResume
        ? "返回工作区，刷新并核对同一任务的恢复控制。"
        : ["pending", "edited", "deferred"].includes(item.status)
          ? "比较范围、影响与来源，再选择拒绝、稍后或批准。"
          : "查看刷新后的状态与证据；不要从命令回调推断完成。"
      : "选择一个审核项。",
    evidence,
    evidenceFeedback: selectedEvidence
      ? `已选择 ${selectedEvidence}；这里只展示引用元数据，不展开敏感正文。`
      : evidence.length === 0
        ? "当前没有可展示的后端证据引用。"
        : undefined,
    technicalDetails: [
      { label: "reviewStatus", value: snapshot.reviewEnvelope.status },
      { label: "reviewItemId", value: item?.id ?? "none" },
      { label: "decision", value: item?.status ?? "none" },
      { label: "materialization", value: item?.materializationStatus ?? "none" },
      { label: "proposalId", value: item?.source.proposalId ?? "none" },
      { label: "taskSessionId", value: item?.taskResumeRelation?.taskSessionId ?? "none" },
      { label: "scopeKind", value: permission?.scopeKind ?? "none" },
      { label: "scopeDigest", value: permission?.scopeDigest ?? "none" },
      { label: "requestDigest", value: permission?.requestDigest ?? "none" },
      { label: "networkDecision", value: permission?.networkPolicyDecisionId ?? "none" },
      { label: "diagnostics", value: diagnosticsText(snapshot) },
    ],
  };
}
