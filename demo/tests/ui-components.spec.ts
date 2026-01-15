import { test, expect } from '@playwright/test';

test('UI概要ページが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=UI Components').first()).toBeVisible();
  await expect(page.locator('text=Component Categories').first()).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/1-1.png' });
});

test('カテゴリカードが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Atoms').first()).toBeVisible();
  await expect(page.locator('text=Molecules').first()).toBeVisible();
  await expect(page.locator('text=Organisms').first()).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/2-1.png' });
});

