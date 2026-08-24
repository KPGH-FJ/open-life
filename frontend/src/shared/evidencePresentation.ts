import type { EvidenceRef, ProviderPrivacyBoundarySummary, ViewModelEnvelope } from "@/tauri";
import type { WorkbenchBoundarySummary, WorkbenchEvidenceReference } from "@/ui/shell";

function uniqueEvidence(refs: ReadonlyArray<EvidenceRef | undefined>): EvidenceRef[] {
  const seen = new Set<string>();
  return refs.filter((ref): ref is EvidenceRef => {
    if (!ref || seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}

export function collectBoundaryEvidence(
  envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>
): EvidenceRef[] {
  return uniqueEvidence([...(envelope.evidenceRefs ?? []), ...(envelope.data?.evidenceRefs ?? [])]);
}

export function toWorkbenchEvidence(ref: EvidenceRef): WorkbenchEvidenceReference {
  const sourceLabels: Record<EvidenceRef["source"], string> = {
    "backend-readmodel": "系统读模型",
    audit: "审计记录",
    task: "任务记录",
    review: "审核记录",
    memory: "记忆记录",
    lifemodel: "LifeModel",
    settings: "设置记录",
    provider: "模型路由",
    resource: "本地资源",
  };
  const sensitivityLabels: Record<NonNullable<EvidenceRef["sensitivity"]>, string> = {
    public: "公开",
    local_private: "本机私密",
    sensitive: "敏感",
    redacted: "已脱敏",
  };
  return {
    id: ref.id,
    label: ref.label,
    source: sourceLabels[ref.source],
    sensitivity: ref.sensitivity ? sensitivityLabels[ref.sensitivity] : "未标注",
  };
}

export function boundaryPresentation(
  envelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>
): WorkbenchBoundarySummary {
  if (envelope.status === "loading") {
    return {
      label: "正在读取传输边界",
      detail: "读取完成前不判断是否保持本地。",
      status: "neutral",
    };
  }
  if (envelope.status === "error" || envelope.data === null) {
    return {
      label: "传输边界未知",
      detail: "系统边界读取失败；外部动作保持关闭。",
      status: envelope.status === "error" ? "error" : "unknown",
    };
  }
  if (envelope.status === "stale") {
    return {
      label: "传输边界已陈旧",
      detail: "刷新成功前不使用旧边界授权外部动作。",
      status: "stale",
    };
  }

  const boundary = envelope.data;
  const evidencePresent = collectBoundaryEvidence(envelope).length > 0;
  const riskKnown = boundary.risk !== "unknown";
  if (
    boundary.routeType === "local" &&
    boundary.externalTransmission === "not_sent" &&
    riskKnown &&
    evidencePresent
  ) {
    return {
      label: "本地路由，未外传",
      detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
      status: "success",
      verified: true,
    };
  }
  if (boundary.externalTransmission === "possible") {
    return {
      label: "可能发生外部传输",
      detail: "目标或传输结果仍需系统证据确认；外部动作保持关闭。",
      status: "unknown",
    };
  }
  if (
    boundary.externalTransmission === "unknown" ||
    boundary.routeType === "unknown" ||
    !riskKnown ||
    !evidencePresent
  ) {
    return {
      label: "是否外传未知",
      detail: "当前证据不足，不能显示本地确定态；外部动作保持关闭。",
      status: "unknown",
    };
  }
  if (boundary.externalTransmission === "sent") {
    return {
      label: "已发生外部传输",
      detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
      status: "waiting",
    };
  }
  return {
    label: "外部路由当前未发送",
    detail: `${boundary.providerLabel} · ${boundary.modelLabel}`,
    status: "neutral",
  };
}
