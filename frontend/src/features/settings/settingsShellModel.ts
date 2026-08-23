import type { EvidenceRef } from "@/tauri";
import type {
  WorkbenchContextSummary,
  WorkbenchEvidenceReference,
  WorkbenchInspectorModel,
} from "@/ui/shell";
import { boundaryPresentation, toWorkbenchEvidence } from "@/shared/evidencePresentation";
import type { SettingsController } from "./useSettingsController";
import {
  endpointHost,
  settingsProviderLabels,
  type SettingsSurfaceId,
} from "./settingsPresentation";

function uniqueEvidence(refs: WorkbenchEvidenceReference[]): WorkbenchEvidenceReference[] {
  const seen = new Set<string>();
  return refs.filter(ref => {
    if (seen.has(ref.id)) return false;
    seen.add(ref.id);
    return true;
  });
}

function evidenceRefs(refs: readonly EvidenceRef[]): WorkbenchEvidenceReference[] {
  return refs.map(toWorkbenchEvidence);
}

export function settingsContext(
  controller: SettingsController,
  surface: SettingsSurfaceId
): WorkbenchContextSummary {
  const title =
    surface === "model-provider"
      ? "模型与供应商"
      : surface === "privacy-network"
        ? "隐私与网络"
        : "产品诊断";
  if (surface === "diagnostics") {
    const diagnostics = controller.snapshot?.productDiagnostics;
    if (controller.loading) {
      return { eyebrow: "设置", title, status: { label: "正在读取", status: "neutral" } };
    }
    if (!diagnostics) {
      return { eyebrow: "设置", title, status: { label: "诊断不可用", status: "error" } };
    }
    return {
      eyebrow: "设置",
      title,
      status: {
        label:
          diagnostics.status === "ready"
            ? "产品链路正常"
            : diagnostics.status === "degraded"
              ? "部分能力降级"
              : "产品链路受阻",
        status:
          diagnostics.status === "ready"
            ? "neutral"
            : diagnostics.status === "degraded"
              ? "waiting"
              : "error",
      },
    };
  }
  if (controller.loading) {
    return { eyebrow: "设置", title, status: { label: "正在读取", status: "neutral" } };
  }
  if (controller.protectionState === "active") {
    return { eyebrow: "设置", title, status: { label: "安全模式", status: "waiting" } };
  }
  if (controller.protectionState === "unknown") {
    return { eyebrow: "设置", title, status: { label: "保护状态未知", status: "unknown" } };
  }
  if (!controller.snapshot?.config) {
    return { eyebrow: "设置", title, status: { label: "配置不可用", status: "error" } };
  }
  switch (controller.state.phase) {
    case "dirty":
      return { eyebrow: "设置", title, status: { label: "有未保存更改", status: "waiting" } };
    case "testing":
      return { eyebrow: "设置", title, status: { label: "正在测试", status: "neutral" } };
    case "tested":
      return { eyebrow: "设置", title, status: { label: "测试完成，尚未保存", status: "waiting" } };
    case "saving":
      return { eyebrow: "设置", title, status: { label: "正在保存", status: "neutral" } };
    case "refreshing_boundary":
      return { eyebrow: "设置", title, status: { label: "正在核对边界", status: "neutral" } };
    case "ready":
      return { eyebrow: "设置", title, status: { label: "已保存并核对", status: "neutral" } };
    case "unknown":
      return { eyebrow: "设置", title, status: { label: "边界待核对", status: "unknown" } };
    case "failed":
      if (controller.state.failureStage === "test") {
        return {
          eyebrow: "设置",
          title,
          status: {
            label: "测试需要处理",
            status: controller.testPresentation?.status ?? "unknown",
          },
        };
      }
      return {
        eyebrow: "设置",
        title,
        status: {
          label: controller.state.failureStage === "save" ? "保存失败" : "边界待核对",
          status: controller.state.failureStage === "save" ? "error" : "unknown",
        },
      };
    case "idle":
      return { eyebrow: "设置", title };
  }
}

function phaseConclusion(controller: SettingsController): string {
  if (controller.loading) {
    return "正在重新读取清理后的配置、LifeStateProjection 与模型传输边界；旧快照不作为当前确定态。";
  }
  if (controller.protectionState === "active") {
    return "系统明确报告安全模式；长期写入保持关闭，当前页面不从配置或提示文案推导恢复状态。";
  }
  if (controller.protectionState === "unknown") {
    return "LifeStateProjection 没有提供可核对的保护状态；配置与边界不能据此宣称正常运行。";
  }
  if (!controller.snapshot?.config) {
    return "系统没有提供可编辑的清理后配置；当前页面不会使用默认值补造设置。";
  }
  switch (controller.state.phase) {
    case "dirty":
      return "当前只修改了页面草稿，系统配置与传输边界尚未改变。";
    case "testing":
      return "正在验证精确的草稿配置；测试与保存是两个独立动作。";
    case "tested":
      return "本次连接测试已有可信回执，但草稿尚未保存。";
    case "saving":
      return "设置保存命令正在执行；当前还没有保存后边界证明。";
    case "refreshing_boundary":
      return "保存命令已经返回，正在重新读取清理后配置和模型传输边界。";
    case "ready":
      return "保存后的配置与传输边界已经由系统读模型重新核对。";
    case "unknown":
      return "设置命令已经返回，但保存后的传输边界仍然未知。";
    case "failed":
      return controller.testPresentation?.label
        ? `连接测试结果：${controller.testPresentation.label}。`
        : "本次设置操作失败，系统产品状态没有因此变为可用。";
    case "idle":
      return "页面显示清理后的 AppConfig；当前路由与外传结论只来自独立边界读模型。";
  }
}

function nextAction(controller: SettingsController): string {
  if (controller.loading) {
    return "等待本次系统读取结束；期间不修改、不测试、不保存。";
  }
  if (controller.protectionState === "active") {
    return "核对检查器中的系统来源；当前读模型未提供凭据恢复资格时，不执行系统凭据操作。";
  }
  if (controller.protectionState === "unknown") {
    return "先恢复并重新读取 LifeStateProjection；保护状态未知时不测试、不保存。";
  }
  const outcome = controller.lastTestOutcome;
  if (outcome?.result.validation_status === "consent_required") {
    return outcome.reviewItem
      ? "按需打开精确待决定项；批准只授权一次请求，之后仍需重新测试。"
      : "先重新读取需处理事项；找不到精确待决定项时不进行猜测跳转。";
  }
  if (controller.state.phase === "dirty") return "可先测试草稿，也可以明确保存；测试不会自动保存。";
  if (controller.state.phase === "tested") return "确认字段后明确保存；保存后仍需等待边界刷新。";
  if (controller.state.phase === "unknown")
    return controller.state.failureStage === "boundary_refresh"
      ? "使用“重新读取保存结果”重新核对精确配置与边界；未知状态下不要依赖本地确定态。"
      : "重新读取设置与边界；未知状态下不要依赖本地确定态。";
  if (controller.state.phase === "failed") return "查看返回说明，修改草稿后再测试或保存。";
  return "修改配置前先核对当前边界；需要更多信息时打开详情。";
}

export function settingsInspector(
  controller: SettingsController,
  surface: SettingsSurfaceId,
  selectedEvidence: string
): WorkbenchInspectorModel {
  if (surface === "diagnostics") {
    const diagnostics = controller.snapshot?.productDiagnostics;
    return {
      title: "产品诊断依据",
      conclusion: diagnostics
        ? `系统 canonical 产品诊断当前为 ${diagnostics.status}；该结论只读取当前产品 owner。`
        : "系统没有提供 canonical 产品诊断。",
      risk: diagnostics?.blockerCodes.length
        ? `存在 ${diagnostics.blockerCodes.length} 个系统阻断代码；不能把部分计数可见理解为产品完全可用。`
        : diagnostics
          ? "当前未报告阻断代码；精确原生与外部 live 证据仍属于独立验收层。"
          : "缺失诊断时，页面保持未知。",
      nextAction: "重新读取诊断；需要原生或外部证明时运行对应验收，而不是从网页状态推断。",
      evidence: [],
      evidenceFeedback: selectedEvidence
        ? `已选择 ${selectedEvidence}；产品诊断当前只展示 metadata-safe 状态。`
        : "产品诊断不暴露凭据、消息正文或内部执行轨迹。",
      technicalDetails: diagnostics
        ? [
            { label: "generatedAt", value: diagnostics.generatedAt },
            { label: "persistenceMode", value: diagnostics.persistenceMode },
            { label: "binaryKind", value: diagnostics.runtimeBuild.binaryKind },
            { label: "bundleIdentifier", value: diagnostics.runtimeBuild.bundleIdentifier },
            { label: "blockerCodes", value: diagnostics.blockerCodes.join(", ") || "none" },
          ]
        : [{ label: "availability", value: "unknown" }],
    };
  }
  const boundaryEnvelope = controller.effectiveBoundaryEnvelope;
  const boundary = boundaryPresentation(boundaryEnvelope);
  const result = controller.lastTestOutcome?.result;
  const receipt = result?.provider_invocation_receipt;
  const boundaryRefs = [
    ...(boundaryEnvelope.evidenceRefs ?? []),
    ...(boundaryEnvelope.data?.evidenceRefs ?? []),
  ];
  const reviewRefs = controller.lastTestOutcome?.reviewItem?.evidenceRefs ?? [];
  const safeModeRefs = controller.snapshot?.safeMode?.sourceRefs ?? [];
  const evidence = uniqueEvidence([
    ...evidenceRefs(boundaryRefs),
    ...evidenceRefs(reviewRefs),
    ...safeModeRefs.map(id => ({
      id,
      label: "安全模式来源",
      source: "LifeStateProjection",
      sensitivity: "metadata_only",
    })),
    ...(result?.network_policy_decision_id
      ? [
          {
            id: result.network_policy_decision_id,
            label: "原始网络策略决定",
            source: "policy",
            sensitivity: "metadata_only",
          },
        ]
      : []),
    ...(result?.effective_network_policy_decision_id
      ? [
          {
            id: result.effective_network_policy_decision_id,
            label: "实际网络策略决定",
            source: "policy",
            sensitivity: "metadata_only",
          },
        ]
      : []),
    ...(receipt
      ? [
          {
            id: receipt.request_id,
            label: "供应商调用终态回执",
            source: "provider",
            sensitivity: "metadata_only",
          },
        ]
      : []),
  ]);
  const draft = controller.draft;
  const diagnostics = controller.snapshot?.diagnostics ?? [];

  return {
    title: surface === "model-provider" ? "模型设置依据" : "隐私与网络依据",
    conclusion: phaseConclusion(controller),
    risk:
      controller.testPresentation?.status === "unknown"
        ? controller.testPresentation.detail
        : `${boundary.label}。${boundary.detail}`,
    nextAction: nextAction(controller),
    evidence,
    evidenceFeedback:
      selectedEvidence || evidence.length === 0
        ? selectedEvidence
          ? `已选择 ${selectedEvidence}。检查器只展示元数据引用，不展示凭据或请求正文。`
          : "当前没有系统提供的可展示证据；页面保持未知，不补造来源。"
        : undefined,
    technicalDetails: [
      { label: "configSource", value: "get_config (sanitized)" },
      {
        label: "safeModeActive",
        value: String(controller.snapshot?.safeMode?.active ?? "unknown"),
      },
      { label: "safeModeReason", value: controller.snapshot?.safeMode?.reason ?? "unknown" },
      {
        label: "safeModeSourceRefs",
        value: controller.snapshot?.safeMode?.sourceRefs.join(", ") || "none",
      },
      { label: "orchestrationPhase", value: controller.state.phase },
      { label: "draftRevision", value: String(controller.state.draftRevision) },
      { label: "savedRevision", value: String(controller.state.savedRevision ?? "none") },
      { label: "boundaryEnvelopeStatus", value: boundaryEnvelope.status },
      { label: "routeType", value: boundaryEnvelope.data?.routeType ?? "unknown" },
      {
        label: "externalTransmission",
        value: boundaryEnvelope.data?.externalTransmission ?? "unknown",
      },
      { label: "risk", value: boundaryEnvelope.data?.risk ?? "unknown" },
      {
        label: "provider",
        value: draft?.llm.provider ? settingsProviderLabels[draft.llm.provider] : "unknown",
      },
      {
        label: "endpointHost",
        value: draft ? (endpointHost(draft.llm.openai_base) ?? "invalid") : "unknown",
      },
      { label: "validationStatus", value: result?.validation_status ?? "not_tested" },
      { label: "consentStatus", value: result?.consent_status ?? "none" },
      { label: "reviewProposalId", value: result?.review_proposal_id ?? "none" },
      { label: "reviewItemId", value: controller.lastTestOutcome?.reviewItem?.id ?? "none" },
      { label: "providerRequestId", value: receipt?.request_id ?? "none" },
      { label: "providerReceiptStatus", value: receipt?.status ?? "none" },
      {
        label: "sourceDiagnostics",
        value:
          diagnostics
            .map(item => `${item.id}:${item.status}${item.message ? ` (${item.message})` : ""}`)
            .join(" | ") || "not_loaded",
      },
    ],
  };
}
