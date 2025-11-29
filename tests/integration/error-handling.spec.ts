import { test, expect } from '../helpers/fixtures.js';

/**
 * Error Handling Tests
 *
 * Tests graceful error handling for 404 pages and invalid parameters.
 * Verifies application-level errors don't crash or show blank pages.
 * Uses authenticated page fixture (auto-login).
 */

/**
 * 404 Route Tests
 * Verifies non-existent resources return 404 status, not crashes or 500 errors
 */
test.describe('Error Handling: 404 Routes', () => {
  test('non-existent title returns 404', async ({ page }) => {
    // Navigate to book page with fake title ID
    const response = await page.goto('/book/nonexistent-id-12345');
    expect(response?.status()).toBe(404);
  });

  test('non-existent entry in reader returns 404', async ({ page }) => {
    // Get a valid title ID, but use fake entry ID
    const library = await (await page.request.get('/api/library')).json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleId = library[0].id;

    // Navigate to reader with valid title but fake entry
    const response = await page.goto(`/reader/${titleId}/fake-entry-id-12345/1`);
    expect(response?.status()).toBe(404);
  });

  test('invalid page number in reader returns 404', async ({ page }) => {
    // Get valid title and entry IDs
    const library = await (await page.request.get('/api/library')).json();

    if (library.length === 0) {
      test.skip();
      return;
    }

    const titleId = library[0].id;
    const titleDetails = await (await page.request.get(`/api/title/${titleId}`)).json();

    if (titleDetails.entries.length === 0) {
      test.skip();
      return;
    }

    const entryId = titleDetails.entries[0].id;

    // Navigate to reader with invalid page number (99999 exceeds any reasonable page count)
    const response = await page.goto(`/reader/${titleId}/${entryId}/99999`);
    expect(response?.status()).toBe(404);
  });
});

/**
 * Graceful Fallback Tests
 * Verifies invalid parameters don't crash - page loads with defaults
 */
test.describe('Error Handling: Graceful Fallbacks', () => {
  test('invalid sort parameter falls back to default', async ({ page }) => {
    // Navigate to library with invalid sort and ascend parameters
    const response = await page.goto('/library?sort=garbage&ascend=xyz');

    // Page should load successfully (not crash)
    expect(response?.status()).toBe(200);

    // Verify page actually rendered (titles grid exists)
    await page.waitForSelector('.titles-grid');
  });
});
