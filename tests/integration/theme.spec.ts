import { test, expect } from '../helpers/fixtures.js';
import {
  verifyTheme,
  toggleTheme,
  getThemeState,
  verifyThemeColors,
  verifyMutualExclusion,
  waitForThemeChange,
  setTheme,
  type Theme,
} from '../helpers/theme-utils.js';
import { NavigationComponent } from '../helpers/page-objects.js';
import { captureEvidence } from '../helpers/test-utils.js';

/**
 * Theme Toggle Integration Tests
 * Tests theme switching functionality across all pages
 * Covers: initial state, toggle, persistence, cross-page consistency
 */

test.describe('Theme Toggle', () => {
  // Note: No beforeEach hook - tests should handle their own state setup
  // Each test navigates to the appropriate page and checks/sets theme as needed

  test('should detect initial theme state on library page', async ({ page }) => {
    // Navigate to library page and wait for network idle but NOT DOMContentLoaded
    // This ensures we check the IIFE theme application before DOMContentLoaded re-applies it
    await page.goto('/library', { waitUntil: 'networkidle' });

    // CRITICAL: Check theme IMMEDIATELY after page load, before any JS executes
    // The IIFE should have applied theme to body BEFORE DOMContentLoaded
    const earlyState = await getThemeState(page);

    // Check theme state after DOMContentLoaded
    await page.waitForLoadState('domcontentloaded');
    const state = await getThemeState(page);

    // Should have either light or dark theme set
    expect(state.domClass).not.toBeNull();
    expect(state.localStorage).not.toBeNull();

    // DOM class and localStorage should match
    expect(state.domClass).toBe(state.localStorage);

    // Verify the body actually HAS the theme class applied BOTH before and after DOMContentLoaded
    // This catches bugs where the IIFE fails but DOMContentLoaded fixes it
    const body = page.locator('body');
    if (state.domClass === 'dark') {
      await expect(body).toHaveClass(/uk-dark/);
      expect(earlyState.domClass).toBe('dark'); // IIFE should have set it
    } else {
      await expect(body).toHaveClass(/uk-light/);
      expect(earlyState.domClass).toBe('light'); // IIFE should have set it
    }

    // Verify mutual exclusion
    await verifyMutualExclusion(page);

    console.log(`Initial theme detected: ${state.domClass} (early: ${earlyState.domClass})`);
  });

  test('should toggle theme on library page', async ({ page }) => {
    // Navigate to library page
    await page.goto('/library');

    // Get initial theme
    const initialState = await getThemeState(page);
    const initialTheme = initialState.domClass;
    expect(initialTheme).not.toBeNull();

    // Capture before screenshot
    await captureEvidence(page, 'theme-toggle-before');

    // Toggle theme
    await toggleTheme(page);

    // Expected theme is opposite of initial
    const expectedTheme: Theme = initialTheme === 'light' ? 'dark' : 'light';

    // Wait for theme change
    await waitForThemeChange(page, expectedTheme);

    // Capture after screenshot
    await captureEvidence(page, 'theme-toggle-after');

    // Verify new theme
    await verifyTheme(page, expectedTheme);

    // Verify theme colors applied
    await verifyThemeColors(page, expectedTheme);

    // Verify mutual exclusion
    await verifyMutualExclusion(page);
  });

  test('should persist theme across page refresh', async ({ page }) => {
    // Navigate to library page
    await page.goto('/library');

    // Set theme to dark
    await setTheme(page, 'dark');
    await waitForThemeChange(page, 'dark');
    await verifyTheme(page, 'dark');

    // Reload the page
    await page.reload();

    // Theme should still be dark after reload
    await verifyTheme(page, 'dark');

    // Verify localStorage persisted
    const darkState = await getThemeState(page);
    expect(darkState.localStorage).toBe('dark');

    // Set theme to light
    await setTheme(page, 'light');
    await waitForThemeChange(page, 'light');
    await verifyTheme(page, 'light');

    // Reload again
    await page.reload();

    // Theme should still be light
    await verifyTheme(page, 'light');

    const lightState = await getThemeState(page);
    expect(lightState.localStorage).toBe('light');
  });

  test('should maintain theme consistency across navigation', async ({ page }) => {
    // Navigate to library page
    await page.goto('/library');

    // Set theme to dark
    await setTheme(page, 'dark');
    await waitForThemeChange(page, 'dark');

    // Navigate to different pages and verify theme persists
    const pagesToTest = [
      { name: 'library', url: '/library' },
      { name: 'tags', url: '/tags' },
    ];

    let darkPageCount = 0;
    for (const pageToTest of pagesToTest) {
      await page.goto(pageToTest.url);
      await verifyTheme(page, 'dark');
      await verifyThemeColors(page, 'dark');
      darkPageCount++;
      console.log(`✓ Theme consistent on ${pageToTest.name} page`);
    }
    expect(darkPageCount).toBe(pagesToTest.length);

    // Change theme to light on one page
    await page.goto('/library');
    await setTheme(page, 'light');
    await waitForThemeChange(page, 'light');

    // Navigate to other pages and verify light theme
    let lightPageCount = 0;
    for (const pageToTest of pagesToTest) {
      await page.goto(pageToTest.url);
      await verifyTheme(page, 'light');
      await verifyThemeColors(page, 'light');
      lightPageCount++;
      console.log(`✓ Theme updated to light on ${pageToTest.name} page`);
    }
    expect(lightPageCount).toBe(pagesToTest.length);
  });

  test('should toggle theme using navigation component', async ({ page }) => {
    // Navigate to library page
    await page.goto('/library');

    const nav = new NavigationComponent(page);

    // Get initial theme
    const initialState = await getThemeState(page);
    const initialTheme = initialState.domClass;
    expect(initialTheme).not.toBeNull();

    // Toggle using NavigationComponent
    await nav.toggleTheme();

    // Expected theme
    const expectedTheme: Theme = initialTheme === 'light' ? 'dark' : 'light';

    // Wait and verify
    await waitForThemeChange(page, expectedTheme);
    await verifyTheme(page, expectedTheme);
    await verifyThemeColors(page, expectedTheme);
  });

  test('should apply correct colors for light theme', async ({ page }) => {
    await page.goto('/library');

    // Set light theme
    await setTheme(page, 'light');
    await waitForThemeChange(page, 'light');

    // Capture screenshot
    await captureEvidence(page, 'light-theme-colors');

    // Verify theme colors
    await verifyThemeColors(page, 'light');

    // Check body class
    const bodyHasLightClass = await page.evaluate(() => {
      return document.body.classList.contains('uk-light');
    });
    expect(bodyHasLightClass).toBe(true);
  });

  test('should apply correct colors for dark theme', async ({ page }) => {
    await page.goto('/library');

    // Set dark theme
    await setTheme(page, 'dark');
    await waitForThemeChange(page, 'dark');

    // Capture screenshot
    await captureEvidence(page, 'dark-theme-colors');

    // Verify theme colors
    await verifyThemeColors(page, 'dark');

    // Check body class
    const bodyHasDarkClass = await page.evaluate(() => {
      return document.body.classList.contains('uk-dark');
    });
    expect(bodyHasDarkClass).toBe(true);
  });

  test('should work on home page', async ({ page }) => {
    await page.goto('/');

    // Toggle theme
    const initialState = await getThemeState(page);
    const initialTheme = initialState.domClass;
    expect(initialTheme).not.toBeNull();

    await toggleTheme(page);

    const expectedTheme: Theme = initialTheme === 'light' ? 'dark' : 'light';
    await waitForThemeChange(page, expectedTheme);
    await verifyTheme(page, expectedTheme);
  });

  test('should prevent theme class conflicts', async ({ page }) => {
    await page.goto('/library');

    // Verify only one theme class is active
    await verifyMutualExclusion(page);
    const state1 = await getThemeState(page);
    expect(state1.domClass).not.toBeNull();

    // Toggle theme
    await toggleTheme(page);
    const expectedTheme1: Theme = state1.domClass === 'light' ? 'dark' : 'light';
    await waitForThemeChange(page, expectedTheme1);

    // Verify still only one theme class
    await verifyMutualExclusion(page);
    const state2 = await getThemeState(page);
    expect(state2.domClass).toBe(expectedTheme1);

    // Toggle again
    await toggleTheme(page);
    const expectedTheme2: Theme = expectedTheme1 === 'light' ? 'dark' : 'light';
    await waitForThemeChange(page, expectedTheme2);

    // Verify mutual exclusion maintained
    await verifyMutualExclusion(page);
    const state3 = await getThemeState(page);
    expect(state3.domClass).toBe(expectedTheme2);
  });
});
