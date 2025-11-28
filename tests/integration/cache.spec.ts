import { test, expect } from '../helpers/fixtures.js';
import { captureEvidence } from '../helpers/test-utils.js';

/**
 * Cache Debug Page Integration Tests
 * Tests cache monitoring and control functionality
 * Covers: Cache statistics display, clear/save/load operations, real-time updates
 */

test.describe('Cache Debug Page', () => {
  test('should load cache debug page with statistics', async ({ page }) => {
    // Navigate to cache debug page
    await page.goto('/debug/cache');

    // Wait for page to load
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Verify cache statistics section is visible
    await expect(page.locator('h3:has-text("LRU Cache Statistics")')).toBeVisible();

    // Verify key statistics are displayed
    const memoryUsage = page.locator('dt:has-text("Memory Usage")');
    await expect(memoryUsage).toBeVisible();

    const hitRate = page.locator('dt:has-text("Hit Rate")');
    await expect(hitRate).toBeVisible();

    const cacheEntries = page.locator('dt:has-text("Cache Entries")');
    await expect(cacheEntries).toBeVisible();

    // Verify library cache file section
    await expect(page.locator('h3:has-text("Library Cache File")')).toBeVisible();

    // Verify cache operations section with buttons
    await expect(page.locator('h3:has-text("Cache Operations")')).toBeVisible();
    await expect(page.locator('button:has-text("Refresh Statistics")')).toBeVisible();
    await expect(page.locator('button:has-text("Save Library to Cache")')).toBeVisible();
    await expect(page.locator('button:has-text("Load Library from Cache")')).toBeVisible();
    await expect(page.locator('button:has-text("Clear All Cache")')).toBeVisible();

    // Verify cache entries table
    await expect(page.locator('h3:has-text("Recent Cache Entries")')).toBeVisible();

    await captureEvidence(page, 'cache-debug-loaded');

    console.log('✓ Cache debug page loaded successfully');
  });

  test('should display cache statistics correctly', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Check that cache status badge exists
    const statusBadge = page.locator('dt:has-text("Cache Status")').locator('..').locator('dd span.uk-badge');
    await expect(statusBadge).toBeVisible();

    // Check progress bar for memory usage
    const progressBar = page.locator('progress.uk-progress');
    await expect(progressBar).toBeVisible();

    // Verify hit rate percentage is displayed
    const hitRateText = await page.locator('dt:has-text("Hit Rate")').locator('..').locator('dd').textContent();
    expect(hitRateText).toMatch(/%/);

    await captureEvidence(page, 'cache-statistics');

    console.log('✓ Cache statistics displayed correctly');
  });

  test('should save library to cache', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Click save button
    const saveButton = page.locator('button:has-text("Save Library to Cache")');
    await saveButton.click();

    // Wait for success message (Alpine.js shows alert on success)
    const alert = page.locator('.uk-alert');
    await expect(alert).toBeVisible({ timeout: 10000 });

    // Verify success message
    const alertText = await alert.textContent();
    expect(alertText).toMatch(/success|saved/i);

    await captureEvidence(page, 'cache-save-success');

    console.log('✓ Library saved to cache successfully');
  });

  test('should load library from cache', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // First save to ensure cache file exists
    const saveButton = page.locator('button:has-text("Save Library to Cache")');
    await saveButton.click();

    // Wait for save to complete
    await page.waitForSelector('.uk-alert', { timeout: 10000 });
    await page.waitForTimeout(500); // Give it a moment to complete

    // Now load from cache
    const loadButton = page.locator('button:has-text("Load Library from Cache")');
    await loadButton.click();

    // Wait for success message
    const alert = page.locator('.uk-alert').last();
    await expect(alert).toBeVisible({ timeout: 10000 });

    const alertText = await alert.textContent();
    expect(alertText).toMatch(/success|loaded/i);

    await captureEvidence(page, 'cache-load-success');

    console.log('✓ Library loaded from cache successfully');
  });

  test('should clear cache with confirmation', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Click clear button (opens modal)
    const clearButton = page.locator('button:has-text("Clear All Cache")');
    await clearButton.click();

    // Wait for confirmation modal
    const modal = page.locator('#clear-confirm');
    await expect(modal).toBeVisible({ timeout: 5000 });

    // Verify modal content
    await expect(page.locator('h2:has-text("Clear All Cache?")')).toBeVisible();

    await captureEvidence(page, 'cache-clear-modal');

    // Click confirm button
    const confirmButton = page.locator('button.uk-button-danger.uk-modal-close:has-text("Clear Cache")');
    await confirmButton.click();

    // Wait for success message
    const alert = page.locator('.uk-alert');
    await expect(alert).toBeVisible({ timeout: 10000 });

    const alertText = await alert.textContent();
    expect(alertText).toMatch(/success|cleared/i);

    await captureEvidence(page, 'cache-cleared-success');

    console.log('✓ Cache cleared successfully');
  });

  test('should refresh statistics', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Get initial entry count
    const entryCountElement = page.locator('dt:has-text("Cache Entries")').locator('..').locator('dd');
    const initialCount = await entryCountElement.textContent();

    // Click refresh button
    const refreshButton = page.locator('button:has-text("Refresh Statistics")');
    await refreshButton.click();

    // Wait for loading state
    await page.waitForSelector('span[uk-spinner]', { state: 'visible', timeout: 2000 }).catch(() => {
      // Spinner might be too fast, that's okay
    });

    // Wait a moment for refresh to complete
    await page.waitForTimeout(1000);

    // Verify page is still showing statistics
    await expect(entryCountElement).toHaveText(/.+/);

    await captureEvidence(page, 'cache-refreshed');

    console.log(`✓ Cache statistics refreshed (was: ${initialCount})`);
  });

  test('should display cache entries table', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Verify table structure
    const table = page.locator('table.uk-table');
    await expect(table).toBeVisible();

    // Check table headers
    await expect(page.locator('th:has-text("Key")')).toBeVisible();
    await expect(page.locator('th:has-text("Size")')).toBeVisible();
    await expect(page.locator('th:has-text("Access Count")')).toBeVisible();

    // Check if there are entries or empty state
    const tbody = table.locator('tbody');
    const rows = tbody.locator('tr');
    const rowCount = await rows.count();

    if (rowCount === 1) {
      // Check for "No cache entries" message
      const emptyMessage = tbody.locator('td:has-text("No cache entries")');
      const hasEmptyMessage = await emptyMessage.count() > 0;
      if (hasEmptyMessage) {
        console.log('⊘ No cache entries to display');
      }
    } else {
      // Verify entry structure
      const firstRow = rows.first();
      const cells = firstRow.locator('td');
      expect(await cells.count()).toBeGreaterThanOrEqual(3);
      console.log(`✓ Cache entries table displayed with ${rowCount} entries`);
    }

    await captureEvidence(page, 'cache-entries-table');
  });

  test('should show cache file metadata', async ({ page }) => {
    await page.goto('/debug/cache');
    await page.waitForSelector('h2:has-text("Cache Debug")');

    // Verify library cache file section
    const fileSection = page.locator('h3:has-text("Library Cache File")').locator('..');

    // Check file path is displayed
    const filePath = fileSection.locator('dt:has-text("File Path")');
    await expect(filePath).toBeVisible();

    // Check status badge
    const status = fileSection.locator('dt:has-text("Status")');
    await expect(status).toBeVisible();

    // Status should be either "Valid" or "No cache file"
    const statusBadge = fileSection.locator('span.uk-badge');
    await expect(statusBadge).toBeVisible();

    await captureEvidence(page, 'cache-file-metadata');

    console.log('✓ Cache file metadata displayed');
  });
});
