import { render, screen, fireEvent, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import ProductShell from "./ProductShell";

function renderShell(path = "/today") {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <ProductShell diagnostics={null} safeMode={false} safeModeReason="">
        <div data-testid="shell-content" />
      </ProductShell>
    </MemoryRouter>
  );
}

describe("ProductShell navigation IA", () => {
  it("renders the Sprint 6B primary navigation in order", () => {
    renderShell("/today");

    const primaryNav = screen.getByRole("navigation", {
      name: "Primary product navigation",
    });
    const links = within(primaryNav).getAllByRole("link");

    expect(links.map(link => link.textContent)).toEqual([
      "Today",
      "Companion",
      "Mailbox",
      "Life Model",
      "Runs",
      "Settings",
    ]);
    expect(links.map(link => link.getAttribute("href"))).toEqual([
      "/today",
      "/companion",
      "/mailbox",
      "/life-model",
      "/runs",
      "/settings",
    ]);
  });

  it.each(["/mailbox"])("marks Mailbox active for %s", path => {
    renderShell(path);

    expect(screen.getByRole("link", { name: "Mailbox" })).toHaveAttribute("href", "/mailbox");
    expect(screen.getByRole("link", { name: "Mailbox" })).toHaveAttribute("aria-current", "page");
  });

  it.each(["/life-model", "/life-model/build", "/memory"])(
    "marks Life Model active for %s secondary surfaces",
    path => {
      renderShell(path);

      expect(screen.getByRole("link", { name: "Life Model" })).toHaveAttribute(
        "aria-current",
        "page"
      );
    }
  );

  it("keeps MCP, A2A, metrics, calibration, and maintenance surfaces in Advanced", () => {
    renderShell("/companion");

    expect(screen.queryByRole("link", { name: "MCP / Tools" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "A2A" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Metrics" })).not.toBeInTheDocument();
    expect(screen.queryByRole("link", { name: "Calibration" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

    const advancedNav = screen.getByRole("navigation", {
      name: "Advanced technical navigation",
    });
    expect(within(advancedNav).getByText("Technical surfaces")).toBeInTheDocument();
    expect(within(advancedNav).getByText("Maintenance")).toBeInTheDocument();
    expect(within(advancedNav).queryByText("Stage / debug / eval")).not.toBeInTheDocument();

    for (const [label, path] of [
      ["MCP / Tools", "/mcp"],
      ["A2A", "/a2a"],
      ["Metrics", "/metrics"],
      ["Calibration", "/calibration"],
      ["Versions", "/versions"],
    ] as const) {
      expect(within(advancedNav).getByRole("link", { name: label })).toHaveAttribute("href", path);
    }

    expect(within(advancedNav).queryByRole("link", { name: "Runs" })).not.toBeInTheDocument();
    expect(within(advancedNav).queryByRole("link", { name: "Settings" })).not.toBeInTheDocument();
  });
});
