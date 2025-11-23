import { test as base, type Page } from '@playwright/test';
import { clearBrowserState } from './test-utils.js';
import { setTheme } from './theme-utils.js';
import { login } from './auth.js';

/**
 * Test fixtures for common test scenarios
 * Provides consistent starting points and automatic cleanup
 *
 * IMPORTANT: All fixtures automatically handle authentication
 */

/**
 * Extended test type with custom fixtures
 */
type CustomFixtures = {
  authenticatedPage: Page;
  cleanPage: Page;
  lightThemePage: Page;
  darkThemePage: Page;
  libraryPage: Page;
};

/**
 * Extend Playwright test with automatic authentication
 * All page fixtures log in automatically before providing the page to tests
 */
export const test = base.extend<CustomFixtures>({
  /**
   * Override default page fixture to automatically login before every test
   * This ensures all tests have an authenticated session
   */
  page: async ({ page: basePage }, use) => {
    // Login before providing page to tests
    await login(basePage);

    // Provide authenticated page to test
    await use(basePage);

    // Cleanup happens automatically
  },

  /**
   * Authenticated page fixture - alias for page
   * (Page is already authenticated via the page override above)
   */
  authenticatedPage: async ({ page }, use) => {
    await use(page);
  },

  /**
   * Clean page fixture - page with cleared browser state
   * Use this when you want a fresh start without any stored data
   */
  cleanPage: async ({ page }, use) => {
    // Page is already authenticated (via page override)
    // Navigate to home page, then clear browser state
    await page.goto('/');
    await clearBrowserState(page);

    await use(page);
  },

  /**
   * Light theme page fixture - page with light theme pre-set
   * Useful for testing theme-specific functionality
   */
  lightThemePage: async ({ page }, use) => {
    // Page is already authenticated
    await page.goto('/');
    await setTheme(page, 'light');

    await use(page);
  },

  /**
   * Dark theme page fixture - page with dark theme pre-set
   * Useful for testing theme-specific functionality
   */
  darkThemePage: async ({ page }, use) => {
    // Page is already authenticated
    await page.goto('/');
    await setTheme(page, 'dark');

    await use(page);
  },

  /**
   * Library page fixture - page already navigated to /library
   * Saves navigation boilerplate in library-focused tests
   */
  libraryPage: async ({ page }, use) => {
    // Page is already authenticated
    await page.goto('/library');
    await page.waitForLoadState('domcontentloaded');

    await use(page);
  },
});

/**
 * Export expect for convenience
 */
export { expect } from '@playwright/test';
