import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
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

  it("renders security governance section", () => {
    render(<PrivacyTab {...baseProps} />);
    expect(screen.getByText(/安全治理与长期记忆/)).toBeInTheDocument();
  });

  it("renders network access policy section", () => {
    render(<PrivacyTab {...baseProps} />);
    expect(screen.getByText(/网络访问策略/)).toBeInTheDocument();
  });

  it("renders agent execution permissions section", () => {
    render(<PrivacyTab {...baseProps} />);
    expect(screen.getByText(/Agent 执行权限/)).toBeInTheDocument();
  });

  it("renders tool registry section", () => {
    render(<PrivacyTab {...baseProps} />);
    expect(screen.getByText(/Tool Registry/)).toBeInTheDocument();
  });
});
