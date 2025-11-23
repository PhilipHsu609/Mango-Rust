import { test, expect, type Page } from '../helpers/fixtures.js';
import { ReaderPage } from '../helpers/page-objects.js';
import { setTheme, getThemeState } from '../helpers/theme-utils.js';
import { captureEvidence } from '../helpers/test-utils.js';

/**
 * Reader Page Theme Integration Tests
 * Tests that theme settings work correctly on the reader page
 * Addresses gap found in validation: theme tests didn't cover reader page
 */

// Helper to navigate to reader
async function navigateToTestReader(page: Page) {
  await page.goto('/library');
  const firstTitle = page.locator('.title-card').first();
  await firstTitle.waitFor({ state: 'visible', timeout: 10000 });
  await firstTitle.click();
  await page.waitForLoadState('domcontentloaded');
  const firstEntry = page.locator('.entry-card').first();
  await firstEntry.waitFor({ state: 'visible', timeout: 10000 });
  await firstEntry.click();
  const fromBeginningButton = page.getByText('FROM BEGINNING');
  await fromBeginningButton.waitFor({ state: 'visible', timeout: 10000 });
  await fromBeginningButton.click();
  await page.waitForLoadState('domcontentloaded');
}

test.describe('Reader Page Theme', () => {
  test('should load reader with light theme when light theme is set', async ({ page }) => {
    // Set light theme in library
    await page.goto('/library');
    await setTheme(page, 'light');

    // Navigate to reader
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Verify theme state
    const themeState = await getThemeState(page);
    expect(themeState.localStorage).toBe('light');

    // Verify settings button exists and is functional
    const settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    await captureEvidence(page, 'reader-light-theme');
    console.log('✓ Reader loaded with light theme');
  });

  test('should load reader with dark theme when dark theme is set', async ({ page }) => {
    // Set dark theme in library
    await page.goto('/library');
    await setTheme(page, 'dark');

    // Navigate to reader
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Verify theme state
    const themeState = await getThemeState(page);
    expect(themeState.localStorage).toBe('dark');

    // Verify settings button exists
    const settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    await captureEvidence(page, 'reader-dark-theme');
    console.log('✓ Reader loaded with dark theme');
  });

  test('should have settings button visible in both themes', async ({ page }) => {
    // Test with light theme
    await page.goto('/library');
    await setTheme(page, 'light');
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    let settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    // Click to verify it works
    await settingsBtn.click();
    let settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    // Close modal (press Escape)
    await page.keyboard.press('Escape');
    await settingsModal.waitFor({ state: 'hidden' });

    console.log('✓ Settings button works in light theme');

    // Switch to dark theme and reload reader
    await page.goto('/library');
    await setTheme(page, 'dark');
    await navigateToTestReader(page);

    await reader.verifyReaderLoaded();

    settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    // Click to verify it works in dark theme too
    await settingsBtn.click();
    settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    console.log('✓ Settings button works in dark theme');
  });

  test('should preserve theme preference across reader sessions', async ({ page }) => {
    // Set light theme
    await page.goto('/library');
    await setTheme(page, 'light');

    // Navigate to reader
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Verify light theme
    let themeState = await getThemeState(page);
    expect(themeState.localStorage).toBe('light');

    // Go back to library
    await page.goto('/library');

    // Change to dark theme
    await setTheme(page, 'dark');

    // Navigate to reader again
    await navigateToTestReader(page);
    await reader.verifyReaderLoaded();

    // Verify dark theme persisted
    themeState = await getThemeState(page);
    expect(themeState.localStorage).toBe('dark');

    console.log('✓ Theme preference preserved across reader sessions');
  });

  test('should have visible and functional settings button', async ({ page }) => {
    await page.goto('/library');
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Check settings button is visible
    const settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    // Verify button is functional - can open settings
    await settingsBtn.click();
    const settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    // Close modal
    await page.keyboard.press('Escape');
    await settingsModal.waitFor({ state: 'hidden' });

    await captureEvidence(page, 'reader-settings-button');

    console.log('✓ Reader settings button is visible and functional');
  });
});
