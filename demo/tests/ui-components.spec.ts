import { test, expect } from '@playwright/test';

test('UI概要ページが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=UI Components')).toBeVisible();
  await expect(page.locator('text=Component Categories')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/1-1.png' });
});

test('カテゴリカードが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Atoms')).toBeVisible();
  await expect(page.locator('text=Molecules')).toBeVisible();
  await expect(page.locator('text=Organisms')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/2-1.png' });
});

test('Atomsページへ遷移', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await page.locator('text=View all Atoms →').click();
  await expect(page).toHaveURL('/topo/docs/ui/atoms');
  await page.screenshot({ path: 'screenshots/ui-components/3-1.png' });
});

test('Moleculesページへ遷移', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await page.locator('text=View all Molecules →').click();
  await expect(page).toHaveURL('/topo/docs/ui/molecules');
  await page.screenshot({ path: 'screenshots/ui-components/4-1.png' });
});

test('Organismsページへ遷移', async ({ page }) => {
  await page.goto('/topo/docs/ui');
  await page.waitForLoadState('networkidle');
  await page.locator('text=View all Organisms →').click();
  await expect(page).toHaveURL('/topo/docs/ui/organisms');
  await page.screenshot({ path: 'screenshots/ui-components/5-1.png' });
});

test('Atomsページが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/atoms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Atoms')).toBeVisible();
  await expect(page.locator('text=Button')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/6-1.png' });
});

test('Buttonセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/atoms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Primary')).toBeVisible();
  await expect(page.locator('text=Secondary')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/7-1.png' });
});

test('Form Controlsセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/atoms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Form Controls')).toBeVisible();
  await expect(page.locator('text=Checkbox')).toBeVisible();
  await expect(page.locator('text=Radio')).toBeVisible();
  await expect(page.locator('text=Switch')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/8-1.png' });
});

test('Moleculesページが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/molecules');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Molecules')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/9-1.png' });
});

test('Alertセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/molecules');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Alert')).toBeVisible();
  await expect(page.locator('text=Success!')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/10-1.png' });
});

test('Tabsセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/molecules');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Tabs')).toBeVisible();
  await expect(page.locator('text=Line Style')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/11-1.png' });
});

test('Organismsページが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/organisms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Organisms')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/12-1.png' });
});

test('DataTableセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/organisms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=DataTable')).toBeVisible();
  await expect(page.locator('text=Name')).toBeVisible();
  await expect(page.locator('text=Email')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/13-1.png' });
});

test('Timelineセクションが表示される', async ({ page }) => {
  await page.goto('/topo/docs/ui/organisms');
  await page.waitForLoadState('networkidle');
  await expect(page.locator('text=Timeline')).toBeVisible();
  await expect(page.locator('text=Project Started')).toBeVisible();
  await page.screenshot({ path: 'screenshots/ui-components/14-1.png' });
});

