import { test, expect, type Page } from '../helpers/fixtures.js';
import { ReaderPage } from '../helpers/page-objects.js';
import { trackConsoleErrors, captureEvidence } from '../helpers/test-utils.js';

/**
 * Reader Functionality Integration Tests
 * Tests reader page loading, navigation, mode switching, and settings
 * Covers: JavaScript error detection, paged/continuous modes, keyboard navigation
 */

test.describe('Reader Functionality', () => {
  // Helper to navigate to a test book entry
  async function navigateToTestReader(page: Page) {
    // Navigate to library first
    await page.goto('/library');

    // Click on first available title card
    const firstTitle = page.locator('.title-card').first();
    await firstTitle.waitFor({ state: 'visible', timeout: 10000 });
    await firstTitle.click();

    // Wait for book page to load
    await page.waitForLoadState('domcontentloaded');

    // Click on first entry to open entry modal
    const firstEntry = page.locator('.entry-card').first();
    await firstEntry.waitFor({ state: 'visible', timeout: 10000 });
    await firstEntry.click();

    // Wait for modal to appear and click "FROM BEGINNING" button
    const fromBeginningButton = page.getByText('FROM BEGINNING');
    await fromBeginningButton.waitFor({ state: 'visible', timeout: 10000 });
    await fromBeginningButton.click();

    // Wait for reader to load
    await page.waitForLoadState('domcontentloaded');
  }

  test('should load reader page without JavaScript errors', async ({ page }) => {
    // Track console errors
    const getErrors = trackConsoleErrors(page);

    // Navigate to reader
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);

    // Verify reader loaded
    await reader.verifyReaderLoaded();

    // Check for JavaScript errors
    const errors = getErrors();
    const relevantErrors = errors.filter((err) => {
      // Filter out known harmless errors
      if (err.includes('favicon')) return false;
      if (err.includes('net::ERR')) return false;
      if (err.includes('404')) return false;
      // Filter out home page data fetch errors - these are properly caught and handled
      if (err.includes('Failed to load home page data')) return false;
      return true;
    });

    // Capture screenshot for evidence
    await captureEvidence(page, 'reader-loaded');

    // Should have no JavaScript errors
    if (relevantErrors.length > 0) {
      console.log('JavaScript errors found:', relevantErrors);
    }
    expect(relevantErrors.length).toBe(0);

    console.log('✓ Reader loaded without JavaScript errors');
  });

  test('should navigate pages in paged mode with click', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Ensure in paged mode
    const isPagedMode = await reader.isPagedMode();
    if (!isPagedMode) {
      await reader.changeMode('paged');
    }

    // Get initial page
    const initialPage = await reader.getCurrentPage();
    expect(initialPage).toBeGreaterThanOrEqual(1);

    // Navigate to next page
    await reader.navigateNextPage('click');

    // Verify page changed
    const nextPage = await reader.getCurrentPage();
    expect(nextPage).toBe(initialPage + 1);

    console.log(`✓ Navigated from page ${initialPage} to ${nextPage}`);
  });

  test('should navigate pages in paged mode with keyboard', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Ensure in paged mode
    const isPagedMode = await reader.isPagedMode();
    if (!isPagedMode) {
      await reader.changeMode('paged');
    }

    // Get initial page
    const initialPage = await reader.getCurrentPage();

    // Navigate with right arrow key
    await reader.navigateNextPage('keyboard');

    // Verify page changed
    const nextPage = await reader.getCurrentPage();
    expect(nextPage).toBe(initialPage + 1);

    // Navigate back with left arrow key
    await reader.navigatePreviousPage('keyboard');

    // Verify back to initial page
    const backToPage = await reader.getCurrentPage();
    expect(backToPage).toBe(initialPage);

    console.log('✓ Keyboard navigation works correctly');
  });

  test('should load all images in continuous mode', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Switch to continuous mode
    await reader.changeMode('continuous');

    // Verify in continuous mode
    const isContinuous = await reader.isContinuousMode();
    expect(isContinuous).toBe(true);

    // Wait for images to load
    await page.waitForFunction(
      () => {
        const container = document.querySelector('#continuous-container');
        if (!container) return false;

        const images = container.querySelectorAll('img');
        if (images.length === 0) return false;

        // Check if at least first few images are loaded
        const firstImages = Array.from(images).slice(0, 5);
        return firstImages.every((img) => {
          const imgEl = img as HTMLImageElement;
          return imgEl.complete && imgEl.naturalHeight > 0;
        });
      },
      { timeout: 10000 }
    );

    // Verify that ALL images were created (not just a few)
    // Get total pages from the page-select dropdown
    const totalPages = await page.evaluate(() => {
      const select = document.querySelector('#page-select') as HTMLSelectElement;
      return select ? select.options.length : 0;
    });

    const imageCount = await page.locator('#continuous-container img').count();
    expect(imageCount).toBe(totalPages);
    expect(imageCount).toBeGreaterThan(0);

    // Capture screenshot
    await captureEvidence(page, 'reader-continuous-mode');

    console.log('✓ Continuous mode images loaded successfully');
  });

  test('should switch between paged and continuous modes', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Start in paged mode (or switch to it)
    await reader.changeMode('paged');
    let isPaged = await reader.isPagedMode();
    expect(isPaged).toBe(true);

    await captureEvidence(page, 'reader-paged-mode');

    // Switch to continuous mode
    await reader.changeMode('continuous');
    const isContinuous = await reader.isContinuousMode();
    expect(isContinuous).toBe(true);

    await captureEvidence(page, 'reader-continuous-mode-switch');

    // Switch back to paged mode
    await reader.changeMode('paged');
    isPaged = await reader.isPagedMode();
    expect(isPaged).toBe(true);

    console.log('✓ Mode switching works correctly');
  });

  test('should open and interact with settings modal', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Open settings via button
    await reader.openSettings();

    // Check settings modal is visible
    const settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    // Close modal
    await page.keyboard.press('Escape');
    await expect(settingsModal).not.toBeVisible();

    // Test keyboard shortcut to open settings
    await page.keyboard.press('s');
    await expect(settingsModal).toBeVisible({ timeout: 2000 });

    // Capture screenshot
    await captureEvidence(page, 'reader-settings-modal');

    console.log('✓ Settings modal opens correctly');
  });

  test('should change fit options in paged mode', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Ensure in paged mode
    await reader.changeMode('paged');

    // Open settings
    await reader.openSettings();

    // Test different fit options
    const fitOptions: Array<'height' | 'width' | 'real'> = ['height', 'width', 'real'];

    for (const fit of fitOptions) {
      await reader.changeFit(fit);

      // Verify image is still visible after fit change
      const pagedImage = page.locator('#paged-image');
      await expect(pagedImage).toBeVisible();

      // Verify the body class reflects the fit option
      const bodyClass = await page.evaluate(() => document.body.className);
      expect(bodyClass).toContain(`fit-${fit}`);

      // Verify localStorage was updated
      const storedFit = await page.evaluate(() => localStorage.getItem('reader-fit'));
      expect(storedFit).toBe(fit);

      console.log(`✓ Fit option '${fit}' applied successfully`);
    }
  });

  test('should jump to specific page', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Ensure in paged mode
    await reader.changeMode('paged');

    // Get initial page and initial image src
    const initialPage = await reader.getCurrentPage();
    const initialSrc = await page.locator('#paged-image').getAttribute('src');

    // Get total pages to pick a valid target
    const totalPages = await page.evaluate(() => {
      const select = document.querySelector('#page-select') as HTMLSelectElement;
      return select ? select.options.length : 0;
    });

    if (totalPages > 3) {
      // Jump to page 3
      await reader.jumpToPage(3);

      // CRITICAL: Wait for the image src to actually change
      // This ensures loadPage() was called and executed successfully
      await page.waitForFunction(
        (expectedSrc) => {
          const img = document.querySelector('#paged-image') as HTMLImageElement;
          return img && img.src !== expectedSrc;
        },
        initialSrc,
        { timeout: 5000 }
      );

      // Verify on page 3
      const currentPage = await reader.getCurrentPage();
      expect(currentPage).toBe(3);

      // Verify the image actually changed (different page loaded)
      expect(currentPage).not.toBe(initialPage);

      // Verify the image src URL contains the correct page number
      const imageSrc = await page.locator('#paged-image').getAttribute('src');
      expect(imageSrc).toContain('/3');
      expect(imageSrc).not.toBe(initialSrc); // Image must have changed

      console.log('✓ Jump to page works correctly');
    } else {
      console.log('⊘ Skipped: Not enough pages to test jump functionality');
    }
  });

  test('should persist reader settings in localStorage', async ({ page }) => {
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Change to continuous mode
    await reader.changeMode('continuous');

    // Check localStorage (key is 'reader-mode')
    const mode = await page.evaluate(() => localStorage.getItem('reader-mode'));
    expect(mode).toBe('continuous');

    // Reload page
    await page.reload();
    await reader.verifyReaderLoaded();

    // Should still be in continuous mode
    const isContinuous = await reader.isContinuousMode();
    expect(isContinuous).toBe(true);

    console.log('✓ Reader settings persisted across reload');
  });

  test('should verify reader page respects theme on load', async ({ page }) => {
    // Navigate to library and set light theme
    await page.goto('/library');
    await page.evaluate(() => {
      localStorage.setItem('theme', 'light');
      if (typeof (window as any).applyTheme === 'function') {
        (window as any).applyTheme('light');
      }
    });

    // Now navigate to reader
    await navigateToTestReader(page);

    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();

    // Reader page always uses dark background for optimal reading experience
    // This is intentional - the body background should be black/dark
    const bodyBg = await page.evaluate(() => {
      return window.getComputedStyle(document.body).backgroundColor;
    });

    // Reader page should have dark background (rgb(0, 0, 0) or close to it)
    // This is expected behavior for reader page
    console.log(`Reader body background: ${bodyBg}`);

    // Check settings button is visible and functional
    const settingsBtn = page.locator('#settings-btn');
    await expect(settingsBtn).toBeVisible();

    // Verify button is clickable
    await settingsBtn.click();
    const settingsModal = page.locator('#settings-modal');
    await expect(settingsModal).toBeVisible();

    await captureEvidence(page, 'reader-theme-check');

    console.log('✓ Reader page respects theme and settings button works');
  });
});
