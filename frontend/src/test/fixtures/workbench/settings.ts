import type {
  AppConfig,
  CloudProviderId,
  EvidenceRef,
  LlmConnectionTestResult,
  ProviderConnectionsViewModel,
  ProviderPrivacyBoundarySummary,
  ReviewAction,
  ReviewItem,
  ViewModelEnvelope,
} from "@/tauri";
import type { SettingsDataSource, SettingsSnapshot } from "@/features/settings/settingsDataSource";
import { cloneSettingsConfig } from "@/features/settings/settingsPresentation";
import type { WorkbenchFixtureId } from "./readOnly";

export const providerTestReviewItemId = "review-provider-connection-deepseek";
const providerTestProposalId = "proposal-provider-connection-deepseek";
const generatedAt = "2026-07-21T04:15:00.000Z";

export type ProviderTestFixtureStage = "pending" | "approved" | "rejected" | "deferred";

const boundaryEvidence: EvidenceRef = {
  id: "provider-boundary:fixture:local-ollama",
  label: "模型传输边界记录",
  source: "provider",
  sensitivity: "local_private",
};

const policyEvidence: EvidenceRef = {
  id: "network-policy:fixture:provider-probe",
  label: "供应商连接网络策略",
  source: "settings",
  sensitivity: "local_private",
};

const localBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "local",
  externalTransmission: "not_sent",
  providerLabel: "本机模型服务",
  modelLabel: "qwen2.5:14b",
  privacyLabel: "仅本机处理",
  risk: "none",
  localOnlyRequired: true,
  evidenceRefs: [boundaryEvidence],
};

const possibleCloudBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "cloud",
  externalTransmission: "possible",
  providerLabel: "DeepSeek",
  modelLabel: "deepseek-chat",
  privacyLabel: "连接需要后端策略确认",
  risk: "medium",
  localOnlyRequired: false,
  blockedReason: "尚未建立本次供应商请求的终态回执。",
  evidenceRefs: [policyEvidence],
};

const unknownBoundary: ProviderPrivacyBoundarySummary = {
  routeType: "unknown",
  externalTransmission: "unknown",
  providerLabel: "未知",
  modelLabel: "未知",
  privacyLabel: "保存后边界读取不完整",
  risk: "unknown",
  localOnlyRequired: false,
  blockedReason: "静态样例模拟保存命令返回后边界仍未知。",
  evidenceRefs: [],
};

function configForFixture(id: WorkbenchFixtureId): AppConfig {
  const external = [
    "fixture-settings-review-required",
    "fixture-settings-refresh-unknown",
    "fixture-settings-save-failed",
  ].includes(id);
  return {
    prefer_local_model: !external,
    local_model: "qwen2.5:14b",
    system: {
      network_policy: {
        enabled: true,
        default_decision: "allow",
        domain_allowlist: [],
        domain_denylist: [],
        tool_overrides: {},
      },
    },
  };
}

type FixtureProviderRoute = {
  providerId: CloudProviderId;
  endpoint: string;
  modelId: string;
};

function providerForFixture(id: WorkbenchFixtureId): FixtureProviderRoute {
  const external = [
    "fixture-settings-review-required",
    "fixture-settings-refresh-unknown",
    "fixture-settings-save-failed",
  ].includes(id);
  return external
    ? {
        providerId: "deepseek",
        endpoint: "https://api.deepseek.com",
        modelId: "deepseek-chat",
      }
    : {
        providerId: "custom",
        endpoint: "http://127.0.0.1:11434/v1",
        modelId: "qwen2.5:14b",
      };
}

function boundaryEnvelope(
  boundary: ProviderPrivacyBoundarySummary,
  status: "ready" | "stale" = "ready"
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data: boundary,
    status,
    lastUpdatedAt: status === "stale" ? "2026-07-17T04:15:00.000Z" : generatedAt,
    source: "backend-readmodel",
    evidenceRefs: boundary.evidenceRefs,
    warnings:
      status === "stale"
        ? [
            {
              code: "fixture.settings_boundary_stale",
              message: "The settings boundary fixture is stale.",
              severity: "warning",
              evidenceRefs: boundary.evidenceRefs,
            },
          ]
        : [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function providerReviewAction(
  kind: "approve" | "reject" | "later" | "view_evidence",
  enabled = true,
  disabledReason?: string
): ReviewAction {
  const labels = {
    approve: "仅允许本次",
    reject: "拒绝",
    later: "稍后处理",
    view_evidence: "查看访问范围",
  } as const;
  return {
    id: `${providerTestReviewItemId}:${kind}`,
    label: labels[kind],
    kind,
    effect: kind === "view_evidence" ? "evidence_only" : "decision_only",
    enabled,
    ...(enabled ? {} : { disabledReason: disabledReason ?? "当前动作不可用。" }),
    requiresConfirmation: kind === "approve",
    targetReviewItemId: providerTestReviewItemId,
    expectedMaterializationStatusAfterDispatch: "not_applicable",
    completionProofAfterDispatch: false,
  } as ReviewAction;
}

export function providerTestReviewItem(stage: ProviderTestFixtureStage): ReviewItem {
  const pending = stage === "pending" || stage === "deferred";
  return {
    id: providerTestReviewItemId,
    type: "tool_permission",
    source: {
      kind: "proposal",
      proposalId: providerTestProposalId,
      proposalSource: "settings_provider_connection_test",
      sourceDetail: "验证 DeepSeek deepseek-chat 的一次外部连接",
    },
    status: stage,
    materializationStatus: "not_applicable",
    decisionContext: {
      reviewItemId: providerTestReviewItemId,
      title: "允许一次模型连接测试",
      summary: "OpenLife 请求向 api.deepseek.com 发送一次最小连接验证。",
      before: {
        kind: "text",
        summary: "请求尚未发送，供应商可用性未知",
        sensitivity: "local_private",
        truncated: false,
      },
      after: {
        kind: "text",
        summary: "仅允许这一次精确的模型验证请求",
        sensitivity: "sensitive",
        truncated: false,
      },
      reasonSummary: "确认当前 API 地址、模型与凭据能否完成一次受控调用。",
      sourceSummary: "来自模型与供应商设置中的明确“测试连接”操作。",
      impactSummary: "批准只建立一次授权；不会自动发送请求、保存设置或改变默认网络策略。",
      affectedObjectLabels: ["DeepSeek", "api.deepseek.com", "deepseek-chat"],
      expiresAt: "首次精确匹配后失效",
      permission: {
        status: "ready",
        scopeKind: "network_policy",
        policy: "allow_once",
        toolLabel: "测试模型连接",
        toolName: "provider.deepseek",
        capabilityLabels: ["发送一次最小模型验证请求"],
        requestedTargetLabel: "api.deepseek.com",
        resolvedTargetLabel: "HTTPS api.deepseek.com",
        purposeSummary: "只验证 deepseek-chat 的当前连接配置。",
        scopeDigest: "sha256:fixture-provider-probe-scope",
        requestDigestKind: "endpoint",
        requestDigest: "sha256:fixture-provider-probe-request",
        requestLengthBytes: 128,
        networkPolicyDecisionId: "network-decision:fixture:provider-probe",
        transmissionBoundary: {
          externalTransmission: "possible",
          summary: "批准并重新测试后，可能向 DeepSeek 发送一次最小请求。",
          targetLabel: "api.deepseek.com",
          evidenceRefs: [policyEvidence],
        },
        expiresAt: "首次精确匹配后失效",
        revocationSummary: "拒绝会终止本次授权；未使用授权不会延续到其他测试。",
        missingFields: [],
        evidenceRefs: [policyEvidence],
      },
      evidenceRefs: [policyEvidence],
    },
    allowedActions: pending
      ? [
          providerReviewAction("approve"),
          providerReviewAction("reject"),
          providerReviewAction("later", stage !== "deferred", "这项请求已经设为稍后处理。"),
          providerReviewAction("view_evidence"),
        ]
      : [providerReviewAction("view_evidence")],
    risk: "medium",
    expiresAt: "首次精确匹配后失效",
    evidenceRefs: [policyEvidence],
    targetRefs: [
      { id: "api.deepseek.com", kind: "external_resource", label: "DeepSeek API" },
      { id: "network-decision:fixture:provider-probe", kind: "policy", label: "网络策略决定" },
    ],
  };
}

function validatedResult(route: FixtureProviderRoute): LlmConnectionTestResult {
  return {
    ok: true,
    provider: route.providerId,
    message: "连接成功，当前模型完成了一次受控验证。",
    validation_status: "validated",
    network_policy_decision_id: "network-decision:fixture:provider-probe",
    effective_network_policy_decision_id: "network-decision:fixture:provider-probe:effective",
    consent_status: "allow_once_consumed",
    permission_id: "permission:fixture:provider-probe:once",
    provider_invocation_receipt: {
      request_id: "provider-request:fixture:settings-test",
      provider: route.providerId,
      model: route.modelId,
      status: "completed",
      started_at: generatedAt,
      finished_at: "2026-07-21T04:15:01.000Z",
      simulated: false,
    },
  };
}

function settingsSnapshot(
  id: WorkbenchFixtureId,
  config: AppConfig,
  saved: boolean
): SettingsSnapshot {
  const stale = id === "fixture-stale";
  const boundary =
    id === "fixture-settings-refresh-unknown" && saved
      ? unknownBoundary
      : id === "fixture-settings-review-required" || id === "fixture-settings-save-failed"
        ? possibleCloudBoundary
        : localBoundary;
  return {
    config: cloneSettingsConfig(config),
    boundaryEnvelope: boundaryEnvelope(boundary, stale ? "stale" : "ready"),
    safeMode: { active: false, reason: "", sourceRefs: [] },
    toolPermissionEnvelope: {
      data: {
        items: [],
        totalCount: 0,
        activeCount: 0,
        revocableCount: 0,
        contractLimitations: [],
      },
      status: "empty",
      lastUpdatedAt: "2026-07-21T04:15:00.000Z",
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    },
    diagnostics: [
      { id: "sanitized_config", status: "loaded" },
      { id: "provider_privacy_boundary", status: "loaded" },
      { id: "life_state_projection", status: "loaded" },
      { id: "review_item_resolution", status: "not_requested" },
    ],
  };
}

function providerConnectionsFixture(route: FixtureProviderRoute): ProviderConnectionsViewModel {
  return {
    connections: [
      {
        id: "fixture-provider-connection",
        providerId: route.providerId,
        displayName: route.providerId === "deepseek" ? "DeepSeek" : "Fixture Provider",
        endpoint: route.endpoint,
        credentialState: "stored",
        validationState: "unverified",
        models: [
          {
            profileId: "fixture-provider-profile",
            modelId: route.modelId,
            displayName: route.modelId,
            selected: true,
            validationState: "unverified",
          },
        ],
      },
    ],
  };
}

export function createSettingsFixture(id: WorkbenchFixtureId): {
  dataSource: SettingsDataSource;
  currentReviewItem: () => ReviewItem | null;
  dispatchReviewAction: (action: ReviewAction) => boolean;
} {
  let config = configForFixture(id);
  let provider = providerForFixture(id);
  let saved = false;
  let reviewStage: ProviderTestFixtureStage = "pending";

  return {
    dataSource: {
      async loadSettings() {
        if (id === "fixture-error") throw new Error("fixture_settings_load_failed");
        return settingsSnapshot(id, config, saved);
      },
      async testSavedProviderConnection() {
        if (id === "fixture-error") throw new Error("fixture_settings_test_failed");
        if (id === "fixture-stale") {
          return {
            result: {
              ok: false,
              provider: provider.providerId,
              message: "请求已经开始，但静态样例没有可信远端终态。",
              validation_status: "remote_unknown",
              network_policy_decision_id: "network-decision:fixture:remote-unknown",
              consent_status: "not_required",
              provider_invocation_receipt: {
                request_id: "provider-request:fixture:remote-unknown",
                provider: provider.providerId,
                model: provider.modelId,
                status: "remote_unknown",
                started_at: generatedAt,
                finished_at: "2026-07-21T04:15:03.000Z",
                simulated: false,
              },
            },
            reviewItem: null,
            reviewResolution: "not_requested",
          };
        }
        if (id === "fixture-settings-review-required" && reviewStage !== "approved") {
          const result: LlmConnectionTestResult = {
            ok: false,
            provider: "DeepSeek",
            message: "需要先明确批准一次模型网络连接；批准前不会发送请求。",
            validation_status: reviewStage === "rejected" ? "blocked" : "consent_required",
            network_policy_decision_id: "network-decision:fixture:provider-probe",
            consent_status: reviewStage === "rejected" ? "blocked" : "pending_review",
            ...(reviewStage === "rejected" ? {} : { review_proposal_id: providerTestProposalId }),
          };
          return {
            result,
            reviewItem: reviewStage === "rejected" ? null : providerTestReviewItem(reviewStage),
            reviewResolution: reviewStage === "rejected" ? "not_requested" : "resolved",
          };
        }
        return {
          result: validatedResult(provider),
          reviewItem: null,
          reviewResolution: "not_requested",
        };
      },
      async loadProviderConnections() {
        return providerConnectionsFixture(provider);
      },
      async saveProviderConnection(input) {
        provider = {
          providerId: input.providerId,
          endpoint: input.endpoint,
          modelId: input.modelId,
        };
        return providerConnectionsFixture(provider);
      },
      async deleteProviderConnection() {
        return { connections: [] };
      },
      async saveSettings(next) {
        if (id === "fixture-settings-save-failed") {
          throw new Error("fixture_settings_save_failed");
        }
        config = cloneSettingsConfig(next);
        saved = true;
      },
    },
    currentReviewItem() {
      return id === "fixture-settings-review-required" ? providerTestReviewItem(reviewStage) : null;
    },
    dispatchReviewAction(action) {
      if (action.targetReviewItemId !== providerTestReviewItemId) return false;
      if (action.kind === "approve") reviewStage = "approved";
      else if (action.kind === "reject") reviewStage = "rejected";
      else if (action.kind === "later") reviewStage = "deferred";
      else throw new Error("fixture_provider_review_action_unsupported");
      return true;
    },
  };
}
