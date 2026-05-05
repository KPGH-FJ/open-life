import { test, expect } from '@playwright/test';

test.describe('OpenLife Smoke Test', () => {
  test('1. 应用启动', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await expect(page.locator('body')).toBeVisible();
  });

  test('2. 发送消息', async ({ page }) => {
    await page.goto('http://localhost:5173');
    // Wait for app to be ready
    await page.waitForSelector('[data-testid="chat-input"]', { timeout: 10000 });
    await page.fill('[data-testid="chat-input"]', 'Hello');
    await page.click('[data-testid="send-button"]');
    // Wait for assistant response (with timeout for model generation)
    await expect(page.locator('[data-testid="assistant-message"]').first())
      .toBeVisible({ timeout: 30000 });
  });

  test('3. 导航到 Builder', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await page.click('text=Builder');
    await expect(page.locator('[data-testid="life-model-editor"]')).toBeVisible();
  });

  test('4. 导航到 Settings', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await page.click('text=Settings');
    await expect(page.locator('text=Provider Configuration')).toBeVisible();
  });

  test('5. 导航到 Review', async ({ page }) => {
    await page.goto('http://localhost:5173');
    await page.click('text=Review');
    await expect(page.locator('[data-testid="proposal-list"]')).toBeVisible();
  });
});
