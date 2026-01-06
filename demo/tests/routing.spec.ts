import { test, expect } from '@playwright/test';

test.describe('Routing', () => {
  test('should display home page at /', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('text=Welcome to topo!')).toBeVisible();
  });

  test('should navigate to about page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=About');
    await expect(page).toHaveURL('/about');
    await expect(page.locator('text=About topo')).toBeVisible();
  });

  test('should navigate to users page', async ({ page }) => {
    await page.goto('/');
    await page.click('text=Users');
    await expect(page).toHaveURL('/users');
    await expect(page.locator('text=Users')).toBeVisible();
  });

  test('should navigate to user detail with dynamic route', async ({ page }) => {
    await page.goto('/users');
    await page.click('text=User 1');
    await expect(page).toHaveURL('/users/1');
    await expect(page.locator('text=User Detail')).toBeVisible();
    // Check that route param is displayed
    await expect(page.locator('text=1').first()).toBeVisible();
  });

  test('should navigate back to home', async ({ page }) => {
    await page.goto('/about');
    await page.click('text=Back to Home');
    await expect(page).toHaveURL('/');
  });

  test('should display current path', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('text=/')).toBeVisible();
  });
});
