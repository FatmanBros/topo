import { test, expect } from '@playwright/test';

test.describe('Login Form', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/login');
  });

  test('should display login form', async ({ page }) => {
    await expect(page.locator('text=Sign In').first()).toBeVisible();
    await expect(page.locator('input[type="email"]')).toBeVisible();
    await expect(page.locator('input[type="password"]')).toBeVisible();
  });

  test('should show validation error for empty email', async ({ page }) => {
    const emailInput = page.locator('input[type="email"]');
    await emailInput.fill('');
    await emailInput.blur();
    // Validation should show error
    await expect(page.locator('text=メールアドレス is required')).toBeVisible();
  });

  test('should show validation error for invalid email', async ({ page }) => {
    const emailInput = page.locator('input[type="email"]');
    await emailInput.fill('invalid-email');
    await emailInput.blur();
    await expect(page.locator('text=メールアドレス must be a valid email')).toBeVisible();
  });

  test('should show validation error for short password', async ({ page }) => {
    const passwordInput = page.locator('input[type="password"]');
    await passwordInput.fill('short');
    await passwordInput.blur();
    await expect(page.locator('text=パスワード must be at least 8 characters')).toBeVisible();
  });

  test('should clear error when valid input is provided', async ({ page }) => {
    const emailInput = page.locator('input[type="email"]');

    // First enter invalid
    await emailInput.fill('invalid');
    await emailInput.blur();

    // Then enter valid
    await emailInput.fill('valid@example.com');
    await emailInput.blur();

    // Error should be gone
    await expect(page.locator('text=メールアドレス must be a valid email')).not.toBeVisible();
  });

  test('should update debug info with email value', async ({ page }) => {
    const emailInput = page.locator('input[type="email"]');
    await emailInput.fill('test@example.com');

    // Debug info should show the email
    await expect(page.locator('text=test@example.com')).toBeVisible();
  });
});
