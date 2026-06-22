import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import ToolsPermissionsTab from "./ToolsPermissionsTab";

const mockConfig = {
  system: {
    safe_paths: ["/Users/tw/Desktop/open-life"],
    network_policy: {
      enabled: true,
      default_decision: "ask",
      domain_allowlist: ["example.com"],
      domain_denylist: ["blocked.example"],
    },
  },
} as any;

describe("ToolsPermissionsTab", () => {
  it("renders web, file, permission, and registry sections", () => {
    render(
      <ToolsPermissionsTab
        diagnostics={{ mcp_server_count: 1, mcp_tool_count: 2 } as any}
        config={mockConfig}
        setConfig={vi.fn()}
        toolPermissions={[
          {
            id: "perm-1",
            toolName: "web.fetch",
            source: "built-in",
            riskLevel: "medium",
            actionType: "read",
            policy: "allow_until_revoked",
            createdAt: "2026-06-22T00:00:00Z",
          },
        ]}
        revokeToolPermission={vi.fn().mockResolvedValue(true)}
        refreshAllDiagnostics={vi.fn().mockResolvedValue(null)}
        refreshSecurityState={vi.fn().mockResolvedValue(undefined)}
        toolManifests={[
          {
            id: "tool-1",
            name: "Read web",
            description: "",
            source: { type: "BuiltIn" },
            risk_level: "low",
            capabilities: ["read"],
            action_type: "read",
            enabled: true,
            declarative_only: false,
          } as any,
        ]}
      />
    );

    expect(screen.getByText(/Web 与网络权限/)).toBeInTheDocument();
    expect(screen.getAllByText(/文件访问/).length).toBeGreaterThan(0);
    expect(screen.getByText(/工具权限与确认/)).toBeInTheDocument();
    expect(screen.getByText(/工具能力清单（高级）/)).toBeInTheDocument();
    expect(screen.getAllByText(/web.fetch/).length).toBeGreaterThan(0);
  });
});
