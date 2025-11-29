import { test as baseTest, expect } from '@playwright/test';
import { test } from '../helpers/fixtures.js';
import { login, TEST_USER, REGULAR_USER } from '../helpers/auth.js';

/**
 * Admin Functionality Tests
 *
 * Tests admin functionality and permission enforcement.
 * Covers access control, library scan, user CRUD operations, and cache debug access.
 * Uses authenticated page fixture (auto-login as admin) for most tests.
 */

/**
 * Access Control Tests
 * Tests admin page access and permission enforcement
 */
test.describe('Admin: Access Control', () => {
  test('admin user can access /admin page', async ({ page }) => {
    // Navigate to admin page (TEST_USER is admin via fixture)
    const response = await page.goto('/admin');
    expect(response?.status()).toBe(200);

    // Verify page loaded with admin content
    await expect(page.locator('h2:has-text("Admin Dashboard")')).toBeVisible();
  });

  baseTest('non-admin user gets 403 for /admin', async ({ browser }) => {
    const context = await browser.newContext();
    const page = await context.newPage();

    try {
      await login(page, REGULAR_USER);

      // Try to access admin page
      const response = await page.goto('/admin');
      expect(response?.status()).toBe(403);
    } finally {
      await context.close();
    }
  });
});

/**
 * Library Scan Tests
 * Tests library scan functionality via API
 */
test.describe('Admin: Library Scan', () => {
  test('POST /api/admin/scan returns scan results', async ({ page }) => {
    const response = await page.request.post('/api/admin/scan');
    expect(response.ok()).toBe(true);

    const data = await response.json();
    expect(data).toMatchObject({
      titles: expect.any(Number),
      milliseconds: expect.any(Number),
    });

    // Verify titles and milliseconds are non-negative
    expect(data.titles).toBeGreaterThanOrEqual(0);
    expect(data.milliseconds).toBeGreaterThanOrEqual(0);
  });
});

/**
 * User CRUD Tests
 * Tests user management operations (create, read, update, delete)
 * Serial execution to avoid conflicts on shared user table
 */
test.describe.serial('Admin: User CRUD', () => {
  // Unique username for this test run
  const TEST_USERNAME = 'admin-test-user-' + Date.now();
  const TEST_PASSWORD = 'testpass123';

  test('GET /api/admin/users returns user list', async ({ page }) => {
    const response = await page.request.get('/api/admin/users');
    expect(response.ok()).toBe(true);

    const users = await response.json();
    expect(Array.isArray(users)).toBe(true);
    expect(users.length).toBeGreaterThan(0);

    // Verify user object structure
    expect(users[0]).toMatchObject({
      username: expect.any(String),
      is_admin: expect.any(Boolean),
    });

    // Verify TEST_USER (admin) exists
    const testUser = users.find((u: any) => u.username === TEST_USER.username);
    expect(testUser).toBeDefined();
    expect(testUser.is_admin).toBe(true);
  });

  test('POST /api/admin/users creates user', async ({ page }) => {
    const response = await page.request.post('/api/admin/users', {
      data: {
        username: TEST_USERNAME,
        password: TEST_PASSWORD,
        is_admin: false,
      },
    });

    expect(response.status()).toBe(201);

    // Verify user was created by fetching user list
    const listResponse = await page.request.get('/api/admin/users');
    const users = await listResponse.json();
    const createdUser = users.find((u: any) => u.username === TEST_USERNAME);

    expect(createdUser).toBeDefined();
    expect(createdUser.is_admin).toBe(false);
  });

  test('PATCH /api/admin/users/:username promotes user', async ({ page }) => {
    // Promote test user to admin
    const response = await page.request.patch(`/api/admin/users/${TEST_USERNAME}`, {
      data: { is_admin: true },
    });

    expect(response.status()).toBe(204);

    // Verify user was promoted
    const listResponse = await page.request.get('/api/admin/users');
    const users = await listResponse.json();
    const promotedUser = users.find((u: any) => u.username === TEST_USERNAME);

    expect(promotedUser).toBeDefined();
    expect(promotedUser.is_admin).toBe(true);

    // Demote back to non-admin for other tests
    const demoteResponse = await page.request.patch(`/api/admin/users/${TEST_USERNAME}`, {
      data: { is_admin: false },
    });
    expect(demoteResponse.status()).toBe(204);
  });

  test('cannot delete yourself (returns 500)', async ({ page }) => {
    // Try to delete the current user (TEST_USER)
    const response = await page.request.delete(`/api/admin/users/${TEST_USER.username}`);

    expect(response.status()).toBe(500);

    // Verify error message
    const body = await response.text();
    expect(body).toContain('Cannot delete yourself');
  });

  test('cannot demote yourself from admin (returns 500)', async ({ page }) => {
    // Try to demote the current user (TEST_USER)
    const response = await page.request.patch(`/api/admin/users/${TEST_USER.username}`, {
      data: { is_admin: false },
    });

    expect(response.status()).toBe(500);

    // Verify error message
    const body = await response.text();
    expect(body).toContain('Cannot demote yourself from admin');
  });

  test('DELETE /api/admin/users/:username deletes user', async ({ page }) => {
    // Delete test user (cleanup)
    const response = await page.request.delete(`/api/admin/users/${TEST_USERNAME}`);

    expect(response.status()).toBe(204);

    // Verify user was deleted
    const listResponse = await page.request.get('/api/admin/users');
    const users = await listResponse.json();
    const deletedUser = users.find((u: any) => u.username === TEST_USERNAME);

    expect(deletedUser).toBeUndefined();
  });
});

/**
 * Cache Debug Access Tests
 * Tests cache debug page access control
 */
test.describe('Admin: Cache Debug', () => {
  test('admin can access /debug/cache', async ({ page }) => {
    const response = await page.goto('/debug/cache');
    expect(response?.status()).toBe(200);

    // Verify cache debug page loaded
    await expect(page.locator('h2:has-text("Cache Debug")')).toBeVisible();
  });

  baseTest('non-admin gets 403 for /debug/cache', async ({ browser }) => {
    const context = await browser.newContext();
    const page = await context.newPage();

    try {
      await login(page, REGULAR_USER);

      // Try to access cache debug page
      const response = await page.goto('/debug/cache');
      expect(response?.status()).toBe(403);
    } finally {
      await context.close();
    }
  });
});
