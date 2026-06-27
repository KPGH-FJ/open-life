import { test, expect } from "@playwright/test";

const APP_URL = "http://localhost:5173";

test.describe("OpenLife Smoke Test", () => {
  test("1. 应用启动", async ({ page }) => {
    await page.goto(APP_URL);
    await expect(page.locator("body")).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "Primary product navigation" })
    ).toBeVisible();
  });

  test("2. Chat composer renders", async ({ page }) => {
    await page.goto(`${APP_URL}/#/chat`);
    await expect(page.getByTestId("chat-input")).toBeVisible({ timeout: 10_000 });
    await page.getByTestId("chat-input").fill("Hello");
    await expect(page.getByTestId("send-button")).toBeEnabled();
  });

  test("3. Builder page renders", async ({ page }) => {
    await page.goto(`${APP_URL}/#/builder`);
    await expect(page.getByRole("heading", { name: "人生模型构建" })).toBeVisible();
  });

  test("4. Settings page renders", async ({ page }) => {
    await page.goto(`${APP_URL}/#/settings`);
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  });

  test("5. Review inbox renders", async ({ page }) => {
    await page.goto(`${APP_URL}/#/mailbox`);
    await expect(page.getByTestId("mailbox-page")).toBeVisible();
  });
});
