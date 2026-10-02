import { test, expect } from '@playwright/test';

test.describe('Smoke tests', () => {
  test.describe.configure({ mode: 'serial' });
  test('logs in and opens populated Home and Library pages', async ({ page }) => {
    await page.goto('/login');
    await expect(page.locator('input[name="username"]')).toBeVisible();
    await expect(page.locator('input[name="password"]')).toBeVisible();
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.getByRole('button', { name: 'Login' }).click();

    await expect(page).toHaveURL('/');
    await expect(page.getByRole('heading', { name: 'Read your first manga' })).toBeVisible();

    await page.goto('/library');
    await expect(page.getByRole('heading', { name: 'Library' })).toBeVisible();
    await expect(page.locator('.uk-card-title').first()).toBeVisible();
  });

  test('logout invalidates access to protected pages', async ({ page }) => {
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpass123');
    await page.getByRole('button', { name: 'Login' }).click();
    await expect(page).toHaveURL('/');

    await page.goto('/logout');
    await expect(page).toHaveURL(/\/login/);
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/);
  });
});
