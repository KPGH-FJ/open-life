import { describe, expect, it } from "vitest";
import type { SystemDiagnostics } from "../tauri";
import { buildCapabilityStatusViewModel, explainGovernanceBlocker } from "./capabilityStatus";

function diagnostics(overrides: Partial<SystemDiagnostics> = {}): SystemDiagnostics {
  return {
    router: {
      onnx_available: false,
      onnx_disabled: false,
      active_backend: "regex",
      latency_threshold_us: 50000,
    },
    mcp_server_count: 0,
    mcp_tool_count: 0,
    mcp_recent_audit_count: 0,
    mcp_recent_pii_count: 0,
    memory_chunk_count: 0,
    unfinished_builder_sessions: 0,
    pending_builder_review_sessions: 0,
    ollama_online: true,
    local_model: "llama3",
    resolved_local_model: "llama3:latest",
    prefer_local_model: true,
    cloud_api_configured: false,
    cloud_provider: "DeepSeek",
    cloud_api_validated: false,
    cloud_api_last_error: null,
    cloud_api_validation_status: "unconfigured",
    cloud_api_validated_at: null,
    cloud_api_failed_at: null,
    cloud_api_validation_source: null,
    chat_ready: true,
    readiness_issues: [],
    data_dir: "/tmp/openlife-test",
    active_data_dir: "/tmp/openlife-test",
    database_status: "ok",
    startup_warnings: [],
    snapshot_count: 1,
    life_model_ready: true,
    app_version: "0.1.0",
    model_empty: false,
    chat_session_count: 1,
    builder_completion: {
      identity: 80,
      goals: 80,
      capabilities: 80,
      state: 80,
      overall: 80,
      lowest_dimension: "identity",
    },
    data_files: {
      messages_db_exists: true,
      messages_db_size_mb: 0,
      vectors_db_exists: true,
      vectors_db_size_mb: 0,
      mcp_audit_db_exists: true,
      mcp_audit_db_size_mb: 0,
      config_yaml_exists: true,
      life_model_yaml_exists: true,
    },
    ollama_models: [],
    config_source: "test",
    agent_run_count: 0,
    agent_run_store_status: "ok",
    pending_proposal_count: 0,
    high_risk_pending_proposal_count: 0,
    proposal_store_status: "ok",
    ...overrides,
  };
}

describe("capabilityStatus", () => {
  it("distinguishes configured but unvalidated cloud API from validated cloud availability", () => {
    const configuredOnly = buildCapabilityStatusViewModel(
      diagnostics({
        cloud_api_configured: true,
        cloud_api_validated: false,
        cloud_api_validation_status: "unvalidated",
      }),
      0
    );
    expect(configuredOnly.modelRouteLabel).toContain("已配置，连接未验证");
    expect(configuredOnly.modelRouteLabel).not.toContain("备用");
    expect(configuredOnly.cloudApiStatusLabel).toBe("DeepSeek 已配置，连接未验证");

    const validated = buildCapabilityStatusViewModel(
      diagnostics({
        cloud_api_configured: true,
        cloud_api_validated: true,
        cloud_api_validation_status: "validated",
      }),
      0
    );
    expect(validated.modelRouteLabel).toContain("备用");
    expect(validated.cloudApiStatusLabel).toBe("DeepSeek 已验证可用");
  });

  it("shows failed and stale provider validation without treating cloud as ready", () => {
    const failed = buildCapabilityStatusViewModel(
      diagnostics({
        cloud_api_configured: true,
        cloud_api_validated: false,
        cloud_api_validation_status: "failed",
        cloud_api_last_error: "http_status:401",
      }),
      0
    );
    expect(failed.modelRouteLabel).toContain("验证失败");
    expect(failed.modelRouteLabel).not.toContain("备用");
    expect(failed.cloudApiStatusLabel).toBe("DeepSeek 验证失败：http_status:401");

    const stale = buildCapabilityStatusViewModel(
      diagnostics({
        cloud_api_configured: true,
        cloud_api_validated: false,
        cloud_api_validation_status: "stale",
      }),
      0
    );
    expect(stale.modelRouteLabel).toContain("验证已过期或配置已变更");
    expect(stale.modelRouteLabel).not.toContain("备用");
    expect(stale.cloudApiStatusLabel).toBe("DeepSeek 验证已过期或配置已变更");
  });

  it("maps exact governance blocker reasons without exposing raw reason codes", () => {
    const disallowed = explainGovernanceBlocker(
      "That tool call is blocked by governance: model_selected_disallowed_tool",
      diagnostics()
    );
    expect(disallowed).toContain("未允许的工具或目标");
    expect(disallowed).not.toContain("model_selected_disallowed_tool");

    const policy = explainGovernanceBlocker(
      "That tool call is blocked by governance: model_selected_tool_policy_blocked",
      diagnostics()
    );
    expect(policy).toContain("未通过本轮执行策略");
    expect(policy).not.toContain("model_selected_tool_policy_blocked");

    const web = explainGovernanceBlocker(
      "That read action is blocked by governance: web_network_policy_blocked",
      diagnostics()
    );
    expect(web).toContain("网络或网页读取策略阻止");
    expect(web).not.toContain("web_network_policy_blocked");
  });

  it("uses a conservative fallback for unknown governance blockers", () => {
    const message = explainGovernanceBlocker(
      "That read action is blocked by governance: custom_policy_reason",
      diagnostics()
    );

    expect(message).toContain("治理策略阻止了这次操作");
    expect(message).toContain("未执行外部工具或写入");
    expect(message).not.toContain("allowlist");
  });
});
