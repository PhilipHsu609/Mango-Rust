import { test, expect } from '@playwright/test';
import { login, logout, TEST_USER, REGULAR_USER } from '../helpers/auth.js';

/**
 * Basic Authentication Flow Tests
 *
 * IMPORTANT: These tests use raw Playwright test, NOT the auto-login fixture
 * from helpers/fixtures.ts, because we're testing authentication boundaries
 * (unauthenticated → authenticated transitions).
 */

test.describe('Authentication: Basic Flows', () => {
  test('valid login redirects to home page', async ({ page }) => {
    // Navigate to login page
    await page.goto('/login');
    expect(page.url()).toContain('/login');

    // Fill in valid credentials
    await page.fill('input[name="username"]', TEST_USER.username);
    await page.fill('input[name="password"]', TEST_USER.password);

    // Submit form
    await page.click('button[type="submit"]');

    // Should redirect to home page
    await page.waitForURL('/');
    expect(page.url()).not.toContain('/login');
    expect(page.url()).toMatch(/\/$/);
  });

  test('invalid credentials shows error message', async ({ page }) => {
    // Navigate to login page
    await page.goto('/login');

    // Fill in invalid credentials (wrong password)
    await page.fill('input[name="username"]', TEST_USER.username);
    await page.fill('input[name="password"]', 'wrongpassword123');

    // Submit form
    await page.click('button[type="submit"]');

    // Should stay on login page
    await page.waitForLoadState('domcontentloaded');
    expect(page.url()).toContain('/login');

    // Should show error alert
    await expect(page.locator('.uk-alert-danger')).toBeVisible();
    await expect(page.locator('.uk-alert-danger')).toContainText('Invalid');
  });

  test('logout clears session and redirects to login', async ({ page }) => {
    // First, login
    await login(page);
    expect(page.url()).not.toContain('/login');

    // Now logout
    await logout(page);

    // Should be on login page
    expect(page.url()).toContain('/login');

    // Try to access a protected route
    await page.goto('/library');
    await page.waitForLoadState('domcontentloaded');

    // Should redirect back to login (session cleared)
    expect(page.url()).toContain('/login');
  });

  test('session persists across page reloads', async ({ page }) => {
    // Login
    await login(page);
    const homeUrl = page.url();
    expect(homeUrl).not.toContain('/login');

    // Reload the page
    await page.reload();
    await page.waitForLoadState('domcontentloaded');

    // Should still be on home page (not redirected to login)
    expect(page.url()).not.toContain('/login');
    expect(page.url()).toBe(homeUrl);
  });

  test('session persists across navigation', async ({ page }) => {
    // Login
    await login(page);
    expect(page.url()).not.toContain('/login');

    // Navigate to library page
    await page.goto('/library');
    await page.waitForLoadState('domcontentloaded');
    expect(page.url()).not.toContain('/login');
    expect(page.url()).toContain('/library');

    // Navigate to tags page
    await page.goto('/tags');
    await page.waitForLoadState('domcontentloaded');
    expect(page.url()).not.toContain('/login');
    expect(page.url()).toContain('/tags');

    // Navigate back to home
    await page.goto('/');
    await page.waitForLoadState('domcontentloaded');
    expect(page.url()).not.toContain('/login');
  });

  test('protected route redirects unauthenticated user to login', async ({ page }) => {
    // Try to access /library without logging in first
    await page.goto('/library');
    await page.waitForLoadState('domcontentloaded');

    // Should be redirected to login page
    expect(page.url()).toContain('/login');

    // Try /tags as well
    await page.goto('/tags');
    await page.waitForLoadState('domcontentloaded');
    expect(page.url()).toContain('/login');
  });
});

/**
 * Authorization and Role-Based Access Tests
 *
 * Tests authorization enforcement (AdminOnly extractor) and multi-user session isolation.
 * Uses separate browser contexts to simulate multiple concurrent users.
 */

test.describe('Authorization: Role-Based Access', () => {
  test('unauthenticated API request redirects to login', async ({ page }) => {
    // Make API request without authentication
    const response = await page.request.get('/api/library');

    // Current behavior: require_auth middleware redirects ALL requests (including API)
    // to /login with a 302, not a 401 Unauthorized
    expect(response.status()).toBe(200); // After following redirect
    expect(response.url()).toContain('/login');
  });

  test('non-admin user cannot access /admin (403)', async ({ page }) => {
    // Login as regular (non-admin) user
    await login(page, REGULAR_USER);
    expect(page.url()).not.toContain('/login');

    // Try to access admin route
    const response = await page.goto('/admin');
    await page.waitForLoadState('domcontentloaded');

    // AdminOnly extractor returns 403 Forbidden
    expect(response?.status()).toBe(403);

    // Should see the error message
    const bodyText = await page.textContent('body');
    expect(bodyText).toContain('Admin access required');
  });

  test('admin user can access /admin (200)', async ({ page }) => {
    // Login as admin user
    await login(page, TEST_USER);
    expect(page.url()).not.toContain('/login');

    // Navigate to admin route
    const response = await page.goto('/admin');
    await page.waitForLoadState('domcontentloaded');

    // Should successfully access admin page
    expect(response?.status()).toBe(200);
    expect(page.url()).toContain('/admin');

    // Should NOT see error message
    const bodyText = await page.textContent('body');
    expect(bodyText).not.toContain('Admin access required');
    expect(bodyText).not.toContain('403');
  });

  test('separate sessions maintain isolation', async ({ browser }) => {
    // Create two separate browser contexts (different users, different sessions)
    const adminContext = await browser.newContext();
    const regularContext = await browser.newContext();

    const adminPage = await adminContext.newPage();
    const regularPage = await regularContext.newPage();

    try {
      // Login admin user in first context
      await login(adminPage, TEST_USER);
      expect(adminPage.url()).not.toContain('/login');

      // Login regular user in second context
      await login(regularPage, REGULAR_USER);
      expect(regularPage.url()).not.toContain('/login');

      // Verify admin can access /admin
      const adminResponse = await adminPage.goto('/admin');
      await adminPage.waitForLoadState('domcontentloaded');
      expect(adminResponse?.status()).toBe(200);
      expect(adminPage.url()).toContain('/admin');

      // Verify regular user CANNOT access /admin
      const regularResponse = await regularPage.goto('/admin');
      await regularPage.waitForLoadState('domcontentloaded');
      expect(regularResponse?.status()).toBe(403);

      const regularBody = await regularPage.textContent('body');
      expect(regularBody).toContain('Admin access required');

      // Verify both sessions are still valid (not corrupted by each other)
      // Admin can still access /library
      await adminPage.goto('/library');
      await adminPage.waitForLoadState('domcontentloaded');
      expect(adminPage.url()).toContain('/library');

      // Regular user can still access /library
      await regularPage.goto('/library');
      await regularPage.waitForLoadState('domcontentloaded');
      expect(regularPage.url()).toContain('/library');

    } finally {
      await adminContext.close();
      await regularContext.close();
    }
  });
});

/**
 * OPDS Authentication Tests
 *
 * Tests RFC 7235 compliant HTTP Basic Auth for OPDS endpoints.
 * OPDS paths return 401 + WWW-Authenticate header for e-reader compatibility.
 * Session auth also works for browser-based OPDS testing.
 */

test.describe('Authorization: OPDS Basic Auth', () => {
  test('OPDS without credentials returns 401 with WWW-Authenticate header', async ({ page }) => {
    // Make OPDS request without authentication
    const response = await page.request.get('/opds');

    // Should return 401 Unauthorized per RFC 7235
    expect(response.status()).toBe(401);

    // Should include WWW-Authenticate header for e-reader auth prompts
    const wwwAuth = response.headers()['www-authenticate'];
    expect(wwwAuth).toBeDefined();
    expect(wwwAuth).toContain('Basic');
    expect(wwwAuth).toContain('realm');
  });

  test('OPDS with valid Basic Auth returns XML catalog', async ({ page }) => {
    // Encode credentials for Basic Auth header
    const credentials = Buffer.from(`${TEST_USER.username}:${TEST_USER.password}`).toString('base64');

    // Request OPDS with valid HTTP Basic Auth credentials
    const response = await page.request.get('/opds', {
      headers: {
        'Authorization': `Basic ${credentials}`,
      },
    });

    // Should successfully return OPDS catalog
    expect(response.status()).toBe(200);

    // Verify it's actually OPDS XML, not HTML
    const contentType = response.headers()['content-type'];
    expect(contentType).toContain('application/atom+xml');
    expect(contentType).toContain('opds-catalog');
  });

  test('OPDS with invalid credentials returns 401', async ({ page }) => {
    // Encode invalid credentials for Basic Auth header
    const credentials = Buffer.from(`${TEST_USER.username}:wrongpassword123`).toString('base64');

    // Request OPDS with invalid HTTP Basic Auth credentials
    const response = await page.request.get('/opds', {
      headers: {
        'Authorization': `Basic ${credentials}`,
      },
    });

    // Invalid credentials should return 401 (not redirect)
    expect(response.status()).toBe(401);

    // Should still include WWW-Authenticate header for retry
    const wwwAuth = response.headers()['www-authenticate'];
    expect(wwwAuth).toBeDefined();
    expect(wwwAuth).toContain('Basic');
  });

  test('OPDS with valid session auth also works (browser support)', async ({ page }) => {
    // Login via browser session
    await login(page, TEST_USER);

    // Access OPDS using session cookie (no Basic Auth header)
    const response = await page.goto('/opds');
    await page.waitForLoadState('domcontentloaded');

    // Should successfully load OPDS via session auth
    expect(response?.status()).toBe(200);
    expect(page.url()).toContain('/opds');

    // Verify we got OPDS XML content (browser may HTML-encode it)
    const content = await page.content();
    expect(content).toContain('feed');
    expect(content).toContain('opds-catalog');
    expect(content).toContain('urn:mango');
  });
});
