import { test, expect } from '../helpers/fixtures.js';
import { BookPage, ReaderPage } from '../helpers/page-objects.js';

/**
 * End-to-End Workflow Tests
 *
 * Verifies that the reader JavaScript automatically saves progress when navigating pages.
 * Tests the full integration from UI navigation through to persistence in the database.
 */

/**
 * Helper: Get valid title and entry IDs from library
 */
async function getValidIds(page: any): Promise<{ titleId: string; entryId: string; entryName: string } | null> {
  const library = await (await page.request.get('/api/library')).json();

  if (library.length === 0) {
    return null;
  }

  const titleId = library[0].id;
  const titleDetails = await (await page.request.get(`/api/title/${titleId}`)).json();

  if (titleDetails.entries.length === 0) {
    return null;
  }

  const entry = titleDetails.entries[0];
  return {
    titleId,
    entryId: entry.id,
    entryName: entry.title,
  };
}

test.describe('End-to-End: Reader Progress Auto-Save', () => {
  test('navigating pages in reader auto-saves progress', async ({ page }) => {
    const ids = await getValidIds(page);

    if (!ids) {
      test.skip();
      return;
    }

    const { titleId, entryId, entryName } = ids;

    // Clear existing progress to start clean
    await page.request.post(`/api/progress/${titleId}/${entryId}`, {
      data: { page: 0 },
    });

    // Navigate to reader via UI (not direct URL) - tests full integration
    const bookPage = new BookPage(page);
    await bookPage.navigate(titleId);
    await bookPage.clickEntry(entryName);
    await bookPage.clickFromBeginning();

    // Verify reader loaded at page 1
    const reader = new ReaderPage(page);
    await reader.verifyReaderLoaded();
    expect(await reader.getCurrentPage()).toBe(1);

    // Navigate 5 pages using reader controls
    for (let i = 0; i < 5; i++) {
      await reader.navigateNextPage('click');
      // Small delay to let saveProgress() JavaScript execute
      await page.waitForTimeout(100);
    }

    // Verify we navigated to page 6
    expect(await reader.getCurrentPage()).toBe(6);

    // Wait for async save to complete
    await page.waitForTimeout(300);

    // CRITICAL VERIFICATION: Check progress was auto-saved by reader JavaScript
    const response = await page.request.get(`/api/progress/${titleId}/${entryId}`);
    expect(response.ok()).toBe(true);

    const progressData = await response.json();

    // Verify progress shows page 6 (started at 1, navigated 5 times)
    expect(progressData.page).toBe(6);
  });
});
