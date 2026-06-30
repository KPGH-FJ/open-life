import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import PrivacyTab from "./PrivacyTab";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockHotCache = { entries: 0, size_bytes: 0, max_capacity: 1000, hit_rate: 0.95 } as any;
const mockPrivacyPolicy = {
  enabled: true,
  redact_mode: "strict",
  allow_cloud_for: [],
  block_network: false,
  network_denylist: [],
} as any;
const mockConfig = {
  system: {
    network_policy: { enabled: true, allow_cloud: true, denylist: [] as string[] },
  },
} as any;

function transmissionItem(status: string, overrides: Record<string, any> = {}) {
  return {
    status,
    run_id: `run-${status}`,
    task_session_id: `task-${status}`,
    provider: status === "sent" ? "deepseek" : "ollama",
    model: status === "sent" ? "deepseek-chat" : "llama3",
    route_type:
      status === "sent"
        ? "cloud"
        : status === "blocked"
          ? "cloud"
          : status === "unknown" || status === "not_instrumented"
            ? "unknown"
            : "local",
    reason: `${status}_fixture_reason`,
    evidence_id: `evidence-${status}`,
    truth_confidence:
      status === "unknown" || status === "not_instrumented" ? "unknown" : "verified",
    data_category: "provider_transmission",
    source_refs: [{ source: "agent_run", status: "present", route_type: "local" }],
    started_at: "2026-06-29T00:00:00Z",
    finished_at: null,
    ...overrides,
  };
}

describe("PrivacyTab", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  const baseProps = {
    diagnostics: null,
    hotCache: mockHotCache,
    privacyPolicy: mockPrivacyPolicy,
    setPrivacyPolicyState: vi.fn(),
    securityLoading: false,
    securityMessage: null,
    handleExportAudit: vi.fn(),
    handleCleanupAudit: vi.fn(),
    handleRotateAuditKey: vi.fn(),
    toolPermissions: [],
    revokeToolPermission: vi.fn().mockResolvedValue(true),
    refreshAllDiagnostics: vi.fn(),
    config: mockConfig,
    setConfig: vi.fn(),
    refreshSecurityState: vi.fn(),
    toolManifests: [] as any[],
    safeMode: false,
    handleSavePrivacyPolicy: vi.fn(),
  };

  it("renders security governance section", async () => {
    render(<PrivacyTab {...baseProps} />);
    await screen.findByText(/旧 run 可能未接入/);
    expect(screen.getByText(/隐私与长期记忆/)).toBeInTheDocument();
  });

  it("renders local audit and PII policy sections", async () => {
    render(<PrivacyTab {...baseProps} />);
    await screen.findByText(/旧 run 可能未接入/);
    expect(screen.getAllByText(/本地审计/).length).toBeGreaterThan(0);
    expect(screen.getByText(/PII 与隐私策略/)).toBeInTheDocument();
  });

  it("does not render tool permissions in the privacy tab", async () => {
    render(<PrivacyTab {...baseProps} />);
    await screen.findByText(/旧 run 可能未接入/);
    expect(screen.queryByText(/工具权限与确认/)).not.toBeInTheDocument();
  });

  it("does not render tool registry in the privacy tab", async () => {
    render(<PrivacyTab {...baseProps} />);
    await screen.findByText(/旧 run 可能未接入/);
    expect(screen.queryByText(/工具能力清单（高级）/)).not.toBeInTheDocument();
  });

  it("renders all provider transmission statuses explicitly", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_provider_transmission_history") {
        return Promise.resolve([
          transmissionItem("sent"),
          transmissionItem("not_sent"),
          transmissionItem("blocked"),
          transmissionItem("unknown"),
          transmissionItem("not_instrumented"),
        ] as any);
      }
      return mockInvoke(cmd, args);
    });

    render(<PrivacyTab {...baseProps} />);

    expect(await screen.findByText(/sent · 已外发/)).toBeInTheDocument();
    expect(screen.getByText(/not_sent · 未外发/)).toBeInTheDocument();
    expect(screen.getByText(/blocked · 已阻断/)).toBeInTheDocument();
    expect(screen.getByText(/unknown · 证据不足/)).toBeInTheDocument();
    expect(screen.getByText(/not_instrumented · 旧 run 未接入/)).toBeInTheDocument();
  });

  it("renders an empty provider transmission history state", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_provider_transmission_history") {
        return Promise.resolve([] as any);
      }
      return mockInvoke(cmd, args);
    });

    render(<PrivacyTab {...baseProps} />);

    expect(await screen.findByText(/旧 run 可能未接入/)).toBeInTheDocument();
  });

  it("does not render key material from provider transmission history", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_provider_transmission_history") {
        return Promise.resolve([
          transmissionItem("sent", {
            run_id: "run-sk-secret-token",
            task_session_id: "task-token=secret-token",
            provider: "deepseek",
            model: "deepseek-chat",
            reason: "api_key=sk-provider-secret password=hunter2",
            evidence_id: "evidence-bearer sk-provider-secret",
            source_refs: [{ source: "provider_validation", status: "token=secret-token" }],
          }),
        ] as any);
      }
      return mockInvoke(cmd, args);
    });

    const { container } = render(<PrivacyTab {...baseProps} />);

    expect(await screen.findAllByText(/redacted_sensitive/)).not.toHaveLength(0);
    const text = container.textContent ?? "";
    for (const forbidden of [
      "sk-provider-secret",
      "secret-token",
      "hunter2",
      "api_key=",
      "token=",
    ]) {
      expect(text).not.toContain(forbidden);
    }
  });

  it("routes audit danger buttons to preflight handlers before final commands", async () => {
    const handleExportAudit = vi.fn().mockResolvedValue(undefined);
    const handleCleanupAudit = vi.fn().mockResolvedValue(undefined);
    const handleRotateAuditKey = vi.fn().mockResolvedValue(undefined);

    render(
      <PrivacyTab
        {...baseProps}
        handleExportAudit={handleExportAudit}
        handleCleanupAudit={handleCleanupAudit}
        handleRotateAuditKey={handleRotateAuditKey}
      />
    );
    await screen.findByText(/旧 run 可能未接入/);

    fireEvent.click(screen.getByRole("button", { name: "导出审计" }));
    fireEvent.click(screen.getByRole("button", { name: "清理旧日志" }));
    fireEvent.click(screen.getByRole("button", { name: "轮换密钥" }));

    expect(handleExportAudit).toHaveBeenCalledOnce();
    expect(handleCleanupAudit).toHaveBeenCalledOnce();
    expect(handleRotateAuditKey).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalledWith("export_mcp_audit_logs", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("cleanup_mcp_audit_logs", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("rotate_mcp_audit_key", undefined);
  });
});
