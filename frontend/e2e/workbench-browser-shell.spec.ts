import { expect, test } from "@playwright/test";

const ERROR_BOUNDARY_HEADING = "界面暂时无法继续";

const CANONICAL_ROUTES = [
  { path: "/today", heading: "今日", mode: "product" },
  { path: "/workspace", heading: "工作区", mode: "product" },
  { path: "/tasks", heading: "任务", mode: "product" },
  { path: "/review", heading: "审核中心", mode: "product" },
  { path: "/life-model", heading: "关于我与 Agent 记忆", mode: "product" },
  { path: "/settings", heading: "模型与供应商", mode: "settings" },
] as const;

test.describe("OpenLife Workbench browser shell", () => {
  for (const route of CANONICAL_ROUTES) {
    test(`${route.path} renders its canonical Workbench surface`, async ({ page }) => {
      const pageErrors: string[] = [];
      page.on("pageerror", error => pageErrors.push(error.message));

      await page.goto(`/#${route.path}`);

      await expect(page.locator(".ol-workbench-shell")).toHaveAttribute(
        "data-shell-mode",
        route.mode
      );
      await expect(page.getByRole("heading", { name: route.heading, level: 1 })).toBeVisible();
      await expect(page.getByText(ERROR_BOUNDARY_HEADING, { exact: true })).toHaveCount(0);
      expect(pageErrors, `${route.path} raised an uncaught browser error`).toEqual([]);
    });
  }

  test("retired Builder route stays unavailable without redirecting", async ({ page }) => {
    const pageErrors: string[] = [];
    page.on("pageerror", error => pageErrors.push(error.message));

    await page.goto("/#/builder");

    await expect(page).toHaveURL(/#\/builder$/);
    await expect(
      page.getByRole("heading", { name: "这个旧页面已从产品中移除", level: 1 })
    ).toBeVisible();
    await expect(page.getByText("/builder", { exact: true })).toBeVisible();
    await expect(page.getByText(ERROR_BOUNDARY_HEADING, { exact: true })).toHaveCount(0);
    expect(pageErrors, "/builder raised an uncaught browser error").toEqual([]);
  });

  test("removed Chat route stays unavailable without compatibility fallback", async ({ page }) => {
    const pageErrors: string[] = [];
    page.on("pageerror", error => pageErrors.push(error.message));

    await page.goto("/#/chat");

    await expect(page).toHaveURL(/#\/chat$/);
    await expect(
      page.getByRole("heading", { name: "OpenLife 没有这个产品页面", level: 1 })
    ).toBeVisible();
    await expect(page.getByText("/chat", { exact: true })).toBeVisible();
    await expect(page.getByText(ERROR_BOUNDARY_HEADING, { exact: true })).toHaveCount(0);
    expect(pageErrors, "/chat raised an uncaught browser error").toEqual([]);
  });
});
