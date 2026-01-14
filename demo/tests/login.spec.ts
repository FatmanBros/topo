import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => {
  await page.goto('/topo/login');
  await page.waitForLoadState('networkidle');
});

test('ログインフォームが表示される', async ({ page }) => {
  await expect(page.locator('[data-field="LoginFormCard.email"]')).toBeVisible();
  await expect(page.locator('[data-field="LoginFormCard.password"]')).toBeVisible();
  await expect(page.locator('button[type="submit"]')).toBeVisible();
  await page.screenshot({ path: 'screenshots/login/1-1.png' });
});

test('空のメールでバリデーションエラー', async ({ page }) => {
  await page.locator('[data-field="LoginFormCard.email"]').fill('');
  await page.locator('button[type="submit"]').click();
  await expect(page.locator('[data-error="LoginFormCard.emailError"]')).toBeVisible();
  await page.screenshot({ path: 'screenshots/login/2-1.png' });
});

test('正常なログインフロー', async ({ page }) => {
  await page.screenshot({ path: 'screenshots/login/3-1.png' });
  await page.locator('[data-field="LoginFormCard.email"]').fill('test@example.com');
  await page.locator('[data-field="LoginFormCard.password"]').fill('password123');
  await page.screenshot({ path: 'screenshots/login/3-2.png' });
  await expect(page.locator('[data-field="LoginFormCard.email"]')).toHaveValue('test@example.com');
  await expect(page.locator('[data-field="LoginFormCard.password"]')).toHaveValue('password123');
  await page.screenshot({ path: 'screenshots/login/3-3.png' });
});

test.skip('未実装テスト', async ({ page }) => {
  await expect(page.locator('[data-field="LoginFormCard.email"]')).toBeDisabled();
});

