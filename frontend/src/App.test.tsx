import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  PublicProductSurfaceId,
  ProductWorkbenchRouteState,
} from "@/ui/journeys/productWorkbench";
import App from "./App";

vi.mock("@/ui/journeys/productWorkbench", async importOriginal => {
  const actual = await importOriginal<typeof import("@/ui/journeys/productWorkbench")>();
  return {
    ...actual,
    ProductWorkbenchJourney: ({
      initialMode,
      initialSurface,
      onRouteChange,
    }: {
      initialMode: ProductWorkbenchRouteState["mode"];
      initialSurface: PublicProductSurfaceId;
      onRouteChange: (route: ProductWorkbenchRouteState) => void;
    }) => (
      <div data-testid="production-workbench" data-mode={initialMode} data-surface={initialSurface}>
        <button
          type="button"
          onClick={() => onRouteChange({ mode: "product", surface: "workspace" })}
        >
          前往工作区
        </button>
        <button
          type="button"
          onClick={() => onRouteChange({ mode: "settings", surface: initialSurface })}
        >
          打开设置
        </button>
      </div>
    ),
  };
});

function renderPath(pathname: string, state?: Record<string, unknown>) {
  return render(
    <MemoryRouter
      initialEntries={[{ pathname, state }]}
      future={{ v7_startTransition: true, v7_relativeSplatPath: true }}
    >
      <App />
    </MemoryRouter>
  );
}

describe("production App route authority", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    ["/workspace", "product", "workspace"],
    ["/life-model", "product", "life-model"],
  ] as const)("maps %s to the canonical %s/%s workbench state", (path, mode, surface) => {
    renderPath(path);

    expect(screen.getByTestId("production-workbench")).toHaveAttribute("data-mode", mode);
    expect(screen.getByTestId("production-workbench")).toHaveAttribute("data-surface", surface);
  });

  it("redirects only the root entry to Workbench", async () => {
    renderPath("/");

    await waitFor(() =>
      expect(screen.getByTestId("production-workbench")).toHaveAttribute(
        "data-surface",
        "workspace"
      )
    );
  });

  it("keeps Settings as a utility context and preserves the product return surface", () => {
    renderPath("/settings", { returnSurface: "workspace" });

    expect(screen.getByTestId("production-workbench")).toHaveAttribute("data-mode", "settings");
    expect(screen.getByTestId("production-workbench")).toHaveAttribute("data-surface", "workspace");
  });

  it("updates the URL-driven workbench when internal navigation requests a product route", async () => {
    renderPath("/workspace");

    fireEvent.click(screen.getByRole("button", { name: "前往工作区" }));
    await waitFor(() =>
      expect(screen.getByTestId("production-workbench")).toHaveAttribute(
        "data-surface",
        "workspace"
      )
    );
  });

  it("opens Settings without losing the current product return surface", async () => {
    renderPath("/workspace");

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    await waitFor(() => {
      expect(screen.getByTestId("production-workbench")).toHaveAttribute("data-mode", "settings");
      expect(screen.getByTestId("production-workbench")).toHaveAttribute(
        "data-surface",
        "workspace"
      );
    });
  });

  it.each([
    "/today",
    "/tasks",
    "/review",
    "/companion",
    "/mailbox",
    "/runs",
    "/runs/task-1",
    "/builder",
    "/mcp",
  ])("shows an explicit retired state for %s without redirecting", path => {
    renderPath(path);

    expect(screen.getByRole("heading", { name: "这个旧页面已从产品中移除" })).toBeInTheDocument();
    expect(screen.getByText(path)).toBeInTheDocument();
    expect(screen.queryByTestId("production-workbench")).not.toBeInTheDocument();
  });

  it("shows an explicit unavailable state for an unknown path", () => {
    renderPath("/unknown-product-path");

    expect(screen.getByRole("heading", { name: "OpenLife 没有这个产品页面" })).toBeInTheDocument();
    expect(screen.queryByTestId("production-workbench")).not.toBeInTheDocument();
  });
});
