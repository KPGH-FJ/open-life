import { expect, test } from "@playwright/test";

const ERROR_BOUNDARY_HEADING = "界面暂时无法继续";

const CANONICAL_ROUTES = [
  { path: "/workspace", heading: "工作区", mode: "product" },
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

  for (const path of ["/today", "/tasks", "/review"]) {
    test(`${path} stays retired instead of duplicating Workbench`, async ({ page }) => {
      await page.goto(`/#${path}`);
      await expect(
        page.getByRole("heading", { name: "这个旧页面已从产品中移除", level: 1 })
      ).toBeVisible();
    });
  }

  test("Workbench keeps conversation, Work, results, and decisions on one product surface", async ({
    page,
  }) => {
    await page.goto("/#/workspace");
    await expect(page.getByRole("button", { name: /^Workbench/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /^对话/ })).toHaveCount(0);
    await expect(page.getByRole("button", { name: /^结果/ })).toHaveCount(0);
    await expect(page.getByRole("button", { name: /^需处理/ })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "工作区", level: 1 })).toBeVisible();
    await expect(page).toHaveURL(/#\/workspace$/);
  });

  test("narrow Workbench keeps core navigation and Settings keyboard reachable", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 520, height: 760 });
    await page.goto("/#/workspace");

    await expect(page.getByRole("button", { name: /^Workbench/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /^个人智能/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "设置" })).toBeVisible();

    await page.getByRole("button", { name: "设置" }).focus();
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/#\/settings$/);
    await expect(page.getByRole("heading", { name: "模型与供应商", level: 1 })).toBeVisible();
  });

  test("skip link moves keyboard focus to the canonical Workbench main region", async ({
    page,
  }) => {
    await page.goto("/#/workspace");
    await page.keyboard.press("Tab");
    const skipLink = page.getByRole("link", { name: "跳到主工作区" });
    await expect(skipLink).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("#ol-shell-main")).toBeFocused();
  });
});
