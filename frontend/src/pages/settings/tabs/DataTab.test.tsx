import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import DataTab from "./DataTab";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("DataTab", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  const baseProps = {
    handleExport: vi.fn(),
    handleImport: vi.fn(),
    exportLoading: false,
    importLoading: false,
    safeMode: false,
    diagnostics: null as any,
    evolutionLoading: false,
    evolutionResult: null,
    setEvolutionLoading: vi.fn(),
    setEvolutionResult: vi.fn(),
    tierLoading: false,
    tierResult: null,
    setTierLoading: vi.fn(),
    setTierResult: vi.fn(),
    handleExportDiagnostics: vi.fn(),
  };

  it("renders data migration section", () => {
    render(<DataTab {...baseProps} />);
    expect(screen.getByText(/数据导出/)).toBeInTheDocument();
    expect(screen.getByText("导出全部数据")).toBeInTheDocument();
    expect(screen.getByText("导入覆盖备份")).toBeInTheDocument();
  });

  it("renders maintenance section with buttons", () => {
    render(<DataTab {...baseProps} />);
    expect(screen.getByText(/高级维护/)).toBeInTheDocument();
    expect(screen.getByText("生成进化报告")).toBeInTheDocument();
    expect(screen.getByText("运行记忆层级维护")).toBeInTheDocument();
    expect(screen.getByText("导出诊断报告")).toBeInTheDocument();
  });

  it("disables import button in safe mode", () => {
    render(<DataTab {...baseProps} safeMode={true} />);
    const importBtn = screen.getByText("导入覆盖备份");
    expect(importBtn).toBeDisabled();
  });

  it("calls handleExport on export button click", () => {
    const handleExport = vi.fn();
    render(<DataTab {...baseProps} handleExport={handleExport} />);
    fireEvent.click(screen.getByText("导出全部数据"));
    expect(handleExport).toHaveBeenCalledOnce();
  });

  it("shows loading state when exporting", () => {
    render(<DataTab {...baseProps} exportLoading={true} />);
    expect(screen.getAllByText("导出中...")).toHaveLength(2);
  });

  it("shows evolution result when present", () => {
    render(<DataTab {...baseProps} evolutionResult="已应用规则 2 条" />);
    expect(screen.getByText("已应用规则 2 条")).toBeInTheDocument();
  });

  it("requires confirmation before running memory tier maintenance", () => {
    render(<DataTab {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "运行记忆层级维护" }));
    expect(screen.getByRole("dialog", { name: "确认运行记忆层级维护" })).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("run_memory_tier_maintenance", undefined);

    fireEvent.click(screen.getByRole("button", { name: "运行维护" }));
    expect(invoke).toHaveBeenCalledWith("run_memory_tier_maintenance", undefined);
  });
});
