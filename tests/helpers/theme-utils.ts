import { Page, expect } from '@playwright/test';

/**
 * Theme verification utilities for testing dark/light theme switching
 * Provides helpers for theme state verification and manipulation
 */

export type Theme = 'dark' | 'light';

/**
 * Get the current theme from both DOM class and localStorage
 * @param page - Playwright page instance
 * @returns Object with DOM theme class and localStorage theme value
 */
export async function getThemeState(page: Page): Promise<{
  domClass: Theme | null;
  localStorage: Theme | null;
}> {
  // Get theme from body class
  const bodyClass = await page.evaluate(() => {
    if (document.body.classList.contains('uk-dark')) {
      return 'dark' as const;
    } else if (document.body.classList.contains('uk-light')) {
      return 'light' as const;
    }
    return null;
  });

  // Get theme from localStorage
  const localStorageTheme = await page.evaluate(() => {
    const theme = localStorage.getItem('theme');
    return theme as Theme | null;
  });

  return {
    domClass: bodyClass,
    localStorage: localStorageTheme,
  };
}

/**
 * Verify that the page has the expected theme applied
 * Checks both body class and localStorage for consistency
 * @param page - Playwright page instance
 * @param expectedTheme - Expected theme ('dark' or 'light')
 */
export async function verifyTheme(page: Page, expectedTheme: Theme): Promise<void> {
  const state = await getThemeState(page);

  // Verify body class
  const expectedClass = expectedTheme === 'dark' ? 'uk-dark' : 'uk-light';
  const actualClass = state.domClass === 'dark' ? 'uk-dark' : 'uk-light';

  expect(actualClass, `Body should have ${expectedClass} class`).toBe(expectedClass);

  // Verify localStorage
  expect(state.localStorage, 'localStorage theme should match').toBe(expectedTheme);

  console.log(
    `✓ Theme verified: ${expectedTheme} (DOM: ${state.domClass}, localStorage: ${state.localStorage})`
  );
}

/**
 * Toggle the theme by clicking the theme toggle button
 * @param page - Playwright page instance
 * Note: Animations are disabled, so theme change is instant
 */
export async function toggleTheme(page: Page): Promise<void> {
  // Get current theme before toggle
  const beforeState = await getThemeState(page);
  const expectedAfter = beforeState.domClass === 'dark' ? 'light' : 'dark';

  // Find the theme toggle button
  const themeToggle = page.locator('a[onclick*="toggleTheme"]').first();

  await themeToggle.click();

  // Wait for actual theme change (no animation, just state change)
  await page.waitForFunction(
    (expected: string) => {
      const expectedClass = expected === 'dark' ? 'uk-dark' : 'uk-light';
      return document.body.classList.contains(expectedClass);
    },
    expectedAfter,
    { timeout: 1000 }
  );

  console.log('Theme toggled via UI button');
}

/**
 * Set theme programmatically using JavaScript
 * Useful for test setup without UI interaction
 * @param page - Playwright page instance
 * @param theme - Theme to set ('dark' or 'light')
 */
export async function setTheme(page: Page, theme: Theme): Promise<void> {
  await page.evaluate((themeValue) => {
    localStorage.setItem('theme', themeValue);
    // Apply theme immediately using the applyTheme function if available
    if (typeof (window as any).applyTheme === 'function') {
      (window as any).applyTheme(themeValue);
    }
  }, theme);

  console.log(`Theme set programmatically to: ${theme}`);
}

/**
 * Verify that theme colors are applied correctly
 * Checks computed styles of key elements
 * @param page - Playwright page instance
 * @param expectedTheme - Expected theme
 */
export async function verifyThemeColors(page: Page, expectedTheme: Theme): Promise<void> {
  // Check navbar background color
  const navbarBg = await page.evaluate(() => {
    const navbar = document.querySelector('.uk-navbar-container');
    if (!navbar) return null;
    return window.getComputedStyle(navbar).backgroundColor;
  });

  if (expectedTheme === 'dark') {
    // Dark theme should have dark navbar
    expect(navbarBg).toContain('42, 42, 42'); // #2a2a2a = rgb(42, 42, 42)
  } else {
    // Light theme should have white/light navbar
    expect(navbarBg).toMatch(/(255, 255, 255)|(250, 250, 250)/); // white or #fafafa
  }

  // Check body background
  const bodyBg = await page.evaluate(() => {
    return window.getComputedStyle(document.body).backgroundColor;
  });

  if (expectedTheme === 'dark') {
    // Dark theme should have dark body background
    expect(bodyBg).toContain('20, 20, 20'); // rgb(20, 20, 20)
  } else {
    // Light theme should have light body background
    expect(bodyBg).toMatch(/(255, 255, 255)|(250, 250, 250)/);
  }

  console.log(`✓ Theme colors verified for ${expectedTheme} theme`);
}

/**
 * Verify mutual exclusion - only one theme class should be active
 * @param page - Playwright page instance
 */
export async function verifyMutualExclusion(page: Page): Promise<void> {
  const classes = await page.evaluate(() => {
    const hasDark = document.body.classList.contains('uk-dark');
    const hasLight = document.body.classList.contains('uk-light');
    return { hasDark, hasLight };
  });

  // Exactly one should be true
  const exclusiveCount = (classes.hasDark ? 1 : 0) + (classes.hasLight ? 1 : 0);
  expect(exclusiveCount, 'Body should have exactly one theme class').toBe(1);

  console.log('✓ Mutual exclusion verified (only one theme class active)');
}

/**
 * Wait for theme change to complete
 * @param page - Playwright page instance
 * @param expectedTheme - Expected theme after change
 */
export async function waitForThemeChange(page: Page, expectedTheme: Theme): Promise<void> {
  const expectedClass = expectedTheme === 'dark' ? 'uk-dark' : 'uk-light';

  await page.waitForFunction((cls) => document.body.classList.contains(cls), expectedClass, {
    timeout: 5000,
  });

  console.log(`✓ Theme changed to ${expectedTheme}`);
}

/**
 * Get system theme preference
 * @param page - Playwright page instance
 * @returns System theme preference ('dark' or 'light')
 */
export async function getSystemTheme(page: Page): Promise<Theme> {
  const systemTheme = await page.evaluate(() => {
    if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
      return 'dark' as const;
    }
    return 'light' as const;
  });

  return systemTheme;
}
